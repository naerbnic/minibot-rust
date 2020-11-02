use async_trait::async_trait;
use fernet::Fernet;
use serde::{de::DeserializeOwned, Serialize};
use std::panic::RefUnwindSafe;

use crate::services::base::token_service::TokenService;

pub struct SerdeTokenService<T>
where
    T: Serialize + DeserializeOwned + Sync + Send + RefUnwindSafe,
{
    encdec: Fernet,
    _data: std::marker::PhantomData<T>,
}

impl<T> SerdeTokenService<T>
where
    T: Serialize + DeserializeOwned + Sync + Send + RefUnwindSafe + 'static,
{
    pub fn new() -> Self {
        SerdeTokenService {
            encdec: Fernet::new(&Fernet::generate_key()).unwrap(),
            _data: std::marker::PhantomData {},
        }
    }
}

#[async_trait]
impl<T: Serialize + DeserializeOwned + Sync + Send + RefUnwindSafe> TokenService<T>
    for SerdeTokenService<T>
{
    async fn to_token(&self, value: T) -> Result<String, anyhow::Error> {
        let encrypted = self
            .encdec
            .encrypt(serde_json::to_string(&value)?.as_bytes());
        Ok(encrypted)
    }

    async fn from_token(&self, token: &str) -> Result<Option<T>, anyhow::Error> {
        let decrypted = self
            .encdec
            .decrypt(token)
            .map_err(|_| anyhow::anyhow!("Unable to decrypt token."))?;
        Ok(Some(serde_json::from_slice(&decrypted)?))
    }
}
