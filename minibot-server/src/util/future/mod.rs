pub mod cancel;
pub mod opt_cell;
pub mod park;

use futures::prelude::*;

pub async fn send_all_propagate<In, Out>(stream: In, mut sink: Out) -> Result<(), Out::Error>
where
    In: Stream + Unpin,
    Out: Sink<In::Item> + Unpin,
{
    sink.send_all(&mut stream.map(Result::Ok)).await
}