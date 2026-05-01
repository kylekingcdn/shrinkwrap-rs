use super::*;

// !- GenTransformToDeepNest

/// Generates a [`shrinkwrap::transform_to_nest`] trait impl into the wrapper of a layered nest
#[derive(Debug, Clone)]
pub(crate) struct GenTransformToDeepNest {
    /// The trait variant
    pub(crate) variant: TransformToNestVariant,

    /// The type of the user-defined struct implementing [`shrinkwrap::Transform`]
    pub(crate) transform_type: Path,

    /// Generic bounds for `transform_type`
    pub(crate) transform_generic_bounds: Option<TokenStream>,

    /// Ident of the data (or nest) struct
    pub(crate) data_ident: Ident,

    /// Wrapper struct type for the nest.
    ///
    /// **MUST** be provided as Option<DataWrapper> for optional nests
    pub(crate) nest_wrapper_ident: Ident,

    /// Struct type for the nest.
    pub(crate) nest_ident: Ident,

    /// Whether or not the destination nest is optional
    pub(crate) optional: bool,
}
impl GenTransformToDeepNest {
    fn nest_type(&self) -> TokenStream {
        let nest_ident = &self.nest_ident;
        match self.optional {
            true => quote! { Option<#nest_ident> },
            false => quote! { #nest_ident },
        }
    }
    fn nest_wrapper_type(&self) -> TokenStream {
        let wrapper_ident = &self.nest_wrapper_ident;
        match self.optional {
            true => quote! { Option<#wrapper_ident> },
            false => quote! { #wrapper_ident },
        }
    }
    fn nest_wrapper_call_type(&self) -> TokenStream {
        let wrapper_ident = &self.nest_wrapper_ident;
        match self.optional {
            true => quote! { Option::<#wrapper_ident> },
            false => quote! { #wrapper_ident },
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
    fn return_type(&self) -> TokenStream {
        let wrapper_type = self.nest_wrapper_type();
        match &self.variant.fallibility {
            Fallibility::Infallible => quote! { #wrapper_type },
            Fallibility::Fallible { .. } => quote! { Result<#wrapper_type, Self::Error> }
        }
    }
}
impl ToTokens for GenTransformToDeepNest {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let nest_type = self.nest_type();
        let wrapper_type = self.nest_wrapper_type();
        let wrapper_call_type = self.nest_wrapper_call_type();
        let trait_name = self.variant.trait_name();
        let trait_fn = self.variant.trait_fn();
        let trait_suffix = self.variant.trait_fn_call_suffix();
        let transform_type = &self.transform_type;
        let transform_generic_bounds = self.transform_generic_bounds.as_ref().map(|params| quote!(<#params>)).unwrap_or_default();
        let associated_types = self.associated_types();
        let return_type = self.return_type();

        let to_nest_with_trait_name = self.variant.fallibility.trait_name(format_ident!("ToNestWith"));
        let to_nest_with_trait_fn = self.variant.fallibility.trait_fn(format_ident!("to_nest_with"));

        let wrap_data_with_name = self.variant.fallibility.trait_name(format_ident!("WrapDataWith"));
        let wrap_data_with_fn = self.variant.fallibility.trait_fn(format_ident!("wrap_data_with"));

        tokens.extend(quote! {
            #[automatically_derived]
            impl #transform_generic_bounds ::shrinkwrap::#trait_name<#wrapper_type> for #transform_type {
                #associated_types

                fn #trait_fn(
                    &self,
                    data: &Self::Data,
                    options: &Self::Options,
                ) -> #return_type {
                    use ::shrinkwrap::{#to_nest_with_trait_name, #wrap_data_with_name};

                    let nest_data: #nest_type = data.#to_nest_with_trait_fn(self, options)#trait_suffix;
                    #wrapper_call_type::#wrap_data_with_fn(nest_data, self, options)
                }
            }
        });
    }
}
