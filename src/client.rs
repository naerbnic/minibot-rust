use crate::util::proof_key;
use futures::channel::oneshot;
use futures::future::BoxFuture;
use futures::prelude::*;
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use url::Url;
use warp::Filter;

pub struct Client {
    auth_url: Url,
    exchange_url: Url,
    server_future: BoxFuture<'static, anyhow::Result<String>>,
    verifier: proof_key::Verifier,
}

impl Client {
    pub fn new(auth_url: &str, exchange_url: &str) -> anyhow::Result<Self> {
        let mut auth_url = Url::parse(auth_url)?;
        anyhow::ensure!(
            auth_url.query() == None,
            "target_url must not have an existing query"
        );
        let exchange_url = Url::parse(exchange_url)?;
        anyhow::ensure!(
            exchange_url.query() == None,
            "exchange_url must not have an existing query"
        );
        let server = Server::new();
        let addr = server.addr();
        let (challenge, verifier) = proof_key::generate_pair();

        #[derive(Serialize)]
        struct Query {
            redirect_url: String,
            challenge: proof_key::Challenge,
        }

        let query = Query {
            redirect_url: format!("http://{addr}/callback", addr = addr),
            challenge,
        };

        auth_url.set_query(Some(&serde_urlencoded::to_string(query)?));

        Ok(Client {
            auth_url,
            exchange_url,
            server_future: server.run().boxed(),
            verifier,
        })
    }

    fn target_url(&self) -> &Url {
        &self.auth_url
    }

    pub async fn run(self, client: &reqwest::Client) -> anyhow::Result<String> {
        let Client {
            server_future,
            verifier,
            exchange_url,
            ..
        } = self;
        let token = server_future.await?;

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

        Ok(access_token)
    }
}

#[derive(Clone, Debug)]
struct SyncSlot<T>(Arc<Mutex<Option<T>>>);

impl<T> SyncSlot<T>
where
    T: Clone + Send,
{
    pub fn new() -> Self {
        SyncSlot(Arc::new(Mutex::new(None)))
    }

    pub fn write(&self, val: T) {
        let mut guard = self.0.lock().unwrap();
        *guard = Some(val);
    }

    pub fn read_clone(&self) -> Option<T> {
        let guard = self.0.lock().unwrap();
        guard.as_ref().cloned()
    }
}

#[derive(Deserialize)]
struct CallbackQuery {
    token: String,
}

fn server_route(
    token_dest: SyncSlot<String>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("callback")
        .and(warp::get())
        .and(warp::query::<CallbackQuery>())
        .map(move |q| {
            let CallbackQuery { token } = q;
            token_dest.write(token);
            "You win!".to_string()
        })
}

struct Server {
    addr: SocketAddr,
    halt_sender: oneshot::Sender<()>,
    token_source: SyncSlot<String>,
    server_future: futures::future::BoxFuture<'static, ()>,
}

impl Server {
    fn new() -> Self {
        let (halt_tx, halt_rx) = oneshot::channel();
        let token_slot = SyncSlot::new();
        let server = warp::serve(server_route(token_slot.clone()));
        let (addr, server_future) =
            server.bind_with_graceful_shutdown((Ipv4Addr::LOCALHOST, 0), async {
                halt_rx.await.ok();
            });

        Server {
            addr,
            halt_sender: halt_tx,
            token_source: token_slot,
            server_future: server_future.boxed(),
        }
    }

    fn addr(&self) -> &SocketAddr {
        &self.addr
    }

    async fn run(self) -> anyhow::Result<String> {
        let Server {
            server_future,
            halt_sender,
            token_source,
            ..
        } = self;
        let server_future = server_future.fuse();
        futures::pin_mut!(server_future);

        let timeout = async {
            tokio::time::delay_until(
                tokio::time::Instant::now() + std::time::Duration::from_secs(3),
            )
            .await;
            let _ = halt_sender.send(());
        };

        let timeout = timeout.fuse();
        futures::pin_mut!(timeout);

        let mut server_running = true;
        while server_running {
            futures::select! {
                _ = server_future => { server_running = false; }
                _ = timeout => {}
            }
        }

        let token = match token_source.read_clone() {
            Some(token) => token,
            None => anyhow::bail!("Attempt to recieve token timed out."),
        };

        Ok(token)
    }
}
