#![allow(dead_code)] // temporary

use darling::{FromDeriveInput, FromField, FromMeta};
use darling::ast::Data;
use darling::util::{Override, PathList, WithOriginal};
use heck::AsTitleCase;
use syn::Path;

/// Root derive options
#[derive(Debug, Clone, FromDeriveInput)]
#[darling(attributes(shrinkwrap), forward_attrs(allow, doc, cfg), supports(struct_named))]
pub(crate) struct DeriveItemOpts {
    ident: syn::Ident,
    attrs: Vec<syn::Attribute>,
    data: Data<(), WithOriginal<DataFieldOpts, syn::Field>>,

    #[darling(default, rename = "wrapper")]
    wrapper_opts: Option<WrapperOpts>,

    #[darling(default, rename = "data")]
    data_opts: Option<DataOpts>,

    #[darling(default, rename = "extra")]
    extra_opts: Option<ExtraOpts>,

    #[darling(default, rename = "nest", multiple)]
    nest_opts: Vec<NestOpts>,
}

/// Options for struct wrapper attribute
///
/// e.g.
/// ```ignore
/// #[shrinkwrap(wrapper(rename = "UserDataWrapper", derive(Debug)))]
/// pub struct User { /* */ }
/// ```
#[derive(Debug, Clone, FromMeta)]
pub struct WrapperOpts {
    /// set the parent wrapper struct name - defaults to `{DataStructName}Wrapper`
    rename: Option<String>,

    /// Derives to apply to the wrapper struct
    #[darling(default)]
    derive: PathList,

    /// override the documentation for the generated Wrapper struct
    doc: Option<String>,

    // TODO: implement Wrapper::from(Data) if Extra::from(Data) is implemented
    //       (aka all nests implement From<Data>)
}
impl WrapperOpts {
    pub fn resolve_name(&self, data_ident: &syn::Ident) -> syn::Ident {
        match &self.rename {
            Some(name) => {
                if name.is_empty() {
                    panic!("Wrapper name cannot be empty")
                } else {
                    syn::Ident::new(name, data_ident.span())
                }
            }
            None => {
                syn::Ident::new(format!("{data_ident}Wrapper").as_str(), data_ident.span())
            }
        }
    }
}

/// Options for struct data attributes
///
/// e.g.
/// ```ignore
/// #[shrinkwrap(data(flatten))]
/// pub struct User { /* */ }
/// ```
#[derive(Debug, Clone, FromMeta)]
pub struct DataOpts {
    #[darling(default = DataOpts::flatten_override_default)]
    flatten: Override<bool>,
}
impl DataOpts {
    fn flatten_default() -> bool {
        true
    }
    fn flatten_override_default() -> Override<bool> {
        Some(Self::flatten_default()).into()
    }
}

/// Options for struct extra attribute
///
/// e.g.
/// ```ignore
/// #[shrinkwrap(extra(rename = "UserTextExtra", field = "extra", derive(Debug)))]
/// pub struct User { /* */ }
/// ```
#[derive(Debug, Clone, FromMeta)]
pub struct ExtraOpts {
    /// set the `extra` struct name - defaults to `{DataStructName}Extra`
    rename: Option<String>,

    /// Derives to apply to the extra struct
    #[darling(default)]
    derive: PathList,

    /// override the documentation for the generated Extra struct
    doc: Option<String>,
}
impl ExtraOpts {
    pub fn resolve_name(&self, data_ident: &syn::Ident) -> syn::Ident {
        match &self.rename {
            Some(name) => {
                if name.is_empty() {
                    panic!("Extra struct name cannot be empty")
                } else {
                    syn::Ident::new(name, data_ident.span())
                }
            }
            None => {
                syn::Ident::new(format!("{data_ident}Extra").as_str(), data_ident.span())
            }
        }
    }
}

/// Options for struct nest attribute
///
/// e.g.
/// ```ignore
/// #[shrinkwrap(nest(name = "text", return = "", transform(from | "some::fn")))]`
/// pub struct User { /* */ }
/// ```
#[derive(Debug, Clone, FromMeta)]
pub struct NestOpts {
    /// used for the nest field key under `data.extra` as well as an identifier for other attributes
    key: String,

    /// sets the name of the nests' generated struct - defaults to `{DataStructName}{titlecased_key}`
    rename: Option<String>,

    /// sets the type for the fields in the nested struct
    field_type: syn::Path,

    /// Path to transform function used to convert data struct into nest struct. from can be used to automatically use a `From<&Data>` impl
    transform: Option<Path>,

    /// Derives the transform using an existing `impl From<&Data> for DataNest`
    #[darling(default)]
    from: bool,

    /// override the documentation for the generated Nest struct
    doc: Option<String>,
}
impl NestOpts {
    pub fn key_titlecase(&self) -> String {
        format!("{}", AsTitleCase(&self.key))
    }
    pub fn resolve_name(&self, data_ident: &syn::Ident) -> syn::Ident {
        match &self.rename {
            Some(name) => {
                if name.is_empty() {
                    panic!("Nest struct name cannot be empty")
                } else {
                    syn::Ident::new(&name, data_ident.span())
                }
            }
            None => {
                let default_name = format!("{data_ident}Nested{}", self.key_titlecase());
                syn::Ident::new(&default_name, data_ident.span())
            }
        }
    }
}

/// Options for struct field attribute
///
/// e.g.
/// ```ignore
/// #[shrinkwrap(doc = "", with_nest(nest1, nest2))]`
/// pub struct User { /* */ }
/// ```
#[derive(Debug, Clone, FromField)]
#[darling(attributes(shrinkwrap))]
pub struct DataFieldOpts {
    /// only None for tuple fields, therefore safe to unwrap
    ident: Option<syn::Ident>,
    ty: syn::Type,

    /// list containing the IDs of nests for which this field should be included/mapped
    #[darling(default)]
    with_nest: PathList,

    /// override the field's documentation for this nest
    doc: Option<String>,
}
impl DataFieldOpts {
    pub fn check_issues() -> Option<String> {
        None
    }
}
