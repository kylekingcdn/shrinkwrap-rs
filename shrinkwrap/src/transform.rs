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
