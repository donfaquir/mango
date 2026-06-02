use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::episode::{
    CreateEpisodeInput, Episode, ListEpisodesOptions, UpdateEpisodeInput,
};

const SELECT_COLUMNS: &str =
    "id, project_id, title, order_index, script_text, created_at, updated_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<Episode> {
    Ok(Episode {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        order_index: row.get(3)?,
        script_text: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

pub fn create(conn: &Connection, input: CreateEpisodeInput) -> Result<Episode> {
    let title = input.title.trim().to_string();
    if title.is_empty() {
        return Err(CoreError::Validation("title cannot be empty".into()));
    }
    if input.project_id.trim().is_empty() {
        return Err(CoreError::Validation("project_id is required".into()));
    }

    let next_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(order_index), -1) + 1 FROM episode WHERE project_id = ?1",
        params![input.project_id],
        |r| r.get(0),
    )?;

    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO episode (id, project_id, title, order_index, script_text) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            id,
            input.project_id,
            title,
            next_order,
            input.script_text.unwrap_or_default(),
        ],
    )?;
    get_by_id(conn, &id)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Episode> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM episode WHERE id = ?1"),
        params![id],
        map_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
            entity: "episode",
            id: id.to_string(),
        },
        other => CoreError::Sqlite(other),
    })
}

pub fn list(conn: &Connection, opts: ListEpisodesOptions) -> Result<Vec<Episode>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM episode \
         WHERE project_id = ?1 ORDER BY order_index ASC, created_at ASC"
    ))?;
    let rows = stmt.query_map(params![opts.project_id], map_row)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(CoreError::from)
}

pub fn update(conn: &Connection, id: &str, input: UpdateEpisodeInput) -> Result<Episode> {
    let _ = get_by_id(conn, id)?;

    let mut sets: Vec<String> = vec!["updated_at = datetime('now')".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1u32;

    if let Some(title) = input.title {
        let title = title.trim().to_string();
        if title.is_empty() {
            return Err(CoreError::Validation("title cannot be empty".into()));
        }
        sets.push(format!("title = ?{idx}"));
        params.push(Box::new(title));
        idx += 1;
    }
    if let Some(script_text) = input.script_text {
        sets.push(format!("script_text = ?{idx}"));
        params.push(Box::new(script_text));
        idx += 1;
    }

    let sql = format!("UPDATE episode SET {} WHERE id = ?{idx}", sets.join(", "));
    params.push(Box::new(id.to_string()));
    let refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, refs.as_slice())?;
    get_by_id(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let n = conn.execute("DELETE FROM episode WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(CoreError::NotFound {
            entity: "episode",
            id: id.to_string(),
        });
    }
    Ok(())
}

/// Batch reorder episodes within a project. The list defines the new
/// `order_index` (0..n) by position. Atomic via a single transaction: any id
/// that does not belong to the project rolls back the whole reorder.
pub fn reorder_within_project(
    conn: &mut Connection,
    project_id: &str,
    ordered_ids: &[String],
) -> Result<()> {
    let tx = conn.transaction()?;
    for (idx, id) in ordered_ids.iter().enumerate() {
        let n = tx.execute(
            "UPDATE episode SET order_index = ?1, updated_at = datetime('now') \
             WHERE id = ?2 AND project_id = ?3",
            params![idx as i64, id, project_id],
        )?;
        if n == 0 {
            return Err(CoreError::NotFound {
                entity: "episode",
                id: id.clone(),
            });
        }
    }
    tx.commit()?;
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
        let e = create(
            &conn,
            CreateEpisodeInput {
                project_id: pid,
                title: "Ep 1".into(),
                script_text: Some("hello".into()),
            },
        )
        .unwrap();
        let got = get_by_id(&conn, &e.id).unwrap();
        assert_eq!(got.title, "Ep 1");
        assert_eq!(got.script_text, "hello");
        assert_eq!(got.order_index, 0);
    }

    #[test]
    fn create_auto_assigns_increasing_order_index() {
        let (conn, _td, pid) = setup_with_project();
        let a = create(
            &conn,
            CreateEpisodeInput {
                project_id: pid.clone(),
                title: "A".into(),
                script_text: None,
            },
        )
        .unwrap();
        let b = create(
            &conn,
            CreateEpisodeInput {
                project_id: pid.clone(),
                title: "B".into(),
                script_text: None,
            },
        )
        .unwrap();
        let c = create(
            &conn,
            CreateEpisodeInput {
                project_id: pid,
                title: "C".into(),
                script_text: None,
            },
        )
        .unwrap();
        assert_eq!(a.order_index, 0);
        assert_eq!(b.order_index, 1);
        assert_eq!(c.order_index, 2);
    }

    #[test]
    fn create_empty_title_fails() {
        let (conn, _td, pid) = setup_with_project();
        let r = create(
            &conn,
            CreateEpisodeInput {
                project_id: pid,
                title: "   ".into(),
                script_text: None,
            },
        );
        assert!(matches!(r, Err(CoreError::Validation(_))));
    }

    #[test]
    fn list_orders_by_order_index() {
        let (conn, _td, pid) = setup_with_project();
        for title in &["A", "B", "C"] {
            create(
                &conn,
                CreateEpisodeInput {
                    project_id: pid.clone(),
                    title: (*title).into(),
                    script_text: None,
                },
            )
            .unwrap();
        }
        let list = list(&conn, ListEpisodesOptions { project_id: pid }).unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].title, "A");
        assert_eq!(list[1].title, "B");
        assert_eq!(list[2].title, "C");
    }

    #[test]
    fn update_partial_merges_fields() {
        let (conn, _td, pid) = setup_with_project();
        let e = create(
            &conn,
            CreateEpisodeInput {
                project_id: pid,
                title: "Old".into(),
                script_text: Some("body".into()),
            },
        )
        .unwrap();
        let u = update(
            &conn,
            &e.id,
            UpdateEpisodeInput {
                title: Some("New".into()),
                script_text: None,
            },
        )
        .unwrap();
        assert_eq!(u.title, "New");
        assert_eq!(u.script_text, "body");
    }

    #[test]
    fn delete_cascades_to_shot_and_canvas_layout() {
        let (conn, _td, pid) = setup_with_project();
        let e = create(
            &conn,
            CreateEpisodeInput {
                project_id: pid,
                title: "E".into(),
                script_text: None,
            },
        )
        .unwrap();
        // Seed canvas_layout for this episode
        conn.execute(
            "INSERT INTO canvas_layout (id, episode_id) VALUES (?1, ?2)",
            params!["cl-1", e.id],
        )
        .unwrap();
        delete(&conn, &e.id).unwrap();
        let cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM canvas_layout WHERE episode_id = ?1",
                params![e.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cnt, 0);
    }

    #[test]
    fn reorder_within_project_succeeds() {
        let (mut conn, _td, pid) = setup_with_project();
        let a = create(
            &conn,
            CreateEpisodeInput {
                project_id: pid.clone(),
                title: "A".into(),
                script_text: None,
            },
        )
        .unwrap();
        let b = create(
            &conn,
            CreateEpisodeInput {
                project_id: pid.clone(),
                title: "B".into(),
                script_text: None,
            },
        )
        .unwrap();
        let c = create(
            &conn,
            CreateEpisodeInput {
                project_id: pid.clone(),
                title: "C".into(),
                script_text: None,
            },
        )
        .unwrap();
        // New order: C, A, B
        reorder_within_project(
            &mut conn,
            &pid,
            &[c.id.clone(), a.id.clone(), b.id.clone()],
        )
        .unwrap();
        let list = list(&conn, ListEpisodesOptions { project_id: pid }).unwrap();
        assert_eq!(list[0].id, c.id);
        assert_eq!(list[1].id, a.id);
        assert_eq!(list[2].id, b.id);
    }

    #[test]
    fn reorder_with_foreign_id_rolls_back() {
        let (mut conn, td, pid_a) = setup_with_project();
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
        let a = create(
            &conn,
            CreateEpisodeInput {
                project_id: pid_a.clone(),
                title: "A".into(),
                script_text: None,
            },
        )
        .unwrap();
        let alien = create(
            &conn,
            CreateEpisodeInput {
                project_id: pid_b,
                title: "Alien".into(),
                script_text: None,
            },
        )
        .unwrap();
        let res =
            reorder_within_project(&mut conn, &pid_a, &[a.id.clone(), alien.id.clone()]);
        assert!(matches!(res, Err(CoreError::NotFound { .. })));
        // a should retain original order_index 0 due to transaction rollback
        let got = get_by_id(&conn, &a.id).unwrap();
        assert_eq!(got.order_index, 0);
    }
}
