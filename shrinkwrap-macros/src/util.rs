use darling::util::PathList;
use proc_macro2::TokenStream;
use syn::{Path, parse2};

pub fn path_parse(tokens: TokenStream) -> Path {
    let error_message = format!("Invalid path: {:#?}", tokens.to_string());
    parse2(tokens).expect(&error_message)
}

#[allow(dead_code)]
pub fn pathlist_parse(tokens: Vec<TokenStream>) -> PathList {
    let mut paths = Vec::with_capacity(tokens.len());
    for token in tokens {
        paths.push(path_parse(token));
    }
    PathList::new(paths)
}

#[allow(unused_imports)]
pub(crate) use expand::{expand_debug, expand_to_tokens, expand_tokens, expand_tokens_unfmt};

/// no-op function signatures for feature toggle
#[cfg(not(feature = "expand"))]
#[allow(dead_code)]
mod expand {
    pub(crate) fn expand_debug<T: std::fmt::Debug>(
        _t: &T,
        _type_name: &'static str,
        _fn_name: &'static str,
    ) {
    }
    pub(crate) fn expand_tokens(_tokens: &proc_macro2::TokenStream, _fn_name: &'static str) {}
    pub(crate) fn expand_to_tokens<T: quote::ToTokens>(
        _t: &T,
        _type_name: &'static str,
        _fn_name: &'static str,
    ) {
    }
    pub(crate) fn expand_tokens_unfmt(_tokens: &proc_macro2::TokenStream, _fn_name: &'static str) {}
}

#[cfg(feature = "expand")]
#[allow(dead_code)]
mod expand {
    // all
    const T_RESET: &str = "\x1b[0m";
    // style
    const T_BOLD: &str = "\x1b[1m";
    const T_UNDERLINE: &str = "\x1b[4m";
    // text color
    const T_C_RESET: &str = "\x1b[39m";
    const T_C_WHITE: &str = "\x1b[97m";
    const T_C_BLACK: &str = "\x1b[30m";
    const T_C_BLUE: &str = "\x1b[34m";
    const T_C_RED: &str = "\x1b[31m";
    // text background color
    const T_B_RESET: &str = "\x1b[49m";
    const T_B_BLUE: &str = "\x1b[44m";
    const T_B_RED: &str = "\x1b[41m";

    /// Dumps the type to stderr using it's Debug impl, but only if the `expand` feature is enabled. Otherwise this is a no-op
    pub(crate) fn expand_debug<T: std::fmt::Debug>(
        t: &T,
        type_name: &'static str,
        fn_name: &'static str,
    ) {
        eprintln!("\n{T_BOLD}{T_C_BLUE}------------------------------------------------{T_RESET}");
        eprintln!(
            "{T_BOLD}{T_B_BLUE}{T_C_BLACK}[{type_name}]{T_B_RESET} {T_C_BLUE}{fn_name}:{T_RESET} \n{t:#?}\n"
        );
        eprintln!("{T_BOLD}{T_C_BLUE}------------------------------------------------{T_RESET}");
    }

    /// Dumps token stream to stderr if the `expand` feature is enabled. Otherwise this is a no-op
    ///
    /// Attempts to format generated rust code, if valid. Otherwise the output is provided unformatted.
    pub(crate) fn expand_tokens(tokens: &proc_macro2::TokenStream, fn_name: &'static str) {
        eprintln!("\n{T_BOLD}{T_C_BLUE}------------------------------------------------{T_RESET}");
        match syn::parse_file(tokens.to_string().as_str()) {
            Ok(tokens_file) => {
                let tokens_fmt = prettyplease::unparse(&tokens_file);
                eprintln!("{T_BOLD}{T_C_BLUE}{fn_name}:{T_RESET} \n{}", &tokens_fmt);
                eprintln!(
                    "{T_BOLD}{T_C_BLUE}------------------------------------------------{T_RESET}"
                );
            }
            Err(err) => {
                eprintln!(
                    "{T_BOLD}{T_B_RED}{T_C_BLACK}{fn_name}:{T_RESET} Failed to render formatted output - err: {err}."
                );
                eprintln!("Output will be unformatted.\n");
                expand_tokens_unfmt(tokens, fn_name)
            }
        }
    }

    /// Helper fn for expand_tokens, where the type's `ToTokens` is automatically called
    pub(crate) fn expand_to_tokens<T: quote::ToTokens>(
        t: &T,
        type_name: &'static str,
        fn_name: &'static str,
    ) {
        eprintln!("\n{T_BOLD}{T_C_BLUE}------------------------------------------------{T_RESET}");
        let token_stream = t.to_token_stream();
        match syn::parse_file(token_stream.to_string().as_str()) {
            Ok(tokens_file) => {
                let tokens_fmt = prettyplease::unparse(&tokens_file);
                eprintln!(
                    "{T_BOLD}{T_B_BLUE}{T_C_BLACK}[{type_name}]{T_RESET} {T_BOLD}{T_C_BLUE}{fn_name}:{T_RESET} \n{}",
                    &tokens_fmt
                );
                eprintln!(
                    "{T_BOLD}{T_C_BLUE}------------------------------------------------{T_RESET}"
                );
            }
            Err(err) => {
                eprintln!(
                    "{T_B_RED}[{type_name}]{T_RESET} {T_BOLD}{T_C_RED}{fn_name}:{T_RESET} Failed to render formatted output - err: {err}."
                );
                eprintln!("Output will be unformatted.\n");
                expand_tokens_unfmt(&token_stream, fn_name)
            }
        }
    }

    /// Dumps token stream to stderr if the `expand` feature is enabled. Otherwise this is a no-op
    ///
    /// Attempts to format generated rust code, if valid. Otherwise the output is provided unformatted.
    pub(crate) fn expand_tokens_unfmt(tokens: &proc_macro2::TokenStream, fn_name: &'static str) {
        eprintln!("\n{T_BOLD}{T_C_BLUE}------------------------------------------------{T_RESET}");
        eprintln!(
            "{T_BOLD}{T_C_BLUE}{fn_name}{T_C_RESET} unformatted: \n{}",
            &tokens
        );
        eprintln!("{T_BOLD}{T_C_BLUE}------------------------------------------------{T_RESET}");
    }
}
