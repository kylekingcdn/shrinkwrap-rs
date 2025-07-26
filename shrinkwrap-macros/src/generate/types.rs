#![doc = "Intermediate structs used to simplify generation"]

use darling::util::PathList;
use darling::ToTokens;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, Path, Type};

use crate::parse::types::{WrapperOpts, NestOpts, ExtraOpts};

// -- TODO, add opt structs as child, only define newly required fields

#[derive(Debug, Clone)]
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
impl Wrapper {
    pub fn new(opts: WrapperOpts, root_ident: &Ident, extra_ident: &Ident) -> Self {
        let struct_name = opts.struct_name(root_ident);
        let data_field_name = opts.data_field_name();
        let extra_field_name = opts.extra_field_name();
        let WrapperOpts { doc, derive, data_field_doc, flatten_data, extra_field_doc, .. } = opts;

        Self {
            struct_name,
            struct_docs: doc,
            derive,
            data_field_name,
            data_struct_name: root_ident.clone(),
            data_field_docs: data_field_doc,
            data_flattened: flatten_data.unwrap_or_default(),
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
            &mut output
        );
        output
    }
    pub fn to_wrapped_impl(&self) -> TokenStream {
        let wrapper_struct_name = &self.struct_name;
        let data_struct_name = &self.data_struct_name;
        eprintln!("{self:#?}");

        quote! {
            impl shrinkwrap::wrap::Wrap for #data_struct_name {
                type Wrapper = #wrapper_struct_name;
                fn to_wrapped(self) -> Self::Wrapper {
                    <Self::Wrapper as From<#data_struct_name>>::from(self)
                }
            }
        }
    }
    pub fn to_wrapped_with_impl(&self, transformer_type: Type, root_nests: &Vec<ExtraNestField>) -> TokenStream {
        let wrapper_struct_name = &self.struct_name;
        let data_struct_name = &self.data_struct_name;
        let data_field_name = &self.data_field_name;
        let extra_field_name = &self.extra_field_name;
        let extra_struct_name = &self.extra_struct_name;

        let mut extra_fields = quote!();
        for nest in root_nests {
            let nest_field_name = &nest.field_name;
            extra_fields.extend(quote!{
                #nest_field_name: transform.transform_to_nest(&self),
            })
        }
        quote! {
            impl shrinkwrap::wrap::WrapWith<#transformer_type> for #data_struct_name {
                type Wrapper = #wrapper_struct_name;
                fn to_wrapped_with(self, transform: &#transformer_type) -> #wrapper_struct_name {
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
#[derive(Debug, Clone)]
pub struct ExtraNestField {
    pub field_name: Ident,
    pub type_ident: Ident,
}
#[derive(Debug, Clone)]
pub struct Extra {
    pub struct_name: Ident,
    pub struct_docs: String,
    pub derive: PathList,

    pub nests: Vec<ExtraNestField>,
}
impl Extra {
    pub fn new(opts: &ExtraOpts, origin_ident: &Ident, nests: Vec<ExtraNestField>) -> Self {
        let struct_name = opts.struct_name(origin_ident);

        Self {
            struct_name,
            struct_docs: opts.doc.clone(),
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
            &mut output
        );
        output
    }
}
impl ToTokens for Extra {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let derives = build_derives_token(&self.derive);
        let doc = build_docs_token(&self.struct_docs);
        let Self { struct_name, .. } = &self;

        let mut nest_field_tokens = TokenStream::new();

        for nest in &self.nests {
            let nest_field_name = &nest.field_name;
            let nest_struct = &nest.type_ident;

            nest_field_tokens.extend(quote! {
                pub #nest_field_name: #nest_struct,
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

#[derive(Debug, Clone)]
pub struct Nest {
    pub struct_name: Ident,
    pub struct_docs: String,
    pub derive: PathList,

    pub origin_ident: Ident,

    pub field_type: Path,
    pub fields: Vec<NestField>,

    /// Some if this nest has additional nests in it's heirachy.
    /// The value is the type ident for the extra struct type
    pub with_extra: Option<ExtraNestField>,
}
impl Nest {
    pub fn new(opts: NestOpts, root_ident: &Ident, fields: Vec<NestField>, with_extra: Option<ExtraNestField>) -> Self {
        let struct_name = opts.struct_name(root_ident);
        let origin_ident = opts.origin(root_ident).clone();
        let NestOpts { derive, doc, field_type, .. } = opts;
        Self {
            struct_name,
            struct_docs: doc,
            derive,
            origin_ident,
            field_type,
            fields,
            with_extra,
        }
    }
}
impl Nest {
    fn to_nest_impl(&self) -> TokenStream {
        let struct_name = &self.struct_name;
        let origin_ident = &self.origin_ident;
        quote! {
          impl shrinkwrap::transform::ToNest<#struct_name> for #origin_ident {
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
        let Self { struct_name, field_type, .. } = &self;

        let mut field_tokens = TokenStream::new();

        for NestField { name, field_doc: field_doc_str } in &self.fields {
            let field_doc = build_docs_token(field_doc_str);
            field_tokens.extend(quote! {
                #field_doc
                pub #name: #field_type,
            });
        }
        if let Some(extra_field) = &self.with_extra {
            let name = &extra_field.field_name;
            let ty = &extra_field.type_ident;
            field_tokens.extend(quote! {
                pub #name: #ty,
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
        tokens.extend(self.to_nest_impl());
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
impl Eq for NestField { }

// -- quote helpers

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

pub(crate) fn build_from_impl(
    from_type: &Ident,
    from_param_name: &Ident,
    to_type: &Ident,
    fields: proc_macro2::TokenStream,
    tokens: &mut proc_macro2::TokenStream,
)  {
   tokens.extend(quote! {
       impl From<#from_type> for #to_type {
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
)  {
   tokens.extend(quote! {
       impl From<&#from_type> for #to_type {
           fn from(#from_param_name: &#from_type) -> Self {
               Self {
                   #fields
               }
           }
       }
   })
}
