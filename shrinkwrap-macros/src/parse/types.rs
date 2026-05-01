#![doc = "Types used for deserializing attributes (via Darling)"]


use darling::ast::Data;
use darling::util::{Flag, Override, PathList, SpannedValue};
use darling::{FromDeriveInput, FromField, FromMeta};
use heck::AsUpperCamelCase;
use proc_macro_error2::{OptionExt, abort, emit_error};
use proc_macro2::{Span, TokenStream};
use quote::format_ident;
use std::collections::HashSet;
use syn::{Attribute, Ident, LitStr, Meta, Path, Type, parse_quote, spanned::Spanned};

// !- Statics & Consts

static FORWARD_ATTR: &str = "shrinkwrap_attr";

// !- Derive entrypoint

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
    pub nest_opts: Vec<SpannedValue<NestOpts>>,

    #[darling(flatten)]
    pub global_opts: GlobalOpts,
}
impl DeriveItemOpts {
    pub(crate) fn validate(&self) -> bool {
        let wrapper_errors = self.wrapper_opts.validate();
        let extra_errors = self.extra_opts.validate();

        let mut nest_errors = 0;
        for nest in &self.nest_opts {
            nest_errors += nest.validate(nest.span());
        }

        let self_errors = self.validate_self();
        let total_errors = wrapper_errors + extra_errors + nest_errors + self_errors;

        total_errors == 0
    }
    fn validate_self(&self) -> usize {
        let mut errors = 0;

        let all_nest_ids = self.nest_opts.iter().map(|nest| nest.id_str().to_string()).collect::<Vec<_>>();
        // validate field nest id's exist
        if let Data::Struct(data) = &self.data {
            for field in &data.fields {
                for nest in &field.nests {
                    if !all_nest_ids.contains(&nest.value()) {
                        emit_error!(nest, "Nest `{}` is not defined", nest.value());
                        errors += 1;
                    }
                }
            }
        } else {
            emit_error!(Span::call_site(), "Only named structs are supported");
            errors += 1;
        }

        // validate for conflicting optional/derive to nest option_field
        let all_optional = self.global_opts.all_optional.is_present();
        for nest in &self.nest_opts {
            let nest_optional = all_optional || nest.optional();
            // nest not optional, option_field set
            if let Some(derive_to_nest) = &nest.derive_to_nest && let Some(option_field) = &derive_to_nest.options_field {
                if !nest_optional {
                    emit_error!(option_field, "options_field can only be used for optional nests");
                }
                errors += 1;
            }
        }

        errors
    }
}

// !- Struct field entrypoint

/// Options for struct field attributes
#[derive(Debug, Clone, FromField)]
#[darling(attributes(shrinkwrap), forward_attrs(shrinkwrap_attr))]
pub(crate) struct DeriveItemFieldOpts {
    /// only None for tuple fields, therefore safe to unwrap
    pub ident: Option<Ident>,
    pub ty: Type,
    pub attrs: Vec<Attribute>,

    #[darling(default)]
    pub nests: NestIdSelection,
}

// !- Container option structs

// !- Global

#[derive(Debug, Clone, FromMeta)]
pub(crate) struct GlobalOpts {
    /// Path of transform type used for this nest group
    pub transform: Path,

    /// Generic type parameters in Transform type, with any required trait
    /// bounds (e.g. `T: Serialize`)
    #[darling(with = Self::parse_transform_generic_params, default)]
    pub transform_generic_params: Option<TokenStream>,

    #[darling(default)]
    pub fallible: Option<GlobalFallibleNestedOpts>,

    /// Enables auto-derivation of `schemars::JsonSchema` on all generated
    /// structs
    schema: Flag,

    /// Implies `schema` flag.
    ///
    /// Adds `#[schemars(inline)]` to all generated structs, enforces flatten on
    /// wrapper structs, adds `#[serde(rename = {OriginStructName})]` on the
    /// primary wrapper (which also implies `schemars(rename)`).
    inline: Flag,

    /// Equivalent to setting `optional` on all nests.
    pub all_optional: Flag,

    /// List of derives to apply to every generated struct: e.g. each wrapper,
    /// extra, nest.
    ///
    /// **Note**: Derive lists are merged. You are free to use both `derive_all`
    /// as well as `derive` on specific struct types (wrapper, extra, nest).
    ///
    /// However, you will still receive an error if the same derive is included
    /// multiple times. This applies to merged derive lists.
    ///
    /// Regardless of user settings, every generated struct will always derive
    /// the following (and therefore should not be manually included in either
    /// a shrinkwrap `derive` attr, or the `derive_all` attr)
    /// - [`Debug`](core::fmt::Debug)
    /// - [`Clone`](core::clone::Clone)
    /// - [`serde::Serialize`](serde::Serialize)
    #[darling(default)]
    pub derive_all: PathList,
}
impl GlobalOpts {
    pub fn schema(&self) -> bool {
        self.schema.is_present() || self.inline.is_present()
    }
    pub fn inline(&self) -> bool {
        self.inline.is_present()
    }
    pub fn parse_transform_generic_params(
        meta: &syn::Meta,
    ) -> darling::Result<Option<TokenStream>> {
        let list = meta.require_list()?;
        Ok(Some(list.tokens.clone()))
    }
}

/// Options for struct nest attribute
#[derive(Debug, Clone, FromMeta)]
pub(crate) struct GlobalFallibleNestedOpts {
    /// Error type used for Result returned by trait impls
    pub error: Path,
}

// ! Wrapper

/// Options for struct wrapper attribute
#[derive(Debug, Clone, FromMeta)]
pub(crate) struct WrapperOpts {
    /// Set the struct name suffix used by all associated wrappers (primary +
    /// any nested wrappers).
    ///
    /// Defaults to `Wrapper`
    ///
    /// E.g. For a data struct named: `MyData`, the default corresponding
    /// wrapper struct would be `MyDataWrapper`
    #[darling(default = WrapperOpts::struct_name_suffix_default)]
    pub struct_suffix: Ident,

    /// Derives to apply to the wrapper struct
    #[darling(default)]
    pub derive: PathList,

    /// Sets documentation for all generated Wrapper structs
    pub struct_doc: Option<String>,

    /// Field name for data struct, defaults to data
    #[darling(default = WrapperOpts::data_field_name_default)]
    pub data_field_name: Ident,

    /// Sets field-level documentation for data field
    pub data_field_doc: Option<String>,

    /// Serializes data contents into the wrapper inline via `#[serde(flatten)`.
    ///
    /// **NOTE:** `#[serde(flatten)]` is applied to the wrapper data field,
    ///  **and not the wrapper itself**.
    ///
    /// `flatten = false` will disable data flattening and retain nesting during
    /// serialization.
    ///
    /// ### Side effects
    ///
    /// Disabling data flattening may cause some unexpected changes in the
    ///  rendered data hierarchy (via `#[shrinkwrap(nest(.., nested(origin = ..)))]`).
    ///
    /// The current behaviour for parent nests (nests with subsequent data
    /// further nested below them), is to provide an intermediate `Wrapper`
    ///  between itself and the deeply nested data.
    ///
    /// This is done on nests for the the same reason it is done on root data struct
    ///  - it provides the exact same set of benefits.
    ///
    /// As a result, when flattening is disabled, data trees become inconsistent.
    /// Where non-leaf nests have an extra 'data' object between it and it's data,
    /// whereas leaf nests will not have this.
    ///
    /// For APIs, this will inevitably lead to a terrible UX for clients.
    /// When resources/data structs are shared among responses,
    /// the resulting effect is data remaining the same,
    /// yet the surrounding schema 'skeleton' changes per-route.
    ///
    /// ##### This is the opposite of what most would expect.
    ///
    /// <div class="warning">
    /// If the derived structs will be exposed as a response format, API or
    /// otherwise, then<br>
    /// <br>
    /// <b>Do not disable struct flattening</b>
    /// </div>
    flatten: Option<Override<bool>>,

    /// Field name for extra struct, defaults to extra
    #[darling(default = WrapperOpts::extra_field_name_default)]
    pub extra_field_name: Ident,

    /// Sets field-level documentation for extra field
    pub extra_field_doc: Option<String>,
}
impl Default for WrapperOpts {
    fn default() -> Self {
        Self {
            struct_suffix: Self::struct_name_suffix_default(),
            derive: PathList::default(),
            struct_doc: None,
            data_field_name: Self::data_field_name_default(),
            data_field_doc: None,
            flatten: None,
            extra_field_name: Self::extra_field_name_default(),
            extra_field_doc: None,
        }
    }
}
impl WrapperOpts {
    fn struct_name_suffix_default() -> Ident {
        format_ident!("Wrapper")
    }
    pub fn struct_name(&self, data_ident: &Ident) -> Ident {
        format_ident!("{data_ident}{}", &self.struct_suffix)
    }
    fn data_field_name_default() -> Ident {
        format_ident!("data")
    }
    pub fn flatten(&self) -> bool {
        match self.flatten {
            Some(Override::Inherit) | Some(Override::Explicit(true)) | None => true,
            Some(Override::Explicit(false)) => false,
        }
    }
    fn extra_field_name_default() -> Ident {
        format_ident!("extra")
    }

    fn validate(&self) -> usize {
        let mut errs = 0;
        if self.data_field_name == self.extra_field_name {
            let invalid_token = if self.data_field_name == Self::data_field_name_default() {
                &self.extra_field_name
            } else {
                &self.data_field_name
            };
            emit_error!(invalid_token, "data_field_name must be different than extra_field_name");
            errs += 1;
        }
        errs
    }
}

// ! Extra

/// Options for struct extra attribute
#[derive(Debug, Clone, FromMeta)]
pub(crate) struct ExtraOpts {
    /// Set the `extra` struct name suffix - defaults to `Extra`
    ///
    /// E.g. For a data struct named: `MyData`,
    /// the default corresponding extra struct would be `MyDataExtra`
    #[darling(default = ExtraOpts::struct_name_suffix_default)]
    pub struct_suffix: Ident,

    /// Derives to apply to the extra struct.
    /// Debug, Clone, and `serde::Serialize` are required and auto-derived
    #[darling(default)]
    pub derive: PathList,

    /// Sets struct-level documentation for all generated Extra structs
    pub struct_doc: Option<String>,
}
impl Default for ExtraOpts {
    fn default() -> Self {
        Self {
            struct_suffix: Self::struct_name_suffix_default(),
            derive: PathList::default(),
            struct_doc: None,
        }
    }
}
impl ExtraOpts {
    fn struct_name_suffix_default() -> Ident {
        format_ident!("Extra")
    }
    pub fn struct_name(&self, parent_data_ident: &Ident) -> Ident {
        format_ident!("{parent_data_ident}{}", &self.struct_suffix)
    }

    fn validate(&self) -> usize {
        let mut errs = 0;
        if self.struct_suffix.to_string().is_empty() {
            emit_error!(self.struct_suffix, "struct_suffix cannot be empty");
            errs += 1;
        }
        errs
    }
}

// ! Nest

#[derive(Debug, Clone, FromMeta)]
pub(crate) struct NestOpts {
    /// used for specifying/identifying a nest from an attribute.
    /// Must be unique among all nests under a given Data struct
    pub id: SpannedValue<String>,

    /// Used for the nest field name under `data.extra`.
    /// Must be unique among the other sibling nests.
    ///
    /// Typically this should only be used when implementing
    /// nested data hierarchies via [`chain_from`](Self::chain_fron)
    ///
    /// Defaults to `self.id`
    pub field_name: Option<Ident>,

    /// sets the name of the nests' generated struct - defaults to
    /// `{SourceStructName}Nested{UpperCamel(field_name || "{self.id}")}`
    pub rename: Option<Ident>,

    /// Derives to apply to the nest struct - `Debug`, `Clone`, and
    /// `serde::Serialize` are required and auto-derived.
    #[darling(default)]
    pub derive: PathList,

    /// Sets the type for the fields in the nested struct.
    //
    /// Cannot be used alongside `derive_to_nest` within the same nest.
    pub field_type: Option<Path>,

    /// Derive `TransformToNest`/`TryTransformToNest` automatically.
    /// Cannot be used alongside `field_type` within the same nest.
    pub derive_to_nest: Option<SpannedValue<DeriveToNest>>,

    // FIXME: validate field is in parent!

    /// Optional Nest ID, allows for embedding  this nest within another nest
    pub chain_from: Option<SpannedValue<String>>,

    /// Sets the struct-level documentation for the generated Nest struct
    pub struct_doc: Option<String>,

    /// The parent extra struct will type the field for this nest with
    /// `Option<T>`, e.g, the generated extra struct would look like
    /// ```rust
    /// pub struct MyDataExtra {
    ///     pub text: Option<MyDataNestedText>,
    /// }
    /// ```
    pub optional: Flag,
}
impl NestOpts {
    pub fn id_str(&self) -> &str {
        self.id.as_str()
    }
    fn field_name_default(&self) -> Ident {
        format_ident!("{}", self.id.as_ref())
    }
    pub fn field_name(&self) -> Ident {
        match &self.field_name {
            Some(name) => name.clone(),
            None => self.field_name_default(),
        }
    }
    pub fn is_root_nest(&self) -> bool {
        self.chain_from.is_none()
    }

    /// `origin_ident`: The ident of the source data struct (origin struct
    /// for root nests, parent nest for deeply nested)
    pub fn build_default_struct_name(
        origin_ident: &Ident,
        field_name: &Ident,
        is_root_nest: bool,
    ) -> Ident {
        // To avoid obnoxiously long struct names, only include the nested
        // keyword once (for root nests only).
        // Any deeply nested structs will evaluate to:
        //   {Root}Nested{each level's nest name concat'd}
        let region_descriptor = if is_root_nest {
            "Nested"
        } else {
            ""
        };
        let suffix = AsUpperCamelCase(field_name.to_string());

        format_ident!("{origin_ident}{region_descriptor}{suffix}")
    }
    /// `origin_ident` is the ident of the source data struct that this nest receives data from.
    /// It is used to form the base struct name isn't explicitly provided
    pub fn struct_name_default(&self, origin_ident: &Ident) -> Ident {
        // let origin_ident = self.origin(root_ident);
        let field_name = self.field_name();
        Self::build_default_struct_name(origin_ident, &field_name, self.is_root_nest())
    }
    /// `root_ident` is the ident of the top-level data struct containing derive(Wrap).
    /// It is used to form the base struct name when an origin isn't explicitly provided
    pub fn struct_name(&self, origin_ident: &Ident) -> Ident {
        match &self.rename {
            Some(name) => name.clone(),
            None => self.struct_name_default(origin_ident),
        }
    }
    pub fn optional(&self) -> bool {
        self.optional.is_present()
    }

    pub fn derive_to_nest_options_field_name(&self) -> Option<Ident> {
        self.derive_to_nest.as_ref().map(|derive_to_nest| {
            let field_name = self.field_name();
            derive_to_nest.options_field_name_or_default(&field_name)
        })
    }

    // scoped validation should have been done prior to any access, allow expect here
    pub fn resolve_field_type(&self) -> &Path {
        if let Some(field_type) = self.field_type.as_ref() {
            field_type
        } else {
            &self.derive_to_nest
                .as_ref()
                .expect_or_abort("Validated field_type XOR derive_transform(value)")
                .value
        }
    }

    fn validate(&self, nest_span: Span) -> usize {
        let mut errs = 0;

        if self.id.is_empty() {
            // emit_error!(self.id, "Nest ID cannot be empty");
            // emit_error!(self.id.to_token_stream(), "Nest ID cannot be empty");
            emit_error!(self.id.span(), "Nest ID cannot be empty");
            errs += 1;
        }
        if let Some(chain_from) = &self.chain_from && chain_from.as_str() == self.id.as_str() {
            emit_error!(chain_from.span(), "Nest cannot be chained from itself");
            errs += 1;
        }
        if let Some(field_type) = &self.field_type && let Some(derive_to_nest) = &self.derive_to_nest {
            emit_error!(derive_to_nest.span(), "`derive_to_nest` defined here");
            emit_error!(field_type, "`field_type` cannot be used with `derive_to_nest`");
            errs += 1;
        }
        if self.field_type.is_none() && self.derive_to_nest.is_none() {
            emit_error!(nest_span, "Either `field_type` or `derive_to_nest` must be configured");
            errs += 1;
        }

        errs
    }
}

// ! Nest auto-transform

/// Configuration for automatically deriving `TransformToNest`/`TryTransformToNest`.
#[derive(Debug, Clone, FromMeta)]
pub struct DeriveToNest {
    /// Sets the resulting value type associated with the genetated fields in
    /// this nest.  This type can be reused in other `shrinkwrap::Wrap` impl'd
    /// structs (and even in other nest under the same wrapper - typically only
    /// done in cases of deep nesting).
    ///
    /// Type must implement `NestValueType`.
    pub value: Path,

    /// Only compatible with `optional` nests. Defaults to `"with_"` + nest `field_name`
    /// attr (as `snake_case`) if unset and nest is optional.
    ///
    /// Allows implementor to retain control of conditional nest rendering when
    ///  using `derive_transform`.
    ///
    /// Should be set to the name of a bool field provided by the struct
    /// implementing the `Transform:::Options` associated type.
    /// The derived transform impl will skip rendering if this field if set to `false`.
    pub options_field: Option<Ident>,
}
impl DeriveToNest {
    pub fn options_field_name_or_default(&self, field_name: &Ident) -> Ident {
        if let Some(options_name) = self.options_field.clone() {
            options_name
        } else {
            self.options_field_name_default(field_name)
        }
    }
    fn options_field_name_default(&self, field_name: &Ident) -> Ident {
        format_ident!("with_{field_name}")
    }
}

// !- Helper types

// !- Filter for nest IDs

/// Nest id list alias for darling/syn from derive
pub(crate) type NestIdSelection = Vec<LitStr>;

// ! Filter for type of derived struct

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) enum StructClass {
    Wrapper,
    Nest,
    Extra,
}
impl StructClass {
    pub(crate) fn key(&self) -> String {
        match self {
            Self::Wrapper => "wrapper",
            Self::Nest => "nest",
            Self::Extra => "extra",
        }
        .into()
    }
}
impl TryFrom<&syn::Path> for StructClass {
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
        Err(darling::Error::custom(
            "Invalid class type specified. Valid types: [wrapper, extra, nest]",
        )
        .with_span(&value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StructClassSelection(HashSet<StructClass>);

impl Default for StructClassSelection {
    fn default() -> Self {
        let mut set = HashSet::new();
        set.insert(StructClass::Wrapper);
        set.insert(StructClass::Extra);
        set.insert(StructClass::Nest);
        Self(set)
    }
}

impl StructClassSelection {
    pub(crate) fn contains(&self, class: StructClass) -> bool {
        self.0.contains(&class)
    }
    pub(crate) fn parse_input(meta: &syn::Meta) -> darling::Result<Option<SpannedValue<Self>>> {
        let pathlist = PathList::from_meta(meta)?;
        let span = meta.span();
        Self::try_from(pathlist).map(|selection| Some(SpannedValue::new(selection, span)))
    }
}
impl TryFrom<PathList> for StructClassSelection {
    type Error = darling::Error;

    fn try_from(paths: PathList) -> Result<Self, Self::Error> {
        let mut set = HashSet::new();
        for path in paths.iter() {
            let class_type = StructClass::try_from(path)?;
            if set.contains(&class_type) {
                let msg = format!("Class type defined multiple times: {}", class_type.key());
                return Err(darling::Error::custom(&msg).with_span(&path));
            }
            set.insert(class_type);
        }
        Ok(Self(set))
    }
}

// !- Attribute passthrough

/// Receives tokens in the form of `attr(serde(rename_all="snake_case"))`
fn extract_passthrough_attr_meta(meta: &Meta) -> Attribute {
    match meta.require_list() {
        Ok(list) => {
            if let Ok(path) = list.path.require_ident() && path == "attr" {
                let inner_attr = &list.tokens;
                parse_quote!(#[#inner_attr])
            } else {
                abort!(
                    list.path.span(),
                    format!(
                        "Unexpected key for passthrough attributes `attr` group. Expected `attr`"
                    )
                )
            }
        },
        Err(error) => {
            abort!(
                meta.span(),
                format!(
                    "Unexpected attr meta type. Expected a list `(that,looks,like,this).\nOriginal error: {error}`"
                )
            );
        }
    }
}

// !- Struct attributes

/// attribute passthrough opts for structs
#[derive(Debug, Clone, FromMeta)]
pub(crate) struct StructProxyAttribute {
    pub attr: Meta,

    #[darling(default)]
    pub limit: StructRestriction,
}
impl StructProxyAttribute {
    pub(crate) fn maybe_from_attribute(attr: &Attribute) -> Option<Self> {
        let forward_ident = format_ident!("{FORWARD_ATTR}");
        if attr.path().get_ident() == Some(&forward_ident) {
            let proxy_attr = match Self::from_meta(&attr.meta) {
                Ok(proxy) => proxy,
                Err(error) => abort!(error.span(), error),
            };
            let limit = &proxy_attr.limit;
            if limit.origin.is_present() && let Some(nests) = &limit.nests {
                emit_error!(nests.span(), "Conflicting `nests` attribute defined here");
                abort!(limit.origin.span(), "`nests` and `origin` cannot be set simultaneously");
            }
            if limit.origin.is_present() && let Some(class) = &limit.class && class.contains(StructClass::Nest) {
                emit_error!(class.span(), "Conflicting `class` attribute defined here. Option 1) remove `nest` from `class` list.");
                abort!(limit.origin.span(), "`class(nest)` and `origin` cannot be set simultaneously. Option 2) remove the `origin` flag.");
            }

            Some(proxy_attr)
        } else {
            None
        }
    }
    pub(crate) fn maybe_extract_from(attr: &Attribute) -> Option<ExtractedStructAttribute> {
        Self::maybe_from_attribute(attr).map(ExtractedStructAttribute::from)
    }
}

/// Filter derived struct selection by nests/origin and/or struct type (wrapper, nest, extra)
#[derive(Debug, Clone, Default, FromMeta)]
pub(crate) struct StructRestriction {
    /// List of nest IDs to restrict assignment to.
    ///
    /// Incompatible with `origin` flag.
    pub nests: Option<SpannedValue<NestIdSelection>>,

    /// Restrict assignment to the structs generated for the primary/derive struct (wrapper/extra).
    ///
    /// If the `class` restriction list is provided, it cannot contain `nest`.
    pub origin: Flag,

    /// Type of generated structs to restrict assignment to.
    ///
    /// If the `origin` restriction flag is provided, `class` cannot contain `nest`
    #[darling(default, with=StructClassSelection::parse_input)]
    pub class: Option<SpannedValue<StructClassSelection>>,
}

// restriction by nest ids or origin flag
#[derive(Debug, Clone)]
pub(crate) enum StructAttributeOriginRestriction {
    Origin,
    Nests(HashSet<String>),
}

// TODO: custom debug impl - relocate to StructAttrResolver?
#[derive(Debug, Clone)]
pub(crate) struct ExtractedStructAttribute {
    pub attr: Attribute,

    // pub nests: Option<HashSet<String>>,
    pub sources: Option<StructAttributeOriginRestriction>,

    pub classes: StructClassSelection,
}
impl ExtractedStructAttribute {
    pub(crate) fn get_origin_attrs(&self, class: StructClass) -> Option<&Attribute> {
        if !self.classes.contains(class) {
            None
        } else {
            match &self.sources {
                None | Some(StructAttributeOriginRestriction::Origin) => Some(&self.attr),
                Some(StructAttributeOriginRestriction::Nests(..)) => None,
            }
        }
    }
    pub(crate) fn get_nest_attrs(&self, class: StructClass, nest_id: &str) -> Option<&Attribute> {
        if !self.classes.contains(class) {
            None
        } else {
            match &self.sources {
                None => Some(&self.attr),
                Some(StructAttributeOriginRestriction::Nests(nest_ids)) => nest_ids.contains(nest_id).then_some(&self.attr),
                Some(StructAttributeOriginRestriction::Origin) => None,
            }
        }
    }
}
impl From<StructProxyAttribute> for ExtractedStructAttribute {
    fn from(proxy_attr: StructProxyAttribute) -> Self {
        let limit = proxy_attr.limit;
        let sources = if let Some(ids) = limit.nests {
            let nest_ids = ids.into_inner().into_iter().map(|id| id.value()).collect();
            Some(StructAttributeOriginRestriction::Nests(nest_ids))
        } else if limit.origin.is_present() {
            Some(StructAttributeOriginRestriction::Origin)
        } else {
            None
        };

        Self {
            attr: extract_passthrough_attr_meta(&proxy_attr.attr),
            sources,
            classes: limit.class.unwrap_or_default().into_inner(),
        }
    }
}

// ! Field attributes

#[derive(Debug, Clone, FromMeta)]
pub(crate) struct FieldProxyAttribute {
    pub attr: Meta,

    #[darling(default)]
    pub limit: SpannedValue<FieldAttrRestriction>,
}
impl FieldProxyAttribute {
    pub(crate) fn maybe_from_attribute(attr: &Attribute) -> Option<Self> {
        let forward_ident = format_ident!("{FORWARD_ATTR}");

        if attr.path().get_ident() == Some(&forward_ident) {
             match Self::from_meta(&attr.meta) {
                Ok(proxy) => Some(proxy),
                Err(error) => abort!(error.span(), error),
            }
        } else {
            None
        }
    }
    pub(crate) fn maybe_extract_from(attr: &Attribute) -> Option<ExtractedFieldAttribute> {
        Self::maybe_from_attribute(attr).map(ExtractedFieldAttribute::from)
    }
}

#[derive(Debug, Clone, Default, FromMeta)]
pub(crate) struct FieldAttrRestriction {
    /// list of nest IDs
    pub nests: Option<NestIdSelection>,
}

// TODO: custom debug impl
#[derive(Debug, Clone)]
pub(crate) struct ExtractedFieldAttribute {
    pub attr: Attribute,

    pub nests: Option<HashSet<String>>,
}
impl ExtractedFieldAttribute {
    pub(crate) fn get(&self, nest_id: &str) -> Option<&Attribute> {
        match &self.nests {
            None => Some(&self.attr),
            Some(ids) => ids.contains(nest_id).then_some(&self.attr)
        }
    }
}
impl From<FieldProxyAttribute> for ExtractedFieldAttribute {
    fn from(proxy_attr: FieldProxyAttribute) -> Self {
        let nests = proxy_attr.limit.into_inner().nests.map(|ids| ids.into_iter().map(|id| id.value()).collect());
        Self {
            attr: extract_passthrough_attr_meta(&proxy_attr.attr),
            nests,
        }
    }
}
