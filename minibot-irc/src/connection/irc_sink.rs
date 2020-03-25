use super::irc_codec::IrcCodec;
use super::net_stream::WriteNetStream;
use crate::messages::Message;
use futures::prelude::*;
use futures::task::{Context, Poll};
use std::pin::Pin;
use tokio_util::codec;

pub struct IrcSink(codec::FramedWrite<WriteNetStream, IrcCodec>);

impl Sink<Message> for IrcSink {
    type Error = super::Error;
    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<std::result::Result<(), Self::Error>> {
        Pin::new(&mut self.0).poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Message) -> std::result::Result<(), Self::Error> {
        Pin::new(&mut self.0).start_send(item)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<std::result::Result<(), Self::Error>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<std::result::Result<(), Self::Error>> {
        Pin::new(&mut self.0).poll_close(cx)
    }
}

pub fn make_sink(write_stream: WriteNetStream) -> IrcSink {
    IrcSink(codec::FramedWrite::new(write_stream, IrcCodec))
}
