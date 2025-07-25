use serde::Serialize;
use std::fmt::Debug;

pub trait Wrap: Debug + Clone + Serialize {
    type Wrapper;
    fn to_wrapped(self) -> Self::Wrapper;
}

pub trait WrapWith<T>: Debug + Clone + Serialize {
    type Wrapper;
    fn to_wrapped_with(self, transform: &T) -> Self::Wrapper;
}
