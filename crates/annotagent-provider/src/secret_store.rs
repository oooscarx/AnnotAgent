//! Secret-store implementations behind Core credential references.

use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
};

use annotagent_core::{
    CredentialReference, CredentialSource, SecretScope, SecretStore, SecretStoreError, SecretValue,
};
use async_trait::async_trait;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyringBackendError;

pub trait KeyringBackend: Send + Sync {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, KeyringBackendError>;
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), KeyringBackendError>;
    fn delete(&self, service: &str, account: &str) -> Result<(), KeyringBackendError>;
}

#[derive(Debug, Default)]
pub struct SystemKeyringBackend;

impl KeyringBackend for SystemKeyringBackend {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, KeyringBackendError> {
        let entry = keyring::Entry::new(service, account).map_err(|_| KeyringBackendError)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(KeyringBackendError),
        }
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), KeyringBackendError> {
        keyring::Entry::new(service, account)
            .map_err(|_| KeyringBackendError)?
            .set_password(secret)
            .map_err(|_| KeyringBackendError)
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), KeyringBackendError> {
        let entry = keyring::Entry::new(service, account).map_err(|_| KeyringBackendError)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(KeyringBackendError),
        }
    }
}

pub struct KeyringSecretStore {
    service: String,
    backend: Arc<dyn KeyringBackend>,
}

impl KeyringSecretStore {
    #[must_use]
    pub fn new(service: impl Into<String>) -> Self {
        Self::with_backend(service, Arc::new(SystemKeyringBackend))
    }

    #[must_use]
    pub fn with_backend(service: impl Into<String>, backend: Arc<dyn KeyringBackend>) -> Self {
        Self {
            service: service.into(),
            backend,
        }
    }

    fn ensure_source(reference: &CredentialReference) -> Result<(), SecretStoreError> {
        reference.validate()?;
        if reference.source != CredentialSource::SystemKeyring {
            return Err(SecretStoreError::InvalidReference(
                "system Keyring store requires a system_keyring reference".to_owned(),
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl SecretStore for KeyringSecretStore {
    async fn put(
        &self,
        scope: SecretScope,
        secret: SecretValue,
    ) -> Result<CredentialReference, SecretStoreError> {
        let reference = scope.reference();
        Self::ensure_source(&reference)?;
        self.backend
            .set(&self.service, &reference.locator, secret.expose_secret())
            .map_err(|_| {
                SecretStoreError::Unavailable(
                    "system credential store rejected the save operation".to_owned(),
                )
            })?;
        Ok(reference)
    }

    async fn resolve(
        &self,
        reference: &CredentialReference,
    ) -> Result<SecretValue, SecretStoreError> {
        Self::ensure_source(reference)?;
        let value = self
            .backend
            .get(&self.service, &reference.locator)
            .map_err(|_| {
                SecretStoreError::Unavailable(
                    "system credential store rejected the read operation".to_owned(),
                )
            })?
            .ok_or(SecretStoreError::NotFound)?;
        SecretValue::new(value)
    }

    async fn delete(&self, reference: &CredentialReference) -> Result<(), SecretStoreError> {
        Self::ensure_source(reference)?;
        self.backend
            .delete(&self.service, &reference.locator)
            .map_err(|_| {
                SecretStoreError::Unavailable(
                    "system credential store rejected the delete operation".to_owned(),
                )
            })
    }

    async fn exists(&self, reference: &CredentialReference) -> Result<bool, SecretStoreError> {
        Self::ensure_source(reference)?;
        self.backend
            .get(&self.service, &reference.locator)
            .map(|value| value.is_some())
            .map_err(|_| {
                SecretStoreError::Unavailable(
                    "system credential store rejected the status operation".to_owned(),
                )
            })
    }
}

#[derive(Debug, Default)]
pub struct EnvironmentSecretStore;

impl EnvironmentSecretStore {
    fn ensure_source(reference: &CredentialReference) -> Result<(), SecretStoreError> {
        reference.validate()?;
        if reference.source != CredentialSource::EnvironmentVariable {
            return Err(SecretStoreError::InvalidReference(
                "environment store requires an environment_variable reference".to_owned(),
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl SecretStore for EnvironmentSecretStore {
    async fn put(
        &self,
        _scope: SecretScope,
        _secret: SecretValue,
    ) -> Result<CredentialReference, SecretStoreError> {
        Err(SecretStoreError::ReadOnly(
            "set the environment variable outside AnnotAgent".to_owned(),
        ))
    }

    async fn resolve(
        &self,
        reference: &CredentialReference,
    ) -> Result<SecretValue, SecretStoreError> {
        Self::ensure_source(reference)?;
        let value = std::env::var(&reference.locator).map_err(|_| SecretStoreError::NotFound)?;
        SecretValue::new(value)
    }

    async fn delete(&self, reference: &CredentialReference) -> Result<(), SecretStoreError> {
        Self::ensure_source(reference)?;
        Err(SecretStoreError::ReadOnly(
            "remove the environment variable outside AnnotAgent".to_owned(),
        ))
    }

    async fn exists(&self, reference: &CredentialReference) -> Result<bool, SecretStoreError> {
        Self::ensure_source(reference)?;
        Ok(std::env::var_os(&reference.locator).is_some())
    }
}

#[derive(Debug, Default)]
pub struct SessionSecretStore {
    secrets: RwLock<HashMap<CredentialReference, SecretValue>>,
}

#[async_trait]
impl SecretStore for SessionSecretStore {
    async fn put(
        &self,
        scope: SecretScope,
        secret: SecretValue,
    ) -> Result<CredentialReference, SecretStoreError> {
        let reference = scope.reference();
        reference.validate()?;
        if reference.source != CredentialSource::SessionOnly {
            return Err(SecretStoreError::InvalidReference(
                "session store requires a session_only reference".to_owned(),
            ));
        }
        self.secrets.write().await.insert(reference.clone(), secret);
        Ok(reference)
    }

    async fn resolve(
        &self,
        reference: &CredentialReference,
    ) -> Result<SecretValue, SecretStoreError> {
        reference.validate()?;
        self.secrets
            .read()
            .await
            .get(reference)
            .cloned()
            .ok_or(SecretStoreError::NotFound)
    }

    async fn delete(&self, reference: &CredentialReference) -> Result<(), SecretStoreError> {
        reference.validate()?;
        self.secrets.write().await.remove(reference);
        Ok(())
    }

    async fn exists(&self, reference: &CredentialReference) -> Result<bool, SecretStoreError> {
        reference.validate()?;
        Ok(self.secrets.read().await.contains_key(reference))
    }
}

#[derive(Debug, Default)]
pub struct InMemorySecretStore {
    secrets: RwLock<HashMap<CredentialReference, SecretValue>>,
}

#[async_trait]
impl SecretStore for InMemorySecretStore {
    async fn put(
        &self,
        scope: SecretScope,
        secret: SecretValue,
    ) -> Result<CredentialReference, SecretStoreError> {
        let reference = scope.reference();
        reference.validate()?;
        self.secrets.write().await.insert(reference.clone(), secret);
        Ok(reference)
    }

    async fn resolve(
        &self,
        reference: &CredentialReference,
    ) -> Result<SecretValue, SecretStoreError> {
        reference.validate()?;
        self.secrets
            .read()
            .await
            .get(reference)
            .cloned()
            .ok_or(SecretStoreError::NotFound)
    }

    async fn delete(&self, reference: &CredentialReference) -> Result<(), SecretStoreError> {
        reference.validate()?;
        self.secrets.write().await.remove(reference);
        Ok(())
    }

    async fn exists(&self, reference: &CredentialReference) -> Result<bool, SecretStoreError> {
        reference.validate()?;
        Ok(self.secrets.read().await.contains_key(reference))
    }
}

#[derive(Debug, Default)]
pub struct LegacyWorkspaceFileSecretStore {
    files: BTreeMap<String, PathBuf>,
}

impl LegacyWorkspaceFileSecretStore {
    #[must_use]
    pub fn single(locator: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            files: BTreeMap::from([(locator.into(), path.into())]),
        }
    }

    fn path_for<'a>(
        &'a self,
        reference: &CredentialReference,
    ) -> Result<&'a Path, SecretStoreError> {
        reference.validate()?;
        if reference.source != CredentialSource::LegacyWorkspaceFile {
            return Err(SecretStoreError::InvalidReference(
                "legacy file store requires a legacy_workspace_file reference".to_owned(),
            ));
        }
        self.files
            .get(&reference.locator)
            .map(PathBuf::as_path)
            .ok_or_else(|| {
                SecretStoreError::InvalidReference(
                    "legacy credential path is not registered".to_owned(),
                )
            })
    }
}

#[async_trait]
impl SecretStore for LegacyWorkspaceFileSecretStore {
    async fn put(
        &self,
        _scope: SecretScope,
        _secret: SecretValue,
    ) -> Result<CredentialReference, SecretStoreError> {
        Err(SecretStoreError::ReadOnly(
            "legacy workspace credentials require explicit migration".to_owned(),
        ))
    }

    async fn resolve(
        &self,
        reference: &CredentialReference,
    ) -> Result<SecretValue, SecretStoreError> {
        let path = self.path_for(reference)?;
        let metadata = std::fs::symlink_metadata(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                SecretStoreError::NotFound
            } else {
                SecretStoreError::Unavailable(
                    "legacy credential file cannot be inspected".to_owned(),
                )
            }
        })?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(SecretStoreError::InvalidReference(
                "legacy credential must be a regular non-symlink file".to_owned(),
            ));
        }
        let value = std::fs::read_to_string(path).map_err(|_| {
            SecretStoreError::Unavailable("legacy credential file cannot be read".to_owned())
        })?;
        SecretValue::new(value.trim().to_owned())
    }

    async fn delete(&self, reference: &CredentialReference) -> Result<(), SecretStoreError> {
        let path = self.path_for(reference)?;
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(SecretStoreError::Unavailable(
                "legacy credential file cannot be removed".to_owned(),
            )),
        }
    }

    async fn exists(&self, reference: &CredentialReference) -> Result<bool, SecretStoreError> {
        let path = self.path_for(reference)?;
        Ok(path.is_file())
    }
}

pub struct SecretStoreRouter {
    pub keyring: Arc<dyn SecretStore>,
    pub environment: Arc<dyn SecretStore>,
    pub session: Arc<dyn SecretStore>,
    pub legacy: Arc<dyn SecretStore>,
}

impl SecretStoreRouter {
    fn store(&self, source: CredentialSource) -> &Arc<dyn SecretStore> {
        match source {
            CredentialSource::SystemKeyring => &self.keyring,
            CredentialSource::EnvironmentVariable => &self.environment,
            CredentialSource::SessionOnly => &self.session,
            CredentialSource::LegacyWorkspaceFile => &self.legacy,
        }
    }
}

#[async_trait]
impl SecretStore for SecretStoreRouter {
    async fn put(
        &self,
        scope: SecretScope,
        secret: SecretValue,
    ) -> Result<CredentialReference, SecretStoreError> {
        self.store(scope.source).put(scope, secret).await
    }

    async fn resolve(
        &self,
        reference: &CredentialReference,
    ) -> Result<SecretValue, SecretStoreError> {
        self.store(reference.source).resolve(reference).await
    }

    async fn delete(&self, reference: &CredentialReference) -> Result<(), SecretStoreError> {
        self.store(reference.source).delete(reference).await
    }

    async fn exists(&self, reference: &CredentialReference) -> Result<bool, SecretStoreError> {
        self.store(reference.source).exists(reference).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use annotagent_core::ProviderId;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MockKeyringBackend {
        secrets: Mutex<HashMap<(String, String), String>>,
    }

    impl KeyringBackend for MockKeyringBackend {
        fn get(&self, service: &str, account: &str) -> Result<Option<String>, KeyringBackendError> {
            Ok(self
                .secrets
                .lock()
                .expect("keyring lock")
                .get(&(service.to_owned(), account.to_owned()))
                .cloned())
        }

        fn set(
            &self,
            service: &str,
            account: &str,
            secret: &str,
        ) -> Result<(), KeyringBackendError> {
            self.secrets
                .lock()
                .expect("keyring lock")
                .insert((service.to_owned(), account.to_owned()), secret.to_owned());
            Ok(())
        }

        fn delete(&self, service: &str, account: &str) -> Result<(), KeyringBackendError> {
            self.secrets
                .lock()
                .expect("keyring lock")
                .remove(&(service.to_owned(), account.to_owned()));
            Ok(())
        }
    }

    fn scope(source: CredentialSource, locator: &str) -> SecretScope {
        SecretScope {
            provider_id: ProviderId::new(),
            source,
            locator: locator.to_owned(),
        }
    }

    #[tokio::test]
    async fn keyring_mock_round_trips_without_exposing_the_value() {
        let backend = Arc::new(MockKeyringBackend::default());
        let store = KeyringSecretStore::with_backend("com.annotagent.test", backend);
        let reference = store
            .put(
                scope(CredentialSource::SystemKeyring, "provider-account"),
                SecretValue::new("keyring-secret").expect("secret"),
            )
            .await
            .expect("put");
        assert!(store.exists(&reference).await.expect("exists"));
        assert_eq!(
            store
                .resolve(&reference)
                .await
                .expect("resolve")
                .expose_secret(),
            "keyring-secret"
        );
        store.delete(&reference).await.expect("delete");
        assert!(!store.exists(&reference).await.expect("missing"));
    }

    #[tokio::test]
    async fn session_and_memory_stores_are_reference_scoped() {
        let session = SessionSecretStore::default();
        let first = scope(CredentialSource::SessionOnly, "session-a");
        let second = SecretScope {
            locator: "session-b".to_owned(),
            ..first.clone()
        };
        let first_ref = session
            .put(first, SecretValue::new("first").expect("secret"))
            .await
            .expect("put");
        assert!(session.exists(&first_ref).await.expect("exists"));
        assert!(!session.exists(&second.reference()).await.expect("missing"));

        let memory = InMemorySecretStore::default();
        let memory_ref = memory
            .put(
                scope(CredentialSource::SystemKeyring, "memory"),
                SecretValue::new("memory-value").expect("secret"),
            )
            .await
            .expect("put");
        assert_eq!(
            memory
                .resolve(&memory_ref)
                .await
                .expect("resolve")
                .expose_secret(),
            "memory-value"
        );
    }

    #[tokio::test]
    async fn environment_store_is_read_only_and_does_not_echo_values() {
        let store = EnvironmentSecretStore;
        let reference = scope(CredentialSource::EnvironmentVariable, "PATH").reference();
        assert!(store.exists(&reference).await.expect("PATH exists"));
        assert!(matches!(
            store
                .put(
                    scope(CredentialSource::EnvironmentVariable, "PATH"),
                    SecretValue::new("must-not-be-written").expect("secret")
                )
                .await,
            Err(SecretStoreError::ReadOnly(_))
        ));
    }

    #[tokio::test]
    async fn legacy_file_is_read_only_until_explicit_delete_or_migration() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("provider-api-key");
        std::fs::write(&path, "legacy-secret\n").expect("fixture");
        let store = LegacyWorkspaceFileSecretStore::single("legacy-provider", &path);
        let reference = scope(CredentialSource::LegacyWorkspaceFile, "legacy-provider").reference();
        assert_eq!(
            store
                .resolve(&reference)
                .await
                .expect("resolve")
                .expose_secret(),
            "legacy-secret"
        );
        assert!(matches!(
            store
                .put(
                    scope(CredentialSource::LegacyWorkspaceFile, "legacy-provider"),
                    SecretValue::new("replacement").expect("secret")
                )
                .await,
            Err(SecretStoreError::ReadOnly(_))
        ));
        store.delete(&reference).await.expect("explicit delete");
        assert!(!path.exists());
    }
}
