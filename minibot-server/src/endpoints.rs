use crate::handlers::{handle_oauth_callback, handle_start_auth_request, OAuthConfig};
use crate::reqwest_middleware::ClientHandle;
use crate::services::twitch_token::{TwitchTokenHandle, TwitchTokenService};
use crate::services::{
    token_service::{TokenService, TokenServiceHandle},
    AuthConfirmInfo, AuthRequestInfo,
};
use crate::util::types::scopes::OAuthScopeList;
use crate::net::ws;
use futures::prelude::*;
use gotham::handler::HandlerError;
use gotham::hyper::{Body, HeaderMap, Response};
use gotham::middleware::state::StateMiddleware;
use gotham::pipeline::{new_pipeline, single::single_pipeline};
use gotham::router::builder::*;
use gotham::router::Router;
use gotham::state::{request_id, FromState, State};
use gotham_derive::{StateData, StaticResponseExtender};
use minibot_common::proof_key::{Challenge, Verifier};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, StateData, StaticResponseExtender)]
pub struct LoginQuery {
    redirect_uri: String,
    challenge: Challenge,
}

async fn login_handler(state: &mut State) -> Result<Response<Body>, HandlerError> {
    let oauth_config = OAuthConfig::borrow_from(state).clone();
    let request_token_service = TokenServiceHandle::<AuthRequestInfo>::take_from(state);
    let login_query = LoginQuery::take_from(state);

    let redirect = handle_start_auth_request(
        login_query.redirect_uri.clone(),
        login_query.challenge.clone(),
        &*request_token_service,
        &oauth_config,
    )
    .await?;

    log::info!("Redirect to Twitch auth endpoint: {}", redirect);

    Ok(gotham::helpers::http::response::create_temporary_redirect(
        state, redirect,
    ))
}

#[derive(Deserialize, Debug, StateData, StaticResponseExtender)]
pub struct CallbackQuery {
    code: String,
    scope: OAuthScopeList,
    state: String,
}

async fn callback_handler(state: &mut State) -> Result<Response<Body>, HandlerError> {
    let request_token_service = TokenServiceHandle::<AuthRequestInfo>::take_from(state);
    let confirm_token_service = TokenServiceHandle::<AuthConfirmInfo>::take_from(state);
    let callback_query = CallbackQuery::take_from(state);

    let redirect = handle_oauth_callback(
        callback_query.code.clone(),
        callback_query.state.clone(),
        &*request_token_service,
        &*confirm_token_service,
    )
    .await?;

    log::info!("Redirect to local callback: {}", redirect);

    Ok(gotham::helpers::http::response::create_temporary_redirect(
        state, redirect,
    ))
}

#[derive(Deserialize, Debug, StateData, StaticResponseExtender)]
pub struct ConfirmQuery {
    token: String,
    verifier: Verifier,
}

#[derive(Serialize, Debug)]
pub struct ConfirmResponse {
    access_token: String,
}

async fn handle_endpoint(
    client: &reqwest::Client,
    q: &ConfirmQuery,
    twitch_token_service: &TwitchTokenService,
    confirm: &dyn TokenService<AuthConfirmInfo>,
) -> anyhow::Result<String> {
    #[derive(Deserialize, Debug)]
    struct TokenResponse {
        access_token: String,
        refresh_token: String,
        expires_in: u64,
        scope: Option<Vec<String>>,
        id_token: Option<String>,
        token_type: String,
    }

    let auth_confirm_info = match confirm.from_token(&q.token).await? {
        Some(info) => info,
        None => anyhow::bail!("Could not find confirmation."),
    };
    q.verifier.verify(&auth_confirm_info.challenge)?;
    let response = twitch_token_service
        .exchange_code(client, &auth_confirm_info.code)
        .await?;
    println!("Retrieved token response: {:#?}", response);

    Ok(serde_json::to_string(&ConfirmResponse {
        access_token: "Hello".to_string(),
    })?)
}

async fn confirm_handler(state: &mut State) -> Result<Response<Body>, HandlerError> {
    let reqwest_client = ClientHandle::take_from(state);
    let confirm_token_service = TokenServiceHandle::<AuthConfirmInfo>::take_from(state);
    let twitch_token_service = TwitchTokenHandle::take_from(state);
    let confirm_query = ConfirmQuery::take_from(state);

    let output = handle_endpoint(
        &reqwest_client,
        &confirm_query,
        &*twitch_token_service,
        &*confirm_token_service,
    )
    .await?;

    Ok(Response::builder().body(Body::from(output))?)
}

async fn socket_handler(state: &mut State) -> Result<Response<Body>, HandlerError> {
    let body = Body::take_from(state);
    let headers = HeaderMap::take_from(state);
    let mut socket_sink =
        ValueWrapper::<futures::channel::mpsc::Sender<ws::WebSocket>>::borrow_from(state)
            .clone_inner();

    let req_id = request_id(state).to_owned();

    if ws::requested(&headers) {
        let (response, ws_future) = ws::accept(&headers, body)?;

        tokio::spawn(async move {
            match ws_future.await {
                Ok(ws) => {
                    log::info!("{}: WebSocket connection started.", req_id);
                    let _ = socket_sink.send(ws).await;
                }
                Err(e) => {
                    log::error!("{}: Error while connecting to websocket: {}", req_id, e);
                }
            }
        });

        Ok(response)
    } else {
        Ok(ws::upgrade_required_response())
    }
}

pub trait CloneSink<V>:
    Sink<V, Error = futures::channel::mpsc::SendError> + Send + Sync + 'static
{
    fn box_clone(&self) -> Box<dyn CloneSink<V>>;
}

impl<T, V> CloneSink<V> for T where
    T: Sink<V, Error = futures::channel::mpsc::SendError> + Send + Sync + Clone + 'static + ?Sized
{
    fn box_clone(&self) -> Box<dyn CloneSink<V>> {
        Box::new(self.clone())
    }
}

impl<V> Clone for Box<dyn CloneSink<V>> where V: 'static {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

#[derive(StateData)]
struct ValueWrapper<T>(std::sync::RwLock<T>)
where
    T: Send + Sync + 'static;

impl<T> Clone for ValueWrapper<T>
where
    T: Send + Clone + Sync + 'static,
{
    fn clone(&self) -> Self {
        let guard = self.0.read().unwrap();
        ValueWrapper(std::sync::RwLock::new(guard.clone()))
    }
}

impl<T> ValueWrapper<T>
where
    T: Send + Sync + 'static,
{
    pub fn new(v: T) -> Self {
        ValueWrapper(std::sync::RwLock::new(v))
    }

    pub fn into_inner(self) -> T {
        let ValueWrapper(rwlock) = self;
        rwlock.into_inner().unwrap()
    }
}

impl<T> ValueWrapper<T>
where
    T: Send + Sync + Clone + 'static,
{
    pub fn clone_inner(&self) -> T {
        let guard = self.0.read().unwrap();
        guard.clone()
    }
}

pub fn router(
    oauth_config: OAuthConfig,
    twitch_token_service: TwitchTokenHandle,
    request_token_service: TokenServiceHandle<AuthRequestInfo>,
    confirm_token_service: TokenServiceHandle<AuthConfirmInfo>,
    socket_sink: Box<dyn CloneSink<ws::WebSocket>>,
) -> Router {
    let (chain, pipelines) = single_pipeline(
        new_pipeline()
            .add(crate::reqwest_middleware::NewReqwestClientMiddleware::new(
                reqwest::Client::new(),
            ))
            .add(StateMiddleware::new(oauth_config))
            .add(StateMiddleware::new(request_token_service))
            .add(StateMiddleware::new(confirm_token_service))
            .add(StateMiddleware::new(twitch_token_service))
            .add(StateMiddleware::new(ValueWrapper::new(socket_sink)))
            .build(),
    );
    build_router(chain, pipelines, |route| {
        route
            .get("/login")
            .with_query_string_extractor::<LoginQuery>()
            .to_async_borrowing(login_handler);

        route
            .get("/callback")
            .with_query_string_extractor::<CallbackQuery>()
            .to_async_borrowing(callback_handler);

        route
            .post("/confirm")
            .with_query_string_extractor::<ConfirmQuery>()
            .to_async_borrowing(confirm_handler)
    })
}
