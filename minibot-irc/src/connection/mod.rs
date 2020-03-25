mod irc_codec;
mod irc_sink;
mod irc_stream;
mod net_stream;

pub use irc_sink::IrcSink;
pub use irc_stream::IrcStream;

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

pub struct IrcConnector(tokio_tls::TlsConnector);

impl IrcConnector {
    pub fn new() -> Result<Self> {
        Ok(IrcConnector(native_tls::TlsConnector::new()?.into()))
    }

    pub async fn connect(&self, host: &str, port: u16) -> Result<(IrcStream, IrcSink)> {
        let (read_stream, write_stream) = net_stream::connect_ssl(&self.0, host, port).await?;
        Ok((
            irc_stream::make_stream(read_stream),
            irc_sink::make_sink(write_stream),
        ))
    }
}
