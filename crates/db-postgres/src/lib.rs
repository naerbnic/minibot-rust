#![allow(dead_code)]

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    PostgresError(#[from] postgres::Error),

    #[error(transparent)]
    R2D2(#[from] r2d2::Error),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error("Recieved invalid argument.")]
    InvalidArgument,

    #[error("")]
    ConnectionTimedOut,
}

pub type Result<T> = std::result::Result<T, Error>;

mod db_handle;
mod user;
mod queries;

pub use db_handle::DbHandle;


