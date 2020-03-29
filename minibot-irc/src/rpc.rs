use crate::connection::{IrcSink, IrcStream};
use crate::messages::Message;
use futures::channel::oneshot;
use futures::lock::Mutex;
use futures::prelude::*;
use std::sync::Arc;

#[derive(Copy, Clone, Debug)]
pub enum FilterResult {
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

    #[error("")]
    HandlerError(#[source] Box<dyn std::error::Error + Send>),
}

struct RpcState {
    response_messages: Vec<Message>,
    call: Box<dyn ObjectSafeRpcCall + Sync + Send>,
}

struct RpcStateAndChannel {
    state: RpcState,
    channel: oneshot::Sender<Result<RpcState, Box<dyn std::any::Any + Send + 'static>>>,
}

struct StreamState(Option<RpcStateAndChannel>);

pub struct IrcRpcConnection {
    sink: IrcSink,
    stream_abort: future::AbortHandle,
    stream_state: Arc<Mutex<StreamState>>,
}

pub trait RpcCall {
    type Output;
    type Err: std::error::Error + std::any::Any + Send + 'static;
    fn send_messages(&self) -> Vec<Message>;
    fn msg_filter(&self, msg: &Message) -> Result<FilterResult, Self::Err>;
    fn recv_messages(&self, msgs: Vec<Message>) -> Result<Self::Output, Self::Err>;
}

#[derive(thiserror::Error, Debug)]
pub enum RpcCallError<E: std::error::Error + 'static> {
    #[error("Error while processing: {0}")]
    CallError(#[source] E),

    #[error("Rpc cancelled by stream")]
    RpcCancelledError,
}

trait ObjectSafeRpcCall {
    fn msg_filter(
        &self,
        msg: &Message,
    ) -> Result<FilterResult, Box<dyn std::any::Any + Send + 'static>>;
    fn to_inner(self: Box<Self>) -> Box<dyn std::any::Any + 'static>;
}

struct ObjectSafeCallWrapper<T>(T);

impl<T: RpcCall + 'static> ObjectSafeRpcCall for ObjectSafeCallWrapper<T> {
    fn msg_filter(
        &self,
        msg: &Message,
    ) -> Result<FilterResult, Box<dyn std::any::Any + Send + 'static>> {
        self.0.msg_filter(msg).map_err(|e| {
            let new_e: Box<dyn std::any::Any + Send + 'static> = Box::new(e);
            new_e
        })
    }

    fn to_inner(self: Box<Self>) -> Box<dyn std::any::Any + 'static> {
        let ObjectSafeCallWrapper(t) = *self;
        Box::new(t)
    }
}

impl IrcRpcConnection {
    pub fn new<F, Fut, E>(mut stream: IrcStream, sink: IrcSink, mut msg_handler: F) -> Self
    where
        F: FnMut(Message) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), E>> + Send,
        E: std::error::Error + Send + 'static,
    {
        let stream_state = Arc::new(Mutex::new(StreamState(None)));
        let handler_future = {
            let stream_state = stream_state.clone();
            async move {
                while let Some(m) = stream.try_next().await? {
                    let mut guard = stream_state.lock().await;
                    if let Some(rpc) = &mut guard.0 {
                        match rpc.state.call.msg_filter(&m) {
                            Ok(r) => match r {
                                FilterResult::Next => rpc.state.response_messages.push(m),
                                FilterResult::Skip => msg_handler(m)
                                    .await
                                    .map_err(|e| Error::HandlerError(Box::new(e)))?,
                                FilterResult::End => {
                                    let RpcStateAndChannel { mut state, channel } =
                                        guard.0.take().unwrap();
                                    state.response_messages.push(m);
                                    let _ = channel.send(Ok(state));
                                }
                            },
                            Err(e) => {
                                let RpcStateAndChannel { channel, .. } = guard.0.take().unwrap();
                                let _ = channel.send(Err(e));
                            }
                        }
                    } else {
                        msg_handler(m)
                            .await
                            .map_err(|e| Error::HandlerError(Box::new(e)))?;
                    }
                }
                Ok::<(), Error>(())
            }
        };
        let (handler_future, stream_abort) = future::abortable(handler_future);
        tokio::spawn(handler_future);

        IrcRpcConnection {
            sink,
            stream_abort,
            stream_state,
        }
    }

    pub async fn call<T: RpcCall + Sync + Send + 'static>(
        &mut self,
        call: T,
    ) -> Result<T::Output, RpcCallError<T::Err>> {
        let messages = call.send_messages();
        let (tx, rx) = oneshot::channel();
        {
            let mut guard = self.stream_state.lock().await;
            assert!(guard.0.is_none());
            guard.0 = Some(RpcStateAndChannel {
                state: RpcState {
                    response_messages: Vec::new(),
                    call: Box::new(ObjectSafeCallWrapper(call)),
                },
                channel: tx,
            });
        }
        self.sink
            .send_all(&mut stream::iter(messages).map(|m| Ok(m)))
            .await
            .map_err(|_| RpcCallError::RpcCancelledError)?;
        match rx.await.map_err(|_| RpcCallError::RpcCancelledError)? {
            Ok(state) => {
                let RpcState {
                    response_messages,
                    call,
                } = state;
                let call = call.to_inner().downcast::<T>().unwrap();
                Ok(call
                    .recv_messages(response_messages)
                    .map_err(RpcCallError::CallError)?)
            }
            Err(e) => Err(RpcCallError::CallError(*e.downcast::<T::Err>().unwrap())),
        }
    }
}
