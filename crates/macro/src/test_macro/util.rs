use std::sync::atomic::{AtomicUsize, Ordering};

use quote::{format_ident, quote};
use syn::ItemFn;

use crate::shared::pascal_case;

static NEXT_SUITE_ID: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn next_suite_id() -> usize {
    NEXT_SUITE_ID.fetch_add(1, Ordering::Relaxed)
}

pub(crate) struct UiTestMarkerBits {
    pub(crate) marker_ident: syn::Ident,
    pub(crate) marker_tokens: proc_macro2::TokenStream,
}

pub(crate) fn synthesize_ui_test_marker(item_fn: &ItemFn) -> UiTestMarkerBits {
    let fn_ident = item_fn.sig.ident.clone();
    let marker_ident = format_ident!("{}", pascal_case(&fn_ident));
    let case_ident = format_ident!("__graphium_ui_case_{}", fn_ident);

    let supported = item_fn.sig.inputs.is_empty() && item_fn.sig.asyncness.is_none();
    let unsupported_reason = if !item_fn.sig.inputs.is_empty() {
        "graphium UI test runner does not support test functions with parameters"
    } else {
        "graphium UI test runner does not support async test functions yet"
    };

    let case_fn = if supported {
        let body = item_fn.block.clone();
        quote! {
            #[cfg(feature = "serialize")]
            fn #case_ident() #body
        }
    } else {
        quote! {}
    };

    let run_body = if supported {
        quote! {
            match ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| #case_ident())) {
                Ok(()) => Ok(()),
                Err(payload) => Err(::graphium::export::panic_payload_to_string(payload)),
            }
        }
    } else {
        let reason_lit = syn::LitStr::new(unsupported_reason, proc_macro2::Span::call_site());
        quote! { Err(#reason_lit.to_string()) }
    };

    let marker_tokens = quote! {
        #case_fn

        #[cfg(feature = "serialize")]
        pub struct #marker_ident;

        #[cfg(feature = "serialize")]
        impl #marker_ident {
            pub const NAME: &'static str = concat!(module_path!(), "::", stringify!(#fn_ident));

            pub fn __graphium_ui_run() -> ::std::result::Result<(), ::std::string::String> {
                #run_body
            }
        }
    };

    UiTestMarkerBits {
        marker_ident,
        marker_tokens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn synthesize_ui_test_marker_generates_marker_type() {
        let item: ItemFn = parse_quote! {
            fn owned_graph_returns_non_zero_split() {}
        };
        let bits = synthesize_ui_test_marker(&item);
        assert_eq!(bits.marker_ident.to_string(), "OwnedGraphReturnsNonZeroSplit");
        let s = bits.marker_tokens.to_string();
        assert!(s.contains("panic_payload_to_string"));
        assert!(s.contains("NAME"));
    }
}
