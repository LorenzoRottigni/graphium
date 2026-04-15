use proc_macro::TokenStream;
use quote::quote;
use std::collections::{BTreeMap, BTreeSet};
use syn::parse_macro_input;

use crate::shared::{
    ExprShape, GeneratedExpr, GraphInput, LoopExpr, NodeCall, NodeExpr, Payload, RouteExpr,
    UsageMap, WhileExpr,
    fresh_ident, is_graph_run_path,
};

enum SelectorParam {
    Ctx { mutable: bool },
    Artifact { ident: syn::Ident, borrowed: bool },
}

fn node_run_call_tokens(
    node_path: &syn::Path,
    nested_graph_path: Option<&syn::Path>,
    ctx_arg: proc_macro2::TokenStream,
    arg_vars: &[syn::Ident],
    async_mode: bool,
) -> proc_macro2::TokenStream {
    let call = if let Some(graph_path) = nested_graph_path {
        if async_mode {
            quote! { #graph_path::__graphium_run_async(#ctx_arg, #( #arg_vars ),*) }
        } else {
            quote! { #graph_path::__graphium_run(#ctx_arg, #( #arg_vars ),*) }
        }
    } else if async_mode {
        quote! { #node_path::__graphium_run_async(#ctx_arg, #( #arg_vars ),*) }
    } else {
        quote! { #node_path::__graphium_run(#ctx_arg, #( #arg_vars ),*) }
    };

    if async_mode {
        quote! { #call.await }
    } else {
        call
    }
}

fn selector_params_for_on_expr(on: &syn::Expr) -> Vec<SelectorParam> {
    if let syn::Expr::Closure(_closure) = on {
        return parse_selector_params(on);
    }

    if let syn::Expr::Path(path) = on {
        if path.qself.is_none() && path.path.segments.len() == 1 {
            let ident = path.path.segments[0].ident.clone();
            return vec![SelectorParam::Artifact {
                ident,
                borrowed: false,
            }];
        }
    }

    Vec::new()
}

// Graph expansion owns the interesting part of the project.
// It reads the parsed graph IR and emits hop-scoped Rust code:
// - each `>>` creates a temporary payload
// - the next step consumes only what it needs
// - fan-out clones only when multiple immediate consumers require the same artifact
// - artifacts die once the hop finishes unless a node re-emits them

/// Expands a `graph!` definition into a graph configuration type plus a
/// `graphium::Graph::run` implementation.
pub fn expand(input: TokenStream) -> TokenStream {
    let GraphInput {
        name,
        context,
        inputs: graph_inputs,
        outputs: graph_outputs,
        nodes,
        async_enabled,
    } = parse_macro_input!(input as GraphInput);

    // for unique local variable names for hop payloads and node outputs.
    let mut counter = 0usize;
    // initial payload available at the root of the graph, which contains the graph inputs.
    let mut root_incoming = Payload::new();
    // array of tokens that declare the `run` entrypoint parameters for the graph inputs.
    let mut run_params = Vec::with_capacity(graph_inputs.len());
    // array of tokens that bind the `run` entrypoint parameters into the initial root payload.
    let mut root_input_bindings = Vec::with_capacity(graph_inputs.len());

    // for each graph input, bind the `run` entrypoint parameter into the initial root payload and prepare a slot for it in the incoming payload of the first node(s).
    for (artifact, ty) in &graph_inputs {
        let param_ident = fresh_ident(&mut counter, "graph_in", &artifact.to_string());
        let payload_ident = fresh_ident(&mut counter, "root_in", &artifact.to_string());
        root_incoming.insert_owned(artifact.to_string(), payload_ident.clone());
        run_params.push(quote! {
            #param_ident: #ty
        });
        root_input_bindings.push(quote! {
            let mut #payload_ident = ::std::option::Option::Some(#param_ident);
        });
    }

    // recursively generate the graph body, which produces the root outgoing payload containing the graph outputs.
    let generated = if async_enabled {
        None
    } else {
        Some(get_node_expr(&nodes, &root_incoming, &mut counter, false, false))
    };
    let generated_async = get_node_expr(&nodes, &root_incoming, &mut counter, false, true);

    // AST Graph::run() return type signature
    let run_return_sig = if graph_outputs.is_empty() {
        // no outputs => `()`
        quote! {}
    } else if graph_outputs.len() == 1 {
        // 1 output => <output>
        let (_, ty) = &graph_outputs[0];
        quote! { -> #ty }
    } else {
        // N outputs => `(A,B,C)`
        let tys = graph_outputs.iter().map(|(_, ty)| ty);
        quote! { -> ( #( #tys ),* ) }
    };

    // AST Graph::run() body
    let run_body = if async_enabled {
        quote! {}
    } else if graph_outputs.is_empty() {
        let generated_tokens = generated.as_ref().expect("sync graph").tokens.clone();
        quote! {{
            #( #root_input_bindings )*
            #generated_tokens
        }}
    } else {
        let generated_tokens = generated.as_ref().expect("sync graph").tokens.clone();
        let output_values: Vec<proc_macro2::TokenStream> = graph_outputs
            .iter()
            .map(|(artifact, _)| {
                let artifact_name = artifact.to_string();
                let output_var = generated
                    .as_ref()
                    .expect("sync graph")
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

    let run_body_async = if graph_outputs.is_empty() {
        let generated_tokens = generated_async.tokens.clone();
        quote! {{
            #( #root_input_bindings )*
            #generated_tokens
        }}
    } else {
        let generated_tokens = generated_async.tokens.clone();
        let output_values: Vec<proc_macro2::TokenStream> = graph_outputs
            .iter()
            .map(|(artifact, _)| {
                let artifact_name = artifact.to_string();
                let output_var = generated_async
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

    // AST Graph::run() trait body
    let trait_run_body = if graph_inputs.is_empty() && graph_outputs.is_empty() {
        quote! {
            Self::__graphium_run(ctx);
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

    let async_trait_run_body = if graph_inputs.is_empty() && graph_outputs.is_empty() {
        quote! {
            Self::__graphium_run_async(ctx).await;
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

    // AST expanded graph!
    let sync_impl = if async_enabled {
        quote! {}
    } else {
        quote! {
            pub fn run(ctx: &mut #context) {
                <Self as ::graphium::Graph<#context>>::run(ctx);
            }

            pub fn __graphium_run(
                ctx: &mut #context,
                #( #run_params ),*
            ) #run_return_sig {
                #run_body
            }
        }
    };

    let graph_impl = if async_enabled {
        quote! {}
    } else {
        quote! {
            impl ::graphium::Graph<#context> for #name {
                fn run(ctx: &mut #context) {
                    #trait_run_body
                }
            }
        }
    };

    let graph_def_tokens = graph_definition_tokens(&name, &nodes);

    let expanded = quote! {
        pub struct #name;

        impl #name {
            #sync_impl

            /// Convenience async entry point that executes the graph directly.
            pub async fn run_async(ctx: &mut #context) {
                #async_trait_run_body
            }

            pub async fn __graphium_run_async(
                ctx: &mut #context,
                #( #run_params ),*
            ) #run_return_sig {
                #run_body_async
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
    };

    TokenStream::from(expanded)
}

/// Dispatches code generation to the correct handler for the current graph IR
/// node.
/// This function is called recursively to generate code for nested graphs and subexpressions.
pub(crate) fn get_node_expr(
    node: &NodeExpr,
    incoming: &Payload,
    counter: &mut usize,
    in_loop: bool,
    async_mode: bool,
) -> GeneratedExpr {
    match node {
        // the graph has a single node, so we can skip the sequence logic and generate it directly in the root scope.
        NodeExpr::Single(call) => get_single_node_expr(call, incoming, counter, async_mode),
        // the graph has at least 2 nodes all connected by `>>`
        NodeExpr::Sequence(nodes) => {
            get_sequence_nodes_expr(nodes, incoming, counter, in_loop, async_mode)
        }
        // the graph has at least 2 nodes all connected by `|` (parallel fan-out)
        NodeExpr::Parallel(nodes) => {
            get_parallel_nodes_expr(nodes, incoming, counter, in_loop, async_mode)
        }
        // the graph has an exclusive route with multiple branches
        NodeExpr::Route(route) => get_route_node_expr(route, incoming, counter, in_loop, async_mode),
        NodeExpr::While(while_expr) => {
            get_while_node_expr(while_expr, incoming, counter, async_mode)
        }
        NodeExpr::Loop(loop_expr) => get_loop_node_expr(loop_expr, incoming, counter, async_mode),
        NodeExpr::Break => {
            if !in_loop {
                panic!("`@break` can only be used inside `@loop` or `@while`");
            }
            GeneratedExpr {
                tokens: quote! { break; },
                outputs: Payload::new(),
            }
        }
    }
}

/// Generates code for a single node invocation or nested graph execution call,
/// consuming artifacts from the incoming hop payload and optionally producing a
/// new outgoing payload.
fn get_single_node_expr(
    call: &NodeCall,
    incoming: &Payload,
    counter: &mut usize,
    async_mode: bool,
) -> GeneratedExpr {
    let node_path = &call.path;

    let nested_graph_path = is_graph_run_path(node_path).then(|| graph_type_path(node_path));
    let has_borrowed_inputs = call.input_borrows.iter().any(|is_borrowed| *is_borrowed);

    if nested_graph_path.is_some() && has_borrowed_inputs {
        panic!("borrowed artifacts are not supported when calling nested graphs");
    }

    // Node with no inputs and no outputs
    if !call.explicit_inputs && call.inputs.is_empty() && call.outputs.is_empty() {
        let run_tokens = node_run_call_tokens(
            node_path,
            nested_graph_path.as_ref(),
            quote! { ctx },
            &[],
            async_mode,
        );

        return GeneratedExpr {
            tokens: quote! { #run_tokens; },
            outputs: Payload::new(),
        };
    }

    // Inside a single node call, the same artifact name might appear more than
    // once. We count those local reads so the last one can move and earlier
    // ones clone from the same incoming payload.
    let mut remaining = UsageMap::new();
    for (input, is_borrowed) in call.inputs.iter().zip(call.input_borrows.iter()) {
        if *is_borrowed {
            continue;
        }
        // for each owned input, increment the usage count for that input name so we know how many times it will be consumed in this node.
        *remaining.entry(input.to_string()).or_insert(0) += 1;
    }

    // list of local variable names we generate for the node inputs, which will be used in the generated code to refer to the incoming artifacts.
    let mut arg_vars = Vec::with_capacity(call.inputs.len());
    // arg_vars and input_bindings correspond to each other by index: for each input, is generates a
    // local variable name for it and a code snippet that initializes that variable by taking or cloning from the incoming payload.
    let mut input_bindings = Vec::with_capacity(call.inputs.len());

    let mut input_by_name: BTreeMap<String, (syn::Ident, bool)> = BTreeMap::new();
    let mut ctx_clone_bindings = Vec::new();
    let mut ctx_store_bindings = Vec::new();
    let mut ctx_cloned = BTreeSet::new();

    let borrowed_outputs: Vec<syn::Ident> = call
        .outputs
        .iter()
        .zip(call.output_borrows.iter())
        .filter_map(|(output, is_borrowed)| {
            if *is_borrowed {
                Some(output.clone())
            } else {
                None
            }
        })
        .collect();

    for (input, is_borrowed) in call.inputs.iter().zip(call.input_borrows.iter()) {
        let artifact_name = input.to_string();
        let arg_ident = fresh_ident(counter, "arg", &artifact_name);

        if *is_borrowed {
            if !incoming.has_borrowed(&artifact_name) {
                panic!("missing borrowed artifact `{artifact_name}` for node call");
            }
            input_bindings.push(quote! {
                let #arg_ident = &ctx.#input;
            });
            input_by_name
                .entry(artifact_name.clone())
                .or_insert((arg_ident.clone(), true));
            arg_vars.push(arg_ident);
            continue;
        }

        let source = incoming
            .get_owned(&artifact_name)
            .unwrap_or_else(|| panic!("missing artifact `{artifact_name}` for node call"));
        let remaining_uses = remaining
            .get_mut(&artifact_name)
            .unwrap_or_else(|| panic!("missing usage count for `{artifact_name}`"));

        if *remaining_uses == 1 {
            input_bindings.push(quote! {
                let #arg_ident = #source
                    .take()
                    .unwrap_or_else(|| panic!(concat!("missing artifact `", stringify!(#input), "`")));
            });
        } else {
            input_bindings.push(quote! {
                let #arg_ident = ::graphium::clone_artifact(
                    #source
                        .as_ref()
                        .unwrap_or_else(|| panic!(concat!("missing artifact `", stringify!(#input), "`")))
                );
            });
        }

        *remaining_uses -= 1;
        input_by_name
            .entry(artifact_name.clone())
            .or_insert((arg_ident.clone(), false));
        arg_vars.push(arg_ident);
    }

    // Prepare clones for borrowed outputs sourced from inputs.
    if !borrowed_outputs.is_empty() {
        for output in &borrowed_outputs {
            let output_name = output.to_string();
            if let Some((arg_ident, input_is_borrowed)) = input_by_name.get(&output_name) {
                if ctx_cloned.insert(output_name.clone()) {
                    let clone_ident = fresh_ident(counter, "ctx_clone", &output_name);
                    let clone_expr = if *input_is_borrowed {
                        quote! { ::graphium::clone_artifact(#arg_ident) }
                    } else {
                        quote! { ::graphium::clone_artifact(&#arg_ident) }
                    };
                    ctx_clone_bindings.push(quote! {
                        let #clone_ident = #clone_expr;
                    });
                    ctx_store_bindings.push(quote! {
                        ctx.#output = #clone_ident;
                    });
                }
            }
        }
    }

    // Node with input and no inputs
    if call.outputs.is_empty() {
        let ctx_arg = if has_borrowed_inputs {
            quote! { &*ctx }
        } else {
            quote! { ctx }
        };
        let run_call = node_run_call_tokens(
            node_path,
            nested_graph_path.as_ref(),
            ctx_arg,
            &arg_vars,
            async_mode,
        );

        return GeneratedExpr {
            tokens: quote! {
                #( #input_bindings )*
                #( #ctx_clone_bindings )*
                #run_call;
                #( #ctx_store_bindings )*
            },
            outputs: Payload::new(),
        };
    }

    // A node with outputs becomes the producer for the next hop.
    // We hold each produced artifact in an `Option<T>` so later code can decide
    // whether to move it (`take`) or clone from it.
    let mut outputs = Payload::new();
    let ctx_arg = if has_borrowed_inputs {
        quote! { &*ctx }
    } else {
        quote! { ctx }
    };
    let run_call = node_run_call_tokens(
        node_path,
        nested_graph_path.as_ref(),
        ctx_arg,
        &arg_vars,
        async_mode,
    );

    let mut borrowed_from_return = Vec::new();
    for (output, is_borrowed) in call.outputs.iter().zip(call.output_borrows.iter()) {
        if *is_borrowed && !input_by_name.contains_key(&output.to_string()) {
            borrowed_from_return.push(output.to_string());
        }
    }

    // node with a single output
    if call.outputs.len() == 1 {
        let artifact_name = call.outputs[0].to_string();
        let is_borrowed = call.output_borrows[0];

        if is_borrowed {
            outputs.insert_borrowed(artifact_name.clone());
            let return_var = fresh_ident(counter, "ret", &artifact_name);
            let return_binding = if borrowed_from_return.contains(&artifact_name) {
                quote! { let #return_var = #run_call; }
            } else {
                quote! { #run_call; }
            };
            let store_binding = if borrowed_from_return.contains(&artifact_name) {
                let output_ident = &call.outputs[0];
                quote! { ctx.#output_ident = #return_var; }
            } else {
                quote! {}
            };

            return GeneratedExpr {
                tokens: quote! {
                    #( #input_bindings )*
                    #( #ctx_clone_bindings )*
                    #return_binding
                    #( #ctx_store_bindings )*
                    #store_binding
                },
                outputs,
            };
        }

        let output_var = fresh_ident(counter, "hop", &artifact_name);
        outputs.insert_owned(artifact_name, output_var.clone());

        GeneratedExpr {
            tokens: quote! {
                #( #input_bindings )*
                #( #ctx_clone_bindings )*
                let mut #output_var = ::std::option::Option::Some(#run_call);
                #( #ctx_store_bindings )*
            },
            outputs,
        }
    } else {
        let tuple_vars: Vec<syn::Ident> = call
            .outputs
            .iter()
            .map(|output| fresh_ident(counter, "ret", &output.to_string()))
            .collect();

        let mut output_stores = Vec::new();
        let mut borrowed_store = Vec::new();
        for ((output, is_borrowed), tuple_var) in call
            .outputs
            .iter()
            .zip(call.output_borrows.iter())
            .zip(tuple_vars.iter())
        {
            let artifact_name = output.to_string();
            if *is_borrowed {
                outputs.insert_borrowed(artifact_name.clone());
                borrowed_store.push(quote! {
                    ctx.#output = #tuple_var;
                });
            } else {
                let output_var = fresh_ident(counter, "hop", &artifact_name);
                outputs.insert_owned(artifact_name, output_var.clone());
                output_stores.push(quote! {
                    let mut #output_var = ::std::option::Option::Some(#tuple_var);
                });
            }
        }

        GeneratedExpr {
            tokens: quote! {
                #( #input_bindings )*
                #( #ctx_clone_bindings )*
                let ( #( #tuple_vars ),* ) = #run_call;
                #( #output_stores )*
                #( #ctx_store_bindings )*
                #( #borrowed_store )*
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
    payload.borrowed = source_outputs.borrowed.clone();
    let mut bindings = Vec::with_capacity(artifacts.len());

    for artifact in artifacts {
        let source = source_outputs
            .get_owned(artifact)
            .unwrap_or_else(|| panic!("missing artifact `{artifact}` for next hop"));
        let payload_var = fresh_ident(counter, prefix, artifact);
        payload.insert_owned(artifact.clone(), payload_var.clone());
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
            .get_owned(&artifact)
            .unwrap_or_else(|| panic!("missing artifact `{artifact}` for parallel step"));
        let remaining_children = remaining
            .get_mut(&artifact)
            .unwrap_or_else(|| panic!("missing usage count for `{artifact}`"));
        let payload_var = fresh_ident(counter, "parallel_in", &artifact);
        payload.insert_owned(artifact.clone(), payload_var.clone());

        if *remaining_children == 1 {
            bindings.push(quote! {
                let mut #payload_var = #source.take();
            });
        } else {
            bindings.push(quote! {
                let mut #payload_var = ::std::option::Option::Some(::graphium::clone_artifact(
                    #source
                        .as_ref()
                        .unwrap_or_else(|| panic!(concat!("missing artifact `", #artifact, "`")))
                ));
            });
        }

        *remaining_children -= 1;
    }

    for artifact in required_borrowed(shape) {
        payload.insert_borrowed(artifact);
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
        outputs.insert_owned(artifact.clone(), output_var.clone());
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
        .owned
        .iter()
        .map(|(artifact, inner)| {
            let outer = outer_outputs
                .get_owned(artifact)
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
    in_loop: bool,
    async_mode: bool,
) -> GeneratedExpr {
    let mut iter = nodes.iter();
    let first = iter
        .next()
        .expect("sequence must contain at least one node");
    let mut current = get_node_expr(first, incoming, counter, in_loop, async_mode);

    for next in iter {
        let shape = analyze_expr(next);
        let required = required_artifacts(&shape);
        // A sequence boundary is the core one-hop rule:
        // take only the artifacts the next step needs, build a fresh payload for
        // that step, and let everything else drop here.
        let (mut next_payload, transfer_tokens) =
            prepare_move_payload(&current.outputs, &required, "payload", counter);
        for artifact in required_borrowed(&shape) {
            next_payload.insert_borrowed(artifact);
        }

        let next_generated = get_node_expr(next, &next_payload, counter, in_loop, async_mode);
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
    in_loop: bool,
    async_mode: bool,
) -> GeneratedExpr {
    let shapes: Vec<ExprShape> = nodes.iter().map(analyze_expr).collect();
    let mut remaining = collect_parallel_entry_usage(&shapes);

    let exit_outputs = collect_parallel_outputs(&shapes);
    let exit_borrowed = collect_parallel_borrowed(&shapes);
    let (mut outputs, output_decl_tokens) =
        prepare_output_slots(&exit_outputs, "parallel_out", counter);
    outputs.borrowed = exit_borrowed;

    let mut blocks = Vec::new();
    for (node, shape) in nodes.iter().zip(shapes.iter()) {
        // A parallel hop distributes the same incoming payload to multiple
        // sibling steps. Early consumers clone if another sibling still needs
        // the artifact; the last sibling moves it.
        let (child_payload, child_bindings) =
            prepare_parallel_payload(incoming, shape, &mut remaining, counter);

        let generated = get_node_expr(node, &child_payload, counter, in_loop, async_mode);
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
    in_loop: bool,
    async_mode: bool,
) -> GeneratedExpr {
    let branch_shapes: Vec<ExprShape> = route
        .routes
        .iter()
        .map(|(_, node)| analyze_expr(node))
        .collect();
    if !route.outputs.is_empty() {
        validate_route_outputs(route, &branch_shapes);
    }
    let (exit_outputs, exit_borrowed) = route_exit_outputs(route, &branch_shapes);

    let (mut outputs, output_decl_tokens) =
        prepare_output_slots(&exit_outputs, "route_out", counter);
    outputs.borrowed = exit_borrowed;

    let on_expr = &route.on;
    let selector_params = selector_params_for_on_expr(on_expr);
    let mut needed_by_branches = BTreeSet::new();
    for shape in &branch_shapes {
        for artifact in required_artifacts(shape) {
            needed_by_branches.insert(artifact);
        }
        for artifact in required_borrowed(shape) {
            needed_by_branches.insert(artifact);
        }
    }

    let selector_tokens = build_selector_bindings(
        &selector_params,
        incoming,
        &needed_by_branches,
        counter,
    );
    let selector_call = build_selector_call(on_expr, &selector_tokens.args, selector_tokens.is_empty);
    let selector_bindings = &selector_tokens.bindings;
    let selector_key_ident = fresh_ident(counter, "selector_key", "if");
    let mut arms = Vec::new();
    for ((key, node), shape) in route.routes.iter().zip(branch_shapes.iter()) {
        // Route branches are exclusive: only one branch runs, so inputs are
        // moved straight into that branch payload.
        let artifacts = required_artifacts(shape);
        let (mut branch_payload, branch_bindings) =
            prepare_move_payload(incoming, &artifacts, "route_in", counter);
        for artifact in required_borrowed(shape) {
            branch_payload.insert_borrowed(artifact);
        }

        let generated = get_node_expr(node, &branch_payload, counter, in_loop, async_mode);
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
    if route.is_if_chain {
        arms.push(quote! {
            _ => {
                panic!("`@if` selector produced an invalid branch index");
            }
        });
    }

    GeneratedExpr {
        tokens: quote! {
            #( #output_decl_tokens )*
            let #selector_key_ident = {
                #( #selector_bindings )*
                #selector_call
            };
            match #selector_key_ident {
                #( #arms, )*
            }
        },
        outputs,
    }
}

fn get_while_node_expr(
    while_expr: &WhileExpr,
    incoming: &Payload,
    counter: &mut usize,
    async_mode: bool,
) -> GeneratedExpr {
    let body_shape = analyze_expr(&while_expr.body);
    if !while_expr.outputs.is_empty() {
        validate_loop_outputs(&while_expr.outputs, &while_expr.output_borrows, &body_shape);
    }
    let (exit_outputs, exit_borrowed) =
        loop_exit_outputs(&while_expr.outputs, &while_expr.output_borrows, &body_shape);
    let (mut outputs, output_decl_tokens) =
        prepare_output_slots(&exit_outputs, "while_out", counter);
    outputs.borrowed = exit_borrowed;

    let cond_params = selector_params_for_on_expr(&while_expr.condition);
    let cond_bindings = build_condition_bindings(&cond_params, incoming, counter);
    let cond_bindings_tokens = &cond_bindings.bindings;
    let cond_call =
        build_condition_call(&while_expr.condition, &cond_bindings.args, cond_bindings.is_empty);

    let required_owned = required_artifacts(&body_shape);
    let required_borrowed = required_borrowed(&body_shape);

    let mut loop_payload_inits = Vec::new();
    let mut loop_payload = Payload::new();
    loop_payload.borrowed = incoming.borrowed.clone();

    for artifact in &required_owned {
        let source = incoming
            .get_owned(artifact)
            .unwrap_or_else(|| panic!("missing artifact `{artifact}` for @while body"));
        let stored = fresh_ident(counter, "while_seed", artifact);
        loop_payload_inits.push(quote! {
            let #stored = #source
                .as_ref()
                .unwrap_or_else(|| panic!(concat!("missing artifact `", #artifact, "`")));
        });
        loop_payload.insert_owned(artifact.clone(), stored.clone());
    }

    let mut iter_payload_bindings = Vec::new();
    let mut iter_payload = Payload::new();
    for artifact in &required_owned {
        let stored = loop_payload
            .get_owned(artifact)
            .unwrap_or_else(|| panic!("missing artifact `{artifact}` for @while body"));
        let iter_var = fresh_ident(counter, "while_in", artifact);
        iter_payload_bindings.push(quote! {
            let mut #iter_var = ::std::option::Option::Some(::graphium::clone_artifact(#stored));
        });
        iter_payload.insert_owned(artifact.clone(), iter_var.clone());
    }
    for artifact in &required_borrowed {
        iter_payload.insert_borrowed(artifact.clone());
    }

    let body_generated = get_node_expr(&while_expr.body, &iter_payload, counter, true, async_mode);
    let body_tokens = body_generated.tokens;
    let output_assigns = assign_outputs_to_slots(&body_generated.outputs, &outputs);

    GeneratedExpr {
        tokens: quote! {
            #( #output_decl_tokens )*
            #( #loop_payload_inits )*
            while {
                #( #cond_bindings_tokens )*
                #cond_call
            } {
                #( #iter_payload_bindings )*
                #body_tokens
                #( #output_assigns )*
            }
        },
        outputs,
    }
}

fn get_loop_node_expr(
    loop_expr: &LoopExpr,
    incoming: &Payload,
    counter: &mut usize,
    async_mode: bool,
) -> GeneratedExpr {
    let body_shape = analyze_expr(&loop_expr.body);
    if !loop_expr.outputs.is_empty() {
        validate_loop_outputs(&loop_expr.outputs, &loop_expr.output_borrows, &body_shape);
    }
    let (exit_outputs, exit_borrowed) =
        loop_exit_outputs(&loop_expr.outputs, &loop_expr.output_borrows, &body_shape);
    let (mut outputs, output_decl_tokens) =
        prepare_output_slots(&exit_outputs, "loop_out", counter);
    outputs.borrowed = exit_borrowed;

    let required_owned = required_artifacts(&body_shape);
    let required_borrowed = required_borrowed(&body_shape);

    let mut loop_payload_inits = Vec::new();
    let mut loop_payload = Payload::new();
    loop_payload.borrowed = incoming.borrowed.clone();

    for artifact in &required_owned {
        let source = incoming
            .get_owned(artifact)
            .unwrap_or_else(|| panic!("missing artifact `{artifact}` for @loop body"));
        let stored = fresh_ident(counter, "loop_seed", artifact);
        loop_payload_inits.push(quote! {
            let #stored = #source
                .as_ref()
                .unwrap_or_else(|| panic!(concat!("missing artifact `", #artifact, "`")));
        });
        loop_payload.insert_owned(artifact.clone(), stored.clone());
    }

    let mut iter_payload_bindings = Vec::new();
    let mut iter_payload = Payload::new();
    for artifact in &required_owned {
        let stored = loop_payload
            .get_owned(artifact)
            .unwrap_or_else(|| panic!("missing artifact `{artifact}` for @loop body"));
        let iter_var = fresh_ident(counter, "loop_in", artifact);
        iter_payload_bindings.push(quote! {
            let mut #iter_var = ::std::option::Option::Some(::graphium::clone_artifact(#stored));
        });
        iter_payload.insert_owned(artifact.clone(), iter_var.clone());
    }
    for artifact in &required_borrowed {
        iter_payload.insert_borrowed(artifact.clone());
    }

    let body_generated = get_node_expr(&loop_expr.body, &iter_payload, counter, true, async_mode);
    let body_tokens = body_generated.tokens;
    let output_assigns = assign_outputs_to_slots(&body_generated.outputs, &outputs);

    GeneratedExpr {
        tokens: quote! {
            #( #output_decl_tokens )*
            #( #loop_payload_inits )*
            loop {
                #( #iter_payload_bindings )*
                #body_tokens
                #( #output_assigns )*
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
        .owned
        .keys()
        .map(|artifact| {
            let outer_var = fresh_ident(counter, "captured", artifact);
            (artifact.clone(), outer_var)
        })
        .collect();

    for (artifact, outer_var) in &declaration_pairs {
        outer_outputs.insert_owned(artifact.clone(), outer_var.clone());
    }

    let declarations = declaration_pairs.iter().map(|(_, outer_var)| {
        quote! {
            let mut #outer_var = ::std::option::Option::None;
        }
    });

    outer_outputs.borrowed = inner_outputs.borrowed.clone();

    let assignments = inner_outputs.owned.iter().map(|(artifact, inner)| {
        let outer = outer_outputs
            .get_owned(artifact)
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
                entry_borrowed: analyze_expr(first).entry_borrowed,
                exit_outputs: analyze_expr(last).exit_outputs,
                exit_borrowed: analyze_expr(last).exit_borrowed,
            }
        }
        NodeExpr::Parallel(nodes) => {
            let shapes: Vec<ExprShape> = nodes.iter().map(analyze_expr).collect();

            ExprShape {
                entry_usage: collect_parallel_entry_usage(&shapes),
                entry_borrowed: collect_parallel_entry_borrowed(&shapes),
                exit_outputs: collect_parallel_outputs(&shapes),
                exit_borrowed: collect_parallel_borrowed(&shapes),
            }
        }
        NodeExpr::Route(route) => {
            let shapes: Vec<ExprShape> = route
                .routes
                .iter()
                .map(|(_, node)| analyze_expr(node))
                .collect();
            let mut entry_usage = UsageMap::new();
            let mut entry_borrowed = BTreeSet::new();
            let selector_params = selector_params_for_on_expr(&route.on);

            // A route only executes one branch, so from the caller's point of
            // view an artifact is required at most once at route entry.
            for shape in &shapes {
                for artifact in required_artifacts(shape) {
                    entry_usage.entry(artifact).or_insert(1);
                }
                for artifact in required_borrowed(shape) {
                    entry_borrowed.insert(artifact);
                }
            }

            for param in selector_params {
                if let SelectorParam::Artifact { ident, borrowed } = param {
                    if borrowed {
                        entry_borrowed.insert(ident.to_string());
                    } else {
                        entry_usage.entry(ident.to_string()).or_insert(1);
                    }
                }
            }

            let (exit_outputs, exit_borrowed) = route_exit_outputs(route, &shapes);

            ExprShape {
                entry_usage,
                entry_borrowed,
                exit_outputs,
                exit_borrowed,
            }
        }
        NodeExpr::While(while_expr) => {
            let body_shape = analyze_expr(&while_expr.body);
            let mut entry_usage = body_shape.entry_usage.clone();
            let mut entry_borrowed = body_shape.entry_borrowed.clone();
            let selector_params = selector_params_for_on_expr(&while_expr.condition);
            for param in selector_params {
                if let SelectorParam::Artifact { ident, borrowed } = param {
                    if borrowed {
                        entry_borrowed.insert(ident.to_string());
                    } else {
                        entry_usage.entry(ident.to_string()).or_insert(1);
                    }
                }
            }

            let (exit_outputs, exit_borrowed) =
                loop_exit_outputs(&while_expr.outputs, &while_expr.output_borrows, &body_shape);

            ExprShape {
                entry_usage,
                entry_borrowed,
                exit_outputs,
                exit_borrowed,
            }
        }
        NodeExpr::Loop(loop_expr) => {
            let body_shape = analyze_expr(&loop_expr.body);
            let (exit_outputs, exit_borrowed) =
                loop_exit_outputs(&loop_expr.outputs, &loop_expr.output_borrows, &body_shape);
            ExprShape {
                entry_usage: body_shape.entry_usage,
                entry_borrowed: body_shape.entry_borrowed,
                exit_outputs,
                exit_borrowed,
            }
        }
        NodeExpr::Break => ExprShape {
            entry_usage: UsageMap::new(),
            entry_borrowed: BTreeSet::new(),
            exit_outputs: Vec::new(),
            exit_borrowed: BTreeSet::new(),
        },
    }
}

/// Computes the shape of a single node call: which artifacts must already be
/// available and which artifact names can leave the node.
fn analyze_single(call: &NodeCall) -> ExprShape {
    if !call.explicit_inputs && call.inputs.is_empty() && call.outputs.is_empty() {
        return ExprShape {
            entry_usage: UsageMap::new(),
            entry_borrowed: BTreeSet::new(),
            exit_outputs: Vec::new(),
            exit_borrowed: BTreeSet::new(),
        };
    }

    let mut entry_usage = UsageMap::new();
    let mut entry_borrowed = BTreeSet::new();
    for (input, is_borrowed) in call.inputs.iter().zip(call.input_borrows.iter()) {
        if *is_borrowed {
            entry_borrowed.insert(input.to_string());
        } else {
            *entry_usage.entry(input.to_string()).or_insert(0) += 1;
        }
    }

    ExprShape {
        entry_usage,
        entry_borrowed,
        exit_outputs: call
            .outputs
            .iter()
            .zip(call.output_borrows.iter())
            .filter_map(|(output, is_borrowed)| {
                if *is_borrowed {
                    None
                } else {
                    Some(output.to_string())
                }
            })
            .collect(),
        exit_borrowed: call
            .outputs
            .iter()
            .zip(call.output_borrows.iter())
            .filter_map(|(output, is_borrowed)| {
                if *is_borrowed {
                    Some(output.to_string())
                } else {
                    None
                }
            })
            .collect(),
    }
}

/// Returns the ordered list of artifact names required at the entry of a graph
/// subexpression.
fn required_artifacts(shape: &ExprShape) -> Vec<String> {
    shape.entry_usage.keys().cloned().collect()
}

fn required_borrowed(shape: &ExprShape) -> Vec<String> {
    shape.entry_borrowed.iter().cloned().collect()
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
        for artifact in &shape.exit_borrowed {
            if !seen.insert(artifact.clone()) {
                panic!("parallel step produces duplicate artifact `{artifact}`");
            }
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
        for artifact in &shape.exit_borrowed {
            seen.insert(artifact.clone());
        }
    }

    outputs
}

fn collect_parallel_entry_borrowed(shapes: &[ExprShape]) -> BTreeSet<String> {
    let mut borrowed = BTreeSet::new();
    for shape in shapes {
        for artifact in required_borrowed(shape) {
            borrowed.insert(artifact);
        }
    }
    borrowed
}

fn collect_parallel_borrowed(shapes: &[ExprShape]) -> BTreeSet<String> {
    let mut borrowed = BTreeSet::new();
    for shape in shapes {
        for artifact in &shape.exit_borrowed {
            borrowed.insert(artifact.clone());
        }
    }
    borrowed
}

fn collect_route_borrowed(shapes: &[ExprShape]) -> BTreeSet<String> {
    let mut borrowed = BTreeSet::new();
    for shape in shapes {
        for artifact in &shape.exit_borrowed {
            borrowed.insert(artifact.clone());
        }
    }
    borrowed
}

struct ConditionBindings {
    bindings: Vec<proc_macro2::TokenStream>,
    args: Vec<proc_macro2::TokenStream>,
    is_empty: bool,
}

fn build_condition_bindings(
    params: &[SelectorParam],
    incoming: &Payload,
    counter: &mut usize,
) -> ConditionBindings {
    let mut bindings = Vec::new();
    let mut args = Vec::new();
    let mut has_borrowed = false;
    let mut wants_mut_ctx = false;

    for param in params {
        match param {
            SelectorParam::Ctx { mutable } => {
                if *mutable {
                    wants_mut_ctx = true;
                    args.push(quote! { ctx });
                } else {
                    args.push(quote! { &*ctx });
                }
            }
            SelectorParam::Artifact { ident, borrowed } => {
                let artifact_name = ident.to_string();
                if *borrowed {
                    has_borrowed = true;
                    if incoming.has_borrowed(&artifact_name) {
                        args.push(quote! { &ctx.#ident });
                    } else {
                        let source = incoming
                            .get_owned(&artifact_name)
                            .unwrap_or_else(|| panic!("missing artifact `{artifact_name}` for @while condition"));
                        let arg_ident = fresh_ident(counter, "cond_borrow", &artifact_name);
                        bindings.push(quote! {
                            let #arg_ident = #source
                                .as_ref()
                                .unwrap_or_else(|| panic!(concat!("missing artifact `", #artifact_name, "`")));
                        });
                        args.push(quote! { #arg_ident });
                    }
                } else {
                    let source = incoming
                        .get_owned(&artifact_name)
                        .unwrap_or_else(|| panic!("missing artifact `{artifact_name}` for @while condition"));
                    let arg_ident = fresh_ident(counter, "cond_arg", &artifact_name);
                    bindings.push(quote! {
                        let #arg_ident = ::graphium::clone_artifact(
                            #source
                                .as_ref()
                                .unwrap_or_else(|| panic!(concat!("missing artifact `", #artifact_name, "`")))
                        );
                    });
                    args.push(quote! { #arg_ident });
                }
            }
        }
    }

    if has_borrowed && wants_mut_ctx {
        panic!("@while condition cannot take `&mut ctx` and borrowed artifacts at the same time");
    }

    ConditionBindings {
        bindings,
        args,
        is_empty: params.is_empty(),
    }
}

fn build_condition_call(
    condition: &syn::Expr,
    args: &[proc_macro2::TokenStream],
    is_empty: bool,
) -> proc_macro2::TokenStream {
    if let syn::Expr::Closure(closure) = condition {
        if closure.inputs.is_empty() {
            return quote! { (#condition)() };
        }
        return quote! { (#condition)(#( #args ),*) };
    }

    if is_empty {
        quote! { #condition }
    } else {
        if let syn::Expr::Path(path) = condition {
            if path.qself.is_none() && path.path.segments.len() == 1 {
                return quote! { #(#args)* };
            }
        }
        quote! { (#condition)(#( #args ),*) }
    }
}

fn loop_exit_outputs(
    outputs: &[syn::Ident],
    output_borrows: &[bool],
    body_shape: &ExprShape,
) -> (Vec<String>, BTreeSet<String>) {
    if outputs.is_empty() {
        return (
            body_shape.exit_outputs.clone(),
            body_shape.exit_borrowed.clone(),
        );
    }

    let mut owned = Vec::new();
    let mut borrowed = BTreeSet::new();
    for (output, is_borrowed) in outputs.iter().zip(output_borrows.iter()) {
        if *is_borrowed {
            borrowed.insert(output.to_string());
        } else {
            owned.push(output.to_string());
        }
    }

    (owned, borrowed)
}

fn validate_loop_outputs(
    outputs: &[syn::Ident],
    output_borrows: &[bool],
    body_shape: &ExprShape,
) {
    let mut expected_owned = BTreeSet::new();
    let mut expected_borrowed = BTreeSet::new();
    for (output, is_borrowed) in outputs.iter().zip(output_borrows.iter()) {
        if *is_borrowed {
            expected_borrowed.insert(output.to_string());
        } else {
            expected_owned.insert(output.to_string());
        }
    }

    for artifact in &body_shape.exit_outputs {
        if !expected_owned.contains(artifact) {
            panic!("loop body produces unexpected artifact `{artifact}`");
        }
    }
    for artifact in &body_shape.exit_borrowed {
        if !expected_borrowed.contains(artifact) {
            panic!("loop body produces unexpected borrowed artifact `{artifact}`");
        }
    }
    for artifact in &expected_owned {
        if !body_shape.exit_outputs.contains(artifact) {
            panic!("loop body missing required artifact `{artifact}`");
        }
    }
    for artifact in &expected_borrowed {
        if !body_shape.exit_borrowed.contains(artifact) {
            panic!("loop body missing required borrowed artifact `{artifact}`");
        }
    }
}
fn route_exit_outputs(
    route: &RouteExpr,
    shapes: &[ExprShape],
) -> (Vec<String>, BTreeSet<String>) {
    if route.outputs.is_empty() {
        return (
            collect_route_outputs(shapes),
            collect_route_borrowed(shapes),
        );
    }

    let mut owned = Vec::new();
    let mut borrowed = BTreeSet::new();
    for (output, is_borrowed) in route
        .outputs
        .iter()
        .zip(route.output_borrows.iter())
    {
        if *is_borrowed {
            borrowed.insert(output.to_string());
        } else {
            owned.push(output.to_string());
        }
    }

    (owned, borrowed)
}

fn validate_route_outputs(route: &RouteExpr, shapes: &[ExprShape]) {
    let mut expected_owned = BTreeSet::new();
    let mut expected_borrowed = BTreeSet::new();
    for (output, is_borrowed) in route
        .outputs
        .iter()
        .zip(route.output_borrows.iter())
    {
        if *is_borrowed {
            expected_borrowed.insert(output.to_string());
        } else {
            expected_owned.insert(output.to_string());
        }
    }

    for shape in shapes {
        for artifact in &shape.exit_outputs {
            if !expected_owned.contains(artifact) {
                panic!("route branch produces unexpected artifact `{artifact}`");
            }
        }
        for artifact in &shape.exit_borrowed {
            if !expected_borrowed.contains(artifact) {
                panic!("route branch produces unexpected borrowed artifact `{artifact}`");
            }
        }
        for artifact in &expected_owned {
            if !shape.exit_outputs.contains(artifact) {
                panic!("route branch missing required artifact `{artifact}`");
            }
        }
        for artifact in &expected_borrowed {
            if !shape.exit_borrowed.contains(artifact) {
                panic!("route branch missing required borrowed artifact `{artifact}`");
            }
        }
    }
}

struct SelectorBindings {
    bindings: Vec<proc_macro2::TokenStream>,
    args: Vec<proc_macro2::TokenStream>,
    is_empty: bool,
}

fn parse_selector_params(on: &syn::Expr) -> Vec<SelectorParam> {
    let syn::Expr::Closure(closure) = on else {
        return Vec::new();
    };

    let mut params = Vec::new();
    for input in &closure.inputs {
        match input {
            syn::Pat::Type(pat_type) => {
                let syn::Pat::Ident(pat_ident) = &*pat_type.pat else {
                    panic!("selector parameters must be identifiers");
                };
                let name = pat_ident.ident.to_string();
                if name == "ctx" || name == "_ctx" {
                    let mutable = matches!(&*pat_type.ty, syn::Type::Reference(r) if r.mutability.is_some());
                    params.push(SelectorParam::Ctx { mutable });
                } else {
                    let borrowed = matches!(&*pat_type.ty, syn::Type::Reference(_));
                    params.push(SelectorParam::Artifact {
                        ident: pat_ident.ident.clone(),
                        borrowed,
                    });
                }
            }
            syn::Pat::Ident(pat_ident) => {
                let name = pat_ident.ident.to_string();
                if name == "ctx" || name == "_ctx" {
                    params.push(SelectorParam::Ctx { mutable: false });
                } else {
                    params.push(SelectorParam::Artifact {
                        ident: pat_ident.ident.clone(),
                        borrowed: false,
                    });
                }
            }
            _ => panic!("selector parameters must be identifiers"),
        }
    }

    params
}

fn build_selector_bindings(
    params: &[SelectorParam],
    incoming: &Payload,
    needed_by_branches: &BTreeSet<String>,
    counter: &mut usize,
) -> SelectorBindings {
    let mut bindings = Vec::new();
    let mut args = Vec::new();
    let mut has_borrowed = false;
    let mut wants_mut_ctx = false;

    for param in params {
        match param {
            SelectorParam::Ctx { mutable } => {
                if *mutable {
                    wants_mut_ctx = true;
                }
                if *mutable {
                    args.push(quote! { ctx });
                } else {
                    args.push(quote! { &*ctx });
                }
            }
            SelectorParam::Artifact { ident, borrowed } => {
                let artifact_name = ident.to_string();
                if *borrowed {
                    if incoming.has_borrowed(&artifact_name) {
                        has_borrowed = true;
                        args.push(quote! { &ctx.#ident });
                    } else {
                        let source = incoming
                            .get_owned(&artifact_name)
                            .unwrap_or_else(|| panic!("missing artifact `{artifact_name}` for @match selector"));
                        let arg_ident = fresh_ident(counter, "selector_borrow", &artifact_name);
                        bindings.push(quote! {
                            let #arg_ident = #source
                                .as_ref()
                                .unwrap_or_else(|| panic!(concat!("missing artifact `", #artifact_name, "`")));
                        });
                        args.push(quote! { #arg_ident });
                    }
                } else {
                    let source = incoming
                        .get_owned(&artifact_name)
                        .unwrap_or_else(|| panic!("missing artifact `{artifact_name}` for @match selector"));
                    let arg_ident = fresh_ident(counter, "selector_arg", &artifact_name);
                    if needed_by_branches.contains(&artifact_name) {
                        // If already marked borrowed, clone to avoid moving shared value.
                        bindings.push(quote! {
                            let #arg_ident = ::graphium::clone_artifact(
                                #source
                                    .as_ref()
                                    .unwrap_or_else(|| panic!(concat!("missing artifact `", #artifact_name, "`")))
                            );
                        });
                    } else {
                        // If no branch needs this artifact, allow moving it into selector.
                        bindings.push(quote! {
                            let #arg_ident = #source
                                .take()
                                .unwrap_or_else(|| panic!(concat!("missing artifact `", #artifact_name, "`")));
                        });
                    }
                    args.push(quote! { #arg_ident });
                }
            }
        }
    }

    if has_borrowed && wants_mut_ctx {
        panic!("@match selector cannot take `&mut ctx` and borrowed artifacts at the same time");
    }

    let is_empty = params.is_empty();
    SelectorBindings {
        bindings,
        args,
        is_empty,
    }
}

fn build_selector_call(
    on_expr: &syn::Expr,
    args: &[proc_macro2::TokenStream],
    is_empty: bool,
) -> proc_macro2::TokenStream {
    if let syn::Expr::Closure(closure) = on_expr {
        if closure.inputs.is_empty() {
            return quote! { (#on_expr)() };
        }
        return quote! { (#on_expr)(#( #args ),*) };
    }

    if !args.is_empty() {
        if let syn::Expr::Path(path) = on_expr {
            if path.qself.is_none() && path.path.segments.len() == 1 {
                return quote! { #(#args)* };
            }
        }
    }

    if is_empty {
        quote! { #on_expr }
    } else {
        quote! { (#on_expr)(#( #args ),*) }
    }
}

fn graph_definition_tokens(name: &syn::Ident, nodes: &NodeExpr) -> proc_macro2::TokenStream {
    let steps = node_expr_steps_tokens(nodes);
    quote! {
        ::graphium::GraphDef {
            name: stringify!(#name),
            steps: vec![ #( #steps ),* ],
        }
    }
}

fn node_expr_steps_tokens(node: &NodeExpr) -> Vec<proc_macro2::TokenStream> {
    match node {
        NodeExpr::Single(call) => vec![node_call_step_tokens(call)],
        NodeExpr::Sequence(nodes) => nodes
            .iter()
            .flat_map(|child| node_expr_steps_tokens(child))
            .collect(),
        NodeExpr::Parallel(nodes) => {
            let branches: Vec<_> = nodes
                .iter()
                .map(|child| {
                    let steps = node_expr_steps_tokens(child);
                    quote! { vec![ #( #steps ),* ] }
                })
                .collect();
            vec![quote! {
                ::graphium::GraphStep::Parallel {
                    branches: vec![ #( #branches ),* ],
                }
            }]
        }
        NodeExpr::Route(route) => {
            let on = &route.on;
            let cases: Vec<_> = route
                .routes
                .iter()
                .map(|(key, node)| {
                    let steps = node_expr_steps_tokens(node);
                    quote! {
                        ::graphium::GraphCase {
                            label: stringify!(#key),
                            steps: vec![ #( #steps ),* ],
                        }
                    }
                })
                .collect();
            vec![quote! {
                ::graphium::GraphStep::Route {
                    on: stringify!(#on),
                    cases: vec![ #( #cases ),* ],
                }
            }]
        }
        NodeExpr::While(while_expr) => {
            let condition = &while_expr.condition;
            let body_steps = node_expr_steps_tokens(&while_expr.body);
            vec![quote! {
                ::graphium::GraphStep::While {
                    condition: stringify!(#condition),
                    body: vec![ #( #body_steps ),* ],
                }
            }]
        }
        NodeExpr::Loop(loop_expr) => {
            let body_steps = node_expr_steps_tokens(&loop_expr.body);
            vec![quote! {
                ::graphium::GraphStep::Loop {
                    body: vec![ #( #body_steps ),* ],
                }
            }]
        }
        NodeExpr::Break => vec![quote! { ::graphium::GraphStep::Break }],
    }
}

fn node_call_step_tokens(call: &NodeCall) -> proc_macro2::TokenStream {
    let node_path = &call.path;
    let nested_graph_path = is_graph_run_path(node_path).then(|| graph_type_path(node_path));
    let input_tokens = artifact_list_tokens(&call.inputs, &call.input_borrows);
    let output_tokens = artifact_list_tokens(&call.outputs, &call.output_borrows);
    if let Some(graph_path) = nested_graph_path {
        quote! {
            ::graphium::GraphStep::Nested {
                graph: Box::new(<#graph_path as ::graphium::GraphDefProvider>::graph_def()),
                inputs: vec![ #( #input_tokens ),* ],
                outputs: vec![ #( #output_tokens ),* ],
            }
        }
    } else {
        quote! {
            ::graphium::GraphStep::Node {
                name: stringify!(#node_path),
                inputs: vec![ #( #input_tokens ),* ],
                outputs: vec![ #( #output_tokens ),* ],
            }
        }
    }
}

fn artifact_list_tokens(
    idents: &[syn::Ident],
    borrows: &[bool],
) -> Vec<proc_macro2::TokenStream> {
    idents
        .iter()
        .zip(borrows.iter())
        .map(|(ident, borrowed)| {
            if *borrowed {
                quote! { concat!("&", stringify!(#ident)) }
            } else {
                quote! { stringify!(#ident) }
            }
        })
        .collect()
}
