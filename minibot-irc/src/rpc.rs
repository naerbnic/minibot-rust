use crate::connection::{IrcSink, IrcStream};
use crate::messages::Message;
use async_trait::async_trait;
use futures::channel::oneshot;
use futures::lock::Mutex;
use futures::prelude::*;
use std::sync::Arc;

#[derive(Copy, Clone, Debug)]
pub enum HandlerResult {
    Next,
    Skip,
    End,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("")]
    ConnectionError(#[from] crate::connection::Error),

    #[error("")]
    RpcCancelledError,
}

#[async_trait]
pub trait ResponseHandler {
    async fn filter(&mut self, m: &Message) -> Result<HandlerResult, Error>;
    async fn handle(&mut self, msgs: Vec<Message>) -> Result<(), Error>;
}

#[async_trait]
pub trait ServerMessageHandler {
    async fn handle(&mut self, m: Message) -> Result<(), Error>;
}

struct RunningRpc {
    response_messages: Vec<Message>,
    filter: Box<dyn FnMut(&Message) -> Result<HandlerResult, Error> + Send>,
    handler: oneshot::Sender<Result<Vec<Message>, Error>>,
}

struct StreamState(Option<RunningRpc>);

pub struct IrcClient {
    sink: IrcSink,
    stream_handle: tokio::task::JoinHandle<Result<(), Error>>,
    stream_state: Arc<Mutex<StreamState>>,
}

impl IrcClient {
    pub fn new(
        mut stream: IrcStream,
        sink: IrcSink,
        mut msg_handler: Box<dyn ServerMessageHandler + Send>,
    ) -> Self {
        let stream_state = Arc::new(Mutex::new(StreamState(None)));
        let stream_handle = tokio::spawn({
            let stream_state = stream_state.clone();
            async move {
                while let Some(m) = stream.try_next().await? {
                    let mut guard = stream_state.lock().await;
                    if let Some(rpc) = &mut guard.0 {
                        match (rpc.filter)(&m) {
                            Ok(r) => match r {
                                HandlerResult::Next => rpc.response_messages.push(m),
                                HandlerResult::Skip => msg_handler.handle(m).await?,
                                HandlerResult::End => {
                                    let RunningRpc {
                                        mut response_messages,
                                        handler,
                                        ..
                                    } = guard.0.take().unwrap();
                                    response_messages.push(m);
                                    let _ = handler.send(Ok(response_messages));
                                }
                            },
                            Err(e) => {
                                let RunningRpc {
                                    handler,
                                    ..
                                } = guard.0.take().unwrap();
                                let _ = handler.send(Err(e));
                            }
                        }
                    } else {
                        msg_handler.handle(m).await?;
                    }
                }
                Ok::<(), Error>(())
            }
        });

        IrcClient {
            sink,
            stream_handle,
            stream_state,
        }
    }

    pub async fn send_rpc<F: FnMut(&Message) -> Result<HandlerResult, Error> + Send + 'static>(
        &mut self,
        messages: impl IntoIterator<Item = Message>,
        filter: F,
    ) -> Result<Vec<Message>, Error> {
        let (tx, rx) = oneshot::channel();
        {
            let mut guard = self.stream_state.lock().await;
            assert!(guard.0.is_none());
            guard.0 = Some(RunningRpc {
                response_messages: Vec::new(),
                filter: Box::new(filter),
                handler: tx,
            });
        }
        self.sink
            .send_all(&mut stream::iter(messages).map(|m| Ok(m)))
            .await?;
        let result = rx.await.map_err(|_| Error::RpcCancelledError)?;
        let messages = result?;
        Ok(messages)
    }
}
