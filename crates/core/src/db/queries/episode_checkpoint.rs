use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::queries::canvas_layout as canvas_queries;
use crate::db::queries::episode as episode_queries;
use crate::db::queries::shot as shot_queries;
use crate::error::{CoreError, Result};
use crate::models::episode_checkpoint::{
    CreateCheckpointInput, EpisodeCheckpoint, EpisodeCheckpointListItem,
};
use crate::models::shot::ListShotsOptions;

const SELECT_COLUMNS: &str = "id, episode_id, version_number, label, trigger_type, \
     script_text, shots_json, canvas_nodes_json, canvas_edges_json, \
     canvas_viewport_json, change_summary, created_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<EpisodeCheckpoint> {
    Ok(EpisodeCheckpoint {
        id: row.get(0)?,
        episode_id: row.get(1)?,
        version_number: row.get(2)?,
        label: row.get(3)?,
        trigger_type: row.get(4)?,
        script_text: row.get(5)?,
        shots_json: row.get(6)?,
        canvas_nodes_json: row.get(7)?,
        canvas_edges_json: row.get(8)?,
        canvas_viewport_json: row.get(9)?,
        change_summary: row.get(10)?,
        created_at: row.get(11)?,
    })
}

const LIST_COLUMNS: &str = "id, episode_id, version_number, trigger_type, label, \
     change_summary, created_at";

fn map_list_row(row: &rusqlite::Row) -> rusqlite::Result<EpisodeCheckpointListItem> {
    Ok(EpisodeCheckpointListItem {
        id: row.get(0)?,
        episode_id: row.get(1)?,
        version_number: row.get(2)?,
        trigger_type: row.get(3)?,
        label: row.get(4)?,
        change_summary: row.get(5)?,
        created_at: row.get(6)?,
    })
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<EpisodeCheckpoint> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM episode_checkpoint WHERE id = ?1"),
        params![id],
        map_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
            entity: "episode_checkpoint",
            id: id.to_string(),
        },
        other => CoreError::Sqlite(other),
    })
}

pub fn list(conn: &Connection, episode_id: &str) -> Result<Vec<EpisodeCheckpointListItem>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {LIST_COLUMNS} FROM episode_checkpoint \
         WHERE episode_id = ?1 ORDER BY version_number DESC"
    ))?;
    let rows = stmt.query_map(params![episode_id], map_list_row)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(CoreError::from)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let n = conn.execute(
        "DELETE FROM episode_checkpoint WHERE id = ?1",
        params![id],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound {
            entity: "episode_checkpoint",
            id: id.to_string(),
        });
    }
    Ok(())
}

pub fn insert_full(
    conn: &Connection,
    input: &CreateCheckpointInput,
) -> Result<EpisodeCheckpoint> {
    if input.episode_id.trim().is_empty() {
        return Err(CoreError::Validation("episode_id is required".into()));
    }

    let episode = episode_queries::get_by_id(conn, &input.episode_id)?;
    let shots = shot_queries::list(
        conn,
        ListShotsOptions {
            episode_id: input.episode_id.clone(),
        },
    )?;
    let shots_json = serde_json::to_string(&shots)
        .map_err(|e| CoreError::Validation(format!("failed to serialize shots: {e}")))?;

    let (nodes_json, edges_json, viewport_json) =
        match canvas_queries::get_by_episode(conn, &input.episode_id)? {
            Some(layout) => (layout.nodes_json, layout.edges_json, layout.viewport_json),
            None => ("[]".into(), "[]".into(), "{}".into()),
        };

    let change_summary = super::episode_checkpoint_ops::compute_change_summary(
        conn,
        &input.episode_id,
        &episode.script_text,
        &shots_json,
        &nodes_json,
    );

    let next_version: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version_number), 0) + 1 \
         FROM episode_checkpoint WHERE episode_id = ?1",
        params![input.episode_id],
        |r| r.get(0),
    )?;

    let id = Uuid::new_v4().to_string();
    let label = input
        .label
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let trigger = input
        .trigger_type
        .as_deref()
        .unwrap_or("manual");

    conn.execute(
        "INSERT INTO episode_checkpoint \
            (id, episode_id, version_number, label, trigger_type, \
             script_text, shots_json, \
             canvas_nodes_json, canvas_edges_json, canvas_viewport_json, \
             change_summary) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            id,
            input.episode_id,
            next_version,
            label,
            trigger,
            episode.script_text,
            shots_json,
            nodes_json,
            edges_json,
            viewport_json,
            change_summary,
        ],
    )?;

    let _ = super::episode_checkpoint_ops::gc_auto_checkpoints(conn, &input.episode_id);

    get_by_id(conn, &id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use crate::db::queries::{
        canvas_layout as canvas_q, episode as episode_queries, project as project_queries,
        shot as shot_queries,
    };
    use crate::models::canvas_layout::UpsertCanvasLayoutInput;
    use crate::models::episode::CreateEpisodeInput;
    use crate::models::project::CreateProjectInput;
    use crate::models::shot::CreateShotInput;
    use std::path::Path;
    use tempfile::{tempdir, TempDir};

    fn setup_with_layout() -> (Connection, TempDir, String) {
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
        let episode = episode_queries::create(
            &conn,
            CreateEpisodeInput {
                project_id: project.id,
                title: "Ep 1".into(),
                script_text: Some("hello world".into()),
            },
        )
        .unwrap();
        canvas_q::upsert(
            &conn,
            UpsertCanvasLayoutInput {
                episode_id: episode.id.clone(),
                nodes_json: "[{\"id\":\"n1\"}]".into(),
                edges_json: "[]".into(),
                viewport_json: "{\"x\":0,\"y\":0,\"zoom\":1}".into(),
            },
        )
        .unwrap();
        (conn, app_data, episode.id)
    }

    #[test]
    fn insert_full_snapshots_script_and_shots() {
        let (conn, _td, eid) = setup_with_layout();
        shot_queries::create(
            &conn,
            CreateShotInput {
                episode_id: eid.clone(),
                summary: Some("opening".into()),
            },
        )
        .unwrap();

        let cp = insert_full(
            &conn,
            &CreateCheckpointInput {
                episode_id: eid.clone(),
                label: Some("v1".into()),
                trigger_type: None,
            },
        )
        .unwrap();

        assert_eq!(cp.script_text, "hello world");
        assert!(cp.shots_json.contains("opening"));
        assert_eq!(cp.trigger_type, "manual");
        assert_eq!(cp.label.as_deref(), Some("v1"));
        assert_eq!(cp.version_number, 1);
        assert_eq!(cp.canvas_nodes_json, "[{\"id\":\"n1\"}]");
    }

    #[test]
    fn insert_full_without_canvas_uses_defaults() {
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
        let episode = episode_queries::create(
            &conn,
            CreateEpisodeInput {
                project_id: project.id,
                title: "Ep".into(),
                script_text: None,
            },
        )
        .unwrap();

        let cp = insert_full(
            &conn,
            &CreateCheckpointInput {
                episode_id: episode.id,
                label: None,
                trigger_type: None,
            },
        )
        .unwrap();

        assert_eq!(cp.canvas_nodes_json, "[]");
        assert_eq!(cp.canvas_edges_json, "[]");
        assert_eq!(cp.canvas_viewport_json, "{}");
    }

    #[test]
    fn insert_full_auto_trigger_type() {
        let (conn, _td, eid) = setup_with_layout();
        let cp = insert_full(
            &conn,
            &CreateCheckpointInput {
                episode_id: eid,
                label: None,
                trigger_type: Some("auto".into()),
            },
        )
        .unwrap();
        assert_eq!(cp.trigger_type, "auto");
    }

    #[test]
    fn list_returns_items_in_desc_order() {
        let (conn, _td, eid) = setup_with_layout();
        insert_full(
            &conn,
            &CreateCheckpointInput {
                episode_id: eid.clone(),
                label: Some("first".into()),
                trigger_type: None,
            },
        )
        .unwrap();
        insert_full(
            &conn,
            &CreateCheckpointInput {
                episode_id: eid.clone(),
                label: Some("second".into()),
                trigger_type: None,
            },
        )
        .unwrap();

        let items = list(&conn, &eid).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].label.as_deref(), Some("second"));
        assert_eq!(items[1].label.as_deref(), Some("first"));
    }

    #[test]
    fn delete_removes_checkpoint() {
        let (conn, _td, eid) = setup_with_layout();
        let cp = insert_full(
            &conn,
            &CreateCheckpointInput {
                episode_id: eid.clone(),
                label: None,
                trigger_type: None,
            },
        )
        .unwrap();
        delete(&conn, &cp.id).unwrap();
        assert!(matches!(
            get_by_id(&conn, &cp.id),
            Err(CoreError::NotFound { .. })
        ));
    }

    #[test]
    fn insert_full_rejects_blank_episode_id() {
        let (conn, _td, _eid) = setup_with_layout();
        let err = insert_full(
            &conn,
            &CreateCheckpointInput {
                episode_id: "  ".into(),
                label: None,
                trigger_type: None,
            },
        )
        .unwrap_err();
        assert!(matches!(err, CoreError::Validation(_)));
    }
}
