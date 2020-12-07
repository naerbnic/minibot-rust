use futures::channel::oneshot;
use futures::prelude::*;
use futures::select;
use minibot_common::proof_key;
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use url::Url;
use warp::Filter;

use crate::AuthnError;

fn make_local_http_auth_url(
    auth_url: &Url,
    addr: &SocketAddr,
    challenge: &proof_key::Challenge,
) -> Url {
    let mut auth_url = auth_url.clone();
    assert!(
        auth_url.query() == None,
        "target_url must not have an existing query"
    );

    #[derive(Serialize)]
    struct Query<'a> {
        auth_method: String,
        redirect_uri: String,
        challenge: &'a proof_key::Challenge,
    }

    let query = Query {
        auth_method: "local_http".to_string(),
        redirect_uri: format!("http://{addr}/callback", addr = addr),
        challenge,
    };

    auth_url.set_query(Some(&serde_urlencoded::to_string(query).unwrap()));
    auth_url
}

pub fn make_token_auth_url(auth_url: &Url) -> (Url, proof_key::Verifier) {
    let (challenge, verifier) = proof_key::generate_pair();

    let mut auth_url = auth_url.clone();
    assert!(
        auth_url.query() == None,
        "target_url must not have an existing query"
    );

    #[derive(Serialize)]
    struct Query<'a> {
        auth_method: String,
        challenge: &'a proof_key::Challenge,
    }

    let query = Query {
        auth_method: "token".to_string(),
        challenge: &challenge,
    };

    auth_url.set_query(Some(&serde_urlencoded::to_string(query).unwrap()));
    (auth_url, verifier)
}

pub async fn exchange_confirm_token(
    client: &reqwest::Client,
    exchange_url: &Url,
    token: &str,
    verifier: &proof_key::Verifier,
) -> Result<String, AuthnError> {
    #[derive(Serialize)]
    struct Query<'a> {
        token: &'a str,
        verifier: &'a proof_key::Verifier,
    }

    let query = Query { token, verifier };

    let response = client
        .post(exchange_url.as_str())
        .query(&query)
        .send()
        .await?
        .error_for_status()?;

    #[derive(Deserialize)]
    struct Body {
        access_token: String,
    }

    let Body { access_token } = response.json::<Body>().await?;

    Ok(access_token)
}

pub async fn get_local_http_access_token<E>(
    client: &reqwest::Client,
    deadline: std::time::Instant,
    auth_url: &Url,
    exchange_url: &Url,
    open_browser_func: impl FnOnce(Url) -> Result<(), E>,
) -> Result<String, AuthnError>
where
    E: std::error::Error + Send + Sync + 'static,
{
    let (addr, server) = run_server(tokio::time::delay_until(deadline.into()));
    let (challenge, verifier) = proof_key::generate_pair();

    open_browser_func(make_local_http_auth_url(auth_url, &addr, &challenge))
        .map_err(|e| AuthnError::OpenBrowserError(Box::new(e)))?;

    let token = server.await.ok_or(AuthnError::DidNotGetToken)?;

    assert!(
        exchange_url.query() == None,
        "exchange_url must not have an existing query"
    );

    exchange_confirm_token(client, exchange_url, &token, &verifier).await
}

fn server_route(
    finished_dest: oneshot::Sender<()>,
    token_dest: oneshot::Sender<String>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    #[derive(Deserialize)]
    struct CallbackQuery {
        token: String,
    }

    let token_dest = Arc::new(Mutex::new(Some((finished_dest, token_dest))));

    warp::path!("callback")
        .and(warp::get())
        .and(warp::query::<CallbackQuery>())
        .map(move |q| {
            let CallbackQuery { token } = q;
            if let Some((finished_dest, token_dest)) = token_dest.lock().unwrap().take() {
                let _ = finished_dest.send(());
                let _ = token_dest.send(token);
            }
            "You win!".to_string()
        })
}

fn run_server(
    shutdown: impl Future<Output = ()> + Send + Unpin + 'static,
) -> (SocketAddr, impl Future<Output = Option<String>>) {
    let (token_tx, mut token_rx) = oneshot::channel();
    let (finished_tx, finished_rx) = oneshot::channel();

    let shutdown = async move {
        let mut shutdown = shutdown.fuse();
        let mut finished = finished_rx.fuse();

        select! {
            _ = shutdown => {}
            _ = finished => {}
        }
    };
    let server = warp::serve(server_route(finished_tx, token_tx));
    let (addr, server_future) =
        server.bind_with_graceful_shutdown((Ipv4Addr::LOCALHOST, 0), shutdown);

    (addr, async move {
        server_future.await;
        token_rx.close();
        token_rx.try_recv().unwrap_or(None)
    })
}
