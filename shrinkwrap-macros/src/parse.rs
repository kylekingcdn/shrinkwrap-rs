use darling::util::SpannedValue;
use proc_macro_error2::{OptionExt, abort, emit_error};
use proc_macro2::Span;
use std::collections::{HashSet, HashMap};
use syn::{Attribute, Ident, Type};

pub mod types;
use types::{
    DeriveItemFieldOpts,
    ExtractedFieldAttribute,
    ExtractedStructAttribute,
    FieldProxyAttribute,
    NestOpts,
    StructClass,
    StructProxyAttribute,
};

// !- Struct attribute resolver

#[derive(Debug, Clone, Default)]
pub(crate) struct StructAttrResolver {
    /// Attributes with further nest ID + class filtering
    pub attrs: Vec<ExtractedStructAttribute>,
}
impl StructAttrResolver {
    pub(crate) fn from_attrs(source_attrs: Vec<&Attribute>) -> Self {
        let mut attrs = Vec::new();
        for attr in source_attrs {
            if let Some(extracted) = StructProxyAttribute::maybe_extract_from(attr) {
                attrs.push(extracted);
            }
        }
        Self {
            attrs
        }
    }
    pub(crate) fn resolve(&self, nest_id: Option<&str>, class: StructClass) -> Vec<Attribute> {
        match nest_id {
            Some(id) => self.resolve_for_nest(id, class),
            None => self.resolve_for_origin(class),
        }
    }
    pub(crate) fn resolve_for_origin(&self, class: StructClass) -> Vec<Attribute> {
        let mut attrs = Vec::new();
        for attr in &self.attrs {
            if let Some(attr) = attr.get_origin_attrs(class) {
                attrs.push(attr.clone());
            }
        }
        attrs
    }
    pub(crate) fn resolve_for_nest(&self, nest_id: &str, class: StructClass) -> Vec<Attribute> {
        let mut attrs = Vec::new();
        for attr in &self.attrs {
            if let Some(attr) = attr.get_nest_attrs(class, nest_id) {
                attrs.push(attr.clone());
            }
        }
        attrs
    }
}

// !- Struct field resolver

#[derive(Debug, Clone, Default)]
pub(crate) struct FieldResolver {
    /// Origin fields, stored for reference
    origin_fields: Vec<Ident>,

    /// Field name ident -> field data
    field_map: HashMap<Ident, ParsedField>,

    /// Nest ID -> field name ident
    nest_fields: HashMap<String, Vec<Ident>>,
}
impl FieldResolver {
    pub(crate) fn new(fields: Vec<ParsedField>) -> Self {
        let mut resolver = Self {
            origin_fields: Vec::with_capacity(fields.len()),
            field_map: HashMap::with_capacity(fields.len()),
            nest_fields: HashMap::with_capacity(5),
        };
        for field in fields {
            resolver.insert_field(field);
        }
        resolver
    }

    pub(crate) fn from_opt_fields(field_opts: Vec<DeriveItemFieldOpts>) -> Self {
        let mut fields = Vec::new();
        for field in field_opts {
            let mut attrs = Vec::new();
            for field_attr in &field.attrs {
                if let Some(extracted) = FieldProxyAttribute::maybe_extract_from(field_attr) {
                    attrs.push(extracted);
                }
            }
            let parsed_field = ParsedField {
                name: field.ident.unwrap_or_else(|| abort!(Span::call_site(), "Only named structs are supported")),
                ty: field.ty,
                nest_ids: field.nests.iter().map(|id| id.value()).collect(),
                attrs,
            };
            fields.push(parsed_field);
        }
        Self::new(fields)
    }

    pub(crate) fn insert_field(&mut self, field: ParsedField) {
        self.field_map.insert(field.name.clone(), field.clone());
        for nest_id in &field.nest_ids {
            self.nest_fields.entry(nest_id.clone()).or_default().push(field.name.clone());
        }
        self.origin_fields.push(field.name.clone());
    }
    /// Checks that a parent nests' fields are a superset of the child fields
    pub(crate) fn validate_parent_field_propagation(&self, nest_hierarchy: &NestHierarchy) -> bool {
        let mut has_error = false;
        for (nest_id, field_idents) in &self.nest_fields {
            let nest_opts = nest_hierarchy.get_nest_opts(nest_id);
            if let Some(parent_id) = &nest_opts.chain_from {
                let parent_decl_span = parent_id.span();
                let parent_id = parent_id.clone().into_inner();
                let parent_fields: Vec<_> = self.nest_fields(&parent_id).into_iter().map(|field| &field.name).collect();

                for nest_field in field_idents {
                    if !parent_fields.contains(&nest_field) {
                        emit_error!(parent_decl_span, "Parent of `{}` nest configured here.", nest_id);
                        emit_error!(nest_field, "Parent nest `{}` does not include field `{}` required by child nest `{}`.", parent_id, nest_field, nest_id);
                        has_error = true;
                    }
                }
            }
        }

        !has_error
    }

    pub(crate) fn nest_fields(&self, nest_id: &str) -> Vec<&ParsedField> {
        self.nest_fields
        .get(nest_id)
        .cloned()
        .unwrap_or_default()
        .iter().map(|ident|
            self.field_map.get(ident).expect_or_abort(format!("field missing from field_map: {ident}").as_str())
        ).collect()
    }

    pub(crate) fn origin_fields(&self) -> Vec<&ParsedField> {
        self.origin_fields.iter().map(|ident|
            self.field_map.get(ident).expect_or_abort(format!("field missing from field_map: {ident}").as_str())
        ).collect()
    }

    /// Does not check if nest contains field, must be done first
    pub(crate) fn attrs(&self, nest_id: &str, field_ident: &Ident) -> Vec<Attribute> {
        self.field_map.get(field_ident).map(|field| {
            let mut attrs = Vec::new();
            for attr in &field.attrs {
                if let Some(attr) = attr.get(nest_id) {
                    attrs.push(attr.clone())
                }
            }
            attrs
        }).unwrap_or_default()
    }
}

// !- Nest hierarchy

/// Builds a tree of the nest hierarchy (parent->child id relationships)
///
/// Performs basic
#[derive(Debug, Clone, Default)]
pub(crate) struct NestHierarchy {
    /// Nest ID -> `NestOpts`
    nest_opts: HashMap<String, NestOpts>,

    /// Parent ID key uses option to handle root/top-level nests (which have no parent)
    parent_children: HashMap<Option<String>, Vec<String>>,

    /// Map of nest ID to (first) span occurence
    nest_span: HashMap<String, Span>,

    /// Map of parent ID to (first) span occurence
    parent_span: HashMap<String, Span>,
}
#[allow(dead_code)]
impl NestHierarchy {
    pub(crate) fn new() -> Self {
        Self::default()
    }
    pub(crate) fn from_nest_opts(nest_opts_list: Vec<SpannedValue<NestOpts>>) -> Self {
        let mut nest_hierarchy = Self::new();
        for nest_opts in nest_opts_list {
            nest_hierarchy.insert(nest_opts.into_inner())
        }
        nest_hierarchy.validate_post_insert();

        nest_hierarchy
    }

    pub(crate) fn all_nest_ids(&self) -> Vec<String> {
        self.nest_span.keys().cloned().collect()
    }
    pub(crate) fn all_root_nest_ids(&self) -> Vec<String> {
        self.parent_children.get(&None).cloned().unwrap_or_default()
    }
    pub(crate) fn all_spanned_nest_ids(&self) -> Vec<SpannedValue<String>> {
        self.nest_span.iter().map(|(id, span)| SpannedValue::new(id.clone(), *span)).collect()
    }

    pub(crate) fn get_nest_opts(&self, nest_id: &str) -> &NestOpts {
        self.nest_opts
            .get(nest_id)
            .expect_or_abort(format!("Internal macro error - nest_opts map missing ID: {nest_id}").as_str())
    }
    pub(crate) fn get_children(&self, parent_id: Option<&str>) -> &Vec<String> {
        let parent_id = parent_id.as_ref().map(|id| id.to_string());
        self.parent_children
            .get(&parent_id)
            .expect_or_abort(format!("Internal macro error - parent_children map missing ID: {}", parent_id.unwrap_or("[none]".to_string())).as_str())
    }
    pub(crate) fn get_nest_id_span(&self, nest_id: &str) -> Span {
        *self.nest_span
            .get(nest_id)
            .expect_or_abort(format!("Internal macro error - nest_span map missing ID: {nest_id}").as_str())
    }
    pub(crate) fn get_parent_id_span(&self, parent_id: &str) -> Span {
        *self.parent_span
            .get(parent_id)
            .expect_or_abort(format!("Internal macro error - parent_span map missing ID: {parent_id}").as_str())
    }

    fn insert(&mut self, opts: NestOpts) {
        // resolve plain id's
        let nest_id = opts.id.clone();
        let parent_id = opts.chain_from.clone();

        // validate insert, destructure nest id span/value
        self.validate_insert(nest_id.clone(), parent_id.clone());
        let (nest_id_span, nest_id) = (nest_id.span(), nest_id.into_inner());

        // insert NestOpts
        self.nest_opts.insert(opts.id.clone().into_inner(), opts);

        // add to parent_children map
        // push nest to parent's children list, establish nest as empty parent if unseen
        self.parent_children.entry(parent_id.clone().map(|id| id.into_inner())).or_default().push(nest_id.clone());
        { let _ = self.parent_children.entry(Some(nest_id.clone())).or_default(); } // add leaf nodes with empty vec

        // add to span maps
        self.nest_span.insert(nest_id, nest_id_span);
        if let Some(parent_id) = parent_id {
            // destructure parent id span/value
            let (parent_id_span, parent_id) = (parent_id.span(), parent_id.into_inner());
            self.parent_span.insert(parent_id, parent_id_span);
        }
    }
    fn validate_insert(&self, nest_id: SpannedValue<String>, parent_id: Option<SpannedValue<String>>) {
        let (nest_id_span, nest_id) = (nest_id.span(), nest_id.into_inner());

        if let Some(span) = self.nest_span.get(&nest_id) {
            emit_error!(
                span,
                format!("First nest with ID `{nest_id}` defined here")
            );
            abort!(
                &nest_id_span,
                format!("Multiple nests exist with ID: {nest_id}")
            );
        }

        // TODO: detect loops with more than 2 nodes
        // check for 1:1 loop
        if let Some(parent_id) = parent_id {
            let (parent_id_span, parent_id) = (parent_id.span(), parent_id.into_inner());
            if let Some(children) = self.parent_children.get(&Some(nest_id.clone())) && children.contains(&parent_id) {

                let nest1_id_span = self.get_nest_id_span(&parent_id);
                let nest1_parent_span = self.get_parent_id_span(&nest_id);
                let nest2_id_span = nest_id_span;
                let nest2_parent_span = parent_id_span;

                emit_error!(nest1_id_span, format!("`{nest_id}`'s parent is defined here"));
                emit_error!(nest1_parent_span, format!("`{nest_id}`'s parent (`{parent_id}`) also has a parent nest assigned, however it is conflicting as it cycles back to `{nest_id}`."));
                emit_error!(nest2_id_span, format!("{nest_id} is defined here"));
                emit_error!(nest2_parent_span, format!("The `{nest_id}` parent is assigned to `{parent_id}` here"));
                abort!(
                    &parent_id_span,
                    format!("Chained nest parent loop detected")
                );
            }
        }
    }
    pub(crate) fn validate_post_insert(&self) {
        let mut has_errors = false;
        // check for any parent IDs that don't have an associated nest defined
        for parent_id in self.parent_children.keys() {
            if let Some(parent_id) = parent_id.as_ref()
                && !self.nest_span.contains_key(parent_id)
                && let Some(parent_span) = self.parent_span.get(parent_id)
            {
                emit_error!(
                    parent_span,
                    format!("Nest with id `{parent_id}` does not exist, yet is referenced here")
                );

                has_errors = true;
            }
        }
        if has_errors {
            abort!(Span::call_site(), "Nest validation failed");
        }
    }
}

// !- Attribute extraction

#[derive(Debug, Clone)]
pub(crate) struct ParsedField {
    /// Field name
    pub name: Ident,

    /// Field type
    pub ty: Type,

    /// Attributes with further nest ID filtering
    pub attrs: Vec<ExtractedFieldAttribute>,

    /// Nest IDs which the field will be added to
    // FIXME: remove
    pub nest_ids: HashSet<String>,
}
