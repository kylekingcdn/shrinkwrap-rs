use crate::{
    nest::NestValueType,
    transform::Transform,
};

pub trait TryBuildNestValue<T, V>: Transform
where
    V: NestValueType
{
    type Error;

    fn try_build_nest_value(&self, source: &T, options: &Self::Options) -> Result<V, Self::Error>;
}
