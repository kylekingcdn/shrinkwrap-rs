#![doc = "Types used for deserializing attributes (via Darling)"]

use std::collections::HashSet;

use darling::ast::Data;
use darling::util::{Flag, Override, PathList, SpannedValue};
use darling::{FromDeriveInput, FromField, FromMeta};
use heck::AsUpperCamelCase;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Attribute, Ident, LitStr, Meta, Path,};

use crate::mapping::types::NestRepo;

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
// - primary darling types

/// Root derive options
#[derive(Debug, Clone, FromDeriveInput)]
#[darling(
    attributes(shrinkwrap),
    forward_attrs(allow, doc, cfg, shrinkwrap_attr),
    supports(struct_named)
)]
pub(crate) struct DeriveItemOpts {
    pub ident: Ident,
    pub data: Data<(), DeriveItemFieldOpts>,
    pub attrs: Vec<Attribute>,

    #[darling(default, rename = "wrapper")]
    pub wrapper_opts: WrapperOpts,

    #[darling(default, rename = "extra")]
    pub extra_opts: ExtraOpts,

    #[darling(default, rename = "nest", multiple)]
    pub nest_opts: Vec<NestOpts>,

    #[darling(flatten)]
    pub global_opts: GlobalOpts,
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

#[derive(Debug, Clone, FromMeta)]
pub struct GlobalOpts {
    /// Path of transform type used for this nest group
    pub transform: Path,

    /// Enables auto-derivation of `schemars::JsonSchema` on all generated structs
    schema: Flag,

    /// Implies `schema`, add #[schemars(inline)] to all generated structs, Adds a schemars rename on the primary wrapper to the origin data struct name.
    inline: Flag,

    /// Equivalent to setting `optional` on all nests.
    all_optional: Flag,
}
impl GlobalOpts {
    pub fn schema(&self) -> bool {
        self.schema.is_present()
    }
    pub fn inline(&self) -> bool {
        self.inline.is_present()
    }
    pub fn all_optional(&self) -> bool {
        self.all_optional.is_present()
    }
}

pub struct State {
    pub nest_repo: NestRepo,

    pub global: GlobalOpts,
    pub wrapper_opts: WrapperOpts,
    pub extra_opts: ExtraOpts,

    pub root_ident: Ident,
}
impl State {
    pub fn new(global: GlobalOpts, wrapper: WrapperOpts, extra: ExtraOpts, root_ident: Ident) -> Self {
        Self {
            nest_repo: NestRepo::new(root_ident.clone()),

            global,
            wrapper_opts: wrapper,
            extra_opts: extra,

            root_ident,
        }
    }
    fn base_derives() -> Vec<TokenStream> {
        [
            quote!(core::fmt::Debug),
            quote!(core::clone::Clone),
            quote!(serde::Serialize),
        ]
        .into()
    }
    pub fn default_derives(&self) -> Vec<TokenStream> {
        let mut derives = Self::base_derives();

        // derive `JsonSchema` if either schema or inline attribute flags are set
        if self.global.schema.is_present() || self.global.inline.is_present() {
            derives.push(quote!(schemars::JsonSchema));
        }

        derives
    }
}

/// Options for struct field attributes
#[derive(Debug, Clone, FromField)]
#[darling(attributes(shrinkwrap), forward_attrs(shrinkwrap_attr))]
pub struct DeriveItemFieldOpts {
    /// only None for tuple fields, therefore safe to unwrap
    pub ident: Option<Ident>,
    pub attrs: Vec<Attribute>,

    #[darling(default)]
    pub nests: NestIdSelection,
}
impl ValidateScoped for DeriveItemFieldOpts {}


/// Options for struct wrapper attribute
#[derive(Debug, Clone, Default, FromMeta)]
pub struct WrapperOpts {
    /// Set the struct name suffix used by all associated wrappers (primary + any nested wrappers).
    ///
    /// Defaults to `Wrapper`
    ///
    /// E.g. For a data struct named: `MyData`, the default corresponding wrapper struct would be `MyDataWrapper`
    #[darling(default)]
    struct_suffix: Option<Ident>,

    /// Derives to apply to the wrapper struct
    #[darling(default)]
    pub derive: PathList,

    /// Sets documentation for the generated Wrapper struct
    pub doc: Option<String>,

    /// Field name for data struct, defaults to data
    #[darling(default)]
    data_field_name: Option<Ident>,

    /// Sets field-level documentation for data field
    pub data_field_doc: Option<String>,

    /// Serializes data contents into the wrapper inline via `#[serde(flatten)`.
    ///
    /// **NOTE:** `#[serde(flatten)]` is applied to the wrapper data field, **and not the wrapper itself**.
    ///
    /// `flatten = false` will disable data flattening and retain nesting during serialization.
    ///
    /// ### Side effects
    ///
    /// Disabling data flattening may cause some unexpected changes to rendered data hierarchy (via `#[shrinkwrap(nest(.., nested(origin = ..)))]`).
    ///
    /// The current behaviour for parent nests (nests with subsequent data further nested below them), is to provide an intermediate `Wrapper` between itself and the deeply nested data.
    ///
    /// This is done on nests for the the same reason it is done on root data struct - it provides the exact same set of benefits.
    ///
    /// As a result, when flattening is disabled, data trees become inconsistent. Where non-leaf nests have an extra 'data' object between it and it's data, whereas leaf nests will not have this.
    ///
    /// For APIs, this will inevitably lead to a terrible UX for clients. When resources/data structs are shared among responses, the resulting effect is data remaining the same, yet the surrounding schema 'skeleton' changes per-route. =
    ///
    /// ##### This is the opposite of what most would expect.
    ///
    /// <div class="warning">
    /// If the derived structs will be exposed as a response format, API or otherwise, then<br>
    /// <br>
    /// <b>Do not disable struct flattening</b>
    /// </div>
    flatten: Option<Override<bool>>,

    /// Field name for extra struct, defaults to data
    #[darling(default)]
    extra_field_name: Option<Ident>,

    /// Sets field-level documentation for extra field
    pub extra_field_doc: Option<String>,
}
impl WrapperOpts {
    fn struct_name_suffix_default() -> Ident {
        format_ident!("Wrapper")
    }
    pub fn struct_name_suffix(&self) -> Ident {
        match &self.struct_suffix {
            Some(suffix) => suffix.clone(),
            None => Self::struct_name_suffix_default(),
        }
    }
    pub fn struct_name(&self, data_ident: &Ident) -> Ident {
        format_ident!("{}{}", data_ident, self.struct_name_suffix())
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
    /// Set the `extra` struct name suffix - defaults to `Extra`
    ///
    /// E.g. For a data struct named: `MyData`, the default corresponding extra struct would be `MyDataExtra`
    #[darling(default)]
    struct_suffix: Option<Ident>,

    /// Derives to apply to the extra struct - Debug, Clone, and serde::Serialize are required and auto-derived
    #[darling(default)]
    pub derive: PathList,

    /// Sets struct-level documentation for the generated Extra struct
    pub doc: Option<String>,
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
        format_ident!("{}{}", parent_data_ident, self.struct_name_suffix())
    }
}
impl ValidateScoped for ExtraOpts {}

/// Options for struct nest attribute
#[derive(Debug, Clone, FromMeta)]
/// Options for struct nest attribute
pub struct DeeplyNestedOpts {
    // #[darling(with=Self::parse_origin)]
    pub origin: Ident,
}

#[derive(Debug, Clone, FromMeta)]
pub struct NestOpts {
    /// used for specifying/identifying a nest from an attribute. Must be unique among all nests under a given Data struct
    pub id: SpannedValue<String>,

    /// used for the nest field name under `data.extra`.
    /// This should typically be identical and must be unique among the other sibling nests.
    /// Typically this should only be used when implementing nested data hierarchies via [`origin`](Self::origin)
    ///
    /// Defaults to `self.id`
    field_name: Option<Ident>,

    /// sets the name of the nests' generated struct - defaults to `{DataStructName}{UpperCamel(field_name ? field_name : id)}`
    rename: Option<Ident>,

    /// Derives to apply to the nest struct - Debug, Clone, and serde::Serialize are required and auto-derived
    #[darling(default)]
    pub derive: PathList,

    /// sets the type for the fields in the nested struct
    pub field_type: Path,

    pub nested: Option<DeeplyNestedOpts>,

    /// Sets the struct-level documentation for the generated Nest struct
    pub struct_doc: Option<String>,

    /// Sets the documentation of any fields that refer to this type (e.g. in `Extra` structs)
    pub parent_field_doc: Option<String>,

    /// The parent extra struct will type the field for this nest with `Option<T>`, e.g, the generated extra struct would look like
    /// ```rust
    /// pub struct MyDataExtra {
    ///     pub text: Option<MyDataNestedText>,
    /// }
    /// ```
    pub optional: Flag,
}
impl NestOpts {
    fn field_name_default(&self) -> Ident {
        format_ident!("{}", self.id.as_ref())
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

        format_ident!("{}{region_descriptor}{}", origin_ident, suffix)
    }
    /// `root_ident` is the ident of the top-level data struct containing derive(Wrap)
    /// It is used to form the base struct name when an origin isn't explicitly provided
    pub fn struct_name_default(&self, root_ident: &Ident) -> Ident {
        let origin_ident = self.origin(root_ident);
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
    pub fn struct_name_span(&self) -> Span {
        if let Some(rename) = &self.rename {
            rename.span()
        } else if let Some(field_name) = &self.field_name {
            field_name.span()
        } else {
            self.id.span()
        }
    }
    pub fn optional(&self) -> bool {
        self.optional.is_present()
    }
    pub fn origin<'a>(&'a self, root_ident: &'a Ident) -> &'a Ident {
        match &self.nested {
            Some(nested_opts) => &nested_opts.origin,
            None => root_ident,
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

// - helper types

pub type NestIdSelection = Vec<LitStr>;

// attribute passthrough opts for structs
#[derive(Debug, Clone, FromMeta)]
pub struct PassthroughStructAttribute {
    pub attr: Meta,

    #[darling(default)]
    pub limit: DerivedStructRestriction,
}
impl From<PassthroughFieldAttribute> for PassthroughStructAttribute {
    fn from(field_attr: PassthroughFieldAttribute) -> Self {
        Self {
            attr: field_attr.attr,
            limit: field_attr.limit.into(),
        }
    }
}

#[derive(Debug, Clone, Default, FromMeta)]
pub struct DerivedStructRestriction {
    // list of nest IDs
    pub nests: Option<NestIdSelection>,

    #[darling(default, with=DerivedStructClassSelection::parse_input)]
    pub class: Option<DerivedStructClassSelection>,
}
impl From<DerivedStructFieldRestriction> for DerivedStructRestriction {
    fn from(field_restrict: DerivedStructFieldRestriction) -> Self {
        Self {
            nests: field_restrict.nests,
            class: None,
        }
    }
}

// attribute passthrough opts for fields

#[derive(Debug, Clone, FromMeta)]
pub struct PassthroughFieldAttribute {
    pub attr: Meta,

    #[darling(default)]
    pub limit: DerivedStructFieldRestriction,
}

#[derive(Debug, Clone, Default, FromMeta)]
pub struct DerivedStructFieldRestriction {
    // list of nest IDs
    pub nests: Option<NestIdSelection>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum DerivedStructClass {
    Wrapper,
    Nest,
    Extra,
}
impl DerivedStructClass {
    pub fn key(&self) -> String {
        match self {
            Self::Wrapper => "wrapper",
            Self::Nest => "nest",
            Self::Extra => "extra",
        }.into()
    }
}
impl TryFrom<&syn::Path> for DerivedStructClass {
    type Error = darling::Error;

    fn try_from(value: &syn::Path) -> Result<Self, Self::Error> {
        if let Some(ident) = value.get_ident() {
            let class_type = match ident.to_string().as_str() {
                "wrapper" => Some(Self::Wrapper),
                "extra" => Some(Self::Extra),
                "nest" => Some(Self::Nest),
                _ => None,
            };
            if let Some(class) = class_type {
                return Ok(class);
            }
        }
        Err(darling::Error::custom("Invalid class type specified. Valid types: [wrapper, extra, nest]").with_span(&value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedStructClassSelection(HashSet<DerivedStructClass>);
impl Default for DerivedStructClassSelection {
    fn default() -> Self {
        let mut set = HashSet::new();
        set.insert(DerivedStructClass::Wrapper);
        set.insert(DerivedStructClass::Extra);
        set.insert(DerivedStructClass::Nest);
        Self(set)
    }
}

impl DerivedStructClassSelection {
    pub fn contains(&self, class: DerivedStructClass) -> bool {
        self.0.contains(&class)
    }
    pub fn parse_input(meta: &syn::Meta) -> darling::Result<Option<Self>> {
        let pathlist = PathList::from_meta(meta)?;
        Self::try_from(pathlist).map(Some)
    }
}
impl TryFrom<PathList> for DerivedStructClassSelection {
    type Error = darling::Error;

    fn try_from(paths: PathList) -> Result<Self, Self::Error> {
        let mut set = HashSet::new();
        for path in paths.iter() {
            let class_type = DerivedStructClass::try_from(path)?;
            if set.contains(&class_type) {
                let msg = format!("Class type defined multiple times: {}", class_type.key());
                return Err(darling::Error::custom(&msg).with_span(&path));
            }
            set.insert(class_type);
        }
        Ok(Self(set))
    }
}
