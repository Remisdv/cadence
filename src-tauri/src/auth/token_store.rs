//! Rangement du jeton GitHub.
//!
//! En production : trousseau de l'OS (Keychain, Credential Manager, Secret Service) via `keyring`.
//! Le jeton n'est jamais écrit sur disque par Cadence, ni dans `config.toml`, ni dans le webview.

use std::sync::Mutex;

use secrecy::SecretString;

use crate::error::{AppError, AppResult};

pub trait TokenStore: Send + Sync {
    fn get(&self) -> AppResult<Option<SecretString>>;
    fn set(&self, token: &str) -> AppResult<()>;
    fn delete(&self) -> AppResult<()>;
}

/// Entrée du trousseau système : service `app.cadence.desktop`, compte `github`.
pub struct KeyringStore {
    service: &'static str,
    account: &'static str,
}

impl KeyringStore {
    pub fn github() -> Self {
        Self { service: super::KEYRING_SERVICE, account: "github" }
    }

    fn entry(&self) -> AppResult<keyring::Entry> {
        keyring::Entry::new(self.service, self.account).map_err(keychain_error)
    }
}

impl TokenStore for KeyringStore {
    fn get(&self) -> AppResult<Option<SecretString>> {
        match self.entry()?.get_password() {
            Ok(token) => Ok(Some(SecretString::from(token))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(keychain_error(err)),
        }
    }

    fn set(&self, token: &str) -> AppResult<()> {
        self.entry()?.set_password(token).map_err(keychain_error)
    }

    fn delete(&self) -> AppResult<()> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(keychain_error(err)),
        }
    }
}

fn keychain_error(err: keyring::Error) -> AppError {
    AppError::Internal(format!("trousseau du système : {err}"))
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
