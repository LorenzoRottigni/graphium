use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

pub(super) fn build_async_impl(
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

