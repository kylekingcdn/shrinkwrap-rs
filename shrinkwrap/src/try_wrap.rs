use serde::Serialize;
use std::fmt::Debug;

use crate::transform::Transform;

/// `TryToWrappedWith` is automatically implemented for data structs when all top-level nests have a [`TransformToNest`] impl on each nest type within the group. All impls must be for the same transform type.
///
/// Furthermore, any nests which are deeply nested require a [`TransformToNest`] converting from their respective data source (the parent nest).
pub trait TryToWrappedWith<T>: Debug + Clone + Serialize
where
    T: Transform,
{
    type Wrapper;
    type Error: Debug;

    fn try_to_wrapped_with(self, transform: &T, options: &T::Options) -> Result<Self::Wrapper, Self::Error>;
}

/// Allows for converting a data struct into a wrapper.
///
/// Automatically implemented across types that provide `TryToWrappedWith`.
///
/// The call is initiated from the wrapper Type itself. Aside from that, it is identical to [`try_to_wrapped_with`].
pub trait TryWrapDataWith<D, T>: Sized
where
    T: Transform,
    D: TryToWrappedWith<T>,
{
    fn try_wrap_data_with(data: D, transform: &T, options: &T::Options) -> Result<Self, D::Error>;
}
impl<D, T> TryWrapDataWith<D, T> for <D as TryToWrappedWith<T>>::Wrapper
where
    T: Transform,
    D: TryToWrappedWith<T>,
{
    fn try_wrap_data_with(data: D, transform: &T, options: &<T as Transform>::Options) -> Result<Self, D::Error> {
        data.try_to_wrapped_with(transform, options)
    }
}
