mod net_stream;

pub use minibot_irc_raw::{Error as IrcError, IrcSink, IrcStream};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    MessageParseError(#[from] IrcError),

    #[error(transparent)]
    NativeTlsError(#[from] native_tls::Error),

    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

type Result<T> = std::result::Result<T, Error>;

pub struct IrcConnector(async_native_tls::TlsConnector);

impl IrcConnector {
    pub fn new() -> Result<Self> {
        Ok(IrcConnector(async_native_tls::TlsConnector::new()))
    }

    pub async fn connect(&self, host: &str, port: u16) -> Result<(IrcStream, IrcSink)> {
        let (read_stream, write_stream) = net_stream::connect_ssl(&self.0, host, port).await?;
        Ok((IrcStream::new(read_stream), IrcSink::new(write_stream)))
    }
}
