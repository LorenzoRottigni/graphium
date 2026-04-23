//! Sync graph execution implementation generator.
//!
//! This module emits the `run` method only for graphs that are not marked
//! as async-enabled. Async graphs omit the sync runner to avoid duplicate
//! execution APIs.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

/// Generate the synchronous `run` method for a non-async graph.
///
/// When the graph is async-enabled, no synchronous runner is emitted.
pub fn build_sync_impl(
    name: &Ident,
    context: &syn::Path,
    async_enabled: bool,
    run_params: &[TokenStream],
    run_return_sig: &TokenStream,
    sync_graph_body: &TokenStream,
) -> TokenStream {
    if async_enabled {
        return quote! {};
    }

    quote! {
        impl #name {
            pub fn run(
                ctx: &mut #context,
                #( #run_params ),*
            ) #run_return_sig {
                #sync_graph_body
            }
        }
    }
}
