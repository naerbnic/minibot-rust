use super::handlers::{handle_start_auth_request, AuthConfirmInfo, AuthMethod, AuthRequestInfo};
use crate::{
    config::oauth,
    http_server::middleware::reqwest::{ClientHandle, NewReqwestClientMiddleware},
    net::ws,
    services::{
        base::token_store::TokenStoreHandle,
        live::twitch_token::{TwitchTokenHandle, TwitchTokenService},
    },
    util::types::scopes::OAuthScopeList,
};

use futures::prelude::*;
use gotham::{
    handler::HandlerError,
    hyper::{Body, Response, StatusCode},
    middleware::{
        logger::{RequestLogger, SimpleLogger},
        state::StateMiddleware,
    },
    pipeline::{new_pipeline, single::single_pipeline},
    router::{builder::*, Router},
    state::{request_id, FromState, State},
};
use gotham_derive::{StateData, StaticResponseExtender};
use minibot_common::proof_key::{Challenge, Verifier};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Serialize, Deserialize, Debug, StateData, StaticResponseExtender)]
pub struct LoginQuery {
    auth_method: String,
    redirect_uri: Option<String>,
    challenge: Challenge,
}

async fn login_handler(state: &mut State) -> Result<Response<Body>, HandlerError> {
    let oauth_config = oauth::Config::borrow_from(state).clone();
    let token_store = TokenStoreHandle::take_from(state);
    let login_query = LoginQuery::take_from(state);

    log::info!("Got login request with params: {:?}", login_query);

    let auth_method: AuthMethod =
        match (login_query.auth_method.as_str(), &login_query.redirect_uri) {
            ("local_http", Some(redirect_uri)) => AuthMethod::LocalHttp {
                redirect_uri: redirect_uri.clone(),
            },
            ("token", None) => AuthMethod::Token,
            _ => return Err(anyhow::anyhow!("Unexpected combination of query fields.").into()),
        };

    log::info!("Found auth_method: {:?}", auth_method);

    let redirect = handle_start_auth_request(
        auth_method,
        login_query.challenge.clone(),
        &token_store,
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
    let token_store = TokenStoreHandle::take_from(state);
    let callback_query = CallbackQuery::take_from(state);

    log::info!("Handling callback with query: {:?}", callback_query);

    let auth_req: AuthRequestInfo = match dbg!(token_store.from_token(&callback_query.state).await)? {
        Some(auth_req) => auth_req,
        None => return Err(anyhow::anyhow!("Could not retrieve token.").into()),
    };

    log::info!("Got auth request info: {:?}", auth_req);

    let confirm_info = AuthConfirmInfo {
        code: callback_query.code.clone(),
        challenge: auth_req.challenge.clone(),
    };

    let token = token_store.to_token(&confirm_info).await?;

    match &auth_req.auth_method {
        AuthMethod::LocalHttp { redirect_uri } => {
            let mut local_redirect_url = Url::parse(&redirect_uri)?;
            local_redirect_url
                .query_pairs_mut()
                .clear()
                .append_pair("token", &token);

            log::info!("Redirect to local callback: {}", local_redirect_url);

            Ok(gotham::helpers::http::response::create_temporary_redirect(
                state,
                local_redirect_url.into_string(),
            ))
        }
        AuthMethod::Token => Ok(gotham::helpers::http::response::create_response(
            state,
            StatusCode::OK,
            mime::TEXT_PLAIN_UTF_8,
            format!(
                concat!(
                    "Your confirmation code is {}\n",
                    "Please copy this to the app which is requesting authentication.\n"
                ),
                token
            ),
        )),
    }
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
    token_store: &TokenStoreHandle,
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

    log::info!("Handling confirmation with params: {:?}", q);

    let auth_confirm_info: AuthConfirmInfo = match dbg!(token_store.from_token(&q.token).await)? {
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
    let token_store = TokenStoreHandle::take_from(state);
    let twitch_token_service = TwitchTokenHandle::take_from(state);
    let confirm_query = ConfirmQuery::take_from(state);

    let output = handle_endpoint(
        &reqwest_client,
        &confirm_query,
        &*twitch_token_service,
        &token_store,
    )
    .await?;

    Ok(Response::builder().body(Body::from(output))?)
}

async fn socket_handler(state: &mut State) -> Result<Response<Body>, HandlerError> {
    let mut socket_sink =
        ValueWrapper::<futures::channel::mpsc::Sender<ws::WebSocket>>::borrow_from(state)
            .clone_inner();

    let req_id = request_id(state).to_owned();

    if ws::requested(state) {
        let (response, ws_future) = ws::accept(state)?;

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

impl<T, V> CloneSink<V> for T
where
    T: Sink<V, Error = futures::channel::mpsc::SendError> + Send + Sync + Clone + 'static + ?Sized,
{
    fn box_clone(&self) -> Box<dyn CloneSink<V>> {
        Box::new(self.clone())
    }
}

impl<V> Clone for Box<dyn CloneSink<V>>
where
    V: 'static,
{
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
    oauth_config: oauth::Config,
    twitch_token_service: TwitchTokenHandle,
    token_store: TokenStoreHandle,
    socket_sink: Box<dyn CloneSink<ws::WebSocket>>,
) -> Router {
    let (chain, pipelines) = single_pipeline(
        new_pipeline()
            .add(SimpleLogger::new(log::Level::Info))
            .add(RequestLogger::new(log::Level::Info))
            .add(NewReqwestClientMiddleware::new(reqwest::Client::new()))
            .add(StateMiddleware::new(oauth_config))
            .add(StateMiddleware::new(token_store))
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
