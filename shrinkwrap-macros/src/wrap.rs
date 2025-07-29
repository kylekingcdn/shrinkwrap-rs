use darling::FromDeriveInput;
use proc_macro::TokenStream;
use proc_macro_error2::abort;
use syn::{DeriveInput, parse_macro_input};

use crate::generate::{generate_entrypoint, expand_tokens};
use crate::parse::types::{DeriveItemOpts, ValidateScoped};

// -- TODO: use nproc macro error

pub(crate) fn derive_wrap_impl(input: TokenStream) -> TokenStream {
    let origin_struct = parse_macro_input!(input as DeriveInput);

    let args = match DeriveItemOpts::from_derive_input(&origin_struct) {
        Ok(v) => v,
        Err(e) => {
            return TokenStream::from(e.write_errors());
        }
    };

    if let Some(invalidity) = &args.validate_within_scope() {
        let errors = invalidity.join("\n\n");
        if !errors.is_empty() {
            abort!(args.ident.span(), format!("{errors}"));
        }
    }
    let out = generate_entrypoint(args);
    expand_tokens(&out, "Full shrinkwrap derive");

    out.into()
}
