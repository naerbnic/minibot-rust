//! A crate that handles low-level IRC stream parsing. This allows for primitive tag parts of
//! messages, both for parsing and for writing. As the low-level version, it does not directly
//! involve itself with capability negotiation, and should be handled at a higher layer.

mod codec;
mod messages;
mod read_bytes;
mod write_bytes;

pub use codec::IrcCodec;
pub use futures::prelude::*;
pub use messages::{Command, CommandNumber, Message};
pub use codec::Error;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_codec::{FramedRead, FramedWrite};

pub fn from_channel<C: futures::io::AsyncRead + futures::io::AsyncWrite + Unpin + Send + 'static>(
    channel: C,
) -> (
    IrcStream,
    IrcSink,
) {
    let (read_half, write_half) = channel.split();
    (IrcStream::new(read_half), IrcSink::new(write_half))
}

pub struct IrcStream(Box<dyn Stream<Item = Result<Message, Error>> + Unpin + Send + 'static>);

impl IrcStream{
    pub fn new<T>(read: T) -> Self where T: AsyncRead + Unpin + Send + 'static {
        IrcStream(Box::new(FramedRead::new(read, IrcCodec)))
    }
}

impl Stream for IrcStream {
    type Item = Result<Message, crate::codec::Error>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<Option<Result<Message, crate::codec::Error>>> {
        Pin::new(&mut self.0).poll_next(cx)
    }
}

pub struct IrcSink(Box<dyn Sink<Message, Error = Error> + Unpin + Send + 'static>);

impl IrcSink {
    pub fn new<T>(write: T) -> Self where T: AsyncWrite + Unpin + Send + 'static {
        IrcSink(Box::new(FramedWrite::new(write, IrcCodec)))
    }
}

impl Sink<Message> for IrcSink {
    type Error = crate::codec::Error;
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
