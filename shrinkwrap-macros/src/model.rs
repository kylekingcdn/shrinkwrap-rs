use proc_macro_error2::abort_call_site;
use proc_macro2::TokenStream;
use quote::ToTokens;
use std::rc::Rc;
use std::collections::HashMap;
use syn::{Attribute, Ident, Path, Type, parse_quote};

use crate::{
    generate::structs::{Derives, Doc, GenStruct, GenStructField, GenVisibility},
    parse::ParsedField,
};

// !- Primary model struct

#[derive(Debug, Clone)]
pub(crate) struct ModelTree {
    /// Origin model
    #[allow(dead_code)]
    pub origin: Rc<OriginData>,

    /// Generated wrapper for the origin struct
    pub origin_wrapper: Rc<Wrapper>,

    #[allow(dead_code)]
    pub parents: ParentRegistry,
}
impl ModelTree {
    pub(crate) fn new(origin_wrapper: Wrapper, origin_data: Rc<OriginData>) -> Self {
        let origin_wrapper_rc = Rc::new(origin_wrapper);
        if let DataVariant::Origin(origin) = origin_wrapper_rc.data.clone() {
            let parents = ParentRegistry::from_origin_wrapper(&origin_wrapper_rc, origin_data);

            Self {
                origin,
                origin_wrapper: origin_wrapper_rc,
                parents,
            }
        } else {
            abort_call_site!("ModelTree wrapper must wrap origin data");
        }
    }
}
impl RecursiveToTokens for ModelTree {
    fn recursive_to_tokens(&self, tokens: &mut TokenStream) {
        self.origin_wrapper.recursive_to_tokens(tokens);
    }
}

// !- Node parent store

/// Provides child->parent access
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub(crate) struct ParentRegistry {
    pub wrapper_parent: HashMap<Ident, WrapperParentVariant>,
    pub nest_parent: HashMap<Ident, NestDataParentVariant>,
    pub extra_parent: HashMap<Ident, Rc<Wrapper>>,
}
#[allow(dead_code)]
impl ParentRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }
    pub(crate) fn from_origin_wrapper(origin_wrapper: &Rc<Wrapper>, origin_data: Rc<OriginData>) -> Self {
        let mut registry = Self::default();
        registry.wrapper_parent.insert(origin_wrapper.ident.clone(), origin_data.clone().into());
        registry.scan_wrapper(origin_wrapper.clone(), WrapperParentVariant::Origin(origin_data));
        registry
    }
    fn scan_wrapper(&mut self, wrapper: Rc<Wrapper>, parent: WrapperParentVariant) {
        self.wrapper_parent.insert(wrapper.ident.clone(), parent);
        self.scan_extra(wrapper.extra.clone(), wrapper.clone());
        if let DataVariant::Nest(nest_data) = &wrapper.data {
            self.scan_nest(nest_data.clone(), wrapper.into());
        }
    }
    fn scan_extra(&mut self, extra: Rc<Extra>, parent: Rc<Wrapper>) {
        self.extra_parent.insert(extra.ident.clone(), parent.clone());

        for field in &extra.fields {
            match field.object.clone() {
                ExtraChildVariant::Nest(nest_data) => {
                    self.scan_nest(nest_data.clone(), extra.clone().into());
                },
                ExtraChildVariant::Wrapper(wrapper) => {
                    self.scan_wrapper(wrapper, extra.clone().into());
                }
            }
        }
    }
    fn scan_nest(&mut self, nest: Rc<NestData>, parent: NestDataParentVariant) {
        self.nest_parent.insert(nest.ident.clone(), parent.clone());
    }

    pub(crate) fn get_wrapper_parent(&self, wrapper_ident: &Ident) -> Option<WrapperParentVariant> {
        self.wrapper_parent.get(wrapper_ident).cloned()
    }
    pub(crate) fn get_nest_parent(&self, nest_ident: &Ident) -> Option<NestDataParentVariant> {
        self.nest_parent.get(nest_ident).cloned()
    }
    pub(crate) fn get_extra_parent(&self, extra_ident: &Ident) -> Option<Rc<Wrapper>> {
        self.extra_parent.get(extra_ident).cloned()
    }
}

// !- Recursive ToTokens trait

pub(crate) trait RecursiveToTokens {
    fn recursive_to_tokens(&self, tokens: &mut TokenStream);
}

// !- Origin data

/// Either origin data or nest data
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct OriginData {
    /// Name of the origin struct
    pub ident: Ident,

    /// All origin fields
    pub fields: Vec<OriginDataField>,
}
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct OriginDataField {
    /// The field name
    pub name: Ident,

    /// The field's full type
    pub ty: Type,
}
impl From<&ParsedField> for OriginDataField {
    fn from(field: &ParsedField) -> Self {
        Self {
            name: field.name.clone(),
            ty: field.ty.clone(),
        }
    }
}

// !- Wrapper

#[derive(Debug, Clone)]
pub(crate) struct Wrapper {
    // /// None for root, otherwise must be an `Extra` struct
    // pub parent: WrapperParentVariant,

    /// Common struct name
    pub ident: Ident,

    /// List of additional derive attrs to include
    pub derives: Derives,

    /// List of custom attributes to apply to the wrapper struct itself
    pub attrs: Vec<Attribute>,

    /// Struct-level docs
    pub doc: Doc,

    /// The name of the field providing the data struct
    pub data_name: Ident,
    /// Field-level docs for the data field
    pub data_doc: Doc,
    /// Flag for data flattening. If enabled, #[serde(flatten)] will be added to
    /// the data field's attributes
    pub data_flatten: bool,
    /// The data object
    pub data: DataVariant,

    /// The name of the field providing the extra struct
    pub extra_name: Ident,
    /// Field-level docs for the extra field
    pub extra_doc: Doc,
    /// The extra object
    pub extra: Rc<Extra>,
}
impl ToTokens for Wrapper {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        GenStruct::from(self).to_tokens(tokens);
    }
}
impl RecursiveToTokens for Wrapper {
    fn recursive_to_tokens(&self, tokens: &mut TokenStream) {
        // write self struct definition
        self.to_tokens(tokens);

        // recurse through children
        self.data.recursive_to_tokens(tokens); // branch only continues for nest, not origin
        self.extra.recursive_to_tokens(tokens);
    }
}
/// The possible parent types for a Wrapper struct
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) enum WrapperParentVariant {
    Origin(Rc<OriginData>),
    Extra(Rc<Extra>),
}
impl From<Rc<OriginData>> for WrapperParentVariant {
    fn from(parent: Rc<OriginData>) -> Self {
        Self::Origin(parent)
    }
}
impl From<Rc<Extra>> for WrapperParentVariant {
    fn from(parent: Rc<Extra>) -> Self {
        Self::Extra(parent)
    }
}

// !- Nest data

/// Either origin data or nest data
#[derive(Debug, Clone)]
pub(crate) struct NestData {
    /// Nest ID
    pub id: String,

    /// Nest struct name / ident
    pub ident: Ident,

    /// List of additional derive attrs to include
    pub derives: Derives,

    /// List of attributes to apply to the nest struct
    pub attrs: Vec<Attribute>,

    /// Struct-level docs
    pub doc: Doc,

    /// Nest fields
    pub fields: Vec<NestDataField>,

    /// Info pertaining to auto-derivation of `TransformToNest` via `build_nest_value`
    pub derive_to_nest: Option<NestAutoDeriveToNest>,
}
impl NestData {
    pub(crate) fn source_types(&self) -> Vec<Type> {
        let mut tys = Vec::new();
        for field in &self.fields {
            if !tys.contains(&field.source_type) {
                tys.push(field.source_type.clone())
            }
        }
        tys
    }
}
impl ToTokens for NestData {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        GenStruct::from(self).to_tokens(tokens);
    }
}
impl RecursiveToTokens for NestData {
    fn recursive_to_tokens(&self, tokens: &mut TokenStream) {
        // only generate self - no child structs
        self.to_tokens(tokens);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NestAutoDeriveToNest {
    pub(crate) options_field_if_optional: Option<Ident>,
    pub(crate) nest_group: Path,
    pub(crate) nest_value: Path,
}

#[derive(Debug, Clone)]
pub(crate) struct NestDataField {
    /// The field name
    pub name: Ident,

    /// The field's full type
    pub ty: Path,

    /// The fields source type
    pub source_type: Type,

    /// List of custom attributes to apply to the field (field docs handled here
    /// as opposed to a dedicated attr type)
    pub attrs: Vec<Attribute>,
}

/// The possible struct types which may contain a nest data struct as a field
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) enum NestDataParentVariant {
    Wrapper(Rc<Wrapper>),
    Extra(Rc<Extra>),
}
impl NestDataParentVariant {
    #[allow(dead_code)]
    pub(crate) fn ident(&self) -> &Ident {
        match self {
            Self::Wrapper(w) => &w.ident,
            Self::Extra(e) => &e.ident,
        }
    }
}
impl From<Rc<Wrapper>> for NestDataParentVariant {
    fn from(parent: Rc<Wrapper>) -> Self {
        Self::Wrapper(parent)
    }
}
impl From<Rc<Extra>> for NestDataParentVariant {
    fn from(parent: Rc<Extra>) -> Self {
        Self::Extra(parent)
    }
}

// ! Data variants

/// The possible types that can occupy the 'data' field in a [`Wrapper`] struct
#[derive(Debug, Clone)]
pub(crate) enum DataVariant {
    /// The user-defined/source struct
    Origin(Rc<OriginData>),

    /// A nest (variant group) generated for the provided origin nest (or sub-nest)
    Nest(Rc<NestData>),
}
impl DataVariant {
    /// Returns some for nests, none for origin
    pub(crate) fn nest_id(&self) -> Option<&str> {
        match self {
            Self::Origin(..) => None,
            Self::Nest(nest_data) => Some(nest_data.id.as_str())
        }
    }
    pub(crate) fn ident(&self) -> &Ident {
        match self {
            Self::Origin(o) => &o.ident,
            Self::Nest(n) => &n.ident,
        }
    }
    #[allow(dead_code)]
    pub(crate) fn is_origin(&self) -> bool {
        match self {
            Self::Origin(..) => true,
            Self::Nest(..) => false,
        }
    }
}
impl From<Rc<OriginData>> for DataVariant {
    fn from(parent: Rc<OriginData>) -> Self {
        Self::Origin(parent)
    }
}
impl From<Rc<NestData>> for DataVariant {
    fn from(parent: Rc<NestData>) -> Self {
        Self::Nest(parent)
    }
}
impl RecursiveToTokens for DataVariant {
    fn recursive_to_tokens(&self, tokens: &mut TokenStream) {
        if let Self::Nest(nest) = self {
            nest.recursive_to_tokens(tokens);
        }
    }
}

// !- Extra

/// The `extra` struct, provides all configured nests for it's [`Data`] sibling
#[derive(Debug, Clone)]
pub(crate) struct Extra {
    /// Common struct name
    pub ident: Ident,

    /// List of additional derive attrs to apply to the struct itself
    pub derives: Derives,

    /// List of custom attributes to apply to the struct itself
    pub attrs: Vec<Attribute>,

    /// Struct-level rust docs
    pub doc: Doc,

    /// Extra struct fields - each will be either `NestData` or a `Wrapper` (for sub-nests)
    pub fields: Vec<ExtraField>,
}
impl ToTokens for Extra {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        GenStruct::from(self).to_tokens(tokens);
    }
}
impl RecursiveToTokens for Extra {
    fn recursive_to_tokens(&self, tokens: &mut TokenStream) {
        // write self struct definition
        self.to_tokens(tokens);
        // recurse through children
        for field in &self.fields {
            field.object.recursive_to_tokens(tokens);
        }
    }
}

/// A field within an [`Extra`] struct
#[derive(Debug, Clone)]
pub(crate) struct ExtraField {
    /// Name of the field
    pub name: Ident,

    /// The underling object for the field
    pub object: ExtraChildVariant,

    /// Whether or not this field is optional
    pub optional: bool,
}
impl ExtraField {
    pub(crate) fn ty(&self) -> Path {
        let ident = self.object.ident();
        if self.optional {
            parse_quote!(Option<#ident>)
        } else {
            parse_quote!(#ident)
        }
    }
}

/// The possible struct types which may occupy an [`Extra`] struct's fields
#[derive(Debug, Clone)]
pub(crate) enum ExtraChildVariant {
    Wrapper(Rc<Wrapper>),
    Nest(Rc<NestData>),
}
impl ExtraChildVariant {
    pub(crate) fn ident(&self) -> &Ident {
        match self {
            Self::Wrapper(w) => &w.ident,
            Self::Nest(n) => &n.ident,
        }
    }
}
impl From<Rc<Wrapper>> for ExtraChildVariant {
    fn from(parent: Rc<Wrapper>) -> Self {
        Self::Wrapper(parent)
    }
}
impl From<Rc<NestData>> for ExtraChildVariant {
    fn from(parent: Rc<NestData>) -> Self {
        Self::Nest(parent)
    }
}
impl RecursiveToTokens for ExtraChildVariant {
    fn recursive_to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Wrapper(w) => w.recursive_to_tokens(tokens),
            Self::Nest(n) => n.recursive_to_tokens(tokens),
        }
    }
}

// !- Gen struct conversion impls

impl From<&Wrapper> for GenStruct {
    fn from(source: &Wrapper) -> Self {
        let ident = source.ident.clone();
        let data_ident = source.data.ident();
        let extra_ident = source.extra.ident.clone();

        let extra_field = GenStructField {
            vis: GenVisibility::Public,
            name: source.extra_name.clone(),
            ty: parse_quote!(#extra_ident),
            attrs: Vec::new(),
            doc: source.extra_doc.clone(),
        };

        // if flatten is enabled, add #[serde(flatten)] to data field
        let data_attrs = if source.data_flatten {
            vec![parse_quote!(#[serde(flatten)])]
        } else {
            Vec::new()
        };
        let data_field = GenStructField {
            vis: GenVisibility::Public,
            name: source.data_name.clone(),
            ty: parse_quote!(#data_ident),
            attrs: data_attrs,
            doc: source.data_doc.clone(),
        };
        let fields = vec![
            extra_field,
            data_field,
        ];
        Self {
            vis: GenVisibility::Public,
            ty: parse_quote!(#ident),
            derives: source.derives.clone(),
            attrs: source.attrs.clone(),
            doc: source.doc.clone(),
            fields,
        }
    }
}

impl From<&NestData> for GenStruct {
    fn from(source: &NestData) -> Self {
        let ident = source.ident.clone();
        let fields = source.fields.iter().map(GenStructField::from).collect::<Vec<_>>();

        Self {
            vis: GenVisibility::Public,
            ty: parse_quote!(#ident),
            derives: source.derives.clone(),
            attrs: source.attrs.clone(),
            doc: source.doc.clone(),
            fields,
        }
    }
}
impl From<&NestDataField> for GenStructField {
    fn from(source: &NestDataField) -> Self {
        Self {
            vis: GenVisibility::Public,
            name: source.name.clone(),
            ty: source.ty.clone(),
            attrs: source.attrs.clone(),
            doc: Doc::default(),
        }
    }
}

impl From<&Extra> for GenStruct {
    fn from(source: &Extra) -> Self {
        let ident = source.ident.clone();
        let fields = source.fields.iter().map(GenStructField::from).collect();

        Self {
            vis: GenVisibility::Public,
            ty: parse_quote!(#ident),
            derives: source.derives.clone(),
            attrs: source.attrs.clone(),
            doc: source.doc.clone(),
            fields,
        }
    }
}
impl From<&ExtraField> for GenStructField {
    fn from(source: &ExtraField) -> Self {
        Self {
            vis: GenVisibility::Public,
            name: source.name.clone(),
            ty: source.ty(),
            attrs: Vec::default(),
            doc: Doc::default(),
        }
    }
}
