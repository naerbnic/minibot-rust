use crate::connection::{IrcConnector, IrcSink, IrcStream};
use crate::messages::Message;
use crate::rpc::{FilterResult, IrcRpcConnection, RpcCall};

#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    #[error(transparent)]
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

        let msg_handler = move |m| async move { Ok::<_, ClientError>(()) };

        let connection = IrcRpcConnection::new(irc_read, irc_write, msg_handler);
        Ok(Client { connection })
    }
}

struct JoinCall {
    channel: String,
}

impl RpcCall for JoinCall {
    type Output = ();
    type Err = ClientError;
    fn send_messages(&self) -> Vec<Message> {
        vec![Message::from_named_command_params("JOIN", &[&self.channel])]
    }

    fn msg_filter(&self, msg: &Message) -> Result<FilterResult, Self::Err> {
        Ok(if msg.has_named_command("JOIN") {
            FilterResult::Next
        } else if msg.has_num_command(353) {
            FilterResult::Next
        } else if msg.has_num_command(366) {
            FilterResult::End
        } else if msg.has_num_command(461) {
            todo!()
        } else {
            FilterResult::Skip
        })
    }

    fn recv_messages(&self, msgs: Vec<Message>) -> Result<Self::Output, Self::Err> {
        todo!()
    }
}

pub type ClientResult<T> = Result<T, ClientError>;

pub struct Client {
    connection: IrcRpcConnection,
}

impl Client {
    pub fn join(&mut self, channel: &str) {}
}
