//! Shared utilities for test macro expansion.
//!
//! This module contains common code used by both `graph_test!` and `node_test!`
//! macros for synthesizing runtime test registry entries.

use std::sync::atomic::{AtomicUsize, Ordering};

use quote::{format_ident, quote};
use syn::{Attribute, ItemFn};

/// Counter for generating unique test suite module names.
static NEXT_SUITE_ID: AtomicUsize = AtomicUsize::new(0);

/// Generates the next unique test suite ID.
pub fn next_suite_id() -> usize {
    NEXT_SUITE_ID.fetch_add(1, Ordering::Relaxed)
}

/// Synthesizes the runtime registry bits for a test function.
///
/// This function:
///
/// 1. Extracts the `#[for_graph(...)]` or `#[for_node(...)]` target attribute
/// 2. Validates the test function signature (no params, no async)
/// 3. Creates wrapper functions for panic-safe execution
/// 4. Generates an inventory submission for runtime test discovery
pub fn synthesize_registry_bits(
    item_fn: &mut ItemFn,
    attr_name: &str,
    kind_tokens: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let target = strip_and_parse_target_attr(item_fn, attr_name);
    let Some(target) = target else {
        return quote! {};
    };

    if !item_fn.sig.inputs.is_empty() {
        panic!("graphium test functions with `#[{attr_name}(...)]` cannot take parameters");
    }
    if item_fn.sig.asyncness.is_some() {
        panic!("graphium runtime UI test registration does not support async test functions yet");
    }

    let fn_ident = item_fn.sig.ident.clone();
    let case_ident = format_ident!("__graphium_runtime_case_{}", fn_ident);
    let runner_ident = format_ident!("__graphium_runtime_runner_{}", fn_ident);

    let body = item_fn.block.clone();

    quote! {
        fn #case_ident() {
            #body
        }

        fn #runner_ident() -> ::std::result::Result<(), ::std::string::String> {
            match ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| #case_ident())) {
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

/// Extracts and removes a target attribute from a test function.
///
/// Parses the attribute path as a type and returns it, while filtering out
/// the attribute from the function's attribute list.
///
/// # Panics
///
/// Panics if:
/// - The attribute argument is not a valid type path
/// - The attribute appears more than once
pub fn strip_and_parse_target_attr(item_fn: &mut ItemFn, attr_name: &str) -> Option<syn::Path> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    use syn::parse_quote;

    #[test]
    fn synthesize_registry_bits_returns_empty_when_no_target() {
        let mut func: ItemFn = parse_quote! {
            fn my_test() {}
        };
        let result = synthesize_registry_bits(
            &mut func,
            "for_graph",
            quote! { ::graphium::test_registry::TestKind::Graph },
        );
        assert!(result.is_empty());
    }

    #[test]
    fn synthesize_registry_bits_generates_inventory_submit() {
        let mut func: ItemFn = parse_quote! {
            #[for_graph(MyGraph)]
            fn my_test() {}
        };
        let result = synthesize_registry_bits(
            &mut func,
            "for_graph",
            quote! { ::graphium::test_registry::TestKind::Graph },
        );
        let result_str = result.to_string();
        assert!(result_str.contains("inventory"));
        assert!(result_str.contains("RegisteredTest"));
        assert!(result_str.contains("graphium"));
    }

    #[test]
    #[should_panic(expected = "cannot take parameters")]
    fn synthesize_registry_bits_rejects_params() {
        let mut func: ItemFn = parse_quote! {
            #[for_graph(MyGraph)]
            fn my_test(x: i32) {}
        };
        synthesize_registry_bits(
            &mut func,
            "for_graph",
            quote! { ::graphium::test_registry::TestKind::Graph },
        );
    }

    #[test]
    #[should_panic(expected = "does not support async")]
    fn synthesize_registry_bits_rejects_async() {
        let mut func: ItemFn = parse_quote! {
            #[for_graph(MyGraph)]
            async fn my_test() {}
        };
        synthesize_registry_bits(
            &mut func,
            "for_graph",
            quote! { ::graphium::test_registry::TestKind::Graph },
        );
    }

    #[test]
    fn strip_and_parse_target_attr_extracts_path() {
        let mut func: ItemFn = parse_quote! {
            #[for_graph(MyGraph)]
            #[other_attr]
            fn my_test() {}
        };
        let target = strip_and_parse_target_attr(&mut func, "for_graph");
        assert!(target.is_some());
        let path = target.unwrap();
        assert_eq!(path.segments.last().unwrap().ident, "MyGraph");
        assert_eq!(func.attrs.len(), 1);
    }

    #[test]
    fn strip_and_parse_target_attr_returns_none_when_missing() {
        let mut func: ItemFn = parse_quote! {
            #[other_attr]
            fn my_test() {}
        };
        let target = strip_and_parse_target_attr(&mut func, "for_graph");
        assert!(target.is_none());
        assert_eq!(func.attrs.len(), 1);
    }

    #[test]
    #[should_panic(expected = "expects a type path")]
    fn strip_and_parse_target_attr_rejects_invalid_path() {
        let mut func: ItemFn = parse_quote! {
            #[for_graph("not a path")]
            fn my_test() {}
        };
        strip_and_parse_target_attr(&mut func, "for_graph");
    }

    #[test]
    #[should_panic(expected = "can be specified at most once")]
    fn strip_and_parse_target_attr_rejects_duplicates() {
        let mut func: ItemFn = parse_quote! {
            #[for_graph(Graph1)]
            #[for_graph(Graph2)]
            fn my_test() {}
        };
        strip_and_parse_target_attr(&mut func, "for_graph");
    }

    #[test]
    fn next_suite_id_increments() {
        let id1 = next_suite_id();
        let id2 = next_suite_id();
        assert!(id2 > id1);
    }
}
