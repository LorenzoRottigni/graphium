//! Helpers for generating playground support for graphs.
//!
//! This module is responsible for producing the `GraphPlayground` trait
//! implementation that the UI runtime can use to render playground forms and
//! execute graph runs from serialized input values.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PlaygroundParseKind {
    String,
    Bool,
    FromStr,
}

/// Generate the `GraphPlayground` trait implementation for the graph.
///
/// This produces playground metadata and the runtime `playground_run` helper
/// which converts form inputs into the actual graph `run()` invocation.
pub(super) fn build_playground_impl(
    name: &Ident,
    context: &syn::Path,
    graph_inputs: &[(Ident, syn::Type)],
    graph_outputs: &[(Ident, syn::Type)],
    async_enabled: bool,
) -> TokenStream {
    // Build playground metadata for graph inputs and outputs. These values are
    // exposed as the schema shape that UI forms rely on.
    let input_params: Vec<_> = graph_inputs
        .iter()
        .map(|(ident, ty)| {
            quote! {
                ::graphium::PlaygroundParam { name: stringify!(#ident), ty: stringify!(#ty) }
            }
        })
        .collect();
    let output_params: Vec<_> = graph_outputs
        .iter()
        .map(|(ident, ty)| {
            quote! {
                ::graphium::PlaygroundParam { name: stringify!(#ident), ty: stringify!(#ty) }
            }
        })
        .collect();

    // Playground support requires a synchronous graph and inputs that can be
    // parsed from string form. If any input type is unsupported, runtime
    // playground execution is disabled for this graph.
    let supported = !async_enabled
        && graph_inputs
            .iter()
            .all(|(_, ty)| playground_parse_kind(ty).is_some());

    let run_body = if supported {
        let mut parse_bindings = Vec::new();
        let mut args = Vec::new();
        for (ident, ty) in graph_inputs {
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

        // If the graph produces no outputs, return a generic OK payload.
        // Otherwise serialize the result with `Debug` for display in the playground.
        let output_format = if graph_outputs.is_empty() {
            quote! { ::std::result::Result::Ok("ok".to_string()) }
        } else {
            quote! { ::std::result::Result::Ok(format!("{:?}", result)) }
        };

        quote! {{
            let mut ctx: #context = ::core::default::Default::default();
            #( #parse_bindings )*
            let result = #name::run(&mut ctx, #( #args ),* );
            #output_format
        }}
    } else {
        quote! {{
            let _ = form;
            ::std::result::Result::Err("playground execution is not supported for this graph (requires a sync graph and supported input types)".to_string())
        }}
    };

    quote! {
        #[cfg(feature = "playground")]
        impl ::graphium::GraphPlayground for #name {
            const PLAYGROUND_SUPPORTED: bool = #supported;

            fn playground_schema() -> ::graphium::PlaygroundSchema {
                static INPUTS: &[::graphium::PlaygroundParam] = &[ #( #input_params ),* ];
                static OUTPUTS: &[::graphium::PlaygroundParam] = &[ #( #output_params ),* ];
                ::graphium::PlaygroundSchema {
                    inputs: INPUTS,
                    outputs: OUTPUTS,
                    context: stringify!(#context),
                }
            }

            fn playground_run(
                form: &::std::collections::HashMap<String, String>,
            ) -> ::std::result::Result<String, String> {
                #run_body
            }
        }
    }
}

/// Determine whether a type can be parsed from a playground form input.
///
/// Supported playground types are `String`, `bool`, and numeric primitives.
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

