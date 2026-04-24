# Graphium Macro

Graphium Macro is the compile-time DSL compiler that transforms declarative workflow definitions into optimized Rust code.

This crate provides the core procedural macros (`graph!`, `node!`, `graph_test!`, `node_test!`) that enable you to define computation workflows using an ergonomic syntax while maintaining zero-runtime abstraction overhead.

All parsing, expansion, and graph construction happens at compile time. The generated code is fully optimized by LLVM, resulting in performance indistinguishable from hand-written Rust.

## Features

- **Declarative DSL**: Define complex workflows with minimal boilerplate using intuitive syntax
- **Compile-Time Expansion**: All graph logic resolved before runtime, enabling aggressive optimizations
- **Control Flow**: Sequential (`>>`), parallel (`&`), conditional (`@match`, `@if`), and looping constructs
- **Artifact Propagation**: Smart value handling with borrowing, moving, cloning, and copying semantics
- **Testing Macros**: `graph_test!` and `node_test!` for unit and integration testing
- **Async Support**: Native async/await support for asynchronous workflows
- **Type Safety**: Full compile-time type checking of graph connectivity and node contracts

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
graphium-macro = "0.1"
```

Define a simple graph:

```rust
use graphium_macro::graph;

graph! {
    MyGraph<Context> -> (result: u32) {
        NodeA() -> (value) >>
        NodeB(value) -> (result)
    }
}
```

## Documentation

See [Graphium](https://github.com/rottigni/graphium) for comprehensive documentation, examples, and guides.
