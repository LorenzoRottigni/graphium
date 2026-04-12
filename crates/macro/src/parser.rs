use syn::{
    Expr, Ident, Path, Result, Token,
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
