# GRAPHIUM

<img src="https://s3.rottigni.tech/public/github/graphium/graphium_logo.png" alt="graphium" width="250px" height="250px" />

Graphium is a Rust framework for defining observable DAG-based computation workflows through a declarative, Rust-friendly DSL.

It allows developers to express computation as composable algorithmic graphs, where logic is modeled as directed acyclic graphs (DAGs) and expanded into Rust code at compile time.

The framework focuses on **zero-cost abstractions**: DSL parsing and graph expansion happen entirely at compile time, and the generated code is further optimized by LLVM.

Runtime behavior is optional and controlled via feature flags. Using Graphium with only the `"macro"` feature results in near-zero runtime overhead, since all computation is resolved at compile time.

---

## Architecture

Graphium separates concerns into two phases:

- **Compile time**
  - DSL parsing
  - Graph expansion
  - Code generation

- **Runtime (optional)**
  - Graph and node metrics via the `"metrics"` feature flag
  - Graph and node serialization via the `"export"` feature flag, using a `DTO` contract (required by `graphium-ui`)
  - Interactive graph playground via the `"playground"` feature flag, enabling execution of graphs directly from `graphium-ui`
  - Additional runtime features planned

The goal of this design is to clearly separate a zero-cost production core from optional runtime capabilities.

A minimal production deployment can rely only on the `"macro"` feature flag, resulting in near-zero runtime overhead. Additional features can then be enabled selectively to introduce observability, serialization, or interactive tooling as needed.

---

## Crates

Graphium is organized into multiple crates, each responsible for a specific layer of the system:

- **core**
  - Contains runtime contracts, primitives, and shared abstractions
  - Defines DTOs, metrics, and execution-related types
  - Provides foundational types used by macro-generated code

- **macro**
  - Implements the DSL and procedural macros
  - Responsible for parsing Graphium DSL constructs (`graph! {}`, `node! {}`, `graph_test! {}`, `node_test! {}`)
  - Generates Rust code at compile time based on graph definitions and feature flags

- **ui**
  - Web-based visualization and interaction layer built with Axum, HTMX, and Alpine.js
  - Allows inspection of graphs, nodes, and tests
  - Supports on-demand execution of graphs

- **examples**
  - Internal crate containing reference implementations and usage examples
  - Demonstrates how Graphium is intended to be used in real-world scenarios

---

## Use cases

Graphium can be used to model computation at different levels of abstraction, depending on application needs:

### Low-level graphs

Nodes represent primitive functions:

```rust
add(a: u32, b: u32) -> u32
```

At this level, graphs act as explicit function pipelines:

```rust
graph! {
    TransformGraph(a: u32, b: u32) -> u32 {
        Add(a, b) -> (c),
        Pow(c) -> (d)
    }
}
```

---

### Mid-level graphs

Nodes represent domain-specific operations:

```rust
fit_model(model: &LinearRegressionModel)
```

Graphs such as `LinearRegressionGraph` orchestrate training and evaluation pipelines.

DSL definition:

```rust
graph! {
    #[metrics("performance", "errors", "count", "caller", "success_rate", "fail_rate")]
    LinearRegressionGraph<'a, Context> -> (model: Model) {
        GetDataset() -> (&'a dataset) >>
        ParseInputFeatures(&'a dataset) -> (input_features) &&
        ParseOutputFeatures(&'a dataset) -> (output_features) >>
        TrainTestSplit(input_features, output_features) -> (&'a X_train, &'a X_test, &'a y_train, &'a y_test) >>
        Preprocessing(&'a mut X_train, *'a mut X_test, *'a mut y_train) >>
        InitModel(&'a X_train, &'a y_train) -> (&'a model) >>
        FitModel(&'a mut model, *'a X_train, *'a y_train) >>
        EvaluateModel(&'a model) >>
        ExportModel(*'a model) -> (model)
    }
}
```

---

### High-level graphs

Entire applications can be expressed as graphs:

```rust
AxumEcommerce
```

Where nested graphs handle application workflows such as:

- `CreateProduct`
  - `validate_product(dto: &CreateProductDto)`
  - `check_product_exists(dto: &CreateProductDto)`

Application-level DSL:

```rust
graph! {
    AxumEcommerce<'a, Context> {
        RouterInit >>
        RegisterProductController >>
        RegisterUserController >>
        RegisterOrderController
    }
}
```

Nested execution graphs:

```rust
graph! {
    async CreateProductGraph<'a, Context>(name: String, price: String) -> (product_dto: Json<Product>) {
        GetProductInput(name, price) -> &'a product_input >>
        ValidateProductInputData(&'a product_input) &&
        CheckProductDoesNotExist(&'a product_input) >>
        ProductCreate(*'a product_input) -> product >>
        SerializeProduct(product) -> product_dto
    }
}
```

This allows a developer to define end-to-end application flows that can be executed directly or exposed through the UI.

---

Graphium is designed to be flexible: it adapts to the level of abstraction that best fits the application, from low-level computation pipelines to full application systems and hybrid architectures combining multiple execution layers.

---

## DSL (Domain Specific Language)

Graphium provides a proprietary DSL for defining and executing graph-based workflows.  
It borrows syntax from Rust while redefining specific operators and annotations to express execution flow, data dependency, and artifact lifecycle.

---

### 1. Execution model

Graph execution is expressed as a directed computation graph where nodes represent transformations and edges define execution strategy.

#### Serial execution

- **Operator:** `>>` (bitwise right shift)
- **Meaning:** executes the right-hand node after the left-hand node completes

```rust
A() >> B()
```

#### Parallel execution

- **Operator:** `&&` (logical AND)
- **Meaning:** executes nodes concurrently when dependencies allow it

```rust
A() && B() && C()
```

---

### 2. Async graphs

- **Keyword:** `async`
- **Meaning:** enables asynchronous execution of the entire graph via `run_async()`

When a graph is marked `async`, all eligible nodes may be scheduled concurrently based on dependency resolution.

```rust
async graph! {
    ...
}
```

---

### 3. Artifact lifecycle & ownership system

Graphium introduces a **lifetime-based artifact system**, inspired by Rust lifetimes but used as a runtime-managed abstraction for graph-scoped data.

#### 3.1 Lifetime annotations

- **Syntax:** `<'a>`
- **Meaning:** defines a scoped artifact lifetime within the graph execution

Lifetimes control:

- when artifacts are accessible
- when they can be mutated
- when they are dropped

> Note: lifetimes are not Rust compiler lifetimes, but runtime graph-scoped identifiers.

---

#### 3.2 References

- **Operator:** `&`
- **Meaning:** borrows an artifact from a given lifetime without taking ownership

```rust
-> (&'a dataset)
```

or

```rust
process(&'a dataset)
```

Artifacts remain owned by the graph lifetime and can be reused by subsequent nodes.

---

#### 3.3 Mutability

- **Keyword:** `mut`
- **Meaning:** grants mutable access to a borrowed artifact

```rust
(&'a mut X_train)
```

Mutability allows in-place transformation of shared artifacts within the same lifetime scope.

---

#### 3.4 Ownership transfer (move semantics)

- **Operator:** `*`
- **Meaning:** consumes an artifact from a lifetime and moves ownership into the node scope

```rust
process(*'a model)
```

This transfers the value out of the graph-managed lifetime into the executing node.

---

### 4. Context system

- **Annotation:** `Context`
- **Meaning:** defines the type of shared execution context available to all nodes in the graph

```rust
Graph<'a, Context>
```

The context is globally accessible to nodes and typically carries configuration, services, or runtime state.

---

## Big plans on the way

### Increasing flexibility by simplifying architecture

#### Current status

`graphium-macro` parses the procedural macro DSL into an internal representation (IR) based on tokens, relying on compile-time dependencies such as `syn`.

Based on this IR, the macro expands into Rust code. The current flow is:

```txt
DSL → IR → codegen
```

Key limitations:

- Works exclusively at compile time through procedural macros  
- Tightly coupled to Rust  
- The IR is compilation-driven, token-based, and implementation-specific  

---

#### Planned refactor

The goal is to clearly separate responsibilities between `graphium-macro` and `graphium-core`.

- `graphium-core` will provide a universal contract to manage:
  - graph and node state  
  - execution flow abstraction  
  - artifacts
  - serialization
  - shared logic for code generation  

  This contract will be represented by DTOs.

- DTOs already model entities (graphs, nodes, etc.). The plan is to extend them with helper methods for generic code generation across multiple targets.

  Focusing on four targets:

  1. **Rust + compile-time**  
     DTO helpers operate on raw tokens (input and output are token streams).

  2. **Rust + runtime**  
     DTO helpers operate on an ABI-like interface, orchestrating execution based on the schema.

  3. **TypeScript + compile-time**  
     DTO helpers operate on `ts.factory` AST nodes and produce a `SourceFile`.

  4. **TypeScript + runtime**  
     DTO helpers generate JavaScript code that orchestrates execution based on the schema.

- `graphium-macro` will be simplified to:
  1. Parse the DSL into `graphium-core` DTOs  
  2. Delegate code generation to DTO helpers  

New flow:

```txt
DSL → DTO → codegen
```

Key benefits:

- Enables both compile-time and runtime execution models  
- Opens support for multiple programming languages  
- Improves separation of concerns  
- Establishes a more universal and extensible architecture  

Choosing runtime execution might open new doors:

- dynamic composition
- remote execution over network
- UI-driven construction
- hot reloading
- plugin systems

---

### OpenTelemetry integration

Graphium integrates with OpenTelemetry to collect metrics, traces, and logs through a shared layer running outside the main execution thread.

Currently, Prometheus requires exposing a metrics endpoint for scraping, which assumes a network-accessible service. By using OpenTelemetry, metrics can be pushed to Prometheus-compatible backends without exposing public endpoints.

---

### Optional EDD (Event-Driven Design)

Graphium will provide an optional EDD API, allowing nodes and graphs to emit events during execution. These events can trigger other nodes or graphs.

```rust
node! {
    #[publish = EventName]
    async fn get_number() -> u32 {
        7
    }
}
```

## Graphium UI

<img src="https://s3.rottigni.tech/public/github/graphium/graphium_graph_hero.png" alt="graphium" width="800px" />

<img src="https://s3.rottigni.tech/public/github/graphium/graphium_graph_body.png" alt="graphium" width="800px" />

<img src="https://s3.rottigni.tech/public/github/graphium/graphium_graph_footer.png" alt="graphium" width="800px" />
