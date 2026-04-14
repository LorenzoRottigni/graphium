use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, Ident, ItemFn, Pat, ReturnType, Type, parse_macro_input};

use crate::shared::{NodeDef, pascal_case};

// Node expansion is intentionally simple now.
// A node macro only validates the user function and generates a thin wrapper
// exposing a uniform `__graphio_run` entry point. Artifact propagation is
// handled entirely by `graph!`.

/// Parses a user node function and emits its generated node wrapper type.
pub fn expand(input: TokenStream) -> TokenStream {
    let func = parse_macro_input!(input as ItemFn);
    let node_def = parse_node_def(&func);

    let fn_name = &node_def.fn_name;
    let struct_name = &node_def.struct_name;
    let ctx_type = &node_def.ctx_type;
    let ctx_param = if node_def.ctx_mut {
        quote! { &mut #ctx_type }
    } else {
        quote! { & #ctx_type }
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

    let expanded = quote! {
        #func

        pub struct #struct_name;

        impl #struct_name {
            pub const NAME: &'static str = stringify!(#fn_name);

            pub fn __graphio_run(
                ctx: #ctx_param,
                #( #input_idents: #input_types ),*
            ) #return_sig {
                println!("Running node: {}", Self::NAME);
                #fn_name(ctx, #( #input_idents ),*)
            }
        }
    };

    TokenStream::from(expanded)
}

/// Extracts the compile-time metadata the graph macro needs from a node
/// function signature.
fn parse_node_def(func: &ItemFn) -> NodeDef {
    let fn_name = func.sig.ident.clone();
    let struct_name = format_ident!("{}", pascal_case(&fn_name));

    let Some(FnArg::Typed(ctx_arg)) = func.sig.inputs.first() else {
        panic!("expected function with `&mut Context` as its first argument");
    };

    let Type::Reference(ctx_ref) = &*ctx_arg.ty else {
        panic!("expected `&Context` or `&mut Context` as the first node argument");
    };

    let ctx_type = (*ctx_ref.elem).clone();
    let ctx_mut = ctx_ref.mutability.is_some();

    let mut inputs = Vec::new();
    for (index, arg) in func.sig.inputs.iter().enumerate() {
        let FnArg::Typed(pat) = arg else {
            panic!("unexpected receiver in node function");
        };

        if index == 0 {
            continue;
        }

        let Pat::Ident(pat_ident) = &*pat.pat else {
            panic!("expected ident pattern for node input");
        };

        if let Type::Reference(reference) = &*pat.ty {
            if reference.mutability.is_some() {
                panic!(
                    "node input `{}` must be shared; mutable references are not supported",
                    pat_ident.ident
                );
            }
        }

        inputs.push((pat_ident.ident.clone(), (*pat.ty).clone()));
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
