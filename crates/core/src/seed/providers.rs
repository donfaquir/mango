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
    /// JSON metadata used by frontend filtering and submit-time validation.
    capabilities_json: &'static str,
    /// JSON defaults merged by UI when users don't specify optional params.
    default_params_json: &'static str,
}

const PROVIDERS: &[ProviderSeed] = &[
    ProviderSeed {
        // spec-16: bailian replaces the dropped `kling`. wan2.7-image-pro is the
        // text-to-image entry; happyhorse-1.0-r2v is the reference-to-video model
        // that requires an OSS-uploaded reference image (see SPEC-17).
        id: "bailian",
        name: "阿里云百炼",
        base_url: "https://dashscope.aliyuncs.com/api/v1",
        docs_url: "https://help.aliyun.com/zh/model-studio/",
        models: &[
            ModelSeed {
                id: "wan2.7-image-pro",
                name: "通义万相 2.7 Pro（文生图）",
                model_type: "image",
                capabilities_json: r#"{"task_types":["image"],"requires_reference_media":false,"sync_submit":true,"supported_sizes":["1K","2K"],"max_n":4}"#,
                default_params_json: r#"{"size":"2K","n":1,"enable_sequential":false,"negative_prompt":null}"#,
            },
            ModelSeed {
                id: "happyhorse-1.0-r2v",
                name: "快乐马 1.0（参考图生视频）",
                model_type: "video",
                capabilities_json: r#"{"task_types":["video"],"requires_reference_media":true,"sync_submit":false,"supported_resolutions":["720P","1080P"],"supported_ratios":["16:9","9:16","1:1"],"duration_sec":[5,10]}"#,
                default_params_json: r#"{"resolution":"720P","ratio":"16:9","duration":5}"#,
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
                capabilities_json: r#"{"task_types":["image"],"requires_reference_media":false}"#,
                default_params_json: r#"{}"#,
            },
            ModelSeed {
                id: "jimeng-video-v1",
                name: "即梦视频 v1",
                model_type: "video",
                capabilities_json: r#"{"task_types":["video"],"requires_reference_media":false}"#,
                default_params_json: r#"{}"#,
            },
        ],
    },
];

/// Provider IDs that have been retired. spec-16 actively deletes their rows
/// (FK CASCADE clears the dependent `model` and `api_account` entries) on
/// every startup so old DBs converge to the new schema.
const DEPRECATED_PROVIDERS: &[&str] = &["kling"];

/// Apply provider/model seed data idempotently. System-owned columns
/// (`name`, `base_url`, `auth_type`, `docs_url`, `model_type`,
/// `capabilities_json`, `default_params_json`) are FORCED to
/// match the code on every startup via `DO UPDATE` — users must not hand-edit
/// these rows. User-owned data (`api_account` rows referencing `provider_id`)
/// is untouched.
pub fn apply(conn: &Connection) -> Result<()> {
    // Deprecated provider rows are removed first. Any orphaned api_account
    // rows that referenced them disappear via FK CASCADE — acceptable in MS2
    // because the project has no shipped users.
    for id in DEPRECATED_PROVIDERS {
        let n = conn.execute("DELETE FROM provider WHERE id = ?1", params![id])?;
        if n > 0 {
            tracing::warn!(
                "deleted deprecated provider '{id}' (cascade removed dependent rows)"
            );
        }
    }

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
                "INSERT INTO model (id, provider_id, name, model_type, capabilities_json, default_params_json) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT(id) DO UPDATE SET \
                    name = excluded.name, \
                    model_type = excluded.model_type, \
                    capabilities_json = excluded.capabilities_json, \
                    default_params_json = excluded.default_params_json",
                params![
                    m.id,
                    p.id,
                    m.name,
                    m.model_type,
                    m.capabilities_json,
                    m.default_params_json
                ],
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
        // bailian + jimeng = 2 providers, 4 models total.
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

        conn.execute(
            "UPDATE provider SET name = 'tampered', base_url = 'http://bogus' \
             WHERE id = 'bailian'",
            [],
        )
        .unwrap();

        apply(&conn).unwrap();

        let (name, base_url): (String, String) = conn
            .query_row(
                "SELECT name, base_url FROM provider WHERE id = 'bailian'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(name, "阿里云百炼");
        assert_eq!(base_url, "https://dashscope.aliyuncs.com/api/v1");
    }

    #[test]
    fn apply_preserves_user_api_accounts() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        apply(&conn).unwrap();
        // bailian is now the canonical provider; user-owned rows referencing
        // it must survive a re-apply.
        conn.execute(
            "INSERT INTO api_account (id, provider_id, api_key_ref) \
             VALUES ('acc1', 'bailian', 'api_account:acc1')",
            [],
        )
        .unwrap();

        apply(&conn).unwrap();

        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM api_account WHERE id = 'acc1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn apply_deletes_kling_with_cascade() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        // Pretend a previous schema version had kling + a child api_account.
        conn.execute(
            "INSERT INTO provider (id, name) VALUES ('kling', 'kling-legacy')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO model (id, provider_id, name, model_type) \
             VALUES ('kling-image-v1', 'kling', 'k', 'image')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO api_account (id, provider_id, api_key_ref) \
             VALUES ('legacy', 'kling', 'api_account:legacy')",
            [],
        )
        .unwrap();

        apply(&conn).unwrap();

        let provider_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM provider WHERE id = 'kling'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(provider_count, 0);
        let account_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM api_account WHERE id = 'legacy'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(account_count, 0);
    }

    #[test]
    fn apply_creates_bailian_with_two_models() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        apply(&conn).unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM model WHERE provider_id = 'bailian'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn apply_writes_model_capabilities_and_defaults() {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        apply(&conn).unwrap();
        let (caps, defaults): (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT capabilities_json, default_params_json \
                 FROM model WHERE id = 'wan2.7-image-pro'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(caps.is_some());
        assert!(defaults.is_some());
    }
}
