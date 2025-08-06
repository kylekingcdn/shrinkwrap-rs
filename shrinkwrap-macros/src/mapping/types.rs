#![doc = "Helper types used to associate related types"]
#![allow(dead_code)]

use darling::util::SpannedValue;
use proc_macro_error2::{abort, emit_error};
use proc_macro2::TokenStream;
use std::collections::HashMap;
use syn::{Ident, LitStr};

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
}
impl NestInfo {
    pub fn new(ident: Ident, nest_opts: NestOpts) -> Self {
        Self {
            ident,
            opts: nest_opts,
            struct_attrs: NestStructAttrInfo::default(),
            fields: HashMap::new(),
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
}
impl NestRepo {
    pub fn new(root_ident: Ident) -> Self {
        Self {
            root_ident,
            nest_map: HashMap::new(),
            origin_children_map: HashMap::new(),
            nest_parent_map: HashMap::new(),
            nest_id_map: HashMap::new(),
            nest_id_span_map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, opts: NestOpts) {
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
        self.nest_map
            .insert(nest_ident.clone(), NestInfo::new(nest_ident, opts));
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
