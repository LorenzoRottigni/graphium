//! Loop and while expansion.
//!
//! Looping expressions keep a seed copy of the required inputs, clone it for
//! each iteration, and validate that the body matches the declared exit
//! artifacts.

use std::collections::BTreeSet;

use quote::quote;

use crate::shared::{ExprShape, GeneratedExpr, LoopExpr, Payload, WhileExpr};

use super::{
    analyze_expr, assign_outputs_to_slots, build_condition_bindings, build_condition_call,
    prepare_output_slots, required_artifacts, required_borrowed,
};

/// Generates code for an `@while` expression.
///
/// Example:
/// providing `@while cond { A >> B } -> out` expands into a Rust `while { ... }`
/// block that seeds iteration payloads, runs the body, and stores `out`.
pub(super) fn get_while_node_expr(
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

    let cond_params = super::selector_params_for_on_expr(&while_expr.condition);
    let cond_bindings = build_condition_bindings(&cond_params, incoming, counter);
    let cond_bindings_tokens = &cond_bindings.bindings;
    let cond_call = build_condition_call(
        &while_expr.condition,
        &cond_bindings.args,
        cond_bindings.is_empty,
    );

    let required_owned = required_artifacts(&body_shape);
    let required_borrowed = required_borrowed(&body_shape);

    let mut loop_payload_inits = Vec::new();
    let mut loop_payload = Payload::new();
    loop_payload.borrowed = incoming.borrowed.clone();

    for artifact in &required_owned {
        let source = incoming
            .get_owned(artifact)
            .unwrap_or_else(|| panic!("missing artifact `{artifact}` for @while body"));
        let stored = crate::shared::fresh_ident(counter, "while_seed", artifact);
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
        let iter_var = crate::shared::fresh_ident(counter, "while_in", artifact);
        iter_payload_bindings.push(quote! {
            let mut #iter_var = ::std::option::Option::Some(::graphium::clone_artifact(#stored));
        });
        iter_payload.insert_owned(artifact.clone(), iter_var.clone());
    }
    iter_payload.borrowed = incoming.borrowed.clone();
    iter_payload
        .borrowed
        .extend(body_shape.exit_borrowed.iter().cloned());
    for artifact in &required_borrowed {
        if !iter_payload.has_borrowed(artifact) {
            panic!("missing borrowed artifact `{artifact}` for @while body");
        }
    }

    let body_generated =
        super::get_node_expr(&while_expr.body, &iter_payload, counter, true, async_mode);
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

/// Generates code for an `@loop` expression.
///
/// Example:
/// providing `@loop { A >> B } -> out` expands into a Rust `loop { ... }` block
/// that clones the seed payload on each iteration and assigns `out`.
pub(super) fn get_loop_node_expr(
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
        let stored = crate::shared::fresh_ident(counter, "loop_seed", artifact);
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
        let iter_var = crate::shared::fresh_ident(counter, "loop_in", artifact);
        iter_payload_bindings.push(quote! {
            let mut #iter_var = ::std::option::Option::Some(::graphium::clone_artifact(#stored));
        });
        iter_payload.insert_owned(artifact.clone(), iter_var.clone());
    }
    iter_payload.borrowed = incoming.borrowed.clone();
    iter_payload
        .borrowed
        .extend(body_shape.exit_borrowed.iter().cloned());
    for artifact in &required_borrowed {
        if !iter_payload.has_borrowed(artifact) {
            panic!("missing borrowed artifact `{artifact}` for @loop body");
        }
    }

    let body_generated =
        super::get_node_expr(&loop_expr.body, &iter_payload, counter, true, async_mode);
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

/// Computes the loop exit payload contract.
///
/// Example:
/// providing declared outputs `[value, &shared]` expands into
/// `(vec!["value"], {"shared"})`; with no explicit outputs it reuses the body shape.
pub(super) fn loop_exit_outputs(
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

/// Validates that a loop body produces exactly the declared artifacts.
///
/// Example:
/// providing declared outputs `["value"]` and a body shape that exits with
/// `["value", "extra"]` expands into a panic because `extra` was not declared.
pub(super) fn validate_loop_outputs(
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use syn::parse_quote;

    use super::{loop_exit_outputs, validate_loop_outputs};
    use crate::shared::ExprShape;

    #[test]
    fn loop_exit_outputs_default_to_body_shape() {
        let shape = ExprShape {
            entry_usage: Default::default(),
            entry_borrowed: BTreeSet::new(),
            exit_outputs: vec!["owned".into()],
            exit_borrowed: ["borrowed".into()].into_iter().collect(),
        };

        let (owned, borrowed) = loop_exit_outputs(&[], &[], &shape);

        assert_eq!(owned, vec!["owned".to_string()]);
        assert!(borrowed.contains("borrowed"));
    }

    #[test]
    fn validate_loop_outputs_rejects_extra_body_artifacts() {
        let shape = ExprShape {
            entry_usage: Default::default(),
            entry_borrowed: BTreeSet::new(),
            exit_outputs: vec!["extra".into()],
            exit_borrowed: BTreeSet::new(),
        };

        let result = std::panic::catch_unwind(|| {
            validate_loop_outputs(&[parse_quote!(expected)], &[false], &shape)
        });

        assert!(result.is_err());
    }
}
