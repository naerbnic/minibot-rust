use diesel::prelude::*;

use crate::db_handle::{DbHandle, Error as DbError};
use crate::model;

#[derive(Clone, Debug)]
pub struct WithId<T>(i64, T);

impl<T> WithId<T> {
    pub fn new(id: i64, v: T) -> Self {
        WithId(id, v)
    }

    pub fn id(&self) -> i64 {
        self.0
    }
    pub fn value(&self) -> &T {
        &self.1
    }
    pub fn value_mut(&mut self) -> &mut T {
        &mut self.1
    }
    pub fn into_value(self) -> T {
        self.1
    }
}

#[derive(Clone, Debug)]
pub struct User {
    streamer_account: String,
    bot_account: Option<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    DatabaseError(#[from] DbError),
}

pub type Result<T> = std::result::Result<T, Error>;

#[async_trait::async_trait]
pub trait UserService {
    // Standard operations
    async fn create_user(&self, twitch_account: &str) -> Result<i64>;
    async fn set_bot_account(&self, user_id: i64, bot_account: &str) -> Result<()>;
    async fn find_user_by_twitch_account(&self, twitch_account: &str) -> Result<Option<i64>>;
}

pub struct UserServiceImpl(DbHandle);

impl UserServiceImpl {
    pub fn new(db: DbHandle) -> Self {
        UserServiceImpl(db)
    }
}

#[async_trait::async_trait]
impl UserService for UserServiceImpl {
    async fn create_user(&self, twitch_account: &str) -> Result<i64> {
        use crate::schema::twitch_accounts::dsl::*;
        use crate::schema::users::{self, dsl::*};
        let twitch_account = twitch_account.to_string();
        let new_id = self
            .0
            .run_tx(move |conn| {
                diesel::insert_into(twitch_accounts)
                    .values(&model::NewTwitchAccount {
                        id: &twitch_account,
                    })
                    .execute(conn)?;

                diesel::insert_into(users)
                    .values(&model::NewUser {
                        twitch_id: &twitch_account,
                    })
                    .execute(conn)?;

                Ok(users
                    .select(users::id)
                    .order(users::id.desc())
                    .first::<i64>(conn)?)
            })
            .await?;
        Ok(new_id)
    }

    async fn set_bot_account(&self, user_id: i64, bot_account_name: &str) -> Result<()> {
        use crate::schema::twitch_accounts;
        use crate::schema::user_bots;

        let bot_account_name = bot_account_name.to_string();
        self.0
            .run_tx(move |conn| {
                diesel::insert_into(twitch_accounts::table)
                    .values(&model::NewTwitchAccount {
                        id: &bot_account_name,
                    })
                    .execute(conn)?;

                let num_entries = user_bots::table
                    .filter(user_bots::user_id.eq(user_id))
                    .select(diesel::dsl::count_star())
                    .get_result::<i64>(conn)?;

                if num_entries > 0 {
                    diesel::update(user_bots::table.filter(user_bots::user_id.eq(user_id)))
                        .set(user_bots::bot_account.eq(&bot_account_name))
                        .execute(conn)?;
                } else {
                    diesel::insert_into(user_bots::table)
                        .values(&model::NewUserBot {
                            user_id: user_id,
                            bot_account: &bot_account_name,
                        })
                        .execute(conn)?;
                }

                Ok(())
            })
            .await?;
        Ok(())
    }

    async fn find_user_by_twitch_account(&self, twitch_account: &str) -> Result<Option<i64>> {
        use crate::schema::users::dsl::*;

        let twitch_account = twitch_account.to_string();
        let id_opt = self
            .0
            .run_tx(move |conn| {
                Ok(users
                    .filter(twitch_id.eq(twitch_account))
                    .select(id)
                    .get_result::<i64>(conn)
                    .optional()?)
            })
            .await?;

        Ok(id_opt)
    }
}

#[cfg(test)]
mod test {
    use crate::db_handle::DbHandle;
    use super::{UserService, UserServiceImpl};

    #[tokio::test]
    async fn create_account() {
        let handle = DbHandle::new("file::memory:").await.unwrap();
        println!("db created");
        let user_service = UserServiceImpl::new(handle.clone());
        let user_id = user_service.create_user("bob_cratchet").await.unwrap();

        let found_user_id = user_service.find_user_by_twitch_account("bob_cratchet").await.unwrap();
        let unknown_person_id = user_service.find_user_by_twitch_account("ebeneazer_scrooge").await.unwrap();

        assert_eq!(Some(user_id), found_user_id);
        assert_eq!(None, unknown_person_id);
    }
}
