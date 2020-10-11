use async_native_tls::{TlsConnector, TlsStream};
use futures::task::{Context, Poll};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncRead as TokioAsyncRead, AsyncWrite as TokioAsyncWrite};
use tokio::net::TcpStream;

struct WrapTcpStream(tokio::net::TcpStream);

impl WrapTcpStream {
    pub fn new(stream: tokio::net::TcpStream) -> Self {
        WrapTcpStream(stream)
    }

    pub fn as_ref(&self) -> &tokio::net::TcpStream {
        &self.0
    }
}

impl futures::io::AsyncRead for WrapTcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl futures::io::AsyncWrite for WrapTcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

type NetStreamInner = Mutex<TlsStream<WrapTcpStream>>;

#[derive(Clone)]
struct NetStream(Arc<NetStreamInner>);

impl NetStream {
    fn call_on_pinned<T, F: FnOnce(Pin<&mut TlsStream<WrapTcpStream>>) -> T>(&self, func: F) -> T {
        let mut guard = self.0.lock().unwrap();
        func(Pin::new(&mut *guard))
    }

    fn close(&self, how: std::net::Shutdown) -> futures::io::Result<()> {
        self.call_on_pinned(|p| p.get_ref().as_ref().shutdown(how))
    }
}

pub struct ReadNetStream(NetStream);

impl futures::io::AsyncRead for ReadNetStream {
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
        let _ = self.0.close(std::net::Shutdown::Read);
    }
}

pub struct WriteNetStream(NetStream);

impl futures::io::AsyncWrite for WriteNetStream {
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

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context) -> Poll<std::io::Result<()>> {
        self.0.call_on_pinned(|p| p.poll_close(cx))
    }
}

impl Drop for WriteNetStream {
    fn drop(&mut self) {
        let _ = self.0.close(std::net::Shutdown::Write);
    }
}

pub async fn connect_ssl(
    connector: &TlsConnector,
    host: &str,
    port: u16,
) -> super::Result<(ReadNetStream, WriteNetStream)> {
    let init_stream = WrapTcpStream::new(TcpStream::connect((host, port)).await?);
    let stream = connector.connect(host, init_stream).await?;

    let net_stream = NetStream(Arc::new(Mutex::new(stream)));

    let read_stream = ReadNetStream(net_stream.clone());
    let write_stream = WriteNetStream(net_stream);

    Ok((read_stream, write_stream))
}
