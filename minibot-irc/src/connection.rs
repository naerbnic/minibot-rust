use bytes::{Buf as _, BytesMut};
use futures::prelude::*;
use futures::task::{Context, Poll};
use std::pin::Pin;
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

fn make_connector() -> Result<tokio_tls::TlsConnector> {
    Ok(native_tls::TlsConnector::new()?.into())
}

pub struct NetStream();

pub async fn irc_connect_ssl(
    connector: &tokio_tls::TlsConnector,
    host: &str,
    port: u16,
) -> Result<(
    Box<dyn AsyncRead + Send + Sync + Unpin + 'static>,
    Box<dyn AsyncWrite + Send + Sync + Unpin + 'static>,
)> {
    let stream = tokio::net::TcpStream::connect((host, port)).await?;
    let ssl_stream = connector.connect(host, stream).await?.compat();

    let (reader, writer) = ssl_stream.split();
    Ok((Box::new(reader), Box::new(writer)))
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
