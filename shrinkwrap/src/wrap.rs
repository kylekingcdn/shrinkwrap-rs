use serde::Serialize;
use std::fmt::Debug;

use crate::transform::Transform;

/// `ToWrappedWith` is automatically implemented for data structs when all top-level nests have a [`TransformToNest`](crate::TransformToNest) impl on each nest type within the group. All impls must be for the same transform type.
///
/// Furthermore, any nests which are deeply nested require a [`TransformToNest`](crate::TransformToNest) converting from their respective data source (the parent nest).
pub trait ToWrappedWith<T>: Debug + Clone + Serialize
where
    T: Transform,
{
    type Wrapper;

    fn to_wrapped_with(self, transform: &T, options: &T::Options) -> Self::Wrapper;
}

/// Allows for converting a data struct into a wrapper.
///
/// Automatically implemented across types that provide [`ToWrappedWith`](crate::ToWrappedWith).
///
/// The call is initiated from the wrapper Type itself. Aside from that, it is identical to [`to_wrapped_with`](crate::ToWrappedWith::to_wrapped_with).
pub trait WrapDataWith<D, T>: Sized
where
    T: Transform,
    D: ToWrappedWith<T>,
{
    fn wrap_data_with(data: D, transform: &T, options: &T::Options) -> Self;
}
impl<D, T> WrapDataWith<D, T> for <D as ToWrappedWith<T>>::Wrapper
where
    T: Transform,
    D: ToWrappedWith<T>,
{
    fn wrap_data_with(data: D, transform: &T, options: &<T as Transform>::Options) -> Self {
        data.to_wrapped_with(transform, options)
    }
}
