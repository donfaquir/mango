use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::timeline::{CreateTimelineKeyframeInput, TimelineKeyframe};

const SELECT_COLUMNS: &str = "id, item_id, property, time_ms, value, easing, created_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<TimelineKeyframe> {
    Ok(TimelineKeyframe {
        id: row.get(0)?,
        item_id: row.get(1)?,
        property: row.get(2)?,
        time_ms: row.get(3)?,
        value: row.get(4)?,
        easing: row.get(5)?,
        created_at: row.get(6)?,
    })
}

pub fn list_by_item(conn: &Connection, item_id: &str) -> Result<Vec<TimelineKeyframe>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM timeline_keyframe WHERE item_id = ?1 ORDER BY property ASC, time_ms ASC"
    ))?;
    let rows = stmt.query_map(params![item_id], map_row)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn create(conn: &Connection, input: CreateTimelineKeyframeInput) -> Result<TimelineKeyframe> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO timeline_keyframe (id, item_id, property, time_ms, value, easing) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, input.item_id, input.property, input.time_ms, input.value, input.easing],
    )?;
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM timeline_keyframe WHERE id = ?1"),
        params![id],
        map_row,
    )
    .map_err(CoreError::Sqlite)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let affected = conn.execute("DELETE FROM timeline_keyframe WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(CoreError::NotFound {
            entity: "timeline_keyframe",
            id: id.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use crate::db::queries::{timeline_item, timeline_track};
    use crate::models::timeline::*;
    use std::path::Path;

    fn setup() -> (Connection, String) {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        conn.execute("INSERT INTO project (id, name) VALUES ('p1', 'P')", []).unwrap();
        conn.execute(
            "INSERT INTO episode (id, project_id, title, order_index) VALUES ('e1', 'p1', 'E1', 0)",
            [],
        ).unwrap();
        let track = timeline_track::create(&conn, CreateTimelineTrackInput {
            episode_id: "e1".into(),
            track_type: TrackType::Video,
            label: "V".into(),
        }).unwrap();
        let item = timeline_item::create(&conn, CreateTimelineItemInput {
            track_id: track.id,
            asset_id: None,
            item_type: ItemType::Clip,
            position_ms: 0,
            duration_ms: 5000,
            in_point_ms: None,
            out_point_ms: 5000,
            params_json: None,
        }).unwrap();
        (conn, item.id)
    }

    #[test]
    fn create_and_list() {
        let (conn, item_id) = setup();
        create(&conn, CreateTimelineKeyframeInput {
            item_id: item_id.clone(),
            property: "scale".into(),
            time_ms: 0,
            value: 1.0,
            easing: "linear".into(),
        }).unwrap();
        create(&conn, CreateTimelineKeyframeInput {
            item_id: item_id.clone(),
            property: "scale".into(),
            time_ms: 4000,
            value: 1.15,
            easing: "ease_out".into(),
        }).unwrap();
        let list = list_by_item(&conn, &item_id).unwrap();
        assert_eq!(list.len(), 2);
        assert!(list[0].time_ms < list[1].time_ms);
    }

    #[test]
    fn delete_keyframe() {
        let (conn, item_id) = setup();
        let kf = create(&conn, CreateTimelineKeyframeInput {
            item_id,
            property: "opacity".into(),
            time_ms: 0,
            value: 0.5,
            easing: "linear".into(),
        }).unwrap();
        delete(&conn, &kf.id).unwrap();
        assert_eq!(list_by_item(&conn, &kf.item_id).unwrap().len(), 0);
    }
}
