use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

pub(super) fn build_sync_impl(
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

