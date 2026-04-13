use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
use std::collections::BTreeSet;
use syn::parse_macro_input;

use crate::shared::{
    ExprShape, GeneratedExpr, GraphInput, NodeCall, NodeExpr, Payload, RouteExpr, UsageMap,
    fresh_ident, is_graph_run_path,
};

// Graph expansion owns the interesting part of the project.
// It reads the parsed graph IR and emits hop-scoped Rust code:
// - each `>>` creates a temporary payload
// - the next step consumes only what it needs
// - fan-out clones only when multiple immediate consumers require the same artifact
// - artifacts die once the hop finishes unless a node re-emits them

/// Expands a `graph!` definition into a graph configuration type plus a
/// `graphio::Graph` implementation.
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

    let execute_body = if graph_inputs.is_empty() && graph_outputs.is_empty() {
        quote! {
            Self::__graphio_run(ctx);
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

    let expanded = quote! {
        pub struct #name;

        impl #name {
            /// Convenience entry point that executes the graph directly.
            pub fn run(ctx: &mut #context) {
                <Self as ::graphio::Graph<#context>>::execute(&Self, ctx);
            }

            pub fn __graphio_run(
                ctx: &mut #context,
                #( #run_params ),*
            ) #run_return_sig {
                #run_body
            }
        }

        impl ::graphio::Graph<#context> for #name {
            fn execute(&self, ctx: &mut #context) {
                let _ = self;
                #execute_body
            }
        }
    };

    TokenStream::from(expanded)
}

/// Expands a `graph_runtime!` definition into a runtime graph value factory
/// plus static metadata (nodes/edges) and executable behavior.
pub fn expand_runtime(input: TokenStream) -> TokenStream {
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

/// Dispatches code generation to the correct handler for the current graph IR
/// node.
/// This function is called recursively to generate code for nested graphs and subexpressions.
fn get_node_expr(node: &NodeExpr, incoming: &Payload, counter: &mut usize) -> GeneratedExpr {
    match node {
        // the graph has a single node, so we can skip the sequence logic and generate it directly in the root scope.
        NodeExpr::Single(call) => get_single_node_expr(call, incoming, counter),
        // the graph has at least 2 nodes all connected by `>>`
        NodeExpr::Sequence(nodes) => get_sequence_nodes_expr(nodes, incoming, counter),
        // the graph has at least 2 nodes all connected by `|` (parallel fan-out)
        NodeExpr::Parallel(nodes) => get_parallel_nodes_expr(nodes, incoming, counter),
        // the graph has an exclusive route with multiple branches
        NodeExpr::Route(route) => get_route_node_expr(route, incoming, counter),
    }
}

/// Generates code for a single node invocation or nested graph execution call,
/// consuming artifacts from the incoming hop payload and optionally producing a
/// new outgoing payload.
fn get_single_node_expr(call: &NodeCall, incoming: &Payload, counter: &mut usize) -> GeneratedExpr {
    let node_path = &call.path;

    let nested_graph_path = is_graph_run_path(node_path).then(|| graph_type_path(node_path));

    // Node with no inputs and no outputs
    if !call.explicit_inputs && call.inputs.is_empty() && call.outputs.is_empty() {
        let run_tokens = if let Some(graph_path) = &nested_graph_path {
            quote! { #graph_path::__graphio_run(ctx); }
        } else {
            quote! { #node_path::__graphio_run(ctx); }
        };

        return GeneratedExpr {
            tokens: run_tokens,
            outputs: Payload::new(),
        };
    }

    // Inside a single node call, the same artifact name might appear more than
    // once. We count those local reads so the last one can move and earlier
    // ones clone from the same incoming payload.
    let mut remaining = UsageMap::new();
    for input in &call.inputs {
        // for each input, increment the usage count for that input name so we know how many times it will be consumed in this node.
        *remaining.entry(input.to_string()).or_insert(0) += 1;
    }

    // list of local variable names we generate for the node inputs, which will be used in the generated code to refer to the incoming artifacts.
    let mut arg_vars = Vec::with_capacity(call.inputs.len());
    // arg_vars and input_bindings correspond to each other by index: for each input, is generates a
    // local variable name for it and a code snippet that initializes that variable by taking or cloning from the incoming payload.
    let mut input_bindings = Vec::with_capacity(call.inputs.len());

    for input in &call.inputs {
        let artifact_name = input.to_string();
        let source = incoming
            .get(&artifact_name)
            .unwrap_or_else(|| panic!("missing artifact `{artifact_name}` for node call"));
        let remaining_uses = remaining
            .get_mut(&artifact_name)
            .unwrap_or_else(|| panic!("missing usage count for `{artifact_name}`"));
        let arg_ident = fresh_ident(counter, "arg", &artifact_name);

        if *remaining_uses == 1 {
            input_bindings.push(quote! {
                let #arg_ident = #source
                    .take()
                    .unwrap_or_else(|| panic!(concat!("missing artifact `", stringify!(#input), "`")));
            });
        } else {
            input_bindings.push(quote! {
                let #arg_ident = ::graphio::clone_artifact(
                    #source
                        .as_ref()
                        .unwrap_or_else(|| panic!(concat!("missing artifact `", stringify!(#input), "`")))
                );
            });
        }

        *remaining_uses -= 1;
        arg_vars.push(arg_ident);
    }

    // Node with input and no inputs
    if call.outputs.is_empty() {
        let run_call = if let Some(graph_path) = &nested_graph_path {
            quote! { #graph_path::__graphio_run(ctx, #( #arg_vars ),*); }
        } else {
            quote! { #node_path::__graphio_run(ctx, #( #arg_vars ),*); }
        };

        return GeneratedExpr {
            tokens: quote! {
                #( #input_bindings )*
                #run_call
            },
            outputs: Payload::new(),
        };
    }

    // A node with outputs becomes the producer for the next hop.
    // We hold each produced artifact in an `Option<T>` so later code can decide
    // whether to move it (`take`) or clone from it.
    let mut outputs = Payload::new();
    // node with a single output
    if call.outputs.len() == 1 {
        let artifact_name = call.outputs[0].to_string();
        let output_var = fresh_ident(counter, "hop", &artifact_name);
        outputs.insert(artifact_name, output_var.clone());
        let run_call = if let Some(graph_path) = &nested_graph_path {
            quote! { #graph_path::__graphio_run(ctx, #( #arg_vars ),*) }
        } else {
            quote! { #node_path::__graphio_run(ctx, #( #arg_vars ),*) }
        };

        GeneratedExpr {
            tokens: quote! {
                #( #input_bindings )*
                let mut #output_var = ::std::option::Option::Some(#run_call);
            },
            outputs,
        }
    // node with multiple outputs
    } else {
        let tuple_vars: Vec<syn::Ident> = call
            .outputs
            .iter()
            .map(|output| fresh_ident(counter, "ret", &output.to_string()))
            .collect();
        let output_stores =
            call.outputs
                .iter()
                .zip(tuple_vars.iter())
                .map(|(output, tuple_var)| {
                    let artifact_name = output.to_string();
                    let output_var = fresh_ident(counter, "hop", &artifact_name);
                    outputs.insert(artifact_name, output_var.clone());
                    quote! {
                        let mut #output_var = ::std::option::Option::Some(#tuple_var);
                    }
                });
        let run_call = if let Some(graph_path) = &nested_graph_path {
            quote! { #graph_path::__graphio_run(ctx, #( #arg_vars ),*) }
        } else {
            quote! { #node_path::__graphio_run(ctx, #( #arg_vars ),*) }
        };

        GeneratedExpr {
            tokens: quote! {
                #( #input_bindings )*
                let ( #( #tuple_vars ),* ) = #run_call;
                #( #output_stores )*
            },
            outputs,
        }
    }
}

/// Extracts the graph type path from a `SomeGraph::run` path so nested graphs
/// can be executed through its generated entrypoint.
fn graph_type_path(path: &syn::Path) -> syn::Path {
    let mut graph_path = path.clone();
    graph_path.segments.pop();
    graph_path.segments.pop_punct();

    if graph_path.segments.is_empty() {
        panic!("invalid graph run path");
    }

    graph_path
}

/// Builds a fresh hop payload by moving the requested artifacts out of the
/// current expression outputs.
fn prepare_move_payload(
    source_outputs: &Payload,
    artifacts: &[String],
    prefix: &str,
    counter: &mut usize,
) -> (Payload, Vec<proc_macro2::TokenStream>) {
    let mut payload = Payload::new();
    let mut bindings = Vec::with_capacity(artifacts.len());

    for artifact in artifacts {
        let source = source_outputs
            .get(artifact)
            .unwrap_or_else(|| panic!("missing artifact `{artifact}` for next hop"));
        let payload_var = fresh_ident(counter, prefix, artifact);
        payload.insert(artifact.clone(), payload_var.clone());
        bindings.push(quote! {
            let mut #payload_var = #source.take();
        });
    }

    (payload, bindings)
}

/// Builds one child payload for a parallel fan-out branch.
///
/// Shared artifacts are cloned for early consumers and moved for the last one.
fn prepare_parallel_payload(
    incoming: &Payload,
    shape: &ExprShape,
    remaining: &mut UsageMap,
    counter: &mut usize,
) -> (Payload, Vec<proc_macro2::TokenStream>) {
    let mut payload = Payload::new();
    let mut bindings = Vec::new();

    for artifact in required_artifacts(shape) {
        let source = incoming
            .get(&artifact)
            .unwrap_or_else(|| panic!("missing artifact `{artifact}` for parallel step"));
        let remaining_children = remaining
            .get_mut(&artifact)
            .unwrap_or_else(|| panic!("missing usage count for `{artifact}`"));
        let payload_var = fresh_ident(counter, "parallel_in", &artifact);
        payload.insert(artifact.clone(), payload_var.clone());

        if *remaining_children == 1 {
            bindings.push(quote! {
                let mut #payload_var = #source.take();
            });
        } else {
            bindings.push(quote! {
                let mut #payload_var = ::std::option::Option::Some(::graphio::clone_artifact(
                    #source
                        .as_ref()
                        .unwrap_or_else(|| panic!(concat!("missing artifact `", #artifact, "`")))
                ));
            });
        }

        *remaining_children -= 1;
    }

    (payload, bindings)
}

/// Declares outer slots for the outputs of a composite expression.
fn prepare_output_slots(
    artifacts: &[String],
    prefix: &str,
    counter: &mut usize,
) -> (Payload, Vec<proc_macro2::TokenStream>) {
    let mut outputs = Payload::new();
    let mut declarations = Vec::with_capacity(artifacts.len());

    for artifact in artifacts {
        let output_var = fresh_ident(counter, prefix, artifact);
        outputs.insert(artifact.clone(), output_var.clone());
        declarations.push(quote! {
            let mut #output_var = ::std::option::Option::None;
        });
    }

    (outputs, declarations)
}

/// Emits assignments that copy a child expression's output locals into the
/// outer slots owned by the parent composite expression.
fn assign_outputs_to_slots(
    inner_outputs: &Payload,
    outer_outputs: &Payload,
) -> Vec<proc_macro2::TokenStream> {
    inner_outputs
        .iter()
        .map(|(artifact, inner)| {
            let outer = outer_outputs
                .get(artifact)
                .unwrap_or_else(|| panic!("missing output slot for `{artifact}`"));
            quote! {
                #outer = #inner;
            }
        })
        .collect()
}

/// Counts how many sibling branches require each artifact at the entry of a
/// parallel expression.
fn collect_parallel_entry_usage(shapes: &[ExprShape]) -> UsageMap {
    let mut remaining = UsageMap::new();

    for shape in shapes {
        for artifact in required_artifacts(shape) {
            *remaining.entry(artifact).or_insert(0) += 1;
        }
    }

    remaining
}

/// Generates code for `A >> B >> C` style execution by forwarding only the
/// artifacts required by the immediate next step at each hop boundary.
fn get_sequence_nodes_expr(
    nodes: &[NodeExpr],
    incoming: &Payload,
    counter: &mut usize,
) -> GeneratedExpr {
    let mut iter = nodes.iter();
    let first = iter
        .next()
        .expect("sequence must contain at least one node");
    let mut current = get_node_expr(first, incoming, counter);

    for next in iter {
        let shape = analyze_expr(next);
        let required = required_artifacts(&shape);
        // A sequence boundary is the core one-hop rule:
        // take only the artifacts the next step needs, build a fresh payload for
        // that step, and let everything else drop here.
        let (next_payload, transfer_tokens) =
            prepare_move_payload(&current.outputs, &required, "payload", counter);

        let next_generated = get_node_expr(next, &next_payload, counter);
        let current_tokens = current.tokens;
        let next_tokens = next_generated.tokens;
        current = capture_outputs(
            quote! {
                #current_tokens
                #( #transfer_tokens )*
                #next_tokens
            },
            next_generated.outputs,
            counter,
        );
    }

    current
}

/// Generates code for a fan-out step where multiple sibling branches consume
/// the same incoming hop payload.
fn get_parallel_nodes_expr(
    nodes: &[NodeExpr],
    incoming: &Payload,
    counter: &mut usize,
) -> GeneratedExpr {
    let shapes: Vec<ExprShape> = nodes.iter().map(analyze_expr).collect();
    let mut remaining = collect_parallel_entry_usage(&shapes);

    let exit_outputs = collect_parallel_outputs(&shapes);
    let (outputs, output_decl_tokens) =
        prepare_output_slots(&exit_outputs, "parallel_out", counter);

    let mut blocks = Vec::new();
    for (node, shape) in nodes.iter().zip(shapes.iter()) {
        // A parallel hop distributes the same incoming payload to multiple
        // sibling steps. Early consumers clone if another sibling still needs
        // the artifact; the last sibling moves it.
        let (child_payload, child_bindings) =
            prepare_parallel_payload(incoming, shape, &mut remaining, counter);

        let generated = get_node_expr(node, &child_payload, counter);
        let generated_tokens = generated.tokens;
        let output_assigns = assign_outputs_to_slots(&generated.outputs, &outputs);

        blocks.push(quote! {
            {
                #( #child_bindings )*
                #generated_tokens
                #( #output_assigns )*
            }
        });
    }

    GeneratedExpr {
        tokens: quote! {
            #( #output_decl_tokens )*
            #( #blocks )*
        },
        outputs,
    }
}

/// Generates code for an exclusive route branch, moving the incoming hop
/// payload into the selected branch only.
fn get_route_node_expr(
    route: &RouteExpr,
    incoming: &Payload,
    counter: &mut usize,
) -> GeneratedExpr {
    let branch_shapes: Vec<ExprShape> = route
        .routes
        .iter()
        .map(|(_, node)| analyze_expr(node))
        .collect();
    let exit_outputs = collect_route_outputs(&branch_shapes);

    let (outputs, output_decl_tokens) = prepare_output_slots(&exit_outputs, "route_out", counter);

    let on_expr = &route.on;
    let mut arms = Vec::new();
    for ((key, node), shape) in route.routes.iter().zip(branch_shapes.iter()) {
        // Route branches are exclusive: only one branch runs, so inputs are
        // moved straight into that branch payload.
        let artifacts = required_artifacts(shape);
        let (branch_payload, branch_bindings) =
            prepare_move_payload(incoming, &artifacts, "route_in", counter);

        let generated = get_node_expr(node, &branch_payload, counter);
        let generated_tokens = generated.tokens;
        let output_assigns = assign_outputs_to_slots(&generated.outputs, &outputs);

        arms.push(quote! {
            #key => {
                #( #branch_bindings )*
                #generated_tokens
                #( #output_assigns )*
            }
        });
    }

    GeneratedExpr {
        tokens: quote! {
            #( #output_decl_tokens )*
            match (#on_expr)(ctx) {
                #( #arms, )*
            }
        },
        outputs,
    }
}

/// Rebinds outputs created inside a nested scope into fresh outer locals so the
/// parent expression can keep propagating the hop payload.
fn capture_outputs(
    inner_tokens: proc_macro2::TokenStream,
    inner_outputs: Payload,
    counter: &mut usize,
) -> GeneratedExpr {
    if inner_outputs.is_empty() {
        return GeneratedExpr {
            tokens: quote! {{
                #inner_tokens
            }},
            outputs: Payload::new(),
        };
    }

    // Nested expressions produce payload variables inside inner scopes.
    // We rebind them into fresh outer locals so the parent sequence step can
    // continue propagating the new hop.
    let mut outer_outputs = Payload::new();
    let declaration_pairs: Vec<(String, syn::Ident)> = inner_outputs
        .keys()
        .map(|artifact| {
            let outer_var = fresh_ident(counter, "captured", artifact);
            (artifact.clone(), outer_var)
        })
        .collect();

    for (artifact, outer_var) in &declaration_pairs {
        outer_outputs.insert(artifact.clone(), outer_var.clone());
    }

    let declarations = declaration_pairs.iter().map(|(_, outer_var)| {
        quote! {
            let mut #outer_var = ::std::option::Option::None;
        }
    });

    let assignments = inner_outputs.iter().map(|(artifact, inner)| {
        let outer = outer_outputs
            .get(artifact)
            .unwrap_or_else(|| panic!("missing captured output slot for `{artifact}`"));
        quote! {
            #outer = #inner;
        }
    });

    GeneratedExpr {
        tokens: quote! {
            #( #declarations )*
            {
                #inner_tokens
                #( #assignments )*
            }
        },
        outputs: outer_outputs,
    }
}

/// Computes the entry requirements and possible exit artifacts of a graph
/// expression without generating executable code.
fn analyze_expr(node: &NodeExpr) -> ExprShape {
    match node {
        NodeExpr::Single(call) => analyze_single(call),
        NodeExpr::Sequence(nodes) => {
            let first = nodes
                .first()
                .unwrap_or_else(|| panic!("sequence must contain at least one node"));
            let last = nodes
                .last()
                .unwrap_or_else(|| panic!("sequence must contain at least one node"));

            ExprShape {
                entry_usage: analyze_expr(first).entry_usage,
                exit_outputs: analyze_expr(last).exit_outputs,
            }
        }
        NodeExpr::Parallel(nodes) => {
            let shapes: Vec<ExprShape> = nodes.iter().map(analyze_expr).collect();

            ExprShape {
                entry_usage: collect_parallel_entry_usage(&shapes),
                exit_outputs: collect_parallel_outputs(&shapes),
            }
        }
        NodeExpr::Route(route) => {
            let shapes: Vec<ExprShape> = route
                .routes
                .iter()
                .map(|(_, node)| analyze_expr(node))
                .collect();
            let mut entry_usage = UsageMap::new();

            // A route only executes one branch, so from the caller's point of
            // view an artifact is required at most once at route entry.
            for shape in &shapes {
                for artifact in required_artifacts(shape) {
                    entry_usage.entry(artifact).or_insert(1);
                }
            }

            ExprShape {
                entry_usage,
                exit_outputs: collect_route_outputs(&shapes),
            }
        }
    }
}

/// Computes the shape of a single node call: which artifacts must already be
/// available and which artifact names can leave the node.
fn analyze_single(call: &NodeCall) -> ExprShape {
    if !call.explicit_inputs && call.inputs.is_empty() && call.outputs.is_empty() {
        return ExprShape {
            entry_usage: UsageMap::new(),
            exit_outputs: Vec::new(),
        };
    }

    let mut entry_usage = UsageMap::new();
    for input in &call.inputs {
        *entry_usage.entry(input.to_string()).or_insert(0) += 1;
    }

    ExprShape {
        entry_usage,
        exit_outputs: call.outputs.iter().map(ToString::to_string).collect(),
    }
}

/// Returns the ordered list of artifact names required at the entry of a graph
/// subexpression.
fn required_artifacts(shape: &ExprShape) -> Vec<String> {
    shape.entry_usage.keys().cloned().collect()
}

/// Collects and validates the outgoing artifact names of a parallel step,
/// rejecting duplicates because sibling outputs would collide.
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

/// Collects the union of artifact names that may leave a route expression
/// across its possible branches.
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
