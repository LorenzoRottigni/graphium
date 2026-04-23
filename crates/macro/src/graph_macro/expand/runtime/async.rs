//! Async graph execution implementation generator.
//!
//! This module emits the `run_async` method for graph types that support
//! asynchronous execution. The generated method is always present, because the
//! graph type may still be used in async form even when sync execution exists.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

/// Generate the `run_async` method for a graph type.
///
/// The method takes a mutable graph context and the declared graph inputs,
/// and returns the configured graph outputs in the generated return type.
pub fn build_async_impl(
    name: &Ident,
    context: &syn::Path,
    run_params: &[TokenStream],
    run_return_sig: &TokenStream,
    async_graph_body: &TokenStream,
) -> TokenStream {
    quote! {
        impl #name {
            pub async fn run_async(
                ctx: &mut #context,
                #( #run_params ),*
            ) #run_return_sig {
                #async_graph_body
            }
        }
    }
}

