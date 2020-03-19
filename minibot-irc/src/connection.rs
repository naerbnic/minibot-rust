use bytes::{Buf as _, BytesMut};
use futures::prelude::*;
use futures::task::{Context, Poll};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::net::TcpStream;
use tokio_tls::{TlsConnector, TlsStream};
use tokio_util::compat::{Compat, Tokio02AsyncReadCompatExt};

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

impl tokio::io::AsyncRead for NetStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        self.call_on_pinned(|p| p.poll_read(cx, buf))
    }
}

impl tokio::io::AsyncWrite for NetStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.call_on_pinned(|p| p.poll_write(cx, buf))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<std::io::Result<()>> {
        self.call_on_pinned(|p| p.poll_flush(cx))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<std::io::Result<()>> {
        self.call_on_pinned(|p| p.poll_shutdown(cx))
    }
}

fn make_connector() -> Result<TlsConnector> {
    Ok(native_tls::TlsConnector::new()?.into())
}

pub async fn irc_connect_ssl(
    connector: &TlsConnector,
    host: &str,
    port: u16,
) -> Result<(
    impl futures::stream::Stream<Item = std::result::Result<crate::messages::Message, Error>>,
    impl futures::sink::Sink<Vec<u8>>,
)> {
    let stream = TcpStream::connect((host, port)).await?;
    let net_stream = NetStream(Arc::new(Mutex::new(connector.connect(host, stream).await?)));
    let ssl_stream = tokio_util::codec::Framed::new(net_stream.clone(), IrcCodec);

    let (sink, stream) = ssl_stream.split();
    Ok((stream, sink))
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

        let message = crate::messages::Message::from_line(&src_bytes[..pos])?;
        src.advance(pos + 2);
        Ok(Some(message))
    }
}

impl tokio_util::codec::Encoder<Vec<u8>> for IrcCodec {
    type Error = self::Error;

    fn encode(&mut self, item: Vec<u8>, dst: &mut BytesMut) -> Result<()> {
        dst.extend_from_slice(&item[..]);
        Ok(())
    }
}
