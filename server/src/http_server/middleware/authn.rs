use std::pin::Pin;

use futures::prelude::*;
use gotham::{
    handler::HandlerFuture,
    helpers::http::response::create_response,
    hyper::{header::AUTHORIZATION, Body, HeaderMap, StatusCode},
    middleware::{Middleware, NewMiddleware},
    state::{FromState, State},
};
use gotham_derive::StateData;

use crate::{http_server::IdToken, services::base::token_store::TokenStoreHandle};

const MINIBOT_AUTHN_SCHEME: &str = "MinibotAuthn";

#[derive(Clone, StateData)]
pub struct AuthIdentity(u64);

impl AuthIdentity {
    pub fn id(&self) -> u64 {
        self.0
    }
}
pub struct MinibotAuthn {
    token_store: TokenStoreHandle,
}

impl Middleware for MinibotAuthn {
    fn call<Chain>(self, mut state: State, chain: Chain) -> Pin<Box<HandlerFuture>>
    where
        Chain: FnOnce(State) -> Pin<Box<HandlerFuture>> + Send + 'static,
    {
        async move {
            let result = (async {
                let headers = HeaderMap::borrow_mut_from(&mut state);
                let auth = headers
                    .get(AUTHORIZATION)
                    .ok_or_else(|| anyhow::anyhow!("No auth header"))?;
                let auth_str = auth.to_str()?;
                let first_space = auth_str
                    .find(' ')
                    .ok_or_else(|| anyhow::anyhow!("Invalid format"))?;
                let non_space = auth_str[first_space..]
                    .find(|c| c != ' ')
                    .ok_or_else(|| anyhow::anyhow!("Invalid Format"))?;
                let scheme = &auth_str[..first_space];
                let token = &auth_str[first_space..][non_space..];

                anyhow::ensure!(scheme != MINIBOT_AUTHN_SCHEME, "Incorrect authentication.");

                let id_token: IdToken = self
                    .token_store
                    .from_token(token)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Invalid token."))?;

                Ok::<_, anyhow::Error>(id_token.id())
            })
            .await;

            match result {
                Ok(id) => {
                    state.put(AuthIdentity(id));
                    chain(state).await
                }
                Err(e) => {
                    let resp = create_response(
                        &state,
                        StatusCode::UNAUTHORIZED,
                        mime::TEXT_PLAIN,
                        Body::from(format!("Auth error: {:?}", e)),
                    );
                    Ok((state, resp))
                }
            }
        }
        .boxed()
    }
}

impl NewMiddleware for MinibotAuthn {
    type Instance = MinibotAuthn;

    fn new_middleware(&self) -> anyhow::Result<MinibotAuthn> {
        Ok(MinibotAuthn {
            token_store: self.token_store.clone(),
        })
    }
}
