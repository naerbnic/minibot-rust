use std::convert::Infallible;
use warp::Filter;

pub fn cloned<T: Clone + Send, R>(val: T) -> impl Filter<Extract = (T,), Error = Infallible> {
    warp::any().map(move || val.clone())
}
