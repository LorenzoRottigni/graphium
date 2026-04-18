use proc_macro2::TokenTree;
use syn::parse::discouraged::Speculative;
use syn::parse_quote;
use syn::{
    parse::{Parse, ParseStream},
    Expr, Ident, Path, Result, Token, Type,
};

use crate::shared::{
    parse_metric_name, GraphInput, LoopExpr, MetricsSpec, NodeCall, NodeExpr, RouteExpr, WhileExpr,
};

// Parsing module for the graph DSL.
// It turns the macro input tokens into a small IR (`NodeExpr`) that later
// drives hop-by-hop code generation.

impl Parse for NodeExpr {
    fn parse(input: ParseStream) -> Result<Self> {
        parse_sequence(input)
    }
}

/// Parses the highest-precedence sequence level of the graph DSL, splitting on
/// `>>` and preserving left-to-right execution order.
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

/// Parses a parallel group, splitting sibling nodes on `&`.
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

/// Parses a single graph atom: either a node call, `@match`, or `@if`
/// expression.
fn parse_primary(input: ParseStream) -> Result<NodeExpr> {
    if input.peek(Token![@]) {
        input.parse::<Token![@]>()?;
        if input.peek(Token![match]) {
            input.parse::<Token![match]>()?;

            let on_expr: Expr = parse_match_on_expr(input)?;
            let (outputs, output_borrows) = if input.peek(Token![->]) {
                input.parse::<Token![->]>()?;
                let out;
                syn::parenthesized!(out in input);
                parse_ident_list(&out)?
            } else {
                (Vec::new(), Vec::new())
            };
            let content;
            syn::braced!(content in input);
            return Ok(NodeExpr::Route(parse_match_routes(
                &content,
                on_expr,
                outputs,
                output_borrows,
            )?));
        }

        if input.peek(Token![if]) {
            input.parse::<Token![if]>()?;
            return Ok(NodeExpr::Route(parse_if_chain(input)?));
        }

        if input.peek(Token![while]) {
            input.parse::<Token![while]>()?;
            let condition: Expr = parse_match_on_expr(input)?;
            let (outputs, output_borrows) = if input.peek(Token![->]) {
                input.parse::<Token![->]>()?;
                let out;
                syn::parenthesized!(out in input);
                parse_ident_list(&out)?
            } else {
                (Vec::new(), Vec::new())
            };
            let content;
            syn::braced!(content in input);
            let body: NodeExpr = content.parse()?;
            return Ok(NodeExpr::While(WhileExpr {
                condition,
                body: Box::new(body),
                outputs,
                output_borrows,
            }));
        }

        if input.peek(Token![loop]) {
            input.parse::<Token![loop]>()?;
            let (outputs, output_borrows) = if input.peek(Token![->]) {
                input.parse::<Token![->]>()?;
                let out;
                syn::parenthesized!(out in input);
                parse_ident_list(&out)?
            } else {
                (Vec::new(), Vec::new())
            };
            let content;
            syn::braced!(content in input);
            let body: NodeExpr = content.parse()?;
            return Ok(NodeExpr::Loop(LoopExpr {
                body: Box::new(body),
                outputs,
                output_borrows,
            }));
        }

        if input.peek(Token![break]) {
            input.parse::<Token![break]>()?;
            return Ok(NodeExpr::Break);
        }

        return Err(input.error("expected `match`, `if`, `while`, `loop`, or `break` after `@`"));
    }

    Ok(NodeExpr::Single(input.parse()?))
}

impl Parse for NodeCall {
    fn parse(input: ParseStream) -> Result<Self> {
        let path: Path = input.parse()?;
        let explicit_inputs = input.peek(syn::token::Paren);
        let (inputs, input_borrows) = if explicit_inputs {
            let content;
            syn::parenthesized!(content in input);
            parse_ident_list(&content)?
        } else {
            (Vec::new(), Vec::new())
        };

        let (outputs, output_borrows) = if input.peek(Token![->]) {
            input.parse::<Token![->]>()?;
            if input.peek(syn::token::Paren) {
                let content;
                syn::parenthesized!(content in input);
                parse_ident_list(&content)?
            } else {
                let is_borrowed = if input.peek(Token![&]) {
                    input.parse::<Token![&]>()?;
                    true
                } else {
                    false
                };
                let ident: Ident = input.parse()?;
                (vec![ident], vec![is_borrowed])
            }
        } else {
            (Vec::new(), Vec::new())
        };

        Ok(Self {
            path,
            explicit_inputs,
            inputs,
            input_borrows,
            outputs,
            output_borrows,
        })
    }
}

/// Parses a comma-separated list of artifact names used for node inputs or
/// outputs in the graph DSL.
fn parse_ident_list(input: ParseStream) -> Result<(Vec<Ident>, Vec<bool>)> {
    let mut idents = Vec::new();
    let mut borrows = Vec::new();

    while !input.is_empty() {
        let is_borrowed = if input.peek(Token![&]) {
            input.parse::<Token![&]>()?;
            true
        } else {
            false
        };
        idents.push(input.parse()?);
        borrows.push(is_borrowed);
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        } else {
            break;
        }
    }

    Ok((idents, borrows))
}

/// Parses a comma-separated list like `(artifact: Type, other: Type)`.
fn parse_typed_ident_list(input: ParseStream) -> Result<Vec<(Ident, Type)>> {
    let mut items = Vec::new();

    while !input.is_empty() {
        let ident: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty: Type = input.parse()?;
        items.push((ident, ty));
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        } else {
            break;
        }
    }

    Ok(items)
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
            outputs: Vec::new(),
            output_borrows: Vec::new(),
            is_if_chain: false,
        })
    }
}

fn parse_match_routes(
    content: ParseStream,
    on: Expr,
    outputs: Vec<Ident>,
    output_borrows: Vec<bool>,
) -> Result<RouteExpr> {
    let mut routes: Vec<(Expr, NodeExpr)> = Vec::new();

    while !content.is_empty() {
        let key_expr: Expr = content.parse()?;
        content.parse::<Token![=>]>()?;
        let value: NodeExpr = content.parse()?;
        routes.push((key_expr, value));
        content.parse::<Token![,]>().ok();
    }

    Ok(RouteExpr {
        on,
        routes,
        outputs,
        output_borrows,
        is_if_chain: false,
    })
}

fn parse_match_on_expr(input: ParseStream) -> Result<Expr> {
    let fork = input.fork();
    let mut tokens = proc_macro2::TokenStream::new();

    while !fork.is_empty() && !fork.peek(Token![->]) && !fork.peek(syn::token::Brace) {
        let tt: TokenTree = fork.parse()?;
        tokens.extend(std::iter::once(tt));
    }

    if tokens.is_empty() {
        return Err(input.error("expected match selector expression"));
    }

    let expr: Expr = syn::parse2(tokens)?;
    input.advance_to(&fork);
    Ok(expr)
}

fn parse_if_chain(input: ParseStream) -> Result<RouteExpr> {
    let (cond_expr, cond_is_closure, cond_params, cond_args) = parse_if_condition(input)?;
    let (outputs, output_borrows) = if input.peek(Token![->]) {
        input.parse::<Token![->]>()?;
        let out;
        syn::parenthesized!(out in input);
        parse_ident_list(&out)?
    } else {
        (Vec::new(), Vec::new())
    };
    let content;
    syn::braced!(content in input);
    let then_branch: NodeExpr = content.parse()?;

    let mut conditions = vec![(cond_expr, cond_is_closure)];
    let mut branches = vec![then_branch];

    while input.peek(Token![@]) {
        let fork = input.fork();
        fork.parse::<Token![@]>()?;
        if fork.peek(Token![else]) {
            input.parse::<Token![@]>()?;
            input.parse::<Token![else]>()?;
            let content;
            syn::braced!(content in input);
            let branch: NodeExpr = content.parse()?;
            branches.push(branch);
            break;
        }

        let ident: Ident = fork.parse()?;
        if ident == "elif" {
            input.parse::<Token![@]>()?;
            input.parse::<Ident>()?;
            let (expr, is_closure, _params, _args) = parse_if_condition(input)?;
            let content;
            syn::braced!(content in input);
            let branch: NodeExpr = content.parse()?;
            conditions.push((expr, is_closure));
            branches.push(branch);
            continue;
        }
        break;
    }

    if branches.len() != conditions.len() + 1 {
        return Err(input.error("`@if` requires a trailing `@else` branch"));
    }

    // Build selector closure.
    let use_closure = conditions.iter().any(|(_, is_closure)| *is_closure);
    if use_closure && !conditions.iter().all(|(_, is_closure)| *is_closure) {
        return Err(
            input.error("all `@if/@elif` conditions must be closures when one is a closure")
        );
    }

    let cond_calls: Vec<proc_macro2::TokenStream> = conditions
        .iter()
        .map(|(expr, is_closure)| {
            if *is_closure {
                quote::quote! { (#expr)(#( #cond_args ),*) }
            } else {
                quote::quote! { (#expr) }
            }
        })
        .collect();

    let mut selector_body = proc_macro2::TokenStream::new();
    for (idx, call) in cond_calls.iter().enumerate() {
        let branch_idx = idx as u32;
        if idx == 0 {
            selector_body.extend(quote::quote! { if #call { #branch_idx } });
        } else {
            selector_body.extend(quote::quote! { else if #call { #branch_idx } });
        }
    }
    let else_idx = cond_calls.len() as u32;
    selector_body.extend(quote::quote! { else { #else_idx } });

    let on_expr_tokens = if use_closure {
        quote::quote! { |#cond_params| { #selector_body } }
    } else {
        quote::quote! { || { #selector_body } }
    };
    let on_expr: Expr = syn::parse2(on_expr_tokens)?;

    let routes: Vec<(Expr, NodeExpr)> = branches
        .into_iter()
        .enumerate()
        .map(|(idx, branch)| {
            let lit = syn::LitInt::new(&idx.to_string(), proc_macro2::Span::call_site());
            let key = Expr::Lit(syn::ExprLit {
                attrs: Vec::new(),
                lit: syn::Lit::Int(lit),
            });
            (key, branch)
        })
        .collect();

    Ok(RouteExpr {
        on: on_expr,
        routes,
        outputs,
        output_borrows,
        is_if_chain: true,
    })
}

fn parse_if_condition(
    input: ParseStream,
) -> Result<(
    Expr,
    bool,
    proc_macro2::TokenStream,
    Vec<proc_macro2::TokenStream>,
)> {
    let expr: Expr = parse_match_on_expr(input)?;
    if let Expr::Closure(closure) = &expr {
        let mut params = proc_macro2::TokenStream::new();
        let mut args = Vec::new();
        for (idx, input) in closure.inputs.iter().enumerate() {
            if idx > 0 {
                params.extend(quote::quote! { , });
            }
            params.extend(quote::quote! { #input });
            let ident = match input {
                syn::Pat::Ident(pat_ident) => pat_ident.ident.clone(),
                syn::Pat::Type(pat_type) => {
                    let syn::Pat::Ident(pat_ident) = &*pat_type.pat else {
                        return Err(syn::Error::new_spanned(
                            pat_type,
                            "selector parameters must be identifiers",
                        ));
                    };
                    pat_ident.ident.clone()
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        input,
                        "selector parameters must be identifiers",
                    ));
                }
            };
            args.push(quote::quote! { #ident });
        }
        return Ok((expr, true, params, args));
    }

    Ok((expr, false, proc_macro2::TokenStream::new(), Vec::new()))
}

impl Parse for GraphInput {
    /// Parses the outer `graph!` object, including graph name, context type,
    /// and the bracketed graph schema.
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(Token![#]) {
            return parse_graph_input_with_metadata(input);
        }

        parse_graph_input_legacy(input)
    }
}

/// Parses the original key/value graph syntax:
/// `name: ..., context: ..., inputs: (...), outputs: (...), schema: [ ... ]`.
fn parse_graph_input_legacy(input: ParseStream) -> Result<GraphInput> {
    let mut name: Option<Ident> = None;
    let mut context: Option<Path> = None;
    let mut graph_inputs: Vec<(Ident, Type)> = Vec::new();
    let mut graph_outputs: Vec<(Ident, Type)> = Vec::new();
    let mut nodes: Option<NodeExpr> = None;
    let mut async_enabled = false;
    let mut metrics = MetricsSpec::default();

    while !input.is_empty() {
        if input.peek(Token![async]) {
            input.parse::<Token![async]>()?;
            input.parse::<Token![:]>()?;
            let lit: syn::LitBool = input.parse()?;
            async_enabled = lit.value;
        } else {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match key.to_string().as_str() {
                "name" => {
                    name = Some(input.parse()?);
                }
                "context" => {
                    context = Some(input.parse()?);
                }
                "inputs" => {
                    let content;
                    syn::parenthesized!(content in input);
                    graph_inputs = parse_typed_ident_list(&content)?;
                }
                "outputs" => {
                    let content;
                    syn::parenthesized!(content in input);
                    graph_outputs = parse_typed_ident_list(&content)?;
                }
                "schema" => {
                    let content;
                    syn::bracketed!(content in input);
                    nodes = Some(content.parse()?);
                }
                "async" => {
                    let lit: syn::LitBool = input.parse()?;
                    async_enabled = lit.value;
                }
                "metrics" => {
                    let content;
                    syn::parenthesized!(content in input);
                    metrics = parse_metrics_list(&content)?;
                }
                _ => {
                    return Err(input
                        .error("expected one of: `name`, `context`, `inputs`, `outputs`, `schema`, `async`, `metrics`"));
                }
            }
        }

        input.parse::<Token![,]>().ok();
    }

    let nodes = nodes.ok_or_else(|| input.error("missing `schema`"))?;
    let context = match context {
        Some(ctx) => ctx,
        None => {
            if node_expr_uses_borrowed_artifacts(&nodes) {
                return Err(input.error(
                    "missing `context`; graphs that borrow artifacts (e.g. `(&x)` or `-> (&x)`) must declare a context type that stores borrowed artifacts as fields",
                ));
            }
            parse_quote!(::graphium::Context)
        }
    };

    Ok(GraphInput {
        name: name.ok_or_else(|| input.error("missing `name`"))?,
        context,
        inputs: graph_inputs,
        outputs: graph_outputs,
        nodes,
        async_enabled,
        metrics,
    })
}

fn node_expr_uses_borrowed_artifacts(node: &NodeExpr) -> bool {
    match node {
        NodeExpr::Single(call) => {
            call.input_borrows.iter().any(|b| *b) || call.output_borrows.iter().any(|b| *b)
        }
        NodeExpr::Sequence(nodes) | NodeExpr::Parallel(nodes) => {
            nodes.iter().any(node_expr_uses_borrowed_artifacts)
        }
        NodeExpr::Route(route) => {
            route.output_borrows.iter().any(|b| *b)
                || route
                    .routes
                    .iter()
                    .any(|(_, node)| node_expr_uses_borrowed_artifacts(node))
        }
        NodeExpr::While(while_expr) => {
            while_expr.output_borrows.iter().any(|b| *b)
                || node_expr_uses_borrowed_artifacts(&while_expr.body)
        }
        NodeExpr::Loop(loop_expr) => {
            loop_expr.output_borrows.iter().any(|b| *b)
                || node_expr_uses_borrowed_artifacts(&loop_expr.body)
        }
        NodeExpr::Break => false,
    }
}

/// Parses the ergonomic metadata style:
/// `#[metadata(context = Ctx, inputs = (...), outputs = (...))] MyGraph { ... }`
fn parse_graph_input_with_metadata(input: ParseStream) -> Result<GraphInput> {
    let mut context: Option<Path> = None;
    let mut graph_inputs: Vec<(Ident, Type)> = Vec::new();
    let mut graph_outputs: Vec<(Ident, Type)> = Vec::new();
    let mut async_enabled = false;
    let mut metrics = MetricsSpec::default();
    let mut metadata_seen = false;

    while input.peek(Token![#]) {
        input.parse::<Token![#]>()?;
        let bracket_content;
        syn::bracketed!(bracket_content in input);

        let attr_name: Ident = bracket_content.parse()?;
        if attr_name == "metadata" {
            metadata_seen = true;
            let metadata_content;
            syn::parenthesized!(metadata_content in bracket_content);
            while !metadata_content.is_empty() {
                let key_string = if metadata_content.peek(Token![async]) {
                    metadata_content.parse::<Token![async]>()?;
                    "async".to_string()
                } else {
                    let key: Ident = metadata_content.parse()?;
                    key.to_string()
                };

                metadata_content.parse::<Token![=]>()?;

                match key_string.as_str() {
                    "context" => {
                        context = Some(metadata_content.parse()?);
                    }
                    "inputs" => {
                        let typed;
                        syn::parenthesized!(typed in metadata_content);
                        graph_inputs = parse_typed_ident_list(&typed)?;
                    }
                    "outputs" => {
                        let typed;
                        syn::parenthesized!(typed in metadata_content);
                        graph_outputs = parse_typed_ident_list(&typed)?;
                    }
                    "async" => {
                        let lit: syn::LitBool = metadata_content.parse()?;
                        async_enabled = lit.value;
                    }
                    _ => {
                        return Err(metadata_content
                            .error("expected one of: `context`, `inputs`, `outputs`, `async`"));
                    }
                }

                if metadata_content.peek(Token![,]) {
                    metadata_content.parse::<Token![,]>()?;
                } else {
                    break;
                }
            }
        } else if attr_name == "metrics" {
            let metrics_content;
            syn::parenthesized!(metrics_content in bracket_content);
            metrics = parse_metrics_list(&metrics_content)?;
        } else {
            return Err(bracket_content.error("expected `metadata` or `metrics`"));
        }

        if !bracket_content.is_empty() {
            return Err(bracket_content.error("unexpected tokens in attribute payload"));
        }
    }

    if !metadata_seen {
        return Err(input.error("missing `#[metadata(...)]` attribute"));
    }

    let name: Ident = input.parse()?;
    let body;
    syn::braced!(body in input);
    let nodes: NodeExpr = body.parse()?;
    if !body.is_empty() {
        return Err(body.error("unexpected tokens after graph schema"));
    }

    input.parse::<Token![,]>().ok();
    if !input.is_empty() {
        return Err(input.error("unexpected tokens after graph definition"));
    }

    let context = match context {
        Some(ctx) => ctx,
        None => {
            if node_expr_uses_borrowed_artifacts(&nodes) {
                return Err(input.error(
                    "missing `context`; graphs that borrow artifacts (e.g. `(&x)` or `-> (&x)`) must declare a context type that stores borrowed artifacts as fields",
                ));
            }
            parse_quote!(::graphium::Context)
        }
    };

    Ok(GraphInput {
        name,
        context,
        inputs: graph_inputs,
        outputs: graph_outputs,
        nodes,
        async_enabled,
        metrics,
    })
}

fn parse_metrics_list(input: ParseStream) -> Result<MetricsSpec> {
    let mut metrics = MetricsSpec::default();
    while !input.is_empty() {
        let metric_name: syn::LitStr = input.parse()?;
        let apply = parse_metric_name(metric_name.value().as_str()).ok_or_else(|| {
            input.error("unsupported metric; allowed: performance, errors, count, caller, success_rate, fail_rate")
        })?;
        apply(&mut metrics);

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        } else {
            break;
        }
    }
    Ok(metrics)
}
