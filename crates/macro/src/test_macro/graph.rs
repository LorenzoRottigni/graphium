//! Main expansion logic for the `graph_test!` procedural macro.
//!
//! This module contains the top-level `expand` function that processes test
//! suites and synthesizes UI-bindable test markers.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Item};

use super::util::{next_suite_id, synthesize_ui_test_case};

/// Expands `graph_test! { ... }` by forwarding standard Rust test items and
/// generating marker types that can be referenced from `graph!` via `#[tests(...)]`.
pub fn expand(input: TokenStream) -> TokenStream {
    let mut file = parse_macro_input!(input as syn::File);
    let mut out_items: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut synthesized_marker_tokens: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut synthesized_marker_idents: Vec<syn::Ident> = Vec::new();

    for item in &mut file.items {
        if let Item::Fn(item_fn) = item {
            let is_test = item_fn.attrs.iter().any(|a| a.path().is_ident("test"));
            if is_test {
                match synthesize_ui_test_case(item_fn.clone()) {
                    Ok(bits) => {
                        out_items.push(bits.wrapper_tokens);
                        out_items.push(bits.case_tokens);
                        synthesized_marker_tokens.push(bits.marker_bits.marker_tokens);
                        synthesized_marker_idents.push(bits.marker_bits.marker_ident);
                    }
                    Err(err) => {
                        out_items.push(err.to_compile_error());
                        out_items.push(quote! { #item_fn });
                    }
                }
            } else {
                out_items.push(quote! { #item_fn });
            }
        } else {
            out_items.push(quote! { #item });
        }
    }

    let module_id = next_suite_id();
    let module_name = format_ident!("__graphium_graph_test_suite_{module_id}");

    TokenStream::from(quote! {
        mod #module_name {
            use super::*;
            #( #out_items )*
            #( #synthesized_marker_tokens )*
        }
        #( #[cfg(feature = "serialize")] pub use #module_name::#synthesized_marker_idents; )*
    })
}
