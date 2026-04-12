use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, punctuated::Punctuated, Expr, FnArg, Ident, ItemFn, Pat, Path, Result,
    ReturnType, Token, Type,
};

// ===================================================
//                    node! macro
// ===================================================
#[proc_macro]
pub fn node(input: TokenStream) -> TokenStream {
    let mut func = parse_macro_input!(input as ItemFn);

    let fn_name = &func.sig.ident;

    let mut output_names: Vec<Ident> = vec![];
    let mut retained_attrs = vec![];
    for attr in func.attrs.iter() {
        if attr.path().is_ident("artifacts") {
            let parsed: Punctuated<Ident, Token![,]> =
                attr.parse_args_with(Punctuated::parse_terminated)
                    .expect("invalid #[artifacts(...)] syntax");
            output_names.extend(parsed.into_iter());
        } else {
            retained_attrs.push(attr.clone());
        }
    }
    func.attrs = retained_attrs;

    // Convert to PascalCase
    let struct_name = format_ident!(
        "{}Node",
        fn_name
            .to_string()
            .split('_')
            .map(|s| {
                let mut chars = s.chars();
                match chars.next() {
                    Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect::<String>()
    );

    // extract &mut Context
    let ctx_type = match func.sig.inputs.first() {
        Some(syn::FnArg::Typed(pat)) => {
            if let syn::Type::Reference(r) = &*pat.ty {
                &r.elem
            } else {
                panic!("expected &mut Context as first arg");
            }
        }
        _ => panic!("expected function with ctx argument"),
    };

    let mut fn_inputs: Vec<(Ident, Type)> = vec![];
    for (idx, arg) in func.sig.inputs.iter().enumerate() {
        let FnArg::Typed(pat) = arg else {
            panic!("unexpected receiver in node fn");
        };
        if idx == 0 {
            continue;
        }
        let Pat::Ident(pat_ident) = &*pat.pat else {
            panic!("expected ident pattern for node arg");
        };
        fn_inputs.push((pat_ident.ident.clone(), (*pat.ty).clone()));
    }

    let return_ty: Option<Type> = match &func.sig.output {
        ReturnType::Type(_, ty) => Some((**ty).clone()),
        ReturnType::Default => None,
    };

    if !output_names.is_empty() {
        match &return_ty {
            Some(Type::Tuple(tuple)) => {
                if output_names.len() != tuple.elems.len() {
                    panic!("artifacts count must match tuple length");
                }
            }
            Some(_) => {
                if output_names.len() != 1 {
                    panic!("artifacts must contain exactly one name for single return type");
                }
            }
            None => {
                panic!("artifacts provided but function returns nothing");
            }
        }
    }

    if let Some(ret_ty) = &return_ty {
        match ret_ty {
            Type::Reference(_) => panic!("node return type must be owned (no references)"),
            Type::Tuple(tuple) => {
                for elem in tuple.elems.iter() {
                    if matches!(elem, Type::Reference(_)) {
                        panic!("node tuple return types must be owned (no references)");
                    }
                }
            }
            _ => {}
        }
    }

    let store_artifacts = if let Some(Type::Tuple(tuple)) = &return_ty {
        let outs = tuple.elems.iter().enumerate().map(|(i, elem)| {
            let idx = syn::Index::from(i);
            if output_names.is_empty() {
                quote! { store.insert::<#elem>(__out.#idx); }
            } else {
                let name = output_names[i].to_string();
                quote! {
                    store.insert_named::<#elem>(#name, __out.#idx);
                }
            }
        });
        quote! { #( #outs )* }
    } else if let Some(ret_ty) = &return_ty {
        if output_names.is_empty() {
            quote! { store.insert::<#ret_ty>(__out); }
        } else {
            let name = output_names[0].to_string();
            quote! {
                store.insert_named::<#ret_ty>(#name, __out);
            }
        }
    } else {
        quote! {}
    };

    let get_arg_exprs = fn_inputs.iter().map(|(ident, ty)| {
        let (store_ty, is_ref, is_ref_mut) = match ty {
            Type::Reference(r) => {
                let inner = &r.elem;
                (quote! { #inner }, true, r.mutability.is_some())
            }
            _ => (quote! { #ty }, false, false),
        };

        let name = ident.to_string();
        if is_ref_mut {
            quote! {
                let #ident = store
                    .get_named_mut::<#store_ty>(#name)
                    .or_else(|| store.get_mut::<#store_ty>())
                    .unwrap_or_else(|| panic!(concat!("missing dependency for ", stringify!(#ident))));
            }
        } else if is_ref {
            quote! {
                let #ident = store
                    .get_named::<#store_ty>(#name)
                    .or_else(|| store.get::<#store_ty>())
                    .unwrap_or_else(|| panic!(concat!("missing dependency for ", stringify!(#ident))));
            }
        } else {
            quote! {
                let #ident = store
                    .take_named::<#store_ty>(#name)
                    .or_else(|| store.take::<#store_ty>())
                    .unwrap_or_else(|| panic!(concat!("missing dependency for ", stringify!(#ident))));
            }
        }
    });

    let call_args = func.sig.inputs.iter().enumerate().map(|(idx, arg)| {
        let FnArg::Typed(pat) = arg else {
            panic!("unexpected receiver in node fn");
        };
        if idx == 0 {
            quote! { ctx }
        } else {
            let Pat::Ident(pat_ident) = &*pat.pat else {
                panic!("expected ident pattern for node arg");
            };
            let ident = &pat_ident.ident;
            quote! { #ident }
        }
    });

    let input_idents: Vec<Ident> = fn_inputs.iter().map(|(ident, _)| ident.clone()).collect();

    let expanded = quote! {
        #func

        pub struct #struct_name;

        impl #struct_name {
            pub const INPUTS: &'static [&'static str] = &[
                #( stringify!(#input_idents) ),*
            ];
            pub const ARTIFACTS: &'static [&'static str] = &[
                #( stringify!(#output_names) ),*
            ];
        }

        impl #struct_name {
            pub const NAME: &'static str = stringify!(#fn_name);
        }

        impl Node<#ctx_type> for #struct_name {
            fn run(ctx: &mut #ctx_type, store: &mut ::graphio::Store) {
                println!("Running node: {}", Self::NAME);
                #( #get_arg_exprs )*
                let __out = #fn_name( #( #call_args ),* );
                #store_artifacts
            }
        }
    };

    TokenStream::from(expanded)
}

// ===================================================
//                    GRAPH IR
// ===================================================

#[derive(Clone)]
enum NodeExpr {
    Single(Path),
    Sequence(Vec<NodeExpr>),
    Parallel(Vec<NodeExpr>),
    Route(RouteExpr),
}

// ===================================================
//                    ROUTE IR
// ===================================================

#[derive(Clone)]
struct RouteExpr {
    on: Expr,
    routes: Vec<(Expr, NodeExpr)>,
}

// ===================================================
//                  GRAPH INPUT
// ===================================================

struct GraphInput {
    name: Ident,
    context: Path,
    nodes: NodeExpr,
}

// ===================================================
//                NODE PARSING
// ===================================================

impl Parse for NodeExpr {
    fn parse(input: ParseStream) -> Result<Self> {
        parse_sequence(input)
    }
}

// ---------------- sequence (>>) ----------------

fn parse_sequence(input: ParseStream) -> Result<NodeExpr> {
    let mut nodes = vec![parse_parallel(input)?];

    while input.peek(Token![>>]) {
        input.parse::<Token![>>]>()?;
        nodes.push(parse_parallel(input)?);
    }

    if nodes.len() == 1 {
        Ok(nodes.remove(0))
    } else {
        Ok(NodeExpr::Sequence(nodes))
    }
}

// ---------------- parallel (&) ----------------

fn parse_parallel(input: ParseStream) -> Result<NodeExpr> {
    let mut nodes = vec![parse_primary(input)?];

    while input.peek(Token![&]) {
        input.parse::<Token![&]>()?;
        nodes.push(parse_primary(input)?);
    }

    if nodes.len() == 1 {
        Ok(nodes.remove(0))
    } else {
        Ok(NodeExpr::Parallel(nodes))
    }
}

// ===================================================
//                PRIMARY EXPRESSIONS
// ===================================================

fn parse_primary(input: ParseStream) -> Result<NodeExpr> {
    if input.peek(Token![@]) {
        input.parse::<Token![@]>()?;
        let ident: Ident = input.parse()?;

        if ident != "route" {
            return Err(input.error("expected `route` after `@`"));
        }

        let content;
        syn::braced!(content in input);

        return Ok(NodeExpr::Route(content.parse()?));
    }

    let path: Path = input.parse()?;
    Ok(NodeExpr::Single(path))
}

// ===================================================
//                ROUTE PARSER
// ===================================================

impl Parse for RouteExpr {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut on: Option<Expr> = None;
        let mut routes: Vec<(Expr, NodeExpr)> = vec![];

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match key.to_string().as_str() {
                "on" => {
                    let expr: Expr = input.parse()?;
                    on = Some(expr);
                    input.parse::<Token![,]>().ok();
                }

                "routes" => {
                    let content;
                    syn::braced!(content in input);

                    while !content.is_empty() {
                        let key_expr: Expr = content.parse()?;
                        content.parse::<Token![=>]>()?;

                        let value: NodeExpr = content.parse()?;

                        routes.push((key_expr, value));
                        content.parse::<Token![,]>().ok();
                    }

                    input.parse::<Token![,]>().ok();
                }

                _ => return Err(input.error("expected `on` or `routes`")),
            }
        }

        Ok(RouteExpr {
            on: on.ok_or_else(|| input.error("missing `on`"))?,
            routes,
        })
    }
}

// ===================================================
//                GRAPH INPUT PARSER
// ===================================================

impl Parse for GraphInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let _: Ident = input.parse()?; // name
        input.parse::<Token![:]>()?;
        let name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;

        let _: Ident = input.parse()?; // context
        input.parse::<Token![:]>()?;
        let context: Path = input.parse()?;
        input.parse::<Token![,]>()?;

        let _: Ident = input.parse()?; // nodes
        input.parse::<Token![:]>()?;

        let content;
        syn::bracketed!(content in input);
        let nodes: NodeExpr = content.parse()?;

        Ok(GraphInput {
            name,
            context,
            nodes,
        })
    }
}

// ===================================================
//                GRAPH MACRO
// ===================================================

#[proc_macro]
pub fn graph(input: TokenStream) -> TokenStream {
    let GraphInput {
        name,
        context,
        nodes,
    } = parse_macro_input!(input as GraphInput);

    let body = generate(&nodes);

    let expanded = quote! {
        pub struct #name;

        impl #name {
            pub fn run(ctx: &mut #context) {
                let mut store = ::graphio::Store::default();
                let store = &mut store;
                #body
            }
        }

        impl Node<#context> for #name {
            fn run(ctx: &mut #context, store: &mut ::graphio::Store) {
                #body
            }
        }
    };

    TokenStream::from(expanded)
}

// ===================================================
//                CODEGEN
// ===================================================

fn generate(node: &NodeExpr) -> proc_macro2::TokenStream {
    match node {
        NodeExpr::Single(path) => {
            let is_run_path = match path.segments.last() {
                Some(seg) => seg.ident == "run",
                None => false,
            };
            if is_run_path {
                let mut type_path = path.clone();
                type_path.segments.pop();
                type_path.segments.pop_punct();
                if type_path.segments.is_empty() {
                    panic!("invalid `run` path");
                }
                quote! {
                    <#type_path as Node<_>>::run(ctx, store);
                }
            } else {
                quote! {
                    <#path as Node<_>>::run(ctx, store);
                }
            }
        }

        NodeExpr::Sequence(nodes) => {
            let parts = nodes.iter().map(generate);
            quote! { #( #parts )* }
        }

        NodeExpr::Parallel(nodes) => {
            let parts = nodes.iter().map(generate);
            quote! {
                // TODO: real parallelism
                #( #parts )*
            }
        }

        NodeExpr::Route(route) => {
            let on_expr = &route.on;

            let routes = route.routes.iter().map(|(key, node)| {
                let body = generate(node);
                quote! {
                    #key => { #body }
                }
            });

            quote! {
                match (#on_expr)(ctx) {
                    #( #routes, )*
                }
            }
        }
    }
}
