//! Graph definition rendering.
//!
//! Besides executable code, the macro also emits a structural `GraphDef`
//! description for UI and inspection features.

use quote::quote;

use crate::shared::{NodeCall, NodeExpr, is_graph_run_path};

use super::graph_type_path;

/// Builds the `GraphDef` literal returned by generated graph types.
///
/// Example:
/// providing `name = DemoGraph` and `A >> B` expands into
/// `::graphium::GraphDef { name: "DemoGraph", steps: vec![...] }`.
pub(super) fn graph_definition_tokens(
    name: &syn::Ident,
    inputs: &[(syn::Ident, syn::Type)],
    outputs: &[(syn::Ident, syn::Type)],
    nodes: &NodeExpr,
) -> proc_macro2::TokenStream {
    let steps = node_expr_steps_tokens(nodes);
    let input_names: Vec<_> = inputs
        .iter()
        .map(|(ident, _)| quote! { stringify!(#ident) })
        .collect();
    let output_names: Vec<_> = outputs
        .iter()
        .map(|(ident, _)| quote! { stringify!(#ident) })
        .collect();
    quote! {
        ::graphium::GraphDef {
            name: stringify!(#name),
            inputs: vec![ #( #input_names ),* ],
            outputs: vec![ #( #output_names ),* ],
            steps: vec![ #( #steps ),* ],
        }
    }
}

/// Flattens a graph expression into the UI-oriented `GraphStep` tree.
///
/// Example:
/// providing `A >> (B | C)` expands into a `Vec<GraphStep>` containing a node
/// step for `A` followed by a `GraphStep::Parallel { ... }`.
fn node_expr_steps_tokens(node: &NodeExpr) -> Vec<proc_macro2::TokenStream> {
    match node {
        NodeExpr::Single(call) => vec![node_call_step_tokens(call)],
        NodeExpr::Sequence(nodes) => nodes.iter().flat_map(node_expr_steps_tokens).collect(),
        NodeExpr::Parallel(nodes) => {
            let shape = super::analyze_expr(node);
            let inputs = super::required_artifacts(&shape);
            let borrowed_inputs = super::required_borrowed(&shape);
            let outputs = shape.exit_outputs;
            let borrowed_outputs: Vec<String> = shape.exit_borrowed.into_iter().collect();

            let mut input_labels = inputs;
            for borrowed in borrowed_inputs {
                input_labels.push(format!("&{borrowed}"));
            }
            let mut output_labels = outputs;
            for borrowed in borrowed_outputs {
                output_labels.push(format!("&{borrowed}"));
            }

            let input_tokens = static_str_list_tokens(&input_labels);
            let output_tokens = static_str_list_tokens(&output_labels);

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
                    inputs: vec![ #( #input_tokens ),* ],
                    outputs: vec![ #( #output_tokens ),* ],
                }
            }]
        }
        NodeExpr::Route(route) => {
            let shape = super::analyze_expr(node);
            let outputs = shape.exit_outputs;
            let borrowed_outputs: Vec<String> = shape.exit_borrowed.into_iter().collect();
            let mut output_labels = outputs;
            for borrowed in borrowed_outputs {
                output_labels.push(format!("&{borrowed}"));
            }
            let output_tokens = static_str_list_tokens(&output_labels);

            let selector_params = super::selector_params_for_on_expr(&route.on);
            let input_labels: Vec<String> = selector_params
                .into_iter()
                .filter_map(|param| match param {
                    super::SelectorParam::Artifact { ident, borrowed } => {
                        let base = ident.to_string();
                        if borrowed {
                            Some(format!("&{base}"))
                        } else {
                            Some(base)
                        }
                    }
                    super::SelectorParam::Ctx { .. } => None,
                })
                .collect();
            let input_tokens = static_str_list_tokens(&input_labels);

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
                    inputs: vec![ #( #input_tokens ),* ],
                    outputs: vec![ #( #output_tokens ),* ],
                }
            }]
        }
        NodeExpr::While(while_expr) => {
            let shape = super::analyze_expr(node);
            let outputs = shape.exit_outputs;
            let borrowed_outputs: Vec<String> = shape.exit_borrowed.into_iter().collect();
            let mut output_labels = outputs;
            for borrowed in borrowed_outputs {
                output_labels.push(format!("&{borrowed}"));
            }
            let output_tokens = static_str_list_tokens(&output_labels);

            let selector_params = super::selector_params_for_on_expr(&while_expr.condition);
            let input_labels: Vec<String> = selector_params
                .into_iter()
                .filter_map(|param| match param {
                    super::SelectorParam::Artifact { ident, borrowed } => {
                        let base = ident.to_string();
                        if borrowed {
                            Some(format!("&{base}"))
                        } else {
                            Some(base)
                        }
                    }
                    super::SelectorParam::Ctx { .. } => None,
                })
                .collect();
            let input_tokens = static_str_list_tokens(&input_labels);

            let condition = &while_expr.condition;
            let body_steps = node_expr_steps_tokens(&while_expr.body);
            vec![quote! {
                ::graphium::GraphStep::While {
                    condition: stringify!(#condition),
                    body: vec![ #( #body_steps ),* ],
                    inputs: vec![ #( #input_tokens ),* ],
                    outputs: vec![ #( #output_tokens ),* ],
                }
            }]
        }
        NodeExpr::Loop(loop_expr) => {
            let shape = super::analyze_expr(node);
            let inputs = super::required_artifacts(&shape);
            let borrowed_inputs = super::required_borrowed(&shape);
            let outputs = shape.exit_outputs;
            let borrowed_outputs: Vec<String> = shape.exit_borrowed.into_iter().collect();
            let mut input_labels = inputs;
            for borrowed in borrowed_inputs {
                input_labels.push(format!("&{borrowed}"));
            }
            let mut output_labels = outputs;
            for borrowed in borrowed_outputs {
                output_labels.push(format!("&{borrowed}"));
            }
            let input_tokens = static_str_list_tokens(&input_labels);
            let output_tokens = static_str_list_tokens(&output_labels);

            let body_steps = node_expr_steps_tokens(&loop_expr.body);
            vec![quote! {
                ::graphium::GraphStep::Loop {
                    body: vec![ #( #body_steps ),* ],
                    inputs: vec![ #( #input_tokens ),* ],
                    outputs: vec![ #( #output_tokens ),* ],
                }
            }]
        }
        NodeExpr::Break => vec![quote! { ::graphium::GraphStep::Break }],
    }
}

fn static_str_list_tokens(values: &[String]) -> Vec<proc_macro2::TokenStream> {
    values
        .iter()
        .map(|value| {
            let lit = syn::LitStr::new(value, proc_macro2::Span::call_site());
            quote! { #lit }
        })
        .collect()
}

/// Converts a node or nested graph call into a `GraphStep`.
///
/// Example:
/// providing `Worker(input) -> output` expands into
/// `::graphium::GraphStep::Node { name: "Worker", ... }`, while
/// `OtherGraph::run(...)` expands into `GraphStep::Nested { ... }`.
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

/// Renders artifact labels, prefixing borrowed values with `&`.
///
/// Example:
/// providing idents `[value, shared]` with borrows `[false, true]` expands into
/// tokens like `["value", "&shared"]`.
fn artifact_list_tokens(idents: &[syn::Ident], borrows: &[bool]) -> Vec<proc_macro2::TokenStream> {
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

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::graph_definition_tokens;
    use crate::shared::{NodeCall, NodeExpr};

    #[test]
    fn graph_definition_tokens_render_parallel_step_tree() {
        let nodes = NodeExpr::Parallel(vec![
            NodeExpr::Single(NodeCall {
                path: parse_quote!(demo::A),
                explicit_inputs: false,
                inputs: Vec::new(),
                input_borrows: Vec::new(),
                outputs: Vec::new(),
                output_borrows: Vec::new(),
            }),
            NodeExpr::Single(NodeCall {
                path: parse_quote!(demo::B),
                explicit_inputs: false,
                inputs: Vec::new(),
                input_borrows: Vec::new(),
                outputs: Vec::new(),
                output_borrows: Vec::new(),
            }),
        ]);

        let tokens = graph_definition_tokens(&parse_quote!(DemoGraph), &[], &[], &nodes).to_string();

        assert!(tokens.contains("GraphStep :: Parallel"));
        assert!(tokens.contains("demo :: A"));
        assert!(tokens.contains("demo :: B"));
    }
}
