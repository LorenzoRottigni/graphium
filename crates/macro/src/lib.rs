use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::{BTreeMap, BTreeSet};
use syn::{
    Expr, FnArg, Ident, ItemFn, Pat, Path, Result, ReturnType, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

#[derive(Clone)]
struct NodeDef {
    fn_name: Ident,
    struct_name: Ident,
    ctx_type: Type,
    inputs: Vec<(Ident, Type)>,
    return_ty: Option<Type>,
}

#[derive(Clone)]
struct NodeCall {
    path: Path,
    inputs: Vec<Ident>,
    outputs: Vec<Ident>,
}

#[derive(Clone)]
enum NodeExpr {
    Single(NodeCall),
    Sequence(Vec<NodeExpr>),
    Parallel(Vec<NodeExpr>),
    Route(RouteExpr),
}

#[derive(Clone)]
struct RouteExpr {
    on: Expr,
    routes: Vec<(Expr, NodeExpr)>,
}

struct GraphInput {
    name: Ident,
    context: Path,
    nodes: NodeExpr,
}

type UsageMap = BTreeMap<String, usize>;

#[proc_macro]
pub fn node(input: TokenStream) -> TokenStream {
    let func = parse_macro_input!(input as ItemFn);
    let node_def = parse_node_def(&func);

    let fn_name = &node_def.fn_name;
    let struct_name = &node_def.struct_name;
    let ctx_type = &node_def.ctx_type;
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
                ctx: &mut #ctx_type,
                #( #input_idents: #input_types ),*
            ) #return_sig {
                println!("Running node: {}", Self::NAME);
                #fn_name(ctx, #( #input_idents ),*)
            }
        }
    };

    TokenStream::from(expanded)
}

impl Parse for NodeExpr {
    fn parse(input: ParseStream) -> Result<Self> {
        parse_sequence(input)
    }
}

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

    Ok(NodeExpr::Single(input.parse()?))
}

impl Parse for NodeCall {
    fn parse(input: ParseStream) -> Result<Self> {
        let path: Path = input.parse()?;
        let inputs = if input.peek(syn::token::Paren) {
            let content;
            syn::parenthesized!(content in input);
            parse_ident_list(&content)?
        } else {
            Vec::new()
        };

        let outputs = if input.peek(Token![->]) {
            input.parse::<Token![->]>()?;
            let content;
            syn::parenthesized!(content in input);
            parse_ident_list(&content)?
        } else {
            Vec::new()
        };

        Ok(Self {
            path,
            inputs,
            outputs,
        })
    }
}

fn parse_ident_list(input: ParseStream) -> Result<Vec<Ident>> {
    let mut idents = Vec::new();

    while !input.is_empty() {
        idents.push(input.parse()?);
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        } else {
            break;
        }
    }

    Ok(idents)
}

impl Parse for RouteExpr {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut on: Option<Expr> = None;
        let mut routes: Vec<(Expr, NodeExpr)> = vec![];

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match key.to_string().as_str() {
                "on" => {
                    on = Some(input.parse()?);
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

impl Parse for GraphInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let _: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;

        let _: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let context: Path = input.parse()?;
        input.parse::<Token![,]>()?;

        let _: Ident = input.parse()?;
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

#[proc_macro]
pub fn graph(input: TokenStream) -> TokenStream {
    let GraphInput {
        name,
        context,
        nodes,
    } = parse_macro_input!(input as GraphInput);

    let mut artifact_names = BTreeSet::new();
    collect_artifact_names(&nodes, &mut artifact_names);

    let declarations = artifact_names.iter().map(|artifact| {
        let slot = artifact_slot_ident(artifact);
        quote! {
            let mut #slot = ::std::option::Option::None;
        }
    });

    let generated = generate_graph_expr(&nodes, &UsageMap::new());
    if !generated.usage_before.is_empty() {
        let missing = generated
            .usage_before
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        panic!("graph requires artifacts before it starts: {missing}");
    }

    let body = generated.tokens;

    let expanded = quote! {
        pub struct #name;

        impl #name {
            pub fn run(ctx: &mut #context) {
                #( #declarations )*
                #body
            }
        }
    };

    TokenStream::from(expanded)
}

struct GeneratedExpr {
    tokens: proc_macro2::TokenStream,
    usage_before: UsageMap,
}

fn generate_graph_expr(node: &NodeExpr, remaining_after: &UsageMap) -> GeneratedExpr {
    match node {
        NodeExpr::Single(call) => generate_single(call, remaining_after),
        NodeExpr::Sequence(nodes) | NodeExpr::Parallel(nodes) => {
            let mut remaining = remaining_after.clone();
            let mut generated = Vec::with_capacity(nodes.len());

            for child in nodes.iter().rev() {
                let part = generate_graph_expr(child, &remaining);
                remaining = part.usage_before.clone();
                generated.push(part.tokens);
            }

            generated.reverse();

            GeneratedExpr {
                tokens: quote! { #( #generated )* },
                usage_before: remaining,
            }
        }
        NodeExpr::Route(route) => {
            let mut usage_before = UsageMap::new();
            let on_expr = &route.on;

            let routes = route.routes.iter().map(|(key, branch)| {
                let generated = generate_graph_expr(branch, remaining_after);
                usage_before = merge_usage_max(&usage_before, &generated.usage_before);
                let branch_tokens = generated.tokens;
                quote! {
                    #key => { #branch_tokens }
                }
            });

            GeneratedExpr {
                tokens: quote! {
                    match (#on_expr)(ctx) {
                        #( #routes, )*
                    }
                },
                usage_before,
            }
        }
    }
}

fn generate_single(call: &NodeCall, remaining_after: &UsageMap) -> GeneratedExpr {
    let path = &call.path;

    if is_graph_run_path(path) {
        if !call.inputs.is_empty() || !call.outputs.is_empty() {
            panic!("graph `run` calls do not support explicit inputs or outputs");
        }

        return GeneratedExpr {
            tokens: quote! {
                #path(ctx);
            },
            usage_before: remaining_after.clone(),
        };
    }

    let input_bindings = call.inputs.iter().map(|input_name| {
        let slot = artifact_slot_ident(&input_name.to_string());
        let remaining_uses = remaining_after
            .get(&input_name.to_string())
            .copied()
            .unwrap_or(0);

        if remaining_uses == 0 {
            quote! {
                let #input_name = #slot
                    .take()
                    .unwrap_or_else(|| panic!(concat!("missing artifact `", stringify!(#input_name), "`")));
            }
        } else {
            quote! {
                let #input_name = ::graphio::clone_artifact(
                    #slot
                        .as_ref()
                        .unwrap_or_else(|| panic!(concat!("missing artifact `", stringify!(#input_name), "`")))
                );
            }
        }
    });

    let call_args = &call.inputs;
    let invoke = if call.outputs.is_empty() {
        quote! {
            #path::__graphio_run(ctx, #( #call_args ),*);
        }
    } else if call.outputs.len() == 1 {
        let output = &call.outputs[0];
        let slot = artifact_slot_ident(&output.to_string());
        quote! {
            #slot = ::std::option::Option::Some(#path::__graphio_run(ctx, #( #call_args ),*));
        }
    } else {
        let bindings: Vec<Ident> = call
            .outputs
            .iter()
            .enumerate()
            .map(|(index, _)| format_ident!("__graphio_out_{index}"))
            .collect();
        let stores = call
            .outputs
            .iter()
            .zip(bindings.iter())
            .map(|(output, binding)| {
                let slot = artifact_slot_ident(&output.to_string());
                quote! {
                    #slot = ::std::option::Option::Some(#binding);
                }
            });

        quote! {
            let ( #( #bindings ),* ) = #path::__graphio_run(ctx, #( #call_args ),*);
            #( #stores )*
        }
    };

    let mut usage_before = remaining_after.clone();
    for output in &call.outputs {
        usage_before.remove(&output.to_string());
    }
    for input in &call.inputs {
        *usage_before.entry(input.to_string()).or_insert(0) += 1;
    }

    GeneratedExpr {
        tokens: quote! {
            #( #input_bindings )*
            #invoke
        },
        usage_before,
    }
}

fn collect_artifact_names(node: &NodeExpr, names: &mut BTreeSet<String>) {
    match node {
        NodeExpr::Single(call) => {
            for input in &call.inputs {
                names.insert(input.to_string());
            }
            for output in &call.outputs {
                names.insert(output.to_string());
            }
        }
        NodeExpr::Sequence(nodes) | NodeExpr::Parallel(nodes) => {
            for child in nodes {
                collect_artifact_names(child, names);
            }
        }
        NodeExpr::Route(route) => {
            for (_, branch) in &route.routes {
                collect_artifact_names(branch, names);
            }
        }
    }
}

fn merge_usage_max(left: &UsageMap, right: &UsageMap) -> UsageMap {
    let mut merged = left.clone();
    for (artifact, count) in right {
        let entry = merged.entry(artifact.clone()).or_insert(0);
        if *entry < *count {
            *entry = *count;
        }
    }
    merged
}

fn parse_node_def(func: &ItemFn) -> NodeDef {
    let fn_name = func.sig.ident.clone();
    let struct_name = format_ident!("{}Node", pascal_case(&fn_name));

    let Some(FnArg::Typed(ctx_arg)) = func.sig.inputs.first() else {
        panic!("expected function with `&mut Context` as its first argument");
    };

    let Type::Reference(ctx_ref) = &*ctx_arg.ty else {
        panic!("expected `&mut Context` as the first node argument");
    };

    if ctx_ref.mutability.is_none() {
        panic!("expected the first node argument to be `&mut Context`");
    }

    let ctx_type = (*ctx_ref.elem).clone();

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

        if matches!(&*pat.ty, Type::Reference(_)) {
            panic!(
                "node input `{}` must be owned; use the context for borrowed data",
                pat_ident.ident
            );
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
        inputs,
        return_ty,
    }
}

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

fn pascal_case(ident: &Ident) -> String {
    ident
        .to_string()
        .split('_')
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<String>()
}

fn artifact_slot_ident(name: &str) -> Ident {
    format_ident!("__graphio_artifact_{}", name)
}

fn is_graph_run_path(path: &Path) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == "run")
}
