# `node!` macro

`node! { ... }` turns a Rust function into a **Graphium node wrapper** with a uniform `run` / `run_async` interface.

Graphs defined with `graph!` do not call your function directly; they call the generated wrapper.

**See also**

- `index.md` (documentation map)
- `graphs.md` (how graphs orchestrate nodes)
- `dsl.md` (how nodes are composed with `>>`, `&&`, `@match`, loops, ...)
- `artifacts.md` (owned vs borrowed vs taken artifacts)

---

## Quickstart

Define a node:

```rust
use graphium_macro::node;

node! {
    fn get_number() -> u32 {
        42
    }
}
```

Use it inside a graph:

```rust
use graphium_macro::{graph, node};

node! { fn get_number() -> u32 { 42 } }
node! { fn inc(n: u32) -> u32 { n + 1 } }

graph! {
    MyGraph -> (out: u32) {
        GetNumber() -> (n) >>
        Inc(n) -> (out)
    }
}
```

---

## What `node!` generates

Given:

```rust
use graphium_macro::node;

node! { fn my_node(input: String) -> String { input } }
```

The macro expands into:

- Your original function (kept as-is)
- A wrapper struct named in PascalCase (`MyNode`)
- A stable entrypoint:
  - `MyNode::run(ctx, input)` for sync nodes
  - `MyNode::run_async(ctx, input).await` for async nodes
- Optional feature-gated metadata/helpers (metrics + UI/playground support)

The implementation is in `crates/macro/src/node_macro/*`.

---

## Signature rules (inputs, outputs, ctx)

### Inputs

Node inputs can be:

- Owned: `T`
- Shared borrow: `&T`
- Mutable borrow: `&mut T`

The `graph!` DSL decides whether an artifact is passed as owned, `&`, or `&mut`.
See `artifacts.md` for the artifact side of this contract.

### Context parameter (`ctx`)

If a node function declares a parameter named `ctx` or `_ctx`, it is treated as the graph context parameter **only if**:

- The parameter type is a reference: `&Context` or `&mut Context`
- There is at most one `ctx` parameter

Example:

```rust
use graphium_macro::node;

#[derive(Default)]
pub struct Ctx { pub counter: u32 }

node! {
    fn bump(ctx: &mut Ctx) {
        ctx.counter += 1;
    }
}
```

This rule is implemented in `crates/macro/src/node_macro/parse.rs` (`parse_node_def`).

### Return types must be owned (no references)

Node return types must be **owned**:

- `T` is OK
- `(A, B, C)` is OK
- `&T` is rejected
- `(&T, T)` is rejected

This is enforced during macro expansion so artifacts always have predictable move/clone semantics when the graph propagates them.
See `crates/macro/src/node_macro/parse.rs` (`validate_return_type`).

---

## How `node!` works internally

Start reading from:

- `crates/macro/src/node_macro/expand.rs` (entry point)
- `crates/macro/src/node_macro/parse.rs` (signature analysis)
- `crates/macro/src/ir.rs` (`NodeDef` IR used by expanders)

### 1) Parse: function signature → `NodeDef`

`node!` parses the function into a typed description (`NodeDef`) that includes:

- Wrapper struct name (PascalCase of the function name, unless overridden)
- Whether the node has a context parameter (`ctx`)
- Input names + types
- Return type + whether it is a `Result<Ok, Err>`
- Docs/tags/deprecation metadata (from attributes)

This parsing step is what makes later codegen deterministic and easy to test.

### 2) Validate: enforce constraints early

The macro panics at compile time for invalid node signatures, e.g.:

- Multiple `ctx` parameters
- `ctx` not being a reference
- Returning references

These are hard constraints because the `graph!` expander assumes the wrapper ABI is uniform and safe.

### 3) Expand: generate wrapper ABI

The wrapper’s job is to present a consistent “call surface” for graphs:

- Sync graphs call `NodeWrapper::run(ctx, inputs...)`
- Async graphs call `NodeWrapper::run_async(ctx, inputs...).await`

`graph!` never needs to care about the original function name or signature details beyond what’s embedded in the wrapper.

### 4) Optional tooling helpers

`node!` can emit additional metadata for UI/playground support:

- It serializes the raw macro input string (`input.to_string()`) for introspection and tooling.
- It can generate parsing helpers for simple primitive inputs/outputs for interactive “playground” execution.

These code paths are in `crates/macro/src/node_macro/expand.rs` and are typically consumed by `graphium-ui`.
