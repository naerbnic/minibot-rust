use crate::pool::DbConn;

use chrono::{DateTime, Utc};
use rand::{distributions::Alphanumeric, Rng};

pub async fn remove_expired_ephemeral_tokens(conn: &mut DbConn<'_>) -> Result<(), crate::Error> {
    conn.with_tx(tx_func!(|tx| {
        tx.batch_execute(include_str!("remove_expired_ephemeral_tokens.sql"))
            .await?;
        Ok(())
    }))
    .await
}

pub async fn create_ephemeral_token<E>(
    conn: &mut DbConn<'_>,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    data: &[u8],
) -> Result<String, crate::Error> {
    let data = data.to_vec();
    // Repeatedly try random tokens until one works. Given how large the space is, this should
    // pretty much never happen.
    loop {
        let token = rand::thread_rng()
            .sample_iter(Alphanumeric)
            .take(32)
            .collect::<String>();

        let tx_result = conn
            .with_tx(tx_func!([token, created_at, expires_at, data], |tx| {
                tx.execute(
                    include_str!("insert_ephemeral_token.sql"),
                    &[&token, &created_at, &expires_at, &data],
                )
                .await?;
                Ok::<_, crate::Error>(())
            }))
            .await;

        if let Err(e) = tx_result {
            if let crate::Error::PostgresError(err) = &e {
                if let Some(code) = err.code() {
                    if code == &tokio_postgres::error::SqlState::UNIQUE_VIOLATION {
                        continue;
                    }
                }
            }
            return Err(e);
        }

        return Ok(token);
    }
}

pub async fn get_ephemeral_token(
    conn: &mut DbConn<'_>,
    token: &str,
) -> Result<Vec<u8>, crate::Error> {
    let token = token.to_string();
    let data = conn
        .with_tx(tx_func!([token], |tx| {
            let row = tx
                .query_one(include_str!("insert_ephemeral_token.sql"), &[&token])
                .await?;
            Ok::<Vec<u8>, crate::Error>(row.try_get::<'_, _, Vec<u8>>(0).unwrap())
        }))
        .await?;
    Ok(data)
}
