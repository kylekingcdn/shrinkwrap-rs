use proc_macro_error2::{OptionExt, ResultExt, abort_call_site};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use std::collections::{HashSet, VecDeque};
use syn::{parse2, Ident, Meta, Path};

use crate::{
    parse::{
        map_fields, parse_struct_attrs,
        types::{DeriveItemOpts, State},
    },
    serialize::types::{
        Extra, ItemVis, Nest, NestedWrapper, RootWrapper, StructCommon, StructField,
        UniversalStruct, Wrapper, WrapperType,
    },
    util::path_parse,
};

pub fn generate(derive_opts: DeriveItemOpts) -> TokenStream {
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

    let passthrough_attr_ident = Ident::new("shrinkwrap_attr", root_ident.span());

    // init state
    let mut state = State::new(global_opts, wrapper_opts, extra_opts, root_ident.clone());

    // build nest repo
    for nest in nest_opts {
        state.nest_repo.insert(nest);
    }
    let all_nest_ids = state.nest_repo.get_all_ids();

    // build map of nest fields
    let origin_fields = data.take_struct().expect("couldnt get root fields").fields;
    map_fields(
        &mut state,
        &all_nest_ids,
        origin_fields,
        &passthrough_attr_ident,
    );

    // map passthrough struct attrs
    {
        let nest_struct_attrs = parse_struct_attrs(&all_nest_ids, &passthrough_attr_ident, &attrs);
        for (nest_id, struct_attrs) in nest_struct_attrs {
            // nest ids already checked in parse fn
            let nest_info = state.nest_repo.get_by_id_mut(nest_id.as_str()).unwrap();
            nest_info.struct_attrs = struct_attrs;
        }
    }

    generate_structs(&state)
}

fn generate_structs(state: &State) -> TokenStream {
    let mut out = quote!();
    let mut impl_out = quote!();

    // generate in top-down, FIFO order
    let mut gen_queue = VecDeque::new();
    gen_queue.push_back(&state.root_ident);

    let schemars_inline_meta: Meta = parse2(quote!(schemars(inline))).unwrap();

    while let Some(origin_ident) = gen_queue.pop_front() {
        let mut nest_out = quote!();

        let extra_ident = state.extra_opts.struct_name(origin_ident);
        let wrapper_ident = state.wrapper_opts.struct_name(origin_ident);

        // add temporary storage for wrapper and extra attrs from associated nests
        let mut wrapper_attrs = Vec::new();
        let mut wrapper_attrs_seen = HashSet::new();
        let mut extra_attrs = Vec::new();
        let mut extra_attrs_seen = HashSet::new();
        let mut extra_nest_fields = Vec::new();

        // handle inline mode changes
        if state.global.inline() {
            if origin_ident != &state.root_ident {
                wrapper_attrs.push(schemars_inline_meta.to_token_stream());
            } else {
                let origin_str = origin_ident.to_string();
                wrapper_attrs.push(
                    parse2(quote!(schemars(rename = #origin_str)))
                        .expect("Unexpected error rendering #[schemars(rename = ...)] attribute"),
                );
            }
            extra_attrs.push(schemars_inline_meta.to_token_stream());
        }

        // build nests
        let origin_nests = state.nest_repo.get_children_by_origin_ident(origin_ident);
        for nest in origin_nests {
            let nest_ident = &nest.ident;

            // add wrapper passthrough attrs
            for wrapper_attr in nest.struct_attrs.wrapper() {
                let wrapper_attr_str = wrapper_attr.to_string();
                if !wrapper_attrs_seen.contains(&wrapper_attr_str) {
                    wrapper_attrs.push(wrapper_attr.clone());
                    wrapper_attrs_seen.insert(wrapper_attr_str);
                }
            }
            // add extra passthrough attrs
            for extra_attr in nest.struct_attrs.extra() {
                let extra_attr_str = extra_attr.to_string();
                if !extra_attrs_seen.contains(&extra_attr_str) {
                    extra_attrs.push(extra_attr.clone());
                    extra_attrs_seen.insert(extra_attr_str);
                }
            }
            // handle nest field in parent extra struct
            let nest_extra_base_type = match state.nest_repo.is_parent_ident(nest_ident) {
                true => state.wrapper_opts.struct_name(nest_ident),
                false => nest_ident.clone(),
            };
            // add new field to extra struct for this nest
            extra_nest_fields.push(StructField::new(
                ItemVis::Public,
                nest.opts.field_name(),
                path_parse(nest_extra_base_type.to_token_stream()),
                state.global.all_optional() || nest.opts.optional(),
                vec![],
                nest.opts.parent_field_doc.clone(),
            ));

            // init nest derives
            let mut nest_derives = state.default_derives();
            nest_derives.extend(nest.opts.derive.iter().map(|d| d.to_token_stream()));

            // init nest attrs, add automatically added attrs first, matching behaviour or wrapper/extra
            let mut nest_attrs: Vec<TokenStream> = Vec::new();
            let mut nest_attrs_seen = HashSet::new();
            if state.global.inline() {
                nest_attrs.push(quote!(schemars(inline)));
            }

            // add nest passthrough attrs
            for nest_attr in nest.struct_attrs.nest() {
                let nest_attr_str = nest_attr.to_string();
                if !nest_attrs_seen.contains(&nest_attr_str) {
                    nest_attrs.push(nest_attr.clone());
                    nest_attrs_seen.insert(nest_attr_str);
                }
            }

            // init nest common struct info
            let nest_common = StructCommon::new(
                ItemVis::Public,
                path_parse(quote!(#nest_ident)),
                nest_derives,
                nest_attrs,
                nest.opts.struct_doc.clone(),
            );

            // build nest fields
            let mut fields = Vec::<StructField>::new();
            for field_info in nest.fields.values() {
                fields.push(StructField::new(
                    ItemVis::Public,
                    field_info.name.clone(),
                    nest.opts.field_type.clone(),
                    false,
                    field_info.attrs.clone(),
                    None,
                ));
            }
            // init full nest struct and output tokens
            let nest = Nest {
                common: nest_common,
                fields,
            };
            nest_out.extend(UniversalStruct::from(nest).to_token_stream());

            // add wrapped nest to gen_queue
            if state.nest_repo.is_parent_ident(nest_ident) {
                gen_queue.push_back(nest_ident);
            }
        }

        // build extra
        let mut extra_derives = state.default_derives();
        extra_derives.extend(state.extra_opts.derive.iter().map(|d| d.to_token_stream()));
        // init extra common struct info
        let extra_common = StructCommon::new(
            ItemVis::Public,
            path_parse(quote!(#extra_ident)),
            extra_derives,
            extra_attrs
                .clone()
                .iter()
                .map(|a| a.to_token_stream())
                .collect(),
            state.extra_opts.struct_doc.clone(),
        );
        // init full extra struct and output tokens
        let extra = Extra {
            common: extra_common,
            nest_fields: extra_nest_fields,
        };

        // build wrapper
        let mut wrapper_data_field_attrs = Vec::new();
        // wrapper derives
        let mut wrapper_derives = state.default_derives();
        wrapper_derives.extend(
            state
                .wrapper_opts
                .derive
                .iter()
                .map(|d| d.to_token_stream()),
        );
        // add flatten to wrapper data attrs
        if state.wrapper_opts.flatten() {
            wrapper_data_field_attrs.push(quote!(serde(flatten)));
        }
        // handle nested/root wrapper differences
        let wrapper_subtype = if origin_ident == &state.root_ident {
            WrapperType::Root(RootWrapper {})
        } else {
            let wrapped_nest = state.nest_repo.get_by_ident(origin_ident).expect_or_abort(
                format!("Failed to resolve wrapped nest by ident: {origin_ident}").as_str(),
            );
            let wrapper_optional = state.global.all_optional() || wrapped_nest.opts.optional();
            WrapperType::Nested(NestedWrapper {
                data_source_ident: origin_ident.clone(),
                optional: wrapper_optional,
            })
        };

        // init wrapper common struct info
        let wrapper_common = StructCommon::new(
            ItemVis::Public,
            path_parse(quote!(#wrapper_ident)),
            wrapper_derives,
            wrapper_attrs
                .clone()
                .iter()
                .map(|a| a.to_token_stream())
                .collect(),
            state.wrapper_opts.struct_doc.clone(),
        );
        // init full wrapper struct and output tokens
        let wrapper = Wrapper {
            common: wrapper_common,
            data_field: StructField::new(
                ItemVis::Public,
                state.wrapper_opts.data_field_name(),
                path_parse(quote!(#origin_ident)),
                false,
                wrapper_data_field_attrs,
                state.wrapper_opts.data_field_doc.clone(),
            ),
            extra_field: StructField::new(
                ItemVis::Public,
                state.wrapper_opts.extra_field_name(),
                path_parse(quote!(#extra_ident)),
                false,
                vec![],
                state.wrapper_opts.extra_field_doc.clone(),
            ),
            wrapper_type: wrapper_subtype,
        };

        let impl_to_wrapped_with_out = generate_to_wrapped_with_impl(
            origin_ident,
            wrapper.common.ty_full(),
            extra.common.ty_full(),
            &extra.nest_fields,
        );
        impl_out.extend(impl_to_wrapped_with_out);

        // non-primary wrapper, add `TransformToNest` util impl
        // (allows for auto conversion of NestWrapper -> Nest for user TransformToNest impls)
        if origin_ident != &state.root_ident {
            let origin_path = parse2(quote!(#origin_ident)).unwrap_or_abort();
            let transform_to_nest_impl_out = generate_deeply_nested_wrapper_transform_to_nest_impl(
                state,
                &wrapper,
                &origin_path,
            );
            impl_out.extend(transform_to_nest_impl_out);
        }
        UniversalStruct::from(wrapper).to_tokens(&mut out);
        UniversalStruct::from(extra).to_tokens(&mut out);
        out.extend(nest_out);
    }
    // add impls last to keep output organized when using expand feature
    out.extend(impl_out);

    out
}

fn generate_to_wrapped_with_impl(
    origin_ident: &Ident,
    wrapper_type: &Path,
    extra_type: &Path,
    extra_fields: &Vec<StructField>,
) -> TokenStream {
    // add transform as base predicate
    let mut where_predicate_tokens = quote!(T: shrinkwrap::Transform,);
    let mut extra_field_tokens = quote!();

    // build following tokens for each nest:
    // - where predicate containing `TransformToNest` bound
    // - corresponding field within Extra struct
    for extra_field in extra_fields {
        let nest_field_name = &extra_field.name;
        let nest_full_type = extra_field.ty_full();
        where_predicate_tokens.extend(quote! {
            T: shrinkwrap::TransformToNest<#nest_full_type, Data = #origin_ident>,
        });
        extra_field_tokens.extend(quote! {
            #nest_field_name: transform.transform_to_nest(&self, options),
        });
    }

    // generate the `ToWrappedWith` impl
    quote! {
        #[automatically_derived]
        impl<T> shrinkwrap::ToWrappedWith<T> for #origin_ident
        where
            #where_predicate_tokens
        {
            type Wrapper = #wrapper_type;

            fn to_wrapped_with(self, transform: &T, options: &<T as shrinkwrap::Transform>::Options) -> Self::Wrapper {
                Self::Wrapper {
                    extra: #extra_type {
                        #extra_field_tokens
                    },
                    data: self
                }
            }
        }
    }
}

fn generate_deeply_nested_wrapper_transform_to_nest_impl(
    state: &State,
    wrapper: &Wrapper,
    nest_full_type: &Path,
) -> TokenStream {
    if let WrapperType::Nested(nested_wrapper) = &wrapper.wrapper_type {
        let wrapper_type = wrapper.common.ty_full();
        let origin_ident = &state
            .nest_repo
            .get_parent_ident(&nested_wrapper.data_source_ident)
            .expect_or_abort(
                format!(
                    "Internal derive error - failed to map nest origin for {}",
                    &nested_wrapper.data_source_ident
                )
                .as_str(),
            );
        let transform_type = &state.global.transform;
        let transform_generics = if let Some(params) = &state.global.transform_generic_params {
            quote! { <#params>}
        } else {
            TokenStream::new()
        };

        if nested_wrapper.optional {
            quote::quote! {
                #[automatically_derived]
                impl #transform_generics shrinkwrap::TransformToNest<Option<#wrapper_type>> for #transform_type {
                    type Data = #origin_ident;

                    fn transform_to_nest(&self, data: &Self::Data, options: &Self::Options) -> Option<#wrapper_type> {
                        use ::shrinkwrap::{ToNestWith, WrapDataWith};
                        let nest_data: Option<#nest_full_type> = data.to_nest_with(self, options);
                        nest_data.map(|some_nest_data| #wrapper_type::wrap_data_with(some_nest_data, self, options))
                    }
                }
            }
        } else {
            quote::quote! {
                #[automatically_derived]
                impl #transform_generics shrinkwrap::TransformToNest<#wrapper_type> for #transform_type {
                    type Data = #origin_ident;

                    fn transform_to_nest(&self, data: &Self::Data, options: &Self::Options) -> #wrapper_type {
                        use ::shrinkwrap::{ToNestWith, WrapDataWith};
                        let nest_data: #nest_full_type = data.to_nest_with(self, options);
                        #wrapper_type::wrap_data_with(nest_data, self, options)
                    }
                }
            }
        }
    } else {
        abort_call_site!(
            "Internal derive error - nested wrapper generation called on unlayered nest"
        );
    }
}
