//! Storage of the only secret the launcher keeps on disk: the Supabase refresh token.
//!
//! It goes into the operating system's credential store (Windows Credential Manager, macOS
//! Keychain, the Secret Service on Linux), never into SQLite or a plain file. The short-lived
//! access token stays in the frontend's memory.

use crate::{Error, Result};

const SERVICE: &str = "com.coreverse.launcher";
const REFRESH_TOKEN_ACCOUNT: &str = "auth-refresh-token";

/// Where secrets physically live. The real implementation talks to the OS keyring.
pub trait SecretBackend: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<String>>;
    fn set(&self, account: &str, value: &str) -> Result<()>;
    fn delete(&self, account: &str) -> Result<()>;
}

/// The operating system's credential store.
#[derive(Debug, Default, Clone, Copy)]
pub struct KeyringBackend;

impl KeyringBackend {
    fn entry(account: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, account).map_err(secret_error)
    }
}

impl SecretBackend for KeyringBackend {
    fn get(&self, account: &str) -> Result<Option<String>> {
        match Self::entry(account)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(secret_error(error)),
        }
    }

    fn set(&self, account: &str, value: &str) -> Result<()> {
        Self::entry(account)?
            .set_password(value)
            .map_err(secret_error)
    }

    fn delete(&self, account: &str) -> Result<()> {
        match Self::entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(secret_error(error)),
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn secret_error(error: keyring::Error) -> Error {
    Error::SecretStore(error.to_string())
}

/// Loads, saves and clears the Supabase refresh token.
pub struct RefreshTokenStore {
    backend: Box<dyn SecretBackend>,
}

impl RefreshTokenStore {
    #[must_use]
    pub fn new(backend: Box<dyn SecretBackend>) -> Self {
        Self { backend }
    }

    /// A store backed by the operating system's credential store.
    #[must_use]
    pub fn system() -> Self {
        Self::new(Box::new(KeyringBackend))
    }

    pub fn load(&self) -> Result<Option<String>> {
        self.backend.get(REFRESH_TOKEN_ACCOUNT)
    }

    /// Saves the token. An empty token clears the stored one instead.
    pub fn save(&self, token: &str) -> Result<()> {
        if token.is_empty() {
            return self.clear();
        }
        self.backend.set(REFRESH_TOKEN_ACCOUNT, token)
    }

    pub fn clear(&self) -> Result<()> {
        self.backend.delete(REFRESH_TOKEN_ACCOUNT)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Mutex};

    use super::*;

    #[derive(Default)]
    struct MemoryBackend(Mutex<HashMap<String, String>>);

    impl SecretBackend for MemoryBackend {
        fn get(&self, account: &str) -> Result<Option<String>> {
            Ok(self.0.lock().unwrap().get(account).cloned())
        }

        fn set(&self, account: &str, value: &str) -> Result<()> {
            self.0
                .lock()
                .unwrap()
                .insert(account.to_owned(), value.to_owned());
            Ok(())
        }

        fn delete(&self, account: &str) -> Result<()> {
            self.0.lock().unwrap().remove(account);
            Ok(())
        }
    }

    fn store() -> RefreshTokenStore {
        RefreshTokenStore::new(Box::new(MemoryBackend::default()))
    }

    #[test]
    fn nothing_is_stored_initially() {
        assert_eq!(store().load().unwrap(), None);
    }

    #[test]
    fn save_then_load_round_trips() {
        let store = store();
        store.save("token-1").unwrap();
        assert_eq!(store.load().unwrap().as_deref(), Some("token-1"));
        store.save("token-2").unwrap();
        assert_eq!(store.load().unwrap().as_deref(), Some("token-2"));
    }

    #[test]
    fn clear_removes_the_token_and_is_idempotent() {
        let store = store();
        store.save("token").unwrap();
        store.clear().unwrap();
        store.clear().unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn saving_an_empty_token_clears_it() {
        let store = store();
        store.save("token").unwrap();
        store.save("").unwrap();
        assert_eq!(store.load().unwrap(), None);
    }
}
