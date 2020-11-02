use futures::channel::oneshot;
use futures::prelude::*;
use std::collections::VecDeque;
use std::sync::Mutex;

struct State {
    curr_tokens: usize,
    curr_running: usize,
    notifiers: VecDeque<oneshot::Sender<()>>,
}

pub struct TokenSource {
    max_tokens: usize,
    state: Mutex<State>,
}

impl TokenSource {
    pub fn new(max_tokens: usize) -> Self {
        let state = State {
            curr_tokens: max_tokens,
            curr_running: 0,
            notifiers: VecDeque::new(),
        };

        TokenSource {
            max_tokens,
            state: Mutex::new(state),
        }
    }

    async fn take_token(&self) {
        let rx = {
            let mut state = self.state.lock().unwrap();
            if state.curr_tokens > 0 {
                state.curr_tokens -= 1;
                state.curr_running += 1;
                return;
            } else {
                let (tx, rx) = oneshot::channel();
                state.notifiers.push_back(tx);
                rx
            }
        };

        rx.await.unwrap()
    }

    fn restore_slot(&self) {
        let mut state = self.state.lock().unwrap();
        state.curr_running -= 1;
    }

    pub async fn run_with_token<F: Future>(&self, fut: F) -> F::Output {
        self.take_token().await;
        let result = fut.await;
        self.restore_slot();
        result
    }

    pub fn add_tokens(&self, num_tokens: usize) {
        if num_tokens == 0 {
            return;
        }
        let mut state = self.state.lock().unwrap();
        let mut num_tokens = std::cmp::min(
            num_tokens,
            self.max_tokens.saturating_sub(state.curr_running),
        );
        while let Some(tx) = state.notifiers.pop_front() {
            // Ignore waiters that were dropped.
            if tx.send(()).is_err() {
                continue;
            }
            state.curr_running += 1;
            num_tokens -= 1;

            if num_tokens == 0 {
                break;
            }
        }

        state.curr_tokens += num_tokens;
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::Mutex;

    #[tokio::test]
    async fn runs_all_available_tokens() {
        let counter = Mutex::new(Vec::new());

        let source = TokenSource::new(3);

        let mut futures = Vec::new();

        for i in 0..3 {
            let counter = &counter;
            futures.push(source.run_with_token(async move { counter.lock().unwrap().push(i) }))
        }

        futures::future::join_all(futures).await;

        assert_eq!(counter.into_inner().unwrap(), vec![0, 1, 2])
    }
}
