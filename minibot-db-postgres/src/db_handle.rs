use r2d2::Pool;
use tokio_postgres::Connection;

pub struct DbHandle(Arc<Pool<Connection>>);