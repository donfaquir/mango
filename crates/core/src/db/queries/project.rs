use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::project::{
    CreateProjectInput, ListProjectsOptions, Project, UpdateProjectInput,
};
use crate::paths;

/// Create a new project.
///
/// `app_data_dir` is the per-installation data directory used only when
/// `input.root_path` is None (CLI ergonomics, internal callers); it is
/// ignored when the caller supplies an explicit path.
pub fn create(
    conn: &Connection,
    app_data_dir: &Path,
    input: CreateProjectInput,
) -> Result<Project> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(CoreError::Validation("name cannot be empty".into()));
    }

    let id = Uuid::new_v4().to_string();

    let root = match input.root_path.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => {
            let p = PathBuf::from(s);
            validate_user_root_path(&p)?;
            p
        }
        _ => paths::convention_root(app_data_dir, &id),
    };

    paths::ensure_project_layout(&root)?;

    conn.execute(
        "INSERT INTO project (id, name, description, style_prompt, global_seed, root_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id,
            name,
            input.description.unwrap_or_default(),
            input.style_prompt.unwrap_or_default(),
            input.global_seed,
            root.to_string_lossy(),
        ],
    )?;

    get_by_id(conn, &id)
}

fn validate_user_root_path(root: &Path) -> Result<()> {
    if !root.is_absolute() {
        return Err(CoreError::Validation(
            "root_path must be absolute".into(),
        ));
    }

    if root.exists() {
        // Exists → must be a directory AND empty.
        if !root.is_dir() {
            return Err(CoreError::Validation(
                "root_path exists but is not a directory".into(),
            ));
        }
        let mut entries = std::fs::read_dir(root)?;
        if entries.next().is_some() {
            return Err(CoreError::Validation(
                "root_path must be empty or non-existent".into(),
            ));
        }
    } else {
        // Doesn't exist → parent must exist (don't recursively create unfamiliar trees).
        match root.parent() {
            Some(parent) if parent.as_os_str().is_empty() => {
                return Err(CoreError::Validation(
                    "root_path parent directory missing".into(),
                ));
            }
            Some(parent) => {
                if !parent.is_dir() {
                    return Err(CoreError::Validation(format!(
                        "root_path parent directory does not exist: {}",
                        parent.display()
                    )));
                }
            }
            None => {
                return Err(CoreError::Validation(
                    "root_path parent directory missing".into(),
                ));
            }
        }
    }

    Ok(())
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

    /// Returns `(conn, tempdir)`. Keep the tempdir bound in the test so it
    /// outlives the project directories created underneath.
    fn setup() -> (Connection, TempDir) {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let app_data = tempdir().unwrap();
        (conn, app_data)
    }

    #[test]
    fn test_create_project_with_convention_path() {
        let (conn, app_data) = setup();
        let project = create(
            &conn,
            app_data.path(),
            CreateProjectInput {
                name: "测试项目".into(),
                root_path: None,
                description: Some("描述".into()),
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap();

        assert!(!project.id.is_empty());
        assert_eq!(project.name, "测试项目");
        assert_eq!(project.description, "描述");
        assert!(!project.created_at.is_empty());
        assert_eq!(project.created_at, project.updated_at);

        let expected_root = paths::convention_root(app_data.path(), &project.id);
        assert_eq!(Path::new(&project.root_path), expected_root);
        assert!(paths::assets_dir(&expected_root).is_dir());
        assert!(paths::thumbnails_dir(&expected_root).is_dir());
    }

    #[test]
    fn test_create_project_with_explicit_root_path() {
        let (conn, app_data) = setup();
        let custom = app_data.path().join("custom_root");
        let project = create(
            &conn,
            app_data.path(),
            CreateProjectInput {
                name: "Custom".into(),
                root_path: Some(custom.to_string_lossy().into_owned()),
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap();

        assert_eq!(Path::new(&project.root_path), custom);
        assert!(paths::assets_dir(&custom).is_dir());
    }

    #[test]
    fn test_create_project_relative_root_path_fails() {
        let (conn, app_data) = setup();
        let result = create(
            &conn,
            app_data.path(),
            CreateProjectInput {
                name: "rel".into(),
                root_path: Some("relative/path".into()),
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        );
        assert!(matches!(result, Err(CoreError::Validation(_))));
    }

    #[test]
    fn test_create_project_non_empty_root_path_fails() {
        let (conn, app_data) = setup();
        let busy = app_data.path().join("busy");
        std::fs::create_dir_all(&busy).unwrap();
        std::fs::write(busy.join("intruder.txt"), b"hi").unwrap();

        let result = create(
            &conn,
            app_data.path(),
            CreateProjectInput {
                name: "busy".into(),
                root_path: Some(busy.to_string_lossy().into_owned()),
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        );
        assert!(matches!(result, Err(CoreError::Validation(_))));
    }

    #[test]
    fn test_create_project_empty_name_fails() {
        let (conn, app_data) = setup();
        let result = create(
            &conn,
            app_data.path(),
            CreateProjectInput {
                name: "  ".into(),
                root_path: None,
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        );
        assert!(matches!(result, Err(CoreError::Validation(_))));
    }

    #[test]
    fn test_get_by_id_not_found() {
        let (conn, _app_data) = setup();
        let result = get_by_id(&conn, "nonexistent");
        assert!(matches!(result, Err(CoreError::NotFound { .. })));
    }

    #[test]
    fn test_list_with_pagination() {
        let (conn, app_data) = setup();
        for i in 0..5 {
            create(
                &conn,
                app_data.path(),
                CreateProjectInput {
                    name: format!("Project {i}"),
                    root_path: None,
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
        let (conn, app_data) = setup();
        let project = create(
            &conn,
            app_data.path(),
            CreateProjectInput {
                name: "Original".into(),
                root_path: None,
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
        let (conn, app_data) = setup();
        let project = create(
            &conn,
            app_data.path(),
            CreateProjectInput {
                name: "With Seed".into(),
                root_path: None,
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
        let (conn, app_data) = setup();
        let project = create(
            &conn,
            app_data.path(),
            CreateProjectInput {
                name: "Test".into(),
                root_path: None,
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
        let (conn, app_data) = setup();
        let project = create(
            &conn,
            app_data.path(),
            CreateProjectInput {
                name: "To Delete".into(),
                root_path: None,
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
        let (conn, _app_data) = setup();
        let result = delete(&conn, "nonexistent");
        assert!(matches!(result, Err(CoreError::NotFound { .. })));
    }
}
