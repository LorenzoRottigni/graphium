use std::sync::atomic::{AtomicUsize, Ordering};

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, Item, ItemFn, parse_macro_input};

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

    let module_id = NEXT_SUITE_ID.fetch_add(1, Ordering::Relaxed);
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

fn synthesize_registry_bits(
    item_fn: &mut ItemFn,
    attr_name: &str,
    kind_tokens: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let target = strip_and_parse_target_attr(item_fn, attr_name);
    let Some(target) = target else {
        return quote! {};
    };

    if !item_fn.sig.inputs.is_empty() {
        panic!("graphium test functions with `#[for_graph(...)]` cannot take parameters");
    }
    if item_fn.sig.asyncness.is_some() {
        panic!("graphium runtime UI test registration does not support async test functions yet");
    }

    let fn_ident = item_fn.sig.ident.clone();
    let runtime_fn_ident = format_ident!("__graphium_runtime_graph_case_{}", fn_ident);
    let runner_ident = format_ident!("__graphium_runtime_graph_runner_{}", fn_ident);

    let body = item_fn.block.clone();

    quote! {
        fn #runtime_fn_ident() {
            #body
        }

        fn #runner_ident() -> ::std::result::Result<(), ::std::string::String> {
            match ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| #runtime_fn_ident())) {
                Ok(()) => Ok(()),
                Err(payload) => Err(::graphium::test_registry::panic_payload_to_string(payload)),
            }
        }

        ::graphium::inventory::submit! {
            ::graphium::test_registry::RegisteredTest {
                name: concat!(module_path!(), "::", stringify!(#fn_ident)),
                target: stringify!(#target),
                kind: #kind_tokens,
                run: #runner_ident,
            }
        }
    }
}

fn strip_and_parse_target_attr(item_fn: &mut ItemFn, attr_name: &str) -> Option<syn::Path> {
    let attrs = std::mem::take(&mut item_fn.attrs);
    let mut filtered: Vec<Attribute> = Vec::with_capacity(attrs.len());
    let mut target: Option<syn::Path> = None;

    for attr in attrs {
        if attr.path().is_ident(attr_name) {
            let parsed = attr.parse_args::<syn::Path>().unwrap_or_else(|_| {
                panic!("`#[{attr_name}(...)]` expects a type path, e.g. `#[{attr_name}(MyType)]`")
            });
            if target.is_some() {
                panic!("`#[{attr_name}(...)]` can be specified at most once per test function");
            }
            target = Some(parsed);
            continue;
        }
        filtered.push(attr);
    }

    item_fn.attrs = filtered;
    target
}

static NEXT_SUITE_ID: AtomicUsize = AtomicUsize::new(0);
