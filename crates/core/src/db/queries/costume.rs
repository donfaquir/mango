use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::costume::{
    Costume, CreateCostumeInput, ListCostumesOptions, UpdateCostumeInput,
};

const SELECT_COLUMNS: &str = "id, project_id, character_id, name, description, \
                              reference_image_path, created_at, updated_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<Costume> {
    Ok(Costume {
        id: row.get(0)?,
        project_id: row.get(1)?,
        character_id: row.get(2)?,
        name: row.get(3)?,
        description: row.get(4)?,
        reference_image_path: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

pub fn create(conn: &Connection, input: CreateCostumeInput) -> Result<Costume> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(CoreError::Validation("name cannot be empty".into()));
    }
    if input.project_id.trim().is_empty() {
        return Err(CoreError::Validation("project_id is required".into()));
    }
    if input.character_id.trim().is_empty() {
        return Err(CoreError::Validation("character_id is required".into()));
    }

    // TOCTOU note: between this check and the INSERT another writer could
    // delete the character row. For a single-user desktop app the window is
    // effectively closed; spec-11 explicitly accepts this in lieu of an
    // explicit transaction wrapper.
    let belongs: i64 = conn.query_row(
        "SELECT COUNT(*) FROM character_profile WHERE id = ?1 AND project_id = ?2",
        params![input.character_id, input.project_id],
        |r| r.get(0),
    )?;
    if belongs == 0 {
        return Err(CoreError::Validation(
            "character_id does not belong to project_id".into(),
        ));
    }

    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO costume \
            (id, project_id, character_id, name, description, reference_image_path) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id,
            input.project_id,
            input.character_id,
            name,
            input.description.unwrap_or_default(),
            input.reference_image_path,
        ],
    )?;
    get_by_id(conn, &id)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Costume> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM costume WHERE id = ?1"),
        params![id],
        map_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
            entity: "costume",
            id: id.to_string(),
        },
        other => CoreError::Sqlite(other),
    })
}

pub fn list(conn: &Connection, opts: ListCostumesOptions) -> Result<Vec<Costume>> {
    let limit = opts.limit.unwrap_or(50).clamp(1, 200);
    let offset = opts.offset.unwrap_or(0).max(0);

    let rows: Vec<Costume> = match opts.character_id {
        Some(cid) => {
            let mut stmt = conn.prepare(&format!(
                "SELECT {SELECT_COLUMNS} FROM costume \
                 WHERE project_id = ?1 AND character_id = ?2 \
                 ORDER BY created_at DESC LIMIT ?3 OFFSET ?4"
            ))?;
            stmt.query_map(params![opts.project_id, cid, limit, offset], map_row)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        }
        None => {
            let mut stmt = conn.prepare(&format!(
                "SELECT {SELECT_COLUMNS} FROM costume \
                 WHERE project_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
            ))?;
            stmt.query_map(params![opts.project_id, limit, offset], map_row)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        }
    };
    Ok(rows)
}

pub fn update(conn: &Connection, id: &str, input: UpdateCostumeInput) -> Result<Costume> {
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
    if let Some(ref_path) = input.reference_image_path {
        sets.push(format!("reference_image_path = ?{idx}"));
        match ref_path {
            None => params.push(Box::new(rusqlite::types::Null)),
            Some(s) => params.push(Box::new(s)),
        }
        idx += 1;
    }

    let sql = format!("UPDATE costume SET {} WHERE id = ?{idx}", sets.join(", "));
    params.push(Box::new(id.to_string()));
    let refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, refs.as_slice())?;
    get_by_id(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let n = conn.execute("DELETE FROM costume WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(CoreError::NotFound {
            entity: "costume",
            id: id.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use crate::db::queries::{character as char_q, project as project_queries};
    use crate::models::character::CreateCharacterInput;
    use crate::models::project::CreateProjectInput;
    use std::path::Path;
    use tempfile::{tempdir, TempDir};

    fn setup() -> (Connection, TempDir, String, String) {
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
        let character = char_q::create(
            &conn,
            CreateCharacterInput {
                project_id: project.id.clone(),
                name: "Hero".into(),
                description: None,
                appearance_prompt: None,
                reference_image_path: None,
            },
        )
        .unwrap();
        (conn, app_data, project.id, character.id)
    }

    #[test]
    fn create_then_get_roundtrip() {
        let (conn, _td, pid, cid) = setup();
        let cos = create(
            &conn,
            CreateCostumeInput {
                project_id: pid.clone(),
                character_id: cid.clone(),
                name: "Cloak".into(),
                description: None,
                reference_image_path: None,
            },
        )
        .unwrap();
        assert_eq!(cos.character_id, cid);
        assert_eq!(cos.project_id, pid);
        assert_eq!(get_by_id(&conn, &cos.id).unwrap().name, "Cloak");
    }

    #[test]
    fn create_in_wrong_project_fails() {
        let (conn, td, _pid_a, cid_a) = setup();
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
        let r = create(
            &conn,
            CreateCostumeInput {
                project_id: pid_b,
                character_id: cid_a,
                name: "X".into(),
                description: None,
                reference_image_path: None,
            },
        );
        assert!(matches!(r, Err(CoreError::Validation(msg)) if msg.contains("character_id")));
    }

    #[test]
    fn create_empty_name_fails() {
        let (conn, _td, pid, cid) = setup();
        let r = create(
            &conn,
            CreateCostumeInput {
                project_id: pid,
                character_id: cid,
                name: "  ".into(),
                description: None,
                reference_image_path: None,
            },
        );
        assert!(matches!(r, Err(CoreError::Validation(_))));
    }

    #[test]
    fn list_filters_by_project_and_character() {
        let (conn, _td, pid, cid_a) = setup();
        let cid_b = char_q::create(
            &conn,
            CreateCharacterInput {
                project_id: pid.clone(),
                name: "Villain".into(),
                description: None,
                appearance_prompt: None,
                reference_image_path: None,
            },
        )
        .unwrap()
        .id;
        for _ in 0..2 {
            create(
                &conn,
                CreateCostumeInput {
                    project_id: pid.clone(),
                    character_id: cid_a.clone(),
                    name: "ca".into(),
                    description: None,
                    reference_image_path: None,
                },
            )
            .unwrap();
        }
        create(
            &conn,
            CreateCostumeInput {
                project_id: pid.clone(),
                character_id: cid_b.clone(),
                name: "cb".into(),
                description: None,
                reference_image_path: None,
            },
        )
        .unwrap();

        let all = list(
            &conn,
            ListCostumesOptions {
                project_id: pid.clone(),
                character_id: None,
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        assert_eq!(all.len(), 3);

        let only_a = list(
            &conn,
            ListCostumesOptions {
                project_id: pid,
                character_id: Some(cid_a),
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        assert_eq!(only_a.len(), 2);
    }

    #[test]
    fn delete_character_cascades_costume() {
        let (conn, _td, pid, cid) = setup();
        let cos = create(
            &conn,
            CreateCostumeInput {
                project_id: pid,
                character_id: cid.clone(),
                name: "C".into(),
                description: None,
                reference_image_path: None,
            },
        )
        .unwrap();
        char_q::delete(&conn, &cid).unwrap();
        assert!(matches!(
            get_by_id(&conn, &cos.id),
            Err(CoreError::NotFound { .. })
        ));
    }

    #[test]
    fn update_partial() {
        let (conn, _td, pid, cid) = setup();
        let cos = create(
            &conn,
            CreateCostumeInput {
                project_id: pid,
                character_id: cid,
                name: "A".into(),
                description: None,
                reference_image_path: Some("/old.png".into()),
            },
        )
        .unwrap();
        let u = update(
            &conn,
            &cos.id,
            UpdateCostumeInput {
                name: Some("B".into()),
                description: None,
                reference_image_path: Some(None),
            },
        )
        .unwrap();
        assert_eq!(u.name, "B");
        assert!(u.reference_image_path.is_none());
    }

    #[test]
    fn delete_then_get_returns_not_found() {
        let (conn, _td, pid, cid) = setup();
        let cos = create(
            &conn,
            CreateCostumeInput {
                project_id: pid,
                character_id: cid,
                name: "C".into(),
                description: None,
                reference_image_path: None,
            },
        )
        .unwrap();
        delete(&conn, &cos.id).unwrap();
        assert!(matches!(
            get_by_id(&conn, &cos.id),
            Err(CoreError::NotFound { .. })
        ));
    }
}
