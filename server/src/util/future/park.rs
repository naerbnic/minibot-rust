use futures::channel::oneshot;
use std::sync::{Arc, Mutex};

struct UnparkerState {
    sender: Option<oneshot::Sender<()>>,
}

pub struct Parker {
    unparker_state: Arc<Mutex<UnparkerState>>,
    recv: Option<oneshot::Receiver<()>>,
}

impl Parker {
    pub fn new() -> Self {
        let (send, recv) = oneshot::channel();
        Parker {
            unparker_state: Arc::new(Mutex::new(UnparkerState { sender: Some(send) })),
            recv: Some(recv),
        }
    }

    pub fn unparker(&self) -> Unparker {
        Unparker(self.unparker_state.clone())
    }

    pub async fn park(&mut self) {
        if let Some(recv) = self.recv.take() {
            recv.await.expect(
                "Parker keeps reference to sender if unused, so it should never be dropped.",
            )
        }
    }
}

#[derive(Clone)]
pub struct Unparker(Arc<Mutex<UnparkerState>>);

impl Unparker {
    pub fn unpark(&self) {
        let sender = {
            let mut guard = self.0.lock().unwrap();
            guard.sender.take()
        };

        if let Some(sender) = sender {
            // The result doesn't matter. We can only fail if the receiver has been dropped which
            // is the Parker's perrogative
            let _ = sender.send(());
        }
    }
}
