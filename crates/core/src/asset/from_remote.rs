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
use crate::provider::error::{ProviderErrorDetail, ProviderErrorKind};

/// Outcome of a successful [`download_to_asset`] call. The runner threads the
/// `bytes` + `duration_ms` into a `download` event so the diagnostics UI can
/// show how big the result was and how long it took to fetch.
#[derive(Debug, Clone)]
pub struct DownloadOutcome {
    pub asset: Asset,
    pub bytes: u64,
    pub duration_ms: u64,
}

/// Download a result URL and persist it as a local asset file + DB row.
///
/// Returns the newly created [`Asset`] together with the raw byte count and
/// wall-clock download duration so callers can record diagnostic events.
pub async fn download_to_asset(
    db: &tokio_rusqlite::Connection,
    project_id: &str,
    shot_id: Option<&str>,
    url: &str,
    asset_type: &str, // "image" | "video" | "audio"
) -> Result<DownloadOutcome> {
    let download_started = std::time::Instant::now();

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
    let resp = reqwest::get(url).await.map_err(|e| {
        CoreError::Provider(
            ProviderErrorDetail::new(ProviderErrorKind::Network, format!("下载失败: {e}"))
                .with_body_excerpt(url.to_string()),
        )
    })?;
    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        return Err(CoreError::Provider(
            ProviderErrorDetail::new(
                ProviderErrorKind::Unknown,
                format!("下载失败 (HTTP {status}): 结果 URL 可能已过期"),
            )
            .with_http_status(status)
            .with_body_excerpt(url.to_string()),
        ));
    }
    let ext = ext_from_content_type(resp.headers(), asset_type);
    let asset_id = Uuid::new_v4().to_string();
    let file_name = format!("{asset_id}.{ext}");
    let file_relative = format!("{}/{file_name}", paths::ASSETS_SUBDIR);
    let dest = paths::assets_dir(&project_root).join(&file_name);

    let bytes = resp.bytes().await.map_err(|e| {
        CoreError::Provider(ProviderErrorDetail::new(
            ProviderErrorKind::Network,
            format!("读取下载内容失败: {e}"),
        ))
    })?;
    let file_size = bytes.len() as i64;

    // Write file (blocking IO, but the file is typically small <100MB).
    std::fs::write(&dest, &bytes)?;

    // 3) Generate thumbnail for images. The thumbnail helper also returns
    // the source dimensions so we can persist `{width, height}` into
    // metadata_json — the asset library uses this for layout hints.
    let (thumb_relative, metadata_json) = if asset_type == "image" {
        let thumb_name = format!("{asset_id}_thumb.webp");
        let thumb_dest = paths::thumbnails_dir(&project_root).join(&thumb_name);
        match thumbnail::generate_with_metadata(&dest, &thumb_dest) {
            Ok((w, h)) => (
                Some(format!("{}/{thumb_name}", paths::THUMBNAILS_SUBDIR)),
                Some(serde_json::json!({ "width": w, "height": h }).to_string()),
            ),
            Err(e) => {
                tracing::warn!("thumbnail for remote result failed: {e}");
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    // 4) INSERT asset row.
    let insert_id = asset_id.clone();
    let insert_pid = project_id.to_string();
    let insert_shot = shot_id.map(|s| s.to_string());
    let insert_type = asset_type.to_string();
    let insert_file_rel = file_relative;
    let insert_thumb = thumb_relative;
    let insert_size = file_size;
    let insert_meta = metadata_json;
    let original_name = format!("generated.{ext}");

    let asset: Asset = db
        .call(move |conn| {
            let insert_result = conn.execute(
                "INSERT INTO asset \
                    (id, project_id, shot_id, asset_type, original_name, \
                     file_path, thumbnail_path, file_size, content_hash, metadata_json, source) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, 'generated')",
                params![
                    insert_id,
                    insert_pid,
                    insert_shot,
                    insert_type,
                    original_name,
                    insert_file_rel,
                    insert_thumb,
                    insert_size,
                    insert_meta,
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

    Ok(DownloadOutcome {
        asset,
        bytes: file_size as u64,
        duration_ms: download_started.elapsed().as_millis() as u64,
    })
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
