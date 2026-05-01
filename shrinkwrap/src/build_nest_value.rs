use crate::{
    nest::NestValueType,
    transform::Transform,
};

/// # Generic parameters
///
/// - `T`: The source value type
/// - `V`: The resulting type used in the nest (must implement [`NestValueType`])
pub trait BuildNestValue<T, V>: Transform
where
    V: NestValueType
{
    fn build_nest_value(&self, source: &T, options: &Self::Options) -> V;
}
