//! Top-level `graph!` expansion.
//!
//! This module owns the macro entrypoint and assembles the generated impl from
//! the lower-level graph-expression helpers.

use proc_macro::TokenStream;
use quote::{ToTokens as _, quote};
use syn::parse_macro_input;

use crate::shared::{GeneratedExpr, GraphInput, MetricsSpec, NodeExpr, Payload, fresh_ident};
use crate::shared::doc_string_from_attrs;

use super::{get_node_expr, graph_definition_tokens};

/// Expands a `graph!` definition into:
/// - `pub struct GraphName;`
/// - inherent `run` / `run_async` / `__graphium_run*` methods
/// - an optional `impl ::graphium::Graph<_> for GraphName`
/// - `impl ::graphium::GraphDefProvider for GraphName`
///
/// Example:
/// providing `graph!(Demo, Ctx => A >> B)` expands into a `Demo` type with
/// generated runner methods and graph-definition helpers.
pub fn expand(input: TokenStream) -> TokenStream {
    let raw_schema_string = input.to_string();

    // Rust string literal (AST node) for runtime.
    let raw_schema_lit =
        syn::LitStr::new(&raw_schema_string, proc_macro2::Span::call_site());

    // Syn parses token stream into GraphInput through GraphInput::parse
    let GraphInput {
        attrs,
        name,
        context,
        inputs: graph_inputs,
        outputs: graph_outputs,
        nodes,
        async_enabled,
        metrics,
        tests,
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
    let trait_run_body = build_trait_run_body(&name, &graph_inputs, &graph_outputs, false);
    let async_trait_run_body = build_trait_run_body(&name, &graph_inputs, &graph_outputs, true);
    let graph_def_tokens = graph_definition_tokens(&name, &graph_inputs, &graph_outputs, &nodes);
    let playground_impl = build_playground_impl(
        &name,
        &context,
        &graph_inputs,
        &graph_outputs,
        async_enabled,
    );
    let metrics_enabled = metrics.enabled();
    let metrics_config_tokens = metric_config_tokens(metrics);
    let sync_graph_body = wrap_sync_graph_body(&run_body, metrics);
    let async_graph_body = wrap_async_graph_body(&run_body_async, metrics_enabled);
    let metrics_defs = build_metrics_defs(&name, metrics_enabled, &metrics_config_tokens);
    let sync_impl = build_sync_impl(
        &context,
        async_enabled,
        &root_setup.run_params,
        &run_return_sig,
        &sync_graph_body,
    );
    let graph_impl = build_graph_impl(&name, &context, async_enabled, &trait_run_body);
    let async_run_params = &root_setup.run_params;
    let export_inputs: Vec<_> = graph_inputs
        .iter()
        .map(|(ident, ty)| {
            quote! {
                ::graphium::export::IoParamDto {
                    name: stringify!(#ident).to_string(),
                    ty: stringify!(#ty).to_string(),
                }
            }
        })
        .collect();
    let export_outputs: Vec<_> = graph_outputs
        .iter()
        .map(|(ident, ty)| {
            quote! {
                ::graphium::export::IoParamDto {
                    name: stringify!(#ident).to_string(),
                    ty: stringify!(#ty).to_string(),
                }
            }
        })
        .collect();
    let export_metrics = metric_names_tokens(metrics);
    let export_paths = collect_export_paths(&nodes);
    let export_nodes: Vec<_> = export_paths
        .node_paths
        .iter()
        .map(|path| quote! { #path::__graphium_dto() })
        .collect();
    let export_subgraphs: Vec<_> = export_paths
        .graph_paths
        .iter()
        .map(|path| quote! { #path::__graphium_dto() })
        .collect();
    let export_graph_tests: Vec<_> = tests
        .iter()
        .map(|test_path| {
            quote! {
                ::graphium::export::TestDto::new(
                    ::graphium::export::TestKindDto::Graph,
                    #test_path::NAME,
                    stringify!(#name),
                )
            }
        })
        .collect();
    let graph_test_runs: Vec<_> = tests
        .iter()
        .map(|test_path| {
            quote! {
                ::graphium::export::TestRun {
                    dto: ::graphium::export::TestDto::new(
                        ::graphium::export::TestKindDto::Graph,
                        #test_path::NAME,
                        stringify!(#name),
                    ),
                    schema: #test_path::__graphium_ui_schema(),
                    default_values: #test_path::__graphium_ui_default_values(),
                    run: #test_path::__graphium_ui_run_with_args,
                }
            }
        })
        .collect();
    let subgraph_test_runs: Vec<_> = export_paths
        .graph_paths
        .iter()
        .map(|path| quote! { out.extend(#path::__graphium_test_runs()); })
        .collect();
    let node_test_runs: Vec<_> = export_paths
        .node_paths
        .iter()
        .map(|path| quote! { out.extend(#path::__graphium_test_runs()); })
        .collect();

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

            /// Convenience async entry point that executes the graph directly.
            pub async fn run_async(ctx: &mut #context) {
                #async_trait_run_body
            }

            pub async fn __graphium_run_async(
                ctx: &mut #context,
                #( #async_run_params ),*
            ) #run_return_sig {
                #async_graph_body
            }

            pub fn graph_def() -> ::graphium::GraphDef {
                #graph_def_tokens
            }
        }
        #graph_impl

        impl ::graphium::GraphDefProvider for #name {
            fn graph_def() -> ::graphium::GraphDef {
                Self::graph_def()
            }
        }

        #playground_impl

        impl #name {
            pub fn __graphium_dto() -> ::graphium::export::GraphDto {
                let def = Self::graph_def();
                ::graphium::export::GraphDto {
                    id: ::graphium::export::slugify(def.name),
                    name: def.name.to_string(),
                    docs: #graph_docs_tokens,
                    tags: vec![ #( #graph_tag_tokens.to_string() ),* ],
                    deprecated: #graph_deprecated_token,
                    deprecated_reason: #graph_deprecated_reason_tokens,
                    schema: ::std::option::Option::Some(::graphium::export::GraphSchemaDto {
                        context: stringify!(#context).to_string(),
                        inputs: vec![ #( #export_inputs ),* ],
                        outputs: vec![ #( #export_outputs ),* ],
                        metrics: vec![ #( #export_metrics ),* ],
                    }),
                    def: ::graphium::export::GraphDefDto::from_def(&def),
                    raw_schema: ::std::option::Option::Some(#raw_schema_lit.to_string()),
                    tests: {
                        #[cfg(feature = "serialize")]
                        {
                            vec![ #( #export_graph_tests ),* ]
                        }
                        #[cfg(not(feature = "serialize"))]
                        {
                            Vec::new()
                        }
                    },
                    nodes: vec![ #( #export_nodes ),* ],
                    subgraphs: vec![ #( #export_subgraphs ),* ],
                    playground: ::std::option::Option::Some(::graphium::export::PlaygroundDto {
                        supported: <Self as ::graphium::GraphPlayground>::PLAYGROUND_SUPPORTED,
                        schema: ::graphium::export::PlaygroundSchemaDto::from_schema(
                            &<Self as ::graphium::GraphPlayground>::playground_schema(),
                        ),
                    }),
                }
            }

            pub fn __graphium_test_runs() -> ::std::vec::Vec<::graphium::export::TestRun> {
                let mut out: ::std::vec::Vec<::graphium::export::TestRun> = ::std::vec::Vec::new();
                #[cfg(feature = "serialize")]
                {
                    out.extend(vec![ #( #graph_test_runs ),* ]);
                    #( #subgraph_test_runs )*
                    #( #node_test_runs )*
                }
                out
            }
        }

        impl ::graphium::GraphUiTests for #name {
            fn graphium_ui_tests() -> ::std::vec::Vec<::graphium::export::TestRun> {
                Self::__graphium_test_runs()
            }
        }

        #[cfg(feature = "serialize")]
        impl ::graphium::serde::Serialize for #name {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: ::graphium::serde::Serializer,
            {
                let dto = Self::__graphium_dto();
                ::graphium::serde::Serialize::serialize(&dto, serializer)
            }
        }
    };

    TokenStream::from(expanded)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlaygroundParseKind {
    String,
    Bool,
    FromStr,
}

fn metric_names_tokens(spec: MetricsSpec) -> Vec<proc_macro2::TokenStream> {
    let mut out = Vec::new();
    if spec.performance {
        out.push(quote! { "performance".to_string() });
    }
    if spec.errors {
        out.push(quote! { "errors".to_string() });
    }
    if spec.count {
        out.push(quote! { "count".to_string() });
    }
    if spec.caller {
        out.push(quote! { "caller".to_string() });
    }
    if spec.success_rate {
        out.push(quote! { "success_rate".to_string() });
    }
    if spec.fail_rate {
        out.push(quote! { "fail_rate".to_string() });
    }
    out
}

struct ExportPaths {
    node_paths: Vec<syn::Path>,
    graph_paths: Vec<syn::Path>,
}

fn collect_export_paths(expr: &NodeExpr) -> ExportPaths {
    use std::collections::BTreeMap;

    let mut nodes = BTreeMap::<String, syn::Path>::new();
    let mut graphs = BTreeMap::<String, syn::Path>::new();
    collect_export_paths_inner(expr, &mut nodes, &mut graphs);

    ExportPaths {
        node_paths: nodes.into_values().collect(),
        graph_paths: graphs.into_values().collect(),
    }
}

fn collect_export_paths_inner(
    expr: &NodeExpr,
    nodes: &mut std::collections::BTreeMap<String, syn::Path>,
    graphs: &mut std::collections::BTreeMap<String, syn::Path>,
) {
    match expr {
        NodeExpr::Single(call) => {
            let path = &call.path;
            if crate::shared::is_graph_run_path(path) {
                let graph_path = super::single::graph_type_path(path);
                graphs
                    .entry(graph_path.to_token_stream().to_string())
                    .or_insert(graph_path);
            } else {
                nodes
                    .entry(path.to_token_stream().to_string())
                    .or_insert_with(|| path.clone());
            }
        }
        NodeExpr::Sequence(items) | NodeExpr::Parallel(items) => {
            for item in items {
                collect_export_paths_inner(item, nodes, graphs);
            }
        }
        NodeExpr::Route(route) => {
            for (_label, item) in &route.routes {
                collect_export_paths_inner(item, nodes, graphs);
            }
        }
        NodeExpr::While(while_expr) => {
            collect_export_paths_inner(while_expr.body.as_ref(), nodes, graphs);
        }
        NodeExpr::Loop(loop_expr) => {
            collect_export_paths_inner(loop_expr.body.as_ref(), nodes, graphs);
        }
        NodeExpr::Break => {}
    }
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

fn build_playground_impl(
    name: &syn::Ident,
    context: &syn::Path,
    graph_inputs: &[(syn::Ident, syn::Type)],
    graph_outputs: &[(syn::Ident, syn::Type)],
    async_enabled: bool,
) -> proc_macro2::TokenStream {
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

    // Playground is purely a UI convenience: it uses the graph's declared IO
    // schema to build a form and runs the graph with a fresh `Ctx::default()`.
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

        let output_format = if graph_outputs.is_empty() {
            quote! { ::std::result::Result::Ok("ok".to_string()) }
        } else {
            quote! { ::std::result::Result::Ok(format!("{:?}", result)) }
        };

        quote! {{
            let mut ctx: #context = ::core::default::Default::default();
            #( #parse_bindings )*
            let result = #name::__graphium_run(&mut ctx, #( #args ),* );
            #output_format
        }}
    } else {
        quote! {{
            let _ = form;
            ::std::result::Result::Err("playground execution is not supported for this graph (requires a sync graph and supported input types)".to_string())
        }}
    };

    quote! {
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

/// Root-level bindings needed before the graph body can run.
struct RootSetup {
    root_incoming: Payload,
    run_params: Vec<proc_macro2::TokenStream>,
    root_input_bindings: Vec<proc_macro2::TokenStream>,
}

/// Generated sync and async graph bodies share the same root payload setup.
struct GeneratedExecution {
    generated_sync: Option<GeneratedExpr>,
    generated_async: GeneratedExpr,
}

/// Expands the declared graph inputs into:
/// - the `__graphium_run*` parameter list, like `arg: Ty`
/// - root `Option<T>` bindings, like `let mut __graphium_root_in_* = Some(arg);`
/// - the initial hop payload map consumed by the first generated node
///
/// Example:
/// providing inputs `[(value, i32), (name, String)]` expands into params like
/// `__graphium_graph_in_*_value: i32` plus root bindings storing them in
/// `Option` slots for the first hop.
fn build_root_setup(graph_inputs: &[(syn::Ident, syn::Type)], counter: &mut usize) -> RootSetup {
    let mut root_incoming = Payload::new();
    let mut run_params = Vec::with_capacity(graph_inputs.len());
    let mut root_input_bindings = Vec::with_capacity(graph_inputs.len());

    for (artifact, ty) in graph_inputs {
        let param_ident = fresh_ident(counter, "graph_in", &artifact.to_string());
        let payload_ident = fresh_ident(counter, "root_in", &artifact.to_string());
        root_incoming.insert_owned(artifact.to_string(), payload_ident.clone());
        run_params.push(quote! {
            #param_ident: #ty
        });
        root_input_bindings.push(quote! {
            let mut #payload_ident = ::std::option::Option::Some(#param_ident);
        });
    }

    RootSetup {
        root_incoming,
        run_params,
        root_input_bindings,
    }
}

/// Expands the parsed graph expression into the executable token trees used by:
/// - `__graphium_run(...) { ... }`
/// - `__graphium_run_async(...) { ... }`
///
/// Example:
/// providing `A >> B` expands into one sync `GeneratedExpr` for
/// `__graphium_run(...)` and one async `GeneratedExpr` for
/// `__graphium_run_async(...).await`.
fn generate_execution(
    nodes: &crate::shared::NodeExpr,
    root_incoming: &Payload,
    counter: &mut usize,
    async_enabled: bool,
) -> GeneratedExecution {
    let generated_sync = if async_enabled {
        None
    } else {
        Some(get_node_expr(nodes, root_incoming, counter, false, false))
    };
    let generated_async = get_node_expr(nodes, root_incoming, counter, false, true);

    GeneratedExecution {
        generated_sync,
        generated_async,
    }
}

/// Expands graph outputs into a Rust return signature:
/// - no outputs => `` (unit)
/// - one output => `-> T`
/// - many outputs => `-> (T1, T2, ...)`
///
/// Example:
/// providing outputs `[(left, A), (right, B)]` expands into `-> (A, B)`.
fn build_run_return_sig(graph_outputs: &[(syn::Ident, syn::Type)]) -> proc_macro2::TokenStream {
    if graph_outputs.is_empty() {
        quote! {}
    } else if graph_outputs.len() == 1 {
        let (_, ty) = &graph_outputs[0];
        quote! { -> #ty }
    } else {
        let tys = graph_outputs.iter().map(|(_, ty)| ty);
        quote! { -> ( #( #tys ),* ) }
    }
}

/// Expands one generated runner body into a block like:
/// - root input bindings
/// - generated node-execution tokens
/// - optional final return expression that extracts graph outputs
///
/// Example:
/// providing one generated node body and one output `result` expands into:
/// `{ let mut __graphium_root_in_* = Some(...); ...generated...; result.take()... }`.
fn build_run_body(
    generated: Option<&GeneratedExpr>,
    root_input_bindings: &[proc_macro2::TokenStream],
    graph_outputs: &[(syn::Ident, syn::Type)],
    disabled: bool,
) -> proc_macro2::TokenStream {
    if disabled {
        return quote! {};
    }

    let generated = generated.expect("generated graph body");
    let generated_tokens = generated.tokens.clone();
    let return_expr = build_return_expr(generated, graph_outputs);

    if graph_outputs.is_empty() {
        quote! {{
            #( #root_input_bindings )*
            #generated_tokens
        }}
    } else {
        quote! {{
            #( #root_input_bindings )*
            #generated_tokens
            #return_expr
        }}
    }
}

/// Expands graph outputs into the final return expression that `take()`s values
/// from the last hop payload, returning either `value` or `(value1, value2, ...)`.
///
/// Example:
/// providing outputs `["left", "right"]` expands into
/// `(left_slot.take().unwrap_or_else(...), right_slot.take().unwrap_or_else(...))`.
fn build_return_expr(
    generated: &GeneratedExpr,
    graph_outputs: &[(syn::Ident, syn::Type)],
) -> proc_macro2::TokenStream {
    let output_values: Vec<proc_macro2::TokenStream> = graph_outputs
        .iter()
        .map(|(artifact, _)| {
            let artifact_name = artifact.to_string();
            let output_var = generated
                .outputs
                .get_owned(&artifact_name)
                .unwrap_or_else(|| {
                    panic!("graph output `{artifact_name}` is not produced by the schema")
                });
            quote! {
                #output_var
                    .take()
                    .unwrap_or_else(|| panic!(concat!("missing graph output `", #artifact_name, "`")))
            }
        })
        .collect();

    if output_values.len() == 1 {
        quote! { #(#output_values)* }
    } else {
        quote! { ( #( #output_values ),* ) }
    }
}

/// Expands the trait-facing entry body into either:
/// - a forwarder like `Self::__graphium_run(ctx);`
/// - a panic explaining that graphs with explicit IO must be called as nested steps
///
/// Example:
/// providing a graph with no explicit IO expands into `Self::__graphium_run(ctx);`
/// while a graph with declared inputs expands into a `panic!(...)` block.
fn build_trait_run_body(
    name: &syn::Ident,
    graph_inputs: &[(syn::Ident, syn::Type)],
    graph_outputs: &[(syn::Ident, syn::Type)],
    async_mode: bool,
) -> proc_macro2::TokenStream {
    if graph_inputs.is_empty() && graph_outputs.is_empty() {
        if async_mode {
            quote! {
                Self::__graphium_run_async(ctx).await;
            }
        } else {
            quote! {
                Self::__graphium_run(ctx);
            }
        }
    } else {
        quote! {
            panic!(concat!(
                "graph `",
                stringify!(#name),
                "` has explicit inputs/outputs; call it as a nested step: `",
                stringify!(#name),
                "(...) -> (...)`"
            ));
        }
    }
}

/// Expands the sync runner body into either:
/// - the unmodified `run_body`
/// - a metrics-wrapped block that times execution and optionally records panics
///
/// Example:
/// providing a plain body `{ ... }` with error metrics enabled expands into a
/// `catch_unwind` wrapper that records success or failure around `{ ... }`.
fn wrap_sync_graph_body(
    run_body: &proc_macro2::TokenStream,
    metrics: MetricsSpec,
) -> proc_macro2::TokenStream {
    if !metrics.enabled() {
        return run_body.clone();
    }

    if metrics.track_panics_sync() {
        quote! {
            let __graphium_metrics = Self::__graphium_graph_metrics();
            let __graphium_start = __graphium_metrics.start_timer();
            let __graphium_result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| #run_body));
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
            let __graphium_metrics = Self::__graphium_graph_metrics();
            let __graphium_start = __graphium_metrics.start_timer();
            let value = #run_body;
            __graphium_metrics.record_success(__graphium_start);
            value
        }
    }
}

/// Expands the async runner body into either:
/// - the unmodified async body
/// - a metrics-wrapped block that times execution and records success
///
/// Example:
/// providing async body `{ ... }` with metrics enabled expands into
/// `{ let metrics = ...; let value = { ... }; metrics.record_success(...); value }`.
fn wrap_async_graph_body(
    run_body_async: &proc_macro2::TokenStream,
    metrics_enabled: bool,
) -> proc_macro2::TokenStream {
    if !metrics_enabled {
        return run_body_async.clone();
    }

    quote! {
        let __graphium_metrics = Self::__graphium_graph_metrics();
        let __graphium_start = __graphium_metrics.start_timer();
        let value = #run_body_async;
        __graphium_metrics.record_success(__graphium_start);
        value
    }
}

/// Expands metrics support into the cached helper method:
/// `fn __graphium_graph_metrics() -> &'static GraphMetricsHandle { ... }`
/// or nothing when graph metrics are disabled.
///
/// Example:
/// providing enabled metrics expands into a `OnceLock`-backed
/// `__graphium_graph_metrics()` helper inside the generated impl block.
fn build_metrics_defs(
    name: &syn::Ident,
    metrics_enabled: bool,
    metrics_config_tokens: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if !metrics_enabled {
        return quote! {};
    }

    quote! {
        fn __graphium_graph_metrics() -> &'static ::graphium::metrics::GraphMetricsHandle {
            static METRICS: ::std::sync::OnceLock<::graphium::metrics::GraphMetricsHandle> = ::std::sync::OnceLock::new();
            METRICS.get_or_init(|| {
                ::graphium::metrics::graph_metrics(
                    stringify!(#name),
                    module_path!(),
                    #metrics_config_tokens,
                )
            })
        }
    }
}

/// Expands sync-only inherent methods into:
/// - `pub fn run(ctx: &mut Ctx)`
/// - `pub fn __graphium_run(ctx: &mut Ctx, ...) -> ...`
/// or nothing for async-only graphs.
///
/// Example:
/// providing a sync graph in context `AppCtx` expands into
/// `pub fn run(ctx: &mut AppCtx)` and `pub fn __graphium_run(ctx: &mut AppCtx, ...)`.
fn build_sync_impl(
    context: &syn::Path,
    async_enabled: bool,
    run_params: &[proc_macro2::TokenStream],
    run_return_sig: &proc_macro2::TokenStream,
    sync_graph_body: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if async_enabled {
        return quote! {};
    }

    quote! {
        pub fn run(ctx: &mut #context) {
            <Self as ::graphium::Graph<#context>>::run(ctx);
        }

        pub fn __graphium_run(
            ctx: &mut #context,
            #( #run_params ),*
        ) #run_return_sig {
            #sync_graph_body
        }
    }
}

/// Expands the trait implementation:
/// `impl ::graphium::Graph<Ctx> for GraphName { fn run(...) { ... } }`
/// or nothing for async-only graphs.
///
/// Example:
/// providing `name = DemoGraph` and `context = AppCtx` expands into
/// `impl ::graphium::Graph<AppCtx> for DemoGraph { ... }`.
fn build_graph_impl(
    name: &syn::Ident,
    context: &syn::Path,
    async_enabled: bool,
    trait_run_body: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if async_enabled {
        return quote! {};
    }

    quote! {
        impl ::graphium::Graph<#context> for #name {
            fn run(ctx: &mut #context) {
                #trait_run_body
            }
        }
    }
}

/// Expands the parsed metric flags into the runtime struct literal:
/// `::graphium::metrics::MetricConfig { ... }`.
///
/// Example:
/// providing `performance = true, errors = false` expands into
/// `::graphium::metrics::MetricConfig { performance: true, errors: false, ... }`.
pub(super) fn metric_config_tokens(metrics: MetricsSpec) -> proc_macro2::TokenStream {
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

#[cfg(test)]
mod tests {
    use super::metric_config_tokens;
    use crate::shared::MetricsSpec;

    #[test]
    fn metric_config_tokens_emit_all_flags() {
        let tokens = metric_config_tokens(MetricsSpec {
            performance: true,
            errors: false,
            count: true,
            caller: false,
            success_rate: true,
            fail_rate: false,
        })
        .to_string();

        assert!(tokens.contains("performance : true"));
        assert!(tokens.contains("errors : false"));
        assert!(tokens.contains("count : true"));
        assert!(tokens.contains("success_rate : true"));
    }
}
