use proc_macro_error2::OptionExt;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Attribute, Ident, Path, Type, parse_quote};
use std::collections::HashMap;
use std::rc::Rc;

use crate::{
    model::{
        DataVariant,
        Extra, ExtraChildVariant, ExtraField,
        ModelTree,
        NestData, NestDataField, NestAutoDeriveToNest,
        OriginData, OriginDataField,
        RecursiveToTokens,
        Wrapper,
    },
    parse::{
        FieldResolver,
        NestHierarchy,
        StructAttrResolver,
        types::{DeriveItemOpts, NestOpts, StructClass},
    },
};

pub(crate) mod state;
use state::State;

pub(crate) mod structs;
use structs::GenStructField;

mod trait_impl;
use trait_impl::{
    Fallibility,
    GenToWrappedWith, GenTransformToDeepNest, GenTransformToNest, GenTransformToNestOptional
};

pub fn generate(derive_opts: DeriveItemOpts, tokens: &mut TokenStream) {
    // destructure input opts
    let DeriveItemOpts {
        ident: root_ident,
        data,
        attrs,
        global_opts,
        wrapper_opts,
        extra_opts,
        nest_opts,
    } = derive_opts;

    // stage 1 - build simple util types that assist in
    //           construction of primary models
    //             - nest hierarchy
    //             - field resolver
    //             - struct attr resolver
    // build nest nest_hierarchy
    let nest_hierarchy = NestHierarchy::from_nest_opts(nest_opts);
    // nest_hierarchy.print();

    // build map of nest fields
    let origin_fields = data.take_struct().expect_or_abort("couldnt get root fields").fields;
    let field_resolver = FieldResolver::from_opt_fields(origin_fields);
    // field_resolver.print();

    // build struct attrs
    let struct_attr_resolver = StructAttrResolver::from_attrs(attrs.iter().collect());
    // struct_attr_resolver.print();

    // init state
    let state = State::new(
        global_opts, wrapper_opts, extra_opts,
        root_ident.clone(),
        nest_hierarchy,
        struct_attr_resolver,
        field_resolver,
    );

    // stage 2 - models
    //           construct primary generators
    // store required trait values
    let fallibility = match &state.global.fallible {
        Some(opts) => Fallibility::Fallible { error_type: opts.error.clone() },
        None => Fallibility::Infallible,
    };
    let transform_type = state.global.transform.clone();
    let transform_bounds = state.global.transform_generic_params.clone();
    // generate model tree
    let models = gen_models(state);

    // stage 3 - codegen
    //           run struct + trait gen from models
    gen_structs(&models, tokens);
    gen_traits(&models, &fallibility, &transform_type, &transform_bounds, tokens);
}

// !- Models

fn gen_models(state: State) -> ModelTree {
    let origin_data = Rc::new(gen_origin_data(&state));
    let mut deep_models = Vec::new();
    let children = state.nest_hierarchy.get_children(None);
    for root_child in children {
        let root_child = root_child.as_str();
        let root_child_opts = state.nest_hierarchy.get_nest_opts(root_child);
        let child_extra_field_obj = gen_models_dfs(&state, root_child);
        let child_extra_field = ExtraField {
            name: root_child_opts.field_name(),
            object: child_extra_field_obj,
            optional: root_child_opts.optional() || state.global.all_optional.is_present(),
        };
        deep_models.push(child_extra_field);
    }

    let data = origin_data.clone().into();
    let extra = Rc::new(gen_extra(&state, deep_models, &data));
    let wrapper = gen_wrapper(&state, data, extra);
    ModelTree::new(wrapper, origin_data)
}

fn gen_models_dfs(state: &State, nest_id: &str) -> ExtraChildVariant {
    let children = state.nest_hierarchy.get_children(Some(nest_id));
    let mut extra_children = Vec::new();
    // first generate any children
    for child in children {
        let child = child.as_str();
        // build child object suitable for assignment as a field in Extra
        let child_opts = state.nest_hierarchy.get_nest_opts(child);
        let child_extra_field_obj = gen_models_dfs(state, child);

        // build the extra field and append to fields list
        let child_extra_field = ExtraField {
            name: child_opts.field_name(),
            object: child_extra_field_obj,
            optional: child_opts.optional() || state.global.all_optional.is_present(),
        };
        extra_children.push(child_extra_field);
    }

    // generate nest struct for current nest id / level
    let nest_opts = state.nest_hierarchy.get_nest_opts(nest_id);
    let nest = Rc::new(gen_nest(state, nest_opts));
    // no sub-nests, just return nest as extra child
    if extra_children.is_empty() {
        ExtraChildVariant::Nest(nest)
    } else {
        // generate dedicated extra/wrappper type
        let data = DataVariant::Nest(nest);
        let extra = Rc::new(gen_extra(state, extra_children, &data));
        let wrapper = Rc::new(gen_wrapper(state, data, extra));
        ExtraChildVariant::Wrapper(wrapper)
    }
}

// !- Output structs

fn gen_structs(models: &ModelTree, tokens: &mut TokenStream) {
    models.recursive_to_tokens(tokens);
}

fn gen_origin_data(state: &State) -> OriginData {
    let fields = state.field_resolver.origin_fields()
        .into_iter()
        .map(OriginDataField::from).collect();
    OriginData {
        ident: state.root_ident.clone(),
        fields,
    }
}

// fixme: drop state, pass in wrapper_opts
fn gen_wrapper(state: &State, data: DataVariant, extra: Rc<Extra>) -> Wrapper {
    Wrapper {
        ident: state.wrapper_opts.struct_name(data.ident()),
        derives: state.full_derives(state.wrapper_opts.derive.clone()).into(),
        attrs: state.full_struct_attrs(data.nest_id(), StructClass::Wrapper),
        doc: state.wrapper_opts.struct_doc.clone().into(),
        data_name: state.wrapper_opts.data_field_name.clone(),
        data_doc: state.wrapper_opts.data_field_doc.clone().into(),
        data_flatten: state.wrapper_opts.flatten(),
        data,
        extra_name: state.wrapper_opts.extra_field_name.clone(),
        extra_doc: state.wrapper_opts.extra_field_doc.clone().into(),
        extra,
    }
}

// fixme: drop state, pass in extra_opts
fn gen_extra(state: &State, fields: Vec<ExtraField>, data: &DataVariant) -> Extra {
    Extra {
        ident: state.extra_opts.struct_name(data.ident()),
        derives: state.full_derives(state.extra_opts.derive.clone()).into(),
        attrs: state.full_struct_attrs(data.nest_id(), StructClass::Extra),
        doc: state.extra_opts.struct_doc.clone().into(),
        fields,
    }
}

// fixme: drop state, opts
fn gen_nest(state: &State, nest_opts: &NestOpts) -> NestData {
    let nest_id_str = nest_opts.id_str();
    let source_ident = state.nest_source_ident(nest_id_str);
    let optional = state.global.all_optional.is_present() || nest_opts.optional();

    let derive_to_nest = nest_opts.derive_to_nest.as_ref().map(|src_derive_to_nest|
        NestAutoDeriveToNest {
            nest_value: src_derive_to_nest.value.clone(),
            options_field_if_optional: optional.then(|| nest_opts.derive_to_nest_options_field_name()).flatten(),
        }
    );
    NestData {
        id: nest_id_str.to_string(),
        ident: nest_opts.struct_name(source_ident),
        derives: state.full_derives(nest_opts.derive.clone()).into(),
        attrs: state.full_struct_attrs(Some(nest_id_str), StructClass::Nest),
        doc: nest_opts.struct_doc.clone().into(),
        fields: gen_nest_fields(state, nest_opts),
        derive_to_nest,
    }
}

fn gen_nest_fields(state: &State, nest_opts: &NestOpts) -> Vec<NestDataField> {
    let nest_id_str = nest_opts.id_str();
    let filtered_origin_fields = state.field_resolver.nest_fields(nest_id_str);
    let field_type = nest_opts.resolve_field_type();
    let parent_nest_field_type: Option<Type> = nest_opts.chain_from.as_ref().map(|parent_id| {
        let path = state.nest_hierarchy.get_nest_opts(parent_id.to_string().as_str()).resolve_field_type().clone();
        parse_quote! { #path }
    });
    let mut out = Vec::new();
    for field in filtered_origin_fields {
        let attrs = state.field_resolver.attrs(nest_id_str, &field.name);
        out.push(NestDataField {
            name: field.name.clone(),
            ty: field_type.clone(),
            source_type: parent_nest_field_type.clone().unwrap_or_else(|| field.ty.clone()),
            attrs,
        });
    }
    out
}

// !- Output trait impls

/// Recurse through models, calling trait genarators as seen fit
fn gen_traits(
    models: &ModelTree,
    fallibility: &Fallibility,
    transform: &Path,
    transform_bounds: &Option<TokenStream>,
    tokens: &mut TokenStream,
) {
    gen_to_wrapped_with(models.origin_wrapper.clone(), fallibility, transform, transform_bounds, tokens);
    gen_transform_to_deep_nest(models.origin_wrapper.clone(), None, false, fallibility, transform, transform_bounds, tokens);
    gen_transform_to_nest(models.origin_wrapper.clone(), fallibility, transform, transform_bounds, tokens);
}

/// Recursively generate to wrapped with impls for the assiciated data struct and for any of the wrapper supported children
fn gen_to_wrapped_with(
    wrapper: Rc<Wrapper>,
    fallibility: &Fallibility,
    transform: &Path,
    transform_bounds: &Option<TokenStream>,
    tokens: &mut TokenStream,
) {
    let to_wrapped_with = GenToWrappedWith {
        variant: fallibility.clone().into(),
        transform_type: transform.clone(),
        transform_generic_bounds: transform_bounds.clone(),
        data_ident: wrapper.data.ident().clone(),
        wrapper_ident: wrapper.ident.clone(),
        extra_struct_ident: wrapper.extra.ident.clone(),
        extra_struct_fields: wrapper.extra.fields.iter().map(GenStructField::from).collect(),
    };
    to_wrapped_with.to_tokens(tokens);

    for extra_field in &wrapper.extra.fields {
        if let ExtraChildVariant::Wrapper(child_wrapper) = &extra_field.object {
            gen_to_wrapped_with(child_wrapper.clone(), fallibility, transform, transform_bounds, tokens);
        }
    }
}

/// Recursively generate transform to nest impls from source data to nested wrapper
fn gen_transform_to_deep_nest(
    wrapper: Rc<Wrapper>,
    wrapper_origin: Option<Ident>,
    optional: bool,
    fallibility: &Fallibility,
    transform: &Path,
    transform_bounds: &Option<TokenStream>,
    tokens: &mut TokenStream,
) {
    if let Some(source_ident) = wrapper_origin {
        // implement whenever a child wrapper is discovered
        let transform_to_deep_nest = GenTransformToDeepNest {
            variant: fallibility.clone().into(),
            transform_type: transform.clone(),
            transform_generic_bounds: transform_bounds.clone(),
            data_ident: source_ident,
            nest_wrapper_ident: wrapper.ident.clone(),
            nest_ident: wrapper.data.ident().clone(),
            optional,
        };
        transform_to_deep_nest.to_tokens(tokens);
    }
    for extra_field in &wrapper.extra.fields {
        if let ExtraChildVariant::Wrapper(child_wrapper) = &extra_field.object {
            gen_transform_to_deep_nest(child_wrapper.clone(), Some(wrapper.data.ident().clone()), extra_field.optional, fallibility, transform, transform_bounds, tokens);
        }
    }
}

/// Recursively generate transform to nest impls for nests with derive to nest set
fn gen_transform_to_nest(
    wrapper: Rc<Wrapper>,
    fallibility: &Fallibility,
    transform: &Path,
    transform_bounds: &Option<TokenStream>,
    tokens: &mut TokenStream,
) {
    let source_ident = wrapper.data.ident();

    // generate for data -> extra.[*]
    for extra_field in &wrapper.extra.fields {
        let nest_data = match extra_field.object.clone() {
            ExtraChildVariant::Nest(nest_data) => nest_data,
            ExtraChildVariant::Wrapper(nest_wrapper) => {
                match nest_wrapper.data.clone() {
                    DataVariant::Nest(nest_data) => Some(nest_data),
                    DataVariant::Origin(..) => None,
                }.unwrap() // guaranteed non-origin data due to recursing through extra
            }
        };

        gen_transform_to_nest_node(nest_data.clone(), source_ident, fallibility, transform, transform_bounds, tokens);

        // recurse through all nested wrappers
        if let ExtraChildVariant::Wrapper(nest_wrapper) = extra_field.object.clone() {
            gen_transform_to_nest(nest_wrapper, fallibility, transform, transform_bounds, tokens);
        }
    }
}

fn gen_transform_to_nest_node(
    nest_data: Rc<NestData>,
    source_ident: &Ident,
    fallibility: &Fallibility,
    transform: &Path,
    transform_bounds: &Option<TokenStream>,
    tokens: &mut TokenStream,
) {
    if let Some(derive_to_nest) = nest_data.derive_to_nest.as_ref() {
        let transform_to_nest = GenTransformToNest {
            variant: fallibility.clone().into(),
            transform_type: transform.clone(),
            transform_generic_bounds: transform_bounds.clone(),
            data_ident: source_ident.clone(),
            nest_fields: nest_data.fields.iter().map(|f| f.into()).collect(),
            source_field_types: nest_data.source_types(),
            nest_struct_ident: nest_data.ident.clone(),
            nest_value_type: derive_to_nest.nest_value.clone(),
            optional: derive_to_nest.options_field_if_optional.clone().map(|options_field_name | GenTransformToNestOptional { options_field_name }),
        };
        transform_to_nest.to_tokens(tokens);
    }
}
