use crate::byte_string::{ByteStr, ByteString};
use crate::connection::{IrcConnector, IrcSink, IrcStream};
use crate::messages::Message;
use crate::rpc::{FilterResult, IrcRpcConnection, RpcCall};
use futures::prelude::*;

fn join_bytes<T: IntoIterator<Item = S>, S: AsRef<[u8]>>(iter: T, connector: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut first = true;
    for item in iter.into_iter() {
        if first {
            first = false;
        } else {
            result.extend(connector);
        }
        result.extend(item.as_ref());
    }
    result
}

#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    #[error(transparent)]
    Connection(#[from] crate::connection::Error),

    #[error("Stream ended unexpectedly")]
    UnexpectedEnd,
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
        let (mut irc_read, mut irc_write) = self.connector.connect(host, port).await?;
        irc_write
            .send(Message::from_named_command_params("CAP", &["LS", "302"]))
            .await?;

        let mut caps = Vec::new();
        loop {
            let message = irc_read.next().await.ok_or(ClientError::UnexpectedEnd)??;
            assert!(
                message.has_named_command("CAP"),
                "Unexpected message: {:?}",
                message
            );
            let params = message.params();
            assert!(params.len() >= 2);
            if params.len() == 2 {
                assert!(params[0].eq_bytes(b"LS"));
                let caps_list = &params[1];
                caps.extend(caps_list.split_spaces().map(ByteStr::to_string));
            } else if params.len() == 3 {
                assert!(params[0].eq_bytes(b"*"));
                assert!(params[1].eq_bytes(b"LS"));
                let caps_list = &params[2];
                caps.extend(caps_list.split_spaces().map(ByteStr::to_string));
                break;
            } else {
                panic!("Unexpected message: {:?}", message);
            }
        }

        eprintln!("Got caps: {:?}", caps);

        // Check that the caps are the expected set.

        let ack_args = join_bytes(caps, b" ");

        irc_write
            .send(Message::from_named_command_params(
                "CAP",
                &[b"REQ", &ack_args[..]],
            ))
            .await?;

        let mut caps = Vec::new();
        loop {
            let message = irc_read.next().await.ok_or(ClientError::UnexpectedEnd)??;
            assert!(
                message.has_named_command("CAP"),
                "Unexpected message: {:?}",
                message
            );
            let params = message.params();
            assert!(params.len() >= 2);
            if params.len() == 2 {
                assert!(params[0].eq_bytes(b"ACK"));
                let caps_list = &params[1];
                caps.extend(caps_list.split_spaces().map(ByteStr::to_string));
            } else if params.len() == 3 {
                assert!(params[0].eq_bytes(b"*"));
                assert!(params[1].eq_bytes(b"ACK"));
                let caps_list = &params[2];
                caps.extend(caps_list.split_spaces().map(ByteStr::to_string));
                break;
            } else {
                panic!("Unexpected message: {:?}", message);
            }
        }

        irc_write
            .send(Message::from_named_command_params(
                "PASS",
                &[&format!("oauth:{}", token)],
            ))
            .await?;
        irc_write
            .send(Message::from_named_command_params("NICK", &[user]))
            .await?;
        irc_write
            .send(Message::from_named_command_params("CAP", &[b"END"]))
            .await?;
        loop {
            let message = irc_read.next().await.ok_or(ClientError::UnexpectedEnd)??;
            if message.has_num_command(376) {
                break;
            }
        }

        let msg_handler = move |m| async move { Ok::<_, ClientError>(()) };

        let connection = IrcRpcConnection::new(irc_read, irc_write, msg_handler);
        Ok(Client { connection })
    }
}

pub type ClientResult<T> = Result<T, ClientError>;

pub struct Client {
    connection: IrcRpcConnection,
}

impl Client {
    pub fn join(&mut self, channel: &str) {}
}
