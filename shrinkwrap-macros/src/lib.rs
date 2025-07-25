mod generate;
mod parse;
mod wrap;

use wrap::derive_wrap_impl;

#[proc_macro_derive(Wrap, attributes(shrinkwrap))]
pub fn derive_wrap(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_wrap_impl(input)
}
