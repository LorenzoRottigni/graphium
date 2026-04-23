//! Recursive graph-expression dispatch.
//!
//! The helpers here route each parsed IR node to the right code generator and
//! rebind nested outputs back into the parent hop scope when needed.

use quote::quote;

use crate::shared::{GeneratedExpr, NodeExpr, Payload};

use super::{
    get_loop_node_expr, get_parallel_nodes_expr, get_route_node_expr, get_sequence_nodes_expr,
    get_single_node_expr, get_while_node_expr,
};

/// Dispatches code generation to the correct handler for the current graph IR
/// node.
///
/// Example:
/// given `NodeExpr::Parallel([A, B])`, this expands into the same
/// `GeneratedExpr` that `get_parallel_nodes_expr(...)` would produce.
pub(crate) fn get_node_expr(
    node: &NodeExpr,
    incoming: &Payload,
    counter: &mut usize,
    in_loop: bool,
    async_mode: bool,
) -> GeneratedExpr {
    match node {
        NodeExpr::Single(call) => get_single_node_expr(call, incoming, counter, async_mode),
        NodeExpr::Sequence(nodes) => {
            get_sequence_nodes_expr(nodes, incoming, counter, in_loop, async_mode)
        }
        NodeExpr::Parallel(nodes) => {
            get_parallel_nodes_expr(nodes, incoming, counter, in_loop, async_mode)
        }
        NodeExpr::Route(route) => {
            get_route_node_expr(route, incoming, counter, in_loop, async_mode)
        }
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

/// Rebinds outputs created inside a nested scope into fresh outer locals so the
/// parent expression can keep propagating the hop payload.
///
/// Example:
/// given inner tokens that create `inner_value` and inner outputs
/// `{"value" => inner_value}`, this expands into outer declarations like
/// `let mut __graphium_captured_*_value = None; { ...; outer = inner_value; }`.
pub(crate) fn capture_outputs(
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

    let mut outer_outputs = Payload::new();
    let declaration_pairs: Vec<(String, syn::Ident)> = inner_outputs
        .owned
        .keys()
        .map(|artifact| {
            let outer_var = crate::shared::fresh_ident(counter, "captured", artifact);
            (artifact.clone(), outer_var)
        })
        .collect();

    for (artifact, outer_var) in &declaration_pairs {
        outer_outputs.insert_owned(artifact.clone(), outer_var.clone());
    }
    outer_outputs.borrowed = inner_outputs.borrowed.clone();

    let declarations = declaration_pairs.iter().map(|(_, outer_var)| {
        quote! {
            let mut #outer_var = ::std::option::Option::None;
        }
    });

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

/// Returns `true` when a subexpression contains `@break`, which prevents the
/// threaded parallel strategy from being valid.
///
/// Example:
/// given `@loop { A >> @break }`, this expands into `true`; given `A | B`, it
/// expands into `false`.
pub(crate) fn contains_break(node: &NodeExpr) -> bool {
    match node {
        NodeExpr::Break => true,
        NodeExpr::Single(_) => false,
        NodeExpr::Sequence(nodes) | NodeExpr::Parallel(nodes) => nodes.iter().any(contains_break),
        NodeExpr::Route(route) => route.routes.iter().any(|(_, n)| contains_break(n)),
        NodeExpr::While(while_expr) => contains_break(&while_expr.body),
        NodeExpr::Loop(loop_expr) => contains_break(&loop_expr.body),
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::{Ident, parse_quote};

    use super::{capture_outputs, contains_break};
    use crate::shared::{LoopExpr, NodeExpr, Payload};

    #[test]
    fn capture_outputs_rebinds_owned_payload_slots() {
        let mut payload = Payload::new();
        payload.insert_owned(
            "value".into(),
            Ident::new("inner_value", proc_macro2::Span::call_site()),
        );

        let generated = capture_outputs(quote! { let inner_value = Some(1); }, payload, &mut 0);
        let tokens = generated.tokens.to_string();

        assert!(tokens.contains("__graphium_captured_0_value"));
        assert!(generated.outputs.get_owned("value").is_some());
    }

    #[test]
    fn contains_break_detects_nested_loop_breaks() {
        let expr = NodeExpr::Loop(LoopExpr {
            body: Box::new(NodeExpr::Sequence(vec![
                NodeExpr::Single(parse_quote!(worker())),
                NodeExpr::Break,
            ])),
            outputs: Vec::new(),
            output_borrows: Vec::new(),
        });

        assert!(contains_break(&expr));
    }

    #[test]
    fn break_outside_loop_panics() {
        let result = std::panic::catch_unwind(|| {
            let _ = super::get_node_expr(&NodeExpr::Break, &Payload::new(), &mut 0, false, false);
        });

        assert!(result.is_err());
    }
}
