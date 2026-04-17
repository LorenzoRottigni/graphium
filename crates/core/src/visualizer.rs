#[derive(Clone, Debug)]
pub struct GraphDef {
    pub name: &'static str,
    pub inputs: Vec<&'static str>,
    pub outputs: Vec<&'static str>,
    pub steps: Vec<GraphStep>,
}

#[derive(Clone, Debug)]
pub struct GraphCase {
    pub label: &'static str,
    pub steps: Vec<GraphStep>,
}

#[derive(Clone, Debug)]
pub enum GraphStep {
    Node {
        name: &'static str,
        inputs: Vec<&'static str>,
        outputs: Vec<&'static str>,
    },
    Nested {
        graph: Box<GraphDef>,
        inputs: Vec<&'static str>,
        outputs: Vec<&'static str>,
    },
    Parallel {
        branches: Vec<Vec<GraphStep>>,
        inputs: Vec<&'static str>,
        outputs: Vec<&'static str>,
    },
    Route {
        on: &'static str,
        cases: Vec<GraphCase>,
        inputs: Vec<&'static str>,
        outputs: Vec<&'static str>,
    },
    While {
        condition: &'static str,
        body: Vec<GraphStep>,
        inputs: Vec<&'static str>,
        outputs: Vec<&'static str>,
    },
    Loop {
        body: Vec<GraphStep>,
        inputs: Vec<&'static str>,
        outputs: Vec<&'static str>,
    },
    Break,
}

pub trait GraphDefProvider {
    fn graph_def() -> GraphDef;
}

#[derive(Clone, Copy, Debug)]
pub struct PlaygroundParam {
    pub name: &'static str,
    pub ty: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct PlaygroundSchema {
    pub inputs: &'static [PlaygroundParam],
    pub outputs: &'static [PlaygroundParam],
    pub context: &'static str,
}

/// Optional UI integration for executing a graph from a web form.
///
/// This is primarily intended for local/dev tooling (Graphium UI).
pub trait GraphPlayground: GraphDefProvider {
    /// Whether this graph can be executed by the generic UI playground runner.
    const PLAYGROUND_SUPPORTED: bool;

    fn playground_schema() -> PlaygroundSchema;

    fn playground_run(
        form: &std::collections::HashMap<String, String>,
    ) -> Result<String, String>;
}

pub struct Visualizer;

impl Visualizer {
    pub fn new() -> Self {
        Self
    }

    pub fn print<G: GraphDefProvider>(&self, _graph: G) {
        let def = G::graph_def();
        self.print_def(&def);
    }

    pub fn print_def(&self, graph: &GraphDef) {
        println!("{}", graph.name);
        self.print_steps(&graph.steps, "");
    }

    fn print_steps(&self, steps: &[GraphStep], prefix: &str) {
        let count = steps.len();
        for (idx, step) in steps.iter().enumerate() {
            let is_last = idx + 1 == count;
            let branch = if is_last { "└─" } else { "├─" };
            match step {
                GraphStep::Node {
                    name,
                    inputs,
                    outputs,
                } => {
                    println!("{}{}{}{}", prefix, branch, name, format_io(inputs, outputs));
                }
                GraphStep::Nested {
                    graph,
                    inputs,
                    outputs,
                } => {
                    println!(
                        "{}{}{}{}",
                        prefix,
                        branch,
                        graph.name,
                        format_io(inputs, outputs)
                    );
                    let next_prefix = if is_last {
                        format!("{}  ", prefix)
                    } else {
                        format!("{}│ ", prefix)
                    };
                    self.print_steps(&graph.steps, &next_prefix);
                }
                GraphStep::Parallel { branches, .. } => {
                    println!("{}{}@parallel", prefix, branch);
                    let next_prefix = if is_last {
                        format!("{}  ", prefix)
                    } else {
                        format!("{}│ ", prefix)
                    };
                    for (b_idx, branch_steps) in branches.iter().enumerate() {
                        let b_last = b_idx + 1 == branches.len();
                        let branch_label = if b_last { "└─" } else { "├─" };
                        println!("{}{}branch", next_prefix, branch_label);
                        let branch_prefix = if b_last {
                            format!("{}  ", next_prefix)
                        } else {
                            format!("{}│ ", next_prefix)
                        };
                        self.print_steps(branch_steps, &branch_prefix);
                    }
                }
                GraphStep::Route { on, cases, .. } => {
                    println!("{}{}@match {}", prefix, branch, on);
                    let next_prefix = if is_last {
                        format!("{}  ", prefix)
                    } else {
                        format!("{}│ ", prefix)
                    };
                    for (c_idx, case) in cases.iter().enumerate() {
                        let c_last = c_idx + 1 == cases.len();
                        let case_label = if c_last { "└─" } else { "├─" };
                        println!("{}{}{}", next_prefix, case_label, case.label);
                        let case_prefix = if c_last {
                            format!("{}  ", next_prefix)
                        } else {
                            format!("{}│ ", next_prefix)
                        };
                        self.print_steps(&case.steps, &case_prefix);
                    }
                }
                GraphStep::While { condition, body, .. } => {
                    println!("{}{}@while {}", prefix, branch, condition);
                    let next_prefix = if is_last {
                        format!("{}  ", prefix)
                    } else {
                        format!("{}│ ", prefix)
                    };
                    self.print_steps(body, &next_prefix);
                }
                GraphStep::Loop { body, .. } => {
                    println!("{}{}@loop", prefix, branch);
                    let next_prefix = if is_last {
                        format!("{}  ", prefix)
                    } else {
                        format!("{}│ ", prefix)
                    };
                    self.print_steps(body, &next_prefix);
                }
                GraphStep::Break => {
                    println!("{}{}@break", prefix, branch);
                }
            }
        }
    }
}

fn format_io(inputs: &[&'static str], outputs: &[&'static str]) -> String {
    if inputs.is_empty() && outputs.is_empty() {
        return String::new();
    }

    let mut formatted = String::new();
    formatted.push_str(" [");

    if !inputs.is_empty() {
        formatted.push_str("in: ");
        formatted.push_str(&inputs.join(", "));
    }

    if !outputs.is_empty() {
        if !inputs.is_empty() {
            formatted.push_str(" | ");
        }
        formatted.push_str("out: ");
        formatted.push_str(&outputs.join(", "));
    }

    formatted.push(']');
    formatted
}
