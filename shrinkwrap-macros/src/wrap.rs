use darling::FromDeriveInput;
use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

use crate::generate::generate_entrypoint;
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

    let fields = &args.data.clone().take_struct().unwrap().fields;
    for field in fields {
        eprintln!("\ndumping field:\n{field:#?}");
        let field_attrs = &field.attrs;
        // let attrs_tokenized = quote::quote! { #field_attrs };
        for attr in field_attrs {
            let path = attr.path();
            match darling::util::parse_attribute_to_meta_list(&attr) {
                Ok(parsed) => {
                    eprintln!("successfully parsed attribute {path:#?}");
                    let parsed_fmt = quote::quote! { #parsed};
                    eprintln!("generated output for attrs: {parsed_fmt}");
                },
                Err(err) => {
                    eprintln!("failed to parse attribute {path:#?} - {err}");
                },
            }
            // let attr_val =    .unwrap();
            // eprintln!("parsed attr_val: {attr_val:#?}");
        }
    }
    if let Some(invalidity) = &args.validate_within_scope() {
        let errors = invalidity.join("\n\n");
        if !errors.is_empty() {
            panic!("{errors}");
        }
    }

    #[cfg(feature = "expand")]
    {
        let out = generate_entrypoint(args);
        let out_file = syn::parse_file(out.to_string().as_str());
        match out_file {
            Ok(out_file) => {
                let out_fmt = prettyplease::unparse(&out_file);
                eprintln!("{}", &out_fmt);
            }
            Err(err) => {
                eprintln!("failed to render formatted output - err: {err}\n\nunformatted: {out}");
            }
        }

        out.into()
    }
    #[cfg(not(feature = "expand"))]
    {
        generate_entrypoint(args).into()
    }
}
