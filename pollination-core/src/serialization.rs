use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "json")]
mod json {
    use super::*;

    pub(crate) fn serialize<T: Serialize>(val: T) -> Result<Vec<u8>, SerializeError> {
        serde_json::to_vec(&val)
    }

    pub(crate) type SerializeError = serde_json::Error;

    pub(crate) fn deserialize<T: for<'de> Deserialize<'de>>(
        val: Vec<u8>,
    ) -> Result<T, DeserializeError> {
        let jd = &mut serde_json::Deserializer::from_slice(&val[..]);
        serde_path_to_error::deserialize(jd)
    }

    pub(crate) type DeserializeError = serde_path_to_error::Error<serde_json::Error>;
}

#[cfg(feature = "json")]
pub use json::*;

#[cfg(not(feature = "json"))]
mod bincode_ {
    use super::*;

    pub(crate) fn serialize<T: Serialize>(val: T) -> Result<Vec<u8>, SerializeError> {
        bincode::serde::encode_to_vec(val, bincode::config::standard())
    }

    pub(crate) type SerializeError = bincode::error::EncodeError;

    pub(crate) fn deserialize<T: for<'de> Deserialize<'de>>(
        val: Vec<u8>,
    ) -> Result<T, DeserializeError> {
        let (res, _) = bincode::serde::decode_from_slice(&val, bincode::config::standard())?;
        Ok(res)
    }

    pub(crate) type DeserializeError = bincode::error::DecodeError;
}

#[cfg(not(feature = "json"))]
pub use bincode_::*;
