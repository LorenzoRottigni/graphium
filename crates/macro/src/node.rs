use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;
use syn::{FnArg, Ident, ItemFn, LitStr, Pat, ReturnType, Token, Type, parse_macro_input};

use crate::shared::{MetricsSpec, NodeDef, ParamKind, parse_metric_name, pascal_case};

// Node expansion is intentionally simple now.
// A node macro only validates the user function and generates a thin wrapper
// exposing a uniform `__graphium_run` entry point. Artifact propagation is
// handled entirely by `graph!`.

/// Parses a user node function and emits its generated node wrapper type.
pub fn expand(input: TokenStream) -> TokenStream {
    let mut func = parse_macro_input!(input as ItemFn);
    let metrics = extract_metrics_from_attrs(&mut func.attrs);
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

/// Extracts the compile-time metadata the graph macro needs from a node
/// function signature.
fn parse_node_def(func: &ItemFn, metrics: MetricsSpec) -> NodeDef {
    let fn_name = func.sig.ident.clone();
    let struct_name = format_ident!("{}", pascal_case(&fn_name));

    let mut ctx_type: Option<Type> = None;
    let mut ctx_mut = false;
    let mut inputs = Vec::new();
    let mut param_kinds = Vec::new();

    for arg in func.sig.inputs.iter() {
        let FnArg::Typed(pat) = arg else {
            panic!("unexpected receiver in node function");
        };

        let Pat::Ident(pat_ident) = &*pat.pat else {
            panic!("expected ident pattern for node input");
        };

        let name = pat_ident.ident.to_string();
        if name == "ctx" || name == "_ctx" {
            if ctx_type.is_some() {
                panic!("node function must declare at most one context parameter");
            }

            let Type::Reference(ctx_ref) = &*pat.ty else {
                panic!("context parameter must be `&Context` or `&mut Context`");
            };

            ctx_type = Some((*ctx_ref.elem).clone());
            ctx_mut = ctx_ref.mutability.is_some();
            param_kinds.push(ParamKind::Ctx);
            continue;
        }

        if let Type::Reference(reference) = &*pat.ty {
            if reference.mutability.is_some() {
                panic!(
                    "node input `{}` must be shared; mutable references are not supported",
                    pat_ident.ident
                );
            }
        }

        let index = inputs.len();
        inputs.push((pat_ident.ident.clone(), (*pat.ty).clone()));
        param_kinds.push(ParamKind::Input(index));
    }

    let return_ty = match &func.sig.output {
        ReturnType::Type(_, ty) => Some((**ty).clone()),
        ReturnType::Default => None,
    };
    let return_is_result = return_ty.as_ref().is_some_and(is_result_type);

    validate_return_type(&return_ty);

    NodeDef {
        fn_name,
        struct_name,
        ctx_type,
        ctx_mut,
        inputs,
        param_kinds,
        return_ty,
        metrics,
        return_is_result,
    }
}

/// Rejects borrowed return types so artifacts always remain owned while the
/// graph is propagating them between nodes.
fn validate_return_type(return_ty: &Option<Type>) {
    match return_ty {
        Some(Type::Reference(_)) => panic!("node return type must be owned (no references)"),
        Some(Type::Tuple(tuple)) => {
            for elem in &tuple.elems {
                if matches!(elem, Type::Reference(_)) {
                    panic!("node tuple return types must be owned (no references)");
                }
            }
        }
        _ => {}
    }
}

fn is_result_type(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };

    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Result")
}

fn extract_metrics_from_attrs(attrs: &mut Vec<syn::Attribute>) -> MetricsSpec {
    let mut metrics = MetricsSpec::default();
    attrs.retain(|attr| {
        if !attr.path().is_ident("metrics") {
            return true;
        }

        let parser = Punctuated::<LitStr, Token![,]>::parse_terminated;
        let values = attr
            .parse_args_with(parser)
            .unwrap_or_else(|_| panic!("failed to parse #[metrics(...)] list"));

        for value in values {
            let apply = parse_metric_name(value.value().as_str()).unwrap_or_else(|| {
                panic!("unsupported metric `{}`; allowed: performance, errors, count, caller, success_rate, fail_rate", value.value())
            });
            apply(&mut metrics);
        }

        false
    });
    metrics
}

fn metric_config_tokens(metrics: MetricsSpec) -> proc_macro2::TokenStream {
    let performance = metrics.performance;
    let errors = metrics.errors;
    let count = metrics.count;
    let caller = metrics.caller;
    let success_rate = metrics.success_rate;
    let fail_rate = metrics.fail_rate;

    quote! {
        ::graphium::metrics::MetricConfig {
            performance: #performance,
            errors: #errors,
            count: #count,
            caller: #caller,
            success_rate: #success_rate,
            fail_rate: #fail_rate,
        }
    }
}
