//! Download a remote URL and persist it as a local [`Asset`].
//!
//! Unlike [`super::import`], this module:
//! - Does NOT enforce the image-extension whitelist (video mp4 is valid).
//! - Does NOT enforce the 50 MiB size cap.
//! - Infers the file extension from the HTTP `Content-Type` header.
//! - Is designed for async callers (the runner calling through
//!   `ResultMaterializer`).

use std::path::{Path, PathBuf};

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
/// `workspace_root` is the absolute path of the mounted workspace; it is
/// joined with the DB-stored relative `project.root_path` to produce the
/// absolute on-disk destination. Callers must have a workspace mounted
/// (the task engine guarantees this — runners do not execute when none
/// is mounted).
///
/// `prompt` is the generation prompt (if any) that produced this asset.
/// When supplied, the full text becomes the asset's `original_name` (so
/// the asset library's keyword search hits any word the user remembers
/// from the prompt) and is also persisted into `metadata_json.prompt`
/// for the preview dialog to display the complete generation hint. If a
/// row with the same `original_name` already exists in the project the
/// new row gets ` #N` appended so users can tell repeated submissions
/// apart in card listings.
///
/// Returns the newly created [`Asset`] together with the raw byte count and
/// wall-clock download duration so callers can record diagnostic events.
pub async fn download_to_asset(
    db: &tokio_rusqlite::Connection,
    workspace_root: &Path,
    project_id: &str,
    shot_id: Option<&str>,
    url: &str,
    asset_type: &str, // "image" | "video" | "audio"
    prompt: Option<&str>,
) -> Result<DownloadOutcome> {
    let download_started = std::time::Instant::now();

    // 1) Resolve project root from DB (relative) and join workspace_root.
    let pid = project_id.to_string();
    let workspace = workspace_root.to_path_buf();
    let project_root: PathBuf = db
        .call(move |conn| {
            let relative: String = conn.query_row(
                "SELECT root_path FROM project WHERE id = ?1",
                params![pid],
                |r| r.get::<_, String>(0),
            )?;
            Ok(crate::paths::resolve_project_root(&workspace, &relative))
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
        })??;

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
    // metadata_json — the asset library uses this for layout hints. We
    // also fold the generation prompt (if any) into metadata so the
    // preview dialog can render a "生成提示词" block independent of
    // however `original_name` got truncated for display.
    let trimmed_prompt: Option<&str> = prompt
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let (thumb_relative, dimensions) = if asset_type == "image" {
        let thumb_name = format!("{asset_id}_thumb.webp");
        let thumb_dest = paths::thumbnails_dir(&project_root).join(&thumb_name);
        match thumbnail::generate_with_metadata(&dest, &thumb_dest) {
            Ok((w, h)) => (
                Some(format!("{}/{thumb_name}", paths::THUMBNAILS_SUBDIR)),
                Some((w, h)),
            ),
            Err(e) => {
                tracing::warn!("thumbnail for remote result failed: {e}");
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    let metadata_json = build_metadata_json(dimensions, trimmed_prompt);

    // 4) INSERT asset row.
    let insert_id = asset_id.clone();
    let insert_pid = project_id.to_string();
    let insert_shot = shot_id.map(|s| s.to_string());
    let insert_type = asset_type.to_string();
    let insert_file_rel = file_relative;
    let insert_thumb = thumb_relative;
    let insert_size = file_size;
    let insert_meta = metadata_json;
    // Base name: a short, display-friendly slice of the prompt. The full
    // prompt is preserved on `metadata_json.prompt` and that's what the
    // asset library's keyword search actually scans, so the truncation
    // here is purely cosmetic — it does NOT shrink searchability. Falls
    // back to "generated.<ext>" when no prompt is available. The same-name
    // disambiguator (`#N`) runs inside the DB closure so the SELECT +
    // INSERT see a consistent view of the project.
    let base_original_name = trimmed_prompt
        .map(|p| {
            let head = display_title_from_prompt(p);
            if head.is_empty() {
                format!("generated.{ext}")
            } else {
                format!("{head}.{ext}")
            }
        })
        .unwrap_or_else(|| format!("generated.{ext}"));

    let asset: Asset = db
        .call(move |conn| {
            let final_name = match disambiguate_original_name(
                conn,
                &insert_pid,
                &base_original_name,
            ) {
                Ok(n) => n,
                Err(e) => return Ok(Err(e)),
            };
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
                    final_name,
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

/// Derive a short display title from a generation prompt.
///
/// Strategy: split off the *first* comma/semicolon/newline-delimited
/// segment (in both ASCII and CJK punctuation) so booru-style trailing
/// commas / quality modifiers don't end up in the filename, then take the
/// first ~12 Unicode code points (not bytes — CJK characters must not be
/// cut mid-codepoint). Trim leading/trailing whitespace at every step so
/// names never end with " " or partial punctuation.
///
/// Examples:
///   "赛博朋克少女, 霓虹街头, anime"   → "赛博朋克少女"
///   "a beautiful girl, masterpiece"   → "a beautiful "  → trimmed: "a beautiful"
///   "夕阳下侧脸"                      → "夕阳下侧脸"
///
/// Returns an empty string only when the prompt is itself empty after
/// trimming; callers fall back to "generated.<ext>" in that case.
fn display_title_from_prompt(prompt: &str) -> String {
    const MAX_CHARS: usize = 12;
    // Explicit unicode escapes for the full-width comma/semicolon so the
    // pattern can't be confused with their ASCII look-alikes during code
    // review (an earlier version accidentally listed `,` twice and the
    // CJK separator silently never matched).
    const FULLWIDTH_COMMA: char = '\u{FF0C}'; // ，
    const FULLWIDTH_SEMICOLON: char = '\u{FF1B}'; // ；
    let first_segment = prompt
        .split(|c: char| {
            matches!(
                c,
                ',' | ';' | '\n' | '\r' | FULLWIDTH_COMMA | FULLWIDTH_SEMICOLON
            )
        })
        .next()
        .unwrap_or("")
        .trim();
    first_segment
        .chars()
        .take(MAX_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

/// Compose the `metadata_json` blob from the optional decoded image
/// dimensions and the optional generation prompt. Returns `None` when both
/// are absent so we don't store an empty `{}` row.
fn build_metadata_json(
    dimensions: Option<(u32, u32)>,
    prompt: Option<&str>,
) -> Option<String> {
    if dimensions.is_none() && prompt.is_none() {
        return None;
    }
    let mut obj = serde_json::Map::new();
    if let Some((w, h)) = dimensions {
        obj.insert("width".into(), serde_json::Value::from(w));
        obj.insert("height".into(), serde_json::Value::from(h));
    }
    if let Some(p) = prompt {
        obj.insert("prompt".into(), serde_json::Value::from(p));
    }
    Some(serde_json::Value::Object(obj).to_string())
}

/// If `base` collides with an existing `original_name` in the same project,
/// rename it `<stem> (2).<ext>`, `<stem> (3).<ext>`, ... — the convention
/// macOS Finder and Windows Explorer use when copying a duplicate file, so
/// users recognise the pattern without any explanation.
///
/// Picks the smallest unused N (not `count + 1`), matching Finder behaviour:
/// if a numbered copy is deleted later, the next disambiguation reuses that
/// freed slot instead of skipping forward. Costs one extra `SELECT 1` per
/// occupied number; in practice N stays tiny (<10) so the loop is cheap.
///
/// Best-effort against concurrent materializers — two simultaneous inserts
/// can race past the SELECT and produce duplicate names, but `original_name`
/// is a display label not a uniqueness constraint so this just shows up as
/// two cards with the same title.
fn disambiguate_original_name(
    conn: &rusqlite::Connection,
    project_id: &str,
    base: &str,
) -> Result<String> {
    // Cheap exit: if the base name itself is free, use it as-is — keeps
    // first-of-a-kind titles clean (no "项羽 (1).png" for a lone "项羽").
    if !name_exists(conn, project_id, base)? {
        return Ok(base.to_string());
    }
    let (stem, ext_with_dot) = split_stem_ext(base);
    let mut n: u32 = 2;
    loop {
        let candidate = format!("{stem} ({n}){ext_with_dot}");
        if !name_exists(conn, project_id, &candidate)? {
            return Ok(candidate);
        }
        n += 1;
    }
}

fn name_exists(
    conn: &rusqlite::Connection,
    project_id: &str,
    name: &str,
) -> Result<bool> {
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM asset WHERE project_id = ?1 AND original_name = ?2",
        params![project_id, name],
        |r| r.get(0),
    )?;
    Ok(exists > 0)
}

/// Split a filename into `(stem, ".ext")` so disambiguation can insert the
/// `(N)` suffix *before* the extension (matching Finder / Explorer). Uses
/// `rfind('.')` so multi-dot stems like `项羽v1.2.png` keep the final
/// segment as the extension. Returns `(name, "")` when no `.` is present
/// (the legacy "generated" fallback never lacks an extension, but we keep
/// the function total).
fn split_stem_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) => name.split_at(i),
        None => (name, ""),
    }
}

#[cfg(test)]
mod disambiguate_tests {
    use super::*;
    use crate::db::open_sync;
    use rusqlite::Connection;
    use std::path::Path;

    fn fresh_conn() -> Connection {
        open_sync(Path::new(":memory:")).unwrap()
    }

    fn insert_named(conn: &Connection, project_id: &str, name: &str) {
        // Minimal project row so the FK passes.
        let _ = conn.execute(
            "INSERT OR IGNORE INTO project (id, name, root_path) \
             VALUES (?1, 'P', 'projects/p')",
            params![project_id],
        );
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO asset \
                (id, project_id, asset_type, original_name, file_path, file_size, source) \
             VALUES (?1, ?2, 'image', ?3, 'assets/x', 0, 'generated')",
            params![id, project_id, name],
        )
        .unwrap();
    }

    #[test]
    fn split_stem_ext_basic() {
        assert_eq!(split_stem_ext("项羽.png"), ("项羽", ".png"));
        assert_eq!(split_stem_ext("项羽v1.2.png"), ("项羽v1.2", ".png"));
        assert_eq!(split_stem_ext("noext"), ("noext", ""));
    }

    #[test]
    fn first_of_a_kind_keeps_clean_name() {
        let conn = fresh_conn();
        let pid = "p1";
        let out = disambiguate_original_name(&conn, pid, "项羽.png").unwrap();
        assert_eq!(out, "项羽.png");
    }

    #[test]
    fn second_collision_becomes_parenthesised_2() {
        let conn = fresh_conn();
        let pid = "p1";
        insert_named(&conn, pid, "项羽.png");
        let out = disambiguate_original_name(&conn, pid, "项羽.png").unwrap();
        assert_eq!(out, "项羽 (2).png");
    }

    #[test]
    fn third_collision_becomes_parenthesised_3() {
        let conn = fresh_conn();
        let pid = "p1";
        insert_named(&conn, pid, "项羽.png");
        insert_named(&conn, pid, "项羽 (2).png");
        let out = disambiguate_original_name(&conn, pid, "项羽.png").unwrap();
        assert_eq!(out, "项羽 (3).png");
    }

    #[test]
    fn reuses_freed_number_after_deletion() {
        // Finder behaviour: deleting "项羽 (2).png" frees slot 2, so the
        // next disambiguation reuses it rather than skipping to (4).
        let conn = fresh_conn();
        let pid = "p1";
        insert_named(&conn, pid, "项羽.png");
        insert_named(&conn, pid, "项羽 (3).png"); // slot 2 intentionally empty
        let out = disambiguate_original_name(&conn, pid, "项羽.png").unwrap();
        assert_eq!(out, "项羽 (2).png");
    }

    #[test]
    fn handles_extensionless_base() {
        // Belt-and-braces: the production code path always supplies an
        // extension (from `ext_from_content_type`), but the function
        // should not blow up if base lacks one.
        let conn = fresh_conn();
        let pid = "p1";
        insert_named(&conn, pid, "noext");
        let out = disambiguate_original_name(&conn, pid, "noext").unwrap();
        assert_eq!(out, "noext (2)");
    }
}

#[cfg(test)]
mod title_tests {
    use super::display_title_from_prompt;

    #[test]
    fn cjk_with_comma_separated_segments_keeps_first_clause() {
        assert_eq!(
            display_title_from_prompt("赛博朋克少女, 霓虹街头, anime, 4K"),
            "赛博朋克少女"
        );
    }

    #[test]
    fn cjk_full_width_comma_also_splits() {
        assert_eq!(
            display_title_from_prompt("夕阳下侧脸，soft lighting"),
            "夕阳下侧脸"
        );
    }

    #[test]
    fn english_first_segment_truncates_to_12_chars_and_trims() {
        // "a beautiful girl" → first 12 chars "a beautiful " → trim → "a beautiful"
        assert_eq!(
            display_title_from_prompt("a beautiful girl, masterpiece"),
            "a beautiful"
        );
    }

    #[test]
    fn no_separator_short_prompt_returns_full_text() {
        assert_eq!(display_title_from_prompt("夕阳下侧脸"), "夕阳下侧脸");
    }

    #[test]
    fn no_separator_long_prompt_truncates_at_12_chars() {
        // 14 ASCII chars → take 12 → "abcdefghijkl"
        assert_eq!(
            display_title_from_prompt("abcdefghijklmn"),
            "abcdefghijkl"
        );
    }

    #[test]
    fn semicolon_and_newline_also_split() {
        assert_eq!(display_title_from_prompt("foo; bar; baz"), "foo");
        assert_eq!(display_title_from_prompt("foo\nbar"), "foo");
    }

    #[test]
    fn leading_whitespace_does_not_leak_into_title() {
        assert_eq!(display_title_from_prompt("   赛博朋克少女, x"), "赛博朋克少女");
    }

    #[test]
    fn empty_prompt_yields_empty_string() {
        assert_eq!(display_title_from_prompt(""), "");
        assert_eq!(display_title_from_prompt("   "), "");
        // Only-separators prompt collapses to empty too — callers fall back
        // to "generated.<ext>".
        assert_eq!(display_title_from_prompt(",,,"), "");
    }
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
