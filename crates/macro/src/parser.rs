use syn::{
    Expr, Ident, Path, Result, Token, Type,
    parse::{Parse, ParseStream},
};

use crate::shared::{GraphInput, NodeCall, NodeExpr, RouteExpr};

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

/// Parses a single graph atom: either a node call or a `@route { ... }`
/// expression.
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
        let explicit_inputs = input.peek(syn::token::Paren);
        let inputs = if explicit_inputs {
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
            explicit_inputs,
            inputs,
            outputs,
        })
    }
}

/// Parses a comma-separated list of artifact names used for node inputs or
/// outputs in the graph DSL.
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
        })
    }
}

impl Parse for GraphInput {
    /// Parses the outer `graph!` object, including graph name, context type,
    /// and the bracketed graph schema.
    fn parse(input: ParseStream) -> Result<Self> {
        let mut name: Option<Ident> = None;
        let mut context: Option<Path> = None;
        let mut graph_inputs: Vec<(Ident, Type)> = Vec::new();
        let mut graph_outputs: Vec<(Ident, Type)> = Vec::new();
        let mut nodes: Option<NodeExpr> = None;

        while !input.is_empty() {
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
                _ => {
                    return Err(input.error(
                        "expected one of: `name`, `context`, `inputs`, `outputs`, `schema`",
                    ));
                }
            }

            input.parse::<Token![,]>().ok();
        }

        Ok(GraphInput {
            name: name.ok_or_else(|| input.error("missing `name`"))?,
            context: context.ok_or_else(|| input.error("missing `context`"))?,
            inputs: graph_inputs,
            outputs: graph_outputs,
            nodes: nodes.ok_or_else(|| input.error("missing `schema`"))?,
        })
    }
}
