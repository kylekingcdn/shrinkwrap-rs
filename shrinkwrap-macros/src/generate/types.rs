#![doc = "Intermediate structs used to simplify generation"]

use darling::ToTokens;
use darling::util::PathList;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{Ident, Path, Type};

use crate::parse::types::{ExtraOpts, NestMapStrategy, NestOpts, PassthroughAttributeContext, WrapperOpts};

// -- TODO, add opt structs as child, only define newly required fields

#[derive(Debug, Clone)]
pub struct Wrapper {
    pub struct_name: Ident,
    pub struct_docs: String,
    pub struct_attrs: Vec<TokenStream>,

    pub derive: PathList,
    pub data_field_name: Ident,
    pub data_struct_name: Ident,
    pub data_field_docs: String,
    pub data_flattened: bool,

    pub extra_field_name: Ident,
    pub extra_struct_name: Ident,
    pub extra_field_docs: String,
}
impl Wrapper {
    pub fn new(opts: WrapperOpts, root_ident: &Ident, extra_ident: &Ident, struct_attrs: Vec<TokenStream>) -> Self {
        let struct_name = opts.struct_name(root_ident);
        let data_field_name = opts.data_field_name();
        let extra_field_name = opts.extra_field_name();
        let data_flattened = opts.flatten();
        let WrapperOpts {
            doc,
            derive,
            data_field_doc,
            extra_field_doc,
            ..
        } = opts;

        Self {
            struct_name,
            struct_docs: doc,
            struct_attrs,
            derive,
            data_field_name,
            data_struct_name: root_ident.clone(),
            data_field_docs: data_field_doc,
            data_flattened,
            extra_field_name,
            extra_struct_name: extra_ident.clone(),
            extra_field_docs: extra_field_doc,
        }
    }
    pub fn build_from_data_impl(&self) -> TokenStream {
        let data_struct_name = &self.data_struct_name;
        let extra_struct_name = &self.extra_struct_name;
        let field_tokens = quote! {
            extra: <#extra_struct_name as From<&#data_struct_name>>::from(&data),
            data,
        };

        let from_param_name = format_ident!("data");
        let mut output = quote!();
        build_from_impl(
            &self.data_struct_name,
            &from_param_name,
            &self.struct_name,
            field_tokens,
            &mut output,
        );
        output
    }
    pub fn to_wrapped_impl(&self) -> TokenStream {
        let wrapper_struct_name = &self.struct_name;
        let data_struct_name = &self.data_struct_name;

        quote! {
            #[automatically_derived]
            impl ::shrinkwrap::wrap::Wrap for #data_struct_name {
                type Wrapper = #wrapper_struct_name;

                fn to_wrapped(self) -> Self::Wrapper {
                    <Self::Wrapper as From<#data_struct_name>>::from(self)
                }
            }
        }
    }
    pub fn to_wrapped_with_impl(
        &self,
        transformer_type: Type,
        root_nests: &Vec<ExtraNestField>,
    ) -> TokenStream {
        let wrapper_struct_name = &self.struct_name;
        let data_struct_name = &self.data_struct_name;
        let data_field_name = &self.data_field_name;
        let extra_field_name = &self.extra_field_name;
        let extra_struct_name = &self.extra_struct_name;

        let mut extra_fields = quote!();
        for nest in root_nests {
            let nest_field_name = &nest.field_name;
            extra_fields.extend(quote! {
                #nest_field_name: transform.transform_to_nest(&self, options),
            })
        }
        quote! {
            #[automatically_derived]
            impl ::shrinkwrap::wrap::WrapWith<#transformer_type> for #data_struct_name {
                type Wrapper = #wrapper_struct_name;

                fn to_wrapped_with(self, transform: &#transformer_type, options: &<#transformer_type as ::shrinkwrap::Transform>::Options) -> #wrapper_struct_name {
                    #wrapper_struct_name {
                        #extra_field_name: #extra_struct_name {
                            #extra_fields
                        },
                        #data_field_name: self,
                    }
                }
            }
        }
    }
}
impl ToTokens for Wrapper {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let derives = build_derives_token(&self.derive);
        let doc = build_docs_token(&self.struct_docs);
        let struct_attrs = build_attributes_token(&self.struct_attrs);
        // expand_debug(&struct_attrs, "Wrapper", "to_tokens");
        let Self {
            struct_name,
            data_field_name,
            data_struct_name,
            extra_field_name,
            extra_struct_name,
            ..
        } = &self;

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
            #struct_attrs
            pub struct #struct_name {
                #data_doc
                #flatten_attr
                pub #data_field_name: #data_struct_name,
                #extra_doc
                pub #extra_field_name: #extra_struct_name,
            }
        };
        // expand_tokens(&output, "Wrapper::ToTokens");
        tokens.extend(output);
    }
}
#[derive(Debug, Clone)]
pub struct ExtraNestField {
    pub field_name: Ident,
    pub type_ident: Ident,
    pub optional: bool,
}
#[derive(Debug, Clone)]
pub struct Extra {
    pub struct_name: Ident,
    pub struct_docs: String,
    pub struct_attrs: Vec<TokenStream>,
    pub derive: PathList,

    pub nests: Vec<ExtraNestField>,
}
impl Extra {
    pub fn new(opts: &ExtraOpts, origin_ident: &Ident, struct_attrs: Vec<TokenStream>, nests: Vec<ExtraNestField>) -> Self {
        let struct_name = opts.struct_name(origin_ident);

        Self {
            struct_name,
            struct_docs: opts.doc.clone(),
            struct_attrs,
            derive: opts.derive.clone(),
            nests,
        }
    }
    pub fn build_from_data_impl(&self, origin_ident: &Ident) -> TokenStream {
        let mut nest_field_tokens = quote!();

        for nest in &self.nests {
            let nest_field_name = &nest.field_name;
            let nest_struct = &nest.type_ident;

            nest_field_tokens.extend(quote! {
                #nest_field_name: <#nest_struct as From<&#origin_ident>>::from(data),
            });
        }

        let from_param_name = format_ident!("data");
        let mut output = quote!();
        build_from_impl_with_ref(
            origin_ident,
            &from_param_name,
            &self.struct_name,
            nest_field_tokens,
            &mut output,
        );
        output
    }
}
impl ToTokens for Extra {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let derives = build_derives_token(&self.derive);
        let doc = build_docs_token(&self.struct_docs);
        let struct_attrs = build_attributes_token(&self.struct_attrs);
        let Self { struct_name, .. } = &self;

        let mut nest_field_tokens = TokenStream::new();

        for nest in &self.nests {
            let nest_field_name = &nest.field_name;
            let nest_struct = &nest.type_ident;

            if nest.optional {
                nest_field_tokens.extend(quote! {
                    pub #nest_field_name: Option<#nest_struct>,
                });
            } else {
                nest_field_tokens.extend(quote! {
                    pub #nest_field_name: #nest_struct,
                });
            }
        }

        let output = quote! {
            #[automatically_derived]
            #doc
            #derives
            #struct_attrs
            pub struct #struct_name {
                #nest_field_tokens
            }
        };
        // expand_tokens(&output, "Extra::ToTokens");
        tokens.extend(output);
    }
}

#[derive(Debug, Clone)]
pub struct Nest {
    pub struct_name: Ident,
    pub struct_docs: String,
    pub struct_attrs: Vec<TokenStream>,
    pub derive: PathList,

    pub origin_ident: Ident,

    pub field_type: Path,
    pub fields: Vec<NestField>,
    pub field_attrs: Vec<NestFieldAttrs>,

    /// false if under root extra
    pub is_nested: bool,
    // /// Some if this nest has additional nests in it's heirachy.
    // /// The value is the type ident for the extra struct type
    // pub with_extra: Option<ExtraNestField>,
}
impl Nest {
    pub fn new(
        opts: NestOpts,
        root_ident: &Ident,
        struct_attrs: Vec<TokenStream>,
        fields: Vec<NestField>,
        field_attrs: Vec<NestFieldAttrs>,
        // with_extra: Option<ExtraNestField>,
    ) -> Self {
        let struct_name = opts.struct_name(root_ident);
        let origin_ident = opts.origin(root_ident).clone();
        let is_nested = matches!(opts.map_strategy, NestMapStrategy::Nested { .. });
        let NestOpts {
            derive,
            doc,
            field_type,
            ..
        } = opts;
        Self {
            struct_name,
            struct_docs: doc,
            struct_attrs,
            derive,
            origin_ident,
            field_type,
            field_attrs,
            fields,
            is_nested,
            // with_extra,
        }
    }
}
impl Nest {
    fn to_nest_impl(&self) -> TokenStream {
        let struct_name = &self.struct_name;
        let origin_ident = &self.origin_ident;
        quote! {
            #[automatically_derived]
            impl ::shrinkwrap::transform::ToNest<#struct_name> for #origin_ident {
                fn to_nest(&self) -> #struct_name {
                    <#struct_name as From<&#origin_ident>>::from(self)
                }
            }
        }
    }
}
impl ToTokens for Nest {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let derives = build_derives_token(&self.derive);
        let doc = build_docs_token(&self.struct_docs);
        let struct_attrs = build_attributes_token(&self.struct_attrs);
        let Self {
            struct_name,
            field_type,
            ..
        } = &self;

        let mut field_tokens = TokenStream::new();


        for NestField {
            name,
            field_doc: field_doc_str,
        } in &self.fields
        {
            let field_attrs = self
                .field_attrs
                .iter()
                .find(|attr| &attr.field_name == name);
            let nested_attr_tokens = match field_attrs {
                Some(attrs) => {
                    let attributes_out = attrs.attributes_token.clone();
                    quote::quote! { #[#attributes_out] }
                }
                None => {
                    quote::quote! {}
                }
            };
            let field_doc = build_docs_token(field_doc_str);
            field_tokens.extend(quote! {
                #field_doc
                #nested_attr_tokens
                pub #name: #field_type,
            });
        }

        let output = quote! {
            #[automatically_derived]
            #doc
            #derives
            #struct_attrs
            pub struct #struct_name {
                #field_tokens
            }
        };
        // expand_tokens(&output, "Nest::ToTokens");
        tokens.extend(output);
        if !self.is_nested {
            tokens.extend(self.to_nest_impl());
        }
    }
}

#[derive(Debug, Clone)]
pub struct NestField {
    pub name: Ident,
    pub field_doc: String,
}
impl PartialEq for NestField {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Eq for NestField {}

#[derive(Debug, Clone)]
pub struct NestFieldAttrs {
    pub field_name: Ident,
    pub attributes_token: TokenStream,
}

#[derive(Debug, Clone, Default)]
pub enum NestSelection {
    #[default]
    Unrestricted,
    Restricted(Vec<String>),
}

#[derive(Debug, Clone, Default)]
pub struct NestScopedAttrs {
    pub nests: NestSelection,
    pub attributes_token: TokenStream,
    pub nests_span: Option<Span>,
    pub context: PassthroughAttributeContext,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum StructGenScope {
    Wrapper,
    Extra,
    Nest
}
impl NestScopedAttrs {
    pub fn has_struct_scope(&self, scope: StructGenScope) -> bool {
        match self.context {
            PassthroughAttributeContext::All => true,
            PassthroughAttributeContext::Wrapper => scope == StructGenScope::Wrapper,
            PassthroughAttributeContext::Extra => scope == StructGenScope::Extra,
            PassthroughAttributeContext::Nest => scope == StructGenScope::Nest,
        }
    }
    pub fn has_struct_scope_for_nest(&self, scope: StructGenScope, nest_id: &str) -> bool {
        if !self.has_struct_scope(scope) {
            return false;
        }
        match &self.nests {
            NestSelection::Unrestricted => true,
            NestSelection::Restricted(ids) => ids.iter().any(|id| id == nest_id)
        }
    }
    pub fn is_permitted_by_filter(&self, scope: StructGenScope, assoc_nest_ids: &Vec<&String>) -> bool {
        if !self.has_struct_scope(scope) {
            return false;
        }
        match &self.nests {
            NestSelection::Unrestricted => true,
            NestSelection::Restricted(permitted_nest_ids) => {
                for permitted_id in permitted_nest_ids {
                    if assoc_nest_ids.contains(&permitted_id) {
                        return true;
                    }
                }
                false
            }
        }
    }
}

/// Provides a mapping of a nest's defined origin (or root) to nest opts
pub(crate) type NestOriginMap<'a> = HashMap<Ident, Vec<NestOpts>>;
/// Provides a mapping of a nest ID to a list of fields that should be mapped to the assoc nest.
pub(crate) type NestFieldMap<'a> = HashMap<String, Vec<NestField>>;
/// Provides a mapping of a nest ID to a list of field x attribute pairs
pub(crate) type NestFieldAttrMap<'a> = HashMap<String, Vec<NestFieldAttrs>>;

// -- quote helpers

fn build_docs_token(doc: &str) -> TokenStream {
    if doc.is_empty() {
        TokenStream::new()
    } else {
        quote! { #[doc=#doc] }
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
fn build_attribute_token(attribute: &TokenStream, tokens: &mut TokenStream) {
    if !attribute.is_empty() {
        tokens.extend(quote! { #[#attribute] })
    }
}
fn build_attributes_token(attributes: &Vec<TokenStream>) -> TokenStream {
    let mut out = quote!();
    for attr in attributes {
        build_attribute_token(attr, &mut out);
    }
    out
}

pub(crate) fn build_from_impl(
    from_type: &Ident,
    from_param_name: &Ident,
    to_type: &Ident,
    fields: proc_macro2::TokenStream,
    tokens: &mut proc_macro2::TokenStream,
) {
    tokens.extend(quote! {
        #[automatically_derived]
        impl ::core::convert::From<#from_type> for #to_type {
            fn from(#from_param_name: #from_type) -> Self {
                Self {
                    #fields
                }
            }
        }
    })
}

pub(crate) fn build_from_impl_with_ref(
    from_type: &Ident,
    from_param_name: &Ident,
    to_type: &Ident,
    fields: proc_macro2::TokenStream,
    tokens: &mut proc_macro2::TokenStream,
) {
    tokens.extend(quote! {
        #[automatically_derived]
        impl ::core::convert::From<&#from_type> for #to_type {
            fn from(#from_param_name: &#from_type) -> Self {
                Self {
                    #fields
                }
            }
        }
    })
}
