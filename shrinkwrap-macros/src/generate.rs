use quote::{quote, ToTokens};
use std::collections::{HashMap, HashSet};
use syn::{Ident, Type};

mod types;

use types::*;
use crate::parse::types::{DeriveItemFieldOpts, DeriveItemOpts, ExtraOpts, NestMapStrategy, NestOpts, WrapperOpts};

/// Provides a mapping of a nest's defined origin (or root) to nest opts
pub(crate) type NestOriginMap<'a> = HashMap<Ident, Vec<NestOpts>>;
/// Provides a mapping of a nest ID to a list of fields that should be mapped to the assoc nest.
pub(crate) type NestFieldMap<'a> = HashMap<String, Vec<NestField>>;

pub(crate) fn generate_entrypoint(root: DeriveItemOpts) -> proc_macro2::TokenStream {
    let origin_fields = root.data.take_struct().expect("couldnt get root fields").fields;
    let nest_ids = &root.nest_opts.iter().map(|n| n.id.clone()).collect();
    let nest_fields = build_nest_fields_map(&origin_fields);

    validate_nests(&nest_fields, nest_ids);

    let DeriveItemOpts { wrapper_opts, extra_opts, nest_opts, ident: root_ident, .. } = root;
    let extra_ident = extra_opts.struct_name(&root_ident);
    let nest_origin_map = build_origin_map(nest_opts, &root_ident);
    let root_nests: Vec<&NestOpts> = nest_origin_map.get(&root_ident).map(|nests| nests.iter().collect()).unwrap_or_default();

    // check if all root nests have a From<origin> impl (or more accurately, have the `from` attribute indicator)
    let all_from_impl = root_nests.iter().all(|root_nest| root_nest.map_strategy.maps_with_from());
    // check if all use same transform
    let first_transform = root_nests.iter().find(|nest| nest.map_strategy.maps_with_transform()).and_then(|n| n.map_strategy.map_transform_type());
    let first_strategy = first_transform.clone().map(NestMapStrategy::Transform);
    let all_identical_transform = match first_strategy {
        Some(strategy) => root_nests.iter().all(|nest| nest.map_strategy == strategy),
        None => false,
    };
    let transform_all_with = match all_identical_transform {
        true => first_transform,
        false => None,
    };
    // cloned for later use after move
    let extra_field_name = wrapper_opts.extra_field_name();

    let mut output = quote!();
    generate_wrapper_struct(wrapper_opts, &root_ident, &extra_ident, &nest_origin_map, &mut output, all_from_impl, transform_all_with);
    generate_extra_structs(&extra_opts, &root_ident, &nest_origin_map, &mut output, all_from_impl);
    generate_nest_structs(nest_origin_map, &extra_field_name, &extra_opts, &root_ident, nest_fields, &mut output);

    output
}

pub(crate) fn generate_wrapper_struct(
    opts: WrapperOpts,
    root_ident: &Ident,
    extra_ident: &Ident,
    origin_map: &NestOriginMap,
    tokens: &mut proc_macro2::TokenStream,
    from_impl: bool,
    from_with_impl: Option<Type>,
) {
    let mut impl_tokens = quote!();
    let wrapper = Wrapper::new(opts, root_ident, extra_ident);
    let mut root_extra_fields = Vec::new();
    if let Some(nests) = origin_map.get(root_ident) {
        for nest in nests {
            root_extra_fields.push(ExtraNestField { field_name: nest.field_name(), type_ident: nest.struct_name(root_ident)});
        }
    }
    if from_impl {
        impl_tokens.extend(wrapper.to_wrapped_impl());
        impl_tokens.extend(wrapper.build_from_data_impl());
    } else if let Some(transform) = from_with_impl {
        impl_tokens.extend(wrapper.to_wrapped_with_impl(transform, &root_extra_fields));
    }
    wrapper.to_tokens(tokens);
    tokens.extend(impl_tokens);
}

pub(crate) fn generate_extra_structs(
    opts: &ExtraOpts,
    root_ident: &Ident,
    origin_map: &NestOriginMap,
    tokens: &mut proc_macro2::TokenStream,
    from_impl: bool,
) {
    for (origin_ident, nest_opts) in origin_map {
        let mut nest_fields = Vec::new();
        // build a list of all children nests for generating the struct definition
        for nest in nest_opts {
            let field = ExtraNestField {
                field_name: nest.field_name(),
                type_ident: nest.struct_name(root_ident)
            };
            nest_fields.push(field);
        }
        let mut impl_tokens = quote!();
        let extra = Extra::new(opts, origin_ident, nest_fields);
        if from_impl && origin_ident == root_ident {
            impl_tokens.extend(extra.build_from_data_impl(origin_ident))
        }
        extra.to_tokens(tokens);
        tokens.extend(impl_tokens);
    }
}

pub(crate) fn generate_nest_structs(
    origin_map: NestOriginMap,
    extra_field_name: &Ident,
    extra_opts: &ExtraOpts,
    root_ident: &Ident,
    nest_fields: NestFieldMap,
    tokens: &mut proc_macro2::TokenStream,
) {
    let mut child_counts = HashMap::new();
    for origin_ident in origin_map.keys() {
        child_counts.entry(origin_ident.clone()).and_modify(|counter| *counter += 1).or_insert(1);
    }
    origin_map.into_iter().for_each(|(_, origin_nests)| {
        for nest in origin_nests {
            // fixme: bad clone
            let fields = nest_fields.get(&nest.id).cloned().unwrap_or(Vec::<NestField>::new());
            let nest_ident = nest.struct_name(root_ident);
            let has_children = child_counts.get(&nest_ident).map(|count| *count > 0i32) == Some(true);
            let with_extra:
                Option<ExtraNestField> = if has_children {
                    Some(ExtraNestField {
                        field_name: extra_field_name.clone(),
                        type_ident: extra_opts.struct_name(&nest_ident)
                    })
                } else {
                    None
                };
            Nest::new(nest, root_ident, fields, with_extra).to_tokens(tokens)
        }
    });
}

// -- utils

// FIXME: error handling
pub(crate) fn validate_nests(nest_field_map: &NestFieldMap, all_nest_ids: &Vec<String>) {
    let mut issues = Vec::new();
    {
        let mut visited_ids = HashSet::new();
        let mut duplicate_ids = HashSet::new();
        for id in all_nest_ids {
            if !visited_ids.insert(id) {
                duplicate_ids.insert(id);
            }
        }
        for dupe_id in duplicate_ids {
            issues.push(format!("Multiple nests are using the same ID: `{dupe_id}`"));
        }
    }

    // ensure all nests specified in fields have been defined
    for (nest_id, nest_fields) in nest_field_map {
        if !all_nest_ids.contains(nest_id) {
            let field_name = nest_fields.first().expect("no field in validate call").name.to_string();
            panic!("Unknown nest '{nest_id}' assigned to field '{field_name}'.\n\nIs the struct missing a `#[shrinkwrap(nest(id = \"{nest_id}\", ..))]` attribute?");
        }
    }
}

pub(crate) fn build_origin_map(nests: Vec<NestOpts>, root_ident: &Ident) -> NestOriginMap<'_> {
    let mut map = NestOriginMap::new();

    for nest in nests {
        let nest_origin = nest.origin(root_ident);
        if !map.contains_key(nest_origin) {
            map.insert(nest_origin.clone(), Vec::new());
        }
        map.get_mut(nest_origin).expect("no field in validate call").push(nest);
    }
    map
}

pub(crate) fn build_nest_fields_map(origin_fields: &Vec<DeriveItemFieldOpts>) -> NestFieldMap<'_> {
    let mut map = NestFieldMap::new();

    for origin_field in origin_fields {
        if let Some(field_ident) = origin_field.ident.clone() {
            for nest_in in &origin_field.nest_in_opts {
                let nest_id_ident = nest_in.nest_id.clone();
                let nest_id_name = nest_id_ident.to_string();
                let field = NestField {
                    name: field_ident.clone(),
                    field_doc: nest_in.field_doc.clone(),
                };
                map.entry(nest_id_name).and_modify(|e: &mut Vec<NestField>| {
                    // check if the nest already contains this field
                    if e.contains(&field) {
                        panic!("Nest '{nest_id_ident}' already contains field: {field_ident}");
                    } else {
                        e.push(field.clone());
                    }
                }).or_insert(vec![field]);
            }
        }
    }
    map
}
