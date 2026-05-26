//! Keyring abstraction. Production code uses `SystemKeyring`, which delegates
//! to whatever default credential store the host process has registered via
//! `keyring::use_native_store(...)` at startup. Tests inject `InMemoryKeyring`
//! so they can run on platforms without an OS keychain (CI).

use crate::error::{CoreError, Result};

/// Service name used for every entry. Aligns with the Tauri bundle identifier
/// so credentials show up under the right app in OS-level keychain UIs.
pub const SERVICE: &str = "com.mango.app";

/// Build the per-account `user` field stored in the keyring entry. Also
/// written verbatim to `api_account.api_key_ref` for audit / future migration.
pub fn user_id(account_id: &str) -> String {
    format!("api_account:{account_id}")
}

/// spec-16: the OSS access_key_secret rides in a *separate* keyring entry so
/// it can be added/rotated/removed independently of the main API key. Callers
/// should pass the result to `keyring.store(...)` etc. — `SystemKeyring`
/// prepends `api_account:` automatically, so the final entry name is
/// `api_account:<account_id>:oss_secret`.
pub fn oss_secret_storage_id(account_id: &str) -> String {
    format!("{account_id}:oss_secret")
}

pub trait KeyringStore: Send + Sync {
    fn store(&self, account_id: &str, key: &str) -> Result<()>;
    fn fetch(&self, account_id: &str) -> Result<String>;
    fn remove(&self, account_id: &str) -> Result<()>;
}

/// Default production implementation. Relies on a default credential store
/// having been registered (e.g. via `keyring::use_native_store(false)`).
pub struct SystemKeyring;

impl SystemKeyring {
    fn entry(account_id: &str) -> Result<keyring_core::Entry> {
        keyring_core::Entry::new(SERVICE, &user_id(account_id))
            .map_err(|e| CoreError::Keyring(e.to_string()))
    }
}

impl KeyringStore for SystemKeyring {
    fn store(&self, account_id: &str, key: &str) -> Result<()> {
        Self::entry(account_id)?
            .set_password(key)
            .map_err(|e| CoreError::Keyring(e.to_string()))
    }

    fn fetch(&self, account_id: &str) -> Result<String> {
        Self::entry(account_id)?
            .get_password()
            .map_err(|e| CoreError::Keyring(e.to_string()))
    }

    fn remove(&self, account_id: &str) -> Result<()> {
        match Self::entry(account_id)?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(CoreError::Keyring(e.to_string())),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Process-local stand-in for the OS keyring. Used by `account::service`
    /// tests and by other crates' tests that need a `KeyringStore` impl.
    #[derive(Default)]
    pub struct InMemoryKeyring {
        inner: Mutex<HashMap<String, String>>,
    }

    impl KeyringStore for InMemoryKeyring {
        fn store(&self, id: &str, key: &str) -> Result<()> {
            self.inner
                .lock()
                .unwrap()
                .insert(id.to_string(), key.to_string());
            Ok(())
        }

        fn fetch(&self, id: &str) -> Result<String> {
            self.inner
                .lock()
                .unwrap()
                .get(id)
                .cloned()
                .ok_or_else(|| CoreError::Keyring(format!("not found: {id}")))
        }

        fn remove(&self, id: &str) -> Result<()> {
            self.inner.lock().unwrap().remove(id);
            Ok(())
        }
    }

    #[test]
    fn user_id_uses_account_prefix() {
        assert_eq!(user_id("abc"), "api_account:abc");
    }

    #[test]
    fn oss_secret_storage_id_distinct_from_user_id() {
        // The OS keyring entry name expands to `api_account:abc:oss_secret`,
        // i.e. the suffix string passed to SystemKeyring; this guards the
        // contract spec-16 relies on.
        assert_eq!(oss_secret_storage_id("abc"), "abc:oss_secret");
        assert_ne!(user_id("abc"), user_id(&oss_secret_storage_id("abc")));
    }

    #[test]
    fn in_memory_roundtrip() {
        let k = InMemoryKeyring::default();
        k.store("a", "secret").unwrap();
        assert_eq!(k.fetch("a").unwrap(), "secret");
        k.remove("a").unwrap();
        assert!(matches!(k.fetch("a"), Err(CoreError::Keyring(_))));
    }

    #[test]
    fn in_memory_remove_is_idempotent() {
        let k = InMemoryKeyring::default();
        // Removing a never-stored id must not error — matches SystemKeyring's
        // NoEntry-to-Ok mapping so callers can treat both identically.
        k.remove("never-stored").unwrap();
    }
}
