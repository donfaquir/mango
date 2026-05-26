//! Download a remote URL and persist it as a local [`Asset`].
//!
//! Unlike [`super::import`], this module:
//! - Does NOT enforce the image-extension whitelist (video mp4 is valid).
//! - Does NOT enforce the 50 MiB size cap.
//! - Infers the file extension from the HTTP `Content-Type` header.
//! - Is designed for async callers (the runner calling through
//!   `ResultMaterializer`).

use std::path::PathBuf;

use rusqlite::params;
use uuid::Uuid;

use crate::asset::thumbnail;
use crate::error::{CoreError, Result};
use crate::models::asset::Asset;
use crate::paths;

/// Download a result URL and persist it as a local asset file + DB row.
///
/// Returns the newly created [`Asset`].
pub async fn download_to_asset(
    db: &tokio_rusqlite::Connection,
    project_id: &str,
    shot_id: Option<&str>,
    url: &str,
    asset_type: &str, // "image" | "video" | "audio"
) -> Result<Asset> {
    // 1) Resolve project root from DB.
    let pid = project_id.to_string();
    let project_root: PathBuf = db
        .call(move |conn| {
            conn.query_row(
                "SELECT root_path FROM project WHERE id = ?1",
                params![pid],
                |r| r.get::<_, String>(0),
            )
            .map(PathBuf::from)
        })
        .await
        .map_err(|e| match e {
            tokio_rusqlite::Error::Error(inner) => match inner {
                rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
                    entity: "project",
                    id: project_id.to_string(),
                },
                other => CoreError::Sqlite(other),
            },
            other => CoreError::TaskEngine(format!("db worker error: {other}")),
        })?;

    paths::ensure_project_layout(&project_root)?;

    // 2) HTTP GET the result URL.
    let resp = reqwest::get(url)
        .await
        .map_err(|e| CoreError::Provider(format!("download failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(CoreError::Provider(format!(
            "download HTTP {}: result url may have expired",
            resp.status()
        )));
    }
    let ext = ext_from_content_type(resp.headers(), asset_type);
    let asset_id = Uuid::new_v4().to_string();
    let file_name = format!("{asset_id}.{ext}");
    let file_relative = format!("{}/{file_name}", paths::ASSETS_SUBDIR);
    let dest = paths::assets_dir(&project_root).join(&file_name);

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| CoreError::Provider(format!("download body: {e}")))?;
    let file_size = bytes.len() as i64;

    // Write file (blocking IO, but the file is typically small <100MB).
    std::fs::write(&dest, &bytes)?;

    // 3) Generate thumbnail for images.
    let thumb_relative = if asset_type == "image" {
        let thumb_name = format!("{asset_id}_thumb.webp");
        let thumb_dest = paths::thumbnails_dir(&project_root).join(&thumb_name);
        match thumbnail::generate_with_metadata(&dest, &thumb_dest) {
            Ok(_) => Some(format!("{}/{thumb_name}", paths::THUMBNAILS_SUBDIR)),
            Err(e) => {
                tracing::warn!("thumbnail for remote result failed: {e}");
                None
            }
        }
    } else {
        None
    };

    // 4) INSERT asset row.
    let insert_id = asset_id.clone();
    let insert_pid = project_id.to_string();
    let insert_shot = shot_id.map(|s| s.to_string());
    let insert_type = asset_type.to_string();
    let insert_file_rel = file_relative;
    let insert_thumb = thumb_relative;
    let insert_size = file_size;
    let original_name = format!("generated.{ext}");

    let asset: Asset = db
        .call(move |conn| {
            let insert_result = conn.execute(
                "INSERT INTO asset \
                    (id, project_id, shot_id, asset_type, original_name, \
                     file_path, thumbnail_path, file_size, content_hash, metadata_json) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL)",
                params![
                    insert_id,
                    insert_pid,
                    insert_shot,
                    insert_type,
                    original_name,
                    insert_file_rel,
                    insert_thumb,
                    insert_size,
                ],
            );
            match insert_result {
                Ok(_) => {
                    use crate::db::queries::asset as queries;
                    Ok(queries::get_by_id(conn, &asset_id))
                }
                Err(e) => Ok(Err(CoreError::Sqlite(e))),
            }
        })
        .await
        .map_err(|e: tokio_rusqlite::Error<rusqlite::Error>| CoreError::TaskEngine(format!("db worker error: {e}")))??;

    Ok(asset)
}

fn ext_from_content_type(headers: &reqwest::header::HeaderMap, asset_type: &str) -> &'static str {
    let ct = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    match (asset_type, ct) {
        ("image", t) if t.contains("png") => "png",
        ("image", t) if t.contains("webp") => "webp",
        ("image", _) => "jpg",
        ("video", t) if t.contains("webm") => "webm",
        ("video", _) => "mp4",
        ("audio", t) if t.contains("wav") => "wav",
        ("audio", _) => "mp3",
        _ => "bin",
    }
}
