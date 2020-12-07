use std::ffi::{OsStr, OsString};

use serde::{
    de::{DeserializeOwned, Error as _},
    ser::Error as _,
    Deserialize, Serialize,
};

/// A simple wrapper type which encodes it's serde-able type as an ASCII string.
#[derive(Copy, Clone, Debug)]
pub struct AsciiWrap<T>(T);

impl<T> AsciiWrap<T> {
    pub fn new(v: T) -> Self {
        AsciiWrap(v)
    }
    pub fn into_inner(self) -> T {
        self.0
    }
    pub fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> std::ops::Deref for AsciiWrap<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for AsciiWrap<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'de, T> Deserialize<'de> for AsciiWrap<T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let obj = from_str(&String::deserialize(deserializer)?).map_err(D::Error::custom)?;
        Ok(AsciiWrap(obj))
    }
}

impl<'de, T> Serialize for AsciiWrap<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let string = to_string(&self.0).map_err(S::Error::custom)?;
        string.serialize(serializer)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum EncodeError {
    #[error("Error while encoding MessagePack")]
    MessagePack(#[from] rmp_serde::encode::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum DecodeError {
    #[error("OsStr was not a valid ascii string")]
    InvalidAscii,

    #[error("Error while decoding base64: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("Error while decoding MessagePack")]
    MessagePack(#[from] rmp_serde::decode::Error),
}

pub fn from_str<T>(enc: &str) -> Result<T, DecodeError>
where
    T: DeserializeOwned,
{
    let bytes = base64::decode(enc)?;
    let obj = rmp_serde::from_read_ref(&bytes)?;
    Ok(obj)
}

pub fn from_os_str<T>(enc: &OsStr) -> Result<T, DecodeError>
where
    T: DeserializeOwned,
{
    if let Some(enc) = enc.to_str() {
        from_str(enc)
    } else {
        Err(DecodeError::InvalidAscii)
    }
}

pub fn to_string<T>(value: &T) -> Result<String, EncodeError>
where
    T: Serialize,
{
    let bytes = rmp_serde::to_vec(value)?;
    Ok(base64::encode(&bytes))
}

pub fn to_os_string<T>(value: &T) -> Result<OsString, EncodeError>
where
    T: Serialize,
{
    Ok(to_string(value)?.into())
}
