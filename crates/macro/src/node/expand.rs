//! Main expansion logic for the `node!` procedural macro.
//!
//! This module contains the top-level `expand` function that orchestrates
//! the transformation of a user-defined node function into a wrapper type
//! with the standard `__graphium_run` entry point.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Ident, Type};

use crate::shared::ParamKind;

use super::metrics::metric_config_tokens;
use super::parse::parse_node_def;

/// Parses a user node function and emits its generated node wrapper type.
///
/// The expansion generates:
/// - The original function preserved as-is
/// - A wrapper struct with the same name in PascalCase
/// - A `NAME` constant for introspection
/// - Optional `__graphium_node_metrics` function if metrics are enabled
/// - `__graphium_run` for sync nodes or `__graphium_run_async` for async nodes
pub fn expand(input: TokenStream) -> TokenStream {
    let mut func = parse_macro_input!(input as syn::ItemFn);
    let metrics = super::metrics::extract_metrics_from_attrs(&mut func.attrs);
    let node_def = parse_node_def(&func, metrics);

    let fn_name = &node_def.fn_name;
    let struct_name = &node_def.struct_name;
    let is_async = func.sig.asyncness.is_some();
    let ctx_generic = if node_def.ctx_type.is_none() {
        quote! { <Ctx> }
    } else {
        quote! {}
    };
    let ctx_param = match &node_def.ctx_type {
        Some(ctx_type) => {
            if node_def.ctx_mut {
                quote! { &mut #ctx_type }
            } else {
                quote! { & #ctx_type }
            }
        }
        None => quote! { &Ctx },
    };
    let input_idents: Vec<Ident> = node_def
        .inputs
        .iter()
        .map(|(ident, _)| ident.clone())
        .collect();
    let input_types: Vec<Type> = node_def.inputs.iter().map(|(_, ty)| ty.clone()).collect();
    let return_sig = match &node_def.return_ty {
        Some(ty) => quote! { -> #ty },
        None => quote! {},
    };
    let metrics_enabled = node_def.metrics.enabled();
    let track_panics = node_def.metrics.track_panics_sync();
    let track_panic_sync = track_panics && metrics_enabled;
    let returns_result = node_def.return_is_result;

    let metrics_config_tokens = metric_config_tokens(node_def.metrics);
    let metrics_defs = if metrics_enabled {
        quote! {
            fn __graphium_node_metrics() -> &'static ::graphium::metrics::NodeMetricsHandle {
                static METRICS: ::std::sync::OnceLock<::graphium::metrics::NodeMetricsHandle> = ::std::sync::OnceLock::new();
                METRICS.get_or_init(|| {
                    ::graphium::metrics::node_metrics(
                        module_path!(),
                        Self::NAME,
                        module_path!(),
                        #metrics_config_tokens,
                    )
                })
            }
        }
    } else {
        quote! {}
    };
    let call_args: Vec<proc_macro2::TokenStream> = node_def
        .param_kinds
        .iter()
        .map(|kind| match kind {
            ParamKind::Ctx => quote! { ctx },
            ParamKind::Input(index) => {
                let ident = &input_idents[*index];
                quote! { #ident }
            }
        })
        .collect();

    let sync_run = if is_async {
        quote! {}
    } else {
        let sync_body = if metrics_enabled {
            if returns_result {
                if track_panic_sync {
                    quote! {
                        let __graphium_metrics = Self::__graphium_node_metrics();
                        let __graphium_start = __graphium_metrics.start_timer();
                        let __graphium_result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| #fn_name(#( #call_args ),*)));
                        match __graphium_result {
                            Ok(value) => {
                                if value.is_err() {
                                    __graphium_metrics.record_failure(__graphium_start);
                                } else {
                                    __graphium_metrics.record_success(__graphium_start);
                                }
                                value
                            }
                            Err(payload) => {
                                __graphium_metrics.record_failure(__graphium_start);
                                ::std::panic::resume_unwind(payload)
                            }
                        }
                    }
                } else {
                    quote! {
                        let __graphium_metrics = Self::__graphium_node_metrics();
                        let __graphium_start = __graphium_metrics.start_timer();
                        let value = #fn_name(#( #call_args ),*);
                        if value.is_err() {
                            __graphium_metrics.record_failure(__graphium_start);
                        } else {
                            __graphium_metrics.record_success(__graphium_start);
                        }
                        value
                    }
                }
            } else if track_panic_sync {
                quote! {
                    let __graphium_metrics = Self::__graphium_node_metrics();
                    let __graphium_start = __graphium_metrics.start_timer();
                    let __graphium_result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| #fn_name(#( #call_args ),*)));
                    match __graphium_result {
                        Ok(value) => {
                            __graphium_metrics.record_success(__graphium_start);
                            value
                        }
                        Err(payload) => {
                            __graphium_metrics.record_failure(__graphium_start);
                            ::std::panic::resume_unwind(payload)
                        }
                    }
                }
            } else {
                quote! {
                    let __graphium_metrics = Self::__graphium_node_metrics();
                    let __graphium_start = __graphium_metrics.start_timer();
                    let value = #fn_name(#( #call_args ),*);
                    __graphium_metrics.record_success(__graphium_start);
                    value
                }
            }
        } else {
            quote! { #fn_name(#( #call_args ),*) }
        };

        quote! {
            pub fn __graphium_run #ctx_generic(
                ctx: #ctx_param,
                #( #input_idents: #input_types ),*
            ) #return_sig {
                println!("Running node: {}", Self::NAME);
                #sync_body
            }
        }
    };

    let async_run = if is_async {
        let async_body = if metrics_enabled {
            if returns_result {
                quote! {
                    let __graphium_metrics = Self::__graphium_node_metrics();
                    let __graphium_start = __graphium_metrics.start_timer();
                    let value = #fn_name(#( #call_args ),*).await;
                    if value.is_err() {
                        __graphium_metrics.record_failure(__graphium_start);
                    } else {
                        __graphium_metrics.record_success(__graphium_start);
                    }
                    value
                }
            } else {
                quote! {
                    let __graphium_metrics = Self::__graphium_node_metrics();
                    let __graphium_start = __graphium_metrics.start_timer();
                    let value = #fn_name(#( #call_args ),*).await;
                    __graphium_metrics.record_success(__graphium_start);
                    value
                }
            }
        } else {
            quote! { #fn_name(#( #call_args ),*).await }
        };
        quote! {
            pub async fn __graphium_run_async #ctx_generic(
                ctx: #ctx_param,
                #( #input_idents: #input_types ),*
            ) #return_sig {
                println!("Running node: {}", Self::NAME);
                #async_body
            }
        }
    } else {
        let async_body = if metrics_enabled {
            if returns_result {
                quote! {
                    let __graphium_metrics = Self::__graphium_node_metrics();
                    let __graphium_start = __graphium_metrics.start_timer();
                    let value = #fn_name(#( #call_args ),*);
                    if value.is_err() {
                        __graphium_metrics.record_failure(__graphium_start);
                    } else {
                        __graphium_metrics.record_success(__graphium_start);
                    }
                    value
                }
            } else {
                quote! {
                    let __graphium_metrics = Self::__graphium_node_metrics();
                    let __graphium_start = __graphium_metrics.start_timer();
                    let value = #fn_name(#( #call_args ),*);
                    __graphium_metrics.record_success(__graphium_start);
                    value
                }
            }
        } else {
            quote! { #fn_name(#( #call_args ),*) }
        };
        quote! {
            pub async fn __graphium_run_async #ctx_generic(
                ctx: #ctx_param,
                #( #input_idents: #input_types ),*
            ) #return_sig {
                println!("Running node: {}", Self::NAME);
                #async_body
            }
        }
    };

    let expanded = quote! {
        #func

        pub struct #struct_name;

        impl #struct_name {
            pub const NAME: &'static str = stringify!(#fn_name);
            #metrics_defs

            #sync_run
            #async_run
        }
    };

    TokenStream::from(expanded)
}
