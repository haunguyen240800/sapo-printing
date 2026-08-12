use crate::application::errors::Error;

pub trait SecretPort: Send + Sync {
    fn store(&self, key: &str, value: &str) -> Result<(), Error>;

    fn retrieve(&self, key: &str) -> Result<Option<String>, Error>;

    fn delete(&self, key: &str) -> Result<(), Error>;
}
