use super::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct Doc(Option<Rc<str>>);

impl ToTokens for Doc {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if let Some(doc) = &self.0 && !doc.is_empty() {
            tokens.extend(quote! { #[doc = #doc] });
        }
    }
}
impl<T: Into<Rc<str>>> From<Option<T>> for Doc {
    fn from(value: Option<T>) -> Self {
        Self(value.map(|doc| doc.into()))
    }
}
