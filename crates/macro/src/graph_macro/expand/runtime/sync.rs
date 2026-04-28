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
    run_args: &[Ident],
    run_return_sig: &TokenStream,
    sync_graph_body: &TokenStream,
) -> TokenStream {
    if async_enabled {
        return quote! {};
    }

    let is_graphium_context = context.leading_colon.is_some()
        && context.segments.len() == 2
        && context.segments[0].ident == "graphium"
        && context.segments[1].ident == "Context";

    let default_runner = if is_graphium_context {
        quote! {
            /// Convenience runner that builds a default `graphium::Context` internally.
            pub fn run_default(
                #( #run_params ),*
            ) #run_return_sig {
                let mut ctx: #context = ::core::default::Default::default();
                Self::run(&mut ctx, #( #run_args ),* )
            }
        }
    } else {
        quote! {}
    };

    quote! {
        impl #name {
            pub fn run(
                ctx: &mut #context,
                #( #run_params ),*
            ) #run_return_sig {
                #[cfg(any(feature = "metrics", feature = "trace", feature = "logs"))]
                let __graphium_telemetry = ::graphium::GraphiumTelemetry::global();
                #[cfg(feature = "trace")]
                let _ = __graphium_telemetry.graph_span(stringify!(#name)).entered();

                #[cfg(feature = "logs")]
                ::graphium::telemetry::tracing::info!(graph = stringify!(#name), "graph started");

                let __graphium_result = #sync_graph_body;

                #[cfg(feature = "logs")]
                ::graphium::telemetry::tracing::info!(graph = stringify!(#name), "graph finished");

                __graphium_result
            }
            #default_runner
        }
    }
}
