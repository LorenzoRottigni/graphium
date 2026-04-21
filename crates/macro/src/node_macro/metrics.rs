//! Metrics extraction and configuration for the `node!` procedural macro.
//!
//! This module handles parsing `#[metrics(...)]` attributes from node functions
//! and generating the corresponding runtime metric configuration tokens.

use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{Attribute, LitStr, Token};

use crate::shared::{MetricsSpec, parse_metric_name};

/// Extracts `#[metrics(...)]` attributes from a node function's attribute list.
///
/// This function processes the attributes in-place, removing any `#[metrics(...)]`
/// entries and collecting their values into a `MetricsSpec` struct.
///
/// # Attributes
///
/// Supported metric names:
/// - `"performance"` - Track execution time
/// - `"errors"` - Track error count
/// - `"count"` - Track invocation count
/// - `"caller"` - Track caller information
/// - `"success_rate"` - Track success/failure ratio
/// - `"fail_rate"` - Track failure ratio
///
/// # Panics
///
/// Panics if:
/// - The attribute format is invalid
/// - An unknown metric name is used
pub fn extract_metrics_from_attrs(attrs: &mut Vec<Attribute>) -> MetricsSpec {
    let mut metrics = MetricsSpec::default();
    attrs.retain(|attr| {
        if !attr.path().is_ident("metrics") {
            return true;
        }

        let parser = Punctuated::<LitStr, Token![,]>::parse_terminated;
        let values = attr
            .parse_args_with(parser)
            .unwrap_or_else(|_| panic!("failed to parse #[metrics(...)] list"));

        for value in values {
            let apply = parse_metric_name(value.value().as_str()).unwrap_or_else(|| {
                panic!(
                    "unsupported metric `{}`; allowed: performance, errors, count, caller, success_rate, fail_rate",
                    value.value()
                )
            });
            apply(&mut metrics);
        }

        false
    });
    metrics
}

/// Generates the `MetricConfig` token stream for a given `MetricsSpec`.
///
/// This produces the Rust code that will be used at runtime to configure
/// the node's metrics collection.
pub fn metric_config_tokens(metrics: MetricsSpec) -> TokenStream {
    let performance = metrics.performance;
    let errors = metrics.errors;
    let count = metrics.count;
    let caller = metrics.caller;
    let success_rate = metrics.success_rate;
    let fail_rate = metrics.fail_rate;

    quote! {
        ::graphium::metrics::MetricConfig {
            performance: #performance,
            errors: #errors,
            count: #count,
            caller: #caller,
            success_rate: #success_rate,
            fail_rate: #fail_rate,
        }
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn parse_attrs(input: proc_macro2::TokenStream) -> Vec<Attribute> {
        let item: syn::ItemFn = syn::parse2(input).unwrap();
        item.attrs
    }

    #[test]
    fn extract_metrics_returns_default_for_no_attrs() {
        let mut attrs: Vec<Attribute> = vec![];
        let metrics = extract_metrics_from_attrs(&mut attrs);
        assert!(!metrics.performance);
        assert!(!metrics.errors);
        assert!(!metrics.count);
    }

    #[test]
    fn extract_metrics_preserves_non_metrics_attrs() {
        let attrs_input = quote! {
            #[derive(Debug)]
            #[other_attr]
            fn my_node() {}
        };
        let mut attrs = parse_attrs(attrs_input);
        let _ = extract_metrics_from_attrs(&mut attrs);
        assert_eq!(attrs.len(), 2);
        assert!(attrs.iter().all(|a| !a.path().is_ident("metrics")));
    }

    #[test]
    fn extract_metrics_removes_metrics_attrs() {
        let attrs_input = quote! {
            #[metrics("performance", "errors")]
            fn my_node() {}
        };
        let mut attrs = parse_attrs(attrs_input);
        let metrics = extract_metrics_from_attrs(&mut attrs);
        assert!(attrs.is_empty());
        assert!(metrics.performance);
        assert!(metrics.errors);
    }

    #[test]
    fn extract_metrics_parses_performance() {
        let attrs_input = quote! {
            #[metrics("performance")]
            fn my_node() {}
        };
        let mut attrs = parse_attrs(attrs_input);
        let metrics = extract_metrics_from_attrs(&mut attrs);
        assert!(metrics.performance);
    }

    #[test]
    fn extract_metrics_parses_errors() {
        let attrs_input = quote! {
            #[metrics("errors")]
            fn my_node() {}
        };
        let mut attrs = parse_attrs(attrs_input);
        let metrics = extract_metrics_from_attrs(&mut attrs);
        assert!(metrics.errors);
    }

    #[test]
    fn extract_metrics_parses_count() {
        let attrs_input = quote! {
            #[metrics("count")]
            fn my_node() {}
        };
        let mut attrs = parse_attrs(attrs_input);
        let metrics = extract_metrics_from_attrs(&mut attrs);
        assert!(metrics.count);
    }

    #[test]
    fn extract_metrics_parses_caller() {
        let attrs_input = quote! {
            #[metrics("caller")]
            fn my_node() {}
        };
        let mut attrs = parse_attrs(attrs_input);
        let metrics = extract_metrics_from_attrs(&mut attrs);
        assert!(metrics.caller);
    }

    #[test]
    fn extract_metrics_parses_success_rate() {
        let attrs_input = quote! {
            #[metrics("success_rate")]
            fn my_node() {}
        };
        let mut attrs = parse_attrs(attrs_input);
        let metrics = extract_metrics_from_attrs(&mut attrs);
        assert!(metrics.success_rate);
    }

    #[test]
    fn extract_metrics_parses_fail_rate() {
        let attrs_input = quote! {
            #[metrics("fail_rate")]
            fn my_node() {}
        };
        let mut attrs = parse_attrs(attrs_input);
        let metrics = extract_metrics_from_attrs(&mut attrs);
        assert!(metrics.fail_rate);
    }

    #[test]
    fn extract_metrics_parses_multiple_metrics() {
        let attrs_input = quote! {
            #[metrics("performance", "errors", "count")]
            fn my_node() {}
        };
        let mut attrs = parse_attrs(attrs_input);
        let metrics = extract_metrics_from_attrs(&mut attrs);
        assert!(metrics.performance);
        assert!(metrics.errors);
        assert!(metrics.count);
    }

    #[test]
    #[should_panic(expected = "unsupported metric")]
    fn extract_metrics_rejects_unknown_metric() {
        let attrs_input = quote! {
            #[metrics("unknown_metric")]
            fn my_node() {}
        };
        let mut attrs = parse_attrs(attrs_input);
        extract_metrics_from_attrs(&mut attrs);
    }

    #[test]
    fn metric_config_tokens_generates_valid_config() {
        let metrics = MetricsSpec {
            performance: true,
            errors: true,
            count: false,
            caller: false,
            success_rate: true,
            fail_rate: false,
        };
        let tokens = metric_config_tokens(metrics);
        let expanded = tokens.to_string();
        assert!(expanded.contains("performance"));
        assert!(expanded.contains("errors"));
        assert!(expanded.contains("count"));
        assert!(expanded.contains("success_rate"));
        assert!(expanded.contains("fail_rate"));
        assert!(expanded.contains("graphium"));
        assert!(expanded.contains("MetricConfig"));
    }
}
