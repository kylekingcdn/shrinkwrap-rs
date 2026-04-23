pub mod nest;
pub mod transform;
pub mod try_nest;
pub mod try_wrap;
pub mod wrap;

pub use crate::{
    nest::{ToNestWith, TransformToNest},
    transform::Transform,
    try_nest::{TryToNestWith, TryTransformToNest},
    try_wrap::{TryWrapDataWith, TryToWrappedWith},
    wrap::{ToWrappedWith, WrapDataWith},
};
pub use shrinkwrap_macros::Wrap;


pub trait NestValueType {}

pub trait NestGroup {
    type Value: NestValueType;
}

pub trait BuildNestValue<T, NV: NestValueType>: Transform {
    fn build_nest_value(&self, source: &T, options: &Self::Options) -> NV;
}

pub trait TryBuildNestValue<T, NV: NestValueType>: Transform {
    type Error;
    fn try_build_nest_value(&self, source: &T, options: &Self::Options) -> Result<NV, Self::Error>;
}
