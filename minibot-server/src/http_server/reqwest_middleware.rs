use gotham::handler::HandlerFuture;
use gotham::middleware::{Middleware, NewMiddleware};
use gotham::state::State;
use gotham_derive::StateData;
use reqwest::Client;
use std::pin::Pin;
use std::sync::RwLock;

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
pub struct NewReqwestClientMiddleware(RwLock<Client>);

impl NewReqwestClientMiddleware {
    pub fn new(client: Client) -> Self {
        NewReqwestClientMiddleware(RwLock::new(client))
    }
}

impl NewMiddleware for NewReqwestClientMiddleware {
    type Instance = ReqwestClientMiddleware;

    fn new_middleware(&self) -> anyhow::Result<Self::Instance> {
        let client = {
            let guard = self.0.read().unwrap();
            guard.clone()
        };
        Ok(ReqwestClientMiddleware(client))
    }
}
