/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use proc_macro2::{Punct, TokenStream};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Data, DeriveInput, Ident, Meta, Token, Type};

struct Method {
    name: Ident,
    types: Punctuated<Type, Token![,]>,
}

impl Parse for Method {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        if input.parse::<Punct>().is_err() {
            return Ok(Method {
                name,
                types: Punctuated::new(),
            });
        }
        let types = Punctuated::<Type, Token![,]>::parse_terminated(input)?;

        Ok(Method { name, types })
    }
}

pub(super) fn derive(input: DeriveInput) -> TokenStream {
    let Data::Enum(enum_data) = &input.data else {
        return quote! {
            compile_error!("derive(AnyConfig) can only be used on enum types");
        };
    };
    let arm_attrs = enum_data
        .variants
        .iter()
        .map(|v| {
            let attrs = &v.attrs;
            quote! {
                #( #attrs ),*
            }
        })
        .collect::<Vec<_>>();
    let arm_idents = enum_data
        .variants
        .iter()
        .map(|v| &v.ident)
        .collect::<Vec<_>>();

    let mut fn_expands = Vec::new();
    for attr in &input.attrs {
        let Meta::List(list) = &attr.meta else {
            continue;
        };

        let is_async = if list.path.is_ident("def_async_fn") {
            true
        } else if list.path.is_ident("def_fn") {
            false
        } else {
            continue;
        };

        let Ok(mut v) = syn::parse2::<Method>(list.tokens.clone()) else {
            continue;
        };

        let fn_name = v.name;
        let expanded = match v.types.pop() {
            Some(pair) => {
                let fn_result = pair.into_value();
                if v.types.is_empty() {
                    if is_async {
                        derive_async_fn_r(&fn_name, fn_result, &arm_attrs, &arm_idents)
                    } else {
                        derive_fn_r(&fn_name, fn_result, &arm_attrs, &arm_idents)
                    }
                } else {
                    let param_names = (0..v.types.len())
                        .map(|n| format_ident!("v{n}"))
                        .collect::<Vec<_>>();
                    let param_types = v.types.pairs().map(|v| v.into_value());

                    let fn_params = quote! {
                        #( #param_names : #param_types ),*
                    };
                    let call_args = quote! {
                        #( #param_names ),*
                    };

                    if is_async {
                        derive_async_fn_p_r(
                            &fn_name,
                            fn_params,
                            fn_result,
                            &arm_attrs,
                            &arm_idents,
                            call_args,
                        )
                    } else {
                        derive_fn_p_r(
                            &fn_name,
                            fn_params,
                            fn_result,
                            &arm_attrs,
                            &arm_idents,
                            call_args,
                        )
                    }
                }
            }
            None => {
                if is_async {
                    derive_async_fn(&fn_name, &arm_attrs, &arm_idents)
                } else {
                    derive_fn(&fn_name, &arm_attrs, &arm_idents)
                }
            }
        };

        fn_expands.extend(expanded);
    }

    let name = input.ident;
    quote! {
        impl #name {
            #( #fn_expands )*
        }
    }
}

fn derive_fn(fn_name: &Ident, arm_attrs: &[TokenStream], arm_idents: &[&Ident]) -> TokenStream {
    quote! {
        pub(crate) fn #fn_name(&self) {
            match self {
                #(
                    #arm_attrs
                    Self::#arm_idents(c) => c.#fn_name(),
                )*
            }
        }
    }
}

fn derive_async_fn(
    fn_name: &Ident,
    arm_attrs: &[TokenStream],
    arm_idents: &[&Ident],
) -> TokenStream {
    quote! {
        pub(crate) async fn #fn_name(&self) {
            match self {
                #(
                    #arm_attrs
                    Self::#arm_idents(c) => c.#fn_name().await,
                )*
            }
        }
    }
}

fn derive_fn_r(
    fn_name: &Ident,
    fn_result: Type,
    arm_attrs: &[TokenStream],
    arm_idents: &[&Ident],
) -> TokenStream {
    quote! {
        pub(crate) fn #fn_name(&self) -> #fn_result {
            match self {
                #(
                    #arm_attrs
                    Self::#arm_idents(c) => c.#fn_name(),
                )*
            }
        }
    }
}

fn derive_async_fn_r(
    fn_name: &Ident,
    fn_result: Type,
    arm_attrs: &[TokenStream],
    arm_idents: &[&Ident],
) -> TokenStream {
    quote! {
        pub(crate) async fn #fn_name(&self) -> #fn_result {
            match self {
                #(
                    #arm_attrs
                    Self::#arm_idents(c) => c.#fn_name().await,
                )*
            }
        }
    }
}

fn derive_fn_p_r(
    fn_name: &Ident,
    fn_params: TokenStream,
    fn_result: Type,
    arm_attrs: &[TokenStream],
    arm_idents: &[&Ident],
    call_args: TokenStream,
) -> TokenStream {
    quote! {
        pub(crate) fn #fn_name(&self, #fn_params) -> #fn_result {
            match self {
                #(
                    #arm_attrs
                    Self::#arm_idents(c) => c.#fn_name( #call_args ),
                )*
            }
        }
    }
}

fn derive_async_fn_p_r(
    fn_name: &Ident,
    fn_params: TokenStream,
    fn_result: Type,
    arm_attrs: &[TokenStream],
    arm_idents: &[&Ident],
    call_args: TokenStream,
) -> TokenStream {
    quote! {
        pub(crate) async fn #fn_name(&self, #fn_params) -> #fn_result {
            match self {
                #(
                    #arm_attrs
                    Self::#arm_idents(c) => c.#fn_name( #call_args ).await,
                )*
            }
        }
    }
}
