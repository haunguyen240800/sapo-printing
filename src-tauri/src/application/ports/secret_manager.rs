use crate::shared::errors::InfrastructureError;

pub trait SecretManager: Send + Sync {
    fn store(&self, key: &str, value: &str) -> Result<(), InfrastructureError>;

    fn retrieve(&self, key: &str) -> Result<Option<String>, InfrastructureError>;

    fn delete(&self, key: &str) -> Result<(), InfrastructureError>;
}
