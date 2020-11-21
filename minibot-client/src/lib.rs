mod access_token;

use minibot_common::secure::SecureString;
use url::Url;

pub use access_token::get_access_token as run_client;

#[derive(thiserror::Error, Debug)]
pub enum AuthnError {
    #[error("Did not get a token from minibot.")]
    DidNotGetToken,

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error(transparent)]
    OpenBrowserError(Box<dyn std::error::Error + Send + Sync>),
}

/// Info for connecting to a minibot server.
#[derive(Clone, Debug)]
pub struct Server {
    auth_url: Url,
    exchange_url: Url,
    ws_url: Url,
}

impl Server {
    pub fn new(server_addr: &str) -> Self {
        let server_addr = url::Url::parse(&server_addr).unwrap();
        
        Server {
            auth_url: server_addr.join("login").unwrap(),
            exchange_url: server_addr.join("confirm").unwrap(),
            ws_url: server_addr.join("connect").unwrap(),
        }
    }
    pub async fn authenticate<F, E>(
        &self,
        deadline: std::time::Instant,
        open_browser_func: F,
    ) -> Result<ClientAuthn, AuthnError>
    where
        F: FnOnce(&str) -> Result<(), E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let client = reqwest::Client::new();
        let token = access_token::get_access_token(
            &client,
            deadline,
            &self.auth_url,
            &self.exchange_url,
            |url| open_browser_func(&url.as_str()),
        )
        .await?;

        Ok(ClientAuthn(token.into()))
    }

    pub async fn connect(&self, authn: &ClientAuthn) {

    }
}

#[derive(Debug, Clone)]
pub struct ClientAuthn(SecureString);
