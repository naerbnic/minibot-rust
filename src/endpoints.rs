pub mod login {
    use crate::filters::cloned;
    use crate::handlers::{handle_start_auth_request, OAuthConfig};
    use crate::services::AuthService;
    use futures::prelude::*;
    use minibot_common::proof_key::Challenge;
    use serde::Deserialize;
    use std::sync::Arc;
    use warp::{get, path, query, Filter, Rejection};

    #[derive(Deserialize, Debug)]
    pub struct Query {
        redirect_uri: String,
        challenge: Challenge,
    }

    #[derive(Debug)]
    pub struct Error(anyhow::Error);

    impl warp::reject::Reject for Error {}

    pub fn endpoint(
        oauth_config: Arc<OAuthConfig>,
        auth_service: Arc<AuthService>,
    ) -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {
        path!("login")
            .and(get())
            .and(query::<Query>())
            .and(cloned(oauth_config))
            .and(cloned(auth_service))
            .and_then(|q: Query, oauth, auth_service| {
                handle_start_auth_request(
                    q.redirect_uri.clone(),
                    q.challenge.clone(),
                    auth_service,
                    oauth,
                )
                .map_err(|e| warp::reject::custom(Error(e)))
            })
            .boxed()
    }
}

pub mod callback {
    use crate::filters::cloned;
    use crate::handlers::handle_oauth_callback;
    use crate::services::{AuthConfirmService, AuthService};
    use crate::util::types::OAuthScopeList;
    use futures::prelude::*;
    use serde::Deserialize;
    use std::sync::Arc;
    use warp::{get, path, query, Filter, Rejection};

    #[derive(Deserialize, Debug)]
    pub struct Query {
        code: String,
        scope: OAuthScopeList,
        state: String,
    }

    #[derive(Debug)]
    pub struct Error(anyhow::Error);

    impl warp::reject::Reject for Error {}

    pub fn endpoint(
        auth_service: Arc<AuthService>,
        confirm_service: Arc<AuthConfirmService>,
    ) -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {
        path!("callback")
            .and(get())
            .and(query::<Query>())
            .and(cloned(auth_service))
            .and(cloned(confirm_service))
            .and_then(|q: Query, auth, confirm| {
                handle_oauth_callback(q.code.clone(), q.state.clone(), auth, confirm)
                    .map_err(|e| warp::reject::custom(Error(e)))
            })
    }
}

pub mod confirm {
    use crate::filters::cloned;
    use crate::handlers::OAuthConfig;
    use crate::services::AuthConfirmService;
    use minibot_common::proof_key;
    use serde::{Serialize, Deserialize};
    use std::sync::Arc;
    use warp::{path, post, query, Filter, Rejection};

    #[derive(Deserialize, Debug)]
    pub struct Query {
        token: String,
        verifier: proof_key::Verifier,
    }

    #[derive(thiserror::Error, Debug)]
    #[error(transparent)]
    pub struct Error(#[from] anyhow::Error);

    impl warp::reject::Reject for Error {}

    async fn handle_endpoint(
        q: &Query,
        twitch_config: &Arc<OAuthConfig>,
        confirm: &Arc<AuthConfirmService>,
    ) -> anyhow::Result<impl warp::Reply> {
        #[derive(Serialize)]
        struct TokenQuery<'a> {
            client_id: &'a str,
            client_secret: &'a str,
            code: &'a str,
            grant_type: &'a str,
            redirect_uri: &'a str,
        }

        #[derive(Deserialize, Debug)]
        struct TokenResponse {
            access_token: String,
            refresh_token: String,
            expires_in: u64,
            scope: Option<Vec<String>>,
            id_token: Option<String>,
            token_type: String,
        }

        let auth_confirm_info = confirm.from_token(&q.token).await?;
        proof_key::verify_challenge(&auth_confirm_info.challenge, &q.verifier)?;
        let client = reqwest::Client::new();
        let response = client.post(&twitch_config.provider.token_endpoint).query(&TokenQuery {
            client_id: &twitch_config.client.client_id,
            client_secret: &twitch_config.client.client_secret,
            code: &auth_confirm_info.code,
            grant_type: "authorization_code",
            redirect_uri: &twitch_config.client.redirect_url,
        }).send().await?;

        let token_response: TokenResponse = response.json().await?;
        log::info!("Retrieved token response: {:#?}", token_response);

        Ok("Hello!")
    }

    pub fn endpoint(
        twitch_config: Arc<OAuthConfig>,
        confirm_service: Arc<AuthConfirmService>,
    ) -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {
        path!("confirm")
            .and(post())
            .and(query::<Query>())
            .and(cloned(twitch_config))
            .and(cloned(confirm_service))
            .and_then(
                |q: Query, twitch_config, confirm: Arc<AuthConfirmService>| async move {
                    handle_endpoint(&q, &twitch_config, &confirm)
                        .await
                        .map_err(|e| warp::reject::custom(Error(e)))
                },
            )
    }
}
