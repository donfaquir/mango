use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::prop::{CreatePropInput, ListPropsOptions, Prop, UpdatePropInput};

const SELECT_COLUMNS: &str = "id, project_id, name, description, reference_image_path, \
                              created_at, updated_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<Prop> {
    Ok(Prop {
        id: row.get(0)?,
        project_id: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        reference_image_path: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

pub fn create(conn: &Connection, input: CreatePropInput) -> Result<Prop> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(CoreError::Validation("name cannot be empty".into()));
    }
    if input.project_id.trim().is_empty() {
        return Err(CoreError::Validation("project_id is required".into()));
    }

    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO prop (id, project_id, name, description, reference_image_path) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            id,
            input.project_id,
            name,
            input.description.unwrap_or_default(),
            input.reference_image_path,
        ],
    )?;
    get_by_id(conn, &id)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Prop> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM prop WHERE id = ?1"),
        params![id],
        map_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
            entity: "prop",
            id: id.to_string(),
        },
        other => CoreError::Sqlite(other),
    })
}

pub fn list(conn: &Connection, opts: ListPropsOptions) -> Result<Vec<Prop>> {
    let limit = opts.limit.unwrap_or(50).clamp(1, 200);
    let offset = opts.offset.unwrap_or(0).max(0);

    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM prop \
         WHERE project_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
    ))?;
    let rows = stmt.query_map(params![opts.project_id, limit, offset], map_row)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(CoreError::from)
}

pub fn update(conn: &Connection, id: &str, input: UpdatePropInput) -> Result<Prop> {
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

    let sql = format!("UPDATE prop SET {} WHERE id = ?{idx}", sets.join(", "));
    params.push(Box::new(id.to_string()));
    let refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, refs.as_slice())?;
    get_by_id(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let n = conn.execute("DELETE FROM prop WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(CoreError::NotFound {
            entity: "prop",
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
                subdir: None,
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
        let p = create(
            &conn,
            CreatePropInput {
                project_id: pid,
                name: "Sword".into(),
                description: Some("rusty".into()),
                reference_image_path: None,
            },
        )
        .unwrap();
        assert_eq!(get_by_id(&conn, &p.id).unwrap().description, "rusty");
    }

    #[test]
    fn create_empty_name_fails() {
        let (conn, _td, pid) = setup_with_project();
        let r = create(
            &conn,
            CreatePropInput {
                project_id: pid,
                name: "  ".into(),
                description: None,
                reference_image_path: None,
            },
        );
        assert!(matches!(r, Err(CoreError::Validation(_))));
    }

    #[test]
    fn list_filters_by_project() {
        let (conn, td, pid_a) = setup_with_project();
        let pid_b = project_queries::create(
            &conn,
            td.path(),
            CreateProjectInput {
                name: "B".into(),
                subdir: None,
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap()
        .id;
        create(
            &conn,
            CreatePropInput {
                project_id: pid_a.clone(),
                name: "a".into(),
                description: None,
                reference_image_path: None,
            },
        )
        .unwrap();
        create(
            &conn,
            CreatePropInput {
                project_id: pid_b,
                name: "b".into(),
                description: None,
                reference_image_path: None,
            },
        )
        .unwrap();
        assert_eq!(
            list(
                &conn,
                ListPropsOptions {
                    project_id: pid_a,
                    limit: None,
                    offset: None,
                }
            )
            .unwrap()
            .len(),
            1
        );
    }

    #[test]
    fn update_partial() {
        let (conn, _td, pid) = setup_with_project();
        let p = create(
            &conn,
            CreatePropInput {
                project_id: pid,
                name: "A".into(),
                description: None,
                reference_image_path: Some("/x.png".into()),
            },
        )
        .unwrap();
        let u = update(
            &conn,
            &p.id,
            UpdatePropInput {
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
        let (conn, _td, pid) = setup_with_project();
        let p = create(
            &conn,
            CreatePropInput {
                project_id: pid,
                name: "Z".into(),
                description: None,
                reference_image_path: None,
            },
        )
        .unwrap();
        delete(&conn, &p.id).unwrap();
        assert!(matches!(get_by_id(&conn, &p.id), Err(CoreError::NotFound { .. })));
    }
}
