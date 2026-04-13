use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::parse_macro_input;

use crate::{
    graph::get_node_expr,
    shared::{GraphInput, NodeCall, NodeExpr, Payload, fresh_ident},
};

/// Expands a `graph_runtime!` definition into a runtime graph value factory
/// plus static metadata (nodes/edges) and executable behavior.
pub fn expand(input: TokenStream) -> TokenStream {
    let GraphInput {
        name,
        context,
        inputs: graph_inputs,
        outputs: graph_outputs,
        nodes,
    } = parse_macro_input!(input as GraphInput);

    let mut counter = 0usize;
    let mut root_incoming = Payload::new();
    let mut run_params = Vec::with_capacity(graph_inputs.len());
    let mut root_input_bindings = Vec::with_capacity(graph_inputs.len());

    for (artifact, ty) in &graph_inputs {
        let param_ident = fresh_ident(&mut counter, "graph_in", &artifact.to_string());
        let payload_ident = fresh_ident(&mut counter, "root_in", &artifact.to_string());
        root_incoming.insert(artifact.to_string(), payload_ident.clone());
        run_params.push(quote! {
            #param_ident: #ty
        });
        root_input_bindings.push(quote! {
            let mut #payload_ident = ::std::option::Option::Some(#param_ident);
        });
    }

    let generated = get_node_expr(&nodes, &root_incoming, &mut counter);
    let run_return_sig = if graph_outputs.is_empty() {
        quote! {}
    } else if graph_outputs.len() == 1 {
        let (_, ty) = &graph_outputs[0];
        quote! { -> #ty }
    } else {
        let tys = graph_outputs.iter().map(|(_, ty)| ty);
        quote! { -> ( #( #tys ),* ) }
    };

    let run_body = if graph_outputs.is_empty() {
        let generated_tokens = generated.tokens;
        quote! {{
            #( #root_input_bindings )*
            #generated_tokens
        }}
    } else {
        let generated_tokens = generated.tokens;
        let output_values: Vec<proc_macro2::TokenStream> = graph_outputs
            .iter()
            .map(|(artifact, _)| {
                let artifact_name = artifact.to_string();
                let output_var = generated.outputs.get(&artifact_name).unwrap_or_else(|| {
                    panic!("graph output `{artifact_name}` is not produced by the schema")
                });
                quote! {
                    #output_var
                        .take()
                        .unwrap_or_else(|| panic!(concat!("missing graph output `", #artifact_name, "`")))
                }
            })
            .collect();
        let return_expr = if output_values.len() == 1 {
            quote! { #(#output_values)* }
        } else {
            quote! { ( #( #output_values ),* ) }
        };

        quote! {{
            #( #root_input_bindings )*
            #generated_tokens
            #return_expr
        }}
    };

    let run_fn_ident = format_ident!("__graphio_runtime_run_{}", name);
    let execute_fn_ident = format_ident!("__graphio_runtime_execute_{}", name);
    let nodes_static_ident = format_ident!("__graphio_runtime_nodes_{}", name);
    let edges_static_ident = format_ident!("__graphio_runtime_edges_{}", name);

    let execute_body = if graph_inputs.is_empty() && graph_outputs.is_empty() {
        quote! {
            #run_fn_ident(ctx);
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
    };

    let runtime_def = build_runtime_definition(&nodes);
    let node_literals = runtime_def.nodes.iter().map(|node| {
        let id = node.id;
        let name = &node.name;
        quote! {
            ::graphio::RuntimeNode { id: #id, name: #name }
        }
    });
    let edge_literals = runtime_def.edges.iter().map(|edge| {
        let from = edge.from;
        let to = edge.to;
        quote! {
            ::graphio::RuntimeEdge { from: #from, to: #to }
        }
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

        #[allow(non_snake_case)]
        fn #execute_fn_ident(ctx: &mut #context) {
            #execute_body
        }

        #[allow(non_snake_case)]
        fn #run_fn_ident(
            ctx: &mut #context,
            #( #run_params ),*
        ) #run_return_sig {
            #run_body
        }

        #[allow(non_snake_case)]
        pub fn #name() -> ::graphio::RuntimeGraph<#context> {
            ::graphio::RuntimeGraph::new(
                stringify!(#name),
                #nodes_static_ident,
                #edges_static_ident,
                #execute_fn_ident,
            )
        }
    };

    TokenStream::from(expanded)
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

struct RuntimeDefinition {
    nodes: Vec<RuntimeNodeDef>,
    edges: Vec<RuntimeEdgeDef>,
}

fn build_runtime_definition(nodes: &NodeExpr) -> RuntimeDefinition {
    let mut next_id = 0usize;
    let segment = build_runtime_segment(nodes, &mut next_id);
    RuntimeDefinition {
        nodes: segment.nodes,
        edges: segment.edges,
    }
}

fn build_runtime_segment(node: &NodeExpr, next_id: &mut usize) -> RuntimeSegment {
    match node {
        NodeExpr::Single(call) => {
            let id = *next_id;
            *next_id += 1;
            let name = runtime_node_name(call);
            RuntimeSegment {
                entries: vec![id],
                exits: vec![id],
                nodes: vec![RuntimeNodeDef { id, name }],
                edges: Vec::new(),
            }
        }
        NodeExpr::Sequence(nodes) => {
            let mut iter = nodes.iter();
            let first = iter
                .next()
                .unwrap_or_else(|| panic!("sequence must contain at least one node"));
            let mut acc = build_runtime_segment(first, next_id);

            for current in iter {
                let next = build_runtime_segment(current, next_id);
                for from in &acc.exits {
                    for to in &next.entries {
                        acc.edges.push(RuntimeEdgeDef {
                            from: *from,
                            to: *to,
                        });
                    }
                }

                acc.nodes.extend(next.nodes);
                acc.edges.extend(next.edges);
                acc.exits = next.exits;
            }

            acc
        }
        NodeExpr::Parallel(nodes) => {
            let mut entries = Vec::new();
            let mut exits = Vec::new();
            let mut all_nodes = Vec::new();
            let mut all_edges = Vec::new();

            for node in nodes {
                let segment = build_runtime_segment(node, next_id);
                entries.extend(segment.entries);
                exits.extend(segment.exits);
                all_nodes.extend(segment.nodes);
                all_edges.extend(segment.edges);
            }

            RuntimeSegment {
                entries,
                exits,
                nodes: all_nodes,
                edges: all_edges,
            }
        }
        NodeExpr::Route(route) => {
            if route.routes.is_empty() {
                panic!("route must contain at least one branch");
            }

            let mut entries = Vec::new();
            let mut exits = Vec::new();
            let mut all_nodes = Vec::new();
            let mut all_edges = Vec::new();

            for (_, route_node) in &route.routes {
                let segment = build_runtime_segment(route_node, next_id);
                entries.extend(segment.entries);
                exits.extend(segment.exits);
                all_nodes.extend(segment.nodes);
                all_edges.extend(segment.edges);
            }

            RuntimeSegment {
                entries,
                exits,
                nodes: all_nodes,
                edges: all_edges,
            }
        }
    }
}

fn runtime_node_name(call: &NodeCall) -> String {
    let mut path = call.path.to_token_stream().to_string();
    path.retain(|ch| !ch.is_whitespace());
    path
}
