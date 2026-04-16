//! Main expansion logic for the `graph_test!` procedural macro.
//!
//! This module contains the top-level `expand` function that processes test
//! suites and optionally registers runtime-discoverable UI tests.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Item};

use super::registry::{next_suite_id, synthesize_registry_bits};

/// Expands `graph_test! { ... }` by forwarding standard Rust test items and
/// optionally registering runtime-discoverable UI tests through `#[for_graph(...)]`.
pub fn expand(input: TokenStream) -> TokenStream {
    let mut file = parse_macro_input!(input as syn::File);
    let mut synthesized = Vec::new();

    for item in &mut file.items {
        if let Item::Fn(item_fn) = item {
            synthesized.push(synthesize_registry_bits(
                item_fn,
                "for_graph",
                quote! { ::graphium::test_registry::TestKind::Graph },
            ));
        }
    }

    let module_id = next_suite_id();
    let module_name = format_ident!("__graphium_graph_test_suite_{module_id}");
    let items = file.items;

    TokenStream::from(quote! {
        mod #module_name {
            use super::*;
            #( #items )*
            #( #synthesized )*
        }
    })
}
