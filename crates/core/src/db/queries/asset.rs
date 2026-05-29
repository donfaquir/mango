use rusqlite::types::ToSql;
use rusqlite::{params, Connection};

use crate::error::{CoreError, Result};
use crate::models::asset::{Asset, AssetSource, AssetType, ListAssetsOptions};

const SELECT_COLUMNS: &str = "id, project_id, shot_id, asset_type, original_name, \
                              file_path, thumbnail_path, file_size, content_hash, \
                              metadata_json, source, label, created_at, updated_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<Asset> {
    let type_str: String = row.get(3)?;
    let asset_type = AssetType::from_db_str(&type_str).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            3,
            rusqlite::types::Type::Text,
            format!("unknown asset_type: {type_str}").into(),
        )
    })?;
    let source_str: String = row.get(10)?;
    let source = AssetSource::from_db_str(&source_str).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            10,
            rusqlite::types::Type::Text,
            format!("unknown asset source: {source_str}").into(),
        )
    })?;
    Ok(Asset {
        id: row.get(0)?,
        project_id: row.get(1)?,
        shot_id: row.get(2)?,
        asset_type,
        original_name: row.get(4)?,
        file_path: row.get(5)?,
        thumbnail_path: row.get(6)?,
        file_size: row.get(7)?,
        content_hash: row.get(8)?,
        metadata_json: row.get(9)?,
        source,
        label: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Asset> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM asset WHERE id = ?1"),
        params![id],
        map_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
            entity: "asset",
            id: id.to_string(),
        },
        other => CoreError::Sqlite(other),
    })
}

pub fn list(conn: &Connection, opts: ListAssetsOptions) -> Result<Vec<Asset>> {
    let limit = opts.limit.unwrap_or(50).clamp(1, 200);
    let offset = opts.offset.unwrap_or(0).max(0);

    // Build the WHERE clause dynamically so callers can combine any subset
    // of (asset_type, source, keyword) without us needing a separate query
    // per shape. Bind values are pushed in lockstep with their `?N` markers.
    let mut sql = format!("SELECT {SELECT_COLUMNS} FROM asset WHERE project_id = ?1");
    let mut args: Vec<Box<dyn ToSql>> = vec![Box::new(opts.project_id.clone())];

    if let Some(t) = opts.asset_type {
        args.push(Box::new(t.as_db_str().to_string()));
        sql.push_str(&format!(" AND asset_type = ?{}", args.len()));
    }
    if let Some(s) = opts.source {
        args.push(Box::new(s.as_db_str().to_string()));
        sql.push_str(&format!(" AND source = ?{}", args.len()));
    }
    if let Some(kw) = opts.keyword.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        // SQLite LIKE is case-insensitive only for ASCII by default, which is
        // fine for our filenames + labels. Wrap the user input in `%` and
        // bind a single value reused for both columns.
        let pattern = format!("%{kw}%");
        args.push(Box::new(pattern));
        let n = args.len();
        sql.push_str(&format!(
            " AND (original_name LIKE ?{n} OR label LIKE ?{n})"
        ));
    }

    args.push(Box::new(limit));
    let limit_idx = args.len();
    args.push(Box::new(offset));
    let offset_idx = args.len();
    sql.push_str(&format!(
        " ORDER BY created_at DESC LIMIT ?{limit_idx} OFFSET ?{offset_idx}"
    ));

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), map_row)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(CoreError::from)
}

/// Update the free-form `label` of an asset. Bumps `updated_at` so library
/// listings re-sort accordingly. Returns the post-update row so the caller
/// (and frontend cache) sees the canonical state in a single roundtrip.
pub fn update_label(conn: &Connection, id: &str, label: &str) -> Result<Asset> {
    let n = conn.execute(
        "UPDATE asset SET label = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![label, id],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound {
            entity: "asset",
            id: id.to_string(),
        });
    }
    get_by_id(conn, id)
}

/// Bind (or unbind) an asset to a shot. `Some(shot_id)` overwrites any existing
/// `asset.shot_id`; `None` clears it. The previous value is intentionally not
/// returned here — callers that need overwrite confirmation should `get_by_id`
/// first and compare. Returns the post-update row so the caller (and frontend
/// cache) sees the canonical state in a single roundtrip.
pub fn assign_to_shot(
    conn: &Connection,
    id: &str,
    shot_id: Option<&str>,
) -> Result<Asset> {
    let n = conn.execute(
        "UPDATE asset SET shot_id = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![shot_id, id],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound {
            entity: "asset",
            id: id.to_string(),
        });
    }
    get_by_id(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let n = conn.execute("DELETE FROM asset WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(CoreError::NotFound {
            entity: "asset",
            id: id.to_string(),
        });
    }
    Ok(())
}

/// Look up an existing asset by `(project_id, content_hash)`. Used by the
/// import pipeline to dedupe re-imports of the same bytes. Returns
/// `Ok(None)` when no row matches.
pub fn find_by_content_hash(
    conn: &Connection,
    project_id: &str,
    content_hash: &str,
) -> Result<Option<Asset>> {
    let res = conn.query_row(
        &format!(
            "SELECT {SELECT_COLUMNS} FROM asset \
             WHERE project_id = ?1 AND content_hash = ?2 LIMIT 1"
        ),
        params![project_id, content_hash],
        map_row,
    );
    match res {
        Ok(a) => Ok(Some(a)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(CoreError::Sqlite(e)),
    }
}

/// Look up an existing asset by `(project_id, file_path)`. The `file_path`
/// column stores a project-root-relative POSIX path (e.g. `assets/<uuid>.png`),
/// which is also what other tables (notably `character.reference_image_path`)
/// persist when the underlying bytes were imported via the asset pipeline.
/// This lets callers translate such a path back into the canonical asset row
/// — e.g. to pass `asset_id` into the generation runner instead of a raw path.
/// Returns `Ok(None)` when no row matches.
pub fn find_by_file_path(
    conn: &Connection,
    project_id: &str,
    file_path: &str,
) -> Result<Option<Asset>> {
    let res = conn.query_row(
        &format!(
            "SELECT {SELECT_COLUMNS} FROM asset \
             WHERE project_id = ?1 AND file_path = ?2 LIMIT 1"
        ),
        params![project_id, file_path],
        map_row,
    );
    match res {
        Ok(a) => Ok(Some(a)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(CoreError::Sqlite(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use crate::db::queries::project as project_queries;
    use crate::models::project::CreateProjectInput;
    use std::path::Path;
    use tempfile::{tempdir, TempDir};

    fn setup() -> (Connection, TempDir, String) {
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
        (conn, app_data, project.id)
    }

    fn insert_raw(conn: &Connection, project_id: &str, hash: Option<&str>, asset_type: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO asset \
                (id, project_id, asset_type, original_name, file_path, file_size, content_hash) \
             VALUES (?1, ?2, ?3, 'x', 'assets/x', 0, ?4)",
            params![id, project_id, asset_type, hash],
        )
        .unwrap();
        id
    }

    #[test]
    fn get_by_id_returns_not_found_for_missing() {
        let (conn, _td, _pid) = setup();
        assert!(matches!(
            get_by_id(&conn, "no-such"),
            Err(CoreError::NotFound { .. })
        ));
    }

    #[test]
    fn list_filters_by_project_and_type() {
        let (conn, td, pid_a) = setup();
        let pid_b = project_queries::create(
            &conn,
            td.path(),
            CreateProjectInput {
                name: "B".into(),
                root_path: None,
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap()
        .id;

        insert_raw(&conn, &pid_a, Some("h1"), "image");
        insert_raw(&conn, &pid_a, Some("h2"), "image");
        insert_raw(&conn, &pid_a, Some("h3"), "video");
        insert_raw(&conn, &pid_b, Some("h4"), "image");

        let all = list(
            &conn,
            ListAssetsOptions {
                project_id: pid_a.clone(),
                asset_type: None,
                source: None,
                keyword: None,
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        assert_eq!(all.len(), 3);

        let images = list(
            &conn,
            ListAssetsOptions {
                project_id: pid_a,
                asset_type: Some(AssetType::Image),
                source: None,
                keyword: None,
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        assert_eq!(images.len(), 2);
        assert!(images.iter().all(|a| matches!(a.asset_type, AssetType::Image)));
    }

    #[test]
    fn delete_then_get_returns_not_found() {
        let (conn, _td, pid) = setup();
        let id = insert_raw(&conn, &pid, Some("hh"), "image");
        delete(&conn, &id).unwrap();
        assert!(matches!(
            get_by_id(&conn, &id),
            Err(CoreError::NotFound { .. })
        ));
    }

    #[test]
    fn delete_missing_returns_not_found() {
        let (conn, _td, _pid) = setup();
        assert!(matches!(
            delete(&conn, "no-such"),
            Err(CoreError::NotFound { .. })
        ));
    }

    fn insert_with_path(
        conn: &Connection,
        project_id: &str,
        file_path: &str,
        hash: &str,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO asset \
                (id, project_id, asset_type, original_name, file_path, file_size, content_hash) \
             VALUES (?1, ?2, 'image', 'x', ?3, 0, ?4)",
            params![id, project_id, file_path, hash],
        )
        .unwrap();
        id
    }

    #[test]
    fn find_by_file_path_returns_matching_row() {
        let (conn, _td, pid) = setup();
        let id = insert_with_path(&conn, &pid, "assets/abc.png", "h-1");

        let found = find_by_file_path(&conn, &pid, "assets/abc.png").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, id);
    }

    #[test]
    fn find_by_file_path_returns_none_when_missing() {
        let (conn, _td, pid) = setup();
        let miss = find_by_file_path(&conn, &pid, "assets/ghost.png").unwrap();
        assert!(miss.is_none());
    }

    #[test]
    fn find_by_file_path_is_scoped_to_project() {
        // Two projects holding rows at the same relative file_path must not
        // collide — the query must only return the project-scoped row.
        let (conn, td, pid_a) = setup();
        let pid_b = project_queries::create(
            &conn,
            td.path(),
            CreateProjectInput {
                name: "B".into(),
                root_path: None,
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap()
        .id;

        let id_a = insert_with_path(&conn, &pid_a, "assets/shared.png", "h-a");
        let id_b = insert_with_path(&conn, &pid_b, "assets/shared.png", "h-b");

        let from_a = find_by_file_path(&conn, &pid_a, "assets/shared.png")
            .unwrap()
            .unwrap();
        assert_eq!(from_a.id, id_a);

        let from_b = find_by_file_path(&conn, &pid_b, "assets/shared.png")
            .unwrap()
            .unwrap();
        assert_eq!(from_b.id, id_b);
    }

    #[test]
    fn find_by_content_hash_scoped_to_project() {
        let (conn, td, pid_a) = setup();
        let pid_b = project_queries::create(
            &conn,
            td.path(),
            CreateProjectInput {
                name: "B".into(),
                root_path: None,
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap()
        .id;

        let id_a = insert_raw(&conn, &pid_a, Some("abc"), "image");
        let _id_b = insert_raw(&conn, &pid_b, Some("abc"), "image");

        let found = find_by_content_hash(&conn, &pid_a, "abc").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, id_a);

        let miss = find_by_content_hash(&conn, &pid_a, "no-such-hash").unwrap();
        assert!(miss.is_none());
    }

    fn insert_full(
        conn: &Connection,
        project_id: &str,
        asset_type: &str,
        original_name: &str,
        source: &str,
        label: &str,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO asset \
                (id, project_id, asset_type, original_name, file_path, file_size, source, label) \
             VALUES (?1, ?2, ?3, ?4, 'assets/x', 0, ?5, ?6)",
            params![id, project_id, asset_type, original_name, source, label],
        )
        .unwrap();
        id
    }

    #[test]
    fn list_filters_by_source() {
        let (conn, _td, pid) = setup();
        insert_full(&conn, &pid, "image", "a.png", "imported", "");
        insert_full(&conn, &pid, "image", "b.png", "generated", "");
        insert_full(&conn, &pid, "image", "c.png", "generated", "");

        let imported = list(
            &conn,
            ListAssetsOptions {
                project_id: pid.clone(),
                asset_type: None,
                source: Some(AssetSource::Imported),
                keyword: None,
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        assert_eq!(imported.len(), 1);
        assert!(imported.iter().all(|a| matches!(a.source, AssetSource::Imported)));

        let generated = list(
            &conn,
            ListAssetsOptions {
                project_id: pid,
                asset_type: None,
                source: Some(AssetSource::Generated),
                keyword: None,
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        assert_eq!(generated.len(), 2);
    }

    #[test]
    fn list_filters_by_keyword_against_name_and_label() {
        let (conn, _td, pid) = setup();
        insert_full(&conn, &pid, "image", "sunset.png", "imported", "");
        insert_full(&conn, &pid, "image", "forest.png", "imported", "hero");
        insert_full(&conn, &pid, "image", "misc.png", "imported", "");

        let by_name = list(
            &conn,
            ListAssetsOptions {
                project_id: pid.clone(),
                asset_type: None,
                source: None,
                keyword: Some("sun".into()),
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].original_name, "sunset.png");

        let by_label = list(
            &conn,
            ListAssetsOptions {
                project_id: pid.clone(),
                asset_type: None,
                source: None,
                keyword: Some("hero".into()),
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        assert_eq!(by_label.len(), 1);
        assert_eq!(by_label[0].label, "hero");

        // Whitespace-only keyword is treated as no filter.
        let no_filter = list(
            &conn,
            ListAssetsOptions {
                project_id: pid,
                asset_type: None,
                source: None,
                keyword: Some("   ".into()),
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        assert_eq!(no_filter.len(), 3);
    }

    #[test]
    fn update_label_persists_value() {
        let (conn, _td, pid) = setup();
        let id = insert_full(&conn, &pid, "image", "a.png", "imported", "");

        let updated = update_label(&conn, &id, "hero").unwrap();
        assert_eq!(updated.label, "hero");
        let row = get_by_id(&conn, &id).unwrap();
        assert_eq!(row.label, "hero");
    }

    #[test]
    fn update_label_missing_returns_not_found() {
        let (conn, _td, _pid) = setup();
        assert!(matches!(
            update_label(&conn, "no-such", "x"),
            Err(CoreError::NotFound { .. })
        ));
    }

    fn insert_episode(conn: &Connection, project_id: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO episode (id, project_id, title) VALUES (?1, ?2, 'ep')",
            params![id, project_id],
        )
        .unwrap();
        id
    }

    fn insert_shot(conn: &Connection, episode_id: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO shot (id, episode_id) VALUES (?1, ?2)",
            params![id, episode_id],
        )
        .unwrap();
        id
    }

    #[test]
    fn assign_to_shot_binds_when_empty() {
        let (conn, _td, pid) = setup();
        let asset = insert_raw(&conn, &pid, Some("h"), "image");
        let eid = insert_episode(&conn, &pid);
        let sid = insert_shot(&conn, &eid);

        let updated = assign_to_shot(&conn, &asset, Some(&sid)).unwrap();
        assert_eq!(updated.shot_id.as_deref(), Some(sid.as_str()));
    }

    #[test]
    fn assign_to_shot_overwrites_existing_binding() {
        let (conn, _td, pid) = setup();
        let asset = insert_raw(&conn, &pid, Some("h"), "image");
        let eid = insert_episode(&conn, &pid);
        let sid_a = insert_shot(&conn, &eid);
        let sid_b = insert_shot(&conn, &eid);

        assign_to_shot(&conn, &asset, Some(&sid_a)).unwrap();
        let updated = assign_to_shot(&conn, &asset, Some(&sid_b)).unwrap();
        assert_eq!(updated.shot_id.as_deref(), Some(sid_b.as_str()));
    }

    #[test]
    fn assign_to_shot_unbinds_with_none() {
        let (conn, _td, pid) = setup();
        let asset = insert_raw(&conn, &pid, Some("h"), "image");
        let eid = insert_episode(&conn, &pid);
        let sid = insert_shot(&conn, &eid);

        assign_to_shot(&conn, &asset, Some(&sid)).unwrap();
        let updated = assign_to_shot(&conn, &asset, None).unwrap();
        assert!(updated.shot_id.is_none());
    }

    #[test]
    fn assign_to_shot_returns_not_found_for_missing_asset() {
        let (conn, _td, _pid) = setup();
        assert!(matches!(
            assign_to_shot(&conn, "no-such", None),
            Err(CoreError::NotFound { .. })
        ));
    }
}
