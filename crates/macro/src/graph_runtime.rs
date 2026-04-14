use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{ToTokens, format_ident, quote};
use std::collections::{BTreeMap, BTreeSet};
use syn::{Ident, parse_macro_input};

use crate::shared::{GraphInput, NodeCall, NodeExpr, RouteExpr, fresh_ident, is_graph_run_path};

/// Expands a `graph_runtime!` definition into a runtime graph value factory
/// plus static metadata and an executable runtime plan.
pub fn expand(input: TokenStream) -> TokenStream {
    let GraphInput {
        name,
        context,
        inputs: graph_inputs,
        outputs: graph_outputs,
        nodes,
    } = parse_macro_input!(input as GraphInput);

    let nodes_static_ident = format_ident!("__graphio_runtime_nodes_{}", name);
    let edges_static_ident = format_ident!("__graphio_runtime_edges_{}", name);

    let mut codegen = RuntimeCodegen::default();
    let built = build_runtime_expr(&nodes, &context, &mut codegen);

    let node_literals = built.segment.nodes.iter().map(|node| {
        let id = node.id;
        let name = &node.name;
        quote! {
            ::graphio::RuntimeNode { id: #id, name: #name }
        }
    });
    let edge_literals = built.segment.edges.iter().map(|edge| {
        let from = edge.from;
        let to = edge.to;
        quote! {
            ::graphio::RuntimeEdge { from: #from, to: #to }
        }
    });

    let helper_fns = &codegen.helper_fns;
    let root_expr = built.expr_tokens;

    let input_names = graph_inputs.iter().map(|(ident, _)| {
        quote! { stringify!(#ident) }
    });
    let output_names = graph_outputs.iter().map(|(ident, _)| {
        quote! { stringify!(#ident) }
    });

    let expanded = quote! {
        #[allow(non_upper_case_globals)]
        static #nodes_static_ident: &[::graphio::RuntimeNode] = &[
            #( #node_literals ),*
        ];
        #[allow(non_upper_case_globals)]
        static #edges_static_ident: &[::graphio::RuntimeEdge] = &[
            #( #edge_literals ),*
        ];

        #( #helper_fns )*

        #[allow(non_snake_case)]
        pub fn #name() -> ::graphio::RuntimeGraph<#context> {
            ::graphio::RuntimeGraph::new(
                stringify!(#name),
                #nodes_static_ident,
                #edges_static_ident,
                #root_expr,
                vec![ #( #input_names ),* ],
                vec![ #( #output_names ),* ],
            )
        }
    };

    TokenStream::from(expanded)
}

#[derive(Default)]
struct RuntimeCodegen {
    next_node_id: usize,
    helper_index: usize,
    helper_fns: Vec<proc_macro2::TokenStream>,
}

struct RuntimeNodeDef {
    id: usize,
    name: String,
}

struct RuntimeEdgeDef {
    from: usize,
    to: usize,
}

struct RuntimeSegment {
    entries: Vec<usize>,
    exits: Vec<usize>,
    nodes: Vec<RuntimeNodeDef>,
    edges: Vec<RuntimeEdgeDef>,
}

struct RuntimeBuildResult {
    expr_tokens: proc_macro2::TokenStream,
    shape: ExprShape,
    segment: RuntimeSegment,
}

#[derive(Clone)]
struct ExprShape {
    entry_usage: BTreeMap<String, usize>,
    exit_outputs: Vec<String>,
}

fn build_runtime_expr(
    node: &NodeExpr,
    context: &syn::Path,
    codegen: &mut RuntimeCodegen,
) -> RuntimeBuildResult {
    match node {
        NodeExpr::Single(call) => build_single(call, context, codegen),
        NodeExpr::Sequence(nodes) => build_sequence(nodes, context, codegen),
        NodeExpr::Parallel(nodes) => build_parallel(nodes, context, codegen),
        NodeExpr::Route(route) => build_route(route, context, codegen),
        NodeExpr::While(_) | NodeExpr::Loop(_) | NodeExpr::Break => {
            panic!("@while/@loop/@break are not supported in graph_runtime");
        }
    }
}

fn build_single(
    call: &NodeCall,
    context: &syn::Path,
    codegen: &mut RuntimeCodegen,
) -> RuntimeBuildResult {
    let id = codegen.next_node_id;
    codegen.next_node_id += 1;

    let node_name = runtime_node_name(call);
    let shape = analyze_single(call);

    let run_fn_ident = format_ident!("__graphio_runtime_node_runner_{}", codegen.helper_index);
    codegen.helper_index += 1;
    let run_fn = build_node_runner_fn(&run_fn_ident, call, context);
    codegen.helper_fns.push(run_fn);

    let entry = required_artifacts(&shape);
    let exits = shape.exit_outputs.clone();

    let entry_tokens = entry.iter().map(|name| {
        quote! { #name }
    });
    let exit_tokens = exits.iter().map(|name| {
        quote! { #name }
    });

    let expr_tokens = quote! {
        ::graphio::RuntimeExpr::Node(::graphio::RuntimeNodeExec {
            id: #id,
            name: #node_name,
            run: #run_fn_ident,
            entry: vec![ #( #entry_tokens ),* ],
            exits: vec![ #( #exit_tokens ),* ],
        })
    };

    RuntimeBuildResult {
        expr_tokens,
        shape,
        segment: RuntimeSegment {
            entries: vec![id],
            exits: vec![id],
            nodes: vec![RuntimeNodeDef {
                id,
                name: node_name,
            }],
            edges: Vec::new(),
        },
    }
}

fn build_sequence(
    nodes: &[NodeExpr],
    context: &syn::Path,
    codegen: &mut RuntimeCodegen,
) -> RuntimeBuildResult {
    let mut iter = nodes.iter();
    let first = iter
        .next()
        .unwrap_or_else(|| panic!("sequence must contain at least one node"));

    let mut built = build_runtime_expr(first, context, codegen);
    let first_shape = built.shape.clone();
    let mut children = vec![built.expr_tokens.clone()];

    for node in iter {
        let next = build_runtime_expr(node, context, codegen);

        for from in &built.segment.exits {
            for to in &next.segment.entries {
                built.segment.edges.push(RuntimeEdgeDef {
                    from: *from,
                    to: *to,
                });
            }
        }

        built.segment.nodes.extend(next.segment.nodes);
        built.segment.edges.extend(next.segment.edges);
        built.segment.exits = next.segment.exits.clone();

        built.shape = ExprShape {
            entry_usage: first_shape.entry_usage.clone(),
            exit_outputs: next.shape.exit_outputs.clone(),
        };

        children.push(next.expr_tokens);
    }

    let entry = required_artifacts(&built.shape);
    let exits = built.shape.exit_outputs.clone();
    let entry_tokens = entry.iter().map(|name| quote! { #name });
    let exit_tokens = exits.iter().map(|name| quote! { #name });

    RuntimeBuildResult {
        expr_tokens: quote! {
            ::graphio::RuntimeExpr::Sequence(::graphio::RuntimeSequenceExec {
                steps: vec![ #( #children ),* ],
                entry: vec![ #( #entry_tokens ),* ],
                exits: vec![ #( #exit_tokens ),* ],
            })
        },
        shape: built.shape,
        segment: built.segment,
    }
}

fn build_parallel(
    nodes: &[NodeExpr],
    context: &syn::Path,
    codegen: &mut RuntimeCodegen,
) -> RuntimeBuildResult {
    let mut branches = Vec::new();
    let mut shapes = Vec::new();

    let mut entries = Vec::new();
    let mut exits = Vec::new();
    let mut all_nodes = Vec::new();
    let mut all_edges = Vec::new();

    for node in nodes {
        let built = build_runtime_expr(node, context, codegen);
        entries.extend(built.segment.entries.clone());
        exits.extend(built.segment.exits.clone());
        all_nodes.extend(built.segment.nodes);
        all_edges.extend(built.segment.edges);
        branches.push(built.expr_tokens);
        shapes.push(built.shape);
    }

    let shape = ExprShape {
        entry_usage: collect_parallel_entry_usage(&shapes),
        exit_outputs: collect_parallel_outputs(&shapes),
    };

    let entry = required_artifacts(&shape);
    let exits_names = shape.exit_outputs.clone();
    let entry_tokens = entry.iter().map(|name| quote! { #name });
    let exit_tokens = exits_names.iter().map(|name| quote! { #name });

    RuntimeBuildResult {
        expr_tokens: quote! {
            ::graphio::RuntimeExpr::Parallel(::graphio::RuntimeParallelExec {
                branches: vec![ #( #branches ),* ],
                entry: vec![ #( #entry_tokens ),* ],
                exits: vec![ #( #exit_tokens ),* ],
            })
        },
        shape,
        segment: RuntimeSegment {
            entries,
            exits,
            nodes: all_nodes,
            edges: all_edges,
        },
    }
}

fn build_route(
    route: &RouteExpr,
    context: &syn::Path,
    codegen: &mut RuntimeCodegen,
) -> RuntimeBuildResult {
    if route.routes.is_empty() {
        panic!("route must contain at least one branch");
    }

    let mut branches = Vec::new();
    let mut branch_shapes = Vec::new();

    let mut entries = Vec::new();
    let mut exits = Vec::new();
    let mut all_nodes = Vec::new();
    let mut all_edges = Vec::new();

    for (_, route_node) in &route.routes {
        let built = build_runtime_expr(route_node, context, codegen);
        entries.extend(built.segment.entries.clone());
        exits.extend(built.segment.exits.clone());
        all_nodes.extend(built.segment.nodes);
        all_edges.extend(built.segment.edges);
        branches.push(built.expr_tokens);
        branch_shapes.push(built.shape);
    }

    let mut entry_usage = BTreeMap::new();
    for shape in &branch_shapes {
        for artifact in required_artifacts(shape) {
            entry_usage.entry(artifact).or_insert(1);
        }
    }

    let shape = ExprShape {
        entry_usage,
        exit_outputs: collect_route_outputs(&branch_shapes),
    };

    let selector_ident = format_ident!("__graphio_runtime_route_selector_{}", codegen.helper_index);
    codegen.helper_index += 1;
    let selector_fn = build_route_selector_fn(&selector_ident, context, route);
    codegen.helper_fns.push(selector_fn);

    let entry = required_artifacts(&shape);
    let exits_names = shape.exit_outputs.clone();
    let entry_tokens = entry.iter().map(|name| quote! { #name });
    let exit_tokens = exits_names.iter().map(|name| quote! { #name });

    RuntimeBuildResult {
        expr_tokens: quote! {
            ::graphio::RuntimeExpr::Route(::graphio::RuntimeRouteExec {
                select: #selector_ident,
                branches: vec![ #( #branches ),* ],
                entry: vec![ #( #entry_tokens ),* ],
                exits: vec![ #( #exit_tokens ),* ],
            })
        },
        shape,
        segment: RuntimeSegment {
            entries,
            exits,
            nodes: all_nodes,
            edges: all_edges,
        },
    }
}

fn build_node_runner_fn(
    fn_ident: &Ident,
    call: &NodeCall,
    context: &syn::Path,
) -> proc_macro2::TokenStream {
    let node_path = &call.path;
    let nested_graph_path = is_graph_run_path(node_path).then(|| graph_type_path(node_path));

    if !call.explicit_inputs && call.inputs.is_empty() && call.outputs.is_empty() {
        let run_tokens = if let Some(graph_path) = nested_graph_path {
            quote! { #graph_path::__graphio_run(ctx); }
        } else {
            quote! { #node_path::__graphio_run(ctx); }
        };

        return quote! {
            fn #fn_ident(
                ctx: &mut #context,
                _incoming: &mut ::graphio::RuntimeArtifacts,
            ) -> ::graphio::RuntimeArtifacts {
                #run_tokens
                ::graphio::RuntimeArtifacts::new()
            }
        };
    }

    let mut usage = BTreeMap::new();
    for input in &call.inputs {
        *usage.entry(input.to_string()).or_insert(0usize) += 1;
    }

    let mut counter = 0usize;
    let mut bindings = Vec::new();
    let mut args = Vec::new();

    for input in &call.inputs {
        let artifact_name = input.to_string();
        let artifact_lit = syn::LitStr::new(&artifact_name, Span::call_site());
        let remaining = usage
            .get_mut(&artifact_name)
            .unwrap_or_else(|| panic!("missing usage count for `{artifact_name}`"));
        let arg_ident = fresh_ident(&mut counter, "arg", &artifact_name);

        if *remaining == 1 {
            bindings.push(quote! {
                let #arg_ident = incoming.take::<_>(#artifact_lit);
            });
        } else {
            bindings.push(quote! {
                let #arg_ident = incoming.clone_value::<_>(#artifact_lit);
            });
        }

        *remaining -= 1;
        args.push(arg_ident);
    }

    let run_call = if let Some(graph_path) = nested_graph_path {
        quote! { #graph_path::__graphio_run(ctx, #( #args ),*) }
    } else {
        quote! { #node_path::__graphio_run(ctx, #( #args ),*) }
    };

    if call.outputs.is_empty() {
        quote! {
            fn #fn_ident(
                ctx: &mut #context,
                incoming: &mut ::graphio::RuntimeArtifacts,
            ) -> ::graphio::RuntimeArtifacts {
                #( #bindings )*
                #run_call;
                ::graphio::RuntimeArtifacts::new()
            }
        }
    } else if call.outputs.len() == 1 {
        let output_ident = &call.outputs[0];
        quote! {
            fn #fn_ident(
                ctx: &mut #context,
                incoming: &mut ::graphio::RuntimeArtifacts,
            ) -> ::graphio::RuntimeArtifacts {
                #( #bindings )*
                let value = #run_call;
                let mut outputs = ::graphio::RuntimeArtifacts::new();
                outputs.insert(stringify!(#output_ident), value);
                outputs
            }
        }
    } else {
        let tuple_vars: Vec<Ident> = call
            .outputs
            .iter()
            .map(|output| {
                let name = output.to_string();
                fresh_ident(&mut counter, "ret", &name)
            })
            .collect();

        let stores = call
            .outputs
            .iter()
            .zip(tuple_vars.iter())
            .map(|(output, var)| {
                quote! {
                    outputs.insert(stringify!(#output), #var);
                }
            });

        quote! {
            fn #fn_ident(
                ctx: &mut #context,
                incoming: &mut ::graphio::RuntimeArtifacts,
            ) -> ::graphio::RuntimeArtifacts {
                #( #bindings )*
                let ( #( #tuple_vars ),* ) = #run_call;
                let mut outputs = ::graphio::RuntimeArtifacts::new();
                #( #stores )*
                outputs
            }
        }
    }
}

fn build_route_selector_fn(
    fn_ident: &Ident,
    context: &syn::Path,
    route: &RouteExpr,
) -> proc_macro2::TokenStream {
    let on_expr = &route.on;
    let arms = route.routes.iter().enumerate().map(|(idx, (key, _))| {
        quote! {
            #key => #idx
        }
    });

    quote! {
        fn #fn_ident(ctx: &mut #context) -> usize {
            match (#on_expr)(ctx) {
                #( #arms, )*
                _ => panic!("route selector evaluated to an undefined branch"),
            }
        }
    }
}

fn analyze_single(call: &NodeCall) -> ExprShape {
    if !call.explicit_inputs && call.inputs.is_empty() && call.outputs.is_empty() {
        return ExprShape {
            entry_usage: BTreeMap::new(),
            exit_outputs: Vec::new(),
        };
    }

    let mut entry_usage = BTreeMap::new();
    for input in &call.inputs {
        *entry_usage.entry(input.to_string()).or_insert(0) += 1;
    }

    ExprShape {
        entry_usage,
        exit_outputs: call.outputs.iter().map(ToString::to_string).collect(),
    }
}

fn required_artifacts(shape: &ExprShape) -> Vec<String> {
    shape.entry_usage.keys().cloned().collect()
}

fn collect_parallel_entry_usage(shapes: &[ExprShape]) -> BTreeMap<String, usize> {
    let mut remaining = BTreeMap::new();

    for shape in shapes {
        for artifact in required_artifacts(shape) {
            *remaining.entry(artifact).or_insert(0) += 1;
        }
    }

    remaining
}

fn collect_parallel_outputs(shapes: &[ExprShape]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut outputs = Vec::new();

    for shape in shapes {
        for artifact in &shape.exit_outputs {
            if !seen.insert(artifact.clone()) {
                panic!("parallel step produces duplicate artifact `{artifact}`");
            }
            outputs.push(artifact.clone());
        }
    }

    outputs
}

fn collect_route_outputs(shapes: &[ExprShape]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut outputs = Vec::new();

    for shape in shapes {
        for artifact in &shape.exit_outputs {
            if seen.insert(artifact.clone()) {
                outputs.push(artifact.clone());
            }
        }
    }

    outputs
}

fn runtime_node_name(call: &NodeCall) -> String {
    let mut path = call.path.to_token_stream().to_string();
    path.retain(|ch| !ch.is_whitespace());
    path
}

/// Extracts the graph type path from a `SomeGraph::run` path.
fn graph_type_path(path: &syn::Path) -> syn::Path {
    let mut graph_path = path.clone();
    graph_path.segments.pop();
    graph_path.segments.pop_punct();

    if graph_path.segments.is_empty() {
        panic!("invalid graph run path");
    }

    graph_path
}
