use futures::channel::mpsc;
use futures::prelude::*;

pub enum SimpleSender<T> {
    Connected(mpsc::Sender<T>),
    Disconnected,
}

impl<T> SimpleSender<T> {
    pub fn new(sender: mpsc::Sender<T>) -> Self {
        SimpleSender::Connected(sender)
    }

    pub async fn send(&mut self, msg: T) {
        if let SimpleSender::Connected(sender) = self {
            if let Err(_) = sender.send(msg).await {
                *self = SimpleSender::Disconnected;
            }
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(self, SimpleSender::Connected(_))
    }
}
