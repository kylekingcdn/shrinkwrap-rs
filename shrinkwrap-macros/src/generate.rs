use quote::{quote, ToTokens};
use std::collections::{HashMap, HashSet};
use syn::Ident;

mod types;

use types::*;
use crate::parse::types::{DeriveItemOpts,DeriveItemFieldOpts, NestOpts, WrapperOpts, ExtraOpts};

pub(crate) fn generate_entrypoint(root: DeriveItemOpts) -> proc_macro2::TokenStream {
    let origin_fields = root.data.take_struct().unwrap().fields;
    let nest_keys = &root.nest_opts.iter().map(|n| n.key.clone()).collect();
    let nest_fields = build_nest_fields_map(&origin_fields);

    validate_nests(&nest_fields, &nest_keys);

    let mut output = quote!();
    let DeriveItemOpts { wrapper_opts, extra_opts, nest_opts, ident: data_ident, .. } = root;
    let extra_ident = extra_opts.struct_name(&data_ident);

    let nests = generate_nests(nest_opts, &data_ident, nest_fields);

    output.extend(generate_wrapper_struct(wrapper_opts, &data_ident, &extra_ident));
    output.extend(generate_extra_struct(extra_opts, &data_ident, &nests));
    output.extend(generate_nest_structs(nests));

    output
}

pub(crate) fn generate_wrapper_struct(
    opts: WrapperOpts,
    data_ident: &Ident,
    extra_ident: &Ident,
) -> proc_macro2::TokenStream {
    let struct_name = opts.struct_name(data_ident);
    let data_field_name = Ident::new(opts.data_field_name().as_str(), data_ident.span());
    let extra_field_name = Ident::new(opts.extra_field_name().as_str(), data_ident.span());
    let WrapperOpts { doc, derive, data_field_doc, flatten_data, extra_field_doc, .. } = opts;

    let wrapper = types::Wrapper {
        struct_name,
        struct_docs: doc,
        derive: derive,
        data_field_name,
        data_struct_name: data_ident.clone(),
        data_field_docs: data_field_doc,
        data_flattened: flatten_data.unwrap_or_default(),
        extra_field_name,
        extra_struct_name: extra_ident.clone(),
        extra_field_docs: extra_field_doc,
    };
    let mut token_stream = quote!();
    wrapper.to_tokens(&mut token_stream);

    token_stream
}

pub(crate) fn generate_extra_struct(
    opts: ExtraOpts,
    data_ident: &Ident,
    nests: &Vec<Nest>,
) -> proc_macro2::TokenStream {
    let nest_fields = nests.iter().map(|n| (n.key.clone(), n.struct_name.clone())).collect();
    let extra = types::Extra {
        struct_name: opts.struct_name(data_ident),
        derive: opts.derive,
        struct_docs: opts.doc,
        nests: nest_fields,
    };

    let mut token_stream = quote!();
    extra.to_tokens(&mut token_stream);

    token_stream
}

// FIXME: error handling
pub(crate) fn validate_nests(nest_field_map: &HashMap<String, Vec<NestField>>, declared_nest_keys: &Vec<String>) {
    // store nest names in set, ensure no duplicate nests defined
    let mut nest_keys = HashSet::new();
    for nest_key in declared_nest_keys {
        if nest_keys.contains(&nest_key) {
            panic!("Multiple nests defined with key '{nest_key}'");
        }
        nest_keys.insert(nest_key);
    }

    // ensure all nests specified in fields have been defined
    for field_nest in nest_field_map.keys() {
        if !nest_keys.contains(field_nest) {
            let field_name = nest_field_map.get(field_nest).unwrap().first().unwrap().name.to_string();
            panic!("Unknown nest '{field_nest}' assigned to field '{field_name}'.\n\nIs the struct missing a `#[shrinkwrap(nest(key = \"{field_nest}\", ..))]` attribute?");
        }
    }
}

pub(crate) fn generate_nests(
    opts: Vec<NestOpts>,
    data_ident: &Ident,
    nest_field_map: HashMap<String, Vec<NestField>>,
) -> Vec<Nest> {
    let mut nests = Vec::new();
    for nest_opts in opts {
        let transform_variant = if nest_opts.from {
            if nest_opts.transform.is_some() {
                panic!("nest(from) and nest(transform = ...) cannot be defined simultaneously");
            }
            types::NestTransform::FromImpl { data_ident: data_ident.clone() }
        }
        else if let Some(transform) = nest_opts.transform.clone() {
            types::NestTransform::Transform { path: transform }
        } else {
            panic!("Either transform or from must be defined for a nest");
        };

        let fields = nest_field_map.get(&nest_opts.key).cloned().unwrap_or(Vec::new());
        let struct_name = nest_opts.struct_name(data_ident);
        let key = Ident::new(nest_opts.key.as_str(), data_ident.span());
        let NestOpts { doc, derive, field_type, .. } = nest_opts;

        let nest = Nest {
            struct_name,
            struct_docs: doc,
            derive,
            key,
            transform: transform_variant,
            field_type,
            fields,
        };

        nests.push(nest)
    }

    nests
}

pub(crate) fn generate_nest_structs(
    nests: Vec<Nest>,
) -> proc_macro2::TokenStream {
    let mut output = quote! {};
    for nest in nests {
        output.extend(quote! { #nest });
    }

    output
}

pub(crate) fn build_nest_fields_map(origin_fields: &Vec<DeriveItemFieldOpts>) -> HashMap<String, Vec<NestField>> {
    let mut map = HashMap::new();

    for origin_field in origin_fields {
        let field_ident = origin_field.ident.clone().unwrap();
        for nest_in in &origin_field.nest_in_opts {
            let nest_key_ident = nest_in.nest_key.clone();
            let nest_key_name = nest_key_ident.to_string();
            let field = NestField {
                name: field_ident.clone(),
                field_doc: nest_in.field_doc.clone(),
            };
            map.entry(nest_key_name).and_modify(|e: &mut Vec<NestField>| {
                // check if the nest already contains this field
                if e.contains(&field) {
                    panic!("Nest '{}' already contains field: {}", nest_key_ident.to_string(), field_ident.to_string());
                } else {
                    e.push(field.clone());
                }
            }).or_insert(vec![field]);
        }
    }

    map
}
