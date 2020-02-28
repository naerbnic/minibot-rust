pub mod login {
    use crate::filters::cloned;
    use crate::handlers::{handle_start_auth_request, OAuthConfig};
    use crate::services::AuthService;
    use futures::prelude::*;
    use serde::Deserialize;
    use std::sync::Arc;
    use warp::{get, path, query, Filter, Rejection};

    #[derive(Deserialize, Debug)]
    pub struct Query {
        redirect_uri: String,
        challenge: String,
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
            }).boxed()
    }
}

pub mod callback {
    use crate::util::types::OAuthScopeList;
    use serde::Deserialize;
    use warp::{get, path, query, Filter, Rejection};

    #[derive(Deserialize, Debug)]
    pub struct Query {
        code: String,
        scope: OAuthScopeList,
        state: String,
    }

    pub fn endpoint() -> impl Filter<Extract = (Query,), Error = Rejection> {
        path!("callback").and(get()).and(query::<Query>())
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
