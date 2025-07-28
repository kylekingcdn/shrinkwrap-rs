mod generate;
mod parse;
mod wrap;

use wrap::derive_wrap_impl;
use darling::{Error, FromMeta};
use darling::ast::NestedMeta;
use syn::ItemFn;
use proc_macro::TokenStream;

#[proc_macro_derive(Wrap, attributes(shrinkwrap, shrinkwrap_attr))]
pub fn derive_wrap(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_wrap_impl(input)
}
