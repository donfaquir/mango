use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::queries::canvas_layout as canvas_queries;
use crate::error::{CoreError, Result};
use crate::models::episode_checkpoint::{CreateCheckpointInput, EpisodeCheckpoint};

const SELECT_COLUMNS: &str = "id, episode_id, version_number, label, trigger_type, \
     canvas_nodes_json, canvas_edges_json, canvas_viewport_json, created_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<EpisodeCheckpoint> {
    Ok(EpisodeCheckpoint {
        id: row.get(0)?,
        episode_id: row.get(1)?,
        version_number: row.get(2)?,
        label: row.get(3)?,
        trigger_type: row.get(4)?,
        canvas_nodes_json: row.get(5)?,
        canvas_edges_json: row.get(6)?,
        canvas_viewport_json: row.get(7)?,
        created_at: row.get(8)?,
    })
}

/// Minimal placeholder writer for spec-24. Reads the current `canvas_layout`
/// row for the episode and writes its three JSON columns into a new
/// `episode_checkpoint` row alongside an auto-incremented `version_number`
/// scoped to the same episode.
///
/// MS4 will extend this with `script_text` / `shots_json` / `change_summary`
/// and add list / restore / GC operations. The `episode_checkpoint` schema in
/// `001_initial.sql` already has columns for those — they default in this
/// minimal write.
pub fn insert_minimal(
    conn: &Connection,
    input: &CreateCheckpointInput,
) -> Result<EpisodeCheckpoint> {
    if input.episode_id.trim().is_empty() {
        return Err(CoreError::Validation("episode_id is required".into()));
    }

    let layout = canvas_queries::get_by_episode(conn, &input.episode_id)?
        .ok_or_else(|| CoreError::NotFound {
            entity: "canvas_layout",
            id: input.episode_id.clone(),
        })?;

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

    conn.execute(
        "INSERT INTO episode_checkpoint \
            (id, episode_id, version_number, label, trigger_type, \
             canvas_nodes_json, canvas_edges_json, canvas_viewport_json) \
         VALUES (?1, ?2, ?3, ?4, 'manual', ?5, ?6, ?7)",
        params![
            id,
            input.episode_id,
            next_version,
            label,
            layout.nodes_json,
            layout.edges_json,
            layout.viewport_json,
        ],
    )?;

    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM episode_checkpoint WHERE id = ?1"),
        params![id],
        map_row,
    )
    .map_err(CoreError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use crate::db::queries::{
        canvas_layout as canvas_q, episode as episode_queries, project as project_queries,
    };
    use crate::models::canvas_layout::UpsertCanvasLayoutInput;
    use crate::models::episode::CreateEpisodeInput;
    use crate::models::project::CreateProjectInput;
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
                script_text: None,
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

    fn setup_episode_without_layout() -> (Connection, TempDir, String) {
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
                script_text: None,
            },
        )
        .unwrap();
        (conn, app_data, episode.id)
    }

    #[test]
    fn insert_minimal_copies_canvas_columns_from_layout() {
        let (conn, _td, eid) = setup_with_layout();
        let cp = insert_minimal(
            &conn,
            &CreateCheckpointInput {
                episode_id: eid.clone(),
                label: Some("first".into()),
            },
        )
        .unwrap();

        assert_eq!(cp.episode_id, eid);
        assert_eq!(cp.canvas_nodes_json, "[{\"id\":\"n1\"}]");
        assert_eq!(cp.canvas_edges_json, "[]");
        assert_eq!(cp.canvas_viewport_json, "{\"x\":0,\"y\":0,\"zoom\":1}");
        assert_eq!(cp.label.as_deref(), Some("first"));
        assert_eq!(cp.trigger_type, "manual");
        assert_eq!(cp.version_number, 1);
    }

    #[test]
    fn insert_minimal_increments_version_per_episode() {
        let (conn, _td, eid) = setup_with_layout();
        let v1 = insert_minimal(
            &conn,
            &CreateCheckpointInput {
                episode_id: eid.clone(),
                label: None,
            },
        )
        .unwrap();
        let v2 = insert_minimal(
            &conn,
            &CreateCheckpointInput {
                episode_id: eid.clone(),
                label: None,
            },
        )
        .unwrap();
        assert_eq!(v1.version_number, 1);
        assert_eq!(v2.version_number, 2);
        assert_ne!(v1.id, v2.id, "each checkpoint gets its own row id");
    }

    #[test]
    fn insert_minimal_returns_not_found_without_layout() {
        let (conn, _td, eid) = setup_episode_without_layout();
        let err = insert_minimal(
            &conn,
            &CreateCheckpointInput {
                episode_id: eid,
                label: None,
            },
        )
        .unwrap_err();
        assert!(
            matches!(err, CoreError::NotFound { entity: "canvas_layout", .. }),
            "expected NotFound for canvas_layout, got: {err:?}"
        );
    }

    #[test]
    fn insert_minimal_normalizes_blank_label_to_null() {
        let (conn, _td, eid) = setup_with_layout();
        let cp_blank = insert_minimal(
            &conn,
            &CreateCheckpointInput {
                episode_id: eid.clone(),
                label: Some("   ".into()),
            },
        )
        .unwrap();
        let cp_missing = insert_minimal(
            &conn,
            &CreateCheckpointInput {
                episode_id: eid.clone(),
                label: None,
            },
        )
        .unwrap();
        assert_eq!(cp_blank.label, None);
        assert_eq!(cp_missing.label, None);
    }

    #[test]
    fn insert_minimal_rejects_blank_episode_id() {
        let (conn, _td, _eid) = setup_with_layout();
        let err = insert_minimal(
            &conn,
            &CreateCheckpointInput {
                episode_id: "  ".into(),
                label: None,
            },
        )
        .unwrap_err();
        assert!(matches!(err, CoreError::Validation(_)));
    }

    #[test]
    fn insert_minimal_versions_are_per_episode() {
        let (conn, _td, eid_a) = setup_with_layout();

        // Make a second episode with its own layout in the same DB.
        let project = project_queries::list(
            &conn,
            crate::models::project::ListProjectsOptions::default(),
        )
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
        let episode_b = episode_queries::create(
            &conn,
            CreateEpisodeInput {
                project_id: project.id,
                title: "Ep 2".into(),
                script_text: None,
            },
        )
        .unwrap();
        canvas_q::upsert(
            &conn,
            UpsertCanvasLayoutInput {
                episode_id: episode_b.id.clone(),
                nodes_json: "[]".into(),
                edges_json: "[]".into(),
                viewport_json: "{}".into(),
            },
        )
        .unwrap();

        let a1 = insert_minimal(
            &conn,
            &CreateCheckpointInput {
                episode_id: eid_a.clone(),
                label: None,
            },
        )
        .unwrap();
        let a2 = insert_minimal(
            &conn,
            &CreateCheckpointInput {
                episode_id: eid_a,
                label: None,
            },
        )
        .unwrap();
        let b1 = insert_minimal(
            &conn,
            &CreateCheckpointInput {
                episode_id: episode_b.id,
                label: None,
            },
        )
        .unwrap();

        assert_eq!(a1.version_number, 1);
        assert_eq!(a2.version_number, 2);
        assert_eq!(
            b1.version_number, 1,
            "version_number is scoped per episode, not per database"
        );
    }
}
