use super::*;

#[derive(Debug, Clone)]
pub(crate) struct Derives(Vec<TokenStream>);

impl ToTokens for Derives {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if !self.0.is_empty() {
            let derives = &self.0;
            tokens.extend(quote! { #[derive(#( #derives ),*)] })
        }
    }
}
impl From<Vec<TokenStream>> for Derives {
    fn from(value: Vec<TokenStream>) -> Self {
        Self(value)
    }
}