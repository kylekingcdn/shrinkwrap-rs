use serde::Serialize;
use std::fmt::Debug;

// TODO: move all non-primary generics into assoc. types

/// Marker trait for a transform impl
///
/// Additionally provides the type for the options parameter used in [`TransformToNest`] and [`ToNestWith`]
///
/// ## Example impl
///
/// ```
/// struct MyTransformOpts {
///     with_text: bool,
///     with_value: bool,
/// }
///
/// struct MyTransform {}
///
/// impl Transform for MyTransform {
///     type Options = MyTransformOpts;
/// }
/// ```
pub trait Transform {
    type Options;
}

/// Primary entrypoint for data -> nest conversions.
///
/// You should implement this on each corresponding nest for your transform.
///
/// Implementations are on the [`Transform`] struct that you have defined to handle conversions.
///
/// ## Examples
///
/// ### Standard
///
/// For regular (non-optional, without deep nesting), the `impl` should look like:
///
/// ```
/// impl TransformToNest<MyDataNestedText> for MyTransform {
///     type Data = MyData;
///     fn transform_to_nest(&self, data: &MyData, _: &MyTransformOptions) -> MyDataNestedText {
///         MyDataNestedText {
///             uptime_sec: data.uptime_sec.to_string(),
///         }
///     }
/// }
/// ```
///
/// ### Optional
///
/// If your nest is optional, the `impl` should look like:
///
/// ```
/// impl TransformToNest<Option<MyDataNestedText>> for MyTransform {
///     type Data = MyData;
///
///     fn transform_to_nest(&self, data: &MyData, options: &MyTransformOptions) -> Option<MyDataNestedText> {
///         options.with_text.then(||
///             MyDataNestedText {
///                 uptime_sec: data.uptime_sec.to_string(),
///             }
///         )
///     }
/// }
/// ```
///
/// ### Deeply Nested
///
/// If the nest is layered under some other nest (deeply nested), the `impl` has a similar structure to the standard impl.
/// The only real change is instead of using the primary data source (`MyData`) as the associated data type, you would use the parent nest.
///
/// This example assumes two nests, a top-level/standard nest `usd_value`, and a deeply nested `text` under `usd_value`
///
/// ```
/// impl TransformToNest<TestDataNestedUsdValueText> for MyTransform {
///     type Data = TestDataNestedUsdValue;
///
///     fn transform_to_nest(&self, data: &TestDataNestedUsdValue, _: &MyTransformOptions) -> TestDataNestedUsdValueText {
///         TestDataNestedUsdValueText {
///             amount: format!("${:.2} USD", data.amount),
///         }
///     }
/// }
/// ```
///
/// ### Optional + Deeply Nested
///
/// Nothing special here, it's a combination of the modifications used in the previous two examples.
///
/// ```
/// impl TransformToNest<Option<TestDataNestedUsdValueText>> for MyTransform {
///     type Data = TestDataNestedUsdValue;
///
///     fn transform_to_nest(&self, data: &TestDataNestedUsdValue, _: &MyTransformOptions) -> Option<TestDataNestedUsdValueText> {
///         options.with_text.then(||
///             TestDataNestedUsdValueText {
///                 amount: format!("${:.2} USD", data.amount),
///             }
///         )
///     }
/// }
/// ```
///
/// ## Notes
///
/// When a nest has child nests layered under it (deeply nested), it's type will be swapped out with a dedicated 'injected' wrapper.
///
/// However, this does not affect the trait impls above - the `Wrap` derive macro automatically adds an implementation for the wrapper->nest translation.
///
/// The only requirement is that `TransformToNest` is implemented from the data source to the nest type.
pub trait TransformToNest<N>: Transform {
    type Data;
    fn transform_to_nest(&self, data: &Self::Data, options: &Self::Options) -> N;
}

/// Not intended for implementation by consumers. Users should instead implement [`TransformToNest`].
///
/// [`ToNestWith`] will be implemented automatically when [`TransformToNest`] is implemented on the corresponding types.
pub trait ToNestWith<N, T: Transform>: Sized
where
    T: TransformToNest<N, Data = Self>,
{
    fn to_nest_with(&self, transform: &T, options: &T::Options) -> N;

}
/// Blanket implementation providing `to_nest_with(transform)` for data structs that have a corresponding transform impl (`impl TransformToNest<Nest> for MyTransform`)
impl<D, N, T> ToNestWith<N, T> for D
where
    T: TransformToNest<N, Data = D>
{
    fn to_nest_with(&self, transform: &T, options: &T::Options) -> N {
        transform.transform_to_nest(self, options)
    }
}

/// `ToWrappedWith` is automatically implemented when all top-level nests have a [`TransformToNest`] impl on each nest type within the group. The same transform type  is implied (and cannot be configured anyhow).
///
/// Furthermore, any nests which are deeply nested require a [`TransformToNest`] converting from their respective data source (the parent nest).
pub trait ToWrappedWith<T>: Debug + Clone + Serialize
where
    T: Transform
{
    type Wrapper;

    fn to_wrapped_with(self, transform: &T, options: &T::Options) -> Self::Wrapper;
}

/// Automatically implemented across types that provide `ToWrappedWith`.
///
/// This allows for converting data via the Wrapper type instead
pub trait WrapDataWith<D, T>
where
    T: Transform,
    D: ToWrappedWith<T>
{
    fn wrap_data_with(data: D, transform: &T, options: &T::Options) -> Self;
}
impl <D,T> WrapDataWith<D,T> for <D as ToWrappedWith<T>>::Wrapper
where
    T: Transform,
    D: ToWrappedWith<T>,
{
    fn wrap_data_with(data: D, transform: &T, options: &<T as Transform>::Options) -> Self {
        data.to_wrapped_with(transform, options)
    }
}
