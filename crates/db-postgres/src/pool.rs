use crate::Result as DbResult;
use bb8::{Pool, PooledConnection, RunError};
use bb8_postgres::PostgresConnectionManager;
use futures::future::BoxFuture;
use tokio_postgres::{Client, Error as DbError, NoTls, Transaction, TransactionBuilder};

pub struct DbConn<'a>(PooledConnection<'a, PostgresConnectionManager<NoTls>>);

impl<'a> DbConn<'a> {
    pub async fn with_tx_builder<F, T, E, G>(&mut self, mut init: F, mut func: G) -> Result<T, E>
    where
        F: for<'c> FnMut(TransactionBuilder<'c>) -> TransactionBuilder<'c>,
        G: for<'b, 'c> FnMut(&'b mut Transaction<'c>) -> BoxFuture<'b, Result<T, E>>,
        E: From<DbError>,
    {
        loop {
            let tx_builder = self.0.build_transaction();
            let tx_builder = init(tx_builder);
            let mut tx = tx_builder.start().await?;
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

    pub async fn with_tx<F, T, E>(&mut self, func: F) -> Result<T, E>
    where
        F: for<'d, 'e> FnMut(&'d mut Transaction<'e>) -> BoxFuture<'d, Result<T, E>>,
        E: From<DbError>,
    {
        self.with_tx_builder(|txb| txb, func).await
    }
}

impl<'a> std::ops::Deref for DbConn<'_> {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl<'a> std::ops::DerefMut for DbConn<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.0
    }
}

#[derive(Clone)]
pub struct DbHandle(Pool<PostgresConnectionManager<NoTls>>);

impl DbHandle {
    pub async fn new(url: String) -> DbResult<Self> {
        let pool = Pool::builder()
            .build(PostgresConnectionManager::new(url.parse()?, NoTls))
            .await?;
        Ok(DbHandle(pool))
    }

    pub async fn with_tx<T, E, F>(&self, func: F) -> std::result::Result<T, E>
    where
        F: for<'d, 'e> FnMut(&'d mut Transaction<'e>) -> BoxFuture<'d, Result<T, E>>,
        E: From<DbError> + From<RunError<DbError>>,
    {
        let mut conn = self.get().await?;
        conn.with_tx(func).await
    }

    pub async fn with_test<F, Fut>(url: String, test: F) -> DbResult<()>
    where
        F: FnOnce(&mut DbConn) -> DbResult<()>,
    {
        let handle = DbHandle::new(url).await?;

        let mut conn = handle.get().await?;

        println!("Got handle.");
        {
            conn.with_tx(tx_func!(|tx| -> Result<(), crate::Error> {
                tx.batch_execute(
                    r#"
                        CREATE SCHEMA test;
                        SET search_path TO test;
                    "#,
                )
                .await?;
                Ok(())
            }))
            .await?;
        }

        let result = {
            let mut guard = handle.get().await?;
            test(&mut guard)
        };

        {
            let mut guard = handle.get().await?;
            guard
                .with_tx(tx_func!(|tx| {
                    tx.batch_execute(
                        r#"
                        SET search_path TO public;
                        DROP SCHEMA test CASCADE;
                    "#,
                    )
                    .await
                }))
                .await?;
        }
        result
    }

    pub async fn get<'a>(&'a self) -> Result<DbConn<'a>, bb8::RunError<DbError>> {
        let conn = self.0.get().await?;
        Ok(DbConn(conn))
    }
}
