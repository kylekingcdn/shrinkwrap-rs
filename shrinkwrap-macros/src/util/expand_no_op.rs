#![doc = "no-op function signatures for feature toggle"]

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