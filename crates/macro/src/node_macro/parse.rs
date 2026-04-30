//! Parsing of node function signatures.
//!
//! This module handles extracting compile-time metadata from user-defined node
//! functions, including parameter classification, return type analysis, and
//! validation of ownership constraints.

use quote::format_ident;
use syn::{FnArg, ItemFn, Pat, ReturnType, Type};

use crate::ir::{MetricsSpec, NodeDef, ParamKind, doc_string_from_attrs, pascal_case};

/// Extracts the compile-time metadata the graph macro needs from a node
/// function signature.
///
/// This function analyzes the function signature to identify:
/// - The function and struct names
/// - Context parameter (if any)
/// - Input parameters and their types
/// - Return type and whether it's a Result
///
/// # Panics
///
/// Panics if the function signature is invalid:
/// - Has a receiver parameter (self, &self, etc.)
/// - Has multiple context parameters
/// - Context parameter is not a reference
/// - Return type contains references
pub fn parse_node_def(func: &ItemFn, metrics: MetricsSpec) -> NodeDef {
    let fn_name = func.sig.ident.clone();
    let struct_name = format_ident!("{}", pascal_case(&fn_name));

    let mut ctx_type: Option<Type> = None;
    let mut ctx_mut = false;
    let mut inputs = Vec::new();
    let mut param_kinds = Vec::new();

    for arg in func.sig.inputs.iter() {
        let FnArg::Typed(pat) = arg else {
            panic!("unexpected receiver in node function");
        };

        let Pat::Ident(pat_ident) = &*pat.pat else {
            panic!("expected ident pattern for node input");
        };

        let name = pat_ident.ident.to_string();
        if name == "ctx" || name == "_ctx" {
            if ctx_type.is_some() {
                panic!("node function must declare at most one context parameter");
            }

            let Type::Reference(ctx_ref) = &*pat.ty else {
                panic!("context parameter must be `&Context` or `&mut Context`");
            };

            ctx_type = Some((*ctx_ref.elem).clone());
            ctx_mut = ctx_ref.mutability.is_some();
            param_kinds.push(ParamKind::Ctx);
            continue;
        }

        // Inputs may be owned, shared (`&T`), or mutable (`&mut T`).
        // The graph DSL is responsible for choosing `&'a` vs `&'a mut` when
        // wiring artifacts between nodes.

        let index = inputs.len();
        inputs.push((pat_ident.ident.clone(), (*pat.ty).clone()));
        param_kinds.push(ParamKind::Input(index));
    }

    let return_ty = match &func.sig.output {
        ReturnType::Type(_, ty) => Some((**ty).clone()),
        ReturnType::Default => None,
    };
    let return_is_result = return_ty.as_ref().is_some_and(is_result_type);

    validate_return_type(&return_ty);

    NodeDef {
        fn_name,
        struct_name,
        ctx_type,
        ctx_mut,
        inputs,
        param_kinds,
        return_ty,
        metrics,
        return_is_result,
        docs: doc_string_from_attrs(&func.attrs),
        tags: Vec::new(),
        deprecated: false,
        deprecated_reason: None,
    }
}

/// Rejects borrowed return types so artifacts always remain owned while the
/// graph is propagating them between nodes.
///
/// Owned return types are required because the graph runtime manages artifact
/// ownership and needs predictable move semantics.
pub fn validate_return_type(return_ty: &Option<Type>) {
    match return_ty {
        Some(Type::Reference(_)) => panic!("node return type must be owned (no references)"),
        Some(Type::Tuple(tuple)) => {
            for elem in &tuple.elems {
                if matches!(elem, Type::Reference(_)) {
                    panic!("node tuple return types must be owned (no references)");
                }
            }
        }
        _ => {}
    }
}

/// Checks if a type is a `Result` type.
///
/// Used to determine whether to apply result-specific metrics tracking
/// (e.g., recording success/failure counts).
pub fn is_result_type(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };

    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Result")
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn parse_node_def_extracts_function_name() {
        let func: ItemFn = parse_quote! {
            fn my_function(input: String) -> String {
                input
            }
        };
        let def = parse_node_def(&func, MetricsSpec::default());
        assert_eq!(def.fn_name.to_string(), "my_function");
    }

    #[test]
    fn parse_node_def_generates_pascal_case_struct() {
        let func: ItemFn = parse_quote! {
            fn my_function(input: String) -> String {
                input
            }
        };
        let def = parse_node_def(&func, MetricsSpec::default());
        assert_eq!(def.struct_name.to_string(), "MyFunction");
    }

    #[test]
    fn parse_node_def_extracts_inputs() {
        let func: ItemFn = parse_quote! {
            fn my_node(a: String, b: i32) -> String {
                a
            }
        };
        let def = parse_node_def(&func, MetricsSpec::default());
        assert_eq!(def.inputs.len(), 2);
        assert_eq!(def.param_kinds.len(), 2);
    }

    #[test]
    fn parse_node_def_detects_ctx_parameter() {
        let func: ItemFn = parse_quote! {
            fn my_node(ctx: &MyCtx, input: String) -> String {
                input
            }
        };
        let def = parse_node_def(&func, MetricsSpec::default());
        assert!(def.ctx_type.is_some());
        assert!(!def.ctx_mut);
        assert_eq!(def.param_kinds[0], ParamKind::Ctx);
        assert_eq!(def.param_kinds[1], ParamKind::Input(0));
    }

    #[test]
    fn parse_node_def_detects_mutable_ctx() {
        let func: ItemFn = parse_quote! {
            fn my_node(ctx: &mut MyCtx, input: String) -> String {
                input
            }
        };
        let def = parse_node_def(&func, MetricsSpec::default());
        assert!(def.ctx_type.is_some());
        assert!(def.ctx_mut);
    }

    #[test]
    fn parse_node_def_detects_result_return() {
        let func: ItemFn = parse_quote! {
            fn my_node(input: String) -> Result<String, Error> {
                Ok(input)
            }
        };
        let def = parse_node_def(&func, MetricsSpec::default());
        assert!(def.return_is_result);
    }

    #[test]
    #[should_panic(expected = "node function must declare at most one context parameter")]
    fn parse_node_def_rejects_multiple_ctx() {
        let func: ItemFn = parse_quote! {
            fn my_node(ctx: &MyCtx, _ctx: &MyCtx) {}
        };
        parse_node_def(&func, MetricsSpec::default());
    }

    #[test]
    #[should_panic(expected = "context parameter must be `&Context` or `&mut Context`")]
    fn parse_node_def_rejects_owned_ctx() {
        let func: ItemFn = parse_quote! {
            fn my_node(ctx: MyCtx) {}
        };
        parse_node_def(&func, MetricsSpec::default());
    }

    #[test]
    #[should_panic(expected = "node return type must be owned")]
    fn validate_return_type_rejects_reference() {
        validate_return_type(&Some(parse_quote!(&str)));
    }

    #[test]
    #[should_panic(expected = "node tuple return types must be owned")]
    fn validate_return_type_rejects_tuple_with_reference() {
        validate_return_type(&Some(parse_quote!((String, &str))));
    }

    #[test]
    fn validate_return_type_accepts_owned() {
        validate_return_type(&Some(parse_quote!(String)));
        validate_return_type(&Some(parse_quote!(Result<String, Error>)));
    }

    #[test]
    fn is_result_type_detects_result() {
        let ty: Type = parse_quote!(Result<String, Error>);
        assert!(is_result_type(&ty));
    }

    #[test]
    fn is_result_type_rejects_non_result() {
        let ty: Type = parse_quote!(String);
        assert!(!is_result_type(&ty));
    }

    #[test]
    fn is_result_type_rejects_non_path_types() {
        let ty: Type = parse_quote!(&str);
        assert!(!is_result_type(&ty));
    }
}
