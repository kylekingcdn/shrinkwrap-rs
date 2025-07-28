#![doc = "Types used for deserializing attributes (via Darling)"]

use darling::ast::Data;
use darling::util::{Flag, Override, PathList};
use darling::{FromDeriveInput, FromField, FromMeta};
use heck::AsUpperCamelCase;
use quote::format_ident;
use syn::{Ident, Path, Type};

// - validate trait

pub(crate) type InvalidityReason = String;
pub(crate) type HasInvalidity = Option<Vec<InvalidityReason>>;

/// Performs baseline validation of local fields.
///
/// Should not perform higher-level validation with other types
pub(crate) trait ValidateScoped {
    fn validate_within_scope(&self) -> HasInvalidity {
        None
    }
}

// - darling types

/// Root derive options
#[derive(Debug, Clone, FromDeriveInput)]
#[darling(
    attributes(shrinkwrap, shrinkwrap_attr),
    forward_attrs(allow, doc, cfg),
    supports(struct_named)
)]
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
        }
    }
}

/// Options for struct wrapper attribute
#[derive(Debug, Clone, Default, FromMeta)]
pub struct WrapperOpts {
    /// set the parent wrapper struct name - defaults to `{DataStructName}Wrapper`
    rename: Option<Ident>,

    /// Derives to apply to the wrapper struct
    #[darling(default)]
    pub derive: PathList,

    /// Sets documentation for the generated Wrapper struct
    #[darling(default = String::new)]
    pub doc: String,

    /// Field name for data struct, defaults to data
    #[darling(default)]
    data_field_name: Option<Ident>,

    /// Sets field-level documentation for data field
    #[darling(default = String::new)]
    pub data_field_doc: String,

    /// Serializes data contents into the wrapper inline via `#[serde(flatten)`.
    ///
    /// **NOTE:** `#[serde(flatten)]` is applied to the wrapper data field, **and not the wrapper itself**
    ///
    /// `flatten = false` will disable data flattening and retain nesting during serialization.
    flatten: Option<Override<bool>>,

    /// Field name for extra struct, defaults to data
    #[darling(default)]
    extra_field_name: Option<Ident>,

    /// Sets field-level documentation for extra field
    #[darling(default = String::new)]
    pub extra_field_doc: String,
}
impl WrapperOpts {
    pub fn struct_name_default(data_ident: &Ident) -> Ident {
        format_ident!("{data_ident}Wrapper")
    }
    pub fn struct_name(&self, data_ident: &Ident) -> Ident {
        match &self.rename {
            Some(name) => name.clone(),
            None => Self::struct_name_default(data_ident),
        }
    }
    fn data_field_name_default() -> Ident {
        format_ident!("data")
    }
    pub fn data_field_name(&self) -> Ident {
        match &self.data_field_name {
            Some(name) => name.clone(),
            None => Self::data_field_name_default(),
        }
    }
    pub fn flatten(&self) -> bool {
        match self.flatten {
            Some(Override::Inherit)
            | Some(Override::Explicit(true))
            | None => true,
            Some(Override::Explicit(false)) => false,
        }
    }
    fn extra_field_name_default() -> Ident {
        format_ident!("extra")
    }
    pub fn extra_field_name(&self) -> Ident {
        match &self.extra_field_name {
            Some(name) => name.clone(),
            None => Self::extra_field_name_default(),
        }
    }
}
impl ValidateScoped for WrapperOpts {}

/// Options for struct extra attribute
#[derive(Debug, Clone, Default, FromMeta)]
pub struct ExtraOpts {
    /// set the `extra` struct name suffix - defaults to `Extra` (full struct name would be {DataStructName}Extra`)
    #[darling(default)]
    struct_suffix: Option<Ident>,

    /// Derives to apply to the extra struct - Debug, Clone, and serde::Serialize are required and auto-derived
    #[darling(default)]
    pub derive: PathList,

    /// Sets struct-level documentation for the generated Extra struct
    #[darling(default = String::new)]
    pub doc: String,
}
impl ExtraOpts {
    fn struct_name_suffix_default() -> Ident {
        format_ident!("Extra")
    }
    pub fn struct_name_suffix(&self) -> Ident {
        match &self.struct_suffix {
            Some(suffix) => suffix.clone(),
            None => Self::struct_name_suffix_default(),
        }
    }
    pub fn struct_name(&self, parent_data_ident: &Ident) -> Ident {
        format_ident!("{parent_data_ident}{}", self.struct_name_suffix())
    }
}
impl ValidateScoped for ExtraOpts {}

#[derive(Debug, Clone, FromMeta, PartialEq, Eq)]
#[darling(rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum NestMapStrategy {
    From,
    Transform { with: Type },
    Nested { origin: Ident }
}
impl NestMapStrategy {
    pub fn maps_with_from(&self) -> bool {
        matches!(self, Self::From)
    }
    pub fn maps_with_transform(&self) -> bool {
        matches!(self, Self::Transform { .. })
    }
    pub fn map_transform_type(&self) -> Option<Type> {
        match &self {
            NestMapStrategy::Transform { with } => Some(with.clone()),
            NestMapStrategy::From => None,
            NestMapStrategy::Nested { .. } => None,
        }
    }
}

/// Options for struct nest attribute
#[derive(Debug, Clone, FromMeta)]
pub struct NestOpts {
    /// used for specifying/identifying a nest from an attribute. Must be unique among all nests under a given Data struct
    pub id: String,

    /// used for the nest field name under `data.extra`.
    /// This should typically be identical and must be unique among the other sibling nests.
    /// Typically this should only be used when implementing nested data hierarchies via [`origin`](Self::origin)
    ///
    /// Defaults to `self.id`
    field_name: Option<Ident>,

    /// sets the name of the nests' generated struct - defaults to `{DataStructName}{upper_camel_case(id)}`
    rename: Option<Ident>,

    /// Derives to apply to the nest struct - Debug, Clone, and serde::Serialize are required and auto-derived
    #[darling(default)]
    pub derive: PathList,

    /// sets the type for the fields in the nested struct
    pub field_type: Path,

    /// Strategy used to map data to this nest.
    ///
    /// 1: **`from`**
    ///
    ///    Use a **pre-existing** `from` impl:
    ///    ```rust
    ///    impl From<&MyData> for MyDataNestedExample
    ///    ```
    /// 2: **`transform(with = "TransformTypeGoesHere")`**
    ///
    ///    Use a **pre-existing** transform impl of the provided type.
    ///
    ///    The transform type must implement `TransformToNest`. e.g.
    ///    ```rust
    ///    struct MyTransform {}
    ///
    ///    impl TransformToNest<TestData, TestDataNestedText> for MyTransform {
    ///        fn transform_to_nest(&self, data: &TestData) -> TestDataNestedText {
    ///            TestDataNestedText {
    ///                random_number: data.random_number.to_string(),
    ///            }
    ///        }
    ///    }
    ///    ```
    /// 3: **`nested(origin = "MyDataNestedExample")`**
    ///
    ///    Nest this under an existing nest, where origin = "NestStructType".
    ///
    ///    No mapping is needed in this case as mappings are only required for root-level nests.
    ///    (The mapping logic is still supplied by you, however as a part of the parent nest's mapping strategy)
    ///
    ///    This allows you to use either the parent nest's data or the root data as the source for transformation.
    ///    **Note:** This cannot be some arbitrary type. It must be:
    ///      1. Built internally by this derive macro
    ///      2. Exist within this data struct tree (rather than a struct generated for another data tree)
    #[darling(flatten)]
    pub map_strategy: NestMapStrategy,

    /// Sets the struct-level documentation for the generated Nest struct
    #[darling(default = String::new)]
    pub doc: String,
}
impl NestOpts {
    fn field_name_default(&self) -> Ident {
        format_ident!("{}", self.id)
    }
    pub fn field_name(&self) -> Ident {
        match &self.field_name {
            Some(name) => name.clone(),
            None => self.field_name_default(),
        }
    }
    pub fn build_struct_name_suffix(field_name: &Ident) -> Ident {
        let suffix = AsUpperCamelCase(field_name.to_string());
        format_ident!("{suffix}")
    }
    pub fn build_default_struct_name(
        origin_ident: &Ident,
        root_ident: &Ident,
        field_name: &Ident,
    ) -> Ident {
        // To avoid obnoxiously long struct names, only include the nested keyword once (root nests)
        let region_descriptor = if origin_ident == root_ident {
            "Nested"
        } else {
            Default::default()
        };
        let suffix = Self::build_struct_name_suffix(field_name);

        format_ident!("{origin_ident}{region_descriptor}{suffix}")
    }
    /// `root_ident` is the ident of the top-level data struct containing derive(Wrap)
    /// It is used to form the base struct name when an origin isn't explicitly provided
    pub fn struct_name_default(&self, root_ident: &Ident) -> Ident {
        let origin_ident = match &self.map_strategy {
            NestMapStrategy::Nested { origin } => origin,
            _ => root_ident
        };
        let field_name = self.field_name();
        Self::build_default_struct_name(origin_ident, root_ident, &field_name)
    }
    /// `root_ident` is the ident of the top-level data struct containing derive(Wrap)
    /// It is used to form the base struct name when an origin isn't explicitly provided
    pub fn struct_name(&self, root_ident: &Ident) -> Ident {
        match &self.rename {
            Some(name) => name.clone(),
            None => self.struct_name_default(root_ident),
        }
    }

    pub fn origin<'a>(&'a self, root_ident: &'a Ident) -> &'a Ident {
        match &self.map_strategy {
            NestMapStrategy::Nested { origin } => origin,
            _ => root_ident,
        }
    }
}
impl ValidateScoped for NestOpts {
    fn validate_within_scope(&self) -> HasInvalidity {
        let mut issues = Vec::new();
        if self.id.is_empty() {
            issues.push("Nest `id` cannot be empty".into());
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
#[darling(attributes(shrinkwrap), forward_attrs(shrinkwrap_attr))]
pub struct DeriveItemFieldOpts {
    /// only None for tuple fields, therefore safe to unwrap
    pub ident: Option<Ident>,

    #[darling(default, multiple, rename = "nest_in")]
    pub nest_in_opts: Vec<NestInOpts>,

    pub attrs: Vec<syn::Attribute>,
}
impl ValidateScoped for DeriveItemFieldOpts {}

#[derive(Debug, Clone, FromMeta)]
pub struct PassthroughAttribute {
    #[darling(default)]
    pub nest: PathList,
    #[darling(multiple)]
    pub attr: Vec<syn::Meta>,
}

/// Wrap field `nest_in` attribute options
#[derive(Debug, Clone, FromMeta)]
pub struct NestInOpts {
    /// Nest key for which this field should be included/mapped
    #[darling(rename = "id")]
    pub nest_id: Ident,

    /// Set the field's documentation for this nest
    #[darling(default = String::new)]
    pub field_doc: String,
}
impl ValidateScoped for NestInOpts {
    fn validate_within_scope(&self) -> HasInvalidity {
        None
    }
}
