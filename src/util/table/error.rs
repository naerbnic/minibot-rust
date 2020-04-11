#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Unknown index: {0}")]
    UnknownIndex(String),

    #[error("Tried to update nonexistent entry: {0}")]
    UpdatedNonexistentEntry(u64),

    #[error("Tried to remove nonexistent entry: {0}")]
    RemovingNonexistentId(u64),

    #[error("Entry already exists.")]
    AlreadyExists,
}

pub type Result<T> = std::result::Result<T, Error>;