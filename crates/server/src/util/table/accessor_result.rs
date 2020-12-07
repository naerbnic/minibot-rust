/// A helper type similar to Cow, but can only access the reference of the containing value.
/// Does not require Clone on T.
pub enum AccessorResult<'a, T> {
    Borrowed(&'a T),
    Owned(T),
}

impl<T> AccessorResult<'_, T> {
    pub fn as_ref(&self) -> &T {
        match self {
            AccessorResult::Borrowed(v) => v,
            AccessorResult::Owned(v) => v,
        }
    }
}
