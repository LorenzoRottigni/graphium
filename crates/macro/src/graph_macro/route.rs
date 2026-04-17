//! Route expansion and validation.
//!
//! Routes execute exactly one branch, so this module validates the branch exit
//! contract and then generates move-only payload transfer into the selected arm.

use std::collections::BTreeSet;

use quote::quote;

use crate::shared::{ExprShape, GeneratedExpr, Payload, RouteExpr};

use super::{
    analyze_expr, assign_outputs_to_slots, build_selector_bindings, build_selector_call,
    prepare_move_payload, prepare_output_slots, required_artifacts, required_borrowed,
    selector_params_for_on_expr,
};

/// Generates code for an exclusive route branch.
///
/// Example:
/// providing `@match on selector { 0 => A, 1 => B }` expands into
/// `let selector_key = ...; match selector_key { 0 => { ... }, 1 => { ... } }`.
pub(super) fn get_route_node_expr(
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

    let selector_tokens =
        build_selector_bindings(&selector_params, incoming, &needed_by_branches, counter);
    let selector_call =
        build_selector_call(on_expr, &selector_tokens.args, selector_tokens.is_empty);
    let selector_bindings = &selector_tokens.bindings;
    let selector_key_ident = crate::shared::fresh_ident(counter, "selector_key", "if");
    let mut arms = Vec::new();
    for ((key, node), shape) in route.routes.iter().zip(branch_shapes.iter()) {
        let artifacts = required_artifacts(shape);
        let (branch_payload, branch_bindings) =
            prepare_move_payload(incoming, &artifacts, "route_in", counter);

        let generated = super::get_node_expr(node, &branch_payload, counter, in_loop, async_mode);
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

/// Computes the route exit payload contract.
///
/// Example:
/// providing declared outputs `[value, &shared]` expands into
/// `(vec!["value"], {"shared"})`; without explicit outputs it unions branch exits.
pub(super) fn route_exit_outputs(
    route: &RouteExpr,
    shapes: &[ExprShape],
) -> (Vec<String>, BTreeSet<String>) {
    if route.outputs.is_empty() {
        return (
            super::collect_route_outputs(shapes),
            super::collect_route_borrowed(shapes),
        );
    }

    let mut owned = Vec::new();
    let mut borrowed = BTreeSet::new();
    for (output, is_borrowed) in route.outputs.iter().zip(route.output_borrows.iter()) {
        if *is_borrowed {
            borrowed.insert(output.to_string());
        } else {
            owned.push(output.to_string());
        }
    }

    (owned, borrowed)
}

/// Validates that every route branch agrees with the declared route outputs.
///
/// Example:
/// providing declared outputs `["value"]` and a branch that exits with `["other"]`
/// expands into a panic because the branch contract does not match.
pub(super) fn validate_route_outputs(route: &RouteExpr, shapes: &[ExprShape]) {
    let mut expected_owned = BTreeSet::new();
    let mut expected_borrowed = BTreeSet::new();
    for (output, is_borrowed) in route.outputs.iter().zip(route.output_borrows.iter()) {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use syn::parse_quote;

    use super::{route_exit_outputs, validate_route_outputs};
    use crate::shared::{ExprShape, RouteExpr};

    #[test]
    fn route_exit_outputs_use_declared_signature_when_present() {
        let route = RouteExpr {
            on: parse_quote!(selector),
            routes: Vec::new(),
            outputs: vec![parse_quote!(owned), parse_quote!(borrowed)],
            output_borrows: vec![false, true],
            is_if_chain: false,
        };

        let (owned, borrowed) = route_exit_outputs(&route, &[]);

        assert_eq!(owned, vec!["owned".to_string()]);
        assert!(borrowed.contains("borrowed"));
    }

    #[test]
    fn validate_route_outputs_rejects_missing_branch_output() {
        let route = RouteExpr {
            on: parse_quote!(selector),
            routes: Vec::new(),
            outputs: vec![parse_quote!(value)],
            output_borrows: vec![false],
            is_if_chain: false,
        };
        let shapes = vec![ExprShape {
            entry_usage: Default::default(),
            entry_borrowed: BTreeSet::new(),
            exit_outputs: Vec::new(),
            exit_borrowed: BTreeSet::new(),
        }];

        let result = std::panic::catch_unwind(|| validate_route_outputs(&route, &shapes));
        assert!(result.is_err());
    }
}
