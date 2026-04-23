use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens as _;
use syn::Ident;

use crate::shared::{MetricsSpec, NodeExpr};

pub(super) fn build_graph_dto(
    name: &Ident,
    context: &syn::Path,
    graph_inputs: &[(Ident, syn::Type)],
    graph_outputs: &[(Ident, syn::Type)],
    nodes: &NodeExpr,
    metrics: &MetricsSpec,
    raw_schema_lit: &syn::LitStr,
    docs_tokens: TokenStream,
    tags_tokens: Vec<syn::LitStr>,
    deprecated_token: bool,
    deprecated_reason_tokens: TokenStream,
) -> proc_macro2::TokenStream {
    let export_inputs: Vec<_> = graph_inputs
        .iter()
        .map(|(ident, ty)| {
            quote! {
                ::graphium::export::IoParamDto {
                    name: stringify!(#ident).to_string(),
                    ty: stringify!(#ty).to_string(),
                }
            }
        })
        .collect();
    let export_outputs: Vec<_> = graph_outputs
        .iter()
        .map(|(ident, ty)| {
            quote! {
                ::graphium::export::IoParamDto {
                    name: stringify!(#ident).to_string(),
                    ty: stringify!(#ty).to_string(),
                }
            }
        })
        .collect();
    let export_metrics = metric_names_tokens(metrics);

    let export_paths = collect_export_paths(nodes);
    let export_nodes: Vec<_> = export_paths
        .node_paths
        .iter()
        .map(|path| quote! { #path::__graphium_dto() })
        .collect();
    let export_subgraphs: Vec<_> = export_paths
        .graph_paths
        .iter()
        .map(|path| quote! { #path::__graphium_dto() })
        .collect();

    let subgraphs_runs: Vec<_> = export_paths
        .graph_paths
        .iter()
        .map(|path| quote! { out.extend(#path::__graphium_test_runs()); })
        .collect();
    let nodes_runs: Vec<_> = export_paths
        .node_paths
        .iter()
        .map(|path| quote! { out.extend(#path::__graphium_test_runs()); })
        .collect();

    quote! {
        impl #name {
            pub fn __graphium_dto() -> ::graphium::export::GraphDto {
                let flow = Self::__graphium_flow();
                ::graphium::export::GraphDto {
                    id: ::graphium::export::slugify(stringify!(#name)),
                    name: stringify!(#name).to_string(),
                    docs: #docs_tokens,
                    tags: vec![ #( #tags_tokens.to_string() ),* ],
                    deprecated: #deprecated_token,
                    deprecated_reason: #deprecated_reason_tokens,
                    schema: ::std::option::Option::Some(::graphium::export::GraphSchemaDto {
                        context: stringify!(#context).to_string(),
                        inputs: vec![ #( #export_inputs ),* ],
                        outputs: vec![ #( #export_outputs ),* ],
                        metrics: vec![ #( #export_metrics ),* ],
                    }),
                    flow,
                    raw_schema: ::std::option::Option::Some(#raw_schema_lit.to_string()),
                    tests: Vec::new(),
                    nodes: vec![ #( #export_nodes ),* ],
                    subgraphs: vec![ #( #export_subgraphs ),* ],
                    playground: ::std::option::Option::Some(::graphium::export::PlaygroundDto {
                        supported: <Self as ::graphium::GraphPlayground>::PLAYGROUND_SUPPORTED,
                        schema: ::graphium::export::PlaygroundSchemaDto::from_schema(
                            &<Self as ::graphium::GraphPlayground>::playground_schema(),
                        ),
                    }),
                }
            }

            pub fn __graphium_test_runs() -> ::std::vec::Vec<::graphium::export::TestRun> {
                let mut out: ::std::vec::Vec<::graphium::export::TestRun> = ::std::vec::Vec::new();
                #[cfg(feature = "serialize")]
                {
                    #( #subgraphs_runs )*
                    #( #nodes_runs )*
                }
                out
            }
        }

        impl ::graphium::GraphUiTests for #name {
            fn graphium_ui_tests() -> ::std::vec::Vec<::graphium::export::TestRun> {
                Self::__graphium_test_runs()
            }
        }

        #[cfg(feature = "serialize")]
        impl ::graphium::serde::Serialize for #name {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: ::graphium::serde::Serializer,
            {
                let dto = Self::__graphium_dto();
                ::graphium::serde::Serialize::serialize(&dto, serializer)
            }
        }
    }
}

fn metric_names_tokens(spec: &MetricsSpec) -> Vec<proc_macro2::TokenStream> {
    let mut out = Vec::new();
    if spec.performance {
        out.push(quote! { "performance".to_string() });
    }
    if spec.errors {
        out.push(quote! { "errors".to_string() });
    }
    if spec.count {
        out.push(quote! { "count".to_string() });
    }
    if spec.caller {
        out.push(quote! { "caller".to_string() });
    }
    if spec.success_rate {
        out.push(quote! { "success_rate".to_string() });
    }
    if spec.fail_rate {
        out.push(quote! { "fail_rate".to_string() });
    }
    out
}

struct ExportPaths {
    node_paths: Vec<syn::Path>,
    graph_paths: Vec<syn::Path>,
}

fn collect_export_paths(expr: &NodeExpr) -> ExportPaths {
    use std::collections::BTreeMap;

    let mut nodes = BTreeMap::<String, syn::Path>::new();
    let mut graphs = BTreeMap::<String, syn::Path>::new();
    collect_paths_inner(expr, &mut nodes, &mut graphs);

    ExportPaths {
        node_paths: nodes.into_values().collect(),
        graph_paths: graphs.into_values().collect(),
    }
}

fn collect_paths_inner(
    expr: &NodeExpr,
    nodes: &mut std::collections::BTreeMap<String, syn::Path>,
    graphs: &mut std::collections::BTreeMap<String, syn::Path>,
) {
    match expr {
        NodeExpr::Single(call) => {
            let path = &call.path;
            if crate::shared::is_graph_run_path(path) {
                let graph_path = super::single::graph_type_path(path);
                graphs
                    .entry(graph_path.to_token_stream().to_string())
                    .or_insert(graph_path);
            } else {
                nodes
                    .entry(path.to_token_stream().to_string())
                    .or_insert_with(|| path.clone());
            }
        }
        NodeExpr::Sequence(items) | NodeExpr::Parallel(items) => {
            for item in items {
                collect_paths_inner(item, nodes, graphs);
            }
        }
        NodeExpr::Route(route) => {
            for (_, item) in &route.routes {
                collect_paths_inner(item, nodes, graphs);
            }
        }
        NodeExpr::While(while_expr) => {
            collect_paths_inner(while_expr.body.as_ref(), nodes, graphs);
        }
        NodeExpr::Loop(loop_expr) => {
            collect_paths_inner(loop_expr.body.as_ref(), nodes, graphs);
        }
        NodeExpr::Break => {}
    }
}
