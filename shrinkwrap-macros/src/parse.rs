use darling::FromMeta;
use proc_macro2::TokenStream;
use proc_macro_error2::abort;
use quote::ToTokens;
use std::collections::HashMap;
use syn::{spanned::Spanned, Attribute, Ident, Meta};

use types::*;
use crate::mapping::types::{NestField, NestStructAttrInfo};

pub mod types;

pub fn parse_struct_attrs(
    all_nest_ids: &Vec<String>,
    forward_ident: &Ident,
    attrs: &Vec<Attribute>,
) -> HashMap<String, NestStructAttrInfo> {
    let mut attr_map = HashMap::new();

    // start by adding all nest ids to map
    for nest_id in all_nest_ids {
        attr_map.insert(nest_id.clone(), NestStructAttrInfo::default());
    }

    for attr in attrs {
        // only handle attributes that are nested under the specified forward ident (e.g. shrinkwrap_attr)
        if attr.path().get_ident() != Some(forward_ident) {
            continue;
        }
        // once the attribute passes the initial from call, convert to general struct attr for unified impls
        match PassthroughStructAttribute::from_meta(&attr.meta) {
            Err(error) => abort!(error.span(), error),
            Ok(attr_field) => {
                let nest_ids = match &attr_field.limit.nests {
                    // add attribute to all nests
                    None => all_nest_ids.clone(),
                    // use limited set of nests as specified
                    Some(nest_ids) => nest_ids.iter().map(|id| id.value()).collect()
                };
                let attr_classes = attr_field.limit.class.unwrap_or_default();

                for nest_id in nest_ids {
                    let attr_info = attr_map.get_mut(&nest_id).unwrap_or_else(|| abort!(&attr, format!("Unknown nest: {nest_id}")));
                    let attr_contents = extract_passthrough_attr_meta_list(&attr_field.attr);
                    if attr_classes.contains(DerivedStructClass::Wrapper) {
                        attr_info.add_wrapper_attr(attr_contents.clone());
                    }
                    if attr_classes.contains(DerivedStructClass::Extra) {
                        attr_info.add_extra_attr(attr_contents.clone());
                    }
                    if attr_classes.contains(DerivedStructClass::Nest) {
                        attr_info.add_nest_attr(attr_contents.clone());
                    }
                }
            }
        };
    }

    attr_map
}

/// Returns a map of nest id's to attribute list
pub fn parse_field_attrs(
    all_nest_ids: &Vec<String>,
    forward_ident: &Ident,
    attrs: &Vec<Attribute>,
) -> HashMap<String, Vec<TokenStream>> {
    let mut attr_map = HashMap::new();

    // start by adding all nest ids to map
    for nest_id in all_nest_ids {
        attr_map.insert(nest_id.clone(), Vec::new());
    }

    for attr in attrs {
        // only handle attributes that are nested under the specified forward ident (e.g. shrinkwrap_attr)
        if attr.path().get_ident() != Some(forward_ident) {
            continue;
        }
        // once the attribute passes the initial from call, convert to general struct attr for unified impls
        match PassthroughFieldAttribute::from_meta(&attr.meta) {
            Err(error) => abort!(error.span(), error),
            Ok(attr_field) => {
                let attr_contents = extract_passthrough_attr_meta_list(&attr_field.attr);
                match &attr_field.limit.nests {
                    // add attribute to all nests
                    None => {
                        for v in attr_map.values_mut() {
                            v.push(attr_contents.clone());
                        }
                    }
                    Some(nest_ids) => {
                        for id in nest_ids {
                            let id_str = &id.value();
                            if !attr_map.contains_key(id_str) {
                                abort!(id, "Unknown nest ID: {id_str}");
                            }
                            attr_map.get_mut(id_str).unwrap().push(attr_contents.clone());
                        }
                    }
                    // nest_ids.iter().map(|id| id.value()).collect(),
                };
            }
        };
    }

    attr_map
}

pub fn map_fields(
    state: &mut State,
    all_nest_ids: &Vec<String>,
    origin_fields: Vec<DeriveItemFieldOpts>,
    passthrough_attr_ident: &Ident,
) {
    for field in origin_fields {
        if let Some(field_ident) = field.ident {

            // build nest -> attr map for field
            let mut nest_attr_map = parse_field_attrs(all_nest_ids, passthrough_attr_ident, &field.attrs);
            // add field to nest
            for nest_id in field.nests {
                let nest_id_str = nest_id.value();
                let attrs = nest_attr_map.remove(&nest_id_str).unwrap_or_else(|| {
                    abort!(&nest_id, format!("Unknown nest: {nest_id_str}"));
                });
                let nest_field = NestField { name: field_ident.clone(), attrs };
                state.nest_repo.add_field_to_nest(&nest_id, nest_field);
            }
            // NOTE: silently ignore extra nest ids -> unable to determine if no limit from implicit all nests
            // if !nest_attr_map.is_empty() {
            //     for (k, _) in nest_attr_map {
            //         emit_call_site_error!(format!("Nest '`{k}`' does not include field `{field_ident}`"));
            //     }
            //     abort!(&field_ident, "Nest included in attribute filter does not include the corresponding field");
            // }
        }
    }
}

fn extract_passthrough_attr_meta_list(attr_meta: &Meta) -> TokenStream {
    match attr_meta.require_list() {
        Ok(list) => { list.tokens.to_token_stream() },
        Err(error) => {
            abort!(attr_meta.span(), format!("Unexpected attr meta type. Expected a list `(that,looks,like,this).\nOriginal error: {error}`"));
        },
    }
}
