use std::sync::atomic::{AtomicUsize, Ordering};

use quote::{format_ident, quote};
use syn::{parse_quote, Attribute, Expr, FnArg, Ident, ItemFn, Pat, PatIdent, ReturnType, Type};

use crate::shared::pascal_case;

static NEXT_SUITE_ID: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn next_suite_id() -> usize {
    NEXT_SUITE_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GraphiumDefaults {
    pub(crate) values: std::collections::HashMap<String, Expr>,
}

pub(crate) fn extract_graphium_defaults(
    attrs: &mut Vec<Attribute>,
) -> syn::Result<GraphiumDefaults> {
    let mut out = GraphiumDefaults::default();
    let mut keep = Vec::with_capacity(attrs.len());

    for attr in attrs.drain(..) {
        if !attr.path().is_ident("graphium") {
            keep.push(attr);
            continue;
        }

        // Parse: #[graphium(defaults(threshold = 0, name = "x"))]
        let mut values = std::collections::HashMap::<String, Expr>::new();
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("defaults") {
                meta.parse_nested_meta(|nested| {
                    let Some(ident) = nested.path.get_ident().cloned() else {
                        return Err(nested.error("expected identifier parameter name"));
                    };
                    nested.input.parse::<syn::Token![=]>()?;
                    let expr: Expr = nested.input.parse()?;
                    values.insert(ident.to_string(), expr);
                    Ok(())
                })?;
            }
            Ok(())
        })?;

        out.values.extend(values);
    }

    *attrs = keep;
    Ok(out)
}

pub(crate) fn cfg_attrs(attrs: &[Attribute]) -> Vec<Attribute> {
    attrs.iter()
        .cloned()
        .filter(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
        .collect()
}

pub(crate) struct UiTestMarkerBits {
    pub(crate) marker_ident: syn::Ident,
    pub(crate) marker_tokens: proc_macro2::TokenStream,
}

pub(crate) struct UiTestCaseBits {
    pub(crate) wrapper_tokens: proc_macro2::TokenStream,
    pub(crate) case_tokens: proc_macro2::TokenStream,
    pub(crate) marker_bits: UiTestMarkerBits,
}

fn is_injected_ref_param(arg: &FnArg) -> Option<(Ident, Type, bool)> {
    // Recognize `graph: &T` / `graph: &mut T` or `node: &T` / `node: &mut T` as injected.
    let FnArg::Typed(pat_type) = arg else {
        return None;
    };
    let Pat::Ident(PatIdent { ident, .. }) = &*pat_type.pat else {
        return None;
    };
    if ident != "graph" && ident != "node" {
        return None;
    }
    let Type::Reference(reference) = &*pat_type.ty else {
        return None;
    };
    let inner = (*reference.elem).clone();
    Some((ident.clone(), inner, reference.mutability.is_some()))
}

fn test_param_kind(ty: &Type) -> proc_macro2::TokenStream {
    let Type::Path(type_path) = ty else {
        return quote! { ::graphium::export::TestParamKind::Text };
    };
    let ident = type_path
        .path
        .segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default();
    match ident.as_str() {
        "bool" => quote! { ::graphium::export::TestParamKind::Bool },
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64"
        | "i128" | "isize" | "f32" | "f64" => quote! { ::graphium::export::TestParamKind::Number },
        _ => quote! { ::graphium::export::TestParamKind::Text },
    }
}

fn is_string_type(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    type_path
        .path
        .segments
        .last()
        .is_some_and(|s| s.ident == "String")
}

fn is_bool_type(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    type_path.path.is_ident("bool")
        || type_path
            .path
            .segments
            .last()
            .is_some_and(|s| s.ident == "bool")
}

fn is_result_unit(output: &ReturnType) -> Option<Type> {
    let ReturnType::Type(_, ty) = output else {
        return None;
    };
    let Type::Path(type_path) = &**ty else {
        return None;
    };
    let last = type_path.path.segments.last()?;
    if last.ident != "Result" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };
    let mut iter = args.args.iter();
    let ok = iter.next()?;
    let err = iter.next()?;
    let ok_is_unit = matches!(ok, syn::GenericArgument::Type(Type::Tuple(t)) if t.elems.is_empty());
    if !ok_is_unit {
        return None;
    }
    let syn::GenericArgument::Type(err_ty) = err else {
        return None;
    };
    Some(err_ty.clone())
}

pub(crate) fn synthesize_ui_test_case(mut item_fn: ItemFn) -> syn::Result<UiTestCaseBits> {
    let fn_ident = item_fn.sig.ident.clone();
    let wrapper_ident = fn_ident.clone();
    let case_ident = format_ident!("__graphium_ui_case_{}", wrapper_ident);
    let marker_ident = format_ident!("{}", pascal_case(&wrapper_ident));

    let supported = item_fn.sig.asyncness.is_none();
    let unsupported_reason = "graphium UI test runner does not support async test functions yet";

    let mut attrs = item_fn.attrs.clone();
    let defaults = extract_graphium_defaults(&mut attrs)?;
    let cfg = cfg_attrs(&attrs);
    let wrapper_attrs: Vec<Attribute> = attrs;

    // Split inputs into optional injected ref param + UI params.
    let mut inputs_iter = item_fn.sig.inputs.iter();
    let injected = inputs_iter
        .next()
        .and_then(|arg| is_injected_ref_param(arg));

    let injected_arg = injected.as_ref().map(|(ident, inner_ty, is_mut)| {
        let var_ident = format_ident!("__graphium_injected_{}", ident);
        let let_tokens = if *is_mut {
            quote! { let mut #var_ident: #inner_ty = ::core::default::Default::default(); }
        } else {
            quote! { let #var_ident: #inner_ty = ::core::default::Default::default(); }
        };
        let pass_tokens = if *is_mut {
            quote! { &mut #var_ident }
        } else {
            quote! { &#var_ident }
        };
        (let_tokens, pass_tokens)
    });

    let mut ui_params: Vec<(Ident, Type, bool)> = Vec::new();
    let mut all_params: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut wrapper_prelude: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut marker_prelude: Vec<proc_macro2::TokenStream> = Vec::new();

    if let Some((let_tokens, _pass_tokens)) = injected_arg.as_ref() {
        wrapper_prelude.push(let_tokens.clone());
        marker_prelude.push(let_tokens.clone());
    }

    for arg in item_fn.sig.inputs.iter().skip(if injected.is_some() { 1 } else { 0 }) {
        let FnArg::Typed(pat_type) = arg else {
            return Err(syn::Error::new_spanned(
                arg,
                "methods with `self` are not supported in graph_test!/node_test!",
            ));
        };
        let Pat::Ident(pat_ident) = &*pat_type.pat else {
            return Err(syn::Error::new_spanned(
                &pat_type.pat,
                "only identifier parameters are supported (e.g. `threshold: u32`)",
            ));
        };
        let ident = pat_ident.ident.clone();
        let is_mut = pat_ident.mutability.is_some();
        let ty = (*pat_type.ty).clone();
        ui_params.push((ident.clone(), ty.clone(), is_mut));

        let default_expr = defaults
            .values
            .get(&ident.to_string())
            .cloned()
            .unwrap_or_else(|| parse_quote! { ::core::default::Default::default() });

        let maybe_mut = if is_mut { quote! { mut } } else { quote! {} };
        wrapper_prelude.push(quote! {
            let #maybe_mut #ident: #ty = #default_expr;
        });
        all_params.push(quote! { #ident });
    }

    // Build the wrapper call args (injected first).
    let mut wrapper_call_args: Vec<proc_macro2::TokenStream> = Vec::new();
    if let Some((_let_tokens, pass_tokens)) = injected_arg.as_ref() {
        wrapper_call_args.push(pass_tokens.clone());
    }
    wrapper_call_args.extend(all_params.clone());

    // If the first parameter is `graph: &T` / `node: &T`, inject a type alias (`type graph = T;`)
    // so the test body can call `graph::__graphium_run(...)` without adding methods to `T`.
    if let Some((ident, inner_ty, _is_mut)) = injected.as_ref() {
        let mut stmts: Vec<syn::Stmt> = Vec::new();
        let ignore_stmt: syn::Stmt = parse_quote! { let _ = #ident; };
        let alias_stmt: syn::Stmt =
            parse_quote! { #[allow(non_camel_case_types)] type #ident = #inner_ty; };
        stmts.push(ignore_stmt);
        stmts.push(alias_stmt);
        stmts.extend(item_fn.block.stmts.clone());
        item_fn.block.stmts = stmts;
    }

    // Create a `case` function from the original item, removing #[test] and using a stable name.
    item_fn.sig.ident = case_ident.clone();
    item_fn.attrs = cfg.clone(); // only keep cfg/cfg_attr on the case so it stays in sync.
    let case_tokens = quote! { #item_fn };

    // Wrapper: same attrs, but signature must be `fn foo() ...`.
    let wrapper_output = item_fn.sig.output.clone();
    let wrapper_block = quote!({
        #( #wrapper_prelude )*
        #case_ident( #( #wrapper_call_args ),* )
    });
    let wrapper_tokens = quote! {
        #( #wrapper_attrs )*
        fn #wrapper_ident() #wrapper_output #wrapper_block
    };

    let schema_tokens = ui_params.iter().map(|(ident, ty, _)| {
        let name = ident.to_string();
        let kind = test_param_kind(ty);
        quote! {
            ::graphium::export::TestParam {
                name: #name.to_string(),
                kind: #kind,
            }
        }
    });

    let defaults_tokens = ui_params.iter().map(|(ident, ty, _)| {
        let name = ident.to_string();
        let default_expr = defaults
            .values
            .get(&ident.to_string())
            .cloned()
            .unwrap_or_else(|| parse_quote! { ::core::default::Default::default() });
        quote! {
            out.insert(#name.to_string(), {
                let value: #ty = #default_expr;
                value.to_string()
            });
        }
    });

    let parse_tokens = ui_params.iter().map(|(ident, ty, _)| {
        let name = ident.to_string();
        let value_ident = format_ident!("__graphium_value_{}", ident);
        let raw_ident = format_ident!("__graphium_raw_{}", ident);
        if is_string_type(ty) {
            quote! {
                let #raw_ident: ::std::string::String = values
                    .get(#name)
                    .cloned()
                    .or_else(|| defaults.get(#name).cloned())
                    .unwrap_or_default();
                let #value_ident: #ty = #raw_ident;
            }
        } else if is_bool_type(ty) {
            quote! {
                let #raw_ident: ::std::string::String = values
                    .get(#name)
                    .cloned()
                    .or_else(|| defaults.get(#name).cloned())
                    .unwrap_or_default();
                let #value_ident: #ty = #raw_ident.parse().map_err(|_| {
                    format!("invalid bool for {name}: {value}", name = #name, value = #raw_ident)
                })?;
            }
        } else {
            quote! {
                let #raw_ident: ::std::string::String = values
                    .get(#name)
                    .cloned()
                    .or_else(|| defaults.get(#name).cloned())
                    .unwrap_or_default();
                let #value_ident: #ty = #raw_ident.parse().map_err(|_| {
                    format!("invalid value for {name}: {value}", name = #name, value = #raw_ident)
                })?;
            }
        }
    });

    let mut marker_call_args: Vec<proc_macro2::TokenStream> = Vec::new();
    if let Some((_let_tokens, pass_tokens)) = injected_arg.as_ref() {
        marker_call_args.push(pass_tokens.clone());
    }
    for (ident, _ty, _is_mut) in &ui_params {
        let value_ident = format_ident!("__graphium_value_{}", ident);
        marker_call_args.push(quote! { #value_ident });
    }

    let return_normalization = if is_result_unit(&item_fn.sig.output).is_some() {
        quote! {
            let res = #case_ident( #( #marker_call_args ),* );
            match res {
                Ok(()) => Ok(()),
                Err(err) => Err(err.to_string()),
            }
        }
    } else if matches!(item_fn.sig.output, ReturnType::Default) {
        quote! {
            #case_ident( #( #marker_call_args ),* );
            Ok(())
        }
    } else {
        let reason_lit = syn::LitStr::new(
            "graphium UI test runner supports only `()` or `Result<(), E>` return types",
            proc_macro2::Span::call_site(),
        );
        quote! { Err(#reason_lit.to_string()) }
    };

    let run_body = if supported {
        quote! {
            let defaults = Self::__graphium_ui_default_values();
            #( #parse_tokens )*
            match ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                #( #marker_prelude )*
                #return_normalization
            })) {
                Ok(res) => res,
                Err(payload) => Err(::graphium::export::panic_payload_to_string(payload)),
            }
        }
    } else {
        let reason_lit = syn::LitStr::new(unsupported_reason, proc_macro2::Span::call_site());
        quote! { Err(#reason_lit.to_string()) }
    };

    let marker_tokens = quote! {
        #( #cfg )*
        #[cfg(feature = "serialize")]
        pub struct #marker_ident;

        #( #cfg )*
        #[cfg(feature = "serialize")]
        impl #marker_ident {
            pub const NAME: &'static str = concat!(module_path!(), "::", stringify!(#wrapper_ident));

            pub fn __graphium_ui_schema() -> ::graphium::export::TestSchema {
                ::graphium::export::TestSchema {
                    params: vec![ #( #schema_tokens ),* ],
                }
            }

            pub fn __graphium_ui_default_values() -> ::std::collections::HashMap<::std::string::String, ::std::string::String> {
                let mut out: ::std::collections::HashMap<::std::string::String, ::std::string::String> = ::std::collections::HashMap::new();
                #( #defaults_tokens )*
                out
            }

            pub fn __graphium_ui_run_with_args(
                values: &::std::collections::HashMap<::std::string::String, ::std::string::String>,
            ) -> ::std::result::Result<(), ::std::string::String> {
                #run_body
            }
        }
    };

    Ok(UiTestCaseBits {
        wrapper_tokens,
        case_tokens,
        marker_bits: UiTestMarkerBits {
            marker_ident,
            marker_tokens,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthesize_ui_test_marker_generates_marker_type() {
        let item: ItemFn = parse_quote! {
            #[test]
            fn owned_graph_returns_non_zero_split(threshold: u32) {}
        };
        let bits = synthesize_ui_test_case(item).expect("generate case").marker_bits;
        assert_eq!(
            bits.marker_ident.to_string(),
            "OwnedGraphReturnsNonZeroSplit"
        );
        let s = bits.marker_tokens.to_string();
        assert!(s.contains("panic_payload_to_string"));
        assert!(s.contains("NAME"));
        assert!(s.contains("TestSchema"));
        assert!(s.contains("default_values"));
    }
}
