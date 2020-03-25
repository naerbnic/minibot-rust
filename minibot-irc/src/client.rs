use crate::connection::{IrcConnector, IrcSink, IrcStream};

#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    #[error("Problem with connection: {0}")]
    Connection(#[from] crate::connection::Error),
}

pub struct ClientFactory {
    connector: IrcConnector,
}

impl ClientFactory {
    pub fn create() -> ClientResult<Self> {
        Ok(ClientFactory {
            connector: IrcConnector::new()?,
        })
    }

    pub async fn connect(
        &self,
        host: &str,
        port: u16,
        user: &str,
        token: &str,
    ) -> ClientResult<Client> {
        let (irc_read, irc_write) = self.connector.connect(host, port).await?;
        Ok(Client {})
    }
}

pub type ClientResult<T> = Result<T, ClientError>;

pub struct Client {}

impl Client {}
