use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::timeline::{CreateTimelineTrackInput, TimelineTrack, TrackType};

const SELECT_COLUMNS: &str =
    "id, episode_id, track_type, label, order_index, muted, locked, created_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<TimelineTrack> {
    let tt_str: String = row.get(2)?;
    let muted_int: i32 = row.get(4 + 1)?;
    let locked_int: i32 = row.get(5 + 1)?;
    Ok(TimelineTrack {
        id: row.get(0)?,
        episode_id: row.get(1)?,
        track_type: TrackType::parse(&tt_str).unwrap_or(TrackType::Video),
        label: row.get(3)?,
        order_index: row.get(4)?,
        muted: muted_int != 0,
        locked: locked_int != 0,
        created_at: row.get(7)?,
    })
}

pub fn list_by_episode(conn: &Connection, episode_id: &str) -> Result<Vec<TimelineTrack>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM timeline_track WHERE episode_id = ?1 ORDER BY order_index ASC"
    ))?;
    let rows = stmt.query_map(params![episode_id], map_row)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<TimelineTrack> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM timeline_track WHERE id = ?1"),
        params![id],
        map_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
            entity: "timeline_track",
            id: id.to_string(),
        },
        other => CoreError::Sqlite(other),
    })
}

pub fn create(conn: &Connection, input: CreateTimelineTrackInput) -> Result<TimelineTrack> {
    let id = Uuid::new_v4().to_string();
    let next_order: i32 = conn.query_row(
        "SELECT COALESCE(MAX(order_index), -1) + 1 FROM timeline_track WHERE episode_id = ?1",
        params![input.episode_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO timeline_track (id, episode_id, track_type, label, order_index) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, input.episode_id, input.track_type.as_str(), input.label, next_order],
    )?;
    get_by_id(conn, &id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let affected = conn.execute("DELETE FROM timeline_track WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(CoreError::NotFound {
            entity: "timeline_track",
            id: id.to_string(),
        });
    }
    Ok(())
}

pub fn reorder(conn: &Connection, ids: &[String]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let first = get_by_id(conn, &ids[0])?;
    let tx = conn.unchecked_transaction()?;
    for (i, id) in ids.iter().enumerate() {
        let affected = tx.execute(
            "UPDATE timeline_track SET order_index = ?1 WHERE id = ?2 AND episode_id = ?3",
            params![i as i32, id, first.episode_id],
        )?;
        if affected == 0 {
            return Err(CoreError::Validation(format!(
                "track '{id}' not found or belongs to a different episode"
            )));
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn update_muted(conn: &Connection, id: &str, muted: bool) -> Result<()> {
    let affected = conn.execute(
        "UPDATE timeline_track SET muted = ?1 WHERE id = ?2",
        params![muted as i32, id],
    )?;
    if affected == 0 {
        return Err(CoreError::NotFound { entity: "timeline_track", id: id.to_string() });
    }
    Ok(())
}

pub fn update_locked(conn: &Connection, id: &str, locked: bool) -> Result<()> {
    let affected = conn.execute(
        "UPDATE timeline_track SET locked = ?1 WHERE id = ?2",
        params![locked as i32, id],
    )?;
    if affected == 0 {
        return Err(CoreError::NotFound { entity: "timeline_track", id: id.to_string() });
    }
    Ok(())
}

pub fn create_defaults(conn: &Connection, episode_id: &str) -> Result<Vec<TimelineTrack>> {
    let defaults = [
        (TrackType::Video, "视频"),
        (TrackType::Overlay, "贴片/特效"),
        (TrackType::Text, "文字"),
        (TrackType::Audio, "配音"),
        (TrackType::Audio, "BGM"),
        (TrackType::Audio, "音效"),
    ];
    let mut tracks = Vec::with_capacity(defaults.len());
    for (tt, label) in defaults {
        let t = create(conn, CreateTimelineTrackInput {
            episode_id: episode_id.to_string(),
            track_type: tt,
            label: label.to_string(),
        })?;
        tracks.push(t);
    }
    Ok(tracks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use std::path::Path;

    fn setup() -> Connection {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        conn.execute("INSERT INTO project (id, name) VALUES ('p1', 'P')", []).unwrap();
        conn.execute(
            "INSERT INTO episode (id, project_id, title, order_index) VALUES ('e1', 'p1', 'E1', 0)",
            [],
        ).unwrap();
        conn
    }

    #[test]
    fn create_and_list() {
        let conn = setup();
        let t = create(&conn, CreateTimelineTrackInput {
            episode_id: "e1".into(),
            track_type: TrackType::Video,
            label: "Video".into(),
        }).unwrap();
        assert_eq!(t.order_index, 0);
        assert!(!t.muted);
        assert!(!t.locked);
        let list = list_by_episode(&conn, "e1").unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn create_defaults_six_tracks() {
        let conn = setup();
        let tracks = create_defaults(&conn, "e1").unwrap();
        assert_eq!(tracks.len(), 6);
        assert_eq!(tracks[0].track_type, TrackType::Video);
        assert_eq!(tracks[3].label, "配音");
        assert_eq!(tracks[5].order_index, 5);
    }

    #[test]
    fn reorder_tracks() {
        let conn = setup();
        let tracks = create_defaults(&conn, "e1").unwrap();
        let reversed: Vec<String> = tracks.iter().rev().map(|t| t.id.clone()).collect();
        reorder(&conn, &reversed).unwrap();
        let list = list_by_episode(&conn, "e1").unwrap();
        assert_eq!(list[0].id, tracks[5].id);
    }

    #[test]
    fn delete_track_cascades_items() {
        let conn = setup();
        let t = create(&conn, CreateTimelineTrackInput {
            episode_id: "e1".into(),
            track_type: TrackType::Video,
            label: "V".into(),
        }).unwrap();
        conn.execute(
            "INSERT INTO timeline_item (id, track_id, item_type, position_ms, duration_ms, out_point_ms) \
             VALUES ('i1', ?1, 'clip', 0, 5000, 5000)",
            params![t.id],
        ).unwrap();
        delete(&conn, &t.id).unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM timeline_item", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn mute_and_lock() {
        let conn = setup();
        let t = create(&conn, CreateTimelineTrackInput {
            episode_id: "e1".into(),
            track_type: TrackType::Audio,
            label: "A".into(),
        }).unwrap();
        update_muted(&conn, &t.id, true).unwrap();
        update_locked(&conn, &t.id, true).unwrap();
        let fetched = get_by_id(&conn, &t.id).unwrap();
        assert!(fetched.muted);
        assert!(fetched.locked);
    }
}
