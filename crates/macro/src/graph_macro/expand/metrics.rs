use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::shared::MetricsSpec;

pub(super) fn wrap_sync_graph_body(run_body: &TokenStream, metrics: &MetricsSpec) -> TokenStream {
    if !metrics.enabled() {
        return run_body.clone();
    }

    if metrics.track_panics_sync() {
        quote! {{
            #[cfg(feature = "metrics")]
            {
                let __graphium_metrics = Self::__graphium_graph_metrics();
                let __graphium_start = __graphium_metrics.start_timer();
                let __graphium_result =
                    ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| #run_body));
                match __graphium_result {
                    Ok(value) => {
                        __graphium_metrics.record_success(__graphium_start);
                        value
                    }
                    Err(payload) => {
                        __graphium_metrics.record_failure(__graphium_start);
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
                let __graphium_metrics = Self::__graphium_graph_metrics();
                let __graphium_start = __graphium_metrics.start_timer();
                let value = #run_body;
                __graphium_metrics.record_success(__graphium_start);
                value
            }

            #[cfg(not(feature = "metrics"))]
            {
                #run_body
            }
        }}
    }
}

pub(super) fn wrap_async_graph_body(run_body_async: &TokenStream, metrics_enabled: bool) -> TokenStream {
    if !metrics_enabled {
        return run_body_async.clone();
    }

    quote! {{
        #[cfg(feature = "metrics")]
        {
            let __graphium_metrics = Self::__graphium_graph_metrics();
            let __graphium_start = __graphium_metrics.start_timer();
            let value = #run_body_async;
            __graphium_metrics.record_success(__graphium_start);
            value
        }

        #[cfg(not(feature = "metrics"))]
        {
            #run_body_async
        }
    }}
}

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
            fn __graphium_graph_metrics() -> &'static ::graphium::metrics::GraphMetricsHandle {
                static METRICS: ::std::sync::OnceLock<::graphium::metrics::GraphMetricsHandle> =
                    ::std::sync::OnceLock::new();
                METRICS.get_or_init(|| {
                    ::graphium::metrics::graph_metrics(
                        stringify!(#name),
                        module_path!(),
                        #metrics_config_tokens,
                    )
                })
            }
        }
    }
}

pub(super) fn metric_config_tokens(metrics: &MetricsSpec) -> TokenStream {
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
}

