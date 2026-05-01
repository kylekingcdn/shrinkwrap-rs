use super::*;

use quote::format_ident;
use std::marker::PhantomData;

mod build_nest_value;
#[allow(unused_imports)]
pub(crate) use build_nest_value::{BuildNestValueTrait, BuildNestValueVariant};

mod to_wrapped_with;
#[allow(unused_imports)]
pub(crate) use to_wrapped_with::{GenToWrappedWith, ToWrappedWithVariant};

mod transform_to_deep_nest;
#[allow(unused_imports)]
pub(crate) use transform_to_deep_nest::GenTransformToDeepNest;

mod transform_to_nest;
#[allow(unused_imports)]
pub(crate) use transform_to_nest::{GenTransformToNest, GenTransformToNestOptional, TransformToNestTrait, TransformToNestVariant};

/// Fallible/infallible variant
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Fallibility {
    Infallible,
    Fallible { error_type: Path },
}
impl Fallibility {
    fn fn_call_suffix(&self) -> TokenStream {
        match self {
            Self::Infallible => quote! { },
            Self::Fallible { .. } => quote! { ? },
        }
    }
    fn map_return(&self, ret_tokens: TokenStream) -> TokenStream {
        match &self {
            Self::Infallible => ret_tokens,
            Self::Fallible { .. } => quote! { Ok(#ret_tokens) },
        }
    }
    fn trait_name(&self, name: Ident) -> TokenStream {
        match &self {
            Self::Infallible => quote!(#name),
            Self::Fallible { .. } => {
                let name = format_ident!("Try{name}");
                quote!(#name)
            },
        }
    }
    fn trait_fn(&self, fn_name: Ident) -> TokenStream {
        match &self {
            Self::Infallible => quote!(#fn_name),
            Self::Fallible { .. } => {
                let fn_name = format_ident!("try_{fn_name}");
                quote!(#fn_name)
            },
        }
    }
}

/// Generic fallible/infallible trait, concrete types provided using type
/// aliases w/ `TransformTrait`
#[derive(Debug, Clone)]
pub(crate) struct TraitFallibility<T: TransformTrait> {
    fallibility: Fallibility,
    transform_trait: PhantomData<T>,
}
#[allow(dead_code)]
impl<T: TransformTrait> TraitFallibility<T> {
    pub fn new_fallible(error_type: Path) -> Self {
        Self {
            fallibility: Fallibility::Fallible { error_type },
            transform_trait: PhantomData,
        }
    }
    pub fn new_infallible() -> Self {
        Self {
            fallibility: Fallibility::Infallible,
            transform_trait: PhantomData,
        }
    }

    pub fn fallibility(&self) -> &Fallibility {
        &self.fallibility
    }
    pub fn is_fallible(&self) -> bool {
        matches!(self.fallibility, Fallibility::Fallible { .. })
    }
    pub fn is_infallible(&self) -> bool {
        matches!(self.fallibility, Fallibility::Infallible)
    }
    pub fn fallibility_associated_types(&self) -> TokenStream {
        match &self.fallibility {
            Fallibility::Infallible => quote! { },
            Fallibility::Fallible { error_type } => quote! {
                type Error = #error_type;
            },
        }
    }

    pub fn error_type(&self) -> Option<Path> {
        if let Fallibility::Fallible { error_type } = &self.fallibility {
            Some(error_type.clone())
        } else {
            None
        }
    }

    // forwarding fn's
    pub fn trait_name(&self) -> TokenStream {
        self.fallibility.trait_name(T::trait_name())
    }
    pub fn trait_fn(&self) -> TokenStream {
        self.fallibility.trait_fn(T::trait_fn())
    }
    pub fn trait_fn_call_suffix(&self) -> TokenStream {
        self.fallibility.fn_call_suffix()
    }
}
impl<T: TransformTrait> From<Fallibility> for TraitFallibility<T> {
    fn from(fallibility: Fallibility) -> Self {
        Self {
            fallibility,
            transform_trait: PhantomData,
        }
    }
}

/// Marker trait for Fallibility impls
pub(crate) trait TransformTrait {
    fn trait_name() -> Ident;
    fn trait_fn() -> Ident;
}
