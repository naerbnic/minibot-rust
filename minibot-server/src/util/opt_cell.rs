use futures::channel::mpsc;
use futures::prelude::*;

#[derive(thiserror::Error, Debug)]
#[error("The replacer for this OptCell was dropped.")]
pub struct ReplacerSendError;

#[derive(Clone, Debug)]
pub struct OptCellReplacer<T>(mpsc::Sender<T>);

impl<T> OptCellReplacer<T> {
    pub async fn replace(&mut self, value: T) -> Result<(), ReplacerSendError> {
        self.0.send(value).await.map_err(|_| ReplacerSendError)
    }
}

pub struct OptCell<T> {
    replacer: mpsc::Receiver<T>,
    curr_data: Option<T>,
}

#[derive(thiserror::Error, Debug)]
#[error("The replacer for this OptCell was dropped.")]
pub struct ReplacerDropError;

impl<T> OptCell<T> {
    pub async fn borrow<'a>(&'a mut self) -> Result<&'a mut T, ReplacerDropError> {
        if self.curr_data.is_none() {
            if let Some(value) = self.replacer.next().await {
                self.curr_data = Some(value);
                return Ok(self.curr_data.as_mut().unwrap());
            } else {
                return Err(ReplacerDropError);
            }
        } else {
            Ok(self.curr_data.as_mut().unwrap())
        }
    }

    pub fn drop_value<'a>(&mut self) {
        self.curr_data = None;
    }
}

pub fn opt_cell<T>() -> (OptCell<T>, OptCellReplacer<T>) {
    let (send, recv) = mpsc::channel(0);
    (
        OptCell {
            replacer: recv,
            curr_data: None,
        },
        OptCellReplacer(send),
    )
}
