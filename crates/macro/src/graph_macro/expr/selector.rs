//! Selector and condition binding helpers.
//!
//! Routes and loops can evaluate closures or function-like selectors. This
//! module parses selector parameters and generates the correct borrow/clone
//! setup for calling them.

use std::collections::BTreeSet;

use quote::quote;

use crate::shared::{Payload, fresh_ident};

/// Describes how a selector or condition receives each parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SelectorParam {
    Ctx { mutable: bool },
    Artifact { ident: syn::Ident, borrowed: bool },
}

/// Selector/condition binding plan shared by route and loop code generation.
pub(crate) struct ConditionBindings {
    pub bindings: Vec<proc_macro2::TokenStream>,
    pub args: Vec<proc_macro2::TokenStream>,
    pub is_empty: bool,
}

/// Selector binding plan for route expressions.
pub(crate) struct SelectorBindings {
    pub bindings: Vec<proc_macro2::TokenStream>,
    pub args: Vec<proc_macro2::TokenStream>,
    pub is_empty: bool,
}

/// Infers selector parameters from a route or loop callback expression.
///
/// Example:
/// providing `|ctx: &mut Ctx, value: &Value, owned| ...` expands into
/// `[Ctx { mutable: true }, Artifact { value, borrowed: true }, Artifact { owned, borrowed: false }]`.
pub(crate) fn selector_params_for_on_expr(on: &syn::Expr) -> Vec<SelectorParam> {
    if let syn::Expr::Closure(_) = on {
        return parse_selector_params(on);
    }

    if let syn::Expr::Path(path) = on {
        if path.qself.is_none() && path.path.segments.len() == 1 {
            let ident = path.path.segments[0].ident.clone();
            return vec![SelectorParam::Artifact {
                ident,
                borrowed: false,
            }];
        }
    }

    Vec::new()
}

/// Parses the parameters accepted by a selector closure.
///
/// Example:
/// providing `|flag, item: &Item| ...` expands into parameter descriptors for
/// owned `flag` and borrowed `item`.
pub(super) fn parse_selector_params(on: &syn::Expr) -> Vec<SelectorParam> {
    let syn::Expr::Closure(closure) = on else {
        return Vec::new();
    };

    let mut params = Vec::new();
    for input in &closure.inputs {
        match input {
            syn::Pat::Type(pat_type) => {
                let syn::Pat::Ident(pat_ident) = &*pat_type.pat else {
                    panic!("selector parameters must be identifiers");
                };
                let name = pat_ident.ident.to_string();
                if name == "ctx" || name == "_ctx" {
                    let mutable =
                        matches!(&*pat_type.ty, syn::Type::Reference(r) if r.mutability.is_some());
                    params.push(SelectorParam::Ctx { mutable });
                } else {
                    let borrowed = matches!(&*pat_type.ty, syn::Type::Reference(_));
                    params.push(SelectorParam::Artifact {
                        ident: pat_ident.ident.clone(),
                        borrowed,
                    });
                }
            }
            syn::Pat::Ident(pat_ident) => {
                let name = pat_ident.ident.to_string();
                if name == "ctx" || name == "_ctx" {
                    params.push(SelectorParam::Ctx { mutable: false });
                } else {
                    params.push(SelectorParam::Artifact {
                        ident: pat_ident.ident.clone(),
                        borrowed: false,
                    });
                }
            }
            _ => panic!("selector parameters must be identifiers"),
        }
    }

    params
}

/// Builds borrowed or cloned arguments for an `@while` condition callback.
///
/// Example:
/// providing params `[Artifact { value, borrowed: false }]` expands into a
/// binding like `let cond_arg = clone_artifact(...);` plus call args `[cond_arg]`.
pub(crate) fn build_condition_bindings(
    params: &[SelectorParam],
    incoming: &Payload,
    counter: &mut usize,
) -> ConditionBindings {
    let mut bindings = Vec::new();
    let mut args = Vec::new();
    let mut has_borrowed = false;
    let mut wants_mut_ctx = false;

    for param in params {
        match param {
            SelectorParam::Ctx { mutable } => {
                if *mutable {
                    wants_mut_ctx = true;
                    args.push(quote! { ctx });
                } else {
                    args.push(quote! { &*ctx });
                }
            }
            SelectorParam::Artifact { ident, borrowed } => {
                let artifact_name = ident.to_string();
                if *borrowed {
                    has_borrowed = true;
                    if incoming.has_borrowed(&artifact_name) {
                        args.push(quote! { &ctx.#ident });
                    } else {
                        let source = incoming.get_owned(&artifact_name).unwrap_or_else(|| {
                            panic!("missing artifact `{artifact_name}` for @while condition")
                        });
                        let arg_ident = fresh_ident(counter, "cond_borrow", &artifact_name);
                        bindings.push(quote! {
                            let #arg_ident = #source
                                .as_ref()
                                .unwrap_or_else(|| panic!(concat!("missing artifact `", #artifact_name, "`")));
                        });
                        args.push(quote! { #arg_ident });
                    }
                } else {
                    let source = incoming.get_owned(&artifact_name).unwrap_or_else(|| {
                        panic!("missing artifact `{artifact_name}` for @while condition")
                    });
                    let arg_ident = fresh_ident(counter, "cond_arg", &artifact_name);
                    bindings.push(quote! {
                        let #arg_ident = ::graphium::clone_artifact(
                            #source
                                .as_ref()
                                .unwrap_or_else(|| panic!(concat!("missing artifact `", #artifact_name, "`")))
                        );
                    });
                    args.push(quote! { #arg_ident });
                }
            }
        }
    }

    if has_borrowed && wants_mut_ctx {
        panic!("@while condition cannot take `&mut ctx` and borrowed artifacts at the same time");
    }

    ConditionBindings {
        bindings,
        args,
        is_empty: params.is_empty(),
    }
}

/// Emits the final call expression for an `@while` condition.
///
/// Example:
/// providing closure `|value| value > 0` and args `[arg0]` expands into
/// `(|value| value > 0)(arg0)`.
pub(crate) fn build_condition_call(
    condition: &syn::Expr,
    args: &[proc_macro2::TokenStream],
    is_empty: bool,
) -> proc_macro2::TokenStream {
    if let syn::Expr::Closure(closure) = condition {
        if closure.inputs.is_empty() {
            return quote! { (#condition)() };
        }
        return quote! { (#condition)(#( #args ),*) };
    }

    if is_empty {
        quote! { #condition }
    } else {
        if let syn::Expr::Path(path) = condition {
            if path.qself.is_none() && path.path.segments.len() == 1 {
                return quote! { #(#args)* };
            }
        }
        quote! { (#condition)(#( #args ),*) }
    }
}

/// Builds borrowed or cloned arguments for a route selector callback.
///
/// Example:
/// providing an owned selector input that branches still need expands into a
/// `clone_artifact(...)` binding instead of a `.take()`.
pub(crate) fn build_selector_bindings(
    params: &[SelectorParam],
    incoming: &Payload,
    needed_by_branches: &BTreeSet<String>,
    counter: &mut usize,
) -> SelectorBindings {
    let mut bindings = Vec::new();
    let mut args = Vec::new();
    let mut has_borrowed = false;
    let mut wants_mut_ctx = false;

    for param in params {
        match param {
            SelectorParam::Ctx { mutable } => {
                if *mutable {
                    wants_mut_ctx = true;
                }
                if *mutable {
                    args.push(quote! { ctx });
                } else {
                    args.push(quote! { &*ctx });
                }
            }
            SelectorParam::Artifact { ident, borrowed } => {
                let artifact_name = ident.to_string();
                if *borrowed {
                    if incoming.has_borrowed(&artifact_name) {
                        has_borrowed = true;
                        args.push(quote! { &ctx.#ident });
                    } else {
                        let source = incoming.get_owned(&artifact_name).unwrap_or_else(|| {
                            panic!("missing artifact `{artifact_name}` for @match selector")
                        });
                        let arg_ident = fresh_ident(counter, "selector_borrow", &artifact_name);
                        bindings.push(quote! {
                            let #arg_ident = #source
                                .as_ref()
                                .unwrap_or_else(|| panic!(concat!("missing artifact `", #artifact_name, "`")));
                        });
                        args.push(quote! { #arg_ident });
                    }
                } else {
                    let source = incoming.get_owned(&artifact_name).unwrap_or_else(|| {
                        panic!("missing artifact `{artifact_name}` for @match selector")
                    });
                    let arg_ident = fresh_ident(counter, "selector_arg", &artifact_name);
                    if needed_by_branches.contains(&artifact_name) {
                        bindings.push(quote! {
                            let #arg_ident = ::graphium::clone_artifact(
                                #source
                                    .as_ref()
                                    .unwrap_or_else(|| panic!(concat!("missing artifact `", #artifact_name, "`")))
                            );
                        });
                    } else {
                        bindings.push(quote! {
                            let #arg_ident = #source
                                .take()
                                .unwrap_or_else(|| panic!(concat!("missing artifact `", #artifact_name, "`")));
                        });
                    }
                    args.push(quote! { #arg_ident });
                }
            }
        }
    }

    if has_borrowed && wants_mut_ctx {
        panic!("@match selector cannot take `&mut ctx` and borrowed artifacts at the same time");
    }

    let is_empty = params.is_empty();
    SelectorBindings {
        bindings,
        args,
        is_empty,
    }
}

/// Emits the final call expression for a route selector.
///
/// Example:
/// providing selector `choose_branch` and args `[value]` expands into
/// `choose_branch(value)`, while a zero-arg closure expands into `(|| ... )()`.
pub(crate) fn build_selector_call(
    on_expr: &syn::Expr,
    args: &[proc_macro2::TokenStream],
    is_empty: bool,
) -> proc_macro2::TokenStream {
    if let syn::Expr::Closure(closure) = on_expr {
        if closure.inputs.is_empty() {
            return quote! { (#on_expr)() };
        }
        return quote! { (#on_expr)(#( #args ),*) };
    }

    if !args.is_empty() {
        if let syn::Expr::Path(path) = on_expr {
            if path.qself.is_none() && path.path.segments.len() == 1 {
                return quote! { #(#args)* };
            }
        }
    }

    if is_empty {
        quote! { #on_expr }
    } else {
        quote! { (#on_expr)(#( #args ),*) }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use syn::{Ident, parse_quote};

    use super::{
        SelectorParam, build_condition_bindings, build_selector_bindings, parse_selector_params,
        selector_params_for_on_expr,
    };
    use crate::shared::Payload;

    #[test]
    fn parse_selector_params_tracks_ctx_and_borrows() {
        let expr: syn::Expr = parse_quote!(|ctx: &mut Ctx, value: &Value, owned| true);
        let params = parse_selector_params(&expr);

        assert_eq!(params[0], SelectorParam::Ctx { mutable: true });
        assert_eq!(
            params[1],
            SelectorParam::Artifact {
                ident: parse_quote!(value),
                borrowed: true,
            }
        );
        assert_eq!(
            params[2],
            SelectorParam::Artifact {
                ident: parse_quote!(owned),
                borrowed: false,
            }
        );
    }

    #[test]
    fn selector_params_for_bare_path_assume_owned_artifact() {
        let params = selector_params_for_on_expr(&parse_quote!(decision));

        assert_eq!(
            params,
            vec![SelectorParam::Artifact {
                ident: parse_quote!(decision),
                borrowed: false,
            }]
        );
    }

    #[test]
    fn build_condition_bindings_rejects_mut_ctx_with_borrowed_artifacts() {
        let params = vec![
            SelectorParam::Ctx { mutable: true },
            SelectorParam::Artifact {
                ident: parse_quote!(value),
                borrowed: true,
            },
        ];
        let mut payload = Payload::new();
        payload.insert_borrowed("value".into());

        let result =
            std::panic::catch_unwind(|| build_condition_bindings(&params, &payload, &mut 0));
        assert!(result.is_err());
    }

    #[test]
    fn build_selector_bindings_clones_shared_selector_inputs() {
        let params = vec![SelectorParam::Artifact {
            ident: Ident::new("value", proc_macro2::Span::call_site()),
            borrowed: false,
        }];
        let mut payload = Payload::new();
        payload.insert_owned(
            "value".into(),
            Ident::new("slot", proc_macro2::Span::call_site()),
        );

        let bindings = build_selector_bindings(
            &params,
            &payload,
            &BTreeSet::from(["value".to_string()]),
            &mut 0,
        );

        assert!(bindings.bindings[0].to_string().contains("clone_artifact"));
    }
}
