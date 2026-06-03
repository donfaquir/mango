use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::shot::{
    CreateShotInput, ListShotsOptions, Shot, ShotLinks, ShotStatus, SubjectKind, UpdateShotInput,
};

const SELECT_COLUMNS: &str = "id, episode_id, order_index, summary, duration_sec, \
                              camera_angle, shot_type, mood, dialogue, video_prompt, \
                              image_prompt, status, adopted_asset_id, created_at, updated_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<Shot> {
    let status_str: String = row.get(11)?;
    let status = ShotStatus::from_db_str(&status_str).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            11,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown shot status '{status_str}'"),
            )),
        )
    })?;
    Ok(Shot {
        id: row.get(0)?,
        episode_id: row.get(1)?,
        order_index: row.get(2)?,
        summary: row.get(3)?,
        duration_sec: row.get(4)?,
        camera_angle: row.get(5)?,
        shot_type: row.get(6)?,
        mood: row.get(7)?,
        dialogue: row.get(8)?,
        video_prompt: row.get(9)?,
        image_prompt: row.get(10)?,
        status,
        adopted_asset_id: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

pub fn create(conn: &Connection, input: CreateShotInput) -> Result<Shot> {
    if input.episode_id.trim().is_empty() {
        return Err(CoreError::Validation("episode_id is required".into()));
    }

    let next_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(order_index), -1) + 1 FROM shot WHERE episode_id = ?1",
        params![input.episode_id],
        |r| r.get(0),
    )?;

    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO shot (id, episode_id, order_index, summary) \
         VALUES (?1, ?2, ?3, ?4)",
        params![
            id,
            input.episode_id,
            next_order,
            input.summary.unwrap_or_default(),
        ],
    )?;
    get_by_id(conn, &id)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Shot> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM shot WHERE id = ?1"),
        params![id],
        map_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
            entity: "shot",
            id: id.to_string(),
        },
        other => CoreError::Sqlite(other),
    })
}

pub fn list(conn: &Connection, opts: ListShotsOptions) -> Result<Vec<Shot>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM shot \
         WHERE episode_id = ?1 ORDER BY order_index ASC, created_at ASC"
    ))?;
    let rows = stmt.query_map(params![opts.episode_id], map_row)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(CoreError::from)
}

pub fn update(conn: &Connection, id: &str, input: UpdateShotInput) -> Result<Shot> {
    let _ = get_by_id(conn, id)?;

    let mut sets: Vec<String> = vec!["updated_at = datetime('now')".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1u32;

    if let Some(v) = input.summary {
        sets.push(format!("summary = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.duration_sec {
        sets.push(format!("duration_sec = ?{idx}"));
        match v {
            None => params.push(Box::new(rusqlite::types::Null)),
            Some(n) => params.push(Box::new(n)),
        }
        idx += 1;
    }
    if let Some(v) = input.camera_angle {
        sets.push(format!("camera_angle = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.shot_type {
        sets.push(format!("shot_type = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.mood {
        sets.push(format!("mood = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.dialogue {
        sets.push(format!("dialogue = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.video_prompt {
        sets.push(format!("video_prompt = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.image_prompt {
        sets.push(format!("image_prompt = ?{idx}"));
        params.push(Box::new(v));
        idx += 1;
    }
    if let Some(v) = input.status {
        sets.push(format!("status = ?{idx}"));
        params.push(Box::new(v.as_db_str().to_string()));
        idx += 1;
    }

    let sql = format!("UPDATE shot SET {} WHERE id = ?{idx}", sets.join(", "));
    params.push(Box::new(id.to_string()));
    let refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, refs.as_slice())?;
    get_by_id(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let n = conn.execute("DELETE FROM shot WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(CoreError::NotFound {
            entity: "shot",
            id: id.to_string(),
        });
    }
    Ok(())
}

pub fn reorder_within_episode(
    conn: &mut Connection,
    episode_id: &str,
    ordered_ids: &[String],
) -> Result<()> {
    let tx = conn.transaction()?;
    for (idx, id) in ordered_ids.iter().enumerate() {
        let n = tx.execute(
            "UPDATE shot SET order_index = ?1, updated_at = datetime('now') \
             WHERE id = ?2 AND episode_id = ?3",
            params![idx as i64, id, episode_id],
        )?;
        if n == 0 {
            return Err(CoreError::NotFound {
                entity: "shot",
                id: id.clone(),
            });
        }
    }
    tx.commit()?;
    Ok(())
}

fn collect_ids(conn: &Connection, sql: &str, shot_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![shot_id], |r| r.get::<_, String>(0))?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(CoreError::from)
}

pub fn list_links(conn: &Connection, shot_id: &str) -> Result<ShotLinks> {
    Ok(ShotLinks {
        character_ids: collect_ids(
            conn,
            "SELECT character_id FROM shot_character WHERE shot_id = ?1",
            shot_id,
        )?,
        scene_ids: collect_ids(
            conn,
            "SELECT scene_id FROM shot_scene WHERE shot_id = ?1",
            shot_id,
        )?,
        prop_ids: collect_ids(
            conn,
            "SELECT prop_id FROM shot_prop WHERE shot_id = ?1",
            shot_id,
        )?,
    })
}

pub fn link_subject(
    conn: &Connection,
    shot_id: &str,
    subject_id: &str,
    kind: SubjectKind,
) -> Result<()> {
    let sql = match kind {
        SubjectKind::Character => {
            "INSERT OR IGNORE INTO shot_character (shot_id, character_id) VALUES (?1, ?2)"
        }
        SubjectKind::Scene => {
            "INSERT OR IGNORE INTO shot_scene (shot_id, scene_id) VALUES (?1, ?2)"
        }
        SubjectKind::Prop => {
            "INSERT OR IGNORE INTO shot_prop (shot_id, prop_id) VALUES (?1, ?2)"
        }
    };
    conn.execute(sql, params![shot_id, subject_id])?;
    Ok(())
}

pub fn unlink_subject(
    conn: &Connection,
    shot_id: &str,
    subject_id: &str,
    kind: SubjectKind,
) -> Result<()> {
    let sql = match kind {
        SubjectKind::Character => {
            "DELETE FROM shot_character WHERE shot_id = ?1 AND character_id = ?2"
        }
        SubjectKind::Scene => "DELETE FROM shot_scene WHERE shot_id = ?1 AND scene_id = ?2",
        SubjectKind::Prop => "DELETE FROM shot_prop WHERE shot_id = ?1 AND prop_id = ?2",
    };
    conn.execute(sql, params![shot_id, subject_id])?;
    Ok(())
}

pub fn adopt_task_result(conn: &Connection, shot_id: &str, task_id: &str) -> Result<Shot> {
    let _ = get_by_id(conn, shot_id)?;

    let task = super::generation_task::get_by_id(conn, task_id)?;

    if task.status != crate::models::generation_task::GenerationTaskStatus::Success {
        return Err(CoreError::Validation(
            "only successful tasks can be adopted".into(),
        ));
    }
    if task.shot_id.as_deref() != Some(shot_id) {
        return Err(CoreError::Validation(
            "task does not belong to this shot".into(),
        ));
    }
    let asset_id = task.result_asset_id.ok_or_else(|| {
        CoreError::Validation("task has no result asset".into())
    })?;

    conn.execute(
        "UPDATE shot SET adopted_asset_id = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![asset_id, shot_id],
    )?;
    get_by_id(conn, shot_id)
}

pub fn unadopt(conn: &Connection, shot_id: &str) -> Result<Shot> {
    let _ = get_by_id(conn, shot_id)?;
    conn.execute(
        "UPDATE shot SET adopted_asset_id = NULL, updated_at = datetime('now') WHERE id = ?1",
        params![shot_id],
    )?;
    get_by_id(conn, shot_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use crate::db::queries::character as character_queries;
    use crate::db::queries::episode as episode_queries;
    use crate::db::queries::project as project_queries;
    use crate::models::character::CreateCharacterInput;
    use crate::models::episode::CreateEpisodeInput;
    use crate::models::project::CreateProjectInput;
    use std::path::Path;
    use tempfile::{tempdir, TempDir};

    fn setup_with_episode() -> (Connection, TempDir, String, String) {
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
                project_id: project.id.clone(),
                title: "Ep".into(),
                script_text: None,
            },
        )
        .unwrap();
        (conn, app_data, project.id, episode.id)
    }

    #[test]
    fn create_then_get_with_default_status_draft() {
        let (conn, _td, _pid, eid) = setup_with_episode();
        let s = create(
            &conn,
            CreateShotInput {
                episode_id: eid,
                summary: Some("opening".into()),
            },
        )
        .unwrap();
        let got = get_by_id(&conn, &s.id).unwrap();
        assert_eq!(got.summary, "opening");
        assert_eq!(got.status, ShotStatus::Draft);
        assert_eq!(got.order_index, 0);
        assert!(got.duration_sec.is_none());
    }

    #[test]
    fn create_auto_assigns_increasing_order_index() {
        let (conn, _td, _pid, eid) = setup_with_episode();
        let a = create(
            &conn,
            CreateShotInput {
                episode_id: eid.clone(),
                summary: None,
            },
        )
        .unwrap();
        let b = create(
            &conn,
            CreateShotInput {
                episode_id: eid,
                summary: None,
            },
        )
        .unwrap();
        assert_eq!(a.order_index, 0);
        assert_eq!(b.order_index, 1);
    }

    #[test]
    fn update_status_and_duration_roundtrip() {
        let (conn, _td, _pid, eid) = setup_with_episode();
        let s = create(
            &conn,
            CreateShotInput {
                episode_id: eid,
                summary: None,
            },
        )
        .unwrap();
        let u = update(
            &conn,
            &s.id,
            UpdateShotInput {
                status: Some(ShotStatus::Ready),
                duration_sec: Some(Some(3.5)),
                dialogue: Some("hi".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(u.status, ShotStatus::Ready);
        assert_eq!(u.duration_sec, Some(3.5));
        assert_eq!(u.dialogue, "hi");
    }

    #[test]
    fn update_clears_duration_via_some_none() {
        let (conn, _td, _pid, eid) = setup_with_episode();
        let s = create(
            &conn,
            CreateShotInput {
                episode_id: eid,
                summary: None,
            },
        )
        .unwrap();
        update(
            &conn,
            &s.id,
            UpdateShotInput {
                duration_sec: Some(Some(2.0)),
                ..Default::default()
            },
        )
        .unwrap();
        let cleared = update(
            &conn,
            &s.id,
            UpdateShotInput {
                duration_sec: Some(None),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(cleared.duration_sec.is_none());
    }

    #[test]
    fn delete_then_get_returns_not_found() {
        let (conn, _td, _pid, eid) = setup_with_episode();
        let s = create(
            &conn,
            CreateShotInput {
                episode_id: eid,
                summary: None,
            },
        )
        .unwrap();
        delete(&conn, &s.id).unwrap();
        assert!(matches!(
            get_by_id(&conn, &s.id),
            Err(CoreError::NotFound { .. })
        ));
    }

    #[test]
    fn reorder_within_episode_succeeds() {
        let (mut conn, _td, _pid, eid) = setup_with_episode();
        let a = create(
            &conn,
            CreateShotInput {
                episode_id: eid.clone(),
                summary: None,
            },
        )
        .unwrap();
        let b = create(
            &conn,
            CreateShotInput {
                episode_id: eid.clone(),
                summary: None,
            },
        )
        .unwrap();
        reorder_within_episode(&mut conn, &eid, &[b.id.clone(), a.id.clone()]).unwrap();
        let list = list(
            &conn,
            ListShotsOptions {
                episode_id: eid,
            },
        )
        .unwrap();
        assert_eq!(list[0].id, b.id);
        assert_eq!(list[1].id, a.id);
    }

    #[test]
    fn list_links_empty_initially() {
        let (conn, _td, _pid, eid) = setup_with_episode();
        let s = create(
            &conn,
            CreateShotInput {
                episode_id: eid,
                summary: None,
            },
        )
        .unwrap();
        let links = list_links(&conn, &s.id).unwrap();
        assert!(links.character_ids.is_empty());
        assert!(links.scene_ids.is_empty());
        assert!(links.prop_ids.is_empty());
    }

    #[test]
    fn link_character_is_idempotent() {
        let (conn, _td, pid, eid) = setup_with_episode();
        let s = create(
            &conn,
            CreateShotInput {
                episode_id: eid,
                summary: None,
            },
        )
        .unwrap();
        let c = character_queries::create(
            &conn,
            CreateCharacterInput {
                project_id: pid,
                name: "Mike".into(),
                description: None,
                appearance_prompt: None,
                reference_image_path: None,
            },
        )
        .unwrap();
        link_subject(&conn, &s.id, &c.id, SubjectKind::Character).unwrap();
        link_subject(&conn, &s.id, &c.id, SubjectKind::Character).unwrap();
        let links = list_links(&conn, &s.id).unwrap();
        assert_eq!(links.character_ids.len(), 1);
        assert_eq!(links.character_ids[0], c.id);
    }

    #[test]
    fn unlink_character_removes_row() {
        let (conn, _td, pid, eid) = setup_with_episode();
        let s = create(
            &conn,
            CreateShotInput {
                episode_id: eid,
                summary: None,
            },
        )
        .unwrap();
        let c = character_queries::create(
            &conn,
            CreateCharacterInput {
                project_id: pid,
                name: "Anna".into(),
                description: None,
                appearance_prompt: None,
                reference_image_path: None,
            },
        )
        .unwrap();
        link_subject(&conn, &s.id, &c.id, SubjectKind::Character).unwrap();
        unlink_subject(&conn, &s.id, &c.id, SubjectKind::Character).unwrap();
        assert!(list_links(&conn, &s.id).unwrap().character_ids.is_empty());
    }

    fn seed_provider_chain(conn: &Connection) {
        conn.execute_batch(
            "INSERT INTO provider (id, name) VALUES ('p1', 'P'); \
             INSERT INTO model (id, provider_id, name, model_type) \
                 VALUES ('m1', 'p1', 'M', 'image'); \
             INSERT INTO api_account (id, provider_id, label, api_key_ref, key_last4) \
                 VALUES ('a1', 'p1', 'L', 'ref', '0000');",
        )
        .unwrap();
    }

    fn make_success_task(conn: &Connection, project_id: &str, shot_id: &str) -> String {
        use crate::db::queries::generation_task as task_q;
        use crate::models::generation_task::{CreateGenerationTaskInput, GenerationTaskStatus, TaskKind};

        let task = task_q::create(
            conn,
            CreateGenerationTaskInput {
                project_id: Some(project_id.into()),
                shot_id: Some(shot_id.into()),
                provider_id: "p1".into(),
                model_id: "m1".into(),
                account_id: "a1".into(),
                task_type: TaskKind::Image,
                params_json: Some(r#"{"prompt":"hi"}"#.into()),
            },
        )
        .unwrap();

        task_q::transition_status(conn, &task.id, GenerationTaskStatus::Running, None).unwrap();
        task_q::transition_status(conn, &task.id, GenerationTaskStatus::Success, None).unwrap();

        let asset_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO asset (id, project_id, asset_type, original_name, file_path) \
             VALUES (?1, ?2, 'image', 'result.png', 'assets/result.png')",
            params![asset_id, project_id],
        )
        .unwrap();
        task_q::set_result_asset_id(conn, &task.id, &asset_id).unwrap();

        task.id
    }

    #[test]
    fn adopt_task_result_sets_adopted_asset_id() {
        let (conn, _td, pid, eid) = setup_with_episode();
        seed_provider_chain(&conn);
        let shot = create(&conn, CreateShotInput { episode_id: eid, summary: None }).unwrap();
        let task_id = make_success_task(&conn, &pid, &shot.id);

        let updated = adopt_task_result(&conn, &shot.id, &task_id).unwrap();
        assert!(updated.adopted_asset_id.is_some());

        let task = crate::db::queries::generation_task::get_by_id(&conn, &task_id).unwrap();
        assert_eq!(updated.adopted_asset_id.as_deref(), task.result_asset_id.as_deref());
    }

    #[test]
    fn adopt_rejects_non_success_task() {
        use crate::db::queries::generation_task as task_q;
        use crate::models::generation_task::{CreateGenerationTaskInput, TaskKind};

        let (conn, _td, pid, eid) = setup_with_episode();
        seed_provider_chain(&conn);
        let shot = create(&conn, CreateShotInput { episode_id: eid, summary: None }).unwrap();
        let task = task_q::create(
            &conn,
            CreateGenerationTaskInput {
                project_id: Some(pid),
                shot_id: Some(shot.id.clone()),
                provider_id: "p1".into(),
                model_id: "m1".into(),
                account_id: "a1".into(),
                task_type: TaskKind::Image,
                params_json: None,
            },
        )
        .unwrap();

        let err = adopt_task_result(&conn, &shot.id, &task.id).unwrap_err();
        assert!(matches!(err, CoreError::Validation(_)));
    }

    #[test]
    fn adopt_rejects_mismatched_shot_id() {
        let (conn, _td, pid, eid) = setup_with_episode();
        seed_provider_chain(&conn);
        let shot_a = create(&conn, CreateShotInput { episode_id: eid.clone(), summary: None }).unwrap();
        let shot_b = create(&conn, CreateShotInput { episode_id: eid, summary: None }).unwrap();
        let task_id = make_success_task(&conn, &pid, &shot_a.id);

        let err = adopt_task_result(&conn, &shot_b.id, &task_id).unwrap_err();
        assert!(matches!(err, CoreError::Validation(_)));
    }

    #[test]
    fn adopt_switch_overwrites_previous() {
        let (conn, _td, pid, eid) = setup_with_episode();
        seed_provider_chain(&conn);
        let shot = create(&conn, CreateShotInput { episode_id: eid, summary: None }).unwrap();
        let task_a = make_success_task(&conn, &pid, &shot.id);
        let task_b = make_success_task(&conn, &pid, &shot.id);

        adopt_task_result(&conn, &shot.id, &task_a).unwrap();
        let updated = adopt_task_result(&conn, &shot.id, &task_b).unwrap();

        let task_b_row = crate::db::queries::generation_task::get_by_id(&conn, &task_b).unwrap();
        assert_eq!(updated.adopted_asset_id.as_deref(), task_b_row.result_asset_id.as_deref());
    }

    #[test]
    fn unadopt_clears_adopted_asset_id() {
        let (conn, _td, pid, eid) = setup_with_episode();
        seed_provider_chain(&conn);
        let shot = create(&conn, CreateShotInput { episode_id: eid, summary: None }).unwrap();
        let task_id = make_success_task(&conn, &pid, &shot.id);

        adopt_task_result(&conn, &shot.id, &task_id).unwrap();
        let cleared = unadopt(&conn, &shot.id).unwrap();
        assert!(cleared.adopted_asset_id.is_none());
    }
}
