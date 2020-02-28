pub mod login {
    use serde::Deserialize;
    use warp::{get, path, query, Filter, Rejection};

    #[derive(Deserialize, Debug)]
    pub struct Query {
        redirect_uri: String,
        challenge: String,
    }

    pub fn endpoint() -> impl Filter<Extract = (Query,), Error = Rejection> {
        path!("login").and(get()).and(query::<Query>())
    }
}

pub mod callback {
    use crate::util::types::OAuthScopeList;
    use serde::Deserialize;
    use warp::{path, post, query, Filter, Rejection};

    #[derive(Deserialize, Debug)]
    pub struct Query {
        code: String,
        scope: OAuthScopeList,
        state: String,
    }

    pub fn endpoint() -> impl Filter<Extract = (Query,), Error = Rejection> {
        path!("callback").and(post()).and(query::<Query>())
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
