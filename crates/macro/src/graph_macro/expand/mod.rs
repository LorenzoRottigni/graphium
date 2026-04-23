//! Top-level `graph!` expansion.
//!
//! The expander is organized so this `mod.rs` is responsible for producing the
//! generated graph "class" (`pub struct GraphName;`). Submodules then append
//! feature-gated impl blocks and helper trait impls.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

use crate::shared::{GraphInput, doc_string_from_attrs};

mod dto;
mod metrics;
mod playground;
mod r#async;
mod sync;

/// Expands a `graph!` definition into:
/// - `pub struct GraphName;`
/// - inherent `run` / `run_async` methods
/// - optional feature-gated helpers (DTO, metrics, playground)
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
        metrics: metrics_spec,
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
    let root_setup = super::execution::build_root_setup(&graph_inputs, &mut counter);
    let execution = super::execution::generate_execution(
        &nodes,
        &root_setup.root_incoming,
        &mut counter,
        async_enabled,
    );
    let run_return_sig = super::execution::build_run_return_sig(&graph_outputs);
    let run_body = super::execution::build_run_body(
        execution.generated_sync.as_ref(),
        &root_setup.root_input_bindings,
        &graph_outputs,
        async_enabled,
    );
    let run_body_async = super::execution::build_run_body(
        Some(&execution.generated_async),
        &root_setup.root_input_bindings,
        &graph_outputs,
        false,
    );

    let metrics_enabled = metrics_spec.enabled();
    let metrics_config_tokens = metrics::metric_config_tokens(&metrics_spec);
    let sync_graph_body = metrics::wrap_sync_graph_body(&run_body, &metrics_spec);
    let async_graph_body = metrics::wrap_async_graph_body(&run_body_async, metrics_enabled);

    let metrics_impl = metrics::build_metrics_impl(&name, metrics_enabled, &metrics_config_tokens);
    let sync_impl = sync::build_sync_impl(
        &name,
        &context,
        async_enabled,
        &root_setup.run_params,
        &run_return_sig,
        &sync_graph_body,
    );
    let async_impl = r#async::build_async_impl(
        &name,
        &context,
        &root_setup.run_params,
        &run_return_sig,
        &async_graph_body,
    );

    let graph_flow_tokens = super::flow::graph_flow_tokens(&graph_inputs, &graph_outputs, &nodes);
    let dto_impl = dto::build_graph_dto(
        &name,
        &context,
        &graph_inputs,
        &graph_outputs,
        &nodes,
        &metrics_spec,
        &raw_schema_lit,
        graph_docs_tokens,
        graph_tag_tokens,
        graph_deprecated_token,
        graph_deprecated_reason_tokens,
    );
    let flow_impl = dto::build_flow_impl(&name, &graph_flow_tokens);

    let playground_impl = playground::build_playground_impl(
        &name,
        &context,
        &graph_inputs,
        &graph_outputs,
        async_enabled,
    );

    let expanded = quote! {
        pub struct #name;

        impl ::core::default::Default for #name {
            fn default() -> Self {
                Self
            }
        }

        #metrics_impl
        #sync_impl
        #async_impl
        #flow_impl
        #playground_impl
        #dto_impl
    };

    TokenStream::from(expanded)
}

