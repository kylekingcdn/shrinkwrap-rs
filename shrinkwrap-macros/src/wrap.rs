use darling::FromDeriveInput;
use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

use crate::generate::generate;
use crate::parse::types::DeriveItemOpts;
use crate::util::expand_tokens;

pub(crate) fn derive_wrap_impl(input: TokenStream) -> TokenStream {
    let origin_struct = parse_macro_input!(input as DeriveInput);

    let args = match DeriveItemOpts::from_derive_input(&origin_struct) {
        Ok(v) => v,
        Err(e) => {
            return TokenStream::from(e.write_errors());
        }
    };
    args.validate();

    let mut out = proc_macro2::TokenStream::default();
    generate(args, &mut out);
    expand_tokens(&out, "Full shrinkwrap derive");

    out.into()
}
