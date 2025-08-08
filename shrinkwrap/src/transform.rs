use serde::Serialize;
use std::fmt::Debug;

/// Marker trait for a transform impl
///
/// Additionally provides the type for the options parameter used in [`TransformToNest`] and [`ToNestWith`]
///
/// ## Example impl
///
/// ```
/// use shrinkwrap::Transform;
///
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
/// # Examples
///
/// ## Standard
///
/// For regular (non-optional, without deep nesting), the `impl` should look like:
///
/// ```
/// # use shrinkwrap::{Transform, Wrap};
/// #
/// # #[derive(Debug, Clone, serde::Serialize, Wrap)]
/// # #[shrinkwrap(transform = MyTransform)]
/// # #[shrinkwrap(nest(id = "text", field_type = String))]
/// # pub struct MyData {
/// #     #[shrinkwrap(nests("text"))]
/// #     uptime_sec: i64,
/// # }
/// #
/// # struct MyTransform {}
/// # type MyTransformOpts = ();
/// # impl Transform for MyTransform {
/// #     type Options = MyTransformOpts;
/// # }
/// use shrinkwrap::TransformToNest;
///
/// impl TransformToNest<MyDataNestedText> for MyTransform {
///     type Data = MyData;
///     fn transform_to_nest(&self, data: &MyData, _: &MyTransformOpts) -> MyDataNestedText {
///         MyDataNestedText {
///             uptime_sec: data.uptime_sec.to_string(),
///         }
///     }
/// }
/// ```
///
/// ## Optional
///
/// If your nest is optional, the `impl` should look like:
///
/// ```
/// # use shrinkwrap::{Transform, Wrap};
/// #
/// # struct MyTransform {}
/// # struct MyTransformOpts {
/// #     with_text: bool,
/// # };
/// # impl Transform for MyTransform {
/// #     type Options = MyTransformOpts;
/// # }
/// #
/// # #[derive(Debug, Clone, serde::Serialize, Wrap)]
/// # #[shrinkwrap(transform = MyTransform)]
/// # #[shrinkwrap(nest(id = "text", field_type = String, optional))]
/// # pub struct MyData {
/// #     #[shrinkwrap(nests("text"))]
/// #     uptime_sec: i64,
/// # }
/// use shrinkwrap::TransformToNest;
///
/// impl TransformToNest<Option<MyDataNestedText>> for MyTransform {
///     type Data = MyData;
///
///     fn transform_to_nest(&self, data: &MyData, options: &MyTransformOpts) -> Option<MyDataNestedText> {
///         options.with_text.then(||
///             MyDataNestedText {
///                 uptime_sec: data.uptime_sec.to_string(),
///             }
///         )
///     }
/// }
/// ```
///
/// ## Deeply Nested
///
/// If the nest is layered under some other nest (deeply nested), the `impl` has a similar structure to the standard impl.
/// The only real change is instead of using the primary data source (`MyData`) as the associated data type, you would use the parent nest.
///
/// This example assumes two nests, a top-level/standard nest `usd_value`, and a deeply nested `text` under `usd_value`
///
/// ```
/// # use shrinkwrap::{Transform, Wrap};
/// #
/// # #[derive(Debug, Clone, serde::Serialize, Wrap)]
/// # #[shrinkwrap(transform = MyTransform)]
/// # #[shrinkwrap(nest(id = "text", field_type = String))]
/// # pub struct MyData {
/// #     #[shrinkwrap(nests("text"))]
/// #     uptime_sec: i64,
/// # }
/// #
/// # struct MyTransform {}
/// # type MyTransformOpts = ();
/// # impl Transform for MyTransform {
/// #     type Options = MyTransformOpts;
/// # }
/// use shrinkwrap::TransformToNest;
///
/// impl TransformToNest<TestDataNestedUsdValueText> for MyTransform {
///     type Data = TestDataNestedUsdValue;
///
///     fn transform_to_nest(&self, data: &TestDataNestedUsdValue, _: &MyTransformOpts) -> TestDataNestedUsdValueText {
///         TestDataNestedUsdValueText {
///             amount: format!("${:.2} USD", data.amount),
///         }
///     }
/// }
/// ```
///
/// ## Optional + Deeply Nested
///
/// Nothing special here, it's a combination of the modifications used in the previous two examples.
///
/// ```
/// # use shrinkwrap::{Transform, Wrap};
/// #
/// # #[derive(Debug, Clone, serde::Serialize, Wrap)]
/// # #[shrinkwrap(transform = MyTransform)]
/// # #[shrinkwrap(nest(id = "text", field_type = String))]
/// # pub struct MyData {
/// #     #[shrinkwrap(nests("text"))]
/// #     uptime_sec: i64,
/// # }
/// #
/// # struct MyTransform {}
/// # type MyTransformOpts = ();
/// # impl Transform for MyTransform {
/// #     type Options = MyTransformOpts;
/// # }
/// use shrinkwrap::TransformToNest;
///
/// impl TransformToNest<Option<TestDataNestedUsdValueText>> for MyTransform {
///     type Data = TestDataNestedUsdValue;
///
///     fn transform_to_nest(&self, data: &TestDataNestedUsdValue, _: &MyTransformOpts) -> Option<TestDataNestedUsdValueText> {
///         options.with_text.then(||
///             TestDataNestedUsdValueText {
///                 amount: format!("${:.2} USD", data.amount),
///             }
///         )
///     }
/// }
/// ```
///
/// # Notes
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

/// Allows for converting a data struct (by reference) to a supported nest type.
///
/// This is implemented automatically when [`TransformToNest`] is implemented on the corresponding types.
///
/// ## Examples
///
/// ```rust
/// use serde::Serialize;
/// use shrinkwrap::{ToNestWith, Transform, TransformToNest, Wrap};
///
/// #[derive(Debug, Clone, Serialize, Wrap)]
/// #[shrinkwrap(transform = MyTransform)]
/// #[shrinkwrap(nest(id = "text", field_type = String))]
/// pub struct MyData {
///     #[shrinkwrap(nests("text"))]
///     uptime_sec: i64,
/// }
///
/// struct MyTransform {}
/// type MyTransformOpts = ();
/// impl Transform for MyTransform {
///     type Options = MyTransformOpts;
/// }
///
/// impl TransformToNest<MyDataNestedText> for MyTransform {
///     type Data = MyData;
///     fn transform_to_nest(&self, data: &MyData, _options: &MyTransformOpts) -> MyDataNestedText {
///         MyDataNestedText {
///             uptime_sec: data.uptime_sec.to_string(),
///         }
///     }
/// }
///
/// let transform = MyTransform {};
/// let transform_opts = ();
/// let data = MyData {
///     uptime_sec: 10
/// };
///
/// let text_variants: MyDataNestedText = data.to_nest_with(&transform, &transform_opts);
/// let uptime_text = text_variants.uptime_sec;
/// println!("Current uptime: {uptime_text}")
/// ```
pub trait ToNestWith<N, T: Transform>: Sized
where
    T: TransformToNest<N, Data = Self>,
{
    fn to_nest_with(&self, transform: &T, options: &T::Options) -> N;
}

/// Blanket implementation providing `to_nest_with(transform)` for data structs that have a corresponding [`TransformToNest<Nest>`] impl.
impl<D, N, T> ToNestWith<N, T> for D
where
    T: TransformToNest<N, Data = D>,
{
    fn to_nest_with(&self, transform: &T, options: &T::Options) -> N {
        transform.transform_to_nest(self, options)
    }
}

/// `ToWrappedWith` is automatically implemented for data structs when all top-level nests have a [`TransformToNest`] impl on each nest type within the group. All impls must be for the same transform type.
///
/// Furthermore, any nests which are deeply nested require a [`TransformToNest`] converting from their respective data source (the parent nest).
pub trait ToWrappedWith<T>: Debug + Clone + Serialize
where
    T: Transform,
{
    type Wrapper;

    fn to_wrapped_with(self, transform: &T, options: &T::Options) -> Self::Wrapper;
}

/// Allows for converting a data struct into a wrapper.
///
/// Automatically implemented across types that provide `ToWrappedWith`.
///
/// The call is initiated from the wrapper Type itself. Aside from that, it is identical to [`to_wrapped_with`].
pub trait WrapDataWith<D, T>
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
