use std::fmt::Write as _;
use std::fs;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::asset::thumbnail;
use crate::db::queries::asset as queries;
use crate::error::{CoreError, Result};
use crate::models::asset::{Asset, ImportAssetInput};
use crate::paths;

pub const MAX_FILE_SIZE_BYTES: u64 = 50 * 1024 * 1024;
pub const ALLOWED_IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp"];

/// Artifacts produced by the filesystem stage of the import pipeline.
/// They are kept in memory until the DB stage inserts them (or replaces
/// them via content-hash dedupe).
pub struct ImportArtifacts {
    pub asset_id: String,
    pub original_name: String,
    pub file_relative: String,
    pub thumb_relative: Option<String>,
    pub file_size: i64,
    pub content_hash: String,
    pub metadata_json: Option<String>,
}

/// Stage 1 — read the project root from the DB. Holds the connection only
/// for the lifetime of a single SELECT.
pub fn resolve_project_root(conn: &Connection, project_id: &str) -> Result<PathBuf> {
    let path: String = conn
        .query_row(
            "SELECT root_path FROM project WHERE id = ?1",
            params![project_id],
            |r| r.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
                entity: "project",
                id: project_id.to_string(),
            },
            other => CoreError::Sqlite(other),
        })?;
    Ok(PathBuf::from(path))
}

/// Stage 2 — pure filesystem work. Does NOT hold any DB connection so a
/// 50 MiB copy + thumbnail decode won't block other IPC calls running
/// against the same tokio-rusqlite worker thread.
pub fn prepare_artifacts(project_root: &Path, source_path: &str) -> Result<ImportArtifacts> {
    let source = PathBuf::from(source_path);
    if !source.is_file() {
        return Err(CoreError::Validation(format!(
            "source file not found: {source_path}"
        )));
    }

    let ext = source
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    if !ALLOWED_IMAGE_EXTS.contains(&ext.as_str()) {
        return Err(CoreError::Validation(format!(
            "unsupported file extension: .{ext}; allowed: jpg, jpeg, png, webp"
        )));
    }

    let metadata = fs::metadata(&source)?;
    if metadata.len() > MAX_FILE_SIZE_BYTES {
        return Err(CoreError::Validation(format!(
            "file too large: {} bytes > {} bytes (max 50 MiB)",
            metadata.len(),
            MAX_FILE_SIZE_BYTES
        )));
    }

    paths::ensure_project_layout(project_root)?;

    let asset_id = Uuid::new_v4().to_string();
    let file_name = format!("{asset_id}.{ext}");
    let dest = paths::assets_dir(project_root).join(&file_name);

    let hash = copy_with_hash(&source, &dest)?;

    let thumb_name = format!("{asset_id}_thumb.webp");
    let thumb_dest = paths::thumbnails_dir(project_root).join(&thumb_name);
    let (thumb_relative, metadata_json) =
        match thumbnail::generate_with_metadata(&dest, &thumb_dest) {
            Ok((w, h)) => (
                Some(format!("{}/{}", paths::THUMBNAILS_SUBDIR, thumb_name)),
                Some(serde_json::json!({ "width": w, "height": h }).to_string()),
            ),
            Err(e) => {
                tracing::warn!("thumbnail generation failed for {asset_id}: {e}");
                (None, None)
            }
        };

    let original_name = source
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let file_relative = format!("{}/{}", paths::ASSETS_SUBDIR, file_name);

    Ok(ImportArtifacts {
        asset_id,
        original_name,
        file_relative,
        thumb_relative,
        file_size: metadata.len() as i64,
        content_hash: hash,
        metadata_json,
    })
}

/// Stage 3 — INSERT into the DB. Holds the connection only for the
/// dedupe check + a single INSERT. On dedupe hit, the new on-disk file
/// (already written in stage 2) is removed and the existing Asset is
/// returned.
pub fn persist_artifacts(
    conn: &Connection,
    project_id: &str,
    shot_id: Option<&str>,
    project_root: &Path,
    artifacts: ImportArtifacts,
) -> Result<Asset> {
    if let Some(existing) =
        queries::find_by_content_hash(conn, project_id, &artifacts.content_hash)?
    {
        // Drop the just-written duplicate copy.
        let _ = fs::remove_file(project_root.join(&artifacts.file_relative));
        if let Some(t) = &artifacts.thumb_relative {
            let _ = fs::remove_file(project_root.join(t));
        }
        return Ok(existing);
    }

    conn.execute(
        "INSERT INTO asset \
            (id, project_id, shot_id, asset_type, original_name, \
             file_path, thumbnail_path, file_size, content_hash, metadata_json) \
         VALUES (?1, ?2, ?3, 'image', ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            artifacts.asset_id,
            project_id,
            shot_id,
            artifacts.original_name,
            artifacts.file_relative,
            artifacts.thumb_relative,
            artifacts.file_size,
            artifacts.content_hash,
            artifacts.metadata_json,
        ],
    )?;

    queries::get_by_id(conn, &artifacts.asset_id)
}

/// High-level helper that runs all three stages on a synchronous Connection.
/// Used by the CLI and tests. The Tauri layer orchestrates the stages itself
/// to avoid holding the DB worker during stage 2.
pub fn import(conn: &Connection, input: ImportAssetInput) -> Result<Asset> {
    let project_root = resolve_project_root(conn, &input.project_id)?;
    let artifacts = prepare_artifacts(&project_root, &input.source_path)?;
    let shot_id = trim_shot_id(input.shot_id.as_deref());
    persist_artifacts(conn, &input.project_id, shot_id, &project_root, artifacts)
}

/// Normalise an optional shot_id coming over the IPC boundary: front-end
/// `<select>` widgets often send `""` for "no selection".
pub fn trim_shot_id(raw: Option<&str>) -> Option<&str> {
    raw.and_then(|s| {
        let t = s.trim();
        if t.is_empty() { None } else { Some(t) }
    })
}

fn copy_with_hash(src: &Path, dst: &Path) -> Result<String> {
    let mut reader = BufReader::new(fs::File::open(src)?);
    let mut writer = fs::File::create(dst)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        writer.write_all(&buf[..n])?;
    }
    writer.flush()?;
    Ok(hex_encode(&hasher.finalize()))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use crate::db::queries::project as project_queries;
    use crate::models::project::CreateProjectInput;
    use image::{ImageBuffer, Rgb};
    use sha2::{Digest, Sha256};
    use std::path::Path;
    use tempfile::{tempdir, TempDir};

    fn setup() -> (Connection, TempDir, String, PathBuf) {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let app_data = tempdir().unwrap();
        let project = project_queries::create(
            &conn,
            app_data.path(),
            CreateProjectInput {
                name: "P".into(),
                root_path: None,
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap();
        let root = PathBuf::from(&project.root_path);
        (conn, app_data, project.id, root)
    }

    fn write_sample_png(path: &Path, w: u32, h: u32) {
        let buf: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(w, h, |x, _| Rgb([(x % 256) as u8, 0, 0]));
        buf.save(path).unwrap();
    }
    

    #[test]
    fn prepare_artifacts_png_succeeds() {
        let (_, _td, _pid, root) = setup();
        let src_dir = tempdir().unwrap();
        let src = src_dir.path().join("orig.png");
        write_sample_png(&src, 320, 240);

        let a = prepare_artifacts(&root, src.to_str().unwrap()).unwrap();

        assert_eq!(a.original_name, "orig.png");
        assert!(a.file_relative.starts_with("assets/"));
        assert!(a.file_relative.ends_with(".png"));
        assert!(a.thumb_relative.as_deref().unwrap().starts_with("thumbnails/"));
        assert_eq!(a.content_hash.len(), 64);
        let meta = a.metadata_json.as_deref().unwrap();
        assert!(meta.contains("\"width\":320"));
        assert!(meta.contains("\"height\":240"));

        assert!(root.join(&a.file_relative).exists());
        assert!(root.join(a.thumb_relative.as_ref().unwrap()).exists());
    }

    #[test]
    fn prepare_artifacts_unsupported_ext_fails() {
        let (_, _td, _pid, root) = setup();
        let src_dir = tempdir().unwrap();
        let src = src_dir.path().join("doc.pdf");
        std::fs::write(&src, b"x").unwrap();
        let r = prepare_artifacts(&root, src.to_str().unwrap());
        assert!(matches!(r, Err(CoreError::Validation(msg)) if msg.contains("unsupported")));
    }

    #[test]
    fn prepare_artifacts_missing_source_fails() {
        let (_, _td, _pid, root) = setup();
        let r = prepare_artifacts(&root, "/no/such/file.png");
        assert!(matches!(r, Err(CoreError::Validation(_))));
    }

    #[test]
    fn prepare_artifacts_oversize_fails() {
        let (_, _td, _pid, root) = setup();
        let src_dir = tempdir().unwrap();
        let src = src_dir.path().join("big.png");
        // Allocate slightly over the cap; content does not need to be a real PNG
        // because the size check runs before image decoding.
        std::fs::write(&src, vec![0u8; (MAX_FILE_SIZE_BYTES as usize) + 1]).unwrap();
        let r = prepare_artifacts(&root, src.to_str().unwrap());
        assert!(matches!(r, Err(CoreError::Validation(msg)) if msg.contains("too large")));
    }

    #[test]
    fn content_hash_matches_sha256_of_source() {
        let (_, _td, _pid, root) = setup();
        let src_dir = tempdir().unwrap();
        let src = src_dir.path().join("orig.png");
        write_sample_png(&src, 16, 16);

        let bytes = std::fs::read(&src).unwrap();
        let mut h = Sha256::new();
        h.update(&bytes);
        let expected = hex_encode(&h.finalize());

        let a = prepare_artifacts(&root, src.to_str().unwrap()).unwrap();
        assert_eq!(a.content_hash, expected);
    }

    #[test]
    fn corrupt_image_succeeds_without_thumbnail() {
        let (_, _td, _pid, root) = setup();
        let src_dir = tempdir().unwrap();
        let src = src_dir.path().join("broken.png");
        // PNG magic bytes, then garbage — passes ext check, fails decoder.
        std::fs::write(
            &src,
            b"\x89PNG\r\n\x1a\n_not_a_real_png_anymore_just_some_bytes",
        )
        .unwrap();

        let a = prepare_artifacts(&root, src.to_str().unwrap()).unwrap();
        assert!(a.thumb_relative.is_none());
        assert!(a.metadata_json.is_none());
        // The copied source file is still on disk.
        assert!(root.join(&a.file_relative).exists());
    }

    #[test]
    fn persist_dedupes_same_content_hash() {
        let (conn, _td, pid, root) = setup();
        let src_dir = tempdir().unwrap();
        let src = src_dir.path().join("orig.png");
        write_sample_png(&src, 32, 32);

        let first = import(
            &conn,
            ImportAssetInput {
                project_id: pid.clone(),
                source_path: src.to_str().unwrap().to_string(),
                shot_id: None,
            },
        )
        .unwrap();
        let second = import(
            &conn,
            ImportAssetInput {
                project_id: pid.clone(),
                source_path: src.to_str().unwrap().to_string(),
                shot_id: None,
            },
        )
        .unwrap();

        assert_eq!(first.id, second.id);
        let on_disk = std::fs::read_dir(paths::assets_dir(&root)).unwrap().count();
        assert_eq!(on_disk, 1, "duplicate copy should have been removed");
    }

    #[test]
    fn shot_id_empty_string_becomes_none() {
        let (conn, _td, pid, _root) = setup();
        let src_dir = tempdir().unwrap();
        let src = src_dir.path().join("orig.png");
        write_sample_png(&src, 32, 32);

        let a = import(
            &conn,
            ImportAssetInput {
                project_id: pid,
                source_path: src.to_str().unwrap().to_string(),
                shot_id: Some("   ".into()),
            },
        )
        .unwrap();
        assert!(a.shot_id.is_none());
    }

    #[test]
    fn import_unknown_project_returns_not_found() {
        let (conn, _td, _pid, _root) = setup();
        let r = import(
            &conn,
            ImportAssetInput {
                project_id: "no-such".into(),
                source_path: "/tmp/whatever.png".into(),
                shot_id: None,
            },
        );
        assert!(matches!(r, Err(CoreError::NotFound { .. })));
    }
}
