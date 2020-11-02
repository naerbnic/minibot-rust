use crate::services::token_service::TokenServiceHandle;

pub mod serde;
pub mod table;

pub fn create_serde<T>() -> TokenServiceHandle<T>
where
    T: ::serde::Serialize
        + ::serde::de::DeserializeOwned
        + Send
        + Sync
        + std::panic::RefUnwindSafe
        + 'static,
{
    TokenServiceHandle::new(serde::SerdeTokenService::new())
}