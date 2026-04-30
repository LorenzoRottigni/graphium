//! Top-level `graph!` expansion.
//!
//! This module is the entry point for expanding a `graph! { ... }` definition
//! into generated Rust code. It orchestrates the creation of the graph struct,
//! runtime execution methods, feature-gated helpers, and export metadata.
//!
//! The generated output currently includes:
//! - `pub struct GraphName;`
//! - `Default` implementation for the graph type
//! - synchronous `run(...)` method for non-async graphs
//! - asynchronous `run_async(...)` method for graphs using `async`
//! - graph flow and DTO export support under `dto` / `export` features
//! - optional playground metadata and runtime helper impls under `playground`

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

use crate::ir::{GraphInput, doc_string_from_attrs};
use std::collections::BTreeSet;

mod flow;
mod metadata;
mod metrics;
mod playground;
mod runtime;

/// Expands a `graph!` definition into:
/// - `pub struct GraphName;`
/// - inherent `run` / `run_async` methods
/// - optional feature-gated helpers (DTO, metrics, playground)
pub fn expand(input: TokenStream) -> TokenStream {
    // Preserve the original macro source text. This raw schema string is
    // exported into the graph DTO for debugging, visualization, and round-trip
    // inspection use cases.
    let raw_schema_string = input.to_string();
    let raw_schema_lit = syn::LitStr::new(&raw_schema_string, proc_macro2::Span::call_site());

    // Parse the graph definition into a strongly typed representation.
    let GraphInput {
        attrs,
        name,
        context,
        lifetimes: _lifetimes,
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

    let borrowed_slot_names = collect_borrowed_slot_names(&nodes);
    let borrowed_slot_idents: Vec<syn::Ident> = borrowed_slot_names
        .iter()
        .map(|name| crate::ir::borrowed_slot_ident(name))
        .collect();

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

    // Build the execution model for the graph body.
    // `root_setup` maps declared graph inputs to generated runtime parameters.
    // `execution` contains the step-by-step generated logic for sync/async paths.
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
        &borrowed_slot_idents,
        &graph_outputs,
        async_enabled,
    );
    let run_body_async = super::execution::build_run_body(
        Some(&execution.generated_async),
        &root_setup.root_input_bindings,
        &borrowed_slot_idents,
        &graph_outputs,
        false,
    );

    // Build the graph run bodies with optional metrics instrumentation.
    // Metrics wrappers are only activated when `metrics` is enabled.
    let metrics_enabled = metrics_spec.enabled();
    let metrics_config_tokens = metrics::metric_config_tokens(&metrics_spec);
    let sync_graph_body = metrics::wrap_sync_graph_body(&run_body, &metrics_spec);
    let async_graph_body = metrics::wrap_async_graph_body(&run_body_async, metrics_enabled);

    let metrics_impl = metrics::build_metrics_impl(&name, metrics_enabled, &metrics_config_tokens);
    let sync_impl = runtime::build_sync_impl(
        &name,
        &context,
        async_enabled,
        &root_setup.run_params,
        &root_setup.run_args,
        &run_return_sig,
        &sync_graph_body,
    );
    let async_impl = runtime::build_async_impl(
        &name,
        &context,
        &root_setup.run_params,
        &root_setup.run_args,
        &run_return_sig,
        &async_graph_body,
    );

    // Generate the graph flow DTO tokens and the complete graph DTO impl.
    // These exports are consumed by tools such as graphium-ui and serve as the
    // serialized graph metadata surface.
    let graph_flow_tokens = flow::graph_flow_tokens(&graph_inputs, &graph_outputs, &nodes);
    let dto_impl = metadata::build_graph_dto(
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
    let flow_impl = metadata::build_flow_impl(&name, &graph_flow_tokens);

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

fn collect_borrowed_slot_names(nodes: &crate::ir::NodeExpr) -> BTreeSet<String> {
    fn walk(expr: &crate::ir::NodeExpr, out: &mut BTreeSet<String>) {
        use crate::ir::NodeExpr;
        match expr {
            NodeExpr::Single(call) => {
                for (ident, kind) in call.inputs.iter().zip(call.input_kinds.iter()) {
                    if matches!(
                        kind,
                        crate::ir::ArtifactInputKind::Borrowed(_)
                            | crate::ir::ArtifactInputKind::Taken(_)
                    ) {
                        out.insert(ident.to_string());
                    }
                }
                for (ident, borrow) in call.outputs.iter().zip(call.output_borrows.iter()) {
                    if borrow.is_some() {
                        out.insert(ident.to_string());
                    }
                }
            }
            NodeExpr::Sequence(nodes) | NodeExpr::Parallel(nodes) => {
                for node in nodes {
                    walk(node, out);
                }
            }
            NodeExpr::Route(route) => {
                for (ident, borrow) in route.outputs.iter().zip(route.output_borrows.iter()) {
                    if borrow.is_some() {
                        out.insert(ident.to_string());
                    }
                }
                for (_, node) in &route.routes {
                    walk(node, out);
                }
            }
            NodeExpr::While(while_expr) => {
                for (ident, borrow) in while_expr
                    .outputs
                    .iter()
                    .zip(while_expr.output_borrows.iter())
                {
                    if borrow.is_some() {
                        out.insert(ident.to_string());
                    }
                }
                walk(&while_expr.body, out);
            }
            NodeExpr::Loop(loop_expr) => {
                for (ident, borrow) in loop_expr
                    .outputs
                    .iter()
                    .zip(loop_expr.output_borrows.iter())
                {
                    if borrow.is_some() {
                        out.insert(ident.to_string());
                    }
                }
                walk(&loop_expr.body, out);
            }
            NodeExpr::Break => {}
        }
    }

    let mut out = BTreeSet::new();
    walk(nodes, &mut out);
    out
}
