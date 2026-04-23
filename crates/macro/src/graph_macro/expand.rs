//! Top-level `graph!` expansion.
//!
//! This module owns the macro entrypoint and assembles the generated impl from
//! the lower-level graph-expression helpers.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

use crate::shared::GraphInput;
use crate::shared::doc_string_from_attrs;

use super::{
    build_graph_dto, build_metrics_defs, build_playground_impl, build_root_setup, build_run_body,
    build_run_return_sig, build_sync_impl, generate_execution, metric_config_tokens,
    wrap_async_graph_body, wrap_sync_graph_body,
};

/// Expands a `graph!` definition into:
/// - `pub struct GraphName;`
/// - inherent `run` / `run_async` methods
///
/// Example:
/// providing `graph!(Demo, Ctx => A >> B)` expands into a `Demo` type with
/// generated runner methods and graph-definition helpers.
pub fn expand(input: TokenStream) -> TokenStream {
    let raw_schema_string = input.to_string();
    let raw_schema_lit = syn::LitStr::new(&raw_schema_string, proc_macro2::Span::call_site());

    let GraphInput {
        attrs,
        name,
        context,
        inputs: graph_inputs,
        outputs: graph_outputs,
        nodes,
        async_enabled,
        metrics,
        tests: _tests,
        tags,
        deprecated,
        deprecated_reason,
    } = parse_macro_input!(input as GraphInput);

    let graph_docs = doc_string_from_attrs(&attrs);
    let graph_docs_tokens = match graph_docs {
        Some(value) => {
            let lit = syn::LitStr::new(&value, proc_macro2::Span::call_site());
            quote! { ::std::option::Option::Some(#lit.to_string()) }
        }
        None => quote! { ::std::option::Option::None },
    };
    let graph_tag_tokens: Vec<_> = tags
        .iter()
        .map(|t| syn::LitStr::new(t, proc_macro2::Span::call_site()))
        .collect();
    let graph_deprecated_token = deprecated;
    let graph_deprecated_reason_tokens = match deprecated_reason {
        Some(value) => {
            let lit = syn::LitStr::new(&value, proc_macro2::Span::call_site());
            quote! { ::std::option::Option::Some(#lit.to_string()) }
        }
        None => quote! { ::std::option::Option::None },
    };

    let mut counter = 0usize;
    let root_setup = build_root_setup(&graph_inputs, &mut counter);
    let execution = generate_execution(
        &nodes,
        &root_setup.root_incoming,
        &mut counter,
        async_enabled,
    );
    let run_return_sig = build_run_return_sig(&graph_outputs);
    let run_body = build_run_body(
        execution.generated_sync.as_ref(),
        &root_setup.root_input_bindings,
        &graph_outputs,
        async_enabled,
    );
    let run_body_async = build_run_body(
        Some(&execution.generated_async),
        &root_setup.root_input_bindings,
        &graph_outputs,
        false,
    );
    let graph_flow_tokens = super::graph_flow_tokens(&graph_inputs, &graph_outputs, &nodes);
    let playground_impl = build_playground_impl(
        &name,
        &context,
        &graph_inputs,
        &graph_outputs,
        async_enabled,
    );
    let metrics_enabled = metrics.enabled();
    let metrics_config_tokens = metric_config_tokens(&metrics);
    let sync_graph_body = wrap_sync_graph_body(&run_body, &metrics);
    let async_graph_body = wrap_async_graph_body(&run_body_async, metrics_enabled);
    let metrics_defs = build_metrics_defs(&name, metrics_enabled, &metrics_config_tokens);
    let sync_impl = build_sync_impl(
        &context,
        async_enabled,
        &root_setup.run_params,
        &run_return_sig,
        &sync_graph_body,
    );
    let async_run_params = &root_setup.run_params;

    let dto_impl = build_graph_dto(
        &name,
        &context,
        &graph_inputs,
        &graph_outputs,
        &nodes,
        &metrics,
        &raw_schema_lit,
        graph_docs_tokens.clone(),
        graph_tag_tokens,
        graph_deprecated_token,
        graph_deprecated_reason_tokens,
    );

    let expanded = quote! {
        pub struct #name;

        impl ::core::default::Default for #name {
            fn default() -> Self {
                Self
            }
        }

        impl #name {
            #metrics_defs
            #sync_impl

            pub async fn run_async(
                ctx: &mut #context,
                #( #async_run_params ),*
            ) #run_return_sig {
                #async_graph_body
            }

            pub fn __graphium_flow() -> ::graphium::export::GraphFlowDto {
                #graph_flow_tokens
            }
        }

        #playground_impl

        #dto_impl
    };

    TokenStream::from(expanded)
}
