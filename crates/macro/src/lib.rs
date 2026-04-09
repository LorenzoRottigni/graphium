use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, parse::{Parse, ParseStream}, Ident, Result, Token, ItemFn, ExprArray, Path,
};

// ------------------ node! macro ------------------
#[proc_macro]
pub fn node(input: TokenStream) -> TokenStream {
    let func = parse_macro_input!(input as ItemFn);
    // Just output the function as-is
    TokenStream::from(quote! { #func })
}

// ------------------ graph! macro ------------------

/// Represents a node or group of nodes in the graph
#[derive(Clone)]
enum NodeExpr {
    Single(Path),               // crate::node::get_data or just get_data
    Sequence(Vec<NodeExpr>),    // A >> B >> C
    Parallel(Vec<NodeExpr>),    // A & B & C
}

struct GraphInput {
    name: Ident,
    nodes: NodeExpr,
}

impl Parse for NodeExpr {
    fn parse(input: ParseStream) -> Result<Self> {
        parse_sequence_expr(input)
    }
}

/// Parse sequential expressions (lowest precedence): A >> B >> C
/// Parallel expressions bind tighter, so: A >> B & C >> D means A >> (B & C) >> D
fn parse_sequence_expr(input: ParseStream) -> Result<NodeExpr> {
    let mut exprs = vec![parse_parallel_expr(input)?];
    
    while input.peek(Token![>>]) {
        input.parse::<Token![>>]>()?;
        exprs.push(parse_parallel_expr(input)?);
    }
    
    if exprs.len() == 1 {
        Ok(exprs.into_iter().next().unwrap())
    } else {
        Ok(NodeExpr::Sequence(exprs))
    }
}

/// Parse parallel expressions (higher precedence): A & B & C
fn parse_parallel_expr(input: ParseStream) -> Result<NodeExpr> {
    let mut exprs = vec![parse_primary_expr(input)?];
    
    while input.peek(Token![&]) {
        input.parse::<Token![&]>()?;
        exprs.push(parse_primary_expr(input)?);
    }
    
    if exprs.len() == 1 {
        Ok(exprs.into_iter().next().unwrap())
    } else {
        Ok(NodeExpr::Parallel(exprs))
    }
}

/// Parse primary expressions: paths (crate::node::func or just func)
fn parse_primary_expr(input: ParseStream) -> Result<NodeExpr> {
    let path: Path = input.parse()?;
    Ok(NodeExpr::Single(path))
}

impl Parse for GraphInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let name_label: Ident = input.parse()?;
        if name_label != "name" {
            return Err(input.error("expected `name`"));
        }
        input.parse::<Token![:]>()?;
        let name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;

        let nodes_label: Ident = input.parse()?;
        if nodes_label != "nodes" {
            return Err(input.error("expected `nodes`"));
        }
        input.parse::<Token![:]>()?;
        
        // Parse the bracket-delimited node expression
        let bracket_content;
        syn::bracketed!(bracket_content in input);
        let nodes: NodeExpr = bracket_content.parse()?;

        Ok(GraphInput { name, nodes })
    }
}

#[proc_macro]
pub fn graph(input: TokenStream) -> TokenStream {
    let GraphInput { name, nodes } = parse_macro_input!(input as GraphInput);

    let run_body = generate_node_calls(&nodes);

    let expanded = quote! {
        pub mod #name {
            pub fn run(ctx: &mut crate::node::Context) {
                #run_body
            }
        }
    };
    TokenStream::from(expanded)
}

/// Generate the appropriate call sequence based on the node structure
fn generate_node_calls(node_expr: &NodeExpr) -> proc_macro2::TokenStream {
    match node_expr {
        NodeExpr::Single(ident) => {
            quote! { #ident(ctx); }
        }
        NodeExpr::Sequence(nodes) => {
            // Execute nodes sequentially (each depends on the previous)
            let calls: Vec<_> = nodes.iter().map(generate_node_calls).collect();
            quote! {
                #( #calls )*
            }
        }
        NodeExpr::Parallel(nodes) => {
            // Execute nodes in parallel (they're independent)
            // For now, we execute them sequentially but mark them as parallel
            // In a real implementation, you'd use a thread pool or async
            let calls: Vec<_> = nodes.iter().map(generate_node_calls).collect();
            quote! {
                // Parallel execution (currently sequential for simplicity)
                #( #calls )*
            }
        }
    }
}

// ------------------ controller! macro ------------------

struct ControllerInput {
    name: Ident,
    graphs: ExprArray,
}

impl Parse for ControllerInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let name_label: Ident = input.parse()?;
        if name_label != "name" {
            return Err(input.error("expected `name`"));
        }
        input.parse::<Token![:]>()?;
        let name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;

        let graphs_label: Ident = input.parse()?;
        if graphs_label != "graphs" {
            return Err(input.error("expected `graphs`"));
        }
        input.parse::<Token![:]>()?;
        let graphs: ExprArray = input.parse()?;

        Ok(ControllerInput { name, graphs })
    }
}

#[proc_macro]
pub fn controller(input: TokenStream) -> TokenStream {
    let ControllerInput { name, graphs } = parse_macro_input!(input as ControllerInput);

    let graph_calls: Vec<_> = graphs.elems.iter().map(|g| quote! { #g::run(&mut ctx); }).collect();

    let expanded = quote! {
        pub struct #name;

        impl #name {
            pub fn run() {
                let mut ctx = crate::node::Context::default();
                #( #graph_calls )*
            }
        }
    };

    TokenStream::from(expanded)
}