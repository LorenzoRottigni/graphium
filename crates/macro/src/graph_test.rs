use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

/// Expands `graph_test! { ... }` by forwarding standard Rust test items.
///
/// This mirrors `node_test!` and keeps graph test suites fully idiomatic:
/// callers write regular Rust tests and attributes, grouped by intent.
pub fn expand(input: TokenStream) -> TokenStream {
    let file = parse_macro_input!(input as syn::File);
    let items = file.items;
    TokenStream::from(quote! { #( #items )* })
}
