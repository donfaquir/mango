use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::account::keyring::{oss_secret_storage_id, user_id, KeyringStore};
use crate::db::queries::api_account as queries;
use crate::error::{CoreError, Result};
use crate::models::api_account::{
    ApiAccount, CreateApiAccountInput, OssConfigInput, UpdateApiAccountInput,
};
use crate::provider::traits::ProviderCredentials;

pub fn create(
    conn: &Connection,
    keyring: &dyn KeyringStore,
    input: CreateApiAccountInput,
) -> Result<ApiAccount> {
    if input.api_key.is_empty() {
        return Err(CoreError::Validation("api_key cannot be empty".into()));
    }
    if input.label.trim().is_empty() {
        return Err(CoreError::Validation("label cannot be empty".into()));
    }
    if let Some(oss) = input.oss.as_ref() {
        validate_oss(oss)?;
    }

    let id = Uuid::new_v4().to_string();
    let key_last4 = last4(&input.api_key);
    let api_key_ref = user_id(&id);

    // 1) keyring first: if this fails the DB stays untouched so we can never
    //    end up with a row pointing at a missing credential.
    keyring.store(&id, &input.api_key)?;

    // 1b) OSS secret, if any. On failure we roll back the main key entry so
    //     the resulting state has neither a half-written keyring nor a DB row.
    if let Some(oss) = input.oss.as_ref()
        && let Err(e) = keyring.store(&oss_secret_storage_id(&id), &oss.access_key_secret)
    {
        if let Err(re) = keyring.remove(&id) {
            tracing::warn!("failed to rollback main keyring entry for {id}: {re}");
        }
        return Err(e);
    }

    // 2) Insert the metadata row (including params_json if oss was given). On
    //    DB failure, best-effort rollback BOTH keyring entries we may have
    //    written.
    let params_json = match input.oss.as_ref() {
        Some(oss) => oss_params_json(oss),
        None => "{}".to_string(),
    };
    let insert = conn.execute(
        "INSERT INTO api_account \
            (id, provider_id, label, api_key_ref, key_last4, params_json) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id,
            input.provider_id,
            input.label.trim(),
            api_key_ref,
            key_last4,
            params_json
        ],
    );
    if let Err(e) = insert {
        if let Err(re) = keyring.remove(&id) {
            tracing::warn!("failed to rollback keyring entry for {id}: {re}");
        }
        if input.oss.is_some()
            && let Err(re) = keyring.remove(&oss_secret_storage_id(&id))
        {
            tracing::warn!("failed to rollback oss keyring entry for {id}: {re}");
        }
        return Err(e.into());
    }

    queries::get_by_id(conn, &id)
}

pub fn delete(conn: &Connection, keyring: &dyn KeyringStore, id: &str) -> Result<()> {
    let n = conn.execute("DELETE FROM api_account WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(CoreError::NotFound {
            entity: "api_account",
            id: id.to_string(),
        });
    }
    if let Err(e) = keyring.remove(id) {
        // Don't bubble up: the metadata row is already gone and the user-facing
        // operation succeeded. Leave a trace so we can spot keyring drift.
        tracing::warn!("keyring entry orphaned for {id}: {e}");
    }
    // best-effort: old (pre-spec-16) accounts won't have an OSS entry, so
    // a "missing" remove is expected and silenced.
    if let Err(e) = keyring.remove(&oss_secret_storage_id(id)) {
        tracing::debug!("oss secret entry not present for {id}: {e}");
    }
    Ok(())
}

pub fn update(
    conn: &Connection,
    keyring: &dyn KeyringStore,
    id: &str,
    input: UpdateApiAccountInput,
) -> Result<ApiAccount> {
    // Touching a non-existent id should NOT silently create a keyring entry.
    let _existing = queries::get_by_id(conn, id)?;

    if let Some(new_key) = input.api_key {
        if new_key.is_empty() {
            return Err(CoreError::Validation("api_key cannot be empty".into()));
        }
        keyring.store(id, &new_key)?;
        let last4 = last4(&new_key);
        conn.execute(
            "UPDATE api_account SET key_last4 = ?1 WHERE id = ?2",
            params![last4, id],
        )?;
    }
    if let Some(label) = input.label {
        let trimmed = label.trim();
        if trimmed.is_empty() {
            return Err(CoreError::Validation("label cannot be empty".into()));
        }
        conn.execute(
            "UPDATE api_account SET label = ?1 WHERE id = ?2",
            params![trimmed, id],
        )?;
    }
    if let Some(oss) = input.oss {
        validate_oss(&oss)?;
        keyring.store(&oss_secret_storage_id(id), &oss.access_key_secret)?;
        conn.execute(
            "UPDATE api_account SET params_json = ?1 WHERE id = ?2",
            params![oss_params_json(&oss), id],
        )?;
    }
    queries::get_by_id(conn, id)
}

/// Verify the keyring still holds a non-empty credential for this account.
/// Deliberately named `verify_storage` rather than `test_connection`: MS2 will
/// add a real network ping under the latter name, and we don't want them to
/// share an IPC method name.
pub fn verify_storage(
    conn: &Connection,
    keyring: &dyn KeyringStore,
    id: &str,
) -> Result<()> {
    let _ = queries::get_by_id(conn, id)?;
    let key = keyring.fetch(id)?;
    if key.is_empty() {
        return Err(CoreError::Validation("stored api key is empty".into()));
    }
    Ok(())
}

/// Assemble the full credential bundle for the task_engine's per-task runner.
/// `extra_json` is `Some(_)` exactly when the account has an `oss` block, with
/// the access_key_secret injected back from keyring. The runner is expected to
/// drop the resulting value as soon as it's been handed to the provider — the
/// secret never lands in DB or logs.
pub fn resolve_credentials(
    conn: &Connection,
    keyring: &dyn KeyringStore,
    account_id: &str,
) -> Result<ProviderCredentials> {
    let _exists = queries::get_by_id(conn, account_id)?;
    let api_key = keyring.fetch(account_id)?;
    let params_json = queries::get_params_json(conn, account_id)?;

    let extra_json = if params_json.is_empty() || params_json == "{}" {
        None
    } else {
        let mut params: serde_json::Value = serde_json::from_str(&params_json)
            .map_err(|e| CoreError::Validation(format!("api_account.params_json: {e}")))?;
        if let Some(oss) = params.get_mut("oss") {
            let secret = keyring.fetch(&oss_secret_storage_id(account_id))?;
            if let Some(obj) = oss.as_object_mut() {
                obj.insert(
                    "access_key_secret".into(),
                    serde_json::Value::String(secret),
                );
            }
        }
        Some(params.to_string())
    };

    Ok(ProviderCredentials { api_key, extra_json })
}

/// Reject obviously-invalid OSS configs at the boundary. `oss-<region>.aliyuncs.com`
/// is enforced because the SDK and the manual presigned URL signer both rely
/// on that shape to derive the request host.
fn validate_oss(oss: &OssConfigInput) -> Result<()> {
    if oss.endpoint.is_empty() {
        return Err(CoreError::Validation("oss.endpoint required".into()));
    }
    if !oss.endpoint.starts_with("oss-") || !oss.endpoint.ends_with(".aliyuncs.com") {
        return Err(CoreError::Validation(
            "oss.endpoint must look like 'oss-<region>.aliyuncs.com'".into(),
        ));
    }
    if oss.bucket.is_empty() {
        return Err(CoreError::Validation("oss.bucket required".into()));
    }
    if oss.access_key_id.is_empty() || oss.access_key_secret.is_empty() {
        return Err(CoreError::Validation("oss credentials required".into()));
    }
    if oss.url_expires_seconds < 60 || oss.url_expires_seconds > 86400 {
        return Err(CoreError::Validation(
            "oss.url_expires_seconds out of [60, 86400]".into(),
        ));
    }
    Ok(())
}

/// Render the OSS half of `params_json` — secret intentionally omitted.
fn oss_params_json(oss: &OssConfigInput) -> String {
    serde_json::json!({
        "oss": {
            "endpoint": oss.endpoint,
            "bucket": oss.bucket,
            "access_key_id": oss.access_key_id,
            "region": oss.region,
            "url_expires_seconds": oss.url_expires_seconds,
        }
    })
    .to_string()
}

/// Take the last 4 *characters* (not bytes) of a secret so the result is
/// always a valid UTF-8 slice even when the key contains multi-byte chars.
fn last4(s: &str) -> String {
    let n = s.chars().count();
    if n <= 4 {
        s.to_string()
    } else {
        s.chars().skip(n - 4).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::keyring::tests::InMemoryKeyring;
    use crate::db::open_sync;
    use std::path::Path;

    fn setup() -> (Connection, InMemoryKeyring, String) {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        conn.execute(
            "INSERT INTO provider (id, name) VALUES ('p1', 'Provider1')",
            [],
        )
        .unwrap();
        (conn, InMemoryKeyring::default(), "p1".to_string())
    }

    fn sample_oss() -> OssConfigInput {
        OssConfigInput {
            endpoint: "oss-cn-hangzhou.aliyuncs.com".into(),
            bucket: "mango-test".into(),
            access_key_id: "LTAI5tFAKE".into(),
            access_key_secret: "secret-zzzz".into(),
            region: Some("cn-hangzhou".into()),
            url_expires_seconds: 3600,
        }
    }

    #[test]
    fn create_writes_keyring_and_db() {
        let (conn, kr, pid) = setup();
        let a = create(
            &conn,
            &kr,
            CreateApiAccountInput {
                provider_id: pid,
                label: "main".into(),
                api_key: "sk-abcdefghij".into(),
                oss: None,
            },
        )
        .unwrap();

        assert_eq!(a.label, "main");
        assert_eq!(a.key_last4, "ghij");
        assert!(a.oss_config.is_none());
        assert_eq!(kr.fetch(&a.id).unwrap(), "sk-abcdefghij");

        // api_key_ref column is set even though it's never returned via ApiAccount.
        let api_key_ref: String = conn
            .query_row(
                "SELECT api_key_ref FROM api_account WHERE id = ?1",
                params![a.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(api_key_ref, format!("api_account:{}", a.id));
    }

    #[test]
    fn create_with_oss_writes_two_keyring_entries() {
        let (conn, kr, pid) = setup();
        let a = create(
            &conn,
            &kr,
            CreateApiAccountInput {
                provider_id: pid,
                label: "main".into(),
                api_key: "sk-abcdefghij".into(),
                oss: Some(sample_oss()),
            },
        )
        .unwrap();

        // Both entries present in keyring.
        assert_eq!(kr.fetch(&a.id).unwrap(), "sk-abcdefghij");
        assert_eq!(
            kr.fetch(&oss_secret_storage_id(&a.id)).unwrap(),
            "secret-zzzz"
        );

        // params_json must NOT contain the secret.
        let raw: String = conn
            .query_row(
                "SELECT params_json FROM api_account WHERE id = ?1",
                params![a.id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(raw.contains("oss-cn-hangzhou"));
        assert!(!raw.contains("secret-zzzz"));

        // Public projection contains AK ID, never secret.
        let oss = a.oss_config.expect("oss_config");
        assert_eq!(oss.access_key_id, "LTAI5tFAKE");
    }

    #[test]
    fn create_rejects_empty_key() {
        let (conn, kr, pid) = setup();
        let r = create(
            &conn,
            &kr,
            CreateApiAccountInput {
                provider_id: pid,
                label: "x".into(),
                api_key: "".into(),
                oss: None,
            },
        );
        assert!(matches!(r, Err(CoreError::Validation(_))));
    }

    #[test]
    fn create_rejects_blank_label() {
        let (conn, kr, pid) = setup();
        let r = create(
            &conn,
            &kr,
            CreateApiAccountInput {
                provider_id: pid,
                label: "   ".into(),
                api_key: "k".into(),
                oss: None,
            },
        );
        assert!(matches!(r, Err(CoreError::Validation(_))));
    }

    #[test]
    fn create_rejects_bad_oss_endpoint() {
        let (conn, kr, pid) = setup();
        let mut bad = sample_oss();
        bad.endpoint = "https://example.com".into();
        let r = create(
            &conn,
            &kr,
            CreateApiAccountInput {
                provider_id: pid,
                label: "x".into(),
                api_key: "k".into(),
                oss: Some(bad),
            },
        );
        assert!(matches!(r, Err(CoreError::Validation(_))));
    }

    #[test]
    fn create_db_failure_rolls_back_keyring() {
        // Use an unknown provider_id so the FK violation triggers a DB error
        // path after the keyring entry has been written.
        let (conn, kr, _pid) = setup();
        let bad = CreateApiAccountInput {
            provider_id: "no-such-provider".into(),
            label: "x".into(),
            api_key: "k123456".into(),
            oss: Some(sample_oss()),
        };
        assert!(create(&conn, &kr, bad).is_err());

        // No DB row, no orphan keyring entries.
        assert_eq!(
            conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM api_account", [], |r| r.get(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn update_replaces_key_and_last4() {
        let (conn, kr, pid) = setup();
        let a = create(
            &conn,
            &kr,
            CreateApiAccountInput {
                provider_id: pid,
                label: "x".into(),
                api_key: "old-1111".into(),
                oss: None,
            },
        )
        .unwrap();

        let updated = update(
            &conn,
            &kr,
            &a.id,
            UpdateApiAccountInput {
                label: Some("renamed".into()),
                api_key: Some("new-9999".into()),
                oss: None,
            },
        )
        .unwrap();

        assert_eq!(updated.label, "renamed");
        assert_eq!(updated.key_last4, "9999");
        assert_eq!(kr.fetch(&a.id).unwrap(), "new-9999");
    }

    #[test]
    fn update_with_oss_writes_secret_and_params() {
        let (conn, kr, pid) = setup();
        let a = create(
            &conn,
            &kr,
            CreateApiAccountInput {
                provider_id: pid,
                label: "x".into(),
                api_key: "k".into(),
                oss: None,
            },
        )
        .unwrap();
        assert!(a.oss_config.is_none());

        let updated = update(
            &conn,
            &kr,
            &a.id,
            UpdateApiAccountInput {
                label: None,
                api_key: None,
                oss: Some(sample_oss()),
            },
        )
        .unwrap();
        assert!(updated.oss_config.is_some());
        assert_eq!(
            kr.fetch(&oss_secret_storage_id(&a.id)).unwrap(),
            "secret-zzzz"
        );
    }

    #[test]
    fn update_unknown_id_returns_not_found() {
        let (conn, kr, _pid) = setup();
        let r = update(
            &conn,
            &kr,
            "no-such",
            UpdateApiAccountInput {
                label: Some("x".into()),
                api_key: None,
                oss: None,
            },
        );
        assert!(matches!(r, Err(CoreError::NotFound { .. })));
    }

    #[test]
    fn delete_removes_db_and_both_keyring_entries() {
        let (conn, kr, pid) = setup();
        let a = create(
            &conn,
            &kr,
            CreateApiAccountInput {
                provider_id: pid,
                label: "x".into(),
                api_key: "k123456".into(),
                oss: Some(sample_oss()),
            },
        )
        .unwrap();

        delete(&conn, &kr, &a.id).unwrap();
        assert!(matches!(
            queries::get_by_id(&conn, &a.id),
            Err(CoreError::NotFound { .. })
        ));
        assert!(kr.fetch(&a.id).is_err());
        assert!(kr.fetch(&oss_secret_storage_id(&a.id)).is_err());
    }

    #[test]
    fn delete_is_idempotent_when_keyring_missing() {
        let (conn, kr, pid) = setup();
        let a = create(
            &conn,
            &kr,
            CreateApiAccountInput {
                provider_id: pid,
                label: "x".into(),
                api_key: "k".into(),
                oss: None,
            },
        )
        .unwrap();
        // Manually drop the keyring entry first — DB delete must still succeed,
        // and the OSS-secret remove (which never existed) is silenced too.
        kr.remove(&a.id).unwrap();
        delete(&conn, &kr, &a.id).unwrap();
    }

    #[test]
    fn delete_unknown_id_returns_not_found() {
        let (conn, kr, _pid) = setup();
        assert!(matches!(
            delete(&conn, &kr, "no-such"),
            Err(CoreError::NotFound { .. })
        ));
    }

    #[test]
    fn verify_storage_succeeds_for_existing() {
        let (conn, kr, pid) = setup();
        let a = create(
            &conn,
            &kr,
            CreateApiAccountInput {
                provider_id: pid,
                label: "x".into(),
                api_key: "k".into(),
                oss: None,
            },
        )
        .unwrap();
        verify_storage(&conn, &kr, &a.id).unwrap();
    }

    #[test]
    fn verify_storage_fails_on_empty_key() {
        let (conn, kr, pid) = setup();
        let a = create(
            &conn,
            &kr,
            CreateApiAccountInput {
                provider_id: pid,
                label: "x".into(),
                api_key: "k".into(),
                oss: None,
            },
        )
        .unwrap();
        kr.store(&a.id, "").unwrap();
        let r = verify_storage(&conn, &kr, &a.id);
        assert!(matches!(r, Err(CoreError::Validation(_))));
    }

    #[test]
    fn verify_storage_unknown_id_returns_not_found() {
        let (conn, kr, _pid) = setup();
        assert!(matches!(
            verify_storage(&conn, &kr, "missing"),
            Err(CoreError::NotFound { .. })
        ));
    }

    #[test]
    fn resolve_credentials_returns_none_extra_when_no_oss() {
        let (conn, kr, pid) = setup();
        let a = create(
            &conn,
            &kr,
            CreateApiAccountInput {
                provider_id: pid,
                label: "x".into(),
                api_key: "k".into(),
                oss: None,
            },
        )
        .unwrap();
        let creds = resolve_credentials(&conn, &kr, &a.id).unwrap();
        assert_eq!(creds.api_key, "k");
        assert!(creds.extra_json.is_none());
    }

    #[test]
    fn resolve_credentials_injects_secret_into_extra_json() {
        let (conn, kr, pid) = setup();
        let a = create(
            &conn,
            &kr,
            CreateApiAccountInput {
                provider_id: pid,
                label: "x".into(),
                api_key: "k".into(),
                oss: Some(sample_oss()),
            },
        )
        .unwrap();
        let creds = resolve_credentials(&conn, &kr, &a.id).unwrap();
        let extra = creds.extra_json.expect("extra_json present");

        // The secret is materialized only here; the underlying DB row must NOT
        // contain it (asserted in `create_with_oss_writes_two_keyring_entries`).
        let v: serde_json::Value = serde_json::from_str(&extra).unwrap();
        assert_eq!(
            v.pointer("/oss/access_key_secret").and_then(|s| s.as_str()),
            Some("secret-zzzz")
        );
        assert_eq!(
            v.pointer("/oss/access_key_id").and_then(|s| s.as_str()),
            Some("LTAI5tFAKE")
        );
    }

    #[test]
    fn last4_handles_short_and_unicode() {
        assert_eq!(last4("abc"), "abc");
        assert_eq!(last4("abcd"), "abcd");
        assert_eq!(last4("abcdef"), "cdef");
        // Five Chinese characters → last 4 chars (NOT last 4 bytes which would
        // slice mid-codepoint and panic).
        assert_eq!(last4("一二三四五"), "二三四五");
    }
}
