//! Sequence and parallel expansion.
//!
//! These helpers implement the hop-to-hop forwarding rules for `>>` and `|`
//! expressions.

use quote::quote;

use crate::shared::{ExprShape, GeneratedExpr, NodeExpr, Payload, UsageMap};

use super::{
    analyze_expr, assign_outputs_to_slots, capture_outputs, collect_parallel_borrowed,
    collect_parallel_outputs, contains_break, get_node_expr, prepare_move_payload,
    prepare_output_slots, prepare_parallel_payload, required_artifacts,
};

/// Counts how many sibling branches require each artifact at the entry of a
/// parallel expression.
///
/// Example:
/// providing branch shapes that both need `value` expands into
/// `{"value": 2}`.
pub(super) fn collect_parallel_entry_usage(shapes: &[ExprShape]) -> UsageMap {
    let mut remaining = UsageMap::new();

    for shape in shapes {
        for artifact in required_artifacts(shape) {
            *remaining.entry(artifact).or_insert(0) += 1;
        }
    }

    remaining
}

/// Generates code for `A >> B >> C` style execution.
///
/// Example:
/// providing `A >> B >> C` expands into sequential blocks that run `A`, move
/// only `B`'s required artifacts into the next hop, then do the same for `C`.
pub(super) fn get_sequence_nodes_expr(
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
        let (next_payload, transfer_tokens) =
            prepare_move_payload(&current.outputs, &required, "payload", counter);

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
///
/// Example:
/// providing `A | B` expands into a `std::thread::scope(...)` block that spawns
/// one branch per child and then joins their results into outer output slots.
pub(super) fn get_parallel_nodes_expr(
    nodes: &[NodeExpr],
    incoming: &Payload,
    counter: &mut usize,
    in_loop: bool,
    async_mode: bool,
) -> GeneratedExpr {
    if async_mode || (in_loop && nodes.iter().any(contains_break)) {
        return get_parallel_nodes_expr_sequential(nodes, incoming, counter, in_loop, async_mode);
    }

    let shapes: Vec<ExprShape> = nodes.iter().map(analyze_expr).collect();
    let uses_borrowed = !incoming.borrowed.is_empty()
        || shapes
            .iter()
            .any(|shape| !shape.entry_borrowed.is_empty() || !shape.exit_borrowed.is_empty());
    if uses_borrowed {
        return get_parallel_nodes_expr_sequential(nodes, incoming, counter, in_loop, async_mode);
    }
    let mut remaining = collect_parallel_entry_usage(&shapes);

    let exit_outputs = collect_parallel_outputs(&shapes);
    let exit_borrowed = collect_parallel_borrowed(&shapes);
    let (mut outputs, output_decl_tokens) =
        prepare_output_slots(&exit_outputs, "parallel_out", counter);
    outputs.borrowed = exit_borrowed;

    let mut spawn_tokens = Vec::new();
    let mut join_tokens = Vec::new();
    for (index, (node, shape)) in nodes.iter().zip(shapes.iter()).enumerate() {
        let (child_payload, child_bindings) =
            prepare_parallel_payload(incoming, shape, &mut remaining, counter);

        let generated = get_node_expr(node, &child_payload, counter, in_loop, async_mode);
        let generated_tokens = generated.tokens;
        let handle_ident =
            crate::shared::fresh_ident(counter, "parallel_handle", &index.to_string());
        let result_ident =
            crate::shared::fresh_ident(counter, "parallel_result", &index.to_string());

        let mut branch_output_idents = Vec::new();
        let mut output_assigns = Vec::new();
        for artifact in generated.outputs.owned.keys() {
            let inner = generated
                .outputs
                .get_owned(artifact)
                .unwrap_or_else(|| panic!("missing parallel branch output for `{artifact}`"));
            let outer = outputs
                .get_owned(artifact)
                .unwrap_or_else(|| panic!("missing parallel output slot for `{artifact}`"));
            let idx = syn::Index::from(branch_output_idents.len());
            output_assigns.push(quote! {
                #outer = #result_ident.#idx;
            });
            branch_output_idents.push(inner.clone());
        }
        let branch_return = if branch_output_idents.is_empty() {
            quote! { () }
        } else {
            quote! { ( #( #branch_output_idents ),*, ) }
        };

        spawn_tokens.push(quote! {
            let #handle_ident = {
                #( #child_bindings )*
                let __graphium_parallel_ctx = &*ctx;
                __graphium_scope.spawn(move || {
                    let ctx = __graphium_parallel_ctx;
                    #generated_tokens
                    #branch_return
                })
            };
        });

        join_tokens.push(quote! {
            let #result_ident = #handle_ident
                .join()
                .unwrap_or_else(|_| panic!("parallel branch panicked"));
            #( #output_assigns )*
        });
    }

    GeneratedExpr {
        tokens: quote! {
            #( #output_decl_tokens )*
            ::std::thread::scope(|__graphium_scope| {
                #( #spawn_tokens )*
                #( #join_tokens )*
            });
        },
        outputs: {
            // Threaded parallel blocks always use `&ctx` and reject borrowed
            // artifacts, so ctx-borrowed state can't change here.
            outputs.borrowed = incoming.borrowed.clone();
            outputs
        },
    }
}

/// Generates the fallback sequential form of a parallel expression.
///
/// Example:
/// providing `A | B` in async mode expands into plain sequential child blocks
/// instead of `std::thread::scope(...)`.
pub(super) fn get_parallel_nodes_expr_sequential(
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
    let mut current_borrowed = incoming.borrowed.clone();
    for (node, shape) in nodes.iter().zip(shapes.iter()) {
        let (child_payload, child_bindings) =
            prepare_parallel_payload(incoming, shape, &mut remaining, counter);
        let mut child_payload = child_payload;
        child_payload.borrowed = current_borrowed.clone();

        let generated = get_node_expr(node, &child_payload, counter, in_loop, async_mode);
        let generated_tokens = generated.tokens;
        let output_assigns = assign_outputs_to_slots(&generated.outputs, &outputs);
        current_borrowed = generated.outputs.borrowed.clone();

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
        outputs: {
            outputs.borrowed = current_borrowed;
            outputs
        },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use syn::parse_quote;

    use super::collect_parallel_entry_usage;
    use crate::shared::{ExprShape, NodeCall, NodeExpr};

    #[test]
    fn collect_parallel_entry_usage_counts_branch_consumers() {
        let left = ExprShape {
            entry_usage: [("value".into(), 1)].into_iter().collect(),
            entry_borrowed: BTreeSet::new(),
            exit_outputs: Vec::new(),
            exit_borrowed: BTreeSet::new(),
        };
        let right = ExprShape {
            entry_usage: [("value".into(), 1), ("other".into(), 1)]
                .into_iter()
                .collect(),
            entry_borrowed: BTreeSet::new(),
            exit_outputs: Vec::new(),
            exit_borrowed: BTreeSet::new(),
        };

        let usage = collect_parallel_entry_usage(&[left, right]);

        assert_eq!(usage.get("value"), Some(&2));
        assert_eq!(usage.get("other"), Some(&1));
    }

    #[test]
    fn parallel_with_async_mode_uses_sequential_strategy() {
        let expr = NodeExpr::Parallel(vec![
            NodeExpr::Single(NodeCall {
                path: parse_quote!(demo::A),
                explicit_inputs: false,
                inputs: Vec::new(),
                input_kinds: Vec::new(),
                outputs: Vec::new(),
                output_borrows: Vec::new(),
            }),
            NodeExpr::Single(NodeCall {
                path: parse_quote!(demo::B),
                explicit_inputs: false,
                inputs: Vec::new(),
                input_kinds: Vec::new(),
                outputs: Vec::new(),
                output_borrows: Vec::new(),
            }),
        ]);

        let generated = super::get_parallel_nodes_expr(
            match &expr {
                NodeExpr::Parallel(nodes) => nodes,
                _ => unreachable!(),
            },
            &crate::shared::Payload::new(),
            &mut 0,
            false,
            true,
        );

        assert!(!generated.tokens.to_string().contains("thread :: scope"));
    }
}
