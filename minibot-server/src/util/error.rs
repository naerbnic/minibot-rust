use std::error::Error;

pub trait FromInternalError {
    fn from_internal<E: Error + Send + 'static>(err: E) -> Self;
}

pub trait ResultExt {
    type T;
    fn map_err_internal<E: FromInternalError>(self) -> Result<Self::T, E>;
}

impl <T, E: Error + Send + 'static> ResultExt for Result<T, E> {
    type T = T;
    fn map_err_internal<E2: FromInternalError>(self) -> Result<T, E2> {
        self.map_err(E2::from_internal)
    }
}