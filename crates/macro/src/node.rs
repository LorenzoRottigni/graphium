use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, Ident, ItemFn, Pat, ReturnType, Type, parse_macro_input};

use crate::shared::{NodeDef, ParamKind, pascal_case};

// Node expansion is intentionally simple now.
// A node macro only validates the user function and generates a thin wrapper
// exposing a uniform `__graphium_run` entry point. Artifact propagation is
// handled entirely by `graph!`.

/// Parses a user node function and emits its generated node wrapper type.
pub fn expand(input: TokenStream) -> TokenStream {
    let func = parse_macro_input!(input as ItemFn);
    let node_def = parse_node_def(&func);

    let fn_name = &node_def.fn_name;
    let struct_name = &node_def.struct_name;
    let is_async = func.sig.asyncness.is_some();
    let ctx_generic = if node_def.ctx_type.is_none() {
        quote! { <Ctx> }
    } else {
        quote! {}
    };
    let ctx_param = match &node_def.ctx_type {
        Some(ctx_type) => {
            if node_def.ctx_mut {
                quote! { &mut #ctx_type }
            } else {
                quote! { & #ctx_type }
            }
        }
        None => quote! { &Ctx },
    };
    let input_idents: Vec<Ident> = node_def
        .inputs
        .iter()
        .map(|(ident, _)| ident.clone())
        .collect();
    let input_types: Vec<Type> = node_def.inputs.iter().map(|(_, ty)| ty.clone()).collect();
    let return_sig = match &node_def.return_ty {
        Some(ty) => quote! { -> #ty },
        None => quote! {},
    };
    let call_args: Vec<proc_macro2::TokenStream> = node_def
        .param_kinds
        .iter()
        .map(|kind| match kind {
            ParamKind::Ctx => quote! { ctx },
            ParamKind::Input(index) => {
                let ident = &input_idents[*index];
                quote! { #ident }
            }
        })
        .collect();

    let sync_run = if is_async {
        quote! {}
    } else {
        quote! {
            pub fn __graphium_run #ctx_generic(
                ctx: #ctx_param,
                #( #input_idents: #input_types ),*
            ) #return_sig {
                println!("Running node: {}", Self::NAME);
                #fn_name(#( #call_args ),*)
            }
        }
    };

    let async_run = if is_async {
        quote! {
            pub async fn __graphium_run_async #ctx_generic(
                ctx: #ctx_param,
                #( #input_idents: #input_types ),*
            ) #return_sig {
                println!("Running node: {}", Self::NAME);
                #fn_name(#( #call_args ),*).await
            }
        }
    } else {
        quote! {
            pub async fn __graphium_run_async #ctx_generic(
                ctx: #ctx_param,
                #( #input_idents: #input_types ),*
            ) #return_sig {
                println!("Running node: {}", Self::NAME);
                #fn_name(#( #call_args ),*)
            }
        }
    };

    let expanded = quote! {
        #func

        pub struct #struct_name;

        impl #struct_name {
            pub const NAME: &'static str = stringify!(#fn_name);

            #sync_run
            #async_run
        }
    };

    TokenStream::from(expanded)
}

/// Extracts the compile-time metadata the graph macro needs from a node
/// function signature.
fn parse_node_def(func: &ItemFn) -> NodeDef {
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

        if let Type::Reference(reference) = &*pat.ty {
            if reference.mutability.is_some() {
                panic!(
                    "node input `{}` must be shared; mutable references are not supported",
                    pat_ident.ident
                );
            }
        }

        let index = inputs.len();
        inputs.push((pat_ident.ident.clone(), (*pat.ty).clone()));
        param_kinds.push(ParamKind::Input(index));
    }

    let return_ty = match &func.sig.output {
        ReturnType::Type(_, ty) => Some((**ty).clone()),
        ReturnType::Default => None,
    };

    validate_return_type(&return_ty);

    NodeDef {
        fn_name,
        struct_name,
        ctx_type,
        ctx_mut,
        inputs,
        param_kinds,
        return_ty,
    }
}

/// Rejects borrowed return types so artifacts always remain owned while the
/// graph is propagating them between nodes.
fn validate_return_type(return_ty: &Option<Type>) {
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
