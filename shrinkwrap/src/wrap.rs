use serde::Serialize;
use std::fmt::Debug;

use crate::transform::Transform;

// TODO: add generic wrapper struct for use for both derive and manual impls

pub trait Wrap: Debug + Clone + Serialize {
    type Wrapper;
    fn to_wrapped(self) -> Self::Wrapper;
}

pub trait WrapWith<T>: Debug + Clone + Serialize
where
    T: Transform
{
    type Wrapper;
    fn to_wrapped_with(self, transform: &T, options: &T::Options) -> Self::Wrapper;
}
