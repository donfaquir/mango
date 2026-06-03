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

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial",
        sql: include_str!("migrations/001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "add_project_root_path",
        sql: include_str!("migrations/002_add_project_root_path.sql"),
    },
    Migration {
        version: 3,
        name: "add_api_account_key_last4",
        sql: include_str!("migrations/003_add_api_account_key_last4.sql"),
    },
    Migration {
        version: 4,
        name: "add_api_account_params_json",
        sql: include_str!("migrations/004_add_api_account_params_json.sql"),
    },
    Migration {
        version: 5,
        name: "add_generation_task_project_id",
        sql: include_str!("migrations/005_add_generation_task_project_id.sql"),
    },
    Migration {
        version: 6,
        name: "add_generation_task_event",
        sql: include_str!("migrations/006_add_generation_task_event.sql"),
    },
    Migration {
        version: 7,
        name: "add_api_account_deleted_at",
        sql: include_str!("migrations/007_add_api_account_deleted_at.sql"),
    },
    Migration {
        version: 8,
        name: "asset_source_label",
        sql: include_str!("migrations/008_asset_source_label.sql"),
    },
    Migration {
        version: 9,
        name: "add_generation_task_batch_id",
        sql: include_str!("migrations/009_generation_task_batch_id.sql"),
    },
    Migration {
        version: 11,
        name: "add_app_preference",
        sql: include_str!("migrations/011_app_preference.sql"),
    },
    Migration {
        version: 12,
        name: "shot_adopted_asset",
        sql: include_str!("migrations/012_shot_adopted_asset.sql"),
    },
];

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
