use super::token_source::TokenSource;
use futures::future::{abortable, AbortHandle};
use futures::prelude::*;
use std::sync::Arc;

pub struct ThrottledTokenSource {
    source: Arc<TokenSource>,
    timer_handle: AbortHandle,
}

impl ThrottledTokenSource {
    pub fn new(max_tokens: usize, interval: std::time::Duration) -> Self {
        let source = Arc::new(TokenSource::new(max_tokens));

        let (timer_task, timer_handle) = abortable({
            let source = source.clone();
            async move {
                tokio::time::interval(interval)
                    .for_each(move |_| {
                        let source = source.clone();
                        async move { source.add_tokens(1) }
                    })
                    .await;
            }
        });

        tokio::spawn(async move {
            let _ = timer_task.await;
        });

        ThrottledTokenSource {
            source,
            timer_handle,
        }
    }

    pub async fn run_with_token<F: Future>(&self, task: F) -> F::Output {
        self.source.run_with_token(task).await
    }
}

impl Drop for ThrottledTokenSource {
    fn drop(&mut self) {
        self.timer_handle.abort();
    }
}
