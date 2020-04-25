#![allow(dead_code)]

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    PostgresError(#[from] tokio_postgres::Error),

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

pub use db_handle::DbHandle;

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!();
}
