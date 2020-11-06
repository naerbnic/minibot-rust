use futures::prelude::*;
use tokio_tungstenite::tungstenite::Message as WsMessage;

pub struct ChannelAcceptor {

}

impl ChannelAcceptor {
    pub fn accept<T: Stream<Item = WsMessage> + Sink<WsMessage>>(&self, conn: T) -> anyhow::Result<()> {
        let (stream, sink) = conn.split();

        todo!();
    }
}