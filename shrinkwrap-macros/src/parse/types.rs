#![doc="Types used for deserializing attributes (via Darling)"]

use darling::{FromDeriveInput, FromField, FromMeta};
use darling::ast::Data;
use darling::util::{Override, PathList};
use heck::AsTitleCase;
use syn::{Ident, Path, Type};

// - validate trait

pub(crate) type InvalidityReason = String;
pub(crate) type HasInvalidity = Option<Vec<InvalidityReason>>;

/// Performs baseline validation of local fields.
///
/// Should not perform higher-level validation with other types
pub(crate) trait ValidateScoped {
    fn validate_within_scope(&self) -> HasInvalidity;
}

// - darling types

/// Root derive options
#[derive(Debug, Clone, FromDeriveInput)]
#[darling(attributes(shrinkwrap), forward_attrs(allow, doc, cfg), supports(struct_named))]
pub(crate) struct DeriveItemOpts {
    pub ident: Ident,
    pub data: Data<(), DeriveItemFieldOpts>,

    #[darling(default, rename = "wrapper")]
    pub wrapper_opts: WrapperOpts,

    #[darling(default, rename = "extra")]
    pub extra_opts: ExtraOpts,

    #[darling(default, rename = "nest", multiple)]
    pub nest_opts: Vec<NestOpts>,
}
impl ValidateScoped for DeriveItemOpts {
    fn validate_within_scope(&self) -> HasInvalidity {
        let mut issues = Vec::new();
        if let Some(new_issues) = self.wrapper_opts.validate_within_scope() {
            issues.extend(new_issues);
        }
        if let Some(new_issues) = self.extra_opts.validate_within_scope() {
            issues.extend(new_issues);
        }
        for nest_group in &self.nest_opts {
            if let Some(nest_issues) = nest_group.validate_within_scope() {
                issues.extend(nest_issues);
            }
        }

        if issues.is_empty() {
            None
        } else {
            issues.into()
        } // TODO: field validation
    }
}

// TODO: add support for nesting of nests (sorry)
//
// e.g. a nest would also be a wrapper for subsequent nests.
// From the client perspective, this is just adding an `extra` object & properties to a nest.
//
// Useful for situations where there are layered representations
// e.g. consider the following 2 nests representing an amount field
//
//   ```
//   amount_in_local_currency -> text
//   amount_in_local_currency -> usd_value
//   ```
//
// there should certainly be this chain as well.
//
// ```
//   amount_in_local_currency -> usd_value -> text
// ```
//
/// Options for struct wrapper attribute
#[derive(Debug, Clone, Default, FromMeta)]
pub struct WrapperOpts {
    /// set the parent wrapper struct name - defaults to `{DataStructName}Wrapper`
    rename: Option<String>,

    /// Derives to apply to the wrapper struct
    #[darling(default)]
    pub derive: PathList,

    /// Sets documentation for the generated Wrapper struct
    #[darling(default = String::new)]
    pub doc: String,

    /// Field name for data struct, defaults to data
    #[darling(default = Self::data_field_name_default)]
    data_field_name: String,

    /// Sets field-level documentation for data field
    #[darling(default = String::new)]
    pub data_field_doc: String,

    /// Serializes data fields inline with the wrapper via `#[serde(flatten)`.
    ///
    /// Set to false to disable and retain nesting during serialization.
    #[darling(default = Self::flatten_data_override_default)]
    pub flatten_data: Override<bool>,

    /// Field name for extra struct, defaults to data
    #[darling(default = Self::extra_field_name_default)]
    extra_field_name: String,

    /// Sets field-level documentation for extra field
    #[darling(default = String::new)]
    pub extra_field_doc: String,
}
impl WrapperOpts {
    pub fn struct_name_default(data_ident: &Ident) -> Ident {
        Ident::new(format!("{data_ident}Wrapper").as_str(), data_ident.span())
    }
    pub fn struct_name(&self, data_ident: &Ident) -> Ident {
        match &self.rename {
            Some(name) => Ident::new(name, data_ident.span()),
            None => Self::struct_name_default(data_ident),
        }
    }
    fn data_field_name_default() -> String {
        "data".into()
    }
    pub fn data_field_name(&self) -> String {
        if self.data_field_name.is_empty() {
            Self::data_field_name_default()
        } else {
            self.data_field_name.clone()
        }
    }
    fn flatten_data_default() -> bool {
        true
    }
    fn flatten_data_override_default() -> Override<bool> {
        Some(Self::flatten_data_default()).into()
    }

    fn extra_field_name_default() -> String {
        "extra".into()
    }
    pub fn extra_field_name(&self) -> String {
        if self.extra_field_name.is_empty() {
            Self::extra_field_name_default()
        } else {
            self.extra_field_name.clone()
        }
    }
}
impl ValidateScoped for WrapperOpts {
    fn validate_within_scope(&self) -> HasInvalidity {
        let mut issues = Vec::new();

        if let Some(rename) = &self.rename {
            if rename.is_empty() {
                issues.push("Wrapper `rename` must have a value when explicitly defined".into());
            }
        }
        if issues.is_empty() {
            None
        } else {
            issues.into()
        }
    }
}

/// Options for struct extra attribute
#[derive(Debug, Clone, Default, FromMeta)]
pub struct ExtraOpts {
    /// set the `extra` struct name - defaults to `{DataStructName}Extra`
    rename: Option<String>,

    /// Derives to apply to the extra struct - Debug, Clone, and serde::Serialize are required and auto-derived
    #[darling(default)]
    pub derive: PathList,

    /// Sets struct-level documentation for the generated Extra struct
    #[darling(default = String::new)]
    pub doc: String,
}
impl ExtraOpts {
    fn struct_name_default(data_ident: &Ident) -> Ident {
        Ident::new(format!("{data_ident}Extra").as_str(), data_ident.span())
    }
    pub fn struct_name(&self, data_ident: &Ident) -> Ident {
        match &self.rename {
            Some(name) => Ident::new(name, data_ident.span()),
            None => Self::struct_name_default(data_ident),
        }
    }
}
impl ValidateScoped for ExtraOpts {
    fn validate_within_scope(&self) -> HasInvalidity {
        let mut issues = Vec::new();

        if let Some(rename) = &self.rename {
            if rename.is_empty() {
                issues.push("Extra `rename` must have a value when explicitly defined".into());
            }
        }
        if issues.is_empty() {
            None
        } else {
            issues.into()
        }
    }
}

/// Options for struct nest attribute
#[derive(Debug, Clone, FromMeta)]
pub struct NestOpts {
    /// used for the nest field key under `data.extra` as well as an identifier for other attributes
    pub key: String,

    /// sets the name of the nests' generated struct - defaults to `{DataStructName}{titlecased_key}`
    rename: Option<String>,

    /// Derives to apply to the nest struct - Debug, Clone, and serde::Serialize are required and auto-derived
    #[darling(default)]
    pub derive: PathList,

    /// sets the type for the fields in the nested struct
    pub field_type: Path,

    /// Path to transform function used to convert data struct into nest struct.
    pub transform: Option<Type>,

    /// Derives the transform using an existing `impl From<&Data> for DataNest`
    #[darling(default)]
    pub from: bool,

    /// Sets the struct-level documentation for the generated Nest struct
    #[darling(default = String::new)]
    pub doc: String,
}
impl NestOpts {
    pub fn build_struct_name_default(data_ident: &Ident, key: &str) -> Ident {
        let key_titlecase = format!("{}", AsTitleCase(key));
        Ident::new(format!("{data_ident}Nested{key_titlecase}").as_str(), data_ident.span())
    }
    pub fn struct_name_default(&self, data_ident: &Ident) -> Ident {
        Self::build_struct_name_default(data_ident, &self.key)
    }
    pub fn struct_name(&self, data_ident: &Ident) -> Ident {
        match &self.rename {
            Some(name) => Ident::new(name, data_ident.span()),
            None => self.struct_name_default(data_ident),
        }
    }
}
impl ValidateScoped for NestOpts {
    fn validate_within_scope(&self) -> HasInvalidity {
        let mut issues = Vec::new();
        if self.key.is_empty() {
            issues.push("Nest `key` cannot be empty".into());
        }
        if let Some(rename) = &self.rename {
            if rename.is_empty() {
                issues.push("Nest `rename` must have a value when explicitly defined".into());
            }
        }
        // skipping complicated `field_type` path check now as it will be done at higher level validation

        let has_transform = self.transform.is_some();
        let has_from = self.from;
        if has_transform && has_from {
            issues.push("Nest attributes `from` and `transform` cannot both be defined in the same nest".into());
        } else if !has_transform && !has_from {
            issues.push("Either `from` or `transform` must be defined for a nest".into());
        }

        if issues.is_empty() {
            None
        } else {
            issues.into()
        }
    }
}

/// Options for struct field attributes
#[derive(Debug, Clone, FromField)]
#[darling(attributes(shrinkwrap))]
pub struct DeriveItemFieldOpts {
    /// only None for tuple fields, therefore safe to unwrap
    pub ident: Option<Ident>,

    #[darling(default, multiple, rename = "nest_in")]
    pub nest_in_opts: Vec<NestInOpts>,
}
impl ValidateScoped for DeriveItemFieldOpts {
    fn validate_within_scope(&self) -> HasInvalidity {
        None
    }
}


/// Wrap field `nest_in` attribute options
#[derive(Debug, Clone, FromMeta)]
pub struct NestInOpts {
    /// Nest key for which this field should be included/mapped
    #[darling(rename = "key")]
    pub nest_key: Ident,

    /// Set the field's documentation for this nest
    #[darling(default = String::new)]
    pub field_doc: String,
}
impl ValidateScoped for NestInOpts {
    fn validate_within_scope(&self) -> HasInvalidity {
        None
    }
}
