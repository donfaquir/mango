use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::canvas_layout::{CanvasLayout, UpsertCanvasLayoutInput};

const SELECT_COLUMNS: &str =
    "id, episode_id, nodes_json, edges_json, viewport_json, updated_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<CanvasLayout> {
    Ok(CanvasLayout {
        id: row.get(0)?,
        episode_id: row.get(1)?,
        nodes_json: row.get(2)?,
        edges_json: row.get(3)?,
        viewport_json: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

pub fn get_by_episode(conn: &Connection, episode_id: &str) -> Result<Option<CanvasLayout>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM canvas_layout WHERE episode_id = ?1"
    ))?;
    let row = stmt
        .query_row(params![episode_id], map_row)
        .optional()?;
    Ok(row)
}

pub fn upsert(conn: &Connection, input: UpsertCanvasLayoutInput) -> Result<CanvasLayout> {
    if input.episode_id.trim().is_empty() {
        return Err(CoreError::Validation("episode_id is required".into()));
    }
    validate_json(&input.nodes_json, "nodes_json")?;
    validate_json(&input.edges_json, "edges_json")?;
    validate_json(&input.viewport_json, "viewport_json")?;

    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO canvas_layout (id, episode_id, nodes_json, edges_json, viewport_json, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now')) \
         ON CONFLICT(episode_id) DO UPDATE SET \
            nodes_json = excluded.nodes_json, \
            edges_json = excluded.edges_json, \
            viewport_json = excluded.viewport_json, \
            updated_at = datetime('now')",
        params![
            id,
            input.episode_id,
            input.nodes_json,
            input.edges_json,
            input.viewport_json,
        ],
    )?;

    get_by_episode(conn, &input.episode_id)?.ok_or_else(|| CoreError::NotFound {
        entity: "canvas_layout",
        id: input.episode_id.clone(),
    })
}

pub fn delete_by_episode(conn: &Connection, episode_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM canvas_layout WHERE episode_id = ?1",
        params![episode_id],
    )?;
    Ok(())
}

fn validate_json(s: &str, field: &'static str) -> Result<()> {
    serde_json::from_str::<serde_json::Value>(s)
        .map_err(|e| CoreError::Validation(format!("{field} is not valid JSON: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use crate::db::queries::episode as episode_queries;
    use crate::db::queries::project as project_queries;
    use crate::models::episode::CreateEpisodeInput;
    use crate::models::project::CreateProjectInput;
    use std::path::Path;
    use tempfile::{tempdir, TempDir};

    fn setup_with_episode() -> (Connection, TempDir, String) {
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
        let episode = episode_queries::create(
            &conn,
            CreateEpisodeInput {
                project_id: project.id,
                title: "Ep 1".into(),
                script_text: None,
            },
        )
        .unwrap();
        (conn, app_data, episode.id)
    }

    fn sample_input(episode_id: &str) -> UpsertCanvasLayoutInput {
        UpsertCanvasLayoutInput {
            episode_id: episode_id.to_string(),
            nodes_json: "[]".into(),
            edges_json: "[]".into(),
            viewport_json: "{\"x\":0,\"y\":0,\"zoom\":1}".into(),
        }
    }

    #[test]
    fn get_by_episode_returns_none_when_empty() {
        let (conn, _td, eid) = setup_with_episode();
        let got = get_by_episode(&conn, &eid).unwrap();
        assert!(got.is_none());
    }

    #[test]
    fn upsert_inserts_first_then_updates_on_conflict() {
        let (conn, _td, eid) = setup_with_episode();
        let first = upsert(&conn, sample_input(&eid)).unwrap();
        let first_id = first.id.clone();
        let first_updated = first.updated_at.clone();

        // small sleep to let datetime('now') tick (SQLite's resolution is seconds)
        std::thread::sleep(std::time::Duration::from_millis(1100));

        let mut next = sample_input(&eid);
        next.nodes_json = "[{\"id\":\"n1\"}]".into();
        let second = upsert(&conn, next).unwrap();

        assert_eq!(second.id, first_id, "upsert keeps the same pk row");
        assert_eq!(second.nodes_json, "[{\"id\":\"n1\"}]");
        assert!(
            second.updated_at >= first_updated,
            "updated_at should not regress: {} -> {}",
            first_updated,
            second.updated_at
        );

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM canvas_layout WHERE episode_id = ?1",
                params![eid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "still exactly one row per episode");
    }

    #[test]
    fn upsert_rejects_invalid_json() {
        let (conn, _td, eid) = setup_with_episode();
        let mut bad = sample_input(&eid);
        bad.nodes_json = "{".into();

        let err = upsert(&conn, bad).unwrap_err();
        assert!(
            matches!(err, CoreError::Validation(_)),
            "expected Validation, got: {err:?}"
        );

        // Confirm we did not write a partial row.
        assert!(get_by_episode(&conn, &eid).unwrap().is_none());
    }

    #[test]
    fn upsert_rejects_blank_episode_id() {
        let (conn, _td, _eid) = setup_with_episode();
        let bad = UpsertCanvasLayoutInput {
            episode_id: "  ".into(),
            nodes_json: "[]".into(),
            edges_json: "[]".into(),
            viewport_json: "{}".into(),
        };
        let err = upsert(&conn, bad).unwrap_err();
        assert!(matches!(err, CoreError::Validation(_)));
    }

    #[test]
    fn delete_by_episode_removes_row() {
        let (conn, _td, eid) = setup_with_episode();
        upsert(&conn, sample_input(&eid)).unwrap();
        delete_by_episode(&conn, &eid).unwrap();
        assert!(get_by_episode(&conn, &eid).unwrap().is_none());
    }

    #[test]
    fn delete_episode_cascades_to_canvas_layout() {
        let (conn, _td, eid) = setup_with_episode();
        upsert(&conn, sample_input(&eid)).unwrap();

        episode_queries::delete(&conn, &eid).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM canvas_layout WHERE episode_id = ?1",
                params![eid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "FK ON DELETE CASCADE should remove the layout");
    }
}
