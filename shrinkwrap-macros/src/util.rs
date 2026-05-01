#![allow(dead_code)]

#[cfg(feature = "expand")]
#[allow(unused_imports)]
mod expand;

#[cfg(not(feature = "expand"))]
#[allow(unused_imports)]
mod expand_no_op;
#[cfg(not(feature = "expand"))]
use expand_no_op as expand;

#[allow(unused_imports)]
pub(crate) use expand::{expand_debug, expand_to_tokens, expand_tokens, expand_tokens_unfmt};

use syn::{Path, PathArguments, GenericArgument};

pub(crate) fn extract_path_generics(path: &Path) -> Vec<&GenericArgument> {
    if let Some(path_base) = path.segments.last() &&
    let PathArguments::AngleBracketed(args) = &path_base.arguments {
        return args.args.iter().collect();
    }
    vec![]
}
