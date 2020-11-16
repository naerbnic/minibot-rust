pub mod cancel;
pub mod opt_cell;
pub mod park;

use futures::prelude::*;
use serde::{de::DeserializeOwned, Serialize};

pub async fn send_all_propagate<In, Out>(stream: In, mut sink: Out) -> Result<(), Out::Error>
where
    In: Stream + Unpin,
    Out: Sink<In::Item> + Unpin,
{
    sink.send_all(&mut stream.map(Result::Ok)).await
}

#[derive(Copy, Clone, thiserror::Error, Debug)]
pub enum PatchError<I, O>
where
    I: std::error::Error + 'static,
    O: std::error::Error + 'static,
{
    #[error("Error from source: {0:?}")]
    Stream(#[source] I),
    #[error("Error from sink: {0:?}")]
    Sink(#[source] O),
}

pub async fn send_all_until_error<In, Out>(
    stream: In,
    mut sink: Out,
) -> Result<(), PatchError<In::Error, Out::Error>>
where
    In: TryStream + Unpin,
    Out: Sink<In::Ok> + Unpin,
    In::Error: std::error::Error + 'static,
    Out::Error: std::error::Error + 'static,
{
    let mut stream = stream.into_stream();
    while let Some(try_item) = stream.next().await {
        match try_item {
            Ok(item) => match sink.send(item).await {
                Ok(()) => {}
                Err(e) => return Err(PatchError::Sink(e)),
            },
            Err(e) => return Err(PatchError::Stream(e)),
        }
    }

    Ok(())
}

/// Link up a stream and a sink, where the stream messages are deserialized and passed along.
pub async fn deser_json_stream<In, Out, Item, T>(
    stream: In,
    sink: Out,
) -> Result<(), PatchError<serde_json::Error, Out::Error>>
where
    In: Stream<Item = Item> + Unpin,
    Item: std::borrow::Borrow<str>,
    T: DeserializeOwned,
    Out: Sink<T> + Unpin,
    Out::Error: std::error::Error + 'static,
{
    send_all_until_error(stream.map(|item| serde_json::from_str(item.borrow())), sink).await
}

/// Link up a stream and a sink, where the stream messages are serialized and passed along.
pub async fn ser_json_stream<In, Out, T>(
    stream: In,
    sink: Out,
) -> Result<(), PatchError<serde_json::Error, Out::Error>>
where
    In: Stream<Item = T> + Unpin,
    T: Serialize,
    Out: Sink<String> + Unpin,
    Out::Error: std::error::Error + 'static,
{
    send_all_until_error(stream.map(|item| serde_json::to_string(&item)), sink).await
}
