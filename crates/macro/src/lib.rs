use proc_macro::TokenStream;

mod graph;
mod node;
mod parser;
mod shared;

#[proc_macro]
pub fn node(input: TokenStream) -> TokenStream {
    node::expand(input)
}

#[proc_macro]
pub fn graph(input: TokenStream) -> TokenStream {
    graph::expand(input)
}
