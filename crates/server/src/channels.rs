use futures::{channel::mpsc, prelude::*};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::WebSocketStream;

use minibot_common::{
    future::cancel::CancelToken,
    net::{
        rpc::{ClientChannel, CommandError, CommandHandler},
        start_websocket_rpc,
    },
};

struct ChannelHandler {
    user_id: u64,
}

#[derive(Serialize, Deserialize)]
struct UserIdResponse {
    user_id: u64,
}

impl CommandHandler for ChannelHandler {
    fn start_command(
        &mut self,
        method: &str,
        _payload: &serde_json::Value,
        mut output: mpsc::Sender<serde_json::Value>,
        _cancel: CancelToken,
    ) -> Result<(), CommandError> {
        match method {
            "user_id" => {
                let user_id = self.user_id;
                tokio::spawn(async move {
                    output
                        .send(serde_json::to_value(UserIdResponse { user_id }).unwrap())
                        .await
                        .unwrap();
                });

                Ok(())
            }

            _ => Err(CommandError::UnknownMethod),
        }
    }
}

pub struct ChannelAcceptor {
    /// A mapping from user ids to available client channels.
    channels: std::sync::Mutex<std::collections::HashMap<u64, Vec<ClientChannel>>>,
}

impl ChannelAcceptor {
    pub fn accept<T>(&self, user_id: u64, conn: WebSocketStream<T>) -> anyhow::Result<()>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let client = start_websocket_rpc(conn, ChannelHandler { user_id });

        let mut guard = self.channels.lock().unwrap();

        guard.entry(user_id).or_insert_with(Vec::new).push(client);

        Ok(())
    }
}
