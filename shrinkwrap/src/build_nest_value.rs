use crate::{
    nest::NestValueType,
    transform::Transform,
};

pub trait BuildNestValue<T, V>: Transform
where
    V: NestValueType
{
    fn build_nest_value(&self, source: &T, options: &Self::Options) -> V;
}
