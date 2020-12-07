mod access_token;

use minibot_common::{
    future::pipe::PipeEnd,
    net::{
        rpc::{ClientChannel, Command, CommandError, CommandHandler, SendCommandError},
        start_websocket_rpc,
    },
    proof_key,
    secure::SecureString,
};

use tokio_tungstenite::{
    connect_async,
    tungstenite::{self, client::IntoClientRequest, http},
};
use url::Url;

pub use access_token::get_local_http_access_token as run_client;

#[derive(thiserror::Error, Debug)]
pub enum AuthnError {
    #[error("Did not get a token from minibot.")]
    DidNotGetToken,

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error(transparent)]
    OpenBrowserError(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(thiserror::Error, Debug)]
pub enum ConnectError {
    #[error(transparent)]
    Tungstenite(#[from] tungstenite::Error),

    #[error(transparent)]
    OpenBrowserError(Box<dyn std::error::Error + Send + Sync>),
}

pub struct NullCommandHandler;

impl CommandHandler for NullCommandHandler {
    fn start_command(
        &mut self,
        _method: &str,
        _payload: &serde_json::Value,
        _output: futures::channel::mpsc::Sender<serde_json::Value>,
        _cancel: minibot_common::future::cancel::CancelToken,
    ) -> Result<(), minibot_common::net::rpc::CommandError> {
        Err(CommandError::UnknownMethod)
    }
}

/// Info for connecting to a minibot server.
#[derive(Clone, Debug)]
pub struct Server {
    client: reqwest::Client,
    auth_url: Url,
    exchange_url: Url,
    ws_url: Url,
}

impl Server {
    pub fn new(server_addr: &str) -> Self {
        let server_addr = url::Url::parse(&server_addr).unwrap();

        Server {
            client: reqwest::Client::new(),
            auth_url: server_addr.join("login").unwrap(),
            exchange_url: server_addr.join("confirm").unwrap(),
            ws_url: server_addr.join("connect").unwrap(),
        }
    }

    pub async fn authenticate_local_http<F, E>(
        &self,
        deadline: std::time::Instant,
        open_browser_func: F,
    ) -> Result<ClientAuthn, AuthnError>
    where
        F: FnOnce(&str) -> Result<(), E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let token = access_token::get_local_http_access_token(
            &self.client,
            deadline,
            &self.auth_url,
            &self.exchange_url,
            |url| open_browser_func(&url.as_str()),
        )
        .await?;

        Ok(ClientAuthn(token.into()))
    }

    pub async fn authenticate_token<'a>(&'a self) -> (String, TokenExchanger<'a>) {
        let (redirect_url, verifier) = access_token::make_token_auth_url(&self.auth_url);
        (
            redirect_url.into_string(),
            TokenExchanger {
                server: self,
                verifier: verifier,
            },
        )
    }

    pub async fn connect(&self, authn: &ClientAuthn) -> Result<Connection, ConnectError> {
        let mut request = (&self.ws_url).into_client_request().unwrap();
        // Add authn header
        request.headers_mut().append(
            http::header::AUTHORIZATION,
            format!("MinibotAuthn {}", &*authn.0).parse().unwrap(),
        );

        let (stream, _) = connect_async(request).await?;

        let client = start_websocket_rpc(stream, NullCommandHandler);

        Ok(Connection { client })
    }
}

pub struct TokenExchanger<'a> {
    server: &'a Server,
    verifier: proof_key::Verifier,
}

impl<'a> TokenExchanger<'a> {
    pub async fn exchange(self, token: &str) -> Result<ClientAuthn, AuthnError> {
        let client = reqwest::Client::new();
        let result = access_token::exchange_confirm_token(
            &client,
            &self.server.exchange_url,
            token,
            &self.verifier,
        )
        .await?;
        
        Ok(ClientAuthn(result.into()))
    }
}

#[derive(Debug, Clone)]
pub struct ClientAuthn(SecureString);

pub struct Connection {
    client: ClientChannel,
}

impl Connection {
    pub async fn send_command<Cmd>(
        &mut self,
        command: Cmd,
    ) -> Result<PipeEnd<Cmd::Response>, SendCommandError>
    where
        Cmd: Command,
    {
        self.client.send_command(command).await
    }
}

// --------------

