use std::pin::Pin;

use futures::{channel::mpsc, prelude::*};
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::net::rpc::{ClientChannel, CommandError, CommandHandler, Message};
use crate::util::future::cancel::{cancel_pair, CancelHandle, CancelToken};
use crate::util::future::send_all_propagate;
use anyhow::Context as _;

struct ChannelHandler {
    user_id: u64,
}

impl CommandHandler for ChannelHandler {
    fn start_command(
        &mut self,
        method: &str,
        payload: &serde_json::Value,
        output: mpsc::Sender<serde_json::Value>,
        cancel: CancelToken,
    ) -> Result<Pin<Box<dyn Future<Output = ()> + Send + 'static>>, CommandError> {
        todo!()
    }
}

pub struct ChannelAcceptor {}

impl ChannelAcceptor {
    pub fn accept<T: Stream<Item = WsMessage> + Sink<WsMessage>>(
        &self,
        conn: T,
    ) -> anyhow::Result<()> {
        let (_sink, _stream) = conn.split();

        // let (error_send, error_recv) = mpsc::channel(1);
        // let (server_send, server_recv) = mpsc::channel(10);

        // let stream = stream.map(|msg| match msg {
        //     WsMessage::Text(text) => Ok::<_, anyhow::Error>(
        //         serde_json::from_str(&text).context("Could not parse rpc message")?,
        //     ),

        //     WsMessage::Binary(_) => Err(anyhow::anyhow!("Unexpected binary message")),
        // });

        // let server_recv = futures::stream::select(server_recv, error_recv);

        // ClientChannel::start_channel(stream, sink, spawner, handler);

        todo!();
    }
}
