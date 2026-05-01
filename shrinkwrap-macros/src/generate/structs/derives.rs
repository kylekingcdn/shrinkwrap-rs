use super::*;

#[derive(Debug, Clone)]
pub(crate) struct Derives(Vec<Path>);

impl ToTokens for Derives {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if !self.0.is_empty() {
            let derives = &self.0;
            tokens.extend(quote! { #[derive(#( #derives ),*)] })
        }
    }
}
impl From<Vec<TokenStream>> for Derives {
    fn from(tokens: Vec<TokenStream>) -> Self {
        Self(tokens.into_iter().map(|t| parse_quote!(#t)).collect())
    }
}
impl From<Vec<Path>> for Derives {
    fn from(paths: Vec<Path>) -> Self {
        Self(paths)
    }
}
