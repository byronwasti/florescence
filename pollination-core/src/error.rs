use thiserror::Error;

#[derive(Debug, Error)]
pub enum PollinationError {
    #[error("No patch present when one was expected")]
    NoPatch,

    #[error("No self present in the ItcMap")]
    NoSelf,
}

pub type Result<T> = std::result::Result<T, PollinationError>;
