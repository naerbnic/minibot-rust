use futures::task::{Context, Poll};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::net::TcpStream;
use tokio_tls::{TlsConnector, TlsStream};

type NetStreamInner = Mutex<TlsStream<tokio::net::TcpStream>>;

#[derive(Clone)]
struct NetStream(Arc<NetStreamInner>);

impl NetStream {
    fn call_on_pinned<T, F: FnOnce(Pin<&mut TlsStream<tokio::net::TcpStream>>) -> T>(
        &self,
        func: F,
    ) -> T {
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

pub async fn connect_ssl(
    connector: &TlsConnector,
    host: &str,
    port: u16,
) -> super::Result<(ReadNetStream, WriteNetStream)> {
    let init_stream = TcpStream::connect((host, port)).await?;
    let stream = connector.connect(host, init_stream).await?;

    let net_stream = NetStream(Arc::new(Mutex::new(stream)));

    let read_stream = ReadNetStream(net_stream.clone());
    let write_stream = WriteNetStream(net_stream);

    Ok((read_stream, write_stream))
}
