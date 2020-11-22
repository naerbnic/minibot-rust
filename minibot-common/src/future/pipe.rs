mod cloner;
mod safe_sender;

use futures::channel::{mpsc, oneshot};
use futures::prelude::*;
use futures::stream::BoxStream;

use super::pipe as run_pipe;

#[derive(Copy, Clone, Debug)]
pub enum Either<A, B> {
    Left(A),
    Right(B),
}

pub struct PipeStart<T>(mpsc::Sender<T>);

impl<T> PipeStart<T> {
    fn into_mpsc(self) -> mpsc::Sender<T> {
        self.0
    }
}

#[derive(thiserror::Error, Debug)]
#[error("This sink was closed.")]
pub struct SinkClosed;

pub type BoxSink<T> = Box<dyn Sink<T, Error = SinkClosed> + Send>;

impl<T> PipeStart<T>
where
    T: Send + 'static,
{
    pub fn wrap<S>(sink: S) -> Self
    where
        S: Sink<T> + Unpin + Send + 'static,
        S::Error: Send,
    {
        let (help_start, help_end) = mpsc::channel(0);
        tokio::spawn(run_pipe(help_end, sink));
        PipeStart(help_start)
    }

    pub fn split(self) -> (Self, Self) {
        let sink = self.into_mpsc();
        (PipeStart(sink.clone()), PipeStart(sink))
    }

    pub fn connect(self, other: PipeEnd<T>) {
        other.connect(self)
    }

    pub fn connect_to_stream<S>(self, other: S)
    where
        S: Stream<Item = T> + Unpin + Send + 'static,
    {
        tokio::spawn(run_pipe(other, self.into_mpsc()));
    }

    pub fn into_sink(self) -> BoxSink<T> {
        Box::new(self.into_mpsc().sink_map_err(|_| SinkClosed))
    }
}

enum PipeEndContents<T> {
    Simple(Option<mpsc::Receiver<T>>),
    Cloned(cloner::ClonerHandle<T>),
}

pub struct PipeEnd<T>(std::sync::Mutex<PipeEndContents<T>>);

impl<T> PipeEnd<T>
where
    T: Send + 'static,
{
    fn into_mpsc(self) -> mpsc::Receiver<T> {
        match self.0.into_inner().unwrap() {
            PipeEndContents::Simple(recv) => recv.unwrap(),
            PipeEndContents::Cloned(mut handle) => {
                let (start, end) = mpsc::channel(0);
                tokio::spawn(async move { handle.add_sender(start).await });
                end
            }
        }
    }

    fn from_mpsc(stream: mpsc::Receiver<T>) -> Self {
        PipeEnd(std::sync::Mutex::new(PipeEndContents::Simple(Some(stream))))
    }

    pub fn wrap<S>(stream: S) -> Self
    where
        S: Stream<Item = T> + Unpin + Send + 'static,
    {
        let (help_start, help_end) = mpsc::channel(0);
        tokio::spawn(run_pipe(stream, help_start));
        PipeEnd::from_mpsc(help_end)
    }

    pub fn merge(self, other: Self) -> Self {
        use tokio::stream::StreamExt;
        PipeEnd::wrap(self.into_mpsc().merge(other.into_mpsc()))
    }

    pub fn map<F, U>(self, f: F) -> PipeEnd<U>
    where
        F: FnMut(T) -> U + Send + 'static,
        U: Send + 'static,
    {
        PipeEnd::wrap(self.into_mpsc().map(f))
    }

    pub fn map_into<U>(self) -> PipeEnd<U>
    where
        T: Into<U>,
        U: Send + 'static,
    {
        self.map(|t| t.into())
    }

    pub fn end_map<F, U>(self, mut f: F) -> PipeEnd<U>
    where
        F: FnMut(T) -> Option<U> + Send + 'static,
        U: Send + 'static,
    {
        let (mut send, recv) = mpsc::channel(0);
        tokio::spawn(async move {
            let mut stream = self.into_mpsc();
            while let Some(item) = stream.next().await {
                let next_val = f(item);
                match next_val {
                    Some(next_val) => {
                        if let Err(_) = send.send(next_val).await {
                            break;
                        }
                    }
                    None => break,
                }
            }
        });

        PipeEnd::wrap(recv)
    }

    pub fn connect(self, pipe_start: PipeStart<T>) {
        tokio::spawn(run_pipe(self.into_mpsc(), pipe_start.into_mpsc()));
    }

    pub fn filter_either_split<F, A, B>(self, f: F) -> (PipeEnd<A>, PipeEnd<B>)
    where
        F: FnMut(T) -> Option<Either<A, B>> + Send + 'static,
        A: Send + 'static,
        B: Send + 'static,
    {
        self.filter_map(f).either_split(|i| i)
    }

    pub fn either_split<F, A, B>(self, mut f: F) -> (PipeEnd<A>, PipeEnd<B>)
    where
        F: FnMut(T) -> Either<A, B> + Send + 'static,
        A: Send + 'static,
        B: Send + 'static,
    {
        let mut stream = self.into_mpsc();
        let (mut t_start, t_end) = mpsc::channel(0);
        let (mut f_start, f_end) = mpsc::channel(0);
        tokio::spawn(async move {
            while let Some(item) = stream.next().await {
                let send_fut = match f(item) {
                    Either::Left(a) => t_start.send(a).left_future(),
                    Either::Right(b) => f_start.send(b).right_future(),
                };

                if let Err(_) = send_fut.await {
                    break;
                }
            }
        });

        (PipeEnd::from_mpsc(t_end), PipeEnd::from_mpsc(f_end))
    }

    pub fn filter_map<F, U>(self, mut f: F) -> PipeEnd<U>
    where
        F: FnMut(T) -> Option<U> + Send + 'static,
        U: Send + 'static,
    {
        let mut stream = self.into_mpsc();
        let (mut help_start, help_end) = mpsc::channel(0);
        tokio::spawn(async move {
            while let Some(item) = stream.next().await {
                if let Some(result) = f(item) {
                    if let Err(_) = help_start.send(result).await {
                        break;
                    }
                }
            }
        });

        PipeEnd::from_mpsc(help_end)
    }

    pub fn filter<F>(self, mut f: F) -> PipeEnd<T>
    where
        F: FnMut(&T) -> bool + Send + 'static,
    {
        self.filter_map(move |item| if f(&item) { Some(item) } else { None })
    }

    pub fn connect_to_sink<S>(self, sink: S)
    where
        S: Sink<T> + Unpin + Send + 'static,
        S::Error: Send,
    {
        tokio::spawn(run_pipe(self.into_mpsc(), sink));
    }

    pub fn into_stream(self) -> BoxStream<'static, T> {
        self.into_mpsc().boxed()
    }
}

impl<T> Clone for PipeEnd<T>
where
    T: Clone + Send + 'static,
{
    fn clone(&self) -> Self {
        let mut guard = self.0.lock().unwrap();
        match &mut *guard {
            PipeEndContents::Simple(recv) => {
                let handle = cloner::ClonerHandle::new(recv.take().unwrap());
                let cloned_handle = handle.clone();
                *guard = PipeEndContents::Cloned(handle);
                PipeEnd(std::sync::Mutex::new(PipeEndContents::Cloned(
                    cloned_handle,
                )))
            }
            PipeEndContents::Cloned(handle) => PipeEnd(std::sync::Mutex::new(
                PipeEndContents::Cloned(handle.clone()),
            )),
        }
    }
}

impl<T, E> PipeEnd<Result<T, E>>
where
    T: Send + 'static,
    E: Send + 'static,
{
    pub fn end_on_error_oneshot(self, err: oneshot::Sender<E>) -> PipeEnd<T> {
        let mut stream = self.into_mpsc();
        let (mut ok_start, ok_end) = mpsc::channel(0);
        tokio::spawn(async move {
            while let Some(item) = stream.next().await {
                match item {
                    Ok(item) => {
                        if let Err(_) = ok_start.send(item).await {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = err.send(e);
                        break;
                    }
                }
            }
        });
        PipeEnd::from_mpsc(ok_end)
    }

    pub fn end_on_error(self) -> PipeEnd<T> {
        let (send, _recv) = oneshot::channel();
        self.end_on_error_oneshot(send)
    }
}

pub fn merge_into<T>(
    left: PipeEnd<impl Into<T> + Send + 'static>,
    right: PipeEnd<impl Into<T> + Send + 'static>,
) -> PipeEnd<T>
where
    T: Send + 'static,
{
    left.map_into().merge(right.map_into())
}

pub fn pipe<T>() -> (PipeStart<T>, PipeEnd<T>)
where
    T: Send + 'static,
{
    let (start, end) = mpsc::channel(0);
    (PipeStart(start), PipeEnd::from_mpsc(end))
}

// -----------
