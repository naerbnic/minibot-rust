use crate::{Error, Result};
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use futures::prelude::*;
use std::pin::Pin;
use tokio_postgres::{NoTls, Transaction};

#[derive(Clone)]
pub struct DbHandle(Pool<PostgresConnectionManager<NoTls>>);

pub trait SavedStatement: 'static {
    fn stmt() -> &'static str;
}

trait TransactionFunc<'a, T> {
    type Fut: Future<Output = Result<T>> + 'a;

    fn call(self, tx: Transaction<'a>) -> Self::Fut;
}

impl<'a, T, F, Fut> TransactionFunc<'a, T> for F
where
    F: FnOnce(Transaction<'a>) -> Fut,
    Fut: Future<Output = Result<T>> + 'a,
{
    type Fut = Fut;

    fn call(self, tx: Transaction<'a>) -> Self::Fut {
        self(tx)
    }
}

trait AnyTransactionFunc<T>: for<'a> TransactionFunc<'a, T> {}

impl<S, T> AnyTransactionFunc<T> for S where S: for<'a> TransactionFunc<'a, T> {}

impl DbHandle {
    pub async fn new(url: String) -> Result<Self> {
        let pool = Pool::builder()
            .build(PostgresConnectionManager::new_from_stringlike(url, NoTls)?)
            .await?;
        Ok(DbHandle(pool))
    }

    pub async fn with_test<F, Fut>(url: String, test: F) -> Result<()>
    where
        F: FnOnce(DbHandle) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let handle = DbHandle::new(url).await?;

        println!("Got handle.");
        handle
            .run_tx(move |tx: Transaction| async move {
                tx.batch_execute(
                    r#"
                        CREATE SCHEMA test;
                        SET search_path TO test;
                    "#,
                )
                .await?;
                tx.commit().await?;
                Ok(())
            })
            .await?;

        let result = test(handle.clone()).await;

        async fn kill_schema<'a>(tx: Transaction<'a>) -> Result<()> {
            tx.batch_execute(
                r#"
                        SET search_path TO public;
                        DROP SCHEMA test CASCADE;
                    "#,
            )
            .await?;
            tx.commit().await?;
            Ok(())
        }

        handle.run_tx(kill_schema).await?;
        result
    }

    pub async fn run_tx<'a, F, Fut, T>(&'a self, op: F) -> Result<T>
    where
        F: FnOnce(Transaction<'a>) -> Fut,
        Fut: Future<Output = Result<T>> + 'a,
    {
        let mut conn = self.0.get().await.map_err(|e| match e {
            bb8::RunError::User(e) => e.into(),
            bb8::RunError::TimedOut => Error::ConnectionTimedOut,
        })?;
        let tx = conn.transaction().await?;
        let result = op.call(tx).await;
        result
    }
}

#[cfg(test)]
mod test {
    use super::DbHandle;

    #[tokio::test]
    pub async fn smoke_test() -> crate::Result<()> {
        DbHandle::with_test(
            "host=/tmp dbname=minibot user=minibot".to_string(),
            |_| async move { Ok(()) },
        )
        .await?;
        Ok(())
    }
}
