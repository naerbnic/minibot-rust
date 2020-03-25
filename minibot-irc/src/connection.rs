use bytes::{Buf as _, BytesMut};
use futures::task::{Context, Poll};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::net::TcpStream;
use tokio_tls::{TlsConnector, TlsStream};
use tokio_util::codec;
use crate::read_bytes::ReadBytes;
use crate::write_bytes::{WriteBytes, ByteSink};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    MessageParseError(#[from] crate::messages::Error),

    #[error(transparent)]
    NativeTlsError(#[from] native_tls::Error),

    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

type Result<T> = std::result::Result<T, Error>;

type NetStreamInner = Mutex<TlsStream<tokio::net::TcpStream>>;

#[derive(Clone)]
pub struct NetStream(Arc<NetStreamInner>);

impl NetStream {
    fn call_on_pinned<T, F: FnOnce(Pin<&mut TlsStream<tokio::net::TcpStream>>) -> T>(&self, func: F) -> T {
        let mut guard = self.0.lock().unwrap();
        func(Pin::new(&mut *guard))
    }
    pub fn shutdown(&self, how: std::net::Shutdown) -> tokio::io::Result<()> {
        self.call_on_pinned(|p| p.get_ref().shutdown(how))
    }
}

pub struct ReadNetStream(NetStream);

impl tokio::io::AsyncRead for ReadNetStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        self.0.call_on_pinned(|p| p.poll_read(cx, buf))
    }
}

impl Drop for ReadNetStream {
    fn drop(&mut self) {
        let _ = self.0.shutdown(std::net::Shutdown::Read);
    }
}

pub struct WriteNetStream(NetStream);

impl tokio::io::AsyncWrite for WriteNetStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.0.call_on_pinned(|p| p.poll_write(cx, buf))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<std::io::Result<()>> {
        self.0.call_on_pinned(|p| p.poll_flush(cx))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<std::io::Result<()>> {
        self.0.call_on_pinned(|p| p.poll_shutdown(cx))
    }
}

impl Drop for WriteNetStream {
    fn drop(&mut self) {
        let _ = self.0.shutdown(std::net::Shutdown::Write);
    }
}

pub fn make_connector() -> Result<TlsConnector> {
    Ok(native_tls::TlsConnector::new()?.into())
}

pub async fn connect_ssl(
    connector: &TlsConnector,
    host: &str, port: u16,
) -> Result<(ReadNetStream, WriteNetStream)> {
    let init_stream = TcpStream::connect((host, port)).await?;
    let stream = connector.connect(host, init_stream).await?;

    let net_stream = NetStream(Arc::new(Mutex::new(stream)));

    let read_stream = ReadNetStream(net_stream.clone());
    let write_stream = WriteNetStream(net_stream);

    Ok((read_stream, write_stream))
}

pub async fn irc_connect_ssl(
    connector: &TlsConnector,
    host: &str,
    port: u16,
) -> Result<(
    impl futures::stream::Stream<Item = std::result::Result<crate::messages::Message, Error>>,
    impl futures::sink::Sink<crate::messages::Message, Error=Error>,
)> {
    let (read_stream, write_stream) = connect_ssl(connector, host, port).await?;
    let framed_read = codec::FramedRead::new(read_stream, IrcCodec);
    let framed_write = codec::FramedWrite::new(write_stream, IrcCodec);
    Ok((framed_read, framed_write))
}

#[derive(Clone)]
pub struct IrcCodec;

impl tokio_util::codec::Decoder for IrcCodec {
    type Item = crate::messages::Message;

    type Error = self::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
        // Look for ending CR LF
        let mut src_bytes;
        let pos = loop {
            src_bytes = src.bytes();
            let pos = match src_bytes.windows(2).position(|s| s == b"\r\n") {
                None => return Ok(None),
                Some(p) => p,
            };

            if pos == 0 {
                // An empty message is just skipped
                src.advance(2);
            } else {
                break pos;
            }
        };

        let message = crate::messages::Message::read_bytes(&src_bytes[..pos])?;
        src.advance(pos + 2);
        Ok(Some(message))
    }
}

impl tokio_util::codec::Encoder<crate::messages::Message> for IrcCodec {
    type Error = self::Error;

    fn encode(&mut self, item: crate::messages::Message, dst: &mut BytesMut) -> Result<()> {
        let mut result = Vec::new();
        item.write_bytes(&mut result).unwrap();
        item.write_bytes(dst).unwrap();
        dst.write(b"\r\n").unwrap();
        Ok(())
    }
}
