# Graphium DSL (`graph!` body)

This document describes the **Graphium DSL** used inside `graph! { ... }` bodies: operators, control-flow atoms, and the rules that make a schema valid.

Related docs:

- `graphs.md` for the outer `graph!` signature and runtime entrypoints
- `nodes.md` for how `node!` wrappers are called
- `artifacts.md` for artifact ownership (`owned` / `&borrowed` / `*taken`)

The DSL is parsed and expanded entirely at compile time by `graphium-macro`.

---

## Mental model

A `graph!` body is parsed into an expression tree (`NodeExpr`) and expanded into hop-by-hop Rust orchestration code.

Key consequences:

- Artifact names are *schema identifiers*, not normal Rust variables.
- Operator precedence is fixed by the parser: **parallel groups bind tighter than sequencing**.
- Control-flow constructs must declare outputs in a way that can be validated statically.

The parser lives in `crates/macro/src/graph_macro/parse.rs`.

---

## Composition operators

### Sequencing: `>>`

`A() >> B()` means “run `A`, then run `B`”.

In the IR this becomes `NodeExpr::Sequence([A, B])`.

### Parallel groups: `&&`

`A() && B() && C()` means “these are siblings in a parallel group”.

In the IR this becomes `NodeExpr::Parallel([A, B, C])`.

Important rules:

- A parallel group cannot produce duplicate artifact names across its branches.
  This is enforced during analysis/codegen (see `crates/macro/src/graph_macro/analysis.rs` `collect_parallel_outputs`).
- Owned artifacts consumed by multiple branches are cloned (fan-out) when needed.

### Precedence

The parser reads the body as:

1. Split into a sequence on `>>`
2. For each sequence item, split into parallel siblings on `&&`

So:

```text
A() && B() >> C()
```

parses as:

```text
(A() && B()) >> C()
```

---

## Node calls

The basic atom is a node call:

```text
SomeNode(inputs...) -> (outputs...)
```

- Inputs are artifact identifiers (optionally marked as `&...`, `&mut ...`, `*...`, `*mut ...`)
- Outputs are artifact identifiers (optionally marked as `&...` / `&mut ...` to persist into graph storage)

Examples:

```rust
GetNumber() -> (n)
Inc(n) -> (n)
Store(n) -> (&n)
Read(&n) -> (out)
Take(*n) -> (out)
```

The input/output parsing rules are implemented in `crates/macro/src/graph_macro/parse.rs` (`parse_input_ident_list` and `parse_output_ident_list`).

For ownership semantics, see `artifacts.md`.

---

## Control-flow atoms (`@...`)

Control-flow atoms always start with `@` and expand into dedicated `NodeExpr` variants.

### `@match`

Shape:

```text
@match <selector_expr> -> (<declared_outputs>) { <routes> }
```

Each route maps a condition to an expression subtree:

```text
<condition> => <expr>
```

Why outputs are declared:

- Only one branch executes at runtime, but the macro must still know what artifacts may exit the `@match`.
- The expander validates that every branch produces the declared outputs.

Validation and exit-shape computation lives under:

- `crates/macro/src/graph_macro/analysis.rs`
- `crates/macro/src/graph_macro/expr/route.rs`

### `@if` (if-chain)

Graphium supports an `@if ... { ... } else if ... { ... } else { ... }`-style chain.
Internally it is represented as a `RouteExpr` (same idea as `@match`).

This is parsed in `crates/macro/src/graph_macro/parse.rs` and validated/expanded via the same route machinery.

### `@while`

Shape:

```text
@while <condition_expr> -> (<declared_outputs>) { <body_expr> }
```

Important rules:

- Loop bodies are validated against the declared outputs.
- Borrowed artifacts required by the condition must be available at the loop entry.

See `crates/macro/src/graph_macro/expr/loops.rs` for validation and expansion.

### `@loop` and `@break`

`@loop` is an unconditional loop:

```text
@loop -> (<declared_outputs>) { <body_expr> }
```

`@break` is a control-flow atom that exits the nearest loop in the IR.

Both are parsed in `crates/macro/src/graph_macro/parse.rs` and expanded in `expr/loops.rs`.

---

## Declared outputs: why the DSL is “static”

In plain Rust, branches and loops can produce different values dynamically.
In Graphium, the macro must produce *typed* Rust code, so the schema must be statically well-formed.

That’s why control-flow atoms (routes/loops) carry declared output lists and the macro checks:

- All required outputs are produced
- No extra outputs escape a loop body when the loop signature doesn’t declare them

If a contract is violated, the macro panics during compilation with an error message that points at the missing/extra artifact.

---

## Where the “rules” live (source map)

If you want to understand exactly how a DSL schema becomes Rust code, start here:

- `crates/macro/src/graph_macro/parse.rs`: DSL tokens → IR (`NodeExpr`, `NodeCall`, `RouteExpr`, loops)
- `crates/macro/src/graph_macro/analysis.rs`: compute entry requirements + exit artifacts for validation
- `crates/macro/src/graph_macro/expr/single.rs`: move/clone/borrow/take semantics for a single node call
- `crates/macro/src/graph_macro/execution.rs`: build `run`/`run_async` bodies (root payload + borrowed slots + return)
- `crates/macro/src/ir.rs`: all IR types used by parsing and expansion

