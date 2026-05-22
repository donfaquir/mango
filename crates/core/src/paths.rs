//! Path conventions for project data directories.
//!
//! Every project owns a root directory containing `assets/` and `thumbnails/`
//! subdirectories. This module is the single source of truth for those names
//! and the helpers that compute / create the layout.

use std::path::{Path, PathBuf};

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

/// Convention path: `<app_data>/projects/{project_id}/`.
///
/// Used to backfill rows with NULL root_path and as the fallback when
/// `CreateProjectInput.root_path` is None. The uuid is chosen over a name
/// slug to avoid collisions; human-friendly names live only in the GUI
/// `suggest_project_root` display value.
pub fn convention_root(app_data_dir: &Path, project_id: &str) -> PathBuf {
    app_data_dir.join(PROJECTS_SUBDIR).join(project_id)
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
    fn convention_root_includes_projects_segment() {
        let app = Path::new("/var/app");
        let r = convention_root(app, "abc-123");
        assert_eq!(r, Path::new("/var/app/projects/abc-123"));
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
