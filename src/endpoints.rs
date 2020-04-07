pub mod login {
    use crate::filters::cloned;
    use crate::handlers::{handle_start_auth_request, OAuthConfig};
    use crate::services::AuthService;
    use minibot_common::proof_key::Challenge;
    use futures::prelude::*;
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
    use serde::Deserialize;
    use warp::{path, post, query, Filter, Rejection};

    #[derive(Deserialize, Debug)]
    pub struct Query {
        token: String,
        verifier: String,
    }

    pub fn endpoint() -> impl Filter<Extract = (Query,), Error = Rejection> {
        path!("confirm").and(post()).and(query::<Query>())
    }
}
