use std::fmt::Debug;

use crate::transform::Transform;

/// Fallible version of [`TransformToNest`](crate::TransformToNest)
///
/// See [`TransformToNest`](crate::TransformToNest) for more information
pub trait TryTransformToNest<N>: Transform {
    type Data;
    type Error: Debug;

    fn try_transform_to_nest(&self, data: &Self::Data, options: &Self::Options) -> Result<N, Self::Error>;
}

/// Fallible version of [`ToNestWith`](crate::ToNestWith)
///
/// See [`ToNestWith`](crate::ToNestWith) for more information
pub trait TryToNestWith<N, T: Transform>: Sized
where
    T: TryTransformToNest<N, Data = Self>,
{
    fn try_to_nest_with(&self, transform: &T, options: &T::Options) -> Result<N, T::Error>;
}

/// Blanket implementation providing [`try_to_nest_with`](crate::TryToNestWith::try_to_nest_with) for data structs that have a corresponding [`TryTransformToNest<Nest>`](crate::TryTransformToNest) impl.
impl<D, N, T> TryToNestWith<N, T> for D
where
    T: TryTransformToNest<N, Data = D>,
{
    fn try_to_nest_with(&self, transform: &T, options: &T::Options) -> Result<N, T::Error> {
        transform.try_transform_to_nest(self, options)
    }
}
