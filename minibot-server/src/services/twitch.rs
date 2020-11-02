mod token_source {
    use futures::channel::oneshot;
    use futures::prelude::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct State {
        curr_tokens: usize,
        curr_running: usize,
        notifiers: VecDeque<oneshot::Sender<()>>,
    }

    pub struct TokenSource {
        max_tokens: usize,
        state: Mutex<State>,
    }

    impl TokenSource {
        pub fn new(max_tokens: usize) -> Self {
            let state = State {
                curr_tokens: max_tokens,
                curr_running: 0,
                notifiers: VecDeque::new(),
            };

            TokenSource {
                max_tokens,
                state: Mutex::new(state),
            }
        }

        async fn take_token(&self) {
            let rx = {
                let mut state = self.state.lock().unwrap();
                if state.curr_tokens > 0 {
                    state.curr_tokens -= 1;
                    state.curr_running += 1;
                    return;
                } else {
                    let (tx, rx) = oneshot::channel();
                    state.notifiers.push_back(tx);
                    rx
                }
            };

            rx.await.unwrap()
        }

        fn restore_slot(&self) {
            let mut state = self.state.lock().unwrap();
            state.curr_running -= 1;
        }

        pub async fn run_with_token<F: Future>(&self, fut: F) -> F::Output {
            self.take_token().await;
            let result = fut.await;
            self.restore_slot();
            result
        }

        pub fn add_tokens(&self, num_tokens: usize) {
            if num_tokens == 0 {
                return;
            }
            let mut state = self.state.lock().unwrap();
            let mut num_tokens = std::cmp::min(
                num_tokens,
                self.max_tokens.saturating_sub(state.curr_running),
            );
            while let Some(tx) = state.notifiers.pop_front() {
                // Ignore waiters that were dropped.
                if tx.send(()).is_err() {
                    continue;
                }
                state.curr_running += 1;
                num_tokens -= 1;

                if num_tokens == 0 {
                    break;
                }
            }

            state.curr_tokens += num_tokens;
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;
        use std::sync::Mutex;

        #[tokio::test]
        async fn runs_all_available_tokens() {
            let counter = Mutex::new(Vec::new());

            let source = TokenSource::new(3);

            let mut futures = Vec::new();

            for i in 0..3 {
                let counter = &counter;
                futures.push(source.run_with_token(async move { counter.lock().unwrap().push(i) }))
            }

            futures::future::join_all(futures).await;

            assert_eq!(counter.into_inner().unwrap(), vec![0, 1, 2])
        }
    }
}

mod throttled_token_source {
    use super::token_source::TokenSource;
    use futures::future::{abortable, AbortHandle};
    use futures::prelude::*;
    use std::sync::Arc;

    pub struct ThrottledTokenSource {
        source: Arc<TokenSource>,
        timer_handle: AbortHandle,
    }

    impl ThrottledTokenSource {
        pub fn new(max_tokens: usize, interval: std::time::Duration) -> Self {
            let source = Arc::new(TokenSource::new(max_tokens));

            let (timer_task, timer_handle) = abortable({
                let source = source.clone();
                async move {
                    tokio::time::interval(interval)
                        .for_each(move |_| {
                            let source = source.clone();
                            async move { source.add_tokens(1) }
                        })
                        .await;
                }
            });

            tokio::spawn(async move {
                let _ = timer_task.await;
            });

            ThrottledTokenSource {
                source,
                timer_handle,
            }
        }

        pub async fn run_with_token<F: Future>(&self, task: F) -> F::Output {
            self.source.run_with_token(task).await
        }
    }

    impl Drop for ThrottledTokenSource {
        fn drop(&mut self) {
            self.timer_handle.abort();
        }
    }
}

use crate::config::OAuthConfig;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::sync::Arc;

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub enum BroadcasterType {
    Normal,
    Partner,
    Affiliate,
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub enum UserType {
    Normal,
    Staff,
    Admin,
    GlobalMod,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TwitchUser {
    broadcaster_type: BroadcasterType,
    description: String,
    display_name: String,
    email: Option<String>,
    id: String,
    name: String,
    offline_image_url: String,
    profile_image_url: String,
    user_type: UserType,
    view_count: u64,
}

/// Many responses from twitch are wrapped in an object with a single "data" array field. This acts as a wrapper for that.
#[derive(Clone, Serialize, Deserialize, Debug)]
struct DataWrapper<T> {
    data: Vec<T>,
}

impl<T> DataWrapper<T> {
    pub fn into_vec(self) -> Vec<T> {
        let DataWrapper { data } = self;
        data
    }
}

pub struct AuthToken {
    api_token: String,
}

#[async_trait::async_trait]
pub trait TwitchClient {
    async fn get_user_info(
        &self,
        auth_token: &AuthToken,
        id: &str,
    ) -> Result<TwitchUser, anyhow::Error>;
}

pub struct HttpTwitchClient<T> {
    client: T,
    config: Arc<OAuthConfig>,
}

impl<T: AsRef<reqwest::Client> + Sync> HttpTwitchClient<T> {
    pub async fn call_api<Out: DeserializeOwned, Q: Serialize + ?Sized>(
        &self,
        auth_token: &AuthToken,
        method: reqwest::Method,
        path: &str,
        query_args: &Q,
    ) -> anyhow::Result<Out> {
        let client = self.client.as_ref();
        let endpoint = self.config.api_endpoint();
        Ok(client
            .request(method, &endpoint.join(path).unwrap().to_string())
            .header("Authorization", format!("Bearer {}", auth_token.api_token))
            .query(query_args)
            .send()
            .await?
            .json::<Out>()
            .await?)
    }
}

#[async_trait::async_trait]
impl<T: AsRef<reqwest::Client> + Sync> TwitchClient for HttpTwitchClient<T> {
    async fn get_user_info(&self, auth_token: &AuthToken, id: &str) -> anyhow::Result<TwitchUser> {
        let mut users = self
            .call_api::<DataWrapper<TwitchUser>, _>(
                auth_token,
                reqwest::Method::GET,
                "helix/users",
                &[("id", id)],
            )
            .await?
            .into_vec();

        anyhow::ensure!(users.len() == 1, "Expected a single user to be returned");

        Ok(users.pop().unwrap())
    }
}
