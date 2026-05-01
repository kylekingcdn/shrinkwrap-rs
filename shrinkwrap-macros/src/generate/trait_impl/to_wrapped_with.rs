use super::*;
// use crate::serialize::types::StructField;

// !- ToWrappedWithTrait

#[derive(Debug, Copy, Clone)]
pub(crate) struct ToWrappedWithTrait;

impl TransformTrait for ToWrappedWithTrait {
    fn trait_name() -> Ident { format_ident!("ToWrappedWith") }
    fn trait_fn() -> Ident { format_ident!("to_wrapped_with") }
}

pub(crate) type ToWrappedWithVariant = TraitFallibility<ToWrappedWithTrait>;

// !- GenToWrappedWith

/// Generates a [`shrinkwrap::try_to_wrapped_with`] trait impl
#[derive(Debug, Clone)]
pub(crate) struct GenToWrappedWith {
    /// The trait variant
    pub(crate) variant: ToWrappedWithVariant,

    /// The type of the user-defined struct implementing [`shrinkwrap::Transform`]
    pub(crate) transform_type: Path,

    /// Generic bounds for `transform_type`
    pub(crate) transform_generic_bounds: Option<TokenStream>,

    /// Ident of the data (or nest) struct
    pub(crate) data_ident: Ident,

    /// The type of the associated wrapper struct
    pub(crate) wrapper_ident: Ident,

    /// The type of the associated extra struct
    pub(crate) extra_struct_ident: Ident,

    /// Fields contained by the associated wrapper's `extra` struct
    pub(crate) extra_struct_fields: Vec<GenStructField>,
}
impl GenToWrappedWith {
    fn associated_types(&self) -> TokenStream {
        let wrapper_type = &self.wrapper_ident;
        let fallibility_associated_types = self.variant.fallibility_associated_types();

        quote! {
            type Wrapper = #wrapper_type;
            #fallibility_associated_types
        }
    }
    fn opt_helper_associated_types(&self) -> TokenStream {
        let wrapper_type = &self.wrapper_ident;
        let fallibility_associated_types = self.variant.fallibility_associated_types();

        quote! {
            type Wrapper = Option<#wrapper_type>;
            #fallibility_associated_types
        }
    }
    fn return_type(&self) -> TokenStream {
        match &self.variant.fallibility {
            Fallibility::Infallible => quote! { Self::Wrapper },
            Fallibility::Fallible { .. } => quote! { Result<Self::Wrapper, Self::Error> }
        }
    }

    /// Generates the `where` conditions used for the blanket impl
    fn gen_where_predicates(&self) -> TokenStream {
        // always add `shrinkwrap::Transform` bound to implementing type
        let mut out = quote!(T: ::shrinkwrap::Transform,);

        let data_ident = &self.data_ident;

        for extra_field in &self.extra_struct_fields {
            // handles wrapping nest type in Option if required
            let nest_full_type = &extra_field.ty;

            out.extend(match &self.variant.fallibility {
                Fallibility::Infallible => quote! {
                    T: ::shrinkwrap::TransformToNest<#nest_full_type, Data = #data_ident>,
                },
                Fallibility::Fallible { error_type } => quote! {
                    T: ::shrinkwrap::TryTransformToNest<#nest_full_type, Data = #data_ident, Error = #error_type>,
                },
            });
        }
        out
    }

    fn map_opt_helper_return(&self, ret_tokens: TokenStream) -> TokenStream {
        if self.variant.is_fallible() {
            quote! { #ret_tokens.transpose() }
        } else {
            ret_tokens
        }
    }

    /// Generates the tokens for all field assignments of the associated `extra`
    /// struct. Each `extra` field is a nest variant struct
    fn gen_extra_fields_assignments(&self) -> TokenStream {
        let mut out = quote! {};

        let transform_to_nest_trait = TransformToNestVariant::from(self.variant.fallibility.clone());
        let trait_fn = transform_to_nest_trait.trait_fn();
        let trait_fn_call_suffix = transform_to_nest_trait.trait_fn_call_suffix();

        for extra_field in &self.extra_struct_fields {
            let field_name = &extra_field.name;

            out.extend(quote! {
                #field_name: transform.#trait_fn(&self, options)#trait_fn_call_suffix,
            });
        }

        out
    }
}
impl ToTokens for GenToWrappedWith {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let data_ident = &self.data_ident;
        let extra_struct_type = &self.extra_struct_ident;
        let extra_struct_field_assignments = self.gen_extra_fields_assignments();
        let trait_name = self.variant.trait_name();
        let trait_fn = self.variant.trait_fn();
        let impl_bounds = self.gen_where_predicates();
        let transform_type = &self.transform_type;
        let transform_generic_bounds = self.transform_generic_bounds.as_ref().map(|params| quote!(<#params>)).unwrap_or_default();
        let associated_types = self.associated_types();

        let return_type = self.return_type();
        let return_statement = self.variant.fallibility.map_return(quote! {
            Self::Wrapper {
                extra: #extra_struct_type {
                    #extra_struct_field_assignments
                },
                data: self
            }
        });

        tokens.extend(quote! {
            #[automatically_derived]
            impl<T> ::shrinkwrap::#trait_name<T> for #data_ident
            where
                #impl_bounds
            {
                #associated_types

                fn #trait_fn(
                    self,
                    transform: &T,
                    options: &<T as ::shrinkwrap::Transform>::Options,
                ) -> #return_type {
                    #return_statement
                }
            }
        });

        // add impl to allow calling wrap_data_with directly on Option
        let opt_helper_associated_types = self.opt_helper_associated_types();
        let opt_helper_return_statement  = self.map_opt_helper_return(quote! {
            self.map(|data| data.#trait_fn(transform, options))
        });
        tokens.extend(quote! {
            #[automatically_derived]
            impl #transform_generic_bounds ::shrinkwrap::#trait_name<#transform_type> for Option<#data_ident> {
                #opt_helper_associated_types

                fn #trait_fn(
                    self,
                    transform: &#transform_type,
                    options: &<#transform_type as ::shrinkwrap::Transform>::Options,
                ) -> #return_type {
                    #opt_helper_return_statement
                }
            }
        });
    }
}
