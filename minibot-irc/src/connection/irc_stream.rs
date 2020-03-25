use super::irc_codec::IrcCodec;
use super::net_stream::ReadNetStream;
use crate::messages::Message;
use futures::prelude::*;
use futures::task::{Context, Poll};
use std::pin::Pin;
use tokio_util::codec;

pub struct IrcStream(codec::FramedRead<ReadNetStream, IrcCodec>);

impl Stream for IrcStream {
    type Item = super::Result<Message>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<Option<super::Result<Message>>> {
        Pin::new(&mut self.0).poll_next(cx)
    }
}

pub fn make_stream(read_stream: ReadNetStream) -> IrcStream {
    IrcStream(codec::FramedRead::new(read_stream, IrcCodec))
}
