use rusqlite::{params, Connection};

use crate::error::{CoreError, Result};
use crate::models::asset::{Asset, AssetType, ListAssetsOptions};

const SELECT_COLUMNS: &str = "id, project_id, shot_id, asset_type, original_name, \
                              file_path, thumbnail_path, file_size, content_hash, \
                              metadata_json, created_at, updated_at";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<Asset> {
    let type_str: String = row.get(3)?;
    let asset_type = AssetType::from_db_str(&type_str).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            3,
            rusqlite::types::Type::Text,
            format!("unknown asset_type: {type_str}").into(),
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
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
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

    if let Some(t) = opts.asset_type {
        let mut stmt = conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM asset \
             WHERE project_id = ?1 AND asset_type = ?2 \
             ORDER BY created_at DESC LIMIT ?3 OFFSET ?4"
        ))?;
        let rows = stmt.query_map(
            params![opts.project_id, t.as_db_str(), limit, offset],
            map_row,
        )?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(CoreError::from)
    } else {
        let mut stmt = conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM asset \
             WHERE project_id = ?1 \
             ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
        ))?;
        let rows = stmt.query_map(params![opts.project_id, limit, offset], map_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(CoreError::from)
    }
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
}
