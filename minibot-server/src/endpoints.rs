pub mod login {
    use crate::filters::cloned;
    use crate::handlers::{handle_start_auth_request, OAuthConfig};
    use crate::services::{token_service::TokenServiceHandle, AuthRequestInfo};
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
        auth_service: TokenServiceHandle<AuthRequestInfo>,
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
    use crate::services::{token_service::TokenServiceHandle, AuthConfirmInfo, AuthRequestInfo};
    use crate::util::types::OAuthScopeList;
    use futures::prelude::*;
    use serde::Deserialize;
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
        auth_service: TokenServiceHandle<AuthRequestInfo>,
        confirm_service: TokenServiceHandle<AuthConfirmInfo>,
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
    use crate::services::twitch_token::TwitchTokenService;
    use crate::services::{token_service::TokenServiceHandle, AuthConfirmInfo};
    use minibot_common::proof_key;
    use serde::Deserialize;
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
        twitch_token_service: &Arc<TwitchTokenService>,
        confirm: &TokenServiceHandle<AuthConfirmInfo>,
    ) -> anyhow::Result<impl warp::Reply> {
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
        proof_key::verify_challenge(&auth_confirm_info.challenge, &q.verifier)?;
        let response = twitch_token_service
            .exchange_code(&auth_confirm_info.code)
            .await?;
        log::info!("Retrieved token response: {:#?}", response);

        Ok("Hello!")
    }

    pub fn endpoint(
        twitch_token_service: Arc<TwitchTokenService>,
        confirm_service: TokenServiceHandle<AuthConfirmInfo>,
    ) -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {
        path!("confirm")
            .and(post())
            .and(query::<Query>())
            .and(cloned(twitch_token_service))
            .and(cloned(confirm_service))
            .and_then(|q: Query, twitch_config, confirm| async move {
                handle_endpoint(&q, &twitch_config, &confirm)
                    .await
                    .map_err(|e| warp::reject::custom(Error(e)))
            })
    }
}
