use proc_macro_error2::proc_macro_error;

mod generate;
mod mapping;
mod parse;
mod serialize;
mod util;
mod wrap;

use wrap::derive_wrap_impl;

#[proc_macro_derive(Wrap, attributes(shrinkwrap, shrinkwrap_attr))]
#[proc_macro_error]
pub fn derive_wrap(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_wrap_impl(input)
}
