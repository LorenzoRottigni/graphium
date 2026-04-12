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
type Payload = BTreeMap<String, Ident>;

#[derive(Clone)]
struct ExprShape {
    entry_usage: UsageMap,
    exit_outputs: Vec<String>,
}

struct GeneratedExpr {
    tokens: proc_macro2::TokenStream,
    outputs: Payload,
}

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

    let mut counter = 0usize;
    let generated = generate_expr(&nodes, &Payload::new(), &mut counter);
    let body = if generated.outputs.is_empty() {
        generated.tokens
    } else {
        let generated_tokens = generated.tokens;
        // Final hop payload is not visible outside the graph run, so drop it immediately.
        quote! {{
            #generated_tokens
        }}
    };

    let expanded = quote! {
        pub struct #name;

        impl #name {
            pub fn run(ctx: &mut #context) {
                #body
            }
        }
    };

    TokenStream::from(expanded)
}

fn generate_expr(node: &NodeExpr, incoming: &Payload, counter: &mut usize) -> GeneratedExpr {
    match node {
        NodeExpr::Single(call) => generate_single(call, incoming, counter),
        NodeExpr::Sequence(nodes) => generate_sequence(nodes, incoming, counter),
        NodeExpr::Parallel(nodes) => generate_parallel(nodes, incoming, counter),
        NodeExpr::Route(route) => generate_route(route, incoming, counter),
    }
}

fn generate_single(call: &NodeCall, incoming: &Payload, counter: &mut usize) -> GeneratedExpr {
    let path = &call.path;

    if is_graph_run_path(path) {
        if !call.inputs.is_empty() || !call.outputs.is_empty() {
            panic!("graph `run` calls do not support explicit inputs or outputs");
        }

        return GeneratedExpr {
            tokens: quote! {
                #path(ctx);
            },
            outputs: Payload::new(),
        };
    }

    let mut remaining = UsageMap::new();
    for input in &call.inputs {
        *remaining.entry(input.to_string()).or_insert(0) += 1;
    }

    let mut arg_vars = Vec::with_capacity(call.inputs.len());
    let mut input_bindings = Vec::with_capacity(call.inputs.len());

    for input in &call.inputs {
        let artifact_name = input.to_string();
        let source = incoming
            .get(&artifact_name)
            .unwrap_or_else(|| panic!("missing artifact `{artifact_name}` for node call"));
        let remaining_uses = remaining
            .get_mut(&artifact_name)
            .unwrap_or_else(|| panic!("missing usage count for `{artifact_name}`"));
        let arg_ident = fresh_ident(counter, "arg", &artifact_name);

        if *remaining_uses == 1 {
            input_bindings.push(quote! {
                let #arg_ident = #source
                    .take()
                    .unwrap_or_else(|| panic!(concat!("missing artifact `", stringify!(#input), "`")));
            });
        } else {
            input_bindings.push(quote! {
                let #arg_ident = ::graphio::clone_artifact(
                    #source
                        .as_ref()
                        .unwrap_or_else(|| panic!(concat!("missing artifact `", stringify!(#input), "`")))
                );
            });
        }

        *remaining_uses -= 1;
        arg_vars.push(arg_ident);
    }

    if call.outputs.is_empty() {
        return GeneratedExpr {
            tokens: quote! {
                #( #input_bindings )*
                #path::__graphio_run(ctx, #( #arg_vars ),*);
            },
            outputs: Payload::new(),
        };
    }

    let mut outputs = Payload::new();
    if call.outputs.len() == 1 {
        let artifact_name = call.outputs[0].to_string();
        let output_var = fresh_ident(counter, "hop", &artifact_name);
        outputs.insert(artifact_name, output_var.clone());

        GeneratedExpr {
            tokens: quote! {
                #( #input_bindings )*
                let mut #output_var = ::std::option::Option::Some(#path::__graphio_run(ctx, #( #arg_vars ),*));
            },
            outputs,
        }
    } else {
        let tuple_vars: Vec<Ident> = call
            .outputs
            .iter()
            .map(|output| fresh_ident(counter, "ret", &output.to_string()))
            .collect();
        let output_stores =
            call.outputs
                .iter()
                .zip(tuple_vars.iter())
                .map(|(output, tuple_var)| {
                    let artifact_name = output.to_string();
                    let output_var = fresh_ident(counter, "hop", &artifact_name);
                    outputs.insert(artifact_name, output_var.clone());
                    quote! {
                        let mut #output_var = ::std::option::Option::Some(#tuple_var);
                    }
                });

        GeneratedExpr {
            tokens: quote! {
                #( #input_bindings )*
                let ( #( #tuple_vars ),* ) = #path::__graphio_run(ctx, #( #arg_vars ),*);
                #( #output_stores )*
            },
            outputs,
        }
    }
}

fn generate_sequence(nodes: &[NodeExpr], incoming: &Payload, counter: &mut usize) -> GeneratedExpr {
    let mut iter = nodes.iter();
    let first = iter
        .next()
        .expect("sequence must contain at least one node");
    let mut current = generate_expr(first, incoming, counter);

    for next in iter {
        let shape = analyze_expr(next);
        let required = required_artifacts(&shape);
        let mut next_payload = Payload::new();
        let mut transfer_tokens = Vec::with_capacity(required.len());

        for artifact in required {
            let source = current
                .outputs
                .get(&artifact)
                .unwrap_or_else(|| panic!("missing artifact `{artifact}` for next hop"));
            let payload_var = fresh_ident(counter, "payload", &artifact);
            next_payload.insert(artifact.clone(), payload_var.clone());
            transfer_tokens.push(quote! {
                let mut #payload_var = #source.take();
            });
        }

        let next_generated = generate_expr(next, &next_payload, counter);
        let current_tokens = current.tokens;
        let next_tokens = next_generated.tokens;
        current = capture_outputs(
            quote! {
                #current_tokens
                #( #transfer_tokens )*
                #next_tokens
            },
            next_generated.outputs,
            counter,
        );
    }

    current
}

fn generate_parallel(nodes: &[NodeExpr], incoming: &Payload, counter: &mut usize) -> GeneratedExpr {
    let shapes: Vec<ExprShape> = nodes.iter().map(analyze_expr).collect();
    let mut remaining = UsageMap::new();
    for shape in &shapes {
        for artifact in required_artifacts(shape) {
            *remaining.entry(artifact).or_insert(0) += 1;
        }
    }

    let exit_outputs = collect_parallel_outputs(&shapes);
    let mut outputs = Payload::new();
    let mut output_decl_tokens = Vec::new();
    for artifact in &exit_outputs {
        let output_var = fresh_ident(counter, "parallel_out", artifact);
        outputs.insert(artifact.clone(), output_var.clone());
        output_decl_tokens.push(quote! {
            let mut #output_var = ::std::option::Option::None;
        });
    }

    let mut blocks = Vec::new();
    for (node, shape) in nodes.iter().zip(shapes.iter()) {
        let mut child_payload = Payload::new();
        let mut child_bindings = Vec::new();

        for artifact in required_artifacts(shape) {
            let source = incoming
                .get(&artifact)
                .unwrap_or_else(|| panic!("missing artifact `{artifact}` for parallel step"));
            let remaining_children = remaining
                .get_mut(&artifact)
                .unwrap_or_else(|| panic!("missing usage count for `{artifact}`"));
            let payload_var = fresh_ident(counter, "parallel_in", &artifact);
            child_payload.insert(artifact.clone(), payload_var.clone());

            if *remaining_children == 1 {
                child_bindings.push(quote! {
                    let mut #payload_var = #source.take();
                });
            } else {
                child_bindings.push(quote! {
                    let mut #payload_var = ::std::option::Option::Some(::graphio::clone_artifact(
                        #source
                            .as_ref()
                            .unwrap_or_else(|| panic!(concat!("missing artifact `", #artifact, "`")))
                    ));
                });
            }

            *remaining_children -= 1;
        }

        let generated = generate_expr(node, &child_payload, counter);
        let generated_tokens = generated.tokens;
        let output_assigns = generated.outputs.iter().map(|(artifact, inner)| {
            let outer = outputs
                .get(artifact)
                .unwrap_or_else(|| panic!("missing parallel output slot for `{artifact}`"));
            quote! {
                #outer = #inner;
            }
        });

        blocks.push(quote! {
            {
                #( #child_bindings )*
                #generated_tokens
                #( #output_assigns )*
            }
        });
    }

    GeneratedExpr {
        tokens: quote! {
            #( #output_decl_tokens )*
            #( #blocks )*
        },
        outputs,
    }
}

fn generate_route(route: &RouteExpr, incoming: &Payload, counter: &mut usize) -> GeneratedExpr {
    let branch_shapes: Vec<ExprShape> = route
        .routes
        .iter()
        .map(|(_, node)| analyze_expr(node))
        .collect();
    let exit_outputs = collect_route_outputs(&branch_shapes);

    let mut outputs = Payload::new();
    let mut output_decl_tokens = Vec::new();
    for artifact in &exit_outputs {
        let output_var = fresh_ident(counter, "route_out", artifact);
        outputs.insert(artifact.clone(), output_var.clone());
        output_decl_tokens.push(quote! {
            let mut #output_var = ::std::option::Option::None;
        });
    }

    let on_expr = &route.on;
    let mut arms = Vec::new();
    for ((key, node), shape) in route.routes.iter().zip(branch_shapes.iter()) {
        let mut branch_payload = Payload::new();
        let mut branch_bindings = Vec::new();

        for artifact in required_artifacts(shape) {
            let source = incoming
                .get(&artifact)
                .unwrap_or_else(|| panic!("missing artifact `{artifact}` for route branch"));
            let payload_var = fresh_ident(counter, "route_in", &artifact);
            branch_payload.insert(artifact, payload_var.clone());
            branch_bindings.push(quote! {
                let mut #payload_var = #source.take();
            });
        }

        let generated = generate_expr(node, &branch_payload, counter);
        let generated_tokens = generated.tokens;
        let output_assigns = generated.outputs.iter().map(|(artifact, inner)| {
            let outer = outputs
                .get(artifact)
                .unwrap_or_else(|| panic!("missing route output slot for `{artifact}`"));
            quote! {
                #outer = #inner;
            }
        });

        arms.push(quote! {
            #key => {
                #( #branch_bindings )*
                #generated_tokens
                #( #output_assigns )*
            }
        });
    }

    GeneratedExpr {
        tokens: quote! {
            #( #output_decl_tokens )*
            match (#on_expr)(ctx) {
                #( #arms, )*
            }
        },
        outputs,
    }
}

fn capture_outputs(
    inner_tokens: proc_macro2::TokenStream,
    inner_outputs: Payload,
    counter: &mut usize,
) -> GeneratedExpr {
    if inner_outputs.is_empty() {
        return GeneratedExpr {
            tokens: quote! {{
                #inner_tokens
            }},
            outputs: Payload::new(),
        };
    }

    let mut outer_outputs = Payload::new();
    let declaration_pairs: Vec<(String, Ident)> = inner_outputs
        .keys()
        .map(|artifact| {
            let outer_var = fresh_ident(counter, "captured", artifact);
            (artifact.clone(), outer_var)
        })
        .collect();

    for (artifact, outer_var) in &declaration_pairs {
        outer_outputs.insert(artifact.clone(), outer_var.clone());
    }

    let declarations = declaration_pairs.iter().map(|(_, outer_var)| {
        quote! {
            let mut #outer_var = ::std::option::Option::None;
        }
    });

    let assignments = inner_outputs.iter().map(|(artifact, inner)| {
        let outer = outer_outputs
            .get(artifact)
            .unwrap_or_else(|| panic!("missing captured output slot for `{artifact}`"));
        quote! {
            #outer = #inner;
        }
    });

    GeneratedExpr {
        tokens: quote! {
            #( #declarations )*
            {
                #inner_tokens
                #( #assignments )*
            }
        },
        outputs: outer_outputs,
    }
}

fn analyze_expr(node: &NodeExpr) -> ExprShape {
    match node {
        NodeExpr::Single(call) => {
            if is_graph_run_path(&call.path) {
                if !call.inputs.is_empty() || !call.outputs.is_empty() {
                    panic!("graph `run` calls do not support explicit inputs or outputs");
                }

                return ExprShape {
                    entry_usage: UsageMap::new(),
                    exit_outputs: Vec::new(),
                };
            }

            let mut entry_usage = UsageMap::new();
            for input in &call.inputs {
                *entry_usage.entry(input.to_string()).or_insert(0) += 1;
            }

            ExprShape {
                entry_usage,
                exit_outputs: call.outputs.iter().map(ToString::to_string).collect(),
            }
        }
        NodeExpr::Sequence(nodes) => {
            let first = nodes
                .first()
                .unwrap_or_else(|| panic!("sequence must contain at least one node"));
            let last = nodes
                .last()
                .unwrap_or_else(|| panic!("sequence must contain at least one node"));

            ExprShape {
                entry_usage: analyze_expr(first).entry_usage,
                exit_outputs: analyze_expr(last).exit_outputs,
            }
        }
        NodeExpr::Parallel(nodes) => {
            let shapes: Vec<ExprShape> = nodes.iter().map(analyze_expr).collect();
            let mut entry_usage = UsageMap::new();

            for shape in &shapes {
                for artifact in required_artifacts(shape) {
                    *entry_usage.entry(artifact).or_insert(0) += 1;
                }
            }

            ExprShape {
                entry_usage,
                exit_outputs: collect_parallel_outputs(&shapes),
            }
        }
        NodeExpr::Route(route) => {
            let shapes: Vec<ExprShape> = route
                .routes
                .iter()
                .map(|(_, node)| analyze_expr(node))
                .collect();
            let mut entry_usage = UsageMap::new();

            for shape in &shapes {
                for artifact in required_artifacts(shape) {
                    entry_usage.entry(artifact).or_insert(1);
                }
            }

            ExprShape {
                entry_usage,
                exit_outputs: collect_route_outputs(&shapes),
            }
        }
    }
}

fn required_artifacts(shape: &ExprShape) -> Vec<String> {
    shape.entry_usage.keys().cloned().collect()
}

fn collect_parallel_outputs(shapes: &[ExprShape]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut outputs = Vec::new();

    for shape in shapes {
        for artifact in &shape.exit_outputs {
            if !seen.insert(artifact.clone()) {
                panic!("parallel step produces duplicate artifact `{artifact}`");
            }
            outputs.push(artifact.clone());
        }
    }

    outputs
}

fn collect_route_outputs(shapes: &[ExprShape]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut outputs = Vec::new();

    for shape in shapes {
        for artifact in &shape.exit_outputs {
            if seen.insert(artifact.clone()) {
                outputs.push(artifact.clone());
            }
        }
    }

    outputs
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

fn fresh_ident(counter: &mut usize, prefix: &str, name: &str) -> Ident {
    let ident = format_ident!("__graphio_{}_{}_{}", prefix, *counter, name);
    *counter += 1;
    ident
}

fn is_graph_run_path(path: &Path) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == "run")
}
