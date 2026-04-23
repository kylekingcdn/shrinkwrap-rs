#![doc = "Helper types used to associate related types"]
#![allow(dead_code)]

use darling::util::{Flag, SpannedValue};
use proc_macro_error2::{abort, emit_error};
use proc_macro2::TokenStream;
use std::{collections::HashMap};
use syn::{Ident, LitStr, Path};

use crate::parse::types::NestOpts;

/// Wrapper around [`NestOpts`]
///
/// Used to store associated info, e.g. nest (and associated class) struct attrs
#[derive(Debug, Clone)]
pub struct NestInfo {
    pub ident: Ident,

    pub opts: NestOpts,

    pub struct_attrs: NestStructAttrInfo,

    /// maps field names to field data for a given nest
    pub fields: HashMap<Ident, NestField>,

    pub transform_gen: Option<NestTransformGeneration>,
}
impl NestInfo {
    pub fn new(ident: Ident, nest_opts: NestOpts, transform_gen: Option<NestTransformGeneration>) -> Self {
        Self {
            ident,
            opts: nest_opts,
            struct_attrs: NestStructAttrInfo::default(),
            fields: HashMap::new(),
            transform_gen,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NestTransformGeneration {
    pub nest_group: Path,
    pub value_type: Path,

    // Set if nest is optional
    pub options_filter_field: Option<Ident>,
}
impl NestTransformGeneration {
    pub fn new(nest_group: Path, value_type: Path, options_filter_field: Option<Ident>) -> Self {
        Self {
            nest_group,
            value_type,
            options_filter_field,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct NestStructAttrInfo {
    wrapper: Vec<TokenStream>,
    extra: Vec<TokenStream>,
    nest: Vec<TokenStream>,
}
impl NestStructAttrInfo {
    pub fn add_wrapper_attr(&mut self, attr: TokenStream) {
        self.wrapper.push(attr);
    }
    pub fn add_extra_attr(&mut self, attr: TokenStream) {
        self.extra.push(attr);
    }
    pub fn add_nest_attr(&mut self, attr: TokenStream) {
        self.nest.push(attr);
    }
    pub fn wrapper(&self) -> &Vec<TokenStream> {
        &self.wrapper
    }
    pub fn extra(&self) -> &Vec<TokenStream> {
        &self.extra
    }
    pub fn nest(&self) -> &Vec<TokenStream> {
        &self.nest
    }
}

#[derive(Debug, Clone)]
pub struct NestRepo {
    nest_map: HashMap<Ident, NestInfo>,

    // utility maps that provide the idents as results. Idents are then resolved to the nestopts via the `nest_map`
    origin_children_map: HashMap<Ident, Vec<Ident>>, // provides are child nests for a given origin
    nest_parent_map: HashMap<Ident, Ident>, // provides the parent of each nest, (root/true origin is excluded)
    nest_id_map: HashMap<String, Ident>,
    nest_id_span_map: HashMap<String, SpannedValue<String>>,

    root_ident: Ident,

    global_optional: Flag,
}
impl NestRepo {
    pub fn new(root_ident: Ident, global_optional: Flag) -> Self {
        Self {
            root_ident,
            nest_map: HashMap::new(),
            origin_children_map: HashMap::new(),
            nest_parent_map: HashMap::new(),
            nest_id_map: HashMap::new(),
            nest_id_span_map: HashMap::new(),
            global_optional,
        }
    }

    pub fn insert(&mut self, mut opts: NestOpts) {
        // fallback to global all optional
        // (don't overwrite if optional is present on nest - avoids span redirection)
        if !opts.optional.is_present() && self.global_optional.is_present() {
            opts.optional = self.global_optional;
        }

        // validate insert
        let id_str = opts.id.as_ref();
        if self.id_exists(id_str) {
            if let Some(spanned_id) = self.get_id_spanned(id_str) {
                emit_error!(
                    spanned_id.span(),
                    format!("First nest with ID `{id_str}` defined here")
                );
            }
            abort!(
                &opts.id.span(),
                format!("Multiple nests exist with ID: {id_str}")
            );
        }

        let nest_ident = opts.struct_name(&self.root_ident);
        if let Some(existing_info) = self.get_by_ident(&nest_ident) {
            emit_error!(
                &existing_info.opts.struct_name_span(),
                format!("First nest with ident `{nest_ident}` defined here")
            );
            abort!(
                &opts.id.span(),
                format!("Multiple nests exist with ident: {nest_ident}")
            );
        }

        // handle derive_transform
        if let Some(field_type) = opts.field_type.as_ref() && let Some(derive_transform) = opts.derive_transform.as_ref() {
            emit_error!(
                &field_type,
                format!("field_type defined here")
            );
            emit_error!(
                &derive_transform.span(),
                format!("derive_transform defined here")
            );
            abort!(&derive_transform.span(), "Nest cannot have both `field_type` and `derive_transform` configured simultaneously");
        }
        else if opts.field_type.is_none() && opts.derive_transform.is_none() {
            abort!(opts.id.span(), "Nest must have either `field_type` or `derive_transform` configured");
        }

        // validate options_field set only for optional nests
        #[allow(clippy::collapsible_if)]
        if let Some(derive_transform) = opts.derive_transform.as_ref() {
            if let Some(options_field) = derive_transform.options_field.as_ref() && !opts.optional() {
                abort!(
                    options_field,
                    format!("options_field not supported for non-optional nests")
                );
            }
        }

        let origin = opts.origin(&self.root_ident).to_owned();

        self.nest_parent_map
            .insert(nest_ident.clone(), origin.clone());
        self.nest_id_map.insert(id_str.clone(), nest_ident.clone());
        self.nest_id_span_map
            .insert(id_str.clone(), opts.id.clone());
        self.origin_children_map
            .entry(origin)
            .and_modify(|children| children.push(nest_ident.clone()))
            .or_insert(vec![nest_ident.clone()]);

        let transform_gen = opts.derive_transform.as_ref().map(|derive_transform| {
            let options_field = if opts.optional.is_present() {
                Some(derive_transform.options_field_name_or_default())
            } else {
                None
            };
            let derive_transform = derive_transform.clone().into_inner();
            NestTransformGeneration::new(derive_transform.nest, derive_transform.value, options_field)
        });

        let nest_info = NestInfo::new(nest_ident.clone(), opts, transform_gen);
        eprintln!("nest_info built: {nest_info:#?}");
        self.nest_map
            .insert(nest_ident, nest_info);

    }

    pub fn count(&self) -> usize {
        self.nest_map.values().count()
    }
    pub fn root_ident(&self) -> &Ident {
        &self.root_ident
    }

    pub fn get_by_ident(&self, nest_ident: &Ident) -> Option<&NestInfo> {
        self.nest_map.get(nest_ident)
    }
    pub fn get_by_ident_mut(&mut self, nest_ident: &Ident) -> Option<&mut NestInfo> {
        self.nest_map.get_mut(nest_ident)
    }
    pub fn get_by_id(&self, nest_id: &str) -> Option<&NestInfo> {
        self.nest_id_map
            .get(nest_id)
            .and_then(|ident| self.get_by_ident(ident))
    }
    pub fn get_by_id_mut(&mut self, nest_id: &str) -> Option<&mut NestInfo> {
        let ident = &self.nest_id_map.get(nest_id).clone();
        match ident {
            Some(i) => self.nest_map.get_mut(i),
            None => None,
        }
    }

    pub fn get_id_spanned(&self, nest_id: &str) -> Option<&SpannedValue<String>> {
        self.nest_id_span_map.get(nest_id)
    }
    pub fn get_children_by_origin_ident(&self, nest_ident: &Ident) -> Vec<&NestInfo> {
        let mut matching = Vec::new();

        if let Some(children_idents) = self.origin_children_map.get(nest_ident) {
            for child_ident in children_idents {
                if let Some(child_opts) = self.get_by_ident(child_ident) {
                    matching.push(child_opts);
                }
            }
        }
        matching
    }
    pub fn get_parent_ident(&self, nest_ident: &Ident) -> Option<&Ident> {
        self.nest_parent_map.get(nest_ident)
    }
    pub fn get_parent_by_ident(&self, nest_ident: &Ident) -> Option<&NestInfo> {
        self.nest_parent_map
            .get(nest_ident)
            .and_then(|ident| self.get_by_ident(ident))
    }
    pub fn is_parent_ident(&self, ident: &Ident) -> bool {
        self.origin_children_map
            .get(ident)
            .map(|children| !children.is_empty())
            == Some(true)
    }

    pub fn contains_nest_ident(&self, nest_ident: &Ident) -> bool {
        self.nest_map.contains_key(nest_ident)
    }

    pub fn get_all_ids(&self) -> Vec<String> {
        self.nest_id_map.keys().cloned().collect()
    }
    pub fn id_exists(&self, nest_id: &str) -> bool {
        self.nest_id_map.contains_key(nest_id)
    }

    pub fn add_field_to_nest(&mut self, nest_id: &LitStr, field: NestField) {
        let nest_id_str = nest_id.value();
        if let Some(info) = self.get_by_id_mut(&nest_id_str) {
            if info.fields.contains_key(&field.name) {
                emit_error!(
                    info.fields.get(&field.name).unwrap().name,
                    "First field defined here"
                );
                abort!(
                    &field.name,
                    "Field name used multiple times for nest {nest_id}"
                );
            }
            info.fields.insert(field.name.clone(), field);
        } else {
            abort!(nest_id, "Unknown nest ID: {nest_id}");
        }
    }
}

#[derive(Clone, Debug)]
pub struct NestField {
    pub name: Ident,

    /// custom attributes passed in via `shrinkwrap_attr`
    pub attrs: Vec<TokenStream>,
}
