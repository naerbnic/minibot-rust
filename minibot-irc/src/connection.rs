use futures::prelude::*;
use tokio_util::compat::{Tokio02AsyncReadCompatExt as _, Tokio02AsyncWriteCompatExt as _};
use futures::task::{Context, Poll};
use std::pin::Pin;
use bytes::{Buf as _, BytesMut};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    NativeTlsError(#[from] native_tls::Error),

    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

type Result<T> = std::result::Result<T, Error>;

fn make_connector() -> Result<tokio_tls::TlsConnector> {
    Ok(native_tls::TlsConnector::new()?.into())
}

pub async fn irc_connect_ssl(connector: &tokio_tls::TlsConnector, host: &str, port: u16) -> Result<(Box<dyn AsyncRead + Send + Sync + Unpin + 'static>, Box<dyn AsyncWrite + Send + Sync + Unpin + 'static>)> {
    let stream = tokio::net::TcpStream::connect((host, port)).await?;
    let ssl_stream = connector.connect(host, stream).await?;
    let (reader, writer) = tokio::io::split(ssl_stream);
    Ok((Box::new(reader.compat()), Box::new(writer.compat_write())))
}

#[derive(Clone)]
pub struct IrcCodec;

impl futures_codec::Decoder for IrcCodec {
    type Item = Vec<u8>;

    type Error = self::Error;

    fn decode(
        &mut self,
        src: &mut BytesMut
    ) -> Result<Option<Self::Item>> {
        // Look for ending CR LF
        let pos = match src.bytes().windows(2).position(|s| s == &[b'\r', b'\n']) {
            None => return Ok(None),
            Some(p) => p,
        };
        let message_data = src.bytes()[..pos].iter().copied().collect::<Vec<_>>();
        src.advance(pos + 2);
        Ok(Some(message_data))
    }
}

impl futures_codec::Encoder for IrcCodec {
    type Item = Vec<u8>;

    type Error = self::Error;

    fn encode(
        &mut self,
        item: Self::Item,
        dst: &mut BytesMut
    ) -> Result<()> {
        dst.extend_from_slice(&item[..]);
        Ok(())
    }
}