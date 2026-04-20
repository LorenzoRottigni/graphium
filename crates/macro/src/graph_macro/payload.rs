//! Hop payload helpers.
//!
//! These helpers move or clone artifacts between expression boundaries while
//! keeping the generated locals explicit and testable.

use quote::quote;

use crate::shared::{fresh_ident, ExprShape, Payload, UsageMap};

use super::required_artifacts;

/// Builds a fresh hop payload by moving the requested artifacts out of the
/// current expression outputs.
///
/// Example:
/// providing source outputs `{"value" => hop_value}` and requested artifacts
/// `["value"]` expands into `let mut next_value = hop_value.take();`.
pub(super) fn prepare_move_payload(
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
/// Example:
/// providing a shared artifact `value` for two branches expands into either
/// `Some(clone_artifact(...))` for early branches or `.take()` for the last one.
pub(super) fn prepare_parallel_payload(
    incoming: &Payload,
    shape: &ExprShape,
    remaining: &mut UsageMap,
    counter: &mut usize,
) -> (Payload, Vec<proc_macro2::TokenStream>) {
    let mut payload = Payload::new();
    payload.borrowed = incoming.borrowed.clone();
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

    (payload, bindings)
}

/// Declares outer slots for the outputs of a composite expression.
///
/// Example:
/// providing artifacts `["left", "right"]` expands into declarations like
/// `let mut __graphium_parallel_out_*_left = None;`.
pub(super) fn prepare_output_slots(
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
///
/// Example:
/// providing inner output `inner_left` and outer slot `outer_left` expands into
/// `outer_left = inner_left;`.
pub(super) fn assign_outputs_to_slots(
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

#[cfg(test)]
mod tests {
    use syn::Ident;

    use super::{
        assign_outputs_to_slots, prepare_move_payload, prepare_output_slots,
        prepare_parallel_payload,
    };
    use crate::shared::{ExprShape, Payload, UsageMap};

    #[test]
    fn prepare_move_payload_preserves_borrowed_state() {
        let mut source = Payload::new();
        source.insert_owned(
            "value".into(),
            Ident::new("slot", proc_macro2::Span::call_site()),
        );
        source.insert_borrowed("shared".into());

        let (payload, bindings) = prepare_move_payload(&source, &["value".into()], "next", &mut 0);

        assert!(payload.has_borrowed("shared"));
        assert_eq!(bindings.len(), 1);
    }

    #[test]
    fn prepare_parallel_payload_clones_until_last_consumer() {
        let mut incoming = Payload::new();
        incoming.insert_owned(
            "value".into(),
            Ident::new("slot", proc_macro2::Span::call_site()),
        );
        incoming.insert_borrowed("borrowed".into());
        let shape = ExprShape {
            entry_usage: [("value".into(), 1)].into_iter().collect(),
            entry_borrowed: ["borrowed".into()].into_iter().collect(),
            exit_outputs: Vec::new(),
            exit_borrowed: Default::default(),
        };
        let mut remaining: UsageMap = [("value".into(), 2)].into_iter().collect();

        let (payload, bindings) =
            prepare_parallel_payload(&incoming, &shape, &mut remaining, &mut 0);

        assert!(payload.get_owned("value").is_some());
        assert!(payload.has_borrowed("borrowed"));
        assert!(bindings[0].to_string().contains("clone_artifact"));
    }

    #[test]
    fn assign_outputs_to_slots_writes_each_owned_artifact() {
        let mut inner = Payload::new();
        let mut outer = Payload::new();
        inner.insert_owned(
            "value".into(),
            Ident::new("inner", proc_macro2::Span::call_site()),
        );
        outer.insert_owned(
            "value".into(),
            Ident::new("outer", proc_macro2::Span::call_site()),
        );

        let assignments = assign_outputs_to_slots(&inner, &outer);

        assert_eq!(assignments.len(), 1);
        assert!(assignments[0].to_string().contains("outer = inner"));
    }

    #[test]
    fn prepare_output_slots_declares_none_initialized_slots() {
        let (payload, declarations) = prepare_output_slots(&["value".into()], "out", &mut 0);

        assert!(payload.get_owned("value").is_some());
        assert!(declarations[0].to_string().contains("Option :: None"));
    }
}
