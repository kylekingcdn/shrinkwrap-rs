#![doc = "Intermediate structs used to simplify generation"]

use darling::util::PathList;
use darling::ToTokens;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Path};

pub struct Wrapper {
    pub struct_name: Ident,
    pub struct_docs: String,
    pub derive: PathList,

    pub data_field_name: Ident,
    pub data_struct_name: Ident,
    pub data_field_docs: String,
    pub data_flattened: bool,

    pub extra_field_name: Ident,
    pub extra_struct_name: Ident,
    pub extra_field_docs: String,
}
impl ToTokens for Wrapper {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let derives = build_derives_token(&self.derive);
        let doc = build_docs_token(&self.struct_docs);
        let Self { struct_name, data_field_name, data_struct_name, extra_field_name, extra_struct_name, .. } = &self;

        let flatten_attr = match self.data_flattened {
            true => quote! {
                #[serde(flatten)]
            },
            false => TokenStream::new(),
        };
        let data_doc = build_docs_token(&self.data_field_docs);
        let extra_doc = build_docs_token(&self.extra_field_docs);

        let output = quote! {
            #[automatically_derived]
            #doc
            #derives
            pub struct #struct_name {
                #data_doc
                #flatten_attr
                pub #data_field_name: #data_struct_name,
                #extra_doc
                pub #extra_field_name: #extra_struct_name,
            }
        };

        tokens.extend(output);
    }
}

pub struct Extra {
    pub struct_name: Ident,
    pub struct_docs: String,
    pub derive: PathList,

    pub nests: Vec<(Ident, Ident)>,
}
impl ToTokens for Extra {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let derives = build_derives_token(&self.derive);
        let doc = build_docs_token(&self.struct_docs);
        let Self { struct_name, .. } = &self;

        let mut nest_field_tokens = TokenStream::new();

        for nest in &self.nests {
            let nest_key = &nest.0;
            let nest_struct = &nest.1;

            // TODO: docs

            nest_field_tokens.extend(quote! {
                pub #nest_key: #nest_struct,
            });
        }

        let output = quote! {
            #[automatically_derived]
            #doc
            #derives
            pub struct #struct_name {
                #nest_field_tokens
            }
        };

        tokens.extend(output);
    }
}

pub struct Nest {
    pub struct_name: Ident,
    pub struct_docs: String,
    pub derive: PathList,

    pub key: Ident,

    pub transform: NestTransform,
    pub field_type: Path,
    pub fields: Vec<NestField>,
}
impl ToTokens for Nest {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let derives = build_derives_token(&self.derive);
        let doc = build_docs_token(&self.struct_docs);
        let Self { struct_name, /*key, transform,*/ field_type, .. } = &self;

        let mut field_tokens = TokenStream::new();
        for NestField { name, field_doc: field_doc_str } in &self.fields {
            let field_doc = build_docs_token(&field_doc_str);
            field_tokens.extend(quote! {
                #field_doc
                pub #name: #field_type,
            });
        }

        let output = quote! {
            #[automatically_derived]
            #doc
            #derives
            pub struct #struct_name {
                #field_tokens
            }
        };

        tokens.extend(output);
    }
}

pub enum NestTransform {
    FromImpl { data_ident: syn::Ident },
    Transform { path: syn::Path },
}

#[derive(Debug, Clone)]
pub struct NestField {
    pub name: Ident,
    pub field_doc: String,
}
impl PartialEq for NestField {
    fn eq(&self, other: &Self) -> bool {
        &self.name == &other.name
    }
}
impl Eq for NestField { }

fn build_docs_token(doc: &str) -> TokenStream {
    if doc.is_empty() {
        TokenStream::new()
    } else {
        quote! {
            #[doc=#doc]
        }
    }
}

fn default_derive_names() -> TokenStream {
    quote! {
        Debug,
        Clone,
        serde::Serialize
    }
}
fn build_derives_token(derives: &PathList) -> TokenStream {
    let default_names = default_derive_names();

    let names = if !derives.is_empty() {
        quote! { #default_names, #(#derives),* }
    } else {
        default_names
    };
    quote! { #[derive(#names)] }
}
