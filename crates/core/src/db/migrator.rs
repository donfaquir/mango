use rusqlite::Connection;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MigrationError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("migration {version} failed: {reason}")]
    Failed { version: i64, reason: String },
}

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "initial",
    sql: include_str!("migrations/001_initial.sql"),
}];

/// Execute all pending migrations. Each migration runs in its own transaction.
pub fn run_migrations(conn: &Connection) -> Result<(), MigrationError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;

    let current_version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM _migrations",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    for migration in MIGRATIONS.iter().filter(|m| m.version > current_version) {
        tracing::info!("applying migration {} ({})", migration.version, migration.name);

        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(migration.sql)?;
        tx.execute(
            "INSERT INTO _migrations (version, name) VALUES (?1, ?2)",
            rusqlite::params![migration.version, migration.name],
        )?;
        tx.commit()?;
    }

    Ok(())
}
