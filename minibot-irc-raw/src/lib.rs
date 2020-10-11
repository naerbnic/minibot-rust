//! A crate that handles low-level IRC stream parsing. This allows for primitive tag parts of
//! messages, both for parsing and for writing. As the low-level version, it does not directly
//! involve itself with capability negotiation, and should be handled at a higher layer.

mod byte_string;
mod codec;
mod messages;
mod read_bytes;
mod write_bytes;

pub use codec::IrcCodec;
pub use futures::prelude::*;
pub use messages::{Command, CommandNumber, Message};

use futures_codec::{FramedRead, FramedWrite};

pub fn from_channel<C: futures::io::AsyncRead + futures::io::AsyncWrite>(
    channel: C,
) -> (
    IrcStream<futures::io::ReadHalf<C>>,
    IrcSink<futures::io::WriteHalf<C>>,
) {
    let (read_half, write_half) = channel.split();
    (
        stream_from_async_read(read_half),
        sink_from_async_write(write_half),
    )
}

pub fn stream_from_async_read<R: futures::io::AsyncRead>(read: R) -> IrcStream<R> {
    IrcStream(FramedRead::new(read, IrcCodec))
}

pub fn sink_from_async_write<R: futures::io::AsyncWrite>(write: R) -> IrcSink<R> {
    IrcSink(FramedWrite::new(write, IrcCodec))
}

pub struct IrcStream<T>(FramedRead<T, IrcCodec>);

impl<T> IrcStream<T>
where
    T: futures::AsyncRead,
{
    pub fn into_inner(self) -> T {
        let IrcStream(val) = self;
        val.into_inner()
    }
}

pub struct IrcSink<T>(FramedWrite<T, IrcCodec>);

impl<T> IrcSink<T>
where
    T: futures::AsyncWrite,
{
    pub fn into_inner(self) -> T {
        let IrcSink(val) = self;
        val.into_inner()
    }
}
