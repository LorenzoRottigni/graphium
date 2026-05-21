# Control flow (`@match`, `@if`, `@while`, `@loop`, `@break`)

This document is the “examples + gotchas” companion to `dsl.md`.

`dsl.md` describes the syntax and where it is implemented; this file focuses on:

- Patterns you can use in real graphs
- Why control-flow atoms require declared outputs
- How artifacts interact with branching and looping

Related docs:

- `dsl.md` (operators + grammar)
- `artifacts.md` (owned vs borrowed vs taken)
- `async.md` (async graphs caveats)

---

## Why Graphium control flow is strict

Graphium must generate **typed Rust code** from your schema at compile time.

That means control-flow constructs need a statically checkable contract:

- A route must know what artifacts it may produce regardless of which branch is chosen.
- A loop must know what artifacts are carried/produced across iterations and what exits the loop.

So Graphium uses “declared outputs” on control-flow atoms and validates them at compile time.

---

## `@match`

### Shape

```text
@match selector_expr -> (outputs...) {
  cond1 => expr1
  cond2 => expr2
  _     => expr_default
}
```

### Example: route to different pipelines but converge to same output

```rust
use graphium_macro::{graph, node};

#[derive(Default)]
pub struct Context { pub mode: u32 }

node! { fn mode(ctx: &Context) -> u32 { ctx.mode } }
node! { fn fast() -> u32 { 1 } }
node! { fn slow() -> u32 { 10 } }
node! { fn finish(x: u32) -> u32 { x + 1 } }

graph! {
    Router<Context> -> (out: u32) {
        Mode() -> (m) >>
        @match m -> (x) {
            0 => Fast() -> (x)
            _ => Slow() -> (x)
        } >>
        Finish(x) -> (out)
    }
}
```

Key rule:

- Each branch must produce `x` (owned in this example). If a branch forgets to produce it, compilation fails.

### Declared outputs with borrowed persistence

Routes can also declare borrowed outputs (persisting into graph storage), but remember:

- Borrowed outputs mean “this artifact is persisted in the graph” (owned value stored, later borrowed).
- Taking with `*...` consumes that stored value.

See `artifacts.md` for the details.

---

## `@if` (if-chain)

Graphium supports `@if` chains (parsed as a route internally).

Pattern: choose a branch based on a boolean condition, but keep output contract identical.

```rust
use graphium_macro::{graph, node};

node! { fn compute() -> u32 { 10 } }
node! { fn inc(x: u32) -> u32 { x + 1 } }
node! { fn dec(x: u32) -> u32 { x - 1 } }

graph! {
    IfChain -> (out: u32) {
        Compute() -> (x) >>
        @if x > 10 -> (y) {
            Inc(x) -> (y)
        } else -> (y) {
            Dec(x) -> (y)
        } >>
        Inc(y) -> (out)
    }
}
```

The same “declared outputs” rule applies: both branches must agree.

---

## `@while`

### Shape

```text
@while condition_expr -> (declared_outputs...) { body_expr }
```

Graphium validates that the loop body produces exactly what the loop claims to output.

### Pattern: accumulate in context, produce final artifact

Loops are typically easiest when you keep long-lived mutable state in `ctx` and only use artifacts for step inputs/outputs.

```rust
use graphium_macro::{graph, node};

#[derive(Default)]
pub struct Context { pub i: u32 }

node! { fn inc(ctx: &mut Context) { ctx.i += 1; } }
node! { fn read(ctx: &Context) -> u32 { ctx.i } }

graph! {
    LoopInCtx<Context> -> (out: u32) {
        @while ctx.i < 3 {
            Inc()
        } >>
        Read() -> (out)
    }
}
```

Notes:

- Conditions are Rust expressions parsed by `syn` (they can reference `ctx` and/or artifacts, depending on how they’re written).
- If the condition expression requires artifacts, those artifacts must be present at the loop entry.

---

## `@loop` + `@break`

`@loop` is an unconditional loop. Use `@break` to exit.

### Pattern: loop + route to break on condition

```rust
use graphium_macro::{graph, node};

#[derive(Default)]
pub struct Context { pub i: u32 }

node! { fn inc(ctx: &mut Context) { ctx.i += 1; } }
node! { fn read(ctx: &Context) -> u32 { ctx.i } }

graph! {
    LoopBreak<Context> -> (out: u32) {
        @loop {
            Inc() >>
            @if ctx.i >= 3 {
                @break
            }
        } >>
        Read() -> (out)
    }
}
```

Important rules:

- `@break` is only valid inside `@loop` or `@while`.
- Parallel groups inside loops may be constrained (sync graphs avoid thread-parallel execution when a loop contains `@break`).

See `crates/macro/src/graph_macro/expr/dispatch.rs` (`break_outside_loop_panics`) and `expr/parallel.rs` for the parallel/loop interaction.

---

## Control flow + artifacts: practical advice

- Prefer `ctx` for loop-carried state. Artifacts shine for “flow between steps”, not for “mutable loop state”.
- When routing, converge branches by producing the same artifact names (even if values come from different pipelines).
- Use borrowed persistence (`-> (&x)`) when multiple downstream steps need access without consuming the value.
- Avoid persisting large values unless you need shared access; persistence implies cloning/storing an owned value in graph-local storage.

---

## Where it is implemented

If you want to follow the exact code paths:

- Parsing: `crates/macro/src/graph_macro/parse.rs`
- Analysis/validation:
  - `crates/macro/src/graph_macro/analysis.rs`
  - `crates/macro/src/graph_macro/expr/route.rs`
  - `crates/macro/src/graph_macro/expr/loops.rs`
- Expansion:
  - `crates/macro/src/graph_macro/expr/*`

