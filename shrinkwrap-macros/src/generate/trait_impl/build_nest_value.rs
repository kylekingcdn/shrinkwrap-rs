use super::*;

// !- BuildNestValueTrait

#[derive(Debug, Copy, Clone)]
pub(crate) struct BuildNestValueTrait;

impl TransformTrait for BuildNestValueTrait {
    fn trait_name() -> Ident { format_ident!("BuildNestValue") }
    fn trait_fn() -> Ident { format_ident!("build_nest_value") }
}

pub(crate) type BuildNestValueVariant = TraitFallibility<BuildNestValueTrait>;
