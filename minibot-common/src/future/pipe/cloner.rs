use super::safe_sender::SafeSender;
use futures::{channel::mpsc, prelude::*, stream::FusedStream};

#[derive(Clone)]
pub struct ClonerHandle<T> {
    sender: mpsc::Sender<SafeSender<T>>,
}

impl<T> ClonerHandle<T> {
    pub fn new(stream: mpsc::Receiver<T>) -> Self
    where
        T: Clone + Send + 'static,
    {
        let (sender_start, sender_end) = mpsc::channel(0);
        tokio::spawn(run_cloner(stream, sender_end));
        ClonerHandle {
            sender: sender_start,
        }
    }

    pub async fn add_sender(&mut self, send: mpsc::Sender<T>) {
        let _ = self.sender.send(SafeSender::new(send)).await;
    }
}

async fn run_cloner<T>(
    mut stream: mpsc::Receiver<T>,
    mut sender_stream: mpsc::Receiver<SafeSender<T>>,
) where
    T: Clone + Send + 'static,
{
    let mut senders: Vec<SafeSender<T>> = Vec::new();
    loop {
        futures::select! {
            item = stream.next() => {
                senders.retain(|ss| !ss.is_closed());
                if senders.is_empty() {
                    if sender_stream.is_terminated() {
                        // We can't get any more streams in here, so this is effectively terminated.
                        break
                    }
                }

                match item {
                    Some(item) => {
                        futures::future::join_all(
                            senders.iter().cloned().map(move |mut ss| {let item = item.clone(); async move { ss.send(item).await }}))
                        .await;
                    }
                    None => break,
                }
            },
            next_sender = sender_stream.next() => {
                if let Some(next_sender) = next_sender {
                    senders.push(next_sender);
                }
            },
        }
    }
}
