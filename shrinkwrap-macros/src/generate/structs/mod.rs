#![allow(dead_code)]
use super::*;

mod derives;
pub(crate) use derives::Derives;
mod doc;
pub(crate) use doc::Doc;

// !- Item visibility

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum GenVisibility {
    Public,
    PublicCrate,
    Private,
}
impl ToTokens for GenVisibility {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Public => { tokens.extend(quote!(pub)); },
            Self::PublicCrate => { tokens.extend(quote!(pub(crate))); },
            Self::Private => {},
        }
    }
}

// !- Named struct generator

/// Generator for a named struct
#[derive(Debug, Clone)]
pub struct GenStruct {
    pub vis: GenVisibility,
    pub ty: Rc<Path>,
    pub derives: Rc<Derives>,
    pub attrs: Rc<Vec<TokenStream>>,
    pub doc: Doc,
    pub fields: Rc<Vec<GenStructField>>,
}
impl GenStruct {
    pub(crate) fn generate(self) -> GenStructOutput {
        GenStructOutput {
            out: Rc::new(self.to_token_stream()),
            source: Rc::new(self),
        }
    }
}
impl ToTokens for GenStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        // destructure self
        let Self { vis, ty, attrs, derives, doc, fields, .. } = &self;

        // build attribute list
        let attrs = quote! { #( #[#attrs] )* };

        tokens.extend(quote! {
            #[automatically_derived]
            #doc
            #derives
            #attrs
            #vis struct #ty {
                #( #fields )*
            }
        });
    }
}
/// Return type of struct `generate` calls
pub(crate) struct GenStructOutput {
    /// Generation source type
    pub(crate) source: Rc<GenStruct>,

    /// Generated tokens
    pub(crate) out: Rc<TokenStream>,
}

// !- Named struct field generator

/// Generator for a single field within a named struct
#[derive(Debug, Clone)]
pub struct GenStructField {
    pub vis: GenVisibility,
    pub name: Rc<Ident>,
    pub ty: Rc<Path>,
    pub attrs: Rc<Vec<TokenStream>>,
    pub doc: Doc,
}
impl ToTokens for GenStructField {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        // destructure self
        let Self { vis, name, ty, attrs, doc, .. } = &self;

        // build attribute list
        let attrs = quote! { #( #[#attrs] )* };

        tokens.extend(quote! {
            #doc
            #attrs
            #vis #name: #ty,
        });
    }
}
