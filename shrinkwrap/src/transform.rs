/// Converts data to a given nest.
///
/// This shouldn't need to be implemented when using the `Wrap` derive macro.
///
/// Users may optionally implement this trait. This will automatically be implemented when deriving [`crate::Wrap`] for any nests that have the [`from`] attribute enabled.
///
/// The derived implementation simply forwards the conversion to `From<&Data> for Nest`
pub trait ToNest<N> {
    fn to_nest(&self) -> N;
}

/// For proper functionality, this should be implemented by any nests configured to use a dedicated transform handler (rather than providing a `From<&Data>` implementation)
pub trait TransformToNest<D, N> {
    fn transform_to_nest(&self, data: &D) -> N;
}

/// Not intended for external implementation.
///
/// Users should instead implement [`TransformToNest`]. This trait is interoperable and can be implemented on any struct, not just Data structs. This allows for dependency injection and/or state management.
///
/// [`ToNestWith`] will be implemented automatically when [`TransformToNest`] is implemented on the corresponding types.
pub trait ToNestWith<N, T>: Sized
where
    T: TransformToNest<Self, N>,
{
    fn to_nest_with(&self, transform: &T) -> N {
        transform.transform_to_nest(self)
    }
}

/// Blanket implementation providing `to_nest_with(transform)` for data structs that have a corresponding transform impl (`impl TransformToNest<Data, Nest> for MyTransform`)
impl<D, N, T> ToNestWith<N, T> for D where T: TransformToNest<D, N> {}
