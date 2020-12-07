use chrono::{DateTime, Utc};
use rand::{distributions::Alphanumeric, Rng};
use sqlx::{prelude::*, Postgres};

pub async fn remove_expired_ephemeral_tokens<'a, E>(conn: E) -> sqlx::Result<()>
where
    E: Acquire<'a, Database = Postgres>,
    E::Connection: Executor<'a, Database = Postgres>,
{
    sqlx::query::<Postgres>(include_str!("remove_expired_ephemeral_tokens.sql"))
        .execute(conn.acquire().await?)
        .await?;
    Ok(())
}

pub async fn create_ephemeral_token<'a, E>(
    conn: E,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    data: &[u8],
) -> sqlx::Result<String>
where
    E: Acquire<'a, Database = Postgres>,
    E::Connection: Executor<'a, Database = Postgres>,
{
    let mut tx = conn.begin().await?;
    loop {
        let token = rand::thread_rng()
            .sample_iter(Alphanumeric)
            .take(32)
            .collect::<String>();
        if let Err(e) = sqlx::query::<Postgres>(include_str!("insert_ephemeral_token.sql"))
            .bind(&token)
            .bind(&created_at)
            .bind(&expires_at)
            .bind(data)
            .execute(&mut tx)
            .await
        {
            if let sqlx::Error::Database(inner) = &e {
                if let Some(code) = inner.code() {
                    if code.as_ref() == "unique_violation" {
                        continue;
                    }
                }
            }
            return Err(e);
        }

        return Ok(token);
    }
}

pub async fn get_ephemeral_token<'a, E>(conn: E, token: &str) -> sqlx::Result<Vec<u8>>
where
    E: Acquire<'a, Database = Postgres>,
    E::Connection: Executor<'a, Database = Postgres>,
{
    let row = sqlx::query(include_str!("get_ephemeral_token.sql"))
        .bind(token)
        .fetch_one(conn.acquire().await?)
        .await?;
    let data: Vec<u8> = row.try_get(0)?;
    Ok(data)
}
