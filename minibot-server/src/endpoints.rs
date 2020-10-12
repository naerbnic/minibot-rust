use crate::handlers::{handle_oauth_callback, handle_start_auth_request, OAuthConfig};
use crate::reqwest_middleware::ClientHandle;
use crate::services::twitch_token::{TwitchTokenHandle, TwitchTokenService};
use crate::services::{
    token_service::{TokenService, TokenServiceHandle},
    AuthConfirmInfo, AuthRequestInfo,
};
use crate::util::types::OAuthScopeList;
use gotham::handler::HandlerError;
use gotham::hyper::{Body, Response};
use gotham::middleware::state::StateMiddleware;
use gotham::pipeline::{new_pipeline, single::single_pipeline};
use gotham::router::builder::*;
use gotham::router::Router;
use gotham::state::{FromState, State};
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

pub fn router(
    oauth_config: OAuthConfig,
    twitch_token_service: TwitchTokenHandle,
    request_token_service: TokenServiceHandle<AuthRequestInfo>,
    confirm_token_service: TokenServiceHandle<AuthConfirmInfo>,
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
