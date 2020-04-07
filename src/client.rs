use crate::util::proof_key;
use futures::prelude::*;
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use url::Url;
use warp::Filter;

fn make_auth_url(auth_url: &str, addr: &SocketAddr, challenge: &proof_key::Challenge) -> Url {
    let mut auth_url = Url::parse(auth_url).unwrap();
    assert!(
        auth_url.query() == None,
        "target_url must not have an existing query"
    );

    #[derive(Serialize)]
    struct Query<'a> {
        redirect_url: String,
        challenge: &'a proof_key::Challenge,
    }

    let query = Query {
        redirect_url: format!("http://{addr}/callback", addr = addr),
        challenge,
    };

    auth_url.set_query(Some(&serde_urlencoded::to_string(query).unwrap()));
    auth_url
}

pub fn run_client<'a>(
    client: &'a reqwest::Client,
    deadline: std::time::Instant,
    auth_url: &str,
    exchange_url: &'a str,
) -> (Url, impl Future<Output = anyhow::Result<String>> + 'a) {
    let (addr, server) = run_server(tokio::time::delay_until(deadline.into()));
    let (challenge, verifier) = proof_key::generate_pair();

    let auth_url = make_auth_url(auth_url, &addr, &challenge);

    (auth_url, async move {
        let token = server
            .await
            .ok_or(anyhow::anyhow!("Didn't get token from server"))?;

        let exchange_url = Url::parse(exchange_url).unwrap();
        assert!(
            exchange_url.query() == None,
            "exchange_url must not have an existing query"
        );

        #[derive(Serialize)]
        struct Query {
            token: String,
            verifier: proof_key::Verifier,
        }

        let query = Query { token, verifier };

        let response = client.post(exchange_url).query(&query).send().await?;

        #[derive(Deserialize)]
        struct Body {
            access_token: String,
        }

        let Body { access_token } = response.json::<Body>().await?;

        Ok::<_, anyhow::Error>(access_token)
    })
}

fn server_route(
    token_dest: Arc<Mutex<Option<String>>>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    #[derive(Deserialize)]
    struct CallbackQuery {
        token: String,
    }
    
    warp::path!("callback")
        .and(warp::get())
        .and(warp::query::<CallbackQuery>())
        .map(move |q| {
            let CallbackQuery { token } = q;
            *token_dest.lock().unwrap() = Some(token);
            "You win!".to_string()
        })
}

fn run_server(
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> (SocketAddr, impl Future<Output = Option<String>>) {
    let token_slot = Arc::new(Mutex::new(None));
    let server = warp::serve(server_route(token_slot.clone()));
    let (addr, server_future) =
        server.bind_with_graceful_shutdown((Ipv4Addr::LOCALHOST, 0), shutdown);

    (addr, async move {
        server_future.await;
        token_slot.lock().unwrap().take()
    })
}
