use darling::FromMeta;
use quote::{ToTokens, quote};
use std::collections::{HashMap, HashSet};
use syn::{Ident, Type};

mod types;

use crate::parse::types::{
    DeriveItemFieldOpts, DeriveItemOpts, ExtraOpts, NestMapStrategy, NestOpts,
    PassthroughAttribute, WrapperOpts,
};
use types::*;

pub(crate) fn generate_entrypoint(root: DeriveItemOpts) -> proc_macro2::TokenStream {
    let origin_fields = root
        .data
        .take_struct()
        .expect("couldnt get root fields")
        .fields;
    let nest_ids = &root.nest_opts.iter().map(|n| n.id.clone()).collect();
    let nest_fields = build_nest_fields_map(&origin_fields);

    validate_nests(&nest_fields, nest_ids);

    let DeriveItemOpts {
        wrapper_opts,
        extra_opts,
        nest_opts,
        ident: root_ident,
        ..
    } = root;
    let nest_attrs = parse_field_attrs(&root_ident, &origin_fields, &nest_fields);
    if let Err(error) = nest_attrs {
        return syn::Error::new(root_ident.span(), error).into_compile_error();
    }
    let nest_attrs = nest_attrs.unwrap();

    let extra_ident = extra_opts.struct_name(&root_ident);
    let nest_origin_map = build_origin_map(nest_opts, &root_ident);
    let root_nests: Vec<&NestOpts> = nest_origin_map
        .get(&root_ident)
        .map(|nests| nests.iter().collect())
        .unwrap_or_default();

    // check if all root nests have a From<origin> impl (or more accurately, have the `from` attribute indicator)
    let all_from_impl = root_nests
        .iter()
        .all(|root_nest| root_nest.map_strategy.maps_with_from());
    // check if all use same transform
    let first_transform = root_nests
        .iter()
        .find(|nest| nest.map_strategy.maps_with_transform())
        .and_then(|n| n.map_strategy.map_transform_type());
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
    generate_wrapper_struct(
        wrapper_opts,
        &root_ident,
        &extra_ident,
        &nest_origin_map,
        &mut output,
        all_from_impl,
        transform_all_with,
    );
    generate_extra_structs(
        &extra_opts,
        &root_ident,
        &nest_origin_map,
        &mut output,
        all_from_impl,
    );
    generate_nest_structs(
        nest_origin_map,
        &extra_field_name,
        &extra_opts,
        &root_ident,
        nest_fields,
        nest_attrs,
        &mut output,
    );

    output
}

pub(crate) fn parse_field_attrs<'a>(
    root_ident: &'a Ident,
    fields: &'a Vec<DeriveItemFieldOpts>,
    nest_fields: &'a NestFieldMap<'a>,
) -> Result<NestFieldAttrMap<'a>, syn::Error> {
    let mut map = NestFieldAttrMap::new();

    let passthrough_attr_ident = Ident::new("shrinkwrap_attr", root_ident.span());
    for field in fields {
        for attr in &field.attrs {
            if attr.path().get_ident() == Some(&passthrough_attr_ident) {
                let attr_struct = PassthroughAttribute::from_meta(&attr.meta)
                    .map_err(|e| syn::Error::new(root_ident.span(), e))?;

                for attr in attr_struct.attr {
                    let attr_contents = &attr.require_list().unwrap().tokens;
                    let token = attr_contents.to_token_stream();
                    let field_ident = field.ident.clone().unwrap();
                    let field_attr = NestFieldAttrs {
                        field_name: field_ident,
                        attributes_token: token,
                    };
                    let mut visited_nest_ids = HashSet::new();
                    if attr_struct.nest.is_empty() {
                        // add to all
                        for nest_id in nest_fields.keys() {
                            if !map.contains_key(nest_id) {
                                map.insert(nest_id.to_owned(), vec![]);
                            }
                            let fa = field_attr.clone();
                            map.get_mut(nest_id).unwrap().push(fa);
                            visited_nest_ids.insert(nest_id.to_owned());
                        }
                    } else {
                        // restrict to specified nests
                        for nest_id in attr_struct.nest.to_strings() {
                            if !nest_fields.contains_key(&nest_id) {
                                panic!(
                                    "Nest '{nest_id}' doesn't exist (defined in `#[shrinkwrap_attr(...)]` field {})",
                                    field_attr.field_name
                                );
                            }
                            if !map.contains_key(&nest_id) {
                                map.insert(nest_id.to_owned(), vec![]);
                            } else if visited_nest_ids.contains(&nest_id) {
                                panic!(
                                    "Nest '{nest_id}' specified multiple times (defined in `#[shrinkwrap_attr(...)]` field {})",
                                    field_attr.field_name
                                );
                            }
                            map.get_mut(&nest_id).unwrap().push(field_attr.clone());
                            visited_nest_ids.insert(nest_id.to_owned());
                        }
                    }
                }
            }
        }
    }

    Ok(map)
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
            root_extra_fields.push(ExtraNestField {
                field_name: nest.field_name(),
                type_ident: nest.struct_name(root_ident),
            });
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
                type_ident: nest.struct_name(root_ident),
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
    nest_field_attrs: NestFieldAttrMap,
    tokens: &mut proc_macro2::TokenStream,
) {
    let mut child_counts = HashMap::new();
    for origin_ident in origin_map.keys() {
        child_counts
            .entry(origin_ident.clone())
            .and_modify(|counter| *counter += 1)
            .or_insert(1);
    }
    origin_map.into_iter().for_each(|(_, origin_nests)| {
        for nest in origin_nests {
            // fixme: bad clone
            let fields = nest_fields
                .get(&nest.id)
                .cloned()
                .unwrap_or(Vec::<NestField>::new());
            let nest_ident = nest.struct_name(root_ident);
            let has_children =
                child_counts.get(&nest_ident).map(|count| *count > 0i32) == Some(true);
            let with_extra: Option<ExtraNestField> = if has_children {
                Some(ExtraNestField {
                    field_name: extra_field_name.clone(),
                    type_ident: extra_opts.struct_name(&nest_ident),
                })
            } else {
                None
            };
            let attrs = nest_field_attrs.get(&nest.id).cloned().unwrap_or_default();
            Nest::new(nest, root_ident, fields, attrs, with_extra).to_tokens(tokens)
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
            let field_name = nest_fields
                .first()
                .expect("no field in validate call")
                .name
                .to_string();
            panic!(
                "Unknown nest '{nest_id}' assigned to field '{field_name}'.\n\nIs the struct missing a `#[shrinkwrap(nest(id = \"{nest_id}\", ..))]` attribute?"
            );
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
        map.get_mut(nest_origin)
            .expect("no field in validate call")
            .push(nest);
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
                map.entry(nest_id_name)
                    .and_modify(|e: &mut Vec<NestField>| {
                        // check if the nest already contains this field
                        if e.contains(&field) {
                            panic!("Nest '{nest_id_ident}' already contains field: {field_ident}");
                        } else {
                            e.push(field.clone());
                        }
                    })
                    .or_insert(vec![field]);
            }
        }
    }
    map
}

#[allow(unused_imports)]
pub(crate) use expand::{expand_debug,expand_to_tokens,expand_tokens,expand_tokens_unfmt};

/// no-op function signatures for feature toggle
#[cfg(not(feature = "expand"))]
#[allow(dead_code)]
mod expand {
    pub(crate) fn expand_debug<T: std::fmt::Debug>(_t: &T, _type_name: &'static str, _fn_name: &'static str) {}
    pub(crate) fn expand_tokens(_tokens: &proc_macro2::TokenStream, _fn_name: &'static str) {}
    pub(crate) fn expand_to_tokens<T: quote::ToTokens>(_t: &T, _type_name: &'static str, _fn_name: &'static str) {}
    pub(crate) fn expand_tokens_unfmt(_tokens: &proc_macro2::TokenStream, _fn_name: &'static str) {}
}

#[cfg(feature = "expand")]
#[allow(dead_code)]
mod expand {
    // all
    const T_RESET: &str = "\x1b[0m";
    // style
    const T_BOLD: &str = "\x1b[1m";
    const T_UNDERLINE: &str = "\x1b[4m";
    // text color
    const T_C_RESET: &str = "\x1b[39m";
    const T_C_WHITE: &str = "\x1b[97m";
    const T_C_BLACK: &str = "\x1b[30m";
    const T_C_BLUE: &str = "\x1b[34m";
    const T_C_RED: &str = "\x1b[31m";
    // text background color
    const T_B_RESET: &str = "\x1b[49m";
    const T_B_BLUE: &str = "\x1b[44m";
    const T_B_RED: &str = "\x1b[41m";

    /// Dumps the type to stderr using it's Debug impl, but only if the `expand` feature is enabled. Otherwise this is a no-op
    pub(crate) fn expand_debug<T: std::fmt::Debug>(t: &T, type_name: &'static str, fn_name: &'static str) {
        eprintln!("\n{T_BOLD}{T_C_BLUE}------------------------------------------------{T_RESET}");
        eprintln!("{T_B_BLUE}[{type_name}]{T_B_RESET} {T_C_BLUE}{fn_name}:{T_RESET} \n{t:#?}\n");
        eprintln!("{T_BOLD}{T_C_BLUE}------------------------------------------------{T_RESET}");
    }

    /// Dumps token stream to stderr if the `expand` feature is enabled. Otherwise this is a no-op
    ///
    /// Attempts to format generated rust code, if valid. Otherwise the output is provided unformatted.
    pub(crate) fn expand_tokens(tokens: &proc_macro2::TokenStream, fn_name: &'static str) {
        eprintln!("\n{T_BOLD}{T_C_BLUE}------------------------------------------------{T_RESET}");
        match syn::parse_file(tokens.to_string().as_str()) {
            Ok(tokens_file) => {
                let tokens_fmt = prettyplease::unparse(&tokens_file);
                eprintln!("{T_BOLD}{T_C_BLUE}{fn_name}:{T_RESET} \n{}", &tokens_fmt);
                eprintln!("{T_BOLD}{T_C_BLUE}------------------------------------------------{T_RESET}");
            }
            Err(err) => {
                eprintln!("{T_BOLD}{T_B_RED}{fn_name}:{T_RESET} Failed to render formatted output - err: {err}.");
                eprintln!("Output will be unformatted.\n");
                expand_tokens_unfmt(tokens, fn_name)
            }
        }
    }

    /// Helper fn for expand_tokens, where the type's `ToTokens` is automatically called
    pub(crate) fn expand_to_tokens<T: quote::ToTokens>(t: &T, type_name: &'static str, fn_name: &'static str) {
        eprintln!("\n{T_BOLD}{T_C_BLUE}------------------------------------------------{T_RESET}");
        let token_stream = t.to_token_stream();
        match syn::parse_file(token_stream.to_string().as_str()) {
            Ok(tokens_file) => {
                let tokens_fmt = prettyplease::unparse(&tokens_file);
                eprintln!("{T_B_BLUE}[{type_name}]{T_RESET} {T_BOLD}{T_C_BLUE}{fn_name}:{T_RESET} \n{}", &tokens_fmt);
                eprintln!("{T_BOLD}{T_C_BLUE}------------------------------------------------{T_RESET}");
            }
            Err(err) => {
                eprintln!("{T_B_RED}[{type_name}]{T_RESET} {T_BOLD}{T_C_RED}{fn_name}:{T_RESET} Failed to render formatted output - err: {err}.");
                eprintln!("Output will be unformatted.\n");
                expand_tokens_unfmt(&token_stream, fn_name)
            }
        }
    }

    /// Dumps token stream to stderr if the `expand` feature is enabled. Otherwise this is a no-op
    ///
    /// Attempts to format generated rust code, if valid. Otherwise the output is provided unformatted.
    pub(crate) fn expand_tokens_unfmt(tokens: &proc_macro2::TokenStream, fn_name: &'static str) {
        eprintln!("\n{T_BOLD}{T_C_BLUE}------------------------------------------------{T_RESET}");
        eprintln!("{T_BOLD}{T_C_BLUE}{fn_name}{T_C_RESET} unformatted: \n{}", &tokens);
        eprintln!("{T_BOLD}{T_C_BLUE}------------------------------------------------{T_RESET}");
    }
}
