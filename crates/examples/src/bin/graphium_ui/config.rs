use graphium::{graph, graph_test, node, node_test};
use graphium_ui::{config::GraphiumUiConfig, graphs};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Success,
    Retry,
    Fail,
}

#[derive(Clone, Debug, Default)]
struct Dataset {
    input: Vec<f32>,
    output: Vec<f32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Model {
    weight: f32,
    bias: f32,
}

#[derive(Default)]
#[allow(non_snake_case)]
struct Context {
    a_number: u32,
    attempts: u32,
    dataset: Dataset,
    X_train: Vec<f32>,
    X_test: Vec<f32>,
    y_train: Vec<f32>,
    y_test: Vec<f32>,
    model: Model,
}

node! {
    #[tests(GetNumberReturns42)]
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    fn get_number() -> u32 {
        42
    }
}

node! {
    #[tests(DuplicateClonesValue)]
    #[metrics("performance", "count", "caller")]
    fn duplicate(value: u32) -> (u32, u32) {
        (value, value)
    }
}

node! {
    #[metrics("performance", "count")]
    fn left_branch(value: u32) -> u32 {
        value + 1
    }
}

node! {
    #[metrics("performance", "count")]
    fn right_branch(value: u32) -> u32 {
        value + 2
    }
}

node! {
    #[metrics("performance", "count")]
    fn combine(left: u32, right: u32) -> u32 {
        left + right
    }
}

node! {
    #[metrics("performance", "count")]
    fn pipe_number(value: u32) -> u32 {
        value
    }
}

node! {
    #[metrics("performance", "count")]
    fn store_number(ctx: &mut Context, a_number: u32) {
        ctx.a_number = a_number;
    }
}

node! {
    #[metrics("performance", "count")]
    fn take_ownership(a_number: &u32) -> u32 {
        *a_number
    }
}

node! {
    #[metrics("performance", "count")]
    fn decide_status(sum: u32) -> (Status, u32) {
        if sum % 2 == 0 {
            (Status::Success, sum)
        } else if sum % 3 == 0 {
            (Status::Retry, sum)
        } else {
            (Status::Fail, sum)
        }
    }
}

node! {
    #[metrics("performance", "count")]
    fn on_success(sum: u32) -> u32 {
        sum * 2
    }
}

node! {
    #[metrics("performance", "count")]
    fn on_retry(sum: u32) -> u32 {
        sum + 5
    }
}

node! {
    #[metrics("performance", "count")]
    fn on_fail(sum: u32) -> u32 {
        sum
    }
}

node! {
    #[metrics("performance", "count")]
    fn init_attempts(ctx: &mut Context) {
        ctx.attempts = 0;
    }
}

node! {
    #[metrics("performance", "count")]
    fn bump_attempts(ctx: &mut Context) {
        ctx.attempts += 1;
    }
}

node! {
    #[metrics("performance", "count")]
    fn read_attempts(ctx: &Context) -> u32 {
        ctx.attempts
    }
}

node! {
    #[metrics("performance", "count")]
    fn status_from_attempt(attempt: u32) -> (Status, u32) {
        if attempt >= 3 {
            (Status::Success, attempt)
        } else {
            (Status::Retry, attempt)
        }
    }
}

node! {
    #[metrics("performance", "count")]
    fn route_success(attempt: u32) -> u32 {
        attempt * 10
    }
}

node! {
    #[metrics("performance", "count")]
    fn route_retry() -> u32 {
        0
    }
}

node! {
    #[metrics("performance", "count")]
    fn route_fail() -> u32 {
        999
    }
}

node! {
    /// this node returns a small, deterministic dataset for the UI demo. In a real graph, this could be a node that loads data from a file or database.
    #[metrics("performance", "count")]
    fn get_dataset() -> Dataset {
        // Small, deterministic dataset for the UI demo.
        let input: Vec<f32> = (0..20).map(|i| i as f32).collect();
        let output: Vec<f32> = input.iter().map(|x| 2.0 * x + 1.0).collect();
        Dataset { input, output }
    }
}

node! {
    #[metrics("performance", "count")]
    fn parse_input_features(dataset: &Dataset) -> Vec<f32> {
        dataset.input.clone()
    }
}

node! {
    #[metrics("performance", "count")]
    fn parse_output_features(dataset: &Dataset) -> Vec<f32> {
        dataset.output.clone()
    }
}

node! {
    #[metrics("performance", "count")]
    fn train_test_split(
        input_features: Vec<f32>,
        output_features: Vec<f32>,
    ) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
        let split = input_features.len().min(output_features.len()) / 2;
        let x_train = input_features[..split].to_vec();
        let x_test = input_features[split..].to_vec();
        let y_train = output_features[..split].to_vec();
        let y_test = output_features[split..].to_vec();
        (x_train, x_test, y_train, y_test)
    }
}

node! {
    #[metrics("performance", "count")]
    fn preprocessing(
        x_train: &Vec<f32>,
        x_test: &Vec<f32>,
        y_train: &Vec<f32>,
    ) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        (x_train.clone(), x_test.clone(), y_train.clone())
    }
}

node! {
    #[metrics("performance", "count")]
    fn init_model(x_train: &Vec<f32>, y_train: &Vec<f32>) -> (Model, Vec<f32>, Vec<f32>) {
        (Model::default(), x_train.clone(), y_train.clone())
    }
}

node! {
    #[metrics("performance", "count")]
    fn fit_model(model: &Model, x_train: &Vec<f32>, y_train: &Vec<f32>) {
        // UI demo only: this node intentionally does not mutate the shared model.
        let _ = (model, x_train.len(), y_train.len());
    }
}

node! {
    #[metrics("performance", "count")]
    fn evaluate_model(model: &Model) {
        let _ = model;
    }
}

node! {
    #[metrics("performance", "count")]
    fn export_model(model: &Model) -> Model {
        model.clone()
    }
}

graph! {
    #[metrics("performance", "count", "success_rate")]
    InnerGraph<'a, Context>(left: u32, right: u32) -> (left: u32, right: u32) {
        PipeNumber(left) -> (left) && PipeNumber(right) -> (right) >>
        LeftBranch(left) -> (left) && RightBranch(right) -> (right)
    }
}

graph! {
    #[metrics("performance", "count", "success_rate")]
    DeepInnerGraph<'a, Context>(left: u32, right: u32) -> (left: u32, right: u32) {
        InnerGraph::run(left, right) -> (left, right) >>
        PipeNumber(left) -> (left) && PipeNumber(right) -> (right)
    }
}

graph! {
    #[metrics("performance", "errors", "count", "caller", "success_rate", "fail_rate")]
    #[tests(OwnedGraphReturnsNonZeroSplit)]
    OwnedGraph<'a, Context> -> (a_split: u32) {
        GetNumber() -> (a_number) >>
        Duplicate(a_number) -> (left, right) >>
        LeftBranch(left) -> (left) && RightBranch(right) -> (right) >>
        DeepInnerGraph::run(left, right) -> (left, right) >>
        Combine(left, right) -> (sum) >>
        DecideStatus(sum) -> (status, sum) >>
        @if |status: Status| status == Status::Success -> (out) {
            OnSuccess(sum) -> (out)
        }
        @elif |status: Status| status == Status::Retry {
            OnRetry(sum) -> (out)
        }
        @else {
            OnFail(sum) -> (out)
        } >>
        PipeNumber(out) -> (a_split)
    }
}

graph! {
    #[metrics("performance", "count", "success_rate")]
    #[tests(BorrowedGraphKeepsOwnershipPath)]
    BorrowedGraph<'a, Context> -> (a_number: u32) {
        GetNumber() -> (a_number) >>
        StoreNumber(a_number) -> (&'a a_number) >>
        TakeOwnership(&'a a_number) -> (a_number) >>
        PipeNumber(a_number) -> (a_number)
    }
}

graph! {
    #[metrics("performance", "count", "success_rate")]
    #[tests(ControlFlowGraphConvergesToSuccessPath)]
    #[deprecated(note = "Testing deprecation")]
    ControlFlowGraph<'a, Context> -> (a_number: u32) {
        InitAttempts() >>
        @while |ctx: &Context| ctx.attempts < 3 {
            BumpAttempts()
        } >>
        ReadAttempts() -> (attempt) >>
        StatusFromAttempt(attempt) -> (status, attempt) >>
        @match status -> (result) {
            Status::Success => RouteSuccess(attempt) -> (result),
            Status::Retry => RouteRetry() -> (result),
            Status::Fail => RouteFail() -> (result),
        } >>
        PipeNumber(result) -> (a_number)
    }
}

graph! {
    /// This is a graph for training a linear regression model on a simple dataset,
    /// included as a demo of a more complex graph with multiple nodes and edges.
    /// The graph is not intended to produce a meaningful model, and intentionally
    /// includes redundant nodes and edges for demonstration purposes.
    #[metrics("performance", "errors", "count", "caller", "success_rate", "fail_rate")]
    #[tests(LinearRegressionGraphExportsDefaultModel)]
    #[tags("ml", "demo")]
    LinearRegressionGraph<'a, Context> -> (model: Model) {
        GetDataset() -> (&'a dataset) >>
        ParseInputFeatures(&'a dataset) -> (input_features)
            && ParseOutputFeatures(&'a dataset) -> (output_features) >>
        TrainTestSplit(input_features, output_features) -> (&'a X_train, &'a X_test, &'a y_train, &'a y_test) >>
        Preprocessing(&'a X_train, &'a X_test, &'a y_train) -> (&'a X_train, &'a X_test, &'a y_train) >>
        InitModel(&'a X_train, &'a y_train) -> (&'a model, &'a X_train, &'a y_train) >>
        FitModel(&'a model, &'a X_train, &'a y_train) -> (&'a model) >>
        EvaluateModel(&'a model) -> (&'a model) >>
        ExportModel(&'a model) -> (model)
    }
}

pub fn config() -> GraphiumUiConfig {
    GraphiumUiConfig {
        bind: std::env::var("GRAPHIUM_UI_BIND")
            .unwrap_or_else(|_| "127.0.0.1:4000".to_string())
            .parse()
            .unwrap_or_else(|_| "127.0.0.1:4000".parse().expect("valid default bind")),
        prometheus_url: std::env::var("GRAPHIUM_PROMETHEUS_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:9090".to_string()),
        loki_url: std::env::var("GRAPHIUM_LOKI_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:3100".to_string()),
        tempo_url: std::env::var("GRAPHIUM_TEMPO_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:3200".to_string()),
        graphs: graphs![
            LinearRegressionGraph,
            OwnedGraph,
            BorrowedGraph,
            ControlFlowGraph,
            DeepInnerGraph
        ],
        ..Default::default()
    }
}

node_test! {
    #[test]
    fn get_number_returns_42() {
        let value = GetNumber::run(&());
        assert_eq!(value, 42);
    }
}

node_test! {
    #[test]
    fn duplicate_clones_value() {
        let (left, right) = Duplicate::run(&(), 7);
        assert_eq!((left, right), (7, 7));
    }
}

graph_test! {
    #[test]
    fn owned_graph_returns_non_zero_split(graph: &OwnedGraph, threshold: u32) {
        let mut ctx = Context::default();
        let out = graph::run(&mut ctx);
        assert!(out > threshold);
    }
}

graph_test! {
    #[test]
    fn borrowed_graph_keeps_ownership_path() {
        let mut ctx = Context::default();
        let out = BorrowedGraph::run(&mut ctx);
        assert_eq!(out, 42);
    }
}

graph_test! {
    #[test]
    fn control_flow_graph_converges_to_success_path() {
        let mut ctx = Context::default();
        let out = ControlFlowGraph::run(&mut ctx);
        assert_eq!(out, 30);
    }
}

graph_test! {
    #[test]
    fn linear_regression_graph_exports_default_model() {
        let mut ctx = Context::default();
        let out = LinearRegressionGraph::run(&mut ctx);
        assert_eq!(out, Model::default());
    }
}
