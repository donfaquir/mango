use std::path::Path;

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::project::{
    CreateProjectInput, ListProjectsOptions, Project, UpdateProjectInput,
};
use crate::paths;

/// Create a new project.
///
/// `workspace_root` is the absolute path of the mounted workspace; it is
/// used to materialise the on-disk `<workspace>/projects/<subdir>/` layout
/// at creation time but is NOT stored — the DB stores only the
/// workspace-relative `root_path`. Callers (the Tauri / CLI shells) must
/// have already verified that a workspace is mounted before invoking this.
pub fn create(
    conn: &Connection,
    workspace_root: &Path,
    input: CreateProjectInput,
) -> Result<Project> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(CoreError::Validation("name cannot be empty".into()));
    }

    let id = Uuid::new_v4().to_string();
    let default_slug = derive_subdir_slug(&name, &id);
    let relative_root = paths::make_relative_project_root(input.subdir.as_deref(), &default_slug)?;
    let absolute_root = paths::resolve_project_root(workspace_root, &relative_root)?;

    // Refuse to clobber an existing non-empty directory — mirrors the
    // previous absolute-path validation so the user does not accidentally
    // shadow another project's files.
    if absolute_root.exists() {
        if !absolute_root.is_dir() {
            return Err(CoreError::Validation(format!(
                "project root exists but is not a directory: {}",
                absolute_root.display()
            )));
        }
        let mut entries = std::fs::read_dir(&absolute_root)?;
        if entries.next().is_some() {
            return Err(CoreError::Validation(format!(
                "project root directory is not empty: {}",
                absolute_root.display()
            )));
        }
    }

    paths::ensure_project_layout(&absolute_root)?;

    conn.execute(
        "INSERT INTO project (id, name, description, style_prompt, global_seed, root_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id,
            name,
            input.description.unwrap_or_default(),
            input.style_prompt.unwrap_or_default(),
            input.global_seed,
            relative_root,
        ],
    )?;

    get_by_id(conn, &id)
}

/// Derive a default subdirectory name from the project's display name.
///
/// Strategy (most → least informative):
///   1. ASCII slug via the `slug` crate ("My Comic" → "my-comic").
///   2. Sanitised unicode form of the original name ("我的第一部漫剧"
///      stays as-is) — modern filesystems on all three platforms accept
///      UTF-8 directory names, and a CJK label is more useful than an
///      opaque uuid for users browsing the workspace folder.
///   3. `p-<uuid8>` as the last resort when both produced an empty
///      string (e.g. a name made entirely of separator chars).
///
/// The `-<uuid8>` suffix guarantees uniqueness across collisions.
fn derive_subdir_slug(name: &str, id: &str) -> String {
    let ascii = slug::slugify(name);
    let candidate = if ascii.is_empty() {
        sanitize_unicode_for_subdir(name)
    } else {
        ascii
    };
    if candidate.is_empty() {
        format!("p-{}", &id[..8])
    } else {
        format!("{candidate}-{}", &id[..8])
    }
}

/// Best-effort cleanup of a unicode display name into something safe for
/// `paths::validate_subdir_segment`. Mirrors the frontend slugify rules:
/// strip path separators, the Windows-reserved set, NUL, and control
/// characters; collapse whitespace runs into `-`; trim leading/trailing
/// `.` and `-`.
fn sanitize_unicode_for_subdir(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_was_dash = false;
    for c in name.chars() {
        if c.is_whitespace() {
            if !last_was_dash && !out.is_empty() {
                out.push('-');
                last_was_dash = true;
            }
            continue;
        }
        if matches!(
            c,
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0'
        ) || c.is_control()
        {
            continue;
        }
        out.push(c);
        last_was_dash = false;
    }
    let trimmed = out.trim_matches(|c: char| c == '.' || c == '-');
    trimmed.to_string()
}

/// Get a project by ID.
pub fn get_by_id(conn: &Connection, id: &str) -> Result<Project> {
    conn.query_row(
        "SELECT id, name, description, style_prompt, root_path, global_seed, created_at, updated_at
         FROM project WHERE id = ?1",
        params![id],
        |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                style_prompt: row.get(3)?,
                root_path: row.get(4)?,
                global_seed: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
            entity: "project",
            id: id.to_string(),
        },
        other => CoreError::Sqlite(other),
    })
}

/// List projects ordered by created_at DESC with pagination.
pub fn list(conn: &Connection, opts: ListProjectsOptions) -> Result<Vec<Project>> {
    let limit = opts.limit.unwrap_or(50).clamp(1, 200);
    let offset = opts.offset.unwrap_or(0).max(0);

    let mut stmt = conn.prepare(
        "SELECT id, name, description, style_prompt, root_path, global_seed, created_at, updated_at
         FROM project ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
    )?;

    let projects = stmt
        .query_map(params![limit, offset], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                style_prompt: row.get(3)?,
                root_path: row.get(4)?,
                global_seed: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(projects)
}

/// Update a project. Only fields that are `Some` will be modified.
/// `root_path` is intentionally not updatable in MS1.
pub fn update(conn: &Connection, id: &str, input: UpdateProjectInput) -> Result<Project> {
    let _ = get_by_id(conn, id)?;

    if let Some(ref name) = input.name
        && name.trim().is_empty()
    {
        return Err(CoreError::Validation("name cannot be empty".into()));
    }

    let mut sets: Vec<String> = vec!["updated_at = datetime('now')".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
    let mut idx = 1u32;

    if let Some(ref name) = input.name {
        sets.push(format!("name = ?{idx}"));
        params.push(Box::new(name.trim().to_string()));
        idx += 1;
    }
    if let Some(ref desc) = input.description {
        sets.push(format!("description = ?{idx}"));
        params.push(Box::new(desc.clone()));
        idx += 1;
    }
    if let Some(ref sp) = input.style_prompt {
        sets.push(format!("style_prompt = ?{idx}"));
        params.push(Box::new(sp.clone()));
        idx += 1;
    }
    if let Some(ref maybe_seed) = input.global_seed {
        sets.push(format!("global_seed = ?{idx}"));
        params.push(Box::new(*maybe_seed));
        idx += 1;
    }

    let sql = format!(
        "UPDATE project SET {} WHERE id = ?{idx}",
        sets.join(", ")
    );
    params.push(Box::new(id.to_string()));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice())?;

    get_by_id(conn, id)
}

/// Delete a project metadata row. The on-disk root_path directory is left in
/// place; V2 will expose an explicit `purge_files` option.
pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let affected = conn.execute("DELETE FROM project WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(CoreError::NotFound {
            entity: "project",
            id: id.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use std::path::Path;
    use tempfile::{tempdir, TempDir};

    /// Returns `(conn, workspace_tempdir)`. Keep the tempdir bound in the
    /// test so it outlives the project directories created underneath.
    fn setup() -> (Connection, TempDir) {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let ws = tempdir().unwrap();
        (conn, ws)
    }

    #[test]
    fn test_create_project_with_default_subdir() {
        let (conn, ws) = setup();
        let project = create(
            &conn,
            ws.path(),
            CreateProjectInput {
                name: "测试项目".into(),
                subdir: None,
                description: Some("描述".into()),
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap();

        assert!(!project.id.is_empty());
        assert_eq!(project.name, "测试项目");
        assert_eq!(project.description, "描述");
        // Workspace-relative.
        assert!(
            project.root_path.starts_with("projects/"),
            "root_path should be workspace-relative, got {}",
            project.root_path
        );
        // The on-disk layout should be materialised under workspace.
        let abs = ws.path().join(&project.root_path);
        assert!(paths::assets_dir(&abs).is_dir());
        assert!(paths::thumbnails_dir(&abs).is_dir());
    }

    #[test]
    fn derive_subdir_slug_uses_ascii_slug_when_available() {
        let s = derive_subdir_slug("My First Comic", "abcd1234-rest");
        assert_eq!(s, "my-first-comic-abcd1234");
    }

    #[test]
    fn derive_subdir_slug_cjk_name_produces_nonempty_segment() {
        // The slug crate transliterates CJK to pinyin (e.g. "我的" → "wo-de"),
        // so the result is non-empty and ends with the uuid suffix. We
        // don't pin the exact transliteration since it depends on slug
        // crate internals — only assert the segment was produced.
        let s = derive_subdir_slug("我的第一部漫剧", "abcd1234-rest");
        assert!(s.ends_with("-abcd1234"));
        assert!(s.len() > "-abcd1234".len());
    }

    #[test]
    fn derive_subdir_slug_falls_back_to_uuid_when_name_yields_nothing() {
        // All chars stripped → final fallback.
        let s = derive_subdir_slug("///", "abcd1234-rest");
        assert_eq!(s, "p-abcd1234");
    }

    #[test]
    fn sanitize_unicode_for_subdir_keeps_cjk() {
        assert_eq!(sanitize_unicode_for_subdir("我的漫剧"), "我的漫剧");
    }

    #[test]
    fn sanitize_unicode_for_subdir_strips_separators_and_collapses_space() {
        assert_eq!(
            sanitize_unicode_for_subdir("路径/测试  名"),
            "路径测试-名"
        );
    }

    #[test]
    fn sanitize_unicode_for_subdir_trims_dots_and_dashes() {
        assert_eq!(sanitize_unicode_for_subdir("--.foo.--"), "foo");
    }

    #[test]
    fn sanitize_unicode_for_subdir_returns_empty_when_all_stripped() {
        assert_eq!(sanitize_unicode_for_subdir("///"), "");
    }

    #[test]
    fn test_create_project_with_cjk_name_succeeds() {
        let (conn, ws) = setup();
        let project = create(
            &conn,
            ws.path(),
            CreateProjectInput {
                name: "我的第一部漫剧".into(),
                subdir: None,
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap();
        // Slug crate transliterates to pinyin — exact form is opaque, but
        // it must land under `projects/` and the on-disk layout must be
        // materialised.
        assert!(
            project.root_path.starts_with("projects/"),
            "got {}",
            project.root_path
        );
        let abs = ws.path().join(&project.root_path);
        assert!(paths::assets_dir(&abs).is_dir());
    }

    #[test]
    fn test_create_project_with_cjk_explicit_subdir_succeeds() {
        let (conn, ws) = setup();
        let project = create(
            &conn,
            ws.path(),
            CreateProjectInput {
                name: "Whatever".into(),
                subdir: Some("我的漫剧".into()),
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap();
        assert_eq!(project.root_path, "projects/我的漫剧");
        let abs = ws.path().join(&project.root_path);
        assert!(paths::assets_dir(&abs).is_dir());
    }

    #[test]
    fn test_create_project_with_explicit_subdir() {
        let (conn, ws) = setup();
        let project = create(
            &conn,
            ws.path(),
            CreateProjectInput {
                name: "Custom".into(),
                subdir: Some("custom-slot".into()),
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap();

        assert_eq!(project.root_path, "projects/custom-slot");
        let abs = ws.path().join(&project.root_path);
        assert!(paths::assets_dir(&abs).is_dir());
    }

    #[test]
    fn test_create_project_subdir_with_separator_fails() {
        let (conn, ws) = setup();
        let result = create(
            &conn,
            ws.path(),
            CreateProjectInput {
                name: "x".into(),
                subdir: Some("foo/bar".into()),
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        );
        assert!(matches!(result, Err(CoreError::Validation(_))));
    }

    #[test]
    fn test_create_project_subdir_collision_with_non_empty_dir_fails() {
        let (conn, ws) = setup();
        let busy = ws.path().join("projects").join("busy");
        std::fs::create_dir_all(&busy).unwrap();
        std::fs::write(busy.join("intruder.txt"), b"hi").unwrap();

        let result = create(
            &conn,
            ws.path(),
            CreateProjectInput {
                name: "busy".into(),
                subdir: Some("busy".into()),
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        );
        assert!(matches!(result, Err(CoreError::Validation(_))));
    }

    #[test]
    fn test_create_project_empty_name_fails() {
        let (conn, ws) = setup();
        let result = create(
            &conn,
            ws.path(),
            CreateProjectInput {
                name: "  ".into(),
                subdir: None,
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        );
        assert!(matches!(result, Err(CoreError::Validation(_))));
    }

    #[test]
    fn test_get_by_id_not_found() {
        let (conn, _ws) = setup();
        let result = get_by_id(&conn, "nonexistent");
        assert!(matches!(result, Err(CoreError::NotFound { .. })));
    }

    #[test]
    fn test_list_with_pagination() {
        let (conn, ws) = setup();
        for i in 0..5 {
            create(
                &conn,
                ws.path(),
                CreateProjectInput {
                    name: format!("Project {i}"),
                    subdir: None,
                    description: None,
                    style_prompt: None,
                    global_seed: None,
                },
            )
            .unwrap();
        }

        let page1 = list(
            &conn,
            ListProjectsOptions {
                limit: Some(2),
                offset: Some(0),
            },
        )
        .unwrap();
        assert_eq!(page1.len(), 2);

        let page2 = list(
            &conn,
            ListProjectsOptions {
                limit: Some(2),
                offset: Some(2),
            },
        )
        .unwrap();
        assert_eq!(page2.len(), 2);

        let all = list(&conn, ListProjectsOptions::default()).unwrap();
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn test_update_project_partial() {
        let (conn, ws) = setup();
        let project = create(
            &conn,
            ws.path(),
            CreateProjectInput {
                name: "Original".into(),
                subdir: None,
                description: Some("desc".into()),
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap();

        let updated = update(
            &conn,
            &project.id,
            UpdateProjectInput {
                name: Some("Renamed".into()),
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap();

        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.description, "desc");
        // root_path is not modifiable via update
        assert_eq!(updated.root_path, project.root_path);
    }

    #[test]
    fn test_update_project_clear_global_seed() {
        let (conn, ws) = setup();
        let project = create(
            &conn,
            ws.path(),
            CreateProjectInput {
                name: "With Seed".into(),
                subdir: None,
                description: None,
                style_prompt: None,
                global_seed: Some(42),
            },
        )
        .unwrap();
        assert_eq!(project.global_seed, Some(42));

        let updated = update(
            &conn,
            &project.id,
            UpdateProjectInput {
                name: None,
                description: None,
                style_prompt: None,
                global_seed: Some(None),
            },
        )
        .unwrap();
        assert_eq!(updated.global_seed, None);
    }

    #[test]
    fn test_update_project_empty_name_fails() {
        let (conn, ws) = setup();
        let project = create(
            &conn,
            ws.path(),
            CreateProjectInput {
                name: "Test".into(),
                subdir: None,
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap();

        let result = update(
            &conn,
            &project.id,
            UpdateProjectInput {
                name: Some("  ".into()),
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        );
        assert!(matches!(result, Err(CoreError::Validation(_))));
    }

    #[test]
    fn test_delete_project() {
        let (conn, ws) = setup();
        let project = create(
            &conn,
            ws.path(),
            CreateProjectInput {
                name: "To Delete".into(),
                subdir: None,
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap();

        delete(&conn, &project.id).unwrap();
        assert!(matches!(
            get_by_id(&conn, &project.id),
            Err(CoreError::NotFound { .. })
        ));
    }

    #[test]
    fn test_delete_nonexistent_fails() {
        let (conn, _ws) = setup();
        let result = delete(&conn, "nonexistent");
        assert!(matches!(result, Err(CoreError::NotFound { .. })));
    }
}
