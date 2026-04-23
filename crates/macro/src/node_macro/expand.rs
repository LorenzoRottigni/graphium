//! Main expansion logic for the `node!` procedural macro.
//!
//! This module contains the top-level `expand` function that orchestrates
//! the transformation of a user-defined node function into a wrapper type
//! with the standard `run` entry point.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Ident, Path, Type, parse_macro_input};

use crate::ir::ParamKind;

use super::metrics::metric_config_tokens;
use super::parse::parse_node_def;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlaygroundParseKind {
    String,
    Bool,
    FromStr,
}

fn playground_parse_kind(ty: &syn::Type) -> Option<PlaygroundParseKind> {
    let syn::Type::Path(type_path) = ty else {
        return None;
    };
    if type_path.qself.is_some() {
        return None;
    }
    let last = type_path.path.segments.last()?.ident.to_string();
    match last.as_str() {
        "String" => Some(PlaygroundParseKind::String),
        "bool" => Some(PlaygroundParseKind::Bool),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
        | "usize" | "f32" | "f64" => Some(PlaygroundParseKind::FromStr),
        _ => None,
    }
}

fn slugify(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut prev_dash = false;
    for ch in value.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn output_supported(return_ty: &Option<Type>, returns_result: bool) -> bool {
    let Some(return_ty) = return_ty else {
        return true;
    };
    if returns_result {
        let syn::Type::Path(type_path) = return_ty else {
            return false;
        };
        let last = type_path.path.segments.last();
        let Some(last) = last else {
            return false;
        };
        if last.ident != "Result" {
            return false;
        }
        let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
            return false;
        };
        let mut type_args = args.args.iter().filter_map(|arg| {
            if let syn::GenericArgument::Type(ty) = arg {
                Some(ty)
            } else {
                None
            }
        });
        let ok_ty = type_args.next();
        let err_ty = type_args.next();
        return ok_ty.is_some_and(|ty| playground_parse_kind(ty).is_some())
            && err_ty.is_some_and(|ty| playground_parse_kind(ty).is_some());
    }

    match return_ty {
        syn::Type::Tuple(tuple) => tuple
            .elems
            .iter()
            .all(|ty| playground_parse_kind(ty).is_some()),
        _ => playground_parse_kind(return_ty).is_some(),
    }
}

/// Parses a user node function and emits its generated node wrapper type.
///
/// The expansion generates:
/// - The original function preserved as-is
/// - A wrapper struct with the same name in PascalCase
/// - A `NAME` constant for introspection
/// - Optional `__graphium_node_metrics` function if metrics are enabled
/// - `run` for sync nodes and `run_async` for async nodes
pub fn expand(input: TokenStream) -> TokenStream {
    let raw_schema_string = input.to_string();
    let raw_schema_lit = syn::LitStr::new(&raw_schema_string, proc_macro2::Span::call_site());

    let mut func = parse_macro_input!(input as syn::ItemFn);
    let name_override = extract_name_from_attrs(&mut func.attrs);
    let tags = extract_tags_from_attrs(&mut func.attrs);
    let (deprecated, deprecated_reason) = extract_deprecation_from_attrs(&func.attrs);
    let tests = extract_tests_from_attrs(&mut func.attrs);
    let metrics = super::metrics::extract_metrics_from_attrs(&mut func.attrs);
    let mut node_def = parse_node_def(&func, metrics);
    if let Some(ident) = name_override {
        node_def.struct_name = ident;
    }
    node_def.tags = tags;
    node_def.deprecated = deprecated;
    node_def.deprecated_reason = deprecated_reason;

    let fn_name = &node_def.fn_name;
    let struct_name = &node_def.struct_name;
    let is_async = func.sig.asyncness.is_some();
    let docs_tokens = match &node_def.docs {
        Some(value) => {
            let lit = syn::LitStr::new(value, proc_macro2::Span::call_site());
            quote! { ::std::option::Option::Some(#lit.to_string()) }
        }
        None => quote! { ::std::option::Option::None },
    };
    let tag_tokens: Vec<_> = node_def
        .tags
        .iter()
        .map(|t| syn::LitStr::new(t, proc_macro2::Span::call_site()))
        .collect();
    let deprecated_token = node_def.deprecated;
    let deprecated_reason_tokens = match &node_def.deprecated_reason {
        Some(value) => {
            let lit = syn::LitStr::new(value, proc_macro2::Span::call_site());
            quote! { ::std::option::Option::Some(#lit.to_string()) }
        }
        None => quote! { ::std::option::Option::None },
    };
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

    let id_literal = syn::LitStr::new(
        &slugify(&struct_name.to_string()),
        proc_macro2::Span::call_site(),
    );
    let ctx_ty_tokens = match &node_def.ctx_type {
        Some(ty) => quote! { stringify!(#ty) },
        None => quote! { "()" },
    };

    let playground_inputs: Vec<_> = node_def
        .inputs
        .iter()
        .map(|(ident, ty)| {
            quote! { ::graphium::PlaygroundParam { name: stringify!(#ident), ty: stringify!(#ty) } }
        })
        .collect();

    let output_params: Vec<_> = match &node_def.return_ty {
        None => Vec::new(),
        Some(ty) => vec![quote! {
            ::graphium::PlaygroundParam { name: "output", ty: stringify!(#ty) }
        }],
    };

    let playground_supported = !is_async
        && node_def
            .inputs
            .iter()
            .all(|(_, ty)| playground_parse_kind(ty).is_some())
        && output_supported(&node_def.return_ty, returns_result);

    let play_inputs_ident = syn::Ident::new(
        &format!("__GRAPHIM_NODE_PLAY_INPUTS_{}", struct_name),
        proc_macro2::Span::call_site(),
    );
    let play_outputs_ident = syn::Ident::new(
        &format!("__GRAPHIM_NODE_PLAY_OUTPUTS_{}", struct_name),
        proc_macro2::Span::call_site(),
    );

    let playground_impl = {
        let (parse_bindings, args) = if playground_supported {
            let mut parse_bindings = Vec::new();
            let mut args = Vec::new();
            for (ident, ty) in &node_def.inputs {
                let key = ident.to_string();
                let raw_ident = syn::Ident::new(
                    &format!("__graphium_ui_raw_{key}"),
                    proc_macro2::Span::call_site(),
                );
                let var_ident = syn::Ident::new(
                    &format!("__graphium_ui_{key}"),
                    proc_macro2::Span::call_site(),
                );
                let kind = playground_parse_kind(ty).unwrap();
                let parse_expr = match kind {
                    PlaygroundParseKind::String => quote! { #raw_ident.to_string() },
                    PlaygroundParseKind::Bool => quote! {{
                        match #raw_ident.trim().to_ascii_lowercase().as_str() {
                            "true" | "1" | "yes" | "on" => true,
                            "false" | "0" | "no" | "off" => false,
                            other => return ::std::result::Result::Err(format!("invalid input `{}`: expected bool, got `{}`", #key, other)),
                        }
                    }},
                    PlaygroundParseKind::FromStr => quote! {{
                        #raw_ident
                            .trim()
                            .parse::<#ty>()
                            .map_err(|e| format!("invalid input `{}`: {}", #key, e))?
                    }},
                };
                let raw_binding = match kind {
                    PlaygroundParseKind::Bool => quote! {
                        let #raw_ident = form.get(#key).map(|v| v.as_str()).unwrap_or("false");
                    },
                    _ => quote! {
                        let #raw_ident = form
                            .get(#key)
                            .map(|v| v.as_str())
                            .ok_or_else(|| format!("missing input `{}`", #key))?;
                    },
                };
                parse_bindings.push(quote! {
                    #raw_binding
                    let #var_ident: #ty = #parse_expr;
                });
                args.push(quote! { #var_ident });
            }
            (parse_bindings, args)
        } else {
            (Vec::new(), Vec::new())
        };

        let ctx_setup = match &node_def.ctx_type {
            None => quote! {
                let ctx = ();
                let __graphium_result = #struct_name::run::<()>(&ctx, #( #args ),* );
            },
            Some(ctx_ty) => {
                if node_def.ctx_mut {
                    quote! {
                        let mut ctx: #ctx_ty = ::core::default::Default::default();
                        let __graphium_result = #struct_name::run(&mut ctx, #( #args ),* );
                    }
                } else {
                    quote! {
                        let ctx: #ctx_ty = ::core::default::Default::default();
                        let __graphium_result = #struct_name::run(&ctx, #( #args ),* );
                    }
                }
            }
        };

        let output_format = match &node_def.return_ty {
            None => quote! { ::std::result::Result::Ok("ok".to_string()) },
            Some(_ty) => {
                if returns_result {
                    quote! {
                        match __graphium_result {
                            Ok(value) => ::std::result::Result::Ok(format!("{:?}", value)),
                            Err(err) => ::std::result::Result::Err(format!("{:?}", err)),
                        }
                    }
                } else {
                    quote! { ::std::result::Result::Ok(format!("{:?}", __graphium_result)) }
                }
            }
        };

        if playground_supported {
            quote! {
                fn __graphium_playground_run(
                    form: &::std::collections::HashMap<String, String>,
                ) -> ::std::result::Result<String, String> {
                    #( #parse_bindings )*
                    #ctx_setup
                    #output_format
                }
            }
        } else {
            quote! {
                fn __graphium_playground_run(
                    _form: &::std::collections::HashMap<String, String>,
                ) -> ::std::result::Result<String, String> {
                    ::std::result::Result::Err("playground execution is not supported for this node (requires a sync node and supported input types)".to_string())
                }
            }
        }
    };

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
    let ctx_access = match &node_def.ctx_type {
        None => quote! { ::graphium::CtxAccess::None },
        Some(_) => {
            if node_def.ctx_mut {
                quote! { ::graphium::CtxAccess::Mut }
            } else {
                quote! { ::graphium::CtxAccess::Ref }
            }
        }
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
            pub fn run #ctx_generic(
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
            pub async fn run_async #ctx_generic(
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
            pub async fn run_async #ctx_generic(
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

        impl ::core::default::Default for #struct_name {
            fn default() -> Self {
                Self
            }
        }

        impl #struct_name {
            pub const NAME: &'static str = stringify!(#fn_name);
            pub const CTX_ACCESS: ::graphium::CtxAccess = #ctx_access;
            #metrics_defs
            #playground_impl

            #sync_run
            #async_run
        }

        static #play_inputs_ident: &[::graphium::PlaygroundParam] = &[ #( #playground_inputs ),* ];
        static #play_outputs_ident: &[::graphium::PlaygroundParam] = &[ #( #output_params ),* ];

        #[cfg(feature = "export")]
        impl #struct_name {
            pub fn test_runs() -> ::std::vec::Vec<::graphium::dto::TestRun> {
                vec![
                    #(
                        ::graphium::dto::TestRun {
                            dto: ::graphium::dto::TestDto::new(
                                ::graphium::dto::TestKindDto::Node,
                                #tests::NAME,
                                stringify!(#struct_name),
                            ),
                            schema: #tests::__graphium_ui_schema(),
                            default_values: #tests::__graphium_ui_default_values(),
                            run: #tests::__graphium_ui_run_with_args,
                        }
                    ),*
                ]
            }

            pub fn dto() -> ::graphium::dto::NodeDto {
                let schema = ::graphium::PlaygroundSchema {
                    inputs: #play_inputs_ident,
                    outputs: #play_outputs_ident,
                    context: #ctx_ty_tokens,
                };
                ::graphium::dto::NodeDto {
                    id: #id_literal.to_string(),
                    target: stringify!(#struct_name).to_string(),
                    label: stringify!(#struct_name).to_string(),
                    docs: #docs_tokens,
                    tags: vec![ #( #tag_tokens.to_string() ),* ],
                    deprecated: #deprecated_token,
                    deprecated_reason: #deprecated_reason_tokens,
                    raw_schema: ::std::option::Option::Some(#raw_schema_lit.to_string()),
                    tests: vec![
                        #(
                            ::graphium::dto::TestDto::new(
                                ::graphium::dto::TestKindDto::Node,
                                #tests::NAME,
                                stringify!(#struct_name),
                            )
                        ),*
                    ],
                    ctx_access: ::graphium::dto::CtxAccessDto::from(#ctx_access),
                    metrics_graph: module_path!().to_string(),
                    metrics_node: stringify!(#fn_name).to_string(),
                    playground_supported: #playground_supported,
                    playground_schema: ::graphium::dto::PlaygroundSchemaDto::from_schema(&schema),
                }
            }
        }

        #[cfg(feature = "export")]
        impl ::graphium::serde::Serialize for #struct_name {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: ::graphium::serde::Serializer,
            {
                let dto = Self::dto();
                ::graphium::serde::Serialize::serialize(&dto, serializer)
            }
        }

    };

    TokenStream::from(expanded)
}

fn extract_name_from_attrs(attrs: &mut Vec<syn::Attribute>) -> Option<Ident> {
    let mut out = None;
    let mut keep: Vec<syn::Attribute> = Vec::with_capacity(attrs.len());
    for attr in attrs.drain(..) {
        if attr.path().is_ident("name") {
            if out.is_some() {
                panic!("node `#[name = ...]` can only be specified once");
            }
            let syn::Meta::NameValue(name_value) = &attr.meta else {
                panic!("node `#[name = ...]` expects `#[name = Ident]` or `#[name = \"Ident\"]`");
            };
            let ident = match &name_value.value {
                syn::Expr::Path(expr_path) => expr_path
                    .path
                    .get_ident()
                    .cloned()
                    .unwrap_or_else(|| panic!("node `#[name = ...]` expects a single ident")),
                syn::Expr::Lit(expr_lit) => match &expr_lit.lit {
                    syn::Lit::Str(lit_str) => syn::parse_str::<Ident>(&lit_str.value())
                        .unwrap_or_else(|_| panic!("node `#[name = ...]` must be a valid ident")),
                    _ => panic!("node `#[name = ...]` must be an ident or string literal"),
                },
                _ => panic!("node `#[name = ...]` must be an ident or string literal"),
            };
            out = Some(ident);
            continue;
        }
        keep.push(attr);
    }
    *attrs = keep;
    out
}

fn extract_tags_from_attrs(attrs: &mut Vec<syn::Attribute>) -> Vec<String> {
    let mut out = Vec::new();
    let mut keep: Vec<syn::Attribute> = Vec::with_capacity(attrs.len());
    for attr in attrs.drain(..) {
        if attr.path().is_ident("tags") {
            let list = attr
                .parse_args_with(
                    syn::punctuated::Punctuated::<syn::LitStr, syn::Token![,]>::parse_terminated,
                )
                .unwrap_or_else(|_| {
                    panic!("`#[tags(...)]` expects string literals, e.g. `#[tags(\"db\", \"io\")]`")
                });
            for item in list {
                let tag = item.value();
                let tag = tag.trim();
                if !tag.is_empty() {
                    out.push(tag.to_string());
                }
            }
            continue;
        }
        keep.push(attr);
    }
    *attrs = keep;
    out
}

fn extract_deprecation_from_attrs(attrs: &[syn::Attribute]) -> (bool, Option<String>) {
    let mut deprecated = false;
    let mut reason: Option<String> = None;

    for attr in attrs {
        if !attr.path().is_ident("deprecated") {
            continue;
        }
        deprecated = true;
        match &attr.meta {
            syn::Meta::Path(_) => {}
            syn::Meta::NameValue(name_value) => {
                let syn::Expr::Lit(expr_lit) = &name_value.value else {
                    continue;
                };
                let syn::Lit::Str(lit) = &expr_lit.lit else {
                    continue;
                };
                let value = lit.value().trim().to_string();
                if !value.is_empty() {
                    reason = Some(value);
                }
            }
            syn::Meta::List(list) => {
                // Accept `#[deprecated(note = "...")]` style too.
                let parsed = syn::parse::Parser::parse2(
                    syn::punctuated::Punctuated::<syn::MetaNameValue, syn::Token![,]>::parse_terminated,
                    list.tokens.clone(),
                );
                if let Ok(items) = parsed {
                    for item in items {
                        if item.path.is_ident("note") {
                            if let syn::Expr::Lit(expr_lit) = &item.value {
                                if let syn::Lit::Str(lit) = &expr_lit.lit {
                                    let value = lit.value().trim().to_string();
                                    if !value.is_empty() {
                                        reason = Some(value);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    (deprecated, reason)
}

fn extract_tests_from_attrs(attrs: &mut Vec<syn::Attribute>) -> Vec<Path> {
    let mut out = Vec::new();
    let mut keep: Vec<syn::Attribute> = Vec::with_capacity(attrs.len());

    for attr in attrs.drain(..) {
        if attr.path().is_ident("tests") {
            let list = attr
                .parse_args_with(
                    syn::punctuated::Punctuated::<Path, syn::Token![,]>::parse_terminated,
                )
                .unwrap_or_else(|_| {
                    panic!("`#[tests(...)]` expects a list of paths, e.g. `#[tests(MyTestMarker)]`")
                });
            out.extend(list.into_iter());
            continue;
        }
        keep.push(attr);
    }

    *attrs = keep;
    out
}
