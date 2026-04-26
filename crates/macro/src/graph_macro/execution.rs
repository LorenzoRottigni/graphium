//! Runtime method synthesis for `graph!`.
//!
//! The `graph!` expander ultimately emits inherent methods on the generated
//! graph type (e.g. `run(...)`, `run_async(...)`).
//!
//! This module is responsible for the pieces that are *specific to the outer
//! graph wrapper*, not the inner expression tree:
//! - turning declared graph inputs into stable runtime parameters
//! - building the "root payload" that seeds the expression expander
//! - choosing between sync/async execution paths
//! - shaping the return signature and return expression
//!
//! The heavy lifting of expanding `NodeExpr` into executable code lives in
//! `graph_macro::expr`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use super::expr::get_node_expr;
use crate::ir::{GeneratedExpr, NodeExpr, Payload, fresh_ident};

pub(super) struct RootSetup {
    pub root_incoming: Payload,
    pub run_params: Vec<TokenStream>,
    pub run_args: Vec<Ident>,
    pub root_input_bindings: Vec<TokenStream>,
}

pub(super) struct GeneratedExecution {
    pub generated_sync: Option<GeneratedExpr>,
    pub generated_async: GeneratedExpr,
}

pub(super) fn build_root_setup(
    graph_inputs: &[(Ident, syn::Type)],
    counter: &mut usize,
) -> RootSetup {
    let mut root_incoming = Payload::new();
    let mut run_params = Vec::with_capacity(graph_inputs.len());
    let mut run_args = Vec::with_capacity(graph_inputs.len());
    let mut root_input_bindings = Vec::with_capacity(graph_inputs.len());

    for (artifact, ty) in graph_inputs {
        let param_ident = fresh_ident(counter, "graph_in", &artifact.to_string());
        let payload_ident = fresh_ident(counter, "root_in", &artifact.to_string());
        root_incoming.insert_owned(artifact.to_string(), payload_ident.clone());
        run_params.push(quote! {
            #param_ident: #ty
        });
        run_args.push(param_ident.clone());
        root_input_bindings.push(quote! {
            let mut #payload_ident = ::std::option::Option::Some(#param_ident);
        });
    }

    RootSetup {
        root_incoming,
        run_params,
        run_args,
        root_input_bindings,
    }
}

pub(super) fn generate_execution(
    nodes: &NodeExpr,
    root_incoming: &Payload,
    counter: &mut usize,
    async_enabled: bool,
) -> GeneratedExecution {
    let generated_sync = if async_enabled {
        None
    } else {
        Some(get_node_expr(nodes, root_incoming, counter, false, false))
    };
    let generated_async = get_node_expr(nodes, root_incoming, counter, false, true);

    GeneratedExecution {
        generated_sync,
        generated_async,
    }
}

pub(super) fn build_run_return_sig(graph_outputs: &[(Ident, syn::Type)]) -> TokenStream {
    if graph_outputs.is_empty() {
        quote! {}
    } else if graph_outputs.len() == 1 {
        let (_, ty) = &graph_outputs[0];
        quote! { -> #ty }
    } else {
        let tys = graph_outputs.iter().map(|(_, ty)| ty);
        quote! { -> ( #( #tys ),* ) }
    }
}

pub(super) fn build_run_body(
    generated: Option<&GeneratedExpr>,
    root_input_bindings: &[TokenStream],
    borrowed_slot_idents: &[syn::Ident],
    graph_outputs: &[(Ident, syn::Type)],
    disabled: bool,
) -> TokenStream {
    if disabled {
        return quote! {};
    }

    let generated = generated.expect("generated graph body");
    let generated_tokens = generated.tokens.clone();
    let return_expr = build_return_expr(generated, graph_outputs);
    let borrowed_slot_decls = borrowed_slot_idents.iter().map(|ident| {
        quote! {
            let mut #ident = ::std::option::Option::None;
        }
    });

    if graph_outputs.is_empty() {
        quote! {{
            #( #root_input_bindings )*
            #( #borrowed_slot_decls )*
            #generated_tokens
        }}
    } else {
        quote! {{
            #( #root_input_bindings )*
            #( #borrowed_slot_decls )*
            #generated_tokens
            #return_expr
        }}
    }
}

fn build_return_expr(
    generated: &GeneratedExpr,
    graph_outputs: &[(Ident, syn::Type)],
) -> TokenStream {
    let output_values: Vec<TokenStream> = graph_outputs
        .iter()
        .map(|(artifact, _)| {
            let artifact_name = artifact.to_string();
            let output_var = generated
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

    if output_values.len() == 1 {
        quote! { #(#output_values)* }
    } else {
        quote! { ( #( #output_values ),* ) }
    }
}
