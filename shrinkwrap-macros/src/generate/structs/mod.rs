use super::*;

mod derives;
pub(crate) use derives::Derives;
mod doc;
pub(crate) use doc::Doc;


// !- Item visibility

#[allow(dead_code)]
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
    pub ty: Path,
    pub derives: Derives,
    pub attrs: Vec<Attribute>,
    pub doc: Doc,
    pub fields: Vec<GenStructField>,
}
impl ToTokens for GenStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        // destructure self
        let Self { vis, ty, attrs, derives, doc, fields, .. } = &self;

        // build attribute list
        let attrs = quote! { #( #attrs )* };

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

// !- Named struct field generator

/// Generator for a single field within a named struct
#[derive(Debug, Clone)]
pub struct GenStructField {
    pub vis: GenVisibility,
    pub name: Ident,
    pub ty: Path,
    pub attrs: Vec<Attribute>,
    pub doc: Doc,
}
impl ToTokens for GenStructField {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        // destructure self
        let Self { vis, name, ty, attrs, doc, .. } = &self;

        // build attribute list
        let attrs = quote! { #( #attrs )* };

        tokens.extend(quote! {
            #doc
            #attrs
            #vis #name: #ty,
        });
    }
}
