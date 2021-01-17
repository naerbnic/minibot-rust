#![allow(dead_code)]

pub use futures;

/// A helper function for writing transaction functions. Captured variables are listed, then a
/// simple function is written. This creates the required clones of the captured variables.
/// The return type of the function can be declared. The function must be declared `move`, or
/// the captured variables must implement `Clone`.
#[macro_export]
macro_rules! tx_func {
    ([$($clones:ident),*], |$tx:ident| -> $out:ty $body:block) => {
        {
            $(let $clones = ::std::clone::Clone::clone(&$clones);)*
            move |$tx| {
                $(let $clones = ::std::clone::Clone::clone(&$clones);)*
                let boxed_fut: $crate::futures::future::BoxFuture<'_, $out> =
                    $crate::futures::future::FutureExt::boxed(async move {$body});
                boxed_fut
            }
        }
    };
    ([$($clones:ident),*], move |$tx:ident| -> $out:ty $body:block) => {
        {
            move |$tx| {
                $(let $clones = ::std::clone::Clone::clone(&$clones);)*
                let boxed_fut: $crate::futures::future::BoxFuture<'_, $out> =
                    $crate::futures::future::FutureExt::boxed(async move {$body});
                boxed_fut
            }
        }
    };

    ([$($clones:ident),*], |$tx:ident| $body:expr) => {
        tx_func!([$($clones),*], |$tx| -> _ {$body})
    };
    
    ([$($clones:ident),*], move |$tx:ident| $body:expr) => {
        tx_func!([$($clones),*], move |$tx| -> _ {$body})
    };

    ([$($clones:ident,)*], |$tx:ident| -> $out:ty $body:block) => {
        tx_func!([$($clones),*], |$tx| -> $out $body)
    };
    ([$($clones:ident,)*], |$tx:ident| $body:expr) => {
        tx_func!([$($clones),*], |$tx| $body)
    };

    ([$($clones:ident,)*], move |$tx:ident| -> $out:ty $body:block) => {
        tx_func!([$($clones),*], move |$tx| -> $out $body)
    };
    ([$($clones:ident,)*], move |$tx:ident| $body:expr) => {
        tx_func!([$($clones),*], move |$tx| $body)
    };

    (|$tx:ident| $body:expr) => {
        tx_func!(|$tx| -> _ {$body})
    };
    (|$tx:ident| -> $out:ty $body:block) => {
        tx_func!([], |$tx| -> $out $body)
    };

    (move |$tx:ident| $body:expr) => {
        tx_func!(|$tx| -> _ {$body})
    };
    (move |$tx:ident| -> $out:ty $body:block) => {
        tx_func!([], |$tx| -> $out $body)
    };
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    PostgresError(#[from] tokio_postgres::Error),

    #[error(transparent)]
    Bb8(#[from] bb8::RunError<tokio_postgres::Error>),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error("Recieved invalid argument.")]
    InvalidArgument,

    #[error("")]
    ConnectionTimedOut,
}

pub type Result<T> = std::result::Result<T, Error>;

mod pool;
mod queries;
mod user;

pub use pool::{DbHandle, DbConn};
