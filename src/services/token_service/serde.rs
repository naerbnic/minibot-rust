use super::TokenService;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

pub struct SerdeTokenService<T>
where
    T: Serialize + DeserializeOwned + Sync + Send,
{
    _data: std::marker::PhantomData<T>,
}

impl<T: Serialize + DeserializeOwned + Sync + Send + 'static> SerdeTokenService<T> {
    pub fn new() -> Arc<dyn TokenService<T> + Send + Sync> {
        Arc::new(SerdeTokenService {
            _data: std::marker::PhantomData {},
        })
    }
}

#[async_trait]
impl<T: Serialize + DeserializeOwned + Sync + Send> TokenService<T> for SerdeTokenService<T> {
    async fn to_token(&self, value: T) -> Result<String, anyhow::Error> {
        Ok(base64::encode_config(
            &serde_json::to_string(&value)?,
            base64::URL_SAFE_NO_PAD,
        ))
    }
    async fn from_token(&self, token: &str) -> Result<T, anyhow::Error> {
        Ok(serde_json::from_slice(&base64::decode_config(
            token,
            base64::URL_SAFE_NO_PAD,
        )?)?)
    }
}
