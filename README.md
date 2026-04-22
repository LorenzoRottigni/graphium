
# GRAPHIUM

<img src="https://s3.rottigni.tech/public/github/graphium/graphium_logo.png" alt="graphium" width="250px" height="250px" />

A Rust framework for defining observable DAG-based computation workflows through declarative procedural macros.

## The Idea

Represent any algorithm as a workflow graph using an ergonomic API, maintaining low abstraction costs.
The example above shows how Graphium, in a few lines, allows you to:

- Define the procedural flow of a linear regression model training pipeline in a childproof syntax.
- Define the propagation strategy of artifacts produced by nodes through a Rust-friendly syntax (smartly managing value borrowing, moving, cloning, and copying).
- Define sequential and parallel execution strategies for nodes using the `>>` and `&` tokens.
- Provide a custom Context struct that nodes can use.
- Define the output type of the graph.
- Define the set of metrics to be tracked for the graph or for specific nodes.

```rust
graph! {
    #[metadata(context = Context)]
    #[metrics("performance", "errors", "count", "caller", "success_rate", "fail_rate")]
    LinearRegressionGraph -> (model: Model) {
        GetDataset() -> (&dataset) >>
        ParseInputFeatures(&dataset) -> (input_features) & ParseOutputFeatures(&dataset) -> (output_features) >>
        TrainTestSplit(input_features, output_features) -> (&X_train, &X_test, &y_train, &y_test) >>
        Preprocessing(&X_train, &X_test, &y_train) -> (&X_train, &X_test, &y_train) >>
        InitModel(&X_train, &y_train) -> (&model, &X_train, &y_train) >>
        FitModel(&model, &X_train, &y_train) -> (&model) >>
        EvaluateModel(&model) -> (&model) >>
        ExportModel(&model) -> (model)
    }
}
```

### Result

- Visual representation of the graph workflow and its artifacts propagation:

<img src="https://s3.rottigni.tech/public/github/graphium/graphium_graph_hero.png" alt="graphium" width="800px" />

- Manually execute the graph, visualize graph configured metrics and its raw provided schema:

<img src="https://s3.rottigni.tech/public/github/graphium/graphium_graph_body.png" alt="graphium" width="800px" />

- Inspect graph's nodes in depth, run graph and nodes tests created using graph_test!{} and node_test!{} macros:

<img src="https://s3.rottigni.tech/public/github/graphium/graphium_graph_footer.png" alt="graphium" width="800px" />


### What else you can do?

- Graphs nesting by wiring inputs/outputs:

```rust
InnerGraph::run(a_split, b_split) -> (a_split, b_split)
```

- Rust-idiomatic conditional branching (and nested conditional branching):

```rust
graph! {
    #[metadata(context = Context)]
    ConditionalGraph {
        GetOperationStatus() -> (&status) >>
        @match ctx.status {
            Status::Success => OnSuccess(&status),
            Status::Fail => OnFail(),
            Status::Retry => OnRetry(),
        }
    }
}
```

- Conditional nodes execution:

```rust
graph! {
    #[metadata(context = Context)]
    IfGraph -> (result: u32) {
        GetOperationStatus() -> (status) >>
        @if |status: Status| status == Status::Success -> (result) {
            OnSuccess() -> (result)
        }
        @elif |status: Status| status == Status::Fail {
            OnFail() -> (result)
        }
        @else {
            OnRetry() -> (result)
        }
    }
}
```

- Loops:

```rust
graph! {
    #[metadata(context = Context)]
    WhileGraph {
        InitCtx() >>
        @while |ctx: &Context| ctx.a_number < 3 {
            IncCtx()
        }
    }
}
```

```rust
graph! {
    #[metadata(context = Context)]
    LoopBreakGraph {
        InitCtx() >>
        @loop {
            IncCtx() >>
            @if |ctx: &Context| ctx.a_number >= 3 {
                @break
            }
            @else {
                Noop()
            }
        }
    }
}
```

- Async graphs:

```rust
graph! {
    #[metadata(context = Context, async = true)]
    AsyncGraph -> (a_number: u32) {
        GetNumber() -> (a_number) >>
        AddOne(a_number) -> (a_number)
    }
}
```

- Node-scoped testing with test-related information available in the UI, along with a real-time playground.

```rust
node_test! {
    #[test]
    fn e2e_node_test_supports_standard_test_items() {
        let out = TestableAdd::__graphium_run(&(), 20, 22);
        assert_eq!(out, 42);
    }
}
```

- Graph-scoped testing with test-related information available in the UI, along with a real-time playground.

```rust
graph_test! {
    #[test]
    #[for_graph(TestableGraph)]
    fn e2e_graph_test_supports_standard_test_items() {
        let mut ctx = Context::default();
        let out = TestableGraph::__graphium_run(&mut ctx);
        assert_eq!(out, 42);
    }
}
```

### What's planned for the future?

- Provide an event-driven API:

```rust
node! {
    #[publish = EventName]
    async fn get_number() -> u32 {
        7
    }
}
```

- Allow explicit node naming:

```rust
node! {
    #[name = GetNumber]
    async fn get_number() -> u32 {
        7
    }
}
```

- Allow unpacking references to remove them from the context after they are read (once "Preprocessing" takes in X_train and X_test, it consumes them from the context):

```text
TrainTestSplit(input_features, output_features) -> (&X_train, &X_test, &y_train, &y_test) >>
Preprocessing(*X_train, *X_test) -> (X_train, X_test)
```
