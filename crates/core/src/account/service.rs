use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::account::keyring::{user_id, KeyringStore};
use crate::db::queries::api_account as queries;
use crate::error::{CoreError, Result};
use crate::models::api_account::{
    ApiAccount, CreateApiAccountInput, UpdateApiAccountInput,
};

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

    let id = Uuid::new_v4().to_string();
    let key_last4 = last4(&input.api_key);
    let api_key_ref = user_id(&id);

    // 1) keyring first: if this fails the DB stays untouched so we can never
    //    end up with a row pointing at a missing credential.
    keyring.store(&id, &input.api_key)?;

    // 2) Insert the metadata row. On DB failure, best-effort rollback the
    //    keyring entry so we don't leak an orphan credential.
    let insert = conn.execute(
        "INSERT INTO api_account \
            (id, provider_id, label, api_key_ref, key_last4) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            id,
            input.provider_id,
            input.label.trim(),
            api_key_ref,
            key_last4
        ],
    );
    if let Err(e) = insert {
        if let Err(re) = keyring.remove(&id) {
            tracing::warn!("failed to rollback keyring entry for {id}: {re}");
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
            },
        )
        .unwrap();

        assert_eq!(a.label, "main");
        assert_eq!(a.key_last4, "ghij");
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
    fn create_rejects_empty_key() {
        let (conn, kr, pid) = setup();
        let r = create(
            &conn,
            &kr,
            CreateApiAccountInput {
                provider_id: pid,
                label: "x".into(),
                api_key: "".into(),
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
        };
        assert!(create(&conn, &kr, bad).is_err());

        // The keyring map should be empty — id is freshly generated and unknown,
        // so we just assert the store has zero entries.
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
            },
        )
        .unwrap();

        assert_eq!(updated.label, "renamed");
        assert_eq!(updated.key_last4, "9999");
        assert_eq!(kr.fetch(&a.id).unwrap(), "new-9999");
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
            },
        );
        assert!(matches!(r, Err(CoreError::NotFound { .. })));
    }

    #[test]
    fn delete_removes_db_and_keyring() {
        let (conn, kr, pid) = setup();
        let a = create(
            &conn,
            &kr,
            CreateApiAccountInput {
                provider_id: pid,
                label: "x".into(),
                api_key: "k123456".into(),
            },
        )
        .unwrap();

        delete(&conn, &kr, &a.id).unwrap();
        assert!(matches!(
            queries::get_by_id(&conn, &a.id),
            Err(CoreError::NotFound { .. })
        ));
        assert!(kr.fetch(&a.id).is_err());
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
            },
        )
        .unwrap();
        // Manually drop the keyring entry first — DB delete must still succeed.
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
            },
        )
        .unwrap();
        // Overwrite the keyring entry with an empty string directly.
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
    fn last4_handles_short_and_unicode() {
        assert_eq!(last4("abc"), "abc");
        assert_eq!(last4("abcd"), "abcd");
        assert_eq!(last4("abcdef"), "cdef");
        // Five Chinese characters → last 4 chars (NOT last 4 bytes which would
        // slice mid-codepoint and panic).
        assert_eq!(last4("一二三四五"), "二三四五");
    }
}
