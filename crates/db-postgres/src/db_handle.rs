use crate::Result as DbResult;
use postgres::{Client, Transaction};
use r2d2::{Pool, PooledConnection};
use r2d2_postgres::{postgres::NoTls, PostgresConnectionManager};

pub trait SavedStatement: 'static {
    fn stmt() -> &'static str;
}


pub struct DbHandleGuard(PooledConnection<PostgresConnectionManager<NoTls>>);

impl DbHandleGuard {
    pub fn transaction(&mut self) -> Result<Transaction, postgres::Error> {
        Ok(self.0.transaction()?)
    }

    pub fn with_tx<F, T, E>(&mut self, mut func: F) -> Result<T, E>
    where
        F: FnMut(&mut Transaction) -> Result<T, E>,
        E: From<postgres::Error> + Send + 'static,
    {
        loop {
            let mut tx = self.transaction()?;
            match func(&mut tx) {
                Ok(val) => {
                    if let Err(err) = tx.commit() {
                        if let Some(code) = err.code() {
                            if code == &postgres::error::SqlState::T_R_SERIALIZATION_FAILURE {
                                continue;
                            }
                        }
                        return Err(err.into());
                    }
                    return Ok(val);
                }
                Err(err) => {
                    tx.rollback()?;
                    return Err(err);
                }
            }
        }
    }
}

impl<'a> std::ops::Deref for DbHandleGuard {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl<'a> std::ops::DerefMut for DbHandleGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.0
    }
}

#[derive(Clone)]
pub struct DbHandle(Pool<PostgresConnectionManager<NoTls>>);

impl DbHandle {
    pub fn new(url: String) -> DbResult<Self> {
        let pool = Pool::new(PostgresConnectionManager::new(url.parse()?, NoTls))?;
        Ok(DbHandle(pool))
    }

    pub async fn with_tx<T, E, F>(&self, func: F) -> std::result::Result<T, E>
    where
        F: FnMut(&mut Transaction) -> std::result::Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: From<r2d2::Error> + From<postgres::Error> + Send + 'static,
    {
        let handle = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = handle.get()?;
            guard.with_tx(func)
        })
        .await
        .unwrap()
    }

    pub fn with_test<F, Fut>(url: String, test: F) -> DbResult<()>
    where
        F: FnOnce(&mut DbHandleGuard) -> DbResult<()>,
    {
        let handle = DbHandle::new(url)?;

        let mut guard = handle.get()?;

        println!("Got handle.");
        {
            let mut tx = guard.transaction()?;
            tx.batch_execute(
                r#"
                    CREATE SCHEMA test;
                    SET search_path TO test;
                "#,
            )?;
            tx.commit()?;
        }

        let result = test(&mut guard);

        {
            let mut guard = handle.get()?;
            let mut tx = guard.transaction()?;
            tx.batch_execute(
                r#"
                    SET search_path TO public;
                    DROP SCHEMA test CASCADE;
                "#,
            )?;
            tx.commit()?;
        }
        result
    }

    pub fn get<'a>(&'a self) -> Result<DbHandleGuard, r2d2::Error> {
        let conn = self.0.get()?;
        Ok(DbHandleGuard(conn))
    }
}
