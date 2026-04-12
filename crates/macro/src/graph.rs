use proc_macro::TokenStream;
use quote::quote;
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

pub fn expand(input: TokenStream) -> TokenStream {
    let GraphInput {
        name,
        context,
        nodes,
    } = parse_macro_input!(input as GraphInput);

    let mut counter = 0usize;
    let generated = generate_expr(&nodes, &Payload::new(), &mut counter);
    let body = if generated.outputs.is_empty() {
        generated.tokens
    } else {
        let generated_tokens = generated.tokens;
        // The root graph cannot expose artifacts to its caller, so any final
        // payload should die at the end of `run`.
        quote! {{
            #generated_tokens
        }}
    };

    let expanded = quote! {
        pub struct #name;

        impl #name {
            pub fn run(ctx: &mut #context) {
                #body
            }
        }
    };

    TokenStream::from(expanded)
}

fn generate_expr(node: &NodeExpr, incoming: &Payload, counter: &mut usize) -> GeneratedExpr {
    match node {
        NodeExpr::Single(call) => generate_single(call, incoming, counter),
        NodeExpr::Sequence(nodes) => generate_sequence(nodes, incoming, counter),
        NodeExpr::Parallel(nodes) => generate_parallel(nodes, incoming, counter),
        NodeExpr::Route(route) => generate_route(route, incoming, counter),
    }
}

fn generate_single(call: &NodeCall, incoming: &Payload, counter: &mut usize) -> GeneratedExpr {
    let path = &call.path;

    if is_graph_run_path(path) {
        if !call.inputs.is_empty() || !call.outputs.is_empty() {
            panic!("graph `run` calls do not support explicit inputs or outputs");
        }

        return GeneratedExpr {
            tokens: quote! {
                #path(ctx);
            },
            outputs: Payload::new(),
        };
    }

    // Inside a single node call, the same artifact name might appear more than
    // once. We count those local reads so the last one can move and earlier
    // ones clone from the same incoming payload.
    let mut remaining = UsageMap::new();
    for input in &call.inputs {
        *remaining.entry(input.to_string()).or_insert(0) += 1;
    }

    let mut arg_vars = Vec::with_capacity(call.inputs.len());
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

    if call.outputs.is_empty() {
        return GeneratedExpr {
            tokens: quote! {
                #( #input_bindings )*
                #path::__graphio_run(ctx, #( #arg_vars ),*);
            },
            outputs: Payload::new(),
        };
    }

    // A node with outputs becomes the producer for the next hop.
    // We hold each produced artifact in an `Option<T>` so later code can decide
    // whether to move it (`take`) or clone from it.
    let mut outputs = Payload::new();
    if call.outputs.len() == 1 {
        let artifact_name = call.outputs[0].to_string();
        let output_var = fresh_ident(counter, "hop", &artifact_name);
        outputs.insert(artifact_name, output_var.clone());

        GeneratedExpr {
            tokens: quote! {
                #( #input_bindings )*
                let mut #output_var = ::std::option::Option::Some(#path::__graphio_run(ctx, #( #arg_vars ),*));
            },
            outputs,
        }
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

        GeneratedExpr {
            tokens: quote! {
                #( #input_bindings )*
                let ( #( #tuple_vars ),* ) = #path::__graphio_run(ctx, #( #arg_vars ),*);
                #( #output_stores )*
            },
            outputs,
        }
    }
}

fn generate_sequence(nodes: &[NodeExpr], incoming: &Payload, counter: &mut usize) -> GeneratedExpr {
    let mut iter = nodes.iter();
    let first = iter
        .next()
        .expect("sequence must contain at least one node");
    let mut current = generate_expr(first, incoming, counter);

    for next in iter {
        let shape = analyze_expr(next);
        let required = required_artifacts(&shape);
        let mut next_payload = Payload::new();
        let mut transfer_tokens = Vec::with_capacity(required.len());

        // A sequence boundary is the core one-hop rule:
        // take only the artifacts the next step needs, build a fresh payload for
        // that step, and let everything else drop here.
        for artifact in required {
            let source = current
                .outputs
                .get(&artifact)
                .unwrap_or_else(|| panic!("missing artifact `{artifact}` for next hop"));
            let payload_var = fresh_ident(counter, "payload", &artifact);
            next_payload.insert(artifact.clone(), payload_var.clone());
            transfer_tokens.push(quote! {
                let mut #payload_var = #source.take();
            });
        }

        let next_generated = generate_expr(next, &next_payload, counter);
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

fn generate_parallel(nodes: &[NodeExpr], incoming: &Payload, counter: &mut usize) -> GeneratedExpr {
    let shapes: Vec<ExprShape> = nodes.iter().map(analyze_expr).collect();
    let mut remaining = UsageMap::new();
    for shape in &shapes {
        for artifact in required_artifacts(shape) {
            *remaining.entry(artifact).or_insert(0) += 1;
        }
    }

    let exit_outputs = collect_parallel_outputs(&shapes);
    let mut outputs = Payload::new();
    let mut output_decl_tokens = Vec::new();
    for artifact in &exit_outputs {
        let output_var = fresh_ident(counter, "parallel_out", artifact);
        outputs.insert(artifact.clone(), output_var.clone());
        output_decl_tokens.push(quote! {
            let mut #output_var = ::std::option::Option::None;
        });
    }

    let mut blocks = Vec::new();
    for (node, shape) in nodes.iter().zip(shapes.iter()) {
        let mut child_payload = Payload::new();
        let mut child_bindings = Vec::new();

        // A parallel hop distributes the same incoming payload to multiple
        // sibling steps. Early consumers clone if another sibling still needs
        // the artifact; the last sibling moves it.
        for artifact in required_artifacts(shape) {
            let source = incoming
                .get(&artifact)
                .unwrap_or_else(|| panic!("missing artifact `{artifact}` for parallel step"));
            let remaining_children = remaining
                .get_mut(&artifact)
                .unwrap_or_else(|| panic!("missing usage count for `{artifact}`"));
            let payload_var = fresh_ident(counter, "parallel_in", &artifact);
            child_payload.insert(artifact.clone(), payload_var.clone());

            if *remaining_children == 1 {
                child_bindings.push(quote! {
                    let mut #payload_var = #source.take();
                });
            } else {
                child_bindings.push(quote! {
                    let mut #payload_var = ::std::option::Option::Some(::graphio::clone_artifact(
                        #source
                            .as_ref()
                            .unwrap_or_else(|| panic!(concat!("missing artifact `", #artifact, "`")))
                    ));
                });
            }

            *remaining_children -= 1;
        }

        let generated = generate_expr(node, &child_payload, counter);
        let generated_tokens = generated.tokens;
        let output_assigns = generated.outputs.iter().map(|(artifact, inner)| {
            let outer = outputs
                .get(artifact)
                .unwrap_or_else(|| panic!("missing parallel output slot for `{artifact}`"));
            quote! {
                #outer = #inner;
            }
        });

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

fn generate_route(route: &RouteExpr, incoming: &Payload, counter: &mut usize) -> GeneratedExpr {
    let branch_shapes: Vec<ExprShape> = route
        .routes
        .iter()
        .map(|(_, node)| analyze_expr(node))
        .collect();
    let exit_outputs = collect_route_outputs(&branch_shapes);

    let mut outputs = Payload::new();
    let mut output_decl_tokens = Vec::new();
    for artifact in &exit_outputs {
        let output_var = fresh_ident(counter, "route_out", artifact);
        outputs.insert(artifact.clone(), output_var.clone());
        output_decl_tokens.push(quote! {
            let mut #output_var = ::std::option::Option::None;
        });
    }

    let on_expr = &route.on;
    let mut arms = Vec::new();
    for ((key, node), shape) in route.routes.iter().zip(branch_shapes.iter()) {
        let mut branch_payload = Payload::new();
        let mut branch_bindings = Vec::new();

        // Route branches are exclusive: only one branch runs, so inputs are
        // moved straight into that branch payload.
        for artifact in required_artifacts(shape) {
            let source = incoming
                .get(&artifact)
                .unwrap_or_else(|| panic!("missing artifact `{artifact}` for route branch"));
            let payload_var = fresh_ident(counter, "route_in", &artifact);
            branch_payload.insert(artifact, payload_var.clone());
            branch_bindings.push(quote! {
                let mut #payload_var = #source.take();
            });
        }

        let generated = generate_expr(node, &branch_payload, counter);
        let generated_tokens = generated.tokens;
        let output_assigns = generated.outputs.iter().map(|(artifact, inner)| {
            let outer = outputs
                .get(artifact)
                .unwrap_or_else(|| panic!("missing route output slot for `{artifact}`"));
            quote! {
                #outer = #inner;
            }
        });

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
            let mut entry_usage = UsageMap::new();

            for shape in &shapes {
                for artifact in required_artifacts(shape) {
                    *entry_usage.entry(artifact).or_insert(0) += 1;
                }
            }

            ExprShape {
                entry_usage,
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

fn analyze_single(call: &NodeCall) -> ExprShape {
    if is_graph_run_path(&call.path) {
        if !call.inputs.is_empty() || !call.outputs.is_empty() {
            panic!("graph `run` calls do not support explicit inputs or outputs");
        }

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

fn required_artifacts(shape: &ExprShape) -> Vec<String> {
    shape.entry_usage.keys().cloned().collect()
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
