//! Parsing of the `graph!` DSL input tokens.
//!
//! This module is intentionally kept close to the `graph_macro` implementation
//! because it defines the syntax the rest of the expander depends on.
//!
//! The primary output of parsing is a small internal IR (`NodeExpr` /
//! `GraphInput`) that the code generator can traverse without needing to look
//! back at raw tokens.

use proc_macro2::TokenTree;
use syn::parse::discouraged::Speculative;
use syn::parse_quote;
use syn::{
    Expr, Ident, Path, Result, Token, Type,
    parse::{Parse, ParseStream},
};

use crate::ir::{
    ArtifactInputKind, GraphInput, LoopExpr, MetricsSpec, NodeCall, NodeExpr, RouteExpr, WhileExpr,
    parse_metric_name,
};

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
                parse_output_ident_list(&out)?
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
                parse_output_ident_list(&out)?
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
                parse_output_ident_list(&out)?
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
        let (inputs, input_kinds) = if explicit_inputs {
            let content;
            syn::parenthesized!(content in input);
            parse_input_ident_list(&content)?
        } else {
            (Vec::new(), Vec::new())
        };

        let (outputs, output_borrows) = if input.peek(Token![->]) {
            input.parse::<Token![->]>()?;
            if input.peek(syn::token::Paren) {
                let content;
                syn::parenthesized!(content in input);
                parse_output_ident_list(&content)?
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
            input_kinds,
            outputs,
            output_borrows,
        })
    }
}

/// Parses a comma-separated list of artifact names used for node inputs or
/// outputs in the graph DSL.
fn parse_input_ident_list(input: ParseStream) -> Result<(Vec<Ident>, Vec<ArtifactInputKind>)> {
    let mut idents = Vec::new();
    let mut kinds = Vec::new();

    while !input.is_empty() {
        let kind = if input.peek(Token![&]) {
            input.parse::<Token![&]>()?;
            ArtifactInputKind::Borrowed
        } else if input.peek(Token![*]) {
            input.parse::<Token![*]>()?;
            ArtifactInputKind::Taken
        } else {
            ArtifactInputKind::Owned
        };
        idents.push(input.parse()?);
        kinds.push(kind);
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        } else {
            break;
        }
    }

    Ok((idents, kinds))
}

fn parse_output_ident_list(input: ParseStream) -> Result<(Vec<Ident>, Vec<bool>)> {
    let mut idents = Vec::new();
    let mut borrows = Vec::new();

    while !input.is_empty() {
        let is_borrowed = if input.peek(Token![&]) {
            input.parse::<Token![&]>()?;
            true
        } else if input.peek(Token![*]) {
            return Err(input.error("`*artifact` is only supported for node inputs"));
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
        parse_output_ident_list(&out)?
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
        parse_graph_input(input)
    }
}

fn node_expr_uses_borrowed_artifacts(node: &NodeExpr) -> bool {
    match node {
        NodeExpr::Single(call) => {
            call.input_kinds
                .iter()
                .any(|k| *k != ArtifactInputKind::Owned)
                || call.output_borrows.iter().any(|b| *b)
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

/// Parses the current graph syntax:
/// `#[metrics(...)] #[tests(...)] async MyGraph<Ctx>(inputs...) -> (outputs...) { ... }`
fn parse_graph_input(input: ParseStream) -> Result<GraphInput> {
    let mut context: Option<Path> = None;
    let mut graph_inputs: Vec<(Ident, Type)> = Vec::new();
    let mut graph_outputs: Vec<(Ident, Type)> = Vec::new();
    let mut async_enabled = false;
    let mut metrics = MetricsSpec::default();
    let mut tests: Vec<Path> = Vec::new();
    let mut attrs: Vec<syn::Attribute> = Vec::new();
    let mut tags: Vec<String> = Vec::new();
    let mut deprecated = false;
    let mut deprecated_reason: Option<String> = None;

    let outer_attrs = input.call(syn::Attribute::parse_outer)?;
    for attr in outer_attrs {
        if attr.path().is_ident("metrics") {
            let syn::Meta::List(list) = &attr.meta else {
                return Err(syn::Error::new_spanned(attr, "expected `#[metrics(...)]`"));
            };
            let parsed = syn::parse::Parser::parse2(parse_metrics_list, list.tokens.clone())?;
            metrics.performance |= parsed.performance;
            metrics.errors |= parsed.errors;
            metrics.count |= parsed.count;
            metrics.caller |= parsed.caller;
            metrics.success_rate |= parsed.success_rate;
            metrics.fail_rate |= parsed.fail_rate;
            continue;
        }
        if attr.path().is_ident("tests") {
            let syn::Meta::List(list) = &attr.meta else {
                return Err(syn::Error::new_spanned(attr, "expected `#[tests(...)]`"));
            };
            let list = syn::parse::Parser::parse2(
                syn::punctuated::Punctuated::<Path, Token![,]>::parse_terminated,
                list.tokens.clone(),
            )?;
            tests.extend(list.into_iter());
            continue;
        }
        if attr.path().is_ident("doc") {
            attrs.push(attr);
            continue;
        }
        if attr.path().is_ident("tags") {
            let syn::Meta::List(list) = &attr.meta else {
                return Err(syn::Error::new_spanned(
                    attr,
                    "expected `#[tags(\"a\", \"b\")]`",
                ));
            };
            let items = syn::parse::Parser::parse2(
                syn::punctuated::Punctuated::<syn::LitStr, Token![,]>::parse_terminated,
                list.tokens.clone(),
            )?;
            for item in items {
                let tag = item.value();
                let tag = tag.trim();
                if !tag.is_empty() {
                    tags.push(tag.to_string());
                }
            }
            continue;
        }
        if attr.path().is_ident("deprecated") {
            deprecated = true;
            match &attr.meta {
                syn::Meta::Path(_) => {}
                syn::Meta::NameValue(name_value) => {
                    if let syn::Expr::Lit(expr_lit) = &name_value.value {
                        if let syn::Lit::Str(lit) = &expr_lit.lit {
                            let value = lit.value().trim().to_string();
                            if !value.is_empty() {
                                deprecated_reason = Some(value);
                            }
                        }
                    }
                }
                syn::Meta::List(list) => {
                    let parsed = syn::parse::Parser::parse2(
                        syn::punctuated::Punctuated::<syn::MetaNameValue, Token![,]>::parse_terminated,
                        list.tokens.clone(),
                    );
                    if let Ok(items) = parsed {
                        for item in items {
                            if item.path.is_ident("note") {
                                if let syn::Expr::Lit(expr_lit) = &item.value {
                                    if let syn::Lit::Str(lit) = &expr_lit.lit {
                                        let value = lit.value().trim().to_string();
                                        if !value.is_empty() {
                                            deprecated_reason = Some(value);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            continue;
        }
        if attr.path().is_ident("metadata") {
            return Err(syn::Error::new_spanned(
                attr,
                "`#[metadata(...)]` is no longer supported for graphs; use `MyGraph<Context>` and the `async` keyword",
            ));
        }

        return Err(syn::Error::new_spanned(
            attr,
            "unsupported graph attribute; allowed: `/// doc`, `#[metrics(...)]`, `#[tests(...)]`",
        ));
    }

    if input.peek(Token![async]) {
        input.parse::<Token![async]>()?;
        async_enabled = true;
    }

    let name: Ident = input.parse()?;

    if input.peek(Token![<]) {
        input.parse::<Token![<]>()?;
        context = Some(input.parse()?);
        input.parse::<Token![>]>()?;
    }

    if input.peek(syn::token::Paren) {
        let typed;
        syn::parenthesized!(typed in input);
        graph_inputs = parse_typed_ident_list(&typed)?;
    }

    if input.peek(Token![->]) {
        input.parse::<Token![->]>()?;
        let typed;
        syn::parenthesized!(typed in input);
        graph_outputs = parse_typed_ident_list(&typed)?;
    }

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
                    "missing context type; graphs that borrow artifacts (e.g. `(&x)` or `-> (&x)`) must declare a context type like `MyGraph<Context>` that stores borrowed artifacts as fields",
                ));
            }
            parse_quote!(::graphium::Context)
        }
    };

    Ok(GraphInput {
        attrs,
        name,
        context,
        inputs: graph_inputs,
        outputs: graph_outputs,
        nodes,
        async_enabled,
        metrics,
        tests,
        tags,
        deprecated,
        deprecated_reason,
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
