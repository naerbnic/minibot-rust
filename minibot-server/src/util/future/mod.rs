pub mod cancel;
pub mod opt_cell;
pub mod park;

use std::borrow::Borrow;

use futures::prelude::*;
use serde::{de::DeserializeOwned, Serialize};

pub async fn pipe<In, Out>(stream: In, mut sink: Out) -> Result<(), Out::Error>
where
    In: Stream + Unpin,
    Out: Sink<In::Item> + Unpin,
{
    sink.send_all(&mut stream.map(Result::Ok)).await
}

#[derive(Copy, Clone, Debug)]
pub enum PipeError<I, O> {
    Stream(I),
    Sink(O),
}

impl<I, O> std::fmt::Display for PipeError<I, O>
where
    I: std::fmt::Display,
    O: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipeError::Stream(e) => f.write_str(&format!("Error from source: {0}", e)),
            PipeError::Sink(e) => f.write_str(&format!("Error from sink: {0}", e)),
        }
    }
}

impl<I, O> std::error::Error for PipeError<I, O>
where
    I: std::error::Error + std::fmt::Debug + 'static,
    O: std::error::Error + std::fmt::Debug + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PipeError::Stream(e) => Some(e),
            PipeError::Sink(e) => Some(e),
        }
    }
}

impl<I, O> PipeError<I, O> {
    pub fn into<T>(self) -> T
    where
        I: Into<T>,
        O: Into<T>,
    {
        match self {
            PipeError::Stream(e) => e.into(),
            PipeError::Sink(e) => e.into(),
        }
    }
}

pub async fn try_map_pipe<In, Out, F, T, E>(
    mut stream: In,
    mut sink: Out,
    mut func: F,
) -> Result<(), PipeError<E, Out::Error>>
where
    In: Stream + Unpin,
    Out: Sink<T> + Unpin,
    F: FnMut(In::Item) -> Result<T, E>,
{
    while let Some(item) = stream.next().await {
        match func(item) {
            Ok(v) => sink.send(v).await.map_err(PipeError::Sink)?,
            Err(e) => {
                return Err(PipeError::Stream(e));
            }
        }
    }

    Ok(())
}


pub async fn try_stream_pipe<In, Out>(
    stream: In,
    sink: Out,
) -> Result<(), PipeError<In::Error, Out::Error>>
where
    In: TryStream + Unpin,
    Out: Sink<In::Ok> + Unpin, {
    try_map_pipe(stream.into_stream(), sink, |item| item).await
}

/// Link up a stream and a sink, where the stream messages are deserialized and passed along.
pub async fn deser_json_pipe<In, Out, T>(
    stream: In,
    sink: Out,
) -> Result<(), PipeError<serde_json::Error, Out::Error>>
where
    In: Stream + Unpin,
    In::Item: Borrow<str>,
    Out: Sink<T> + Unpin,
    T: DeserializeOwned,
{
    try_map_pipe(stream, sink, |item| serde_json::from_str(item.borrow())).await
}

/// Link up a stream and a sink, where the stream messages are serialized and passed along.
pub async fn ser_json_pipe<In, Out>(
    stream: In,
    sink: Out
) -> Result<(), PipeError<serde_json::Error, Out::Error>>
where
    In: Stream + Unpin,
    In::Item: Serialize,
    Out: Sink<String> + Unpin,
{
    try_map_pipe(stream, sink, |item| serde_json::to_string(&item)).await
}
