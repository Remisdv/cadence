//! Rangement du jeton GitHub de Cadence 1 : compte `github` du trousseau générique
//! (`crate::secrets`). Le service `repos` de Cadence 2 le reprendra.
//!
//! Le jeton n'est jamais écrit sur disque par Cadence, ni dans `config.toml`, ni dans le webview.

use std::sync::{Arc, Mutex};

use secrecy::SecretString;

use crate::{error::AppResult, secrets::SecretStore};

pub trait TokenStore: Send + Sync {
    fn get(&self) -> AppResult<Option<SecretString>>;
    fn set(&self, token: &str) -> AppResult<()>;
    fn delete(&self) -> AppResult<()>;
}

/// Une entrée du trousseau de Cadence.
pub struct SecretEntry {
    secrets: Arc<dyn SecretStore>,
    account: &'static str,
}

impl SecretEntry {
    /// Compte `github` (service `app.cadence.desktop`).
    pub fn github(secrets: Arc<dyn SecretStore>) -> Self {
        Self { secrets, account: "github" }
    }
}

impl TokenStore for SecretEntry {
    fn get(&self) -> AppResult<Option<SecretString>> {
        self.secrets.get(self.account)
    }

    fn set(&self, token: &str) -> AppResult<()> {
        self.secrets.set(self.account, token)
    }

    fn delete(&self) -> AppResult<()> {
        self.secrets.delete(self.account)
    }
}

/// Rangement en mémoire, pour les tests.
#[derive(Default)]
pub struct MemoryStore(Mutex<Option<String>>);

impl TokenStore for MemoryStore {
    fn get(&self) -> AppResult<Option<SecretString>> {
        Ok(self.0.lock().unwrap().clone().map(SecretString::from))
    }

    fn set(&self, token: &str) -> AppResult<()> {
        *self.0.lock().unwrap() = Some(token.to_string());
        Ok(())
    }

    fn delete(&self) -> AppResult<()> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}
