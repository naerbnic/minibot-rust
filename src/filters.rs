use std::convert::Infallible;
use warp::Filter;

pub fn cloned<T: Clone + Send>(val: T) -> impl Filter<Extract = (T,), Error = Infallible> + Clone {
    warp::any().map(move || val.clone())
}
