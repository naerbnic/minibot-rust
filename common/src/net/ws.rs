use futures::prelude::*;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::{tungstenite::Message as BaseMessage, WebSocketStream};

use crate::future::pipe::{pipe, Either, PipeEnd, PipeStart};

pub type BoxSink<'a, T, E> = Box<dyn Sink<T, Error = E> + Send + 'a>;

#[derive(Clone, Debug)]
pub enum Message {
    Text(String),
    Binary(Vec<u8>),
}

pub fn handle_websocket_message_stream<In, Out, E>(
    stream: In,
    sink: Out,
) -> (PipeEnd<Message>, PipeStart<Message>)
where
    In: Stream<Item = Result<BaseMessage, E>> + Unpin + Send + 'static,
    Out: Sink<BaseMessage> + Unpin + Send + 'static,
    Out::Error: Send + 'static,
    E: Send + 'static,
{
    let client_in_end = PipeEnd::wrap(stream);
    let client_out_start = PipeStart::wrap(sink);

    pub enum KeepAliveMessage {
        Ping(Vec<u8>),
        Pong(Vec<u8>),
    }

    pub enum NonCloseMessage {
        Message(Message),
        KeepAlive(KeepAliveMessage),
    }

    pub enum ProcessedMessage {
        NonClose(NonCloseMessage),
        Close(Option<tokio_tungstenite::tungstenite::protocol::CloseFrame<'static>>),
    }

    let (msg_end, keep_alive_end) = client_in_end
        .end_on_error()
        .map(|item| match item {
            BaseMessage::Text(text) => {
                ProcessedMessage::NonClose(NonCloseMessage::Message(Message::Text(text)))
            }
            BaseMessage::Binary(bin) => {
                ProcessedMessage::NonClose(NonCloseMessage::Message(Message::Binary(bin)))
            }
            BaseMessage::Ping(ping) => {
                ProcessedMessage::NonClose(NonCloseMessage::KeepAlive(KeepAliveMessage::Ping(ping)))
            }
            BaseMessage::Pong(pong) => {
                ProcessedMessage::NonClose(NonCloseMessage::KeepAlive(KeepAliveMessage::Pong(pong)))
            }
            BaseMessage::Close(reason) => ProcessedMessage::Close(reason),
        })
        .end_map(|item| match item {
            ProcessedMessage::NonClose(nc) => Some(nc),
            ProcessedMessage::Close(_) => None,
        })
        .either_split(|item| match item {
            NonCloseMessage::Message(msg) => Either::Left(msg),
            NonCloseMessage::KeepAlive(keep_alive) => Either::Right(keep_alive),
        });

    let ping_end = keep_alive_end.filter_map(|item| match item {
        KeepAliveMessage::Ping(ping) => Some(ping),
        KeepAliveMessage::Pong(_) => None,
    });

    let pong_end = ping_end.map(BaseMessage::Pong);

    let (out_start, out_end) = pipe();

    let out_end = out_end
        .map(|msg| match msg {
            Message::Text(text) => BaseMessage::Text(text),
            Message::Binary(bin) => BaseMessage::Binary(bin),
        })
        .merge(pong_end);

    client_out_start.connect(out_end);

    (msg_end, out_start)
}

pub fn handle_websocket_stream<T>(
    ws_stream: WebSocketStream<T>,
) -> (PipeEnd<Message>, PipeStart<Message>)
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (sink, stream) = ws_stream.split();
    handle_websocket_message_stream(stream, sink)
}
