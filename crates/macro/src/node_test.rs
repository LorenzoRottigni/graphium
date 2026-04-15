use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

/// Expands `node_test! { ... }` by forwarding standard Rust test items.
///
/// The macro intentionally stays thin and idiomatic: users write normal
/// `#[test]` (or runtime-specific test attributes) and this macro just groups
/// node-scoped tests in one block.
pub fn expand(input: TokenStream) -> TokenStream {
    let file = parse_macro_input!(input as syn::File);
    let items = file.items;
    TokenStream::from(quote! { #( #items )* })
}
