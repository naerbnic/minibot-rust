use bb8::{Pool, PooledConnection};
use bb8_postgres::PostgresConnectionManager;
use futures::future::BoxFuture;
use tokio_postgres::{NoTls, Transaction};

type PgPool = Pool<PostgresConnectionManager<NoTls>>;

type PgPoolConn<'a> = PooledConnection<'a, PostgresConnectionManager<NoTls>>;

pub async fn run_tx<'a, 'b, 'c, F, T, E>(conn: &'b mut PgPoolConn<'a>, mut func: F) -> Result<T, E>
where
    F: for<'d, 'e> FnMut(&'d mut Transaction<'e>) -> BoxFuture<'d, Result<T, E>>,
    E: From<tokio_postgres::Error>,
{
    loop {
        let mut tx = conn.transaction().await?;
        match func(&mut tx).await {
            Ok(v) => match tx.commit().await {
                Ok(()) => return Ok(v),
                Err(e) => {
                    if let Some(code) = e.code() {
                        if code == &tokio_postgres::error::SqlState::T_R_SERIALIZATION_FAILURE {
                            continue;
                        }
                    }

                    return Err(e.into());
                }
            },
            Err(e) => {
                tx.rollback().await?;
                return Err(e);
            }
        }
    }
}

/// A helper function for writing transaction functions. Captured variables are listed, then a
/// simple function is written. This creates the required clones of the captured variables.
/// The return type of the function can be declared. Captured variables must implement `Clone`.
macro_rules! tx_func {
    ([$($clones:ident),*], |$tx:ident| -> $out:ty $body:block) => {
        {
            $(let $clones = ::std::clone::Clone::clone(&$clones);)*
            move |$tx| {
                $(let $clones = ::std::clone::Clone::clone(&$clones);)*
                let boxed_fut: BoxFuture<'_, $out> =
                    ::futures::future::FutureExt::boxed(async move {$body});
                boxed_fut
            }
        }
    };
    ([$($clones:ident),*], |$tx:ident| $body:expr) => {
        tx_func!([$($clones),*], |$tx| -> _ {$body})
    };
    ([$($clones:ident,)*], |$tx:ident| -> $out:ty $body:block) => {
        tx_func!([$($clones),*], |$tx| -> $out $body)
    };
    ([$($clones:ident,)*], |$tx:ident| $body:expr) => {
        tx_func!([$($clones),*], |$tx| $body)
    };
}

pub async fn test_fn(pool: &mut PgPool) {
    let x = "abc".to_string();
    let mut conn = pool.get().await.unwrap();
    run_tx(
        &mut conn,
        tx_func!([x], |tx| {
            tx.batch_execute(&x).await?;
            Ok::<_, tokio_postgres::Error>(())
        }),
    )
    .await
    .unwrap();
}
