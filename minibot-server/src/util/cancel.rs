use futures::channel::oneshot::{channel, Receiver, Sender};
use futures::future::Fuse;
use futures::prelude::*;
use std::convert::Infallible;

/// A cancel handle indicates cancellation by simply being dropped.
pub struct CancelHandle(Sender<Infallible>);

pub struct CancelToken(Receiver<Infallible>);

impl CancelToken {
    pub fn is_cancelled(&mut self) -> bool {
        match self.0.try_recv() {
            Ok(Some(_)) => unreachable!("due to infallible"),
            Ok(None) => false,
            Err(_) => true,
        }
    }

    pub fn on_cancelled<'a>(
        &'a mut self,
    ) -> Fuse<Box<dyn Future<Output = ()> + Send + Unpin + 'a>> {
        let fut_box: Box<dyn Future<Output = ()> + Send + Unpin + 'a> =
            Box::new((&mut self.0).map(|_| ()));
        fut_box.fuse()
    }
}

pub fn cancel_pair() -> (CancelHandle, CancelToken) {
    let (send, recv) = channel();
    (CancelHandle(send), CancelToken(recv))
}
