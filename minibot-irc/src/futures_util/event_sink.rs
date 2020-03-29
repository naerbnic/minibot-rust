use futures::channel::mpsc;
use futures::prelude::*;
use super::simple_sender::SimpleSender;

pub struct EventSink<T> {
    sinks: Vec<SimpleSender<T>>,
}

impl<T: Clone> EventSink<T> {
    pub fn new() -> Self {
        EventSink { sinks: Vec::new() }
    }

    fn removed_disconnected(&mut self) {
        self.sinks.retain(|sink| sink.is_connected());
    }

    pub async fn send(&mut self, msg: T) {
        let joinables = self
            .sinks
            .iter_mut()
            .map(|sender| {
                sender.send(msg.clone())
            });

        future::join_all(joinables).await;

        self.removed_disconnected();
    }

    pub fn add_sink(&mut self, sender: mpsc::Sender<T>) {
        self.sinks.push(SimpleSender::new(sender));
    }
}
