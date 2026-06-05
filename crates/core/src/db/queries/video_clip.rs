use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::video_clip::{CreateVideoClipInput, UpdateVideoClipInput, VideoClip};

const SELECT_COLUMNS: &str =
    "id, project_id, episode_id, source_asset_id, label, trim_start_ms, trim_end_ms, order_index, created_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<VideoClip> {
    Ok(VideoClip {
        id: row.get(0)?,
        project_id: row.get(1)?,
        episode_id: row.get(2)?,
        source_asset_id: row.get(3)?,
        label: row.get(4)?,
        trim_start_ms: row.get(5)?,
        trim_end_ms: row.get(6)?,
        order_index: row.get(7)?,
        created_at: row.get(8)?,
    })
}

pub fn list(conn: &Connection, episode_id: &str) -> Result<Vec<VideoClip>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM video_clip WHERE episode_id = ?1 ORDER BY order_index ASC"
    ))?;
    let rows = stmt.query_map(params![episode_id], map_row)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<VideoClip> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM video_clip WHERE id = ?1"),
        params![id],
        map_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
            entity: "video_clip",
            id: id.to_string(),
        },
        other => CoreError::Sqlite(other),
    })
}

pub fn create(conn: &Connection, input: CreateVideoClipInput) -> Result<VideoClip> {
    let id = Uuid::new_v4().to_string();

    let next_order: i32 = conn.query_row(
        "SELECT COALESCE(MAX(order_index), -1) + 1 FROM video_clip WHERE episode_id = ?1",
        params![input.episode_id],
        |r| r.get(0),
    )?;

    conn.execute(
        "INSERT INTO video_clip (id, project_id, episode_id, source_asset_id, label, trim_start_ms, trim_end_ms, order_index)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            input.project_id,
            input.episode_id,
            input.source_asset_id,
            input.label,
            input.trim_start_ms,
            input.trim_end_ms,
            next_order,
        ],
    )?;

    get_by_id(conn, &id)
}

pub fn update(conn: &Connection, id: &str, input: UpdateVideoClipInput) -> Result<VideoClip> {
    let existing = get_by_id(conn, id)?;

    let label = input.label.or(existing.label);
    let trim_start = input.trim_start_ms.or(existing.trim_start_ms);
    let trim_end = input.trim_end_ms.or(existing.trim_end_ms);

    conn.execute(
        "UPDATE video_clip SET label = ?1, trim_start_ms = ?2, trim_end_ms = ?3 WHERE id = ?4",
        params![label, trim_start, trim_end, id],
    )?;

    get_by_id(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let affected = conn.execute("DELETE FROM video_clip WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(CoreError::NotFound {
            entity: "video_clip",
            id: id.to_string(),
        });
    }
    Ok(())
}

pub fn reorder(conn: &Connection, ids: &[String]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for (i, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE video_clip SET order_index = ?1 WHERE id = ?2",
            params![i as i32, id],
        )?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use std::path::Path;

    fn setup() -> Connection {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        conn.execute(
            "INSERT INTO project (id, name) VALUES ('p1', 'Test Project')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO episode (id, project_id, title, order_index) VALUES ('e1', 'p1', 'Episode 1', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO asset (id, project_id, asset_type, file_path, file_size, original_name)
             VALUES ('a1', 'p1', 'video', 'assets/a1.mp4', 1000, 'a1.mp4')",
            [],
        )
        .unwrap();
        conn
    }

    fn make_input() -> CreateVideoClipInput {
        CreateVideoClipInput {
            project_id: "p1".into(),
            episode_id: Some("e1".into()),
            source_asset_id: "a1".into(),
            label: Some("Clip 1".into()),
            trim_start_ms: Some(1000),
            trim_end_ms: Some(5000),
        }
    }

    #[test]
    fn create_and_get() {
        let conn = setup();
        let clip = create(&conn, make_input()).unwrap();
        assert_eq!(clip.order_index, 0);
        assert_eq!(clip.label, Some("Clip 1".into()));

        let fetched = get_by_id(&conn, &clip.id).unwrap();
        assert_eq!(fetched.id, clip.id);
    }

    #[test]
    fn auto_order_index() {
        let conn = setup();
        let c1 = create(&conn, make_input()).unwrap();
        let c2 = create(&conn, make_input()).unwrap();
        let c3 = create(&conn, make_input()).unwrap();
        assert_eq!(c1.order_index, 0);
        assert_eq!(c2.order_index, 1);
        assert_eq!(c3.order_index, 2);
    }

    #[test]
    fn list_ordered() {
        let conn = setup();
        create(&conn, make_input()).unwrap();
        create(&conn, make_input()).unwrap();
        let clips = list(&conn, "e1").unwrap();
        assert_eq!(clips.len(), 2);
        assert!(clips[0].order_index < clips[1].order_index);
    }

    #[test]
    fn delete_clip() {
        let conn = setup();
        let clip = create(&conn, make_input()).unwrap();
        delete(&conn, &clip.id).unwrap();
        assert!(get_by_id(&conn, &clip.id).is_err());
    }

    #[test]
    fn reorder_clips() {
        let conn = setup();
        let a = create(&conn, make_input()).unwrap();
        let b = create(&conn, make_input()).unwrap();
        let c = create(&conn, make_input()).unwrap();

        reorder(&conn, &[c.id.clone(), a.id.clone(), b.id.clone()]).unwrap();

        let clips = list(&conn, "e1").unwrap();
        assert_eq!(clips[0].id, c.id);
        assert_eq!(clips[1].id, a.id);
        assert_eq!(clips[2].id, b.id);
    }

    #[test]
    fn update_clip() {
        let conn = setup();
        let clip = create(&conn, make_input()).unwrap();
        let updated = update(
            &conn,
            &clip.id,
            UpdateVideoClipInput {
                label: Some("New Label".into()),
                trim_start_ms: Some(2000),
                trim_end_ms: None,
            },
        )
        .unwrap();
        assert_eq!(updated.label, Some("New Label".into()));
        assert_eq!(updated.trim_start_ms, Some(2000));
        assert_eq!(updated.trim_end_ms, Some(5000));
    }
}
