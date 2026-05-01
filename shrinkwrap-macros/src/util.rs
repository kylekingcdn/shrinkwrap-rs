#![allow(dead_code, unused_imports)]

#[cfg(feature = "expand")]
mod expand;

#[cfg(not(feature = "expand"))]
mod expand_no_op;
#[cfg(not(feature = "expand"))]
use expand_no_op as expand;

pub(crate) use expand::{expand_debug, expand_to_tokens, expand_tokens, expand_tokens_unfmt};
