use crate::{DbHandle, Error, Result};
use futures::prelude::*;
use std::convert::TryInto;

#[async_trait::async_trait]
pub trait UserService {
    // Standard operations
    async fn create_user(&self, twitch_account: &str) -> Result<i64>;
    async fn set_bot_account(&self, user_id: i64, bot_account: &str) -> Result<()>;
    async fn find_user_by_twitch_account(&self, twitch_account: &str) -> Result<Option<i64>>;
}

// Database Helper Methods

struct UserServiceImpl(DbHandle);

#[async_trait::async_trait]
impl UserService for UserServiceImpl {
    // Standard operations
    async fn create_user(&self, twitch_account: &str) -> Result<i64> {
        let twitch_account = twitch_account.to_string();
        let mut guard = self.0.get().await?;
        let tx = guard.transaction().await?;
        tx.execute(
            r#"
                INSERT INTO twitch_accounts (id)
                VALUES (?)
                ON CONFLICT DO NOTHING
            "#,
            &[&twitch_account],
        )
        .await?;

        let row = tx
            .query_one(
                r#"
                INSERT INTO users (twitch_id)
                VALUES (?)
                RETURNING id
            "#,
                &[&twitch_account],
            )
            .await?;

        tx.commit().await?;

        Ok(row.get::<_, i32>(0).into())
    }

    async fn set_bot_account(&self, user_id: i64, bot_account: &str) -> Result<()> {
        let bot_account = bot_account.to_string();
        let mut guard = self.0.get().await?;
        let tx = guard.transaction().await?;
        let user_id: i32 = user_id.try_into().map_err(|_| Error::InvalidArgument)?;
        tx.execute(
            r#"
                INSERT INTO twitch_accounts (id)
                VALUES (?)
                ON CONFLICT DO NOTHING
            "#,
            &[&bot_account],
        )
        .await?;

        tx.execute(
            r#"
                INSERT INTO user_bots (user_id, bot_account)
                VALUES (?, ?)
            "#,
            &[&user_id, &bot_account],
        )
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn find_user_by_twitch_account(&self, twitch_account: &str) -> Result<Option<i64>> {
        let twitch_account = twitch_account.to_string();
        let mut guard = self.0.get().await?;
        let tx = guard.transaction().await?;
        let id_opt = tx
            .query_opt(
                r#"
                    SELECT id FROM users
                    WHERE users.twitch_id == ?
                "#,
                &[&twitch_account],
            )
            .await?;

        tx.commit().await?;
        Ok(id_opt.map(|r| r.get::<_, i32>(0).into()))
    }
}
