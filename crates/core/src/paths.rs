//! Path conventions for project data directories.
//!
//! Every project owns a root directory containing `assets/` and `thumbnails/`
//! subdirectories. The directory itself lives under `<workspace>/projects/`;
//! the `project.root_path` column stores a *workspace-relative* POSIX path
//! (e.g. `projects/ep4-the-rain`) so the workspace as a whole is portable.
//!
//! This module is the single source of truth for the subdirectory names and
//! for translating between the relative storage form and an absolute
//! filesystem path.

use std::path::{Component, Path, PathBuf};

use crate::error::{CoreError, Result};

pub const ASSETS_SUBDIR: &str = "assets";
pub const THUMBNAILS_SUBDIR: &str = "thumbnails";
pub const PROJECTS_SUBDIR: &str = "projects";

pub fn assets_dir(project_root: &Path) -> PathBuf {
    project_root.join(ASSETS_SUBDIR)
}

pub fn thumbnails_dir(project_root: &Path) -> PathBuf {
    project_root.join(THUMBNAILS_SUBDIR)
}

/// Create the project directory layout (root + assets + thumbnails).
/// Idempotent: repeated calls on an existing layout succeed.
pub fn ensure_project_layout(project_root: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(assets_dir(project_root))?;
    std::fs::create_dir_all(thumbnails_dir(project_root))?;
    Ok(())
}

/// The workspace-relative root for a new project given its user-chosen
/// subdirectory name. Always `projects/<subdir>` with POSIX `/` separator
/// so the same string round-trips across platforms when the workspace is
/// zipped + restored elsewhere.
pub fn default_relative_project_root(subdir: &str) -> String {
    format!("{PROJECTS_SUBDIR}/{subdir}")
}

/// Resolve a workspace-relative `project.root_path` to an absolute filesystem
/// path under the given workspace root. Returns Validation if the relative
/// path is empty, absolute, or escapes the workspace via `..`. The check is
/// purely structural — we do not stat the resolved path.
pub fn resolve_project_root(workspace_root: &Path, relative: &str) -> Result<PathBuf> {
    let trimmed = relative.trim();
    if trimmed.is_empty() {
        return Err(CoreError::Validation(
            "project.root_path is empty".into(),
        ));
    }
    let rel = Path::new(trimmed);
    if rel.is_absolute() {
        return Err(CoreError::Validation(format!(
            "project.root_path must be workspace-relative, got absolute path: {trimmed}"
        )));
    }
    for c in rel.components() {
        if matches!(c, Component::ParentDir) {
            return Err(CoreError::Validation(format!(
                "project.root_path must not escape workspace via '..': {trimmed}"
            )));
        }
    }
    Ok(workspace_root.join(rel))
}

/// Validate that a user-supplied subdirectory name is safe to use as the
/// project's slot under `<workspace>/projects/`. Rules:
///   - non-empty after trim
///   - no path separators (must be a single segment)
///   - no `.` / `..` sentinels
///   - no NUL or control characters
fn validate_subdir_segment(subdir: &str) -> Result<()> {
    let s = subdir.trim();
    if s.is_empty() {
        return Err(CoreError::Validation("subdir cannot be empty".into()));
    }
    if s == "." || s == ".." {
        return Err(CoreError::Validation(format!("subdir is reserved: {s}")));
    }
    if s.contains('/') || s.contains('\\') {
        return Err(CoreError::Validation(format!(
            "subdir must be a single path segment, got: {s}"
        )));
    }
    if s.contains('\0') || s.chars().any(|c| c.is_control()) {
        return Err(CoreError::Validation(
            "subdir contains invalid control character".into(),
        ));
    }
    Ok(())
}

/// Normalise a user-supplied subdir (or generate a default from a slug if
/// the user did not supply one) and return the resulting workspace-relative
/// `root_path` value. Used by `project_queries::create` to build the value
/// it writes into the `project.root_path` column.
pub fn make_relative_project_root(user_subdir: Option<&str>, default_slug: &str) -> Result<String> {
    let subdir = match user_subdir.map(str::trim) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => default_slug.to_string(),
    };
    validate_subdir_segment(&subdir)?;
    Ok(default_relative_project_root(&subdir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn assets_dir_joins_subdir() {
        let root = Path::new("/tmp/p1");
        assert_eq!(assets_dir(root), Path::new("/tmp/p1/assets"));
    }

    #[test]
    fn thumbnails_dir_joins_subdir() {
        let root = Path::new("/tmp/p1");
        assert_eq!(thumbnails_dir(root), Path::new("/tmp/p1/thumbnails"));
    }

    #[test]
    fn default_relative_project_root_uses_posix_separator() {
        assert_eq!(default_relative_project_root("ep4"), "projects/ep4");
    }

    #[test]
    fn resolve_joins_relative_under_workspace() {
        let ws = Path::new("/var/ws");
        let abs = resolve_project_root(ws, "projects/ep4").unwrap();
        assert_eq!(abs, Path::new("/var/ws/projects/ep4"));
    }

    #[test]
    fn resolve_rejects_absolute_path() {
        let ws = Path::new("/var/ws");
        let err = resolve_project_root(ws, "/etc/passwd").unwrap_err();
        assert!(matches!(err, CoreError::Validation(_)));
    }

    #[test]
    fn resolve_rejects_parent_dir_escape() {
        let ws = Path::new("/var/ws");
        let err = resolve_project_root(ws, "projects/../../../etc").unwrap_err();
        assert!(matches!(err, CoreError::Validation(_)));
    }

    #[test]
    fn resolve_rejects_empty() {
        let ws = Path::new("/var/ws");
        assert!(matches!(
            resolve_project_root(ws, "   "),
            Err(CoreError::Validation(_))
        ));
    }

    #[test]
    fn make_relative_uses_user_subdir() {
        let s = make_relative_project_root(Some("my-comic"), "fallback").unwrap();
        assert_eq!(s, "projects/my-comic");
    }

    #[test]
    fn make_relative_falls_back_to_slug_when_blank() {
        let s = make_relative_project_root(Some("  "), "fallback").unwrap();
        assert_eq!(s, "projects/fallback");
        let s = make_relative_project_root(None, "fallback").unwrap();
        assert_eq!(s, "projects/fallback");
    }

    #[test]
    fn make_relative_rejects_path_separator_in_subdir() {
        let err = make_relative_project_root(Some("foo/bar"), "fb").unwrap_err();
        assert!(matches!(err, CoreError::Validation(_)));
        let err = make_relative_project_root(Some("foo\\bar"), "fb").unwrap_err();
        assert!(matches!(err, CoreError::Validation(_)));
    }

    #[test]
    fn make_relative_rejects_dot_sentinels() {
        for s in [".", ".."] {
            assert!(matches!(
                make_relative_project_root(Some(s), "fb"),
                Err(CoreError::Validation(_))
            ));
        }
    }

    #[test]
    fn ensure_project_layout_is_idempotent() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("proj");
        ensure_project_layout(&root).unwrap();
        assert!(assets_dir(&root).is_dir());
        assert!(thumbnails_dir(&root).is_dir());

        // Second call must succeed without error.
        ensure_project_layout(&root).unwrap();
        assert!(assets_dir(&root).is_dir());
    }
}
