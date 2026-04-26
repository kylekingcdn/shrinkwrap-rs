use super::*;

// !- TransformToNestTrait

#[derive(Debug, Copy, Clone)]
pub(crate) struct TransformToNestTrait;

impl TransformTrait for TransformToNestTrait {
    fn trait_name() -> Ident { format_ident!("TransformToNest") }
    fn trait_fn() -> Ident { format_ident!("transform_to_nest") }
}

pub(crate) type TransformToNestVariant = TraitFallibility<TransformToNestTrait>;

