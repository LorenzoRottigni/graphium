//! Main expansion logic for the `node_test!` procedural macro.
//!
//! This module contains the top-level `expand` function that processes test
//! suites and synthesizes UI-bindable test markers.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Item};

use super::util::{next_suite_id, synthesize_ui_test_marker};

/// Expands `node_test! { ... }` by forwarding standard Rust test items and
/// generating marker types that can be referenced from `node!` via `#[tests(...)]`.
pub fn expand(input: TokenStream) -> TokenStream {
    let mut file = parse_macro_input!(input as syn::File);
    let mut synthesized_marker_tokens = Vec::new();
    let mut synthesized_marker_idents = Vec::new();

    for item in &mut file.items {
        if let Item::Fn(item_fn) = item {
            let bits = synthesize_ui_test_marker(item_fn);
            synthesized_marker_tokens.push(bits.marker_tokens);
            synthesized_marker_idents.push(bits.marker_ident);
        }
    }

    let module_id = next_suite_id();
    let module_name = format_ident!("__graphium_node_test_suite_{module_id}");
    let items = file.items;

    TokenStream::from(quote! {
        mod #module_name {
            use super::*;
            #( #items )*
            #( #synthesized_marker_tokens )*
        }
        #( #[cfg(feature = "serialize")] pub use #module_name::#synthesized_marker_idents; )*
    })
}
