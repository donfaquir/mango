use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::timeline::{
    CreateTimelineItemInput, ItemType, MoveTimelineItemInput, TimelineItem, UpdateTimelineItemInput,
};

const SELECT_COLUMNS: &str =
    "id, track_id, asset_id, item_type, position_ms, duration_ms, in_point_ms, out_point_ms, params_json, order_index, created_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<TimelineItem> {
    let it_str: String = row.get(3)?;
    Ok(TimelineItem {
        id: row.get(0)?,
        track_id: row.get(1)?,
        asset_id: row.get(2)?,
        item_type: ItemType::parse(&it_str).unwrap_or(ItemType::Clip),
        position_ms: row.get(4)?,
        duration_ms: row.get(5)?,
        in_point_ms: row.get(6)?,
        out_point_ms: row.get(7)?,
        params_json: row.get(8)?,
        order_index: row.get(9)?,
        created_at: row.get(10)?,
    })
}

pub fn list_by_track(conn: &Connection, track_id: &str) -> Result<Vec<TimelineItem>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM timeline_item WHERE track_id = ?1 ORDER BY position_ms ASC"
    ))?;
    let rows = stmt.query_map(params![track_id], map_row)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn list_by_episode(conn: &Connection, episode_id: &str) -> Result<Vec<TimelineItem>> {
    let mut stmt = conn.prepare(
        "SELECT i.id, i.track_id, i.asset_id, i.item_type, i.position_ms, i.duration_ms, \
         i.in_point_ms, i.out_point_ms, i.params_json, i.order_index, i.created_at \
         FROM timeline_item i \
         JOIN timeline_track t ON i.track_id = t.id \
         WHERE t.episode_id = ?1 \
         ORDER BY t.order_index ASC, i.position_ms ASC",
    )?;
    let rows = stmt.query_map(params![episode_id], map_row)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<TimelineItem> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM timeline_item WHERE id = ?1"),
        params![id],
        map_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
            entity: "timeline_item",
            id: id.to_string(),
        },
        other => CoreError::Sqlite(other),
    })
}

pub fn create(conn: &Connection, input: CreateTimelineItemInput) -> Result<TimelineItem> {
    let id = Uuid::new_v4().to_string();
    let next_order: i32 = conn.query_row(
        "SELECT COALESCE(MAX(order_index), -1) + 1 FROM timeline_item WHERE track_id = ?1",
        params![input.track_id],
        |r| r.get(0),
    )?;
    let in_pt = input.in_point_ms.unwrap_or(0);
    let params_json = input.params_json.as_deref().unwrap_or("{}");
    conn.execute(
        "INSERT INTO timeline_item (id, track_id, asset_id, item_type, position_ms, duration_ms, in_point_ms, out_point_ms, params_json, order_index) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            input.track_id,
            input.asset_id,
            input.item_type.as_str(),
            input.position_ms,
            input.duration_ms,
            in_pt,
            input.out_point_ms,
            params_json,
            next_order,
        ],
    )?;
    get_by_id(conn, &id)
}

pub fn update(
    conn: &Connection,
    id: &str,
    input: UpdateTimelineItemInput,
) -> Result<TimelineItem> {
    let existing = get_by_id(conn, id)?;
    let pos = input.position_ms.unwrap_or(existing.position_ms);
    let dur = input.duration_ms.unwrap_or(existing.duration_ms);
    let in_pt = input.in_point_ms.unwrap_or(existing.in_point_ms);
    let out_pt = input.out_point_ms.unwrap_or(existing.out_point_ms);
    let params = input.params_json.as_deref().unwrap_or(&existing.params_json);
    conn.execute(
        "UPDATE timeline_item SET position_ms=?1, duration_ms=?2, in_point_ms=?3, out_point_ms=?4, params_json=?5 WHERE id=?6",
        params![pos, dur, in_pt, out_pt, params, id],
    )?;
    get_by_id(conn, id)
}

pub fn move_item(
    conn: &Connection,
    id: &str,
    input: MoveTimelineItemInput,
) -> Result<TimelineItem> {
    get_by_id(conn, id)?;
    conn.execute(
        "UPDATE timeline_item SET track_id=?1, position_ms=?2 WHERE id=?3",
        params![input.track_id, input.position_ms, id],
    )?;
    get_by_id(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let affected = conn.execute("DELETE FROM timeline_item WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(CoreError::NotFound {
            entity: "timeline_item",
            id: id.to_string(),
        });
    }
    Ok(())
}

pub fn batch_create(
    conn: &Connection,
    inputs: Vec<CreateTimelineItemInput>,
) -> Result<Vec<TimelineItem>> {
    let tx = conn.unchecked_transaction()?;
    let mut items = Vec::with_capacity(inputs.len());
    for input in inputs {
        let item = create(&tx, input)?;
        items.push(item);
    }
    tx.commit()?;
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use crate::db::queries::timeline_track;
    use crate::models::timeline::{CreateTimelineTrackInput, TrackType};
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
            label: "Video".into(),
        }).unwrap();
        (conn, track.id)
    }

    fn make_input(track_id: &str, pos: i64, dur: i64) -> CreateTimelineItemInput {
        CreateTimelineItemInput {
            track_id: track_id.into(),
            asset_id: None,
            item_type: ItemType::Clip,
            position_ms: pos,
            duration_ms: dur,
            in_point_ms: None,
            out_point_ms: dur,
            params_json: None,
        }
    }

    #[test]
    fn create_and_list() {
        let (conn, tid) = setup();
        let item = create(&conn, make_input(&tid, 0, 5000)).unwrap();
        assert_eq!(item.position_ms, 0);
        assert_eq!(item.in_point_ms, 0);
        assert_eq!(item.order_index, 0);
        let list = list_by_track(&conn, &tid).unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn update_position_and_params() {
        let (conn, tid) = setup();
        let item = create(&conn, make_input(&tid, 0, 5000)).unwrap();
        let updated = update(&conn, &item.id, UpdateTimelineItemInput {
            position_ms: Some(1000),
            duration_ms: None,
            in_point_ms: Some(500),
            out_point_ms: Some(4500),
            params_json: Some(r#"{"type":"Clip","ken_burns_preset":"slow_zoom"}"#.into()),
        }).unwrap();
        assert_eq!(updated.position_ms, 1000);
        assert_eq!(updated.in_point_ms, 500);
        assert!(updated.params_json.contains("slow_zoom"));
    }

    #[test]
    fn move_item_cross_track() {
        let (conn, tid1) = setup();
        let tid2 = timeline_track::create(&conn, CreateTimelineTrackInput {
            episode_id: "e1".into(),
            track_type: TrackType::Audio,
            label: "Audio".into(),
        }).unwrap().id;
        let item = create(&conn, make_input(&tid1, 0, 3000)).unwrap();
        let moved = move_item(&conn, &item.id, MoveTimelineItemInput {
            track_id: tid2.clone(),
            position_ms: 2000,
        }).unwrap();
        assert_eq!(moved.track_id, tid2);
        assert_eq!(moved.position_ms, 2000);
    }

    #[test]
    fn delete_item() {
        let (conn, tid) = setup();
        let item = create(&conn, make_input(&tid, 0, 5000)).unwrap();
        delete(&conn, &item.id).unwrap();
        assert!(get_by_id(&conn, &item.id).is_err());
    }

    #[test]
    fn batch_create_items() {
        let (conn, tid) = setup();
        let inputs = vec![
            make_input(&tid, 0, 3000),
            make_input(&tid, 3000, 2000),
            make_input(&tid, 5000, 4000),
        ];
        let items = batch_create(&conn, inputs).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].order_index, 0);
        assert_eq!(items[2].order_index, 2);
    }

    #[test]
    fn list_by_episode_joins() {
        let (conn, tid) = setup();
        create(&conn, make_input(&tid, 0, 5000)).unwrap();
        create(&conn, make_input(&tid, 5000, 3000)).unwrap();
        let all = list_by_episode(&conn, "e1").unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn delete_item_cascades_keyframes() {
        let (conn, tid) = setup();
        let item = create(&conn, make_input(&tid, 0, 5000)).unwrap();
        conn.execute(
            "INSERT INTO timeline_keyframe (id, item_id, property, time_ms, value) VALUES ('k1', ?1, 'scale', 0, 1.0)",
            params![item.id],
        ).unwrap();
        delete(&conn, &item.id).unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM timeline_keyframe", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0);
    }
}
