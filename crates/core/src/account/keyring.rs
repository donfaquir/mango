//! Keyring abstraction. Production code uses `SystemKeyring`, which delegates
//! to whatever default credential store the host process has registered via
//! `keyring::use_native_store(...)` at startup. Tests inject `InMemoryKeyring`
//! so they can run on platforms without an OS keychain (CI).

use std::collections::HashMap;
use std::sync::RwLock;

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

// ---------------------------------------------------------------------------
// CachedKeyringStore — session-level in-memory cache to reduce OS keychain
// popup frequency on macOS.
// ---------------------------------------------------------------------------

/// Internal cache state for a single account entry.
enum CacheEntry {
    /// The secret value is known to exist.
    Present(String),
    /// The entry is known to be absent (avoids repeated queries that trigger popups).
    Absent,
}

/// A session-level caching wrapper around any [`KeyringStore`] implementation.
///
/// On macOS, every call to the system keychain may trigger a password dialog.
/// `CachedKeyringStore` keeps results in memory so repeated accesses within
/// the same app session avoid redundant system calls.
pub struct CachedKeyringStore {
    inner: Box<dyn KeyringStore>,
    cache: RwLock<HashMap<String, CacheEntry>>,
}

impl CachedKeyringStore {
    /// Wrap an existing [`KeyringStore`] with an in-memory cache layer.
    pub fn new(inner: Box<dyn KeyringStore>) -> Self {
        Self {
            inner,
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Remove a single entry from the cache. Useful when external state changes
    /// (e.g. a credential rotation performed outside this process) invalidate
    /// our cached knowledge.
    pub fn invalidate(&self, account_id: &str) {
        self.cache.write().unwrap().remove(account_id);
    }

    /// Clear the entire cache. All subsequent operations will hit the
    /// underlying store again.
    pub fn clear_cache(&self) {
        self.cache.write().unwrap().clear();
    }
}

/// Returns `true` if the error represents a "not found" condition in the
/// keyring (as opposed to a transient/auth failure that should be retried).
fn is_keyring_not_found(err: &CoreError) -> bool {
    match err {
        CoreError::Keyring(msg) => {
            let lower = msg.to_lowercase();
            lower.contains("no entry") || lower.contains("not found")
        }
        _ => false,
    }
}

impl KeyringStore for CachedKeyringStore {
    fn fetch(&self, account_id: &str) -> Result<String> {
        // Check cache first (read lock).
        {
            let cache = self.cache.read().unwrap();
            if let Some(entry) = cache.get(account_id) {
                return match entry {
                    CacheEntry::Present(val) => Ok(val.clone()),
                    CacheEntry::Absent => {
                        Err(CoreError::Keyring(format!("not found: {account_id}")))
                    }
                };
            }
        }

        // Cache miss — query the underlying store.
        match self.inner.fetch(account_id) {
            Ok(val) => {
                self.cache
                    .write()
                    .unwrap()
                    .insert(account_id.to_string(), CacheEntry::Present(val.clone()));
                Ok(val)
            }
            Err(e) if is_keyring_not_found(&e) => {
                self.cache
                    .write()
                    .unwrap()
                    .insert(account_id.to_string(), CacheEntry::Absent);
                Err(e)
            }
            Err(e) => {
                // Transient / auth errors are NOT cached so the user can retry.
                Err(e)
            }
        }
    }

    fn store(&self, account_id: &str, secret: &str) -> Result<()> {
        self.inner.store(account_id, secret)?;
        self.cache
            .write()
            .unwrap()
            .insert(account_id.to_string(), CacheEntry::Present(secret.to_string()));
        Ok(())
    }

    fn remove(&self, account_id: &str) -> Result<()> {
        // If already known absent, skip the system call entirely.
        {
            let cache = self.cache.read().unwrap();
            if let Some(CacheEntry::Absent) = cache.get(account_id) {
                return Ok(());
            }
        }

        match self.inner.remove(account_id) {
            Ok(()) => {
                self.cache
                    .write()
                    .unwrap()
                    .insert(account_id.to_string(), CacheEntry::Absent);
                Ok(())
            }
            Err(e) if is_keyring_not_found(&e) => {
                // Entry was already gone in the underlying store.
                self.cache
                    .write()
                    .unwrap()
                    .insert(account_id.to_string(), CacheEntry::Absent);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

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

    /// Shared-state mock allowing tests to (a) mutate the underlying store
    /// behind the cache and (b) count how many times each method was hit.
    #[derive(Clone, Default)]
    struct SharedMockKeyring {
        data: Arc<Mutex<HashMap<String, String>>>,
        fetch_calls: Arc<AtomicUsize>,
        store_calls: Arc<AtomicUsize>,
        remove_calls: Arc<AtomicUsize>,
    }

    impl SharedMockKeyring {
        fn set_raw(&self, id: &str, key: &str) {
            self.data
                .lock()
                .unwrap()
                .insert(id.to_string(), key.to_string());
        }

        fn remove_raw(&self, id: &str) {
            self.data.lock().unwrap().remove(id);
        }
    }

    impl KeyringStore for SharedMockKeyring {
        fn store(&self, id: &str, key: &str) -> Result<()> {
            self.store_calls.fetch_add(1, Ordering::SeqCst);
            self.data
                .lock()
                .unwrap()
                .insert(id.to_string(), key.to_string());
            Ok(())
        }

        fn fetch(&self, id: &str) -> Result<String> {
            self.fetch_calls.fetch_add(1, Ordering::SeqCst);
            self.data
                .lock()
                .unwrap()
                .get(id)
                .cloned()
                .ok_or_else(|| CoreError::Keyring(format!("not found: {id}")))
        }

        fn remove(&self, id: &str) -> Result<()> {
            self.remove_calls.fetch_add(1, Ordering::SeqCst);
            self.data.lock().unwrap().remove(id);
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

    // ---------------- CachedKeyringStore tests ----------------

    #[test]
    fn cached_fetch_hits_cache_and_skips_inner() {
        let mock = SharedMockKeyring::default();
        let cache = CachedKeyringStore::new(Box::new(mock.clone()));

        // store populates the cache without needing an inner fetch.
        cache.store("acc1", "secret1").unwrap();
        assert_eq!(mock.store_calls.load(Ordering::SeqCst), 1);

        // Wipe the inner state — if the cache works, fetch must not consult inner.
        mock.remove_raw("acc1");

        let val = cache.fetch("acc1").unwrap();
        assert_eq!(val, "secret1");
        // Cache hit: inner.fetch must not have been invoked.
        assert_eq!(mock.fetch_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn cached_fetch_absent_skips_subsequent_inner_calls() {
        let mock = SharedMockKeyring::default();
        let cache = CachedKeyringStore::new(Box::new(mock.clone()));

        // First fetch: miss → inner returns NotFound → cached as Absent.
        let err = cache.fetch("missing").unwrap_err();
        assert!(is_keyring_not_found(&err));
        assert_eq!(mock.fetch_calls.load(Ordering::SeqCst), 1);

        // Mutate inner state directly behind the cache's back.
        mock.set_raw("missing", "now-present");

        // Second fetch: must still hit the Absent cache, not inner.
        let err = cache.fetch("missing").unwrap_err();
        assert!(is_keyring_not_found(&err));
        assert_eq!(mock.fetch_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn cached_store_then_fetch_returns_cached_value() {
        let mock = SharedMockKeyring::default();
        let cache = CachedKeyringStore::new(Box::new(mock.clone()));

        cache.store("acc1", "secret1").unwrap();
        let val = cache.fetch("acc1").unwrap();
        assert_eq!(val, "secret1");
        // store is delegated to inner exactly once; fetch should never hit it.
        assert_eq!(mock.store_calls.load(Ordering::SeqCst), 1);
        assert_eq!(mock.fetch_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn cached_remove_skips_inner_for_absent_entries() {
        let mock = SharedMockKeyring::default();
        let cache = CachedKeyringStore::new(Box::new(mock.clone()));

        // Cause the cache to mark this key as Absent.
        let _ = cache.fetch("ghost").unwrap_err();
        assert_eq!(mock.fetch_calls.load(Ordering::SeqCst), 1);

        // remove on an Absent entry should be a no-op against inner.
        cache.remove("ghost").unwrap();
        assert_eq!(mock.remove_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn cached_remove_existing_then_fetch_returns_not_found() {
        let mock = SharedMockKeyring::default();
        let cache = CachedKeyringStore::new(Box::new(mock.clone()));

        cache.store("acc1", "secret1").unwrap();
        cache.remove("acc1").unwrap();
        assert_eq!(mock.remove_calls.load(Ordering::SeqCst), 1);

        let err = cache.fetch("acc1").unwrap_err();
        assert!(is_keyring_not_found(&err));
        // After remove, cache holds Absent → fetch must not hit inner.
        assert_eq!(mock.fetch_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn cached_invalidate_clears_single_entry() {
        let mock = SharedMockKeyring::default();
        let cache = CachedKeyringStore::new(Box::new(mock.clone()));

        cache.store("acc1", "secret1").unwrap();

        // Invalidate the cache, then mutate the underlying store.
        cache.invalidate("acc1");
        mock.set_raw("acc1", "rotated");

        // Next fetch must reload from inner and observe the rotated value.
        let val = cache.fetch("acc1").unwrap();
        assert_eq!(val, "rotated");
        assert_eq!(mock.fetch_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn cached_clear_cache_drops_all_entries() {
        let mock = SharedMockKeyring::default();
        let cache = CachedKeyringStore::new(Box::new(mock.clone()));

        cache.store("acc1", "s1").unwrap();
        cache.store("acc2", "s2").unwrap();

        cache.clear_cache();

        // Both subsequent fetches must consult inner.
        assert_eq!(cache.fetch("acc1").unwrap(), "s1");
        assert_eq!(cache.fetch("acc2").unwrap(), "s2");
        assert_eq!(mock.fetch_calls.load(Ordering::SeqCst), 2);
    }
}
