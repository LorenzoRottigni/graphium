//! Static shape analysis for graph expressions.
//!
//! Before code generation, each expression is summarized into entry and exit
//! artifact requirements. That summary lets parent expressions decide which
//! artifacts to forward and validate branch/loop contracts.

use std::collections::BTreeSet;

use crate::shared::{ArtifactInputKind, ExprShape, NodeCall, NodeExpr, UsageMap};

use super::{
    SelectorParam, collect_parallel_entry_usage, loop_exit_outputs, route_exit_outputs,
    selector_params_for_on_expr,
};

/// Computes the entry requirements and possible exit artifacts of a graph
/// expression without generating executable code.
///
/// Example:
/// given `A(x) -> y >> B(y) -> z`, this expands into an `ExprShape` roughly like
/// `entry_usage = { "x": 1 }` and `exit_outputs = ["z"]`.
pub(super) fn analyze_expr(node: &NodeExpr) -> ExprShape {
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

/// Computes the shape of a single node call.
///
/// Example:
/// given `Worker(input, &shared) -> output`, this expands into an `ExprShape`
/// with owned entry usage for `input`, borrowed entry usage for `shared`, and
/// `exit_outputs = ["output"]`.
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
    for (input, kind) in call.inputs.iter().zip(call.input_kinds.iter()) {
        match kind {
            ArtifactInputKind::Owned => {
                *entry_usage.entry(input.to_string()).or_insert(0) += 1;
            }
            ArtifactInputKind::Borrowed | ArtifactInputKind::Taken => {
                entry_borrowed.insert(input.to_string());
            }
        }
    }

    ExprShape {
        entry_usage,
        entry_borrowed,
        exit_outputs: call
            .outputs
            .iter()
            .zip(call.output_borrows.iter())
            .filter_map(|(output, is_borrowed)| (!*is_borrowed).then(|| output.to_string()))
            .collect(),
        exit_borrowed: call
            .outputs
            .iter()
            .zip(call.output_borrows.iter())
            .filter_map(|(output, is_borrowed)| (*is_borrowed).then(|| output.to_string()))
            .collect(),
    }
}

/// Returns the ordered list of artifact names required at the entry of a graph
/// subexpression.
///
/// Example:
/// given `ExprShape { entry_usage: { "a": 1, "b": 2 }, .. }`, this expands into
/// `vec!["a", "b"]`.
pub(super) fn required_artifacts(shape: &ExprShape) -> Vec<String> {
    shape.entry_usage.keys().cloned().collect()
}

/// Returns the borrowed artifacts required at expression entry.
///
/// Example:
/// given `ExprShape { entry_borrowed: {"ctx_value"}, .. }`, this expands into
/// `vec!["ctx_value"]`.
pub(super) fn required_borrowed(shape: &ExprShape) -> Vec<String> {
    shape.entry_borrowed.iter().cloned().collect()
}

/// Collects and validates the outgoing artifact names of a parallel step.
///
/// Example:
/// given branch shapes that exit with `["left"]` and `["right"]`, this expands
/// into `vec!["left", "right"]`; duplicate names panic.
pub(super) fn collect_parallel_outputs(shapes: &[ExprShape]) -> Vec<String> {
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
///
/// Example:
/// given route branches producing `["a"]` and `["b", "a"]`, this expands into
/// `vec!["a", "b"]`.
pub(super) fn collect_route_outputs(shapes: &[ExprShape]) -> Vec<String> {
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

/// Collects the union of borrowed entry requirements across parallel branches.
///
/// Example:
/// given branches borrowing `left_ref` and `right_ref`, this expands into
/// `{"left_ref", "right_ref"}`.
pub(super) fn collect_parallel_entry_borrowed(shapes: &[ExprShape]) -> BTreeSet<String> {
    let mut borrowed = BTreeSet::new();
    for shape in shapes {
        for artifact in required_borrowed(shape) {
            borrowed.insert(artifact);
        }
    }
    borrowed
}

/// Collects the union of borrowed outputs across parallel branches.
///
/// Example:
/// given branch exit borrows `{"shared"}` and `{"tail"}`, this expands into
/// `{"shared", "tail"}`.
pub(super) fn collect_parallel_borrowed(shapes: &[ExprShape]) -> BTreeSet<String> {
    let mut borrowed = BTreeSet::new();
    for shape in shapes {
        for artifact in &shape.exit_borrowed {
            borrowed.insert(artifact.clone());
        }
    }
    borrowed
}

/// Collects the union of borrowed outputs across route branches.
///
/// Example:
/// given route branches that borrow `selected` and `fallback`, this expands
/// into `{"fallback", "selected"}`.
pub(super) fn collect_route_borrowed(shapes: &[ExprShape]) -> BTreeSet<String> {
    let mut borrowed = BTreeSet::new();
    for shape in shapes {
        for artifact in &shape.exit_borrowed {
            borrowed.insert(artifact.clone());
        }
    }
    borrowed
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use syn::parse_quote;

    use super::{
        analyze_expr, collect_parallel_outputs, collect_route_outputs, required_artifacts,
    };
    use crate::shared::{ArtifactInputKind, ExprShape, LoopExpr, NodeCall, NodeExpr};

    #[test]
    fn analyze_single_counts_duplicate_owned_inputs() {
        let expr = NodeExpr::Single(NodeCall {
            path: parse_quote!(demo::Node),
            explicit_inputs: true,
            inputs: vec![parse_quote!(value), parse_quote!(value)],
            input_kinds: vec![ArtifactInputKind::Owned, ArtifactInputKind::Owned],
            outputs: vec![parse_quote!(out)],
            output_borrows: vec![false],
        });

        let shape = analyze_expr(&expr);

        assert_eq!(shape.entry_usage.get("value"), Some(&2));
        assert_eq!(shape.exit_outputs, vec!["out".to_string()]);
    }

    #[test]
    fn analyze_loop_reuses_body_requirements() {
        let expr = NodeExpr::Loop(LoopExpr {
            body: Box::new(NodeExpr::Single(NodeCall {
                path: parse_quote!(demo::Node),
                explicit_inputs: true,
                inputs: vec![parse_quote!(value)],
                input_kinds: vec![ArtifactInputKind::Owned],
                outputs: vec![parse_quote!(out)],
                output_borrows: vec![false],
            })),
            outputs: vec![parse_quote!(out)],
            output_borrows: vec![false],
        });

        let shape = analyze_expr(&expr);

        assert_eq!(required_artifacts(&shape), vec!["value".to_string()]);
        assert_eq!(shape.exit_outputs, vec!["out".to_string()]);
    }

    #[test]
    fn collect_parallel_outputs_rejects_duplicates() {
        let shape = ExprShape {
            entry_usage: Default::default(),
            entry_borrowed: BTreeSet::new(),
            exit_outputs: vec!["dup".into()],
            exit_borrowed: BTreeSet::new(),
        };

        let result = std::panic::catch_unwind(|| collect_parallel_outputs(&[shape.clone(), shape]));
        assert!(result.is_err());
    }

    #[test]
    fn collect_route_outputs_returns_union_in_order() {
        let first = ExprShape {
            entry_usage: Default::default(),
            entry_borrowed: BTreeSet::new(),
            exit_outputs: vec!["a".into()],
            exit_borrowed: BTreeSet::new(),
        };
        let second = ExprShape {
            entry_usage: Default::default(),
            entry_borrowed: BTreeSet::new(),
            exit_outputs: vec!["b".into(), "a".into()],
            exit_borrowed: BTreeSet::new(),
        };

        assert_eq!(
            collect_route_outputs(&[first, second]),
            vec!["a".to_string(), "b".to_string()]
        );
    }
}
