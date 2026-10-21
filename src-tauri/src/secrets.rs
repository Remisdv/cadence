//! Trousseau générique : Keychain (macOS), Credential Manager (Windows), Secret Service (Linux)
//! via `keyring`, sous le service `app.cadence.desktop`.
//!
//! Comptes : `plugin/<id>/<secret>` pour les secrets déclarés par les plugins (plan Cadence 2,
//! § 3.7), `github` pour le jeton de Cadence 1 (repris par le service `repos`). Un secret n'est
//! jamais écrit sur disque ni renvoyé au webview : le front peut seulement l'écrire, savoir
//! s'il existe et le supprimer.

use std::{collections::HashMap, sync::Mutex};

use secrecy::SecretString;

use crate::error::{AppError, AppResult};

/// Service sous lequel Cadence range ses entrées.
pub const SERVICE: &str = "app.cadence.desktop";

/// Service effectif : `CADENCE_SECRETS_SERVICE` le remplace pour une instance de test isolée
/// (skill `tester-cadence`), afin qu'elle ne lise ni n'écrive jamais les secrets de l'utilisateur.
pub fn service() -> String {
    std::env::var("CADENCE_SECRETS_SERVICE").ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| SERVICE.to_string())
}

pub trait SecretStore: Send + Sync {
    fn get(&self, account: &str) -> AppResult<Option<SecretString>>;
    fn set(&self, account: &str, value: &str) -> AppResult<()>;
    /// Supprime l'entrée ; une entrée absente n'est pas une erreur.
    fn delete(&self, account: &str) -> AppResult<()>;

    fn has(&self, account: &str) -> AppResult<bool> {
        Ok(self.get(account)?.is_some())
    }
}

/// Compte d'un secret de plugin, après vérification des deux identifiants.
pub fn plugin_account(plugin: &str, secret: &str) -> AppResult<String> {
    if !crate::plugins::manifest::is_plugin_id(plugin) {
        return Err(AppError::Validation(format!("« {plugin} » n'est pas un identifiant de plugin.")));
    }
    if !is_secret_id(secret) {
        return Err(AppError::Validation(format!("« {secret} » n'est pas un identifiant de secret.")));
    }
    Ok(format!("plugin/{plugin}/{secret}"))
}

/// `[a-z0-9_]+`, 64 caractères au plus (devient `CADENCE_SECRET_<ID>`).
pub fn is_secret_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Trousseau du système.
#[derive(Default)]
pub struct KeyringSecrets;

impl KeyringSecrets {
    fn entry(account: &str) -> AppResult<keyring::Entry> {
        keyring::Entry::new(&service(), account).map_err(keychain_error)
    }
}

impl SecretStore for KeyringSecrets {
    fn get(&self, account: &str) -> AppResult<Option<SecretString>> {
        match Self::entry(account)?.get_password() {
            Ok(value) => Ok(Some(SecretString::from(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(keychain_error(err)),
        }
    }

    fn set(&self, account: &str, value: &str) -> AppResult<()> {
        Self::entry(account)?.set_password(value).map_err(keychain_error)
    }

    fn delete(&self, account: &str) -> AppResult<()> {
        match Self::entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(keychain_error(err)),
        }
    }
}

fn keychain_error(err: keyring::Error) -> AppError {
    AppError::Internal(format!("trousseau du système : {err}"))
}

/// Trousseau en mémoire, pour les tests.
#[derive(Default)]
pub struct MemorySecrets(Mutex<HashMap<String, String>>);

impl SecretStore for MemorySecrets {
    fn get(&self, account: &str) -> AppResult<Option<SecretString>> {
        Ok(self.0.lock().unwrap().get(account).cloned().map(SecretString::from))
    }

    fn set(&self, account: &str, value: &str) -> AppResult<()> {
        self.0.lock().unwrap().insert(account.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&self, account: &str) -> AppResult<()> {
        self.0.lock().unwrap().remove(account);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use secrecy::ExposeSecret;

    use super::*;

    #[test]
    fn names_plugin_accounts_and_rejects_odd_ids() {
        assert_eq!(plugin_account("gestion-projet", "github_token").unwrap(), "plugin/gestion-projet/github_token");
        assert!(plugin_account("../x", "token").is_err());
        assert!(plugin_account("gestion-projet", "Token").is_err());
        assert!(plugin_account("gestion-projet", "").is_err());
        assert!(plugin_account("gestion-projet", "a/b").is_err());
    }

    #[test]
    fn memory_store_round_trips() {
        let store = MemorySecrets::default();
        let account = plugin_account("documents", "api_key").unwrap();
        assert!(!store.has(&account).unwrap());
        store.set(&account, "s3cret").unwrap();
        assert_eq!(store.get(&account).unwrap().unwrap().expose_secret(), "s3cret");
        store.delete(&account).unwrap();
        store.delete(&account).unwrap();
        assert!(!store.has(&account).unwrap());
    }
}
