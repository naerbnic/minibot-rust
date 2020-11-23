pub mod rpc;
pub mod ws;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::WebSocketStream;

/// Starts an RPC channel from a WebSocketStream.
pub fn start_websocket_rpc<T, H>(ws_stream: WebSocketStream<T>, handler: H) -> rpc::ClientChannel
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    H: rpc::CommandHandler + 'static,
{
    let (in_end, out_start) = ws::handle_websocket_stream(ws_stream);

    rpc::ClientChannel::new_channel(
        in_end
            .end_map(|item| match item {
                ws::Message::Text(text) => Some(text),
                ws::Message::Binary(_) => None,
            })
            .into_stream(),
        out_start.map_before(ws::Message::Text).into_sink(),
        handler,
    )
}
