use rusqlite::Connection;
use std::path::Path;
use tokio_rusqlite::Connection as AsyncConnection;

#[derive(thiserror::Error, Debug)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("async sqlite error: {0}")]
    AsyncSqlite(#[from] tokio_rusqlite::Error),
    #[error("migration error: {0}")]
    Migration(#[from] super::migrator::MigrationError),
}

fn init_pragmas(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;
         PRAGMA synchronous = NORMAL;",
    )
}

/// Open a synchronous database connection (for CLI usage).
/// Automatically applies pragmas and pending migrations.
pub fn open_sync(path: &Path) -> Result<Connection, DbError> {
    let conn = Connection::open(path)?;
    init_pragmas(&conn)?;
    super::migrator::run_migrations(&conn)?;
    Ok(conn)
}

/// Open an async database connection (for Tauri usage).
/// Automatically applies pragmas and pending migrations.
pub async fn open_async(path: &Path) -> Result<AsyncConnection, DbError> {
    let path = path.to_path_buf();
    let conn = AsyncConnection::open(&path).await?;
    conn.call(|conn| {
        init_pragmas(conn)?;
        super::migrator::run_migrations(conn)?;
        Ok(())
    })
    .await
    .map_err(|e| match e {
        tokio_rusqlite::Error::ConnectionClosed => {
            DbError::AsyncSqlite(tokio_rusqlite::Error::ConnectionClosed)
        }
        tokio_rusqlite::Error::Close((c, err)) => {
            DbError::AsyncSqlite(tokio_rusqlite::Error::Close((c, err)))
        }
        tokio_rusqlite::Error::Error(inner) => inner,
        _ => DbError::AsyncSqlite(tokio_rusqlite::Error::ConnectionClosed),
    })?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn open_memory() -> Connection {
        open_sync(Path::new(":memory:")).unwrap()
    }

    #[test]
    fn test_migrations_create_all_tables() {
        let conn = open_memory();

        let tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master \
                 WHERE type='table' AND name NOT LIKE '\\_%' ESCAPE '\\' \
                 ORDER BY name",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(tables.len(), 17, "expected 17 tables, got: {tables:?}");
    }

    #[test]
    fn test_migrations_idempotent() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let conn = open_sync(tmp.path()).unwrap();
        drop(conn);
        let _conn = open_sync(tmp.path()).unwrap();
    }

    #[test]
    fn test_foreign_keys_enforced() {
        let conn = open_memory();
        let result = conn.execute(
            "INSERT INTO episode (id, project_id, title) VALUES ('e1', 'nonexistent', 'test')",
            [],
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_wal_mode_enabled() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let conn = open_sync(tmp.path()).unwrap();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }

    #[test]
    fn test_pragmas_configured() {
        let conn = open_memory();

        let timeout: i64 = conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .unwrap();
        assert_eq!(timeout, 5000);

        let sync: i64 = conn
            .query_row("PRAGMA synchronous", [], |row| row.get(0))
            .unwrap();
        assert_eq!(sync, 1); // NORMAL = 1
    }

    #[tokio::test]
    async fn test_async_connection() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let conn = open_async(tmp.path()).await.unwrap();

        let count: i64 = conn
            .call(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                    [],
                    |row| row.get(0),
                )
            })
            .await
            .unwrap();

        // 17 business tables + 1 _migrations table
        assert!(count >= 17);
    }

    #[test]
    fn test_generation_task_restrict_delete() {
        let conn = open_memory();

        conn.execute_batch(
            "INSERT INTO provider (id, name) VALUES ('p1', 'TestProvider');
             INSERT INTO model (id, provider_id, name, model_type) VALUES ('m1', 'p1', 'TestModel', 'image');
             INSERT INTO api_account (id, provider_id, api_key_ref) VALUES ('a1', 'p1', 'ref');
             INSERT INTO generation_task (id, provider_id, model_id, account_id, task_type)
                 VALUES ('t1', 'p1', 'm1', 'a1', 'image');",
        )
        .unwrap();

        let result = conn.execute("DELETE FROM provider WHERE id = 'p1'", []);
        assert!(result.is_err());
    }
}
