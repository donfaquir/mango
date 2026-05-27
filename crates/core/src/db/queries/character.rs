use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::character::{
    Character, CreateCharacterInput, ListCharactersOptions, UpdateCharacterInput,
};

const SELECT_COLUMNS: &str = "id, project_id, name, description, appearance_prompt, \
                              reference_image_path, created_at, updated_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<Character> {
    Ok(Character {
        id: row.get(0)?,
        project_id: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        appearance_prompt: row.get(4)?,
        reference_image_path: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

pub fn create(conn: &Connection, input: CreateCharacterInput) -> Result<Character> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(CoreError::Validation("name cannot be empty".into()));
    }
    if input.project_id.trim().is_empty() {
        return Err(CoreError::Validation("project_id is required".into()));
    }

    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO character_profile \
            (id, project_id, name, description, appearance_prompt, reference_image_path) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id,
            input.project_id,
            name,
            input.description.unwrap_or_default(),
            input.appearance_prompt.unwrap_or_default(),
            input.reference_image_path,
        ],
    )?;
    get_by_id(conn, &id)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Character> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM character_profile WHERE id = ?1"),
        params![id],
        map_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
            entity: "character",
            id: id.to_string(),
        },
        other => CoreError::Sqlite(other),
    })
}

pub fn list(conn: &Connection, opts: ListCharactersOptions) -> Result<Vec<Character>> {
    let limit = opts.limit.unwrap_or(50).clamp(1, 200);
    let offset = opts.offset.unwrap_or(0).max(0);

    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM character_profile \
         WHERE project_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
    ))?;
    let rows = stmt.query_map(params![opts.project_id, limit, offset], map_row)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(CoreError::from)
}

pub fn update(conn: &Connection, id: &str, input: UpdateCharacterInput) -> Result<Character> {
    let _ = get_by_id(conn, id)?;

    let mut sets: Vec<String> = vec!["updated_at = datetime('now')".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1u32;

    if let Some(name) = input.name {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(CoreError::Validation("name cannot be empty".into()));
        }
        sets.push(format!("name = ?{idx}"));
        params.push(Box::new(name));
        idx += 1;
    }
    if let Some(desc) = input.description {
        sets.push(format!("description = ?{idx}"));
        params.push(Box::new(desc));
        idx += 1;
    }
    if let Some(prompt) = input.appearance_prompt {
        sets.push(format!("appearance_prompt = ?{idx}"));
        params.push(Box::new(prompt));
        idx += 1;
    }
    if let Some(ref_path) = input.reference_image_path {
        sets.push(format!("reference_image_path = ?{idx}"));
        match ref_path {
            None => params.push(Box::new(rusqlite::types::Null)),
            Some(s) => params.push(Box::new(s)),
        }
        idx += 1;
    }

    let sql = format!(
        "UPDATE character_profile SET {} WHERE id = ?{idx}",
        sets.join(", ")
    );
    params.push(Box::new(id.to_string()));
    let refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, refs.as_slice())?;
    get_by_id(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let n = conn.execute("DELETE FROM character_profile WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(CoreError::NotFound {
            entity: "character",
            id: id.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use crate::db::queries::project as project_queries;
    use crate::models::project::CreateProjectInput;
    use std::path::Path;
    use tempfile::{tempdir, TempDir};

    fn setup_with_project() -> (Connection, TempDir, String) {
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
        (conn, app_data, project.id)
    }

    #[test]
    fn create_then_get_roundtrip() {
        let (conn, _td, pid) = setup_with_project();
        let c = create(
            &conn,
            CreateCharacterInput {
                project_id: pid.clone(),
                name: "Alice".into(),
                description: Some("hero".into()),
                appearance_prompt: Some("red hair".into()),
                reference_image_path: None,
            },
        )
        .unwrap();
        assert_eq!(c.name, "Alice");
        assert_eq!(c.project_id, pid);
        assert_eq!(c.appearance_prompt, "red hair");
        assert!(c.reference_image_path.is_none());

        let fetched = get_by_id(&conn, &c.id).unwrap();
        assert_eq!(fetched.id, c.id);
    }

    #[test]
    fn create_empty_name_fails() {
        let (conn, _td, pid) = setup_with_project();
        let r = create(
            &conn,
            CreateCharacterInput {
                project_id: pid,
                name: "   ".into(),
                description: None,
                appearance_prompt: None,
                reference_image_path: None,
            },
        );
        assert!(matches!(r, Err(CoreError::Validation(_))));
    }

    #[test]
    fn create_missing_project_fails() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let r = create(
            &conn,
            CreateCharacterInput {
                project_id: "no-such-project".into(),
                name: "A".into(),
                description: None,
                appearance_prompt: None,
                reference_image_path: None,
            },
        );
        // FK violation surfaces as Sqlite error
        assert!(matches!(r, Err(CoreError::Sqlite(_))));
    }

    #[test]
    fn list_filters_by_project() {
        let (conn, td, pid_a) = setup_with_project();
        let pid_b = project_queries::create(
            &conn,
            td.path(),
            CreateProjectInput {
                name: "B".into(),
                root_path: None,
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap()
        .id;

        for n in 0..3 {
            create(
                &conn,
                CreateCharacterInput {
                    project_id: pid_a.clone(),
                    name: format!("a{n}"),
                    description: None,
                    appearance_prompt: None,
                    reference_image_path: None,
                },
            )
            .unwrap();
        }
        for n in 0..2 {
            create(
                &conn,
                CreateCharacterInput {
                    project_id: pid_b.clone(),
                    name: format!("b{n}"),
                    description: None,
                    appearance_prompt: None,
                    reference_image_path: None,
                },
            )
            .unwrap();
        }

        let a = list(
            &conn,
            ListCharactersOptions {
                project_id: pid_a,
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        assert_eq!(a.len(), 3);
        let b = list(
            &conn,
            ListCharactersOptions {
                project_id: pid_b,
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        assert_eq!(b.len(), 2);
    }

    #[test]
    fn list_default_limit_caps_at_50() {
        let (conn, _td, pid) = setup_with_project();
        for n in 0..51 {
            create(
                &conn,
                CreateCharacterInput {
                    project_id: pid.clone(),
                    name: format!("c{n}"),
                    description: None,
                    appearance_prompt: None,
                    reference_image_path: None,
                },
            )
            .unwrap();
        }
        let rows = list(
            &conn,
            ListCharactersOptions {
                project_id: pid,
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        assert_eq!(rows.len(), 50);
    }

    #[test]
    fn update_partial_and_bump_updated_at() {
        let (conn, _td, pid) = setup_with_project();
        let c = create(
            &conn,
            CreateCharacterInput {
                project_id: pid,
                name: "A".into(),
                description: None,
                appearance_prompt: None,
                reference_image_path: Some("/tmp/a.png".into()),
            },
        )
        .unwrap();
        // Ensure datetime('now') advances at least 1 second.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let updated = update(
            &conn,
            &c.id,
            UpdateCharacterInput {
                name: Some("AA".into()),
                description: None,
                appearance_prompt: None,
                reference_image_path: Some(None),
            },
        )
        .unwrap();
        assert_eq!(updated.name, "AA");
        assert!(updated.reference_image_path.is_none());
        assert!(updated.updated_at > c.updated_at);
    }

    #[test]
    fn update_empty_name_fails() {
        let (conn, _td, pid) = setup_with_project();
        let c = create(
            &conn,
            CreateCharacterInput {
                project_id: pid,
                name: "A".into(),
                description: None,
                appearance_prompt: None,
                reference_image_path: None,
            },
        )
        .unwrap();
        let r = update(
            &conn,
            &c.id,
            UpdateCharacterInput {
                name: Some(" ".into()),
                description: None,
                appearance_prompt: None,
                reference_image_path: None,
            },
        );
        assert!(matches!(r, Err(CoreError::Validation(_))));
    }

    #[test]
    fn delete_then_get_returns_not_found() {
        let (conn, _td, pid) = setup_with_project();
        let c = create(
            &conn,
            CreateCharacterInput {
                project_id: pid,
                name: "A".into(),
                description: None,
                appearance_prompt: None,
                reference_image_path: None,
            },
        )
        .unwrap();
        delete(&conn, &c.id).unwrap();
        assert!(matches!(
            get_by_id(&conn, &c.id),
            Err(CoreError::NotFound { .. })
        ));
    }
}
