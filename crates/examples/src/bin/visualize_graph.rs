use graphium::{graph, node};

#[derive(Default)]
struct Context;

node! {
    fn get_number() -> u32 {
        0
    }
}

node! {
    fn duplicate(value: u32) -> (u32, u32) {
        (value, value)
    }
}

node! {
    fn pipe_number(value: u32) -> u32 {
        value
    }
}

node! {
    fn inner_start(a_split: u32, b_split: u32) -> (u32, u32) {
        (a_split, b_split)
    }
}

node! {
    fn inner_finish(a_split: u32, b_split: u32) -> (u32, u32) {
        (a_split, b_split)
    }
}

node! {
    fn left_branch(value: u32) -> u32 {
        value
    }
}

node! {
    fn right_branch(value: u32) -> u32 {
        value
    }
}

#[derive(Clone, Copy)]
enum Status {
    Success,
    Fail,
}

graph! {
    InnerGraph<Context>(a_split: u32, b_split: u32) -> (a_split: u32, b_split: u32) {
        InnerStart(a_split, b_split) -> (a_split, b_split) >>
        InnerFinish(a_split, b_split) -> (a_split, b_split)
    }
}

graph! {
    OwnedGraph<Context> {
        GetNumber() -> (a_number) >>
        Duplicate(a_number) -> (a_split, b_split) >>
        LeftBranch(a_split) -> (a_split) && RightBranch(b_split) -> (b_split) >>
        InnerGraph::run(a_split, b_split) -> (a_split, b_split) >>
        @match Status::Success -> (a_split) {
            Status::Success => PipeNumber(a_split) -> (a_split),
            Status::Fail => PipeNumber(b_split) -> (a_split),
        } >>
        PipeNumber(a_split) -> (a_split)
    }
}

#[cfg(feature = "export")]
fn main() {
    let dto = OwnedGraph::dto();
    println!("{}", dto.name);
    print_steps(&dto.flow.steps, "");
}

#[cfg(not(feature = "export"))]
fn main() {
    eprintln!("enable the `export` feature to render graph DTOs");
}

fn print_steps(steps: &[graphium::dto::GraphStepDto], prefix: &str) {
    let count = steps.len();
    for (idx, step) in steps.iter().enumerate() {
        let is_last = idx + 1 == count;
        let branch = if is_last { "└─" } else { "├─" };
        match step {
            graphium::dto::GraphStepDto::Node {
                name,
                ctx,
                inputs,
                outputs,
            } => {
                let ctx_label = match ctx {
                    graphium::dto::CtxAccessDto::None => "",
                    graphium::dto::CtxAccessDto::Ref => " (ctx: &)",
                    graphium::dto::CtxAccessDto::Mut => " (ctx: &mut)",
                };
                println!(
                    "{}{}{}{}{}",
                    prefix,
                    branch,
                    name,
                    ctx_label,
                    format_io(inputs, outputs)
                );
            }
            graphium::dto::GraphStepDto::Nested {
                graph,
                ctx: _,
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
            }
            graphium::dto::GraphStepDto::Parallel { branches, .. } => {
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
                    print_steps(branch_steps, &branch_prefix);
                }
            }
            graphium::dto::GraphStepDto::Route { on, cases, .. } => {
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
                    print_steps(&case.steps, &case_prefix);
                }
            }
            graphium::dto::GraphStepDto::While {
                condition, body, ..
            } => {
                println!("{}{}@while {}", prefix, branch, condition);
                let next_prefix = if is_last {
                    format!("{}  ", prefix)
                } else {
                    format!("{}│ ", prefix)
                };
                print_steps(body, &next_prefix);
            }
            graphium::dto::GraphStepDto::Loop { body, .. } => {
                println!("{}{}@loop", prefix, branch);
                let next_prefix = if is_last {
                    format!("{}  ", prefix)
                } else {
                    format!("{}│ ", prefix)
                };
                print_steps(body, &next_prefix);
            }
            graphium::dto::GraphStepDto::Break => {
                println!("{}{}@break", prefix, branch);
            }
        }
    }
}

fn format_io(inputs: &[String], outputs: &[String]) -> String {
    if inputs.is_empty() && outputs.is_empty() {
        return String::new();
    }
    let ins = if inputs.is_empty() {
        String::new()
    } else {
        format!(" in: {}", inputs.join(", "))
    };
    let outs = if outputs.is_empty() {
        String::new()
    } else {
        format!(" out: {}", outputs.join(", "))
    };
    format!("{}{}", ins, outs)
}
