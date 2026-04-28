//! Helpers for generating optional graph metrics instrumentation.
//!
//! This module creates the generated code that wraps graph execution with
//! metrics tracking and exposes a per-graph metrics handle when enabled.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::ir::MetricsSpec;

/// Wrap synchronous graph execution in metrics instrumentation when enabled.
///
/// This helper only changes the generated body when metrics are enabled.
/// It records success/failure and optionally tracks panic propagation.
pub(super) fn wrap_sync_graph_body(run_body: &TokenStream, metrics: &MetricsSpec) -> TokenStream {
    if !metrics.enabled() {
        return run_body.clone();
    }

    if metrics.track_panics_sync() {
        quote! {{
            #[cfg(feature = "metrics")]
            {
                let __graphium_telemetry = Self::__graphium_graph_telemetry();
                let __graphium_start = __graphium_telemetry.start_timer();
                let __graphium_result =
                    ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| #run_body));
                match __graphium_result {
                    Ok(value) => {
                        __graphium_telemetry.record_success(__graphium_start);
                        value
                    }
                    Err(payload) => {
                        __graphium_telemetry.record_failure(__graphium_start);
                        ::std::panic::resume_unwind(payload)
                    }
                }
            }

            #[cfg(not(feature = "metrics"))]
            {
                #run_body
            }
        }}
    } else {
        quote! {{
            #[cfg(feature = "metrics")]
            {
                let __graphium_telemetry = Self::__graphium_graph_telemetry();
                let __graphium_start = __graphium_telemetry.start_timer();
                let value = #run_body;
                __graphium_telemetry.record_success(__graphium_start);
                value
            }

            #[cfg(not(feature = "metrics"))]
            {
                #run_body
            }
        }}
    }
}

/// Wrap asynchronous graph execution with metrics instrumentation.
///
/// Async graphs do not support panic tracking in the same way as sync graphs,
/// but they still record success metrics if enabled.
pub(super) fn wrap_async_graph_body(
    run_body_async: &TokenStream,
    metrics_enabled: bool,
) -> TokenStream {
    if !metrics_enabled {
        return run_body_async.clone();
    }

    quote! {{
        #[cfg(feature = "metrics")]
        {
            let __graphium_telemetry = Self::__graphium_graph_telemetry();
            let __graphium_start = __graphium_telemetry.start_timer();
            let value = #run_body_async;
            __graphium_telemetry.record_success(__graphium_start);
            value
        }

        #[cfg(not(feature = "metrics"))]
        {
            #run_body_async
        }
    }}
}

/// Emit the compile-time metrics helper implementation for a graph.
///
/// This creates a lazily initialized static metrics handle that can be reused
/// across graph executions and ensures instrumentation is only compiled in
/// when `#[cfg(feature = "metrics")]` is active.
pub(super) fn build_metrics_impl(
    name: &Ident,
    metrics_enabled: bool,
    metrics_config_tokens: &TokenStream,
) -> TokenStream {
    if !metrics_enabled {
        return quote! {};
    }

    quote! {
        #[cfg(feature = "metrics")]
        impl #name {
            fn __graphium_graph_telemetry() -> &'static ::graphium::telemetry::GraphTelemetryHandle {
                static TELEMETRY: ::std::sync::OnceLock<::graphium::telemetry::GraphTelemetryHandle> =
                    ::std::sync::OnceLock::new();
                TELEMETRY.get_or_init(|| {
                    ::graphium::GraphiumTelemetry::global().graph_metrics(
                        stringify!(#name),
                        module_path!(),
                        #metrics_config_tokens,
                    )
                })
            }
        }
    }
}

/// Build a `MetricConfig` literal from the parsed metrics specification.
///
/// This token stream is used by the generated metrics helper to configure
/// what counters and tracking modes are active for the graph.
pub(super) fn metric_config_tokens(metrics: &MetricsSpec) -> TokenStream {
    let performance = metrics.performance;
    let errors = metrics.errors;
    let count = metrics.count;
    let caller = metrics.caller;
    let success_rate = metrics.success_rate;
    let fail_rate = metrics.fail_rate;

    quote! {
        ::graphium::telemetry::MetricConfig {
            performance: #performance,
            errors: #errors,
            count: #count,
            caller: #caller,
            success_rate: #success_rate,
            fail_rate: #fail_rate,
        }
    }
}
