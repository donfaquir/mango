use rusqlite::{params, Connection};

use crate::error::Result;

struct ProviderSeed {
    id: &'static str,
    name: &'static str,
    /// Empty until the provider's real endpoint is wired up; reflected back to
    /// the UI as-is so missing entries surface during MS2.
    base_url: &'static str,
    docs_url: &'static str,
    models: &'static [ModelSeed],
}

struct ModelSeed {
    id: &'static str,
    name: &'static str,
    /// One of: 'image' | 'video' | 'text' | 'audio' (matches the CHECK on
    /// `model.model_type`).
    model_type: &'static str,
}

const PROVIDERS: &[ProviderSeed] = &[
    ProviderSeed {
        id: "kling",
        name: "可灵",
        base_url: "",
        docs_url: "",
        models: &[
            ModelSeed {
                id: "kling-image-v1",
                name: "可灵图像 v1",
                model_type: "image",
            },
            ModelSeed {
                id: "kling-video-v1",
                name: "可灵视频 v1",
                model_type: "video",
            },
        ],
    },
    ProviderSeed {
        id: "jimeng",
        name: "即梦",
        base_url: "",
        docs_url: "",
        models: &[
            ModelSeed {
                id: "jimeng-image-v1",
                name: "即梦图像 v1",
                model_type: "image",
            },
            ModelSeed {
                id: "jimeng-video-v1",
                name: "即梦视频 v1",
                model_type: "video",
            },
        ],
    },
];

/// Apply provider/model seed data idempotently. System-owned columns
/// (`name`, `base_url`, `auth_type`, `docs_url`, `model_type`) are FORCED to
/// match the code on every startup via `DO UPDATE` — users must not hand-edit
/// these rows. User-owned data (`api_account` rows referencing `provider_id`)
/// is untouched.
pub fn apply(conn: &Connection) -> Result<()> {
    for p in PROVIDERS {
        conn.execute(
            "INSERT INTO provider (id, name, base_url, auth_type, docs_url) \
             VALUES (?1, ?2, ?3, 'api_key', ?4) \
             ON CONFLICT(id) DO UPDATE SET \
                name = excluded.name, \
                base_url = excluded.base_url, \
                auth_type = excluded.auth_type, \
                docs_url = excluded.docs_url",
            params![p.id, p.name, p.base_url, p.docs_url],
        )?;
        for m in p.models {
            conn.execute(
                "INSERT INTO model (id, provider_id, name, model_type) \
                 VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT(id) DO UPDATE SET \
                    name = excluded.name, \
                    model_type = excluded.model_type",
                params![m.id, p.id, m.name, m.model_type],
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use std::path::Path;

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn apply_creates_expected_rows() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        apply(&conn).unwrap();
        assert_eq!(count(&conn, "provider"), 2);
        assert_eq!(count(&conn, "model"), 4);
    }

    #[test]
    fn apply_is_idempotent() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        apply(&conn).unwrap();
        apply(&conn).unwrap();
        apply(&conn).unwrap();
        assert_eq!(count(&conn, "provider"), 2);
        assert_eq!(count(&conn, "model"), 4);
    }

    #[test]
    fn apply_overwrites_system_owned_columns() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        apply(&conn).unwrap();

        // Pretend a user (or a buggy migration) damaged the kling row.
        conn.execute(
            "UPDATE provider SET name = 'tampered', base_url = 'http://bogus' \
             WHERE id = 'kling'",
            [],
        )
        .unwrap();

        apply(&conn).unwrap();

        let (name, base_url): (String, String) = conn
            .query_row(
                "SELECT name, base_url FROM provider WHERE id = 'kling'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(name, "可灵");
        assert_eq!(base_url, "");
    }

    #[test]
    fn apply_preserves_user_api_accounts() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        apply(&conn).unwrap();
        conn.execute(
            "INSERT INTO api_account (id, provider_id, api_key_ref) \
             VALUES ('acc1', 'kling', 'api_account:acc1')",
            [],
        )
        .unwrap();

        apply(&conn).unwrap();

        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM api_account WHERE id = 'acc1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }
}
