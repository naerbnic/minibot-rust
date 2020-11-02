use std::{panic::AssertUnwindSafe, pin::Pin};

use gotham::{
    handler::HandlerFuture,
    middleware::{Middleware, NewMiddleware},
    state::State,
};
use gotham_derive::StateData;
use reqwest::Client;

#[derive(Clone, StateData)]
pub struct ClientHandle(Client);

impl std::ops::Deref for ClientHandle {
    type Target = Client;
    fn deref(&self) -> &Client {
        &self.0
    }
}

pub struct ReqwestClientMiddleware(Client);

impl Middleware for ReqwestClientMiddleware {
    fn call<Chain>(self, mut state: State, chain: Chain) -> Pin<Box<HandlerFuture>>
    where
        Chain: FnOnce(State) -> Pin<Box<HandlerFuture>> + Send + 'static,
    {
        state.put(ClientHandle(self.0.clone()));
        chain(state)
    }
}

// Note: The RwLock is necessary to meet the requirement that RefUnwindSafe is implemented for the type.
pub struct NewReqwestClientMiddleware(AssertUnwindSafe<Client>);

impl NewReqwestClientMiddleware {
    pub fn new(client: Client) -> Self {
        NewReqwestClientMiddleware(AssertUnwindSafe(client))
    }
}

impl NewMiddleware for NewReqwestClientMiddleware {
    type Instance = ReqwestClientMiddleware;

    fn new_middleware(&self) -> anyhow::Result<Self::Instance> {
        Ok(ReqwestClientMiddleware(self.0.clone()))
    }
}
