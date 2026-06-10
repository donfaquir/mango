use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::shot_audio::{
    AudioRole, CreateShotAudioInput, ShotAudio, UpdateShotAudioInput,
};

const SELECT_COLUMNS: &str =
    "id, shot_id, asset_id, audio_role, volume, offset_ms, order_index, created_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<ShotAudio> {
    let role_str: String = row.get(3)?;
    Ok(ShotAudio {
        id: row.get(0)?,
        shot_id: row.get(1)?,
        asset_id: row.get(2)?,
        audio_role: AudioRole::parse(&role_str).unwrap_or(AudioRole::Sfx),
        volume: row.get(4)?,
        offset_ms: row.get(5)?,
        order_index: row.get(6)?,
        created_at: row.get(7)?,
    })
}

pub fn list_by_shot(conn: &Connection, shot_id: &str) -> Result<Vec<ShotAudio>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM shot_audio WHERE shot_id = ?1 \
         ORDER BY audio_role ASC, order_index ASC"
    ))?;
    let rows = stmt.query_map(params![shot_id], map_row)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<ShotAudio> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM shot_audio WHERE id = ?1"),
        params![id],
        map_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
            entity: "shot_audio",
            id: id.to_string(),
        },
        other => CoreError::Sqlite(other),
    })
}

pub fn create(conn: &Connection, input: CreateShotAudioInput) -> Result<ShotAudio> {
    if input.volume < 0.0 || input.volume > 2.0 {
        return Err(CoreError::Validation("volume must be between 0.0 and 2.0".into()));
    }

    if input.audio_role.is_unique() {
        let existing: i64 = conn.query_row(
            "SELECT COUNT(*) FROM shot_audio WHERE shot_id = ?1 AND audio_role = ?2",
            params![input.shot_id, input.audio_role.as_str()],
            |r| r.get(0),
        )?;
        if existing > 0 {
            let role_label = match input.audio_role {
                AudioRole::Voice => "voice",
                AudioRole::Bgm => "bgm",
                _ => unreachable!(),
            };
            return Err(CoreError::Validation(format!(
                "shot already has a {role_label} binding"
            )));
        }
    }

    let id = Uuid::new_v4().to_string();

    let next_order: i32 = conn.query_row(
        "SELECT COALESCE(MAX(order_index), -1) + 1 FROM shot_audio \
         WHERE shot_id = ?1 AND audio_role = ?2",
        params![input.shot_id, input.audio_role.as_str()],
        |r| r.get(0),
    )?;

    conn.execute(
        "INSERT INTO shot_audio (id, shot_id, asset_id, audio_role, volume, offset_ms, order_index) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            input.shot_id,
            input.asset_id,
            input.audio_role.as_str(),
            input.volume,
            input.offset_ms,
            next_order,
        ],
    )?;

    get_by_id(conn, &id)
}

pub fn update(conn: &Connection, id: &str, input: UpdateShotAudioInput) -> Result<ShotAudio> {
    let existing = get_by_id(conn, id)?;

    let new_volume = input.volume.unwrap_or(existing.volume);
    if !(0.0..=2.0).contains(&new_volume) {
        return Err(CoreError::Validation("volume must be between 0.0 and 2.0".into()));
    }
    let new_offset = input.offset_ms.unwrap_or(existing.offset_ms);

    conn.execute(
        "UPDATE shot_audio SET volume = ?1, offset_ms = ?2 WHERE id = ?3",
        params![new_volume, new_offset, id],
    )?;

    get_by_id(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let affected = conn.execute("DELETE FROM shot_audio WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(CoreError::NotFound {
            entity: "shot_audio",
            id: id.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use crate::db::queries::{
        episode as episode_queries, project as project_queries, shot as shot_queries,
    };
    use crate::models::episode::CreateEpisodeInput;
    use crate::models::project::CreateProjectInput;
    use crate::models::shot::CreateShotInput;
    use std::path::Path;
    use tempfile::tempdir;

    fn setup() -> (Connection, String, String) {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let td = tempdir().unwrap();
        let project = project_queries::create(
            &conn,
            td.path(),
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
                title: "E1".into(),
                script_text: None,
            },
        )
        .unwrap();
        let shot = shot_queries::create(
            &conn,
            CreateShotInput {
                episode_id: episode.id.clone(),
                summary: Some("test shot".into()),
            },
        )
        .unwrap();
        std::mem::forget(td);
        (conn, shot.id, project.id)
    }

    fn insert_audio_asset(conn: &Connection, project_id: &str) -> String {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO asset (id, project_id, asset_type, original_name, file_path, file_size) \
             VALUES (?1, ?2, 'audio', 'test.mp3', 'assets/test.mp3', 1024)",
            params![id, project_id],
        )
        .unwrap();
        id
    }

    #[test]
    fn create_and_list() {
        let (conn, shot_id, pid) = setup();
        let asset_id = insert_audio_asset(&conn, &pid);

        let sa = create(
            &conn,
            CreateShotAudioInput {
                shot_id: shot_id.clone(),
                asset_id,
                audio_role: AudioRole::Sfx,
                volume: 0.8,
                offset_ms: 100,
            },
        )
        .unwrap();
        assert_eq!(sa.audio_role, AudioRole::Sfx);
        assert!((sa.volume - 0.8).abs() < f64::EPSILON);
        assert_eq!(sa.offset_ms, 100);
        assert_eq!(sa.order_index, 0);

        let list = list_by_shot(&conn, &shot_id).unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn voice_uniqueness() {
        let (conn, shot_id, pid) = setup();
        let a1 = insert_audio_asset(&conn, &pid);
        let a2 = insert_audio_asset(&conn, &pid);

        create(
            &conn,
            CreateShotAudioInput {
                shot_id: shot_id.clone(),
                asset_id: a1,
                audio_role: AudioRole::Voice,
                volume: 1.0,
                offset_ms: 0,
            },
        )
        .unwrap();

        let r = create(
            &conn,
            CreateShotAudioInput {
                shot_id: shot_id.clone(),
                asset_id: a2,
                audio_role: AudioRole::Voice,
                volume: 1.0,
                offset_ms: 0,
            },
        );
        assert!(matches!(r, Err(CoreError::Validation(_))));
    }

    #[test]
    fn sfx_allows_multiple() {
        let (conn, shot_id, pid) = setup();
        let a1 = insert_audio_asset(&conn, &pid);
        let a2 = insert_audio_asset(&conn, &pid);

        create(
            &conn,
            CreateShotAudioInput {
                shot_id: shot_id.clone(),
                asset_id: a1,
                audio_role: AudioRole::Sfx,
                volume: 1.0,
                offset_ms: 0,
            },
        )
        .unwrap();
        create(
            &conn,
            CreateShotAudioInput {
                shot_id: shot_id.clone(),
                asset_id: a2,
                audio_role: AudioRole::Sfx,
                volume: 0.5,
                offset_ms: 200,
            },
        )
        .unwrap();

        let list = list_by_shot(&conn, &shot_id).unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn invalid_volume_rejected() {
        let (conn, shot_id, pid) = setup();
        let aid = insert_audio_asset(&conn, &pid);

        let r = create(
            &conn,
            CreateShotAudioInput {
                shot_id,
                asset_id: aid,
                audio_role: AudioRole::Sfx,
                volume: 3.0,
                offset_ms: 0,
            },
        );
        assert!(matches!(r, Err(CoreError::Validation(_))));
    }

    #[test]
    fn update_volume_and_offset() {
        let (conn, shot_id, pid) = setup();
        let aid = insert_audio_asset(&conn, &pid);

        let sa = create(
            &conn,
            CreateShotAudioInput {
                shot_id,
                asset_id: aid,
                audio_role: AudioRole::Sfx,
                volume: 1.0,
                offset_ms: 0,
            },
        )
        .unwrap();

        let updated = update(
            &conn,
            &sa.id,
            UpdateShotAudioInput {
                volume: Some(0.5),
                offset_ms: Some(300),
            },
        )
        .unwrap();
        assert!((updated.volume - 0.5).abs() < f64::EPSILON);
        assert_eq!(updated.offset_ms, 300);
    }

    #[test]
    fn delete_removes_binding() {
        let (conn, shot_id, pid) = setup();
        let aid = insert_audio_asset(&conn, &pid);

        let sa = create(
            &conn,
            CreateShotAudioInput {
                shot_id: shot_id.clone(),
                asset_id: aid,
                audio_role: AudioRole::Sfx,
                volume: 1.0,
                offset_ms: 0,
            },
        )
        .unwrap();

        delete(&conn, &sa.id).unwrap();
        let list = list_by_shot(&conn, &shot_id).unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn shot_cascade_deletes_audio() {
        let (conn, shot_id, pid) = setup();
        let aid = insert_audio_asset(&conn, &pid);

        create(
            &conn,
            CreateShotAudioInput {
                shot_id: shot_id.clone(),
                asset_id: aid,
                audio_role: AudioRole::Sfx,
                volume: 1.0,
                offset_ms: 0,
            },
        )
        .unwrap();

        shot_queries::delete(&conn, &shot_id).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM shot_audio", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
