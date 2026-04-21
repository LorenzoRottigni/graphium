//! Expansion for a single node invocation.
//!
//! This is the leaf code generator for the graph IR. It is responsible for
//! binding node inputs, choosing move vs clone semantics, and packaging the
//! produced artifacts back into a hop payload.

use std::collections::{BTreeMap, BTreeSet};

use quote::quote;

use crate::shared::{
    fresh_ident, is_graph_run_path, ArtifactInputKind, GeneratedExpr, NodeCall, Payload, UsageMap,
};

/// Builds the actual call expression for a node wrapper or nested graph entry
/// point, including the async `.await` when required.
///
/// Example:
/// providing `Worker`, args `[value]`, and sync mode expands into
/// `Worker::__graphium_run(ctx, value)`, while async nested-graph mode expands
/// into `OtherGraph::__graphium_run_async(ctx, value).await`.
pub(super) fn node_run_call_tokens(
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

/// Generates code for a single node invocation or nested graph execution call.
///
/// Example:
/// providing `Worker(input) -> output` expands into bindings that read `input`
/// from the incoming payload and a slot like
/// `let mut __graphium_hop_*_output = Some(Worker::__graphium_run(...));`.
pub(super) fn get_single_node_expr(
    call: &NodeCall,
    incoming: &Payload,
    counter: &mut usize,
    async_mode: bool,
) -> GeneratedExpr {
    let node_path = &call.path;
    let nested_graph_path = is_graph_run_path(node_path).then(|| graph_type_path(node_path));
    let has_borrowed_inputs = call
        .input_kinds
        .iter()
        .any(|kind| *kind == ArtifactInputKind::Borrowed);

    if nested_graph_path.is_some() && has_borrowed_inputs {
        panic!("borrowed artifacts are not supported when calling nested graphs");
    }

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

    let mut next_borrowed = incoming.borrowed.clone();

    let mut remaining = UsageMap::new();
    for (input, kind) in call.inputs.iter().zip(call.input_kinds.iter()) {
        if *kind != ArtifactInputKind::Owned {
            continue;
        }
        *remaining.entry(input.to_string()).or_insert(0) += 1;
    }

    let arg_idents: Vec<syn::Ident> = call
        .inputs
        .iter()
        .map(|input| fresh_ident(counter, "arg", &input.to_string()))
        .collect();

    let arg_vars = arg_idents.clone();
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

    // Bind taken inputs first so later `&ctx.field` borrows don't conflict with
    // `&mut ctx.other_field` takes.
    for ((input, kind), arg_ident) in call
        .inputs
        .iter()
        .zip(call.input_kinds.iter())
        .zip(arg_idents.iter())
    {
        if *kind != ArtifactInputKind::Taken {
            continue;
        }
        let artifact_name = input.to_string();
        if !incoming.has_borrowed(&artifact_name) {
            panic!("missing borrowed artifact `{artifact_name}` for node call");
        }
        next_borrowed.remove(&artifact_name);
        input_bindings.push(quote! {
            let #arg_ident = ::core::mem::take(&mut ctx.#input);
        });
        input_by_name
            .entry(artifact_name.clone())
            .or_insert((arg_ident.clone(), false));
    }

    // Then bind borrowed inputs.
    for ((input, kind), arg_ident) in call
        .inputs
        .iter()
        .zip(call.input_kinds.iter())
        .zip(arg_idents.iter())
    {
        if *kind != ArtifactInputKind::Borrowed {
            continue;
        }
        let artifact_name = input.to_string();
        if !incoming.has_borrowed(&artifact_name) {
            panic!("missing borrowed artifact `{artifact_name}` for node call");
        }
        input_bindings.push(quote! {
            let #arg_ident = &ctx.#input;
        });
        input_by_name
            .entry(artifact_name.clone())
            .or_insert((arg_ident.clone(), true));
    }

    // Finally bind owned inputs from the hop payload.
    for ((input, kind), arg_ident) in call
        .inputs
        .iter()
        .zip(call.input_kinds.iter())
        .zip(arg_idents.iter())
    {
        if *kind != ArtifactInputKind::Owned {
            continue;
        }
        let artifact_name = input.to_string();
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
    }

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

        let mut outputs = Payload::new();
        outputs.borrowed = next_borrowed.clone();
        return GeneratedExpr {
            tokens: quote! {
                #( #input_bindings )*
                #( #ctx_clone_bindings )*
                #run_call;
                #( #ctx_store_bindings )*
            },
            outputs,
        };
    }

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
                outputs: {
                    outputs.borrowed.extend(next_borrowed.iter().cloned());
                    outputs
                },
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
            outputs: {
                outputs.borrowed.extend(next_borrowed.iter().cloned());
                outputs
            },
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
            outputs: {
                outputs.borrowed.extend(next_borrowed.iter().cloned());
                outputs
            },
        }
    }
}

/// Extracts the graph type path from a `SomeGraph::run` path so nested graphs
/// can be executed through its generated entrypoint.
///
/// Example:
/// providing `demo::MyGraph::run` expands into the path `demo::MyGraph`.
pub(super) fn graph_type_path(path: &syn::Path) -> syn::Path {
    let mut graph_path = path.clone();
    graph_path.segments.pop();
    graph_path.segments.pop_punct();

    if graph_path.segments.is_empty() {
        panic!("invalid graph run path");
    }

    graph_path
}

#[cfg(test)]
mod tests {
    use quote::{quote, ToTokens};
    use syn::{parse_quote, Ident};

    use super::{get_single_node_expr, graph_type_path, node_run_call_tokens};
    use crate::shared::{NodeCall, Payload};

    #[test]
    fn graph_type_path_strips_run_segment() {
        let path: syn::Path = parse_quote!(demo::MyGraph::run);

        let graph_path = graph_type_path(&path);

        assert_eq!(graph_path.to_token_stream().to_string(), "demo :: MyGraph");
    }

    #[test]
    fn node_run_call_tokens_awaits_async_nested_graphs() {
        let node_path: syn::Path = parse_quote!(demo::MyGraph::run);
        let graph_path: syn::Path = parse_quote!(demo::MyGraph);
        let args = vec![Ident::new("value", proc_macro2::Span::call_site())];

        let tokens = node_run_call_tokens(&node_path, Some(&graph_path), quote!(ctx), &args, true);

        assert!(tokens.to_string().contains("__graphium_run_async"));
        assert!(tokens.to_string().contains(". await"));
    }

    #[test]
    fn single_node_without_outputs_emits_plain_call() {
        let call = NodeCall {
            path: parse_quote!(demo::Worker),
            explicit_inputs: false,
            inputs: Vec::new(),
            input_kinds: Vec::new(),
            outputs: Vec::new(),
            output_borrows: Vec::new(),
        };

        let generated = get_single_node_expr(&call, &Payload::new(), &mut 0, false);

        assert!(generated
            .tokens
            .to_string()
            .contains("demo :: Worker :: __graphium_run"));
        assert!(generated.outputs.is_empty());
    }
}
