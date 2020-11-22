use futures::{channel::mpsc, prelude::*};

#[derive(Clone)]
pub struct SafeSender<T>(Option<mpsc::Sender<T>>);

impl<T> SafeSender<T> {
    pub fn new(sink: mpsc::Sender<T>) -> Self {
        SafeSender(Some(sink))
    }

    /// Sends the item into the sender, or does nothing if the sender has been dropped.
    pub async fn send(&mut self, item: T) {
        match &mut self.0 {
            Some(sink) => match sink.send(item).await {
                Ok(()) => {}
                Err(_) => self.0 = None,
            },
            None => {}
        }
    }

    pub fn is_closed(&self) -> bool {
        match &self.0 {
            None => true,
            Some(sink) => sink.is_closed(),
        }
    }
}
