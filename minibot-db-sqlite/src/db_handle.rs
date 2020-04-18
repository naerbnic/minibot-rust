use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, CustomizeConnection, Error as PoolError, Pool};
use diesel::result::{ConnectionError, Error as DbError};
use std::sync::Arc;

#[derive(Debug)]
struct Customizer;

impl CustomizeConnection<SqliteConnection, PoolError> for Customizer {
    fn on_acquire(&self, conn: &mut SqliteConnection) -> std::result::Result<(), PoolError> {
        conn.batch_execute("PRAGMA foreign_keys=ON;").map_err(|e| {
            PoolError::ConnectionError(ConnectionError::CouldntSetupConfiguration(e))
        })?;
        match crate::embedded_migrations::run(conn) {
            Ok(()) => Ok(()),
            Err(diesel_migrations::RunMigrationsError::QueryError(e)) => Err(
                PoolError::ConnectionError(ConnectionError::CouldntSetupConfiguration(e)),
            ),
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    R2D2Error(#[from] r2d2::Error),

    #[error(transparent)]
    DatabaseError(#[from] DbError),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
pub struct DbHandle(Arc<Pool<ConnectionManager<SqliteConnection>>>);

impl DbHandle {
    pub async fn new(db_url: &str) -> Result<Self> {
        let db_url = db_url.to_string();
        tokio::task::spawn_blocking(move || {
            let pool = Pool::builder()
                .connection_customizer(Box::new(Customizer))
                .build(ConnectionManager::new(db_url))?;

            Ok(DbHandle(Arc::new(pool)))
        })
        .await
        .unwrap()
    }

    pub async fn run<F, T>(&self, op: F) -> Result<T>
    where
        F: FnOnce(&SqliteConnection) -> std::result::Result<T, DbError> + Send + 'static,
        T: Send + 'static,
    {
        let pool_handle = self.0.clone();
        tokio::task::spawn_blocking(move || {
            let pooled_conn = pool_handle.get()?;
            let result = op(&*pooled_conn).map_err(Error::DatabaseError)?;
            Ok::<_, Error>(result)
        })
        .await
        .unwrap()
    }

    pub async fn run_tx<F, T>(&self, op: F) -> Result<T>
    where
        F: FnOnce(&SqliteConnection) -> std::result::Result<T, DbError> + Send + 'static,
        T: Send + 'static,
    {
        self.run(move |conn| op(conn)).await
    }
}
