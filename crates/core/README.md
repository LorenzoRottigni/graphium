# Graphium Core

Graphium Core provides the runtime execution engine and data structures that power Graphium workflows.

This crate handles graph execution, metrics collection, and observable workflow state management. It works seamlessly with compiled graphs from `graphium-macro`, offering optional runtime features that can be enabled via feature flags without sacrificing performance.

The core design prioritizes minimal abstraction overhead. Execution contracts are defined at compile time through macro expansion, allowing LLVM to perform aggressive optimizations on generated code. Runtime behavior is entirely optional and controlled by feature selection.

## Features

- **Compile-Time Graph Definition**: Leverage the macro system for zero-runtime parsing overhead
- **Observable Execution**: Comprehensive metrics and execution tracing built-in
- **Feature-Gated Runtime**: Optional runtime abstractions enabled only when needed
- **Context Management**: Flexible context passing for stateful computations
- **Error Handling**: Structured error types with detailed diagnostics
- **Type-Safe Artifacts**: Compile-time verification of artifact flow through graph nodes

## Architecture

Graphium Core operates in two modes:

- **Macro Mode** (default): Graphs are fully expanded at compile time with zero runtime overhead
- **Runtime Mode** (feature-gated): Optional runtime controller for dynamic graph orchestration

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
graphium-core = "0.1"
graphium-macro = "0.1"
```

Define and run a graph:

```rust
use graphium_macro::graph;

#[derive(Default)]
struct Context;

graph! {
    MyGraph<Context> -> (result: u32) {
        ComputeValue() -> (value) >>
        ProcessResult(value) -> (result)
    }
}

fn main() {
    let mut ctx = Context::default();
    let result = MyGraph::run(&mut ctx);
    println!("Result: {}", result);
}
```

## Roadmap

Planned additions to Core:

- Runtime `Controller` for dynamic workflow orchestration
- Explicit node naming and lifecycle hooks
- Event-driven node publish/subscribe patterns
- Reference unpacking for context cleanup

## Documentation

See [Graphium](https://github.com/rottigni/graphium) for comprehensive documentation, examples, and guides.
