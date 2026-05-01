use super::*;

// !- TransformToNestTrait

#[derive(Debug, Copy, Clone)]
pub(crate) struct TransformToNestTrait;

impl TransformTrait for TransformToNestTrait {
    fn trait_name() -> Ident { format_ident!("TransformToNest") }
    fn trait_fn() -> Ident { format_ident!("transform_to_nest") }
}

pub(crate) type TransformToNestVariant = TraitFallibility<TransformToNestTrait>;

// !- GenTransformToNestOptional

#[derive(Debug, Clone)]
pub(crate) struct GenTransformToNestOptional {
    /// Name of the field on [`Transform::Options`] used to control nest inclusion
    pub(crate) options_field_name: Ident,
}

// !- GenTransformToNest

/// Generates a [`shrinkwrap::transform_to_nest`] trait impl into the wrapper of a layered nest
#[derive(Debug, Clone)]
pub(crate) struct GenTransformToNest {
    /// The trait variant
    pub(crate) variant: TransformToNestVariant,

    /// The type of the user-defined struct implementing [`shrinkwrap::Transform`]
    pub(crate) transform_type: Path,

    /// Generic bounds for `transform_type`
    pub(crate) transform_generic_bounds: Option<TokenStream>,

    /// Ident of the source data struct
    pub(crate) data_ident: Ident,

    /// Fields included in the nest
    pub(crate) nest_fields: Vec<GenStructField>,

    /// List of (nest_type, source field) types (only fields that are actually included in this nest).
    /// Must already be de-duplicated.
    pub(crate) field_source_type_pairings: Vec<(Path, Type)>,

    /// Struct type for the nest.
    pub(crate) nest_struct_ident: Ident,

    /// Whether or not the destination nest is optional, inlcudes config for optional handling
    pub(crate) optional: Option<GenTransformToNestOptional>,
}

impl GenTransformToNest {
    fn build_value_trait(&self) -> BuildNestValueVariant {
        BuildNestValueVariant::from(self.variant.fallibility.clone())
    }

    fn trait_bounds(&self) -> TokenStream {
        let mut tokens = TokenStream::default();
        for (field_value_type, source_type) in &self.field_source_type_pairings {
            tokens.extend(match &self.variant.fallibility {
                Fallibility::Infallible => quote! {
                    Self: ::shrinkwrap::BuildNestValue<#source_type, #field_value_type>,
                },
                Fallibility::Fallible { error_type } => quote! {
                    Self: ::shrinkwrap::TryBuildNestValue<#source_type, #field_value_type, Error = #error_type>,
                }
            });
        }

        tokens
    }
    fn nest_full_type(&self) -> TokenStream {
        let nest_ident = &self.nest_struct_ident;

        if self.optional.is_none() {
            quote! { #nest_ident }
        } else {
            quote! { Option<#nest_ident> }
        }
    }
    fn associated_types(&self) -> TokenStream {
        let data_ident = &self.data_ident;
        let fallibility_associated_types = self.variant.fallibility_associated_types();

        quote! {
            type Data = #data_ident;
            #fallibility_associated_types
        }
    }

    fn field_assignments(&self) -> TokenStream {
        let mut tokens = TokenStream::default();
        let build_value_trait = BuildNestValueVariant::from(self.variant.fallibility.clone());
        let build_value_trait_fn = build_value_trait.trait_fn();
        let build_value_call_suffix = build_value_trait.trait_fn_call_suffix();

        for field in &self.nest_fields {
            let field_name = &field.name;
            let field_tokens = quote! {
                #field_name: self.#build_value_trait_fn(&data.#field_name, options)#build_value_call_suffix,
            };
            tokens.extend(field_tokens);
        }

        tokens
    }
    fn return_type(&self) -> TokenStream {
        let nest_ident = &self.nest_struct_ident;
        let mut return_type = quote!(#nest_ident);

        if self.optional.is_some() {
            return_type = quote!(Option<#return_type>);
        }

        if let Fallibility::Fallible { error_type } = &self.variant.fallibility {
            return_type = quote!(Result<#return_type, #error_type>);
        }

        return_type
    }
    fn return_statement(&self, mut out: TokenStream) -> TokenStream {
        if self.variant.is_fallible() {
            out = quote!(Ok(#out));
        }

        if let Some(optional_config) = self.optional.as_ref() {
            let option_name = &optional_config.options_field_name;
            out = quote!(options.#option_name.then(|| #out ));
            if self.variant.is_fallible() {
                out.extend(quote!(.transpose()));
            }
        }

        out
    }
}
impl ToTokens for GenTransformToNest {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let nest_ident = &self.nest_struct_ident;
        let nest_full_type = self.nest_full_type();
        let trait_name = self.variant.trait_name();
        let trait_fn = self.variant.trait_fn();
        let trait_bounds = self.trait_bounds();
        let transform_type = &self.transform_type;
        let transform_generic_bounds = &self.transform_generic_bounds;
        let associated_types = self.associated_types();
        let field_assignments = self.field_assignments();
        let build_value_trait_name = self.build_value_trait().trait_name();

        let nest_definition = quote! {
            #nest_ident {
                #field_assignments
            }
        };
        let return_type = self.return_type();
        let return_statement = self.return_statement(nest_definition);

        tokens.extend(quote! {
            #[automatically_derived]
            impl #transform_generic_bounds ::shrinkwrap::#trait_name<#nest_full_type> for #transform_type
            where
                #trait_bounds
            {
                #associated_types

                fn #trait_fn(&self, data: &Self::Data, options: &Self::Options) -> #return_type {
                    use ::shrinkwrap::#build_value_trait_name;

                    #return_statement
                }
            }
        });
    }
}
