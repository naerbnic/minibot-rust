use super::simple_sender::SimpleSender;
use futures::channel::mpsc;
use futures::prelude::*;
use std::mem;
use std::sync::{Arc, Mutex};

struct Inner<T> {
    sinks: Mutex<Vec<SimpleSender<T>>>,
}

impl<T: Clone> Inner<T> {
    pub async fn send(&self, msg: T) {
        let mut sinks = {
            let mut guard = self.sinks.lock().unwrap();
            mem::replace(&mut *guard, Vec::new())
        };

        let joinables = sinks.iter_mut().map(|sender| sender.send(msg.clone()));

        future::join_all(joinables).await;

        sinks.retain(|sink| sink.is_connected());

        let mut guard = self.sinks.lock().unwrap();
        sinks.extend(guard.drain(..));
        *guard = sinks;
    }

    pub fn add_sink(&self, sender: mpsc::Sender<T>) {
        let mut guard = self.sinks.lock().unwrap();
        guard.push(SimpleSender::new(sender))
    }
}

pub struct EventSink<T> {
    inner: Arc<Inner<T>>,
}

impl<T: Clone + Send + Sync + 'static> EventSink<T> {
    pub fn new<S: Stream<Item = T> + Unpin + Send + 'static>(mut stream: S) -> Self {
        let inner = Inner {
            sinks: Mutex::new(Vec::new()),
        };

        let arc_inner = Arc::new(inner);

        tokio::spawn({
            let arc_inner = arc_inner.clone();
            async move {
                while let Some(msg) = stream.next().await {
                    arc_inner.send(msg).await;
                }
            }
        });

        EventSink { inner: arc_inner }
    }

    pub fn add_sink(&mut self, sender: mpsc::Sender<T>) {
        self.inner.add_sink(sender);
    }
}
