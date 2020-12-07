use futures::channel::oneshot::{channel, Receiver, Sender};
use futures::prelude::*;

/// A cancel handle indicates cancellation by simply being dropped.
///
/// Calling `ignore()` on it instead will treat it as if it is never dropped.
pub struct CancelHandle(Sender<()>);

impl CancelHandle {
    /// Cancel the handle, indicating cancellation on the token.
    ///
    /// This method is not necessary to be called, being equivalent to std::mem::drop(handle).
    pub fn cancel(self) {
        // No body: let self be dropped.
    }

    /// Ignore the handle, effectively dropping it without canceling the token.
    pub fn ignore(self) {
        // An error indicates that the token was dropped, which is not a real error.
        let _ = self.0.send(());
    }
}

enum TokenState {
    Pending(Receiver<()>),
    Canceled,
    Ignored,
}

/// A future that will resolve if canceled by the equivalent CancelHandle.
pub struct CancelToken(TokenState);

impl Future for CancelToken {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<()> {
        match &mut self.0 {
            TokenState::Pending(recv) => match futures::ready!(recv.poll_unpin(cx)) {
                Ok(()) => {
                    self.0 = TokenState::Ignored;
                    std::task::Poll::Pending
                }
                Err(_) => {
                    self.0 = TokenState::Canceled;
                    std::task::Poll::Ready(())
                }
            },
            TokenState::Ignored => std::task::Poll::Pending,
            TokenState::Canceled => std::task::Poll::Ready(()),
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[error("The future was canceled")]
pub struct Canceled;

impl CancelToken {
    pub async fn with_canceled<F>(self, fut: F) -> Result<F::Output, Canceled>
    where
        F: Future + Unpin,
    {
        let mut token = self;
        futures::select! {
            out = fut.fuse() => Ok(out),
            _ = token => Err(Canceled),
        }
    }

    pub async fn with_canceled_or_else<F>(self, default: F::Output, fut: F) -> F::Output
    where
        F: Future + Unpin,
    {
        let mut token = self;
        futures::select! {
            out = fut.fuse() => out,
            _ = token => default,
        }
    }

    /// Runs the given function when this token is canceled. The future will complete
    /// without calling the function if the handle is ignored. Spawning this future
    /// will not leak a task.
    pub async fn on_canceled<F>(mut self, func: F)
    where
        F: FnOnce(),
    {
        match std::mem::replace(&mut self.0, TokenState::Ignored) {
            TokenState::Pending(recv) => match recv.await {
                Ok(()) => {}
                Err(_) => func(),
            },
            TokenState::Canceled => func(),
            TokenState::Ignored => {}
        }
    }
}

impl futures::future::FusedFuture for CancelToken {
    fn is_terminated(&self) -> bool {
        matches!(self.0, TokenState::Canceled)
    }
}

pub fn cancel_pair() -> (CancelHandle, CancelToken) {
    let (send, recv) = channel();
    (CancelHandle(send), CancelToken(TokenState::Pending(recv)))
}

/// Returns a CancelToken which will never be canceled.
pub fn ignored_token() -> CancelToken {
    CancelToken(TokenState::Ignored)
}
