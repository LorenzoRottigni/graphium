use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, parse::{Parse, ParseStream}, Ident, Result, Token, ItemFn, ExprArray,
};

// ------------------ node! macro ------------------
#[proc_macro]
pub fn node(input: TokenStream) -> TokenStream {
    let func = parse_macro_input!(input as ItemFn);
    // Just output the function as-is
    TokenStream::from(quote! { #func })
}

// ------------------ graph! macro ------------------

struct GraphInput {
    name: Ident,
    nodes: ExprArray,
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
        let nodes: ExprArray = input.parse()?;

        Ok(GraphInput { name, nodes })
    }
}

#[proc_macro]
pub fn graph(input: TokenStream) -> TokenStream {
    let GraphInput { name, nodes } = parse_macro_input!(input as GraphInput);

    // Extract the identifiers from the ExprArray
    let node_idents: Vec<_> = nodes.elems.iter().map(|expr| quote! { #expr }).collect();

    let expanded = quote! {
        pub mod #name {
            use crate::node::*;
            pub fn run(ctx: &mut Context) {
                #( #node_idents(ctx); )*
            }
        }
    };
    TokenStream::from(expanded)
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