use rusqlite::{params, Connection};
use std::collections::HashMap;

use crate::db::queries::canvas_layout as canvas_queries;
use crate::db::queries::episode as episode_queries;
use crate::db::queries::episode_checkpoint;
use crate::error::{CoreError, Result};
use crate::models::canvas_layout::UpsertCanvasLayoutInput;
use crate::models::episode::Episode;
use crate::models::episode_checkpoint::CreateCheckpointInput;

pub fn compute_change_summary(
    conn: &Connection,
    episode_id: &str,
    current_script: &str,
    current_shots_json: &str,
    current_nodes_json: &str,
) -> Option<String> {
    let prev = conn
        .query_row(
            "SELECT script_text, shots_json, canvas_nodes_json \
             FROM episode_checkpoint WHERE episode_id = ?1 \
             ORDER BY created_at DESC LIMIT 1",
            params![episode_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .ok();

    let (prev_script, prev_shots, prev_nodes) = match prev {
        Some((s, sh, n)) => (s, sh, n),
        None => (String::new(), "[]".into(), "[]".into()),
    };

    let mut parts: Vec<String> = Vec::new();

    if let Some(shot_desc) = diff_shots(&prev_shots, current_shots_json) {
        parts.push(shot_desc);
    }

    let prev_chars = prev_script.chars().count() as i64;
    let cur_chars = current_script.chars().count() as i64;
    let diff = cur_chars - prev_chars;
    if diff != 0 {
        let sign = if diff > 0 { "+" } else { "" };
        parts.push(format!("剧本 {sign}{diff} 字"));
    }

    if let (Ok(prev_arr), Ok(cur_arr)) = (
        serde_json::from_str::<Vec<serde_json::Value>>(&prev_nodes),
        serde_json::from_str::<Vec<serde_json::Value>>(current_nodes_json),
    ) {
        let p = prev_arr.len();
        let c = cur_arr.len();
        if p != c {
            parts.push(format!("画布节点 {p}→{c}"));
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("；"))
    }
}

fn diff_shots(prev_json: &str, cur_json: &str) -> Option<String> {
    let prev: Vec<serde_json::Value> = serde_json::from_str(prev_json).ok()?;
    let cur: Vec<serde_json::Value> = serde_json::from_str(cur_json).ok()?;

    let prev_map: HashMap<&str, &serde_json::Value> = prev
        .iter()
        .filter_map(|v| v.get("id").and_then(|id| id.as_str()).map(|id| (id, v)))
        .collect();
    let cur_map: HashMap<&str, &serde_json::Value> = cur
        .iter()
        .filter_map(|v| v.get("id").and_then(|id| id.as_str()).map(|id| (id, v)))
        .collect();

    let added = cur_map.keys().filter(|k| !prev_map.contains_key(*k)).count();
    let deleted = prev_map.keys().filter(|k| !cur_map.contains_key(*k)).count();
    let modified = cur_map
        .iter()
        .filter(|(k, v)| prev_map.get(*k).is_some_and(|pv| pv.to_string() != v.to_string()))
        .count();

    let mut parts = Vec::new();
    if added > 0 {
        parts.push(format!("新增 {added} 个分镜"));
    }
    if deleted > 0 {
        parts.push(format!("删除 {deleted} 个分镜"));
    }
    if modified > 0 {
        parts.push(format!("修改 {modified} 个分镜"));
    }

    if parts.is_empty() {
        None
    } else {
        let total = cur.len();
        Some(format!("{}（共 {total}）", parts.join("、")))
    }
}

pub fn restore(conn: &mut Connection, checkpoint_id: &str) -> Result<Episode> {
    let cp = episode_checkpoint::get_by_id(conn, checkpoint_id)?;

    episode_checkpoint::insert_full(
        conn,
        &CreateCheckpointInput {
            episode_id: cp.episode_id.clone(),
            label: None,
            trigger_type: Some("auto".into()),
        },
    )?;

    let tx = conn.transaction()?;

    tx.execute(
        "DELETE FROM shot WHERE episode_id = ?1",
        params![cp.episode_id],
    )?;

    let shots: Vec<serde_json::Value> = serde_json::from_str(&cp.shots_json)
        .map_err(|e| CoreError::Validation(format!("invalid shots_json in checkpoint: {e}")))?;

    for shot in &shots {
        let id = shot.get("id").and_then(|v| v.as_str()).ok_or_else(|| {
            CoreError::Validation("shot missing id field".into())
        })?;
        let episode_id = shot.get("episode_id").and_then(|v| v.as_str()).unwrap_or(&cp.episode_id);
        let order_index = shot.get("order_index").and_then(|v| v.as_i64()).unwrap_or(0);
        let summary = shot.get("summary").and_then(|v| v.as_str()).unwrap_or("");
        let duration_sec = shot.get("duration_sec").and_then(|v| v.as_f64());
        let camera_angle = shot.get("camera_angle").and_then(|v| v.as_str()).unwrap_or("");
        let shot_type = shot.get("shot_type").and_then(|v| v.as_str()).unwrap_or("");
        let mood = shot.get("mood").and_then(|v| v.as_str()).unwrap_or("");
        let dialogue = shot.get("dialogue").and_then(|v| v.as_str()).unwrap_or("");
        let video_prompt = shot.get("video_prompt").and_then(|v| v.as_str()).unwrap_or("");
        let image_prompt = shot.get("image_prompt").and_then(|v| v.as_str()).unwrap_or("");
        let status = shot.get("status").and_then(|v| v.as_str()).unwrap_or("draft");
        let adopted_asset_id = shot.get("adopted_asset_id").and_then(|v| v.as_str());

        tx.execute(
            "INSERT INTO shot (id, episode_id, order_index, summary, duration_sec, \
             camera_angle, shot_type, mood, dialogue, video_prompt, image_prompt, \
             status, adopted_asset_id, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, \
             datetime('now'), datetime('now'))",
            params![
                id, episode_id, order_index, summary, duration_sec,
                camera_angle, shot_type, mood, dialogue, video_prompt,
                image_prompt, status, adopted_asset_id,
            ],
        )?;
    }

    canvas_queries::upsert(
        &tx,
        UpsertCanvasLayoutInput {
            episode_id: cp.episode_id.clone(),
            nodes_json: cp.canvas_nodes_json,
            edges_json: cp.canvas_edges_json,
            viewport_json: cp.canvas_viewport_json,
        },
    )?;

    episode_queries::update(
        &tx,
        &cp.episode_id,
        crate::models::episode::UpdateEpisodeInput {
            title: None,
            script_text: Some(cp.script_text),
        },
    )?;

    tx.commit()?;

    episode_queries::get_by_id(conn, &cp.episode_id)
}

pub fn gc_auto_checkpoints(conn: &Connection, episode_id: &str) -> Result<usize> {
    let mut stmt = conn.prepare(
        "SELECT id, created_at FROM episode_checkpoint \
         WHERE episode_id = ?1 AND trigger_type = 'auto' \
         ORDER BY created_at DESC",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map(params![episode_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    if rows.is_empty() {
        return Ok(0);
    }

    let now = chrono::Utc::now();
    let mut to_delete: Vec<String> = Vec::new();
    let mut kept_days: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut kept_weeks: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (id, created_at) in &rows {
        let ts = chrono::NaiveDateTime::parse_from_str(created_at, "%Y-%m-%d %H:%M:%S")
            .unwrap_or_else(|_| now.naive_utc());
        let age = now.naive_utc() - ts;
        let hours = age.num_hours();

        if hours < 24 {
            continue;
        } else if hours < 24 * 7 {
            let day_key = ts.format("%Y-%m-%d").to_string();
            if kept_days.contains(&day_key) {
                to_delete.push(id.clone());
            } else {
                kept_days.insert(day_key);
            }
        } else {
            let week_key = format!("{}-W{}", ts.format("%Y"), ts.format("%W"));
            if kept_weeks.contains(&week_key) {
                to_delete.push(id.clone());
            } else {
                kept_weeks.insert(week_key);
            }
        }
    }

    if to_delete.is_empty() {
        return Ok(0);
    }

    let placeholders: Vec<String> = (1..=to_delete.len()).map(|i| format!("?{i}")).collect();
    let sql = format!(
        "DELETE FROM episode_checkpoint WHERE id IN ({})",
        placeholders.join(", ")
    );
    let params: Vec<&dyn rusqlite::types::ToSql> =
        to_delete.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
    let deleted = conn.execute(&sql, params.as_slice())?;
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use crate::db::queries::{
        canvas_layout as canvas_q, episode as episode_queries,
        episode_checkpoint, project as project_queries, shot as shot_queries,
    };
    use crate::models::canvas_layout::UpsertCanvasLayoutInput;
    use crate::models::episode::CreateEpisodeInput;
    use crate::models::project::CreateProjectInput;
    use crate::models::shot::CreateShotInput;
    use std::path::Path;
    use tempfile::tempdir;

    fn setup() -> (Connection, String) {
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
                script_text: Some("original script".into()),
            },
        )
        .unwrap();
        // Leak tempdir so it doesn't get cleaned up while conn is alive
        std::mem::forget(app_data);
        (conn, episode.id)
    }

    #[test]
    fn change_summary_detects_shot_changes() {
        let (conn, eid) = setup();
        shot_queries::create(
            &conn,
            CreateShotInput { episode_id: eid.clone(), summary: Some("a".into()) },
        ).unwrap();

        episode_checkpoint::insert_full(
            &conn,
            &CreateCheckpointInput { episode_id: eid.clone(), label: None, trigger_type: None },
        ).unwrap();

        shot_queries::create(
            &conn,
            CreateShotInput { episode_id: eid.clone(), summary: Some("b".into()) },
        ).unwrap();

        let shots = shot_queries::list(&conn, crate::models::shot::ListShotsOptions { episode_id: eid.clone() }).unwrap();
        let shots_json = serde_json::to_string(&shots).unwrap();
        let summary = compute_change_summary(&conn, &eid, "original script", &shots_json, "[]");

        assert!(summary.is_some());
        let s = summary.unwrap();
        assert!(s.contains("新增 1 个分镜"), "got: {s}");
        assert!(s.contains("共 2"), "got: {s}");
    }

    #[test]
    fn restore_replaces_shots_and_script() {
        let (mut conn, eid) = setup();
        canvas_q::upsert(
            &conn,
            UpsertCanvasLayoutInput {
                episode_id: eid.clone(),
                nodes_json: "[{\"id\":\"n1\"}]".into(),
                edges_json: "[]".into(),
                viewport_json: "{}".into(),
            },
        ).unwrap();

        shot_queries::create(
            &conn,
            CreateShotInput { episode_id: eid.clone(), summary: Some("keep me".into()) },
        ).unwrap();

        let cp = episode_checkpoint::insert_full(
            &conn,
            &CreateCheckpointInput { episode_id: eid.clone(), label: Some("v1".into()), trigger_type: None },
        ).unwrap();

        shot_queries::create(
            &conn,
            CreateShotInput { episode_id: eid.clone(), summary: Some("new shot".into()) },
        ).unwrap();
        episode_queries::update(
            &conn, &eid,
            crate::models::episode::UpdateEpisodeInput { title: None, script_text: Some("changed".into()) },
        ).unwrap();

        let restored_ep = restore(&mut conn, &cp.id).unwrap();
        assert_eq!(restored_ep.script_text, "original script");

        let shots = shot_queries::list(
            &conn,
            crate::models::shot::ListShotsOptions { episode_id: eid.clone() },
        ).unwrap();
        assert_eq!(shots.len(), 1);
        assert_eq!(shots[0].summary, "keep me");
    }

    #[test]
    fn restore_creates_auto_checkpoint_first() {
        let (mut conn, eid) = setup();
        canvas_q::upsert(
            &conn,
            UpsertCanvasLayoutInput {
                episode_id: eid.clone(),
                nodes_json: "[]".into(),
                edges_json: "[]".into(),
                viewport_json: "{}".into(),
            },
        ).unwrap();

        let cp = episode_checkpoint::insert_full(
            &conn,
            &CreateCheckpointInput { episode_id: eid.clone(), label: None, trigger_type: None },
        ).unwrap();

        restore(&mut conn, &cp.id).unwrap();

        let all = episode_checkpoint::list(&conn, &eid).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].trigger_type, "auto");
        assert_eq!(all[1].trigger_type, "manual");
    }

    #[test]
    fn gc_preserves_manual_and_recent_auto() {
        let (conn, eid) = setup();

        // Insert rows directly to control timestamps without triggering GC.
        let manual_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO episode_checkpoint \
             (id, episode_id, version_number, label, trigger_type, script_text, shots_json) \
             VALUES (?1, ?2, 1, 'manual', 'manual', '', '[]')",
            params![manual_id, eid],
        ).unwrap();

        let recent_auto_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO episode_checkpoint \
             (id, episode_id, version_number, trigger_type, script_text, shots_json) \
             VALUES (?1, ?2, 2, 'auto', '', '[]')",
            params![recent_auto_id, eid],
        ).unwrap();

        // Two old autos in the same week — GC keeps one, deletes the other.
        let old_auto_a = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO episode_checkpoint \
             (id, episode_id, version_number, trigger_type, script_text, shots_json, \
              created_at) \
             VALUES (?1, ?2, 3, 'auto', '', '[]', datetime('now', '-10 days'))",
            params![old_auto_a, eid],
        ).unwrap();

        let old_auto_b = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO episode_checkpoint \
             (id, episode_id, version_number, trigger_type, script_text, shots_json, \
              created_at) \
             VALUES (?1, ?2, 4, 'auto', '', '[]', datetime('now', '-11 days'))",
            params![old_auto_b, eid],
        ).unwrap();

        let deleted = gc_auto_checkpoints(&conn, &eid).unwrap();
        assert_eq!(deleted, 1, "should delete older auto in same week");

        let remaining = episode_checkpoint::list(&conn, &eid).unwrap();
        assert_eq!(remaining.len(), 3);
        let types: Vec<&str> = remaining.iter().map(|r| r.trigger_type.as_str()).collect();
        assert!(types.contains(&"manual"), "manual must survive GC");
    }
}
