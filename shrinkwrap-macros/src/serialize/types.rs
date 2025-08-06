#![doc = "Rudimentary types used for serializing final output. Should contain little to no logic."]

use darling::ToTokens;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Path};

use crate::util::path_parse;

#[allow(dead_code)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ItemVis {
    Public,
    Private,
}
impl ToTokens for ItemVis {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if matches!(self, Self::Public) {
            tokens.extend(quote! {pub});
        }
    }
}

#[derive(Debug, Clone)]
pub struct StructCommon {
    pub vis: ItemVis,
    ty: Path,
    pub derives: Vec<TokenStream>,
    pub attrs: Vec<TokenStream>,
    pub doc: Option<String>,
}
impl StructCommon {
    pub fn new(
        vis: ItemVis,
        ty: Path,
        derives: Vec<TokenStream>,
        attrs: Vec<TokenStream>,
        doc: Option<String>,
    ) -> Self {
        Self {
            vis,
            ty,
            derives,
            attrs,
            doc,
        }
    }
    pub fn ty_full(&self) -> &Path {
        &self.ty
    }
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub vis: ItemVis,
    pub name: Ident,
    ty: Path,
    pub optional: bool,
    pub attrs: Vec<TokenStream>,
    pub doc: Option<String>,
}
impl StructField {
    pub fn new(
        vis: ItemVis,
        name: Ident,
        ty: Path,
        optional: bool,
        attrs: Vec<TokenStream>,
        doc: Option<String>,
    ) -> Self {
        Self {
            vis,
            name,
            ty,
            optional,
            attrs,
            doc,
        }
    }
    #[allow(dead_code)]
    pub fn ty_base(&self) -> &Path {
        &self.ty
    }
    pub fn ty_full(&self) -> Path {
        let ty = &self.ty;
        let tokens = match self.optional {
            true => quote!(Option<#ty>),
            false => quote!(#ty),
        };
        path_parse(tokens)
    }
}
impl ToTokens for StructField {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self {
            vis, name, attrs, ..
        } = &self;
        let ty = self.ty_full();
        let mut attr_tokens = quote! {#( #[#attrs] )*};
        if let Some(doc) = &self.doc {
            attr_tokens.extend(quote! {#[doc = #doc]});
        }
        tokens.extend(quote! {
            #attr_tokens
            #vis #name: #ty,
        });
    }
}

#[derive(Debug, Clone)]
pub struct Wrapper {
    pub common: StructCommon,
    pub wrapper_type: WrapperType,
    pub data_field: StructField,
    pub extra_field: StructField,
}

#[derive(Debug, Clone)]
pub struct NestedWrapper {
    pub data_source_ident: Ident,
    pub optional: bool,
}

#[derive(Debug, Clone)]
pub struct RootWrapper {}

#[derive(Debug, Clone)]
pub enum WrapperType {
    Root(RootWrapper),
    Nested(NestedWrapper),
}

#[derive(Debug, Clone)]
pub struct Extra {
    pub common: StructCommon,
    pub nest_fields: Vec<StructField>,
}

#[derive(Debug, Clone)]
pub struct Nest {
    pub common: StructCommon,
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone)]
pub struct UniversalStruct {
    pub common: StructCommon,
    pub fields: Vec<StructField>,
}
impl From<Wrapper> for UniversalStruct {
    fn from(input: Wrapper) -> Self {
        Self {
            common: input.common,
            fields: vec![input.data_field, input.extra_field],
        }
    }
}
impl From<Extra> for UniversalStruct {
    fn from(input: Extra) -> Self {
        Self {
            common: input.common,
            fields: input.nest_fields,
        }
    }
}
impl From<Nest> for UniversalStruct {
    fn from(input: Nest) -> Self {
        Self {
            common: input.common,
            fields: input.fields,
        }
    }
}
impl ToTokens for UniversalStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let StructCommon {
            vis,
            derives,
            attrs,
            ..
        } = &self.common;
        let ty = self.common.ty_full();
        let fields = &self.fields;

        let mut attr_tokens = build_derives(derives);
        attr_tokens.extend(quote! {#( #[#attrs] )*});
        if let Some(doc) = &self.common.doc {
            attr_tokens.extend(quote! {#[doc = #doc]});
        }
        let out = quote! {
            #[automatically_derived]
            #attr_tokens
            #vis struct #ty {
                #( #fields )*
            }
        };

        tokens.extend(out);
    }
}

fn build_derives(derives: &Vec<TokenStream>) -> TokenStream {
    if derives.is_empty() {
        TokenStream::new()
    } else {
        quote! { #[derive(#( #derives ),*)]}
    }
}
