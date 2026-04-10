use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input,
    parse::{Parse, ParseStream},
    Ident, Result, Token, ItemFn, Path, Expr,
};

// ===================================================
//                    node! macro
// ===================================================

#[proc_macro]
pub fn node(input: TokenStream) -> TokenStream {
    let func = parse_macro_input!(input as ItemFn);
    TokenStream::from(quote! { #func })
}

// ===================================================
//                    GRAPH IR
// ===================================================

#[derive(Clone)]
enum NodeExpr {
    Single(Path),
    Sequence(Vec<NodeExpr>),
    Parallel(Vec<NodeExpr>),
    Route(RouteExpr),
}

// ===================================================
//                    ROUTE IR
// ===================================================

#[derive(Clone)]
struct RouteExpr {
    on: Expr,
    routes: Vec<(Expr, NodeExpr)>,
}

// ===================================================
//                  GRAPH INPUT
// ===================================================

struct GraphInput {
    name: Ident,
    context: Path,
    nodes: NodeExpr,
}

// ===================================================
//                NODE PARSING
// ===================================================

impl Parse for NodeExpr {
    fn parse(input: ParseStream) -> Result<Self> {
        parse_sequence(input)
    }
}

// ---------------- sequence (>>) ----------------

fn parse_sequence(input: ParseStream) -> Result<NodeExpr> {
    let mut nodes = vec![parse_parallel(input)?];

    while input.peek(Token![>>]) {
        input.parse::<Token![>>]>()?;
        nodes.push(parse_parallel(input)?);
    }

    if nodes.len() == 1 {
        Ok(nodes.remove(0))
    } else {
        Ok(NodeExpr::Sequence(nodes))
    }
}

// ---------------- parallel (&) ----------------

fn parse_parallel(input: ParseStream) -> Result<NodeExpr> {
    let mut nodes = vec![parse_primary(input)?];

    while input.peek(Token![&]) {
        input.parse::<Token![&]>()?;
        nodes.push(parse_primary(input)?);
    }

    if nodes.len() == 1 {
        Ok(nodes.remove(0))
    } else {
        Ok(NodeExpr::Parallel(nodes))
    }
}

// ===================================================
//                PRIMARY EXPRESSIONS
// ===================================================

fn parse_primary(input: ParseStream) -> Result<NodeExpr> {
    // detect @route
    if input.peek(Token![@]) {
        input.parse::<Token![@]>()?;
        let ident: Ident = input.parse()?;

        if ident != "route" {
            return Err(input.error("expected `route` after `@`"));
        }

        let content;
        syn::braced!(content in input);

        return Ok(NodeExpr::Route(content.parse()?));
    }

    // normal node path
    let path: Path = input.parse()?;
    Ok(NodeExpr::Single(path))
}

// ===================================================
//                ROUTE PARSER
// ===================================================

impl Parse for RouteExpr {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut on: Option<Expr> = None;
        let mut routes: Vec<(Expr, NodeExpr)> = vec![];

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match key.to_string().as_str() {
                "on" => {
                    let expr: Expr = input.parse()?;
                    on = Some(expr);
                    input.parse::<Token![,]>().ok();
                }

                "routes" => {
                    let content;
                    syn::braced!(content in input);

                    while !content.is_empty() {
                        let key_expr: Expr = content.parse()?;
                        content.parse::<Token![=>]>()?;

                        let value: NodeExpr = content.parse()?;

                        routes.push((key_expr, value));

                        content.parse::<Token![,]>().ok();
                    }

                    input.parse::<Token![,]>().ok();
                }

                _ => return Err(input.error("expected `on` or `routes` in @route")),
            }
        }

        Ok(RouteExpr {
            on: on.ok_or_else(|| input.error("missing `on` in @route"))?,
            routes,
        })
    }
}

// ===================================================
//                GRAPH INPUT PARSER
// ===================================================

impl Parse for GraphInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let name_label: Ident = input.parse()?;
        if name_label != "name" {
            return Err(input.error("expected `name`"));
        }
        input.parse::<Token![:]>()?;
        let name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;

        let context_label: Ident = input.parse()?;
        if context_label != "context" {
            return Err(input.error("expected `context`"));
        }
        input.parse::<Token![:]>()?;
        let context: Path = input.parse()?;
        input.parse::<Token![,]>()?;

        let nodes_label: Ident = input.parse()?;
        if nodes_label != "nodes" {
            return Err(input.error("expected `nodes`"));
        }
        input.parse::<Token![:]>()?;

        let content;
        syn::bracketed!(content in input);
        let nodes: NodeExpr = content.parse()?;

        Ok(GraphInput { name, context, nodes })
    }
}

// ===================================================
//                GRAPH MACRO
// ===================================================

#[proc_macro]
pub fn graph(input: TokenStream) -> TokenStream {
    let GraphInput { name, context, nodes } =
        parse_macro_input!(input as GraphInput);

    let body = generate(&nodes);

    let expanded = quote! {
        pub struct #name;

        impl #name {
            pub fn run(ctx: &mut #context) {
                #body
            }
        }
    };

    TokenStream::from(expanded)
}

// ===================================================
//                CODEGEN
// ===================================================

fn generate(node: &NodeExpr) -> proc_macro2::TokenStream {
    match node {
        NodeExpr::Single(path) => {
            quote! {
                #path(ctx);
            }
        }

        NodeExpr::Sequence(nodes) => {
            let parts = nodes.iter().map(generate);
            quote! {
                #( #parts )*
            }
        }

        NodeExpr::Parallel(nodes) => {
            let parts = nodes.iter().map(generate);
            quote! {
                // parallel (currently sequential execution)
                #( #parts )*
            }
        }

        NodeExpr::Route(route) => {
            let on_expr = &route.on;

            let routes = route.routes.iter().map(|(key, node)| {
                let body = generate(node);
                quote! {
                    #key => { #body }
                }
            });

            quote! {
                match (#on_expr)(ctx) {
                    #( #routes, )*
                }
            }
        }
    }
}