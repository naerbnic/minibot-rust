#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    #[error("Problem with connection: {0}")]
    Connection(#[from] crate::connection::Error),
}

pub struct ClientFactory {
    connector: tokio_tls::TlsConnector,
}

impl ClientFactory {
    pub fn create() -> ClientResult<Self> {
        Ok(ClientFactory {
            connector: crate::connection::make_connector()?
        })
    }

    pub async fn connect(&self, host: &str, port: u16, user: &str, token: &str) -> ClientResult<Client> {
        let (irc_read, irc_write) = crate::connection::irc_connect_ssl(&self.connector, host, port).await?;
        Ok(Client {})
    }
}

pub type ClientResult<T> = Result<T, ClientError>;

pub struct Client {
}

impl Client {
}