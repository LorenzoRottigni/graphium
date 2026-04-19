use std::collections::{HashMap, HashSet};

use graphium::{GraphCase, GraphDef, GraphStep};

use crate::util::{escape_label, next_id, normalize_symbol, parse_artifact, slugify};

pub(crate) fn to_mermaid(
    graph: &GraphDef,
    context_label: Option<&str>,
    graph_id: Option<&str>,
    linkable_graphs: &HashSet<String>,
) -> String {
    let mut lines = Vec::new();
    let mut counter = 0usize;

    // Tune layout a bit so linear graphs read cleanly and complex graphs don't
    // feel as cramped by default.
    lines.push(
        r#"%%{init: {"flowchart": {"curve":"basis","nodeSpacing":50,"rankSpacing":70}} }%%"#
            .to_string(),
    );
    // Prefer a horizontal layout so execution reads left-to-right; the UI wraps
    // the SVG in a horizontal scroller.
    lines.push("flowchart LR".to_string());

    lines.push(
        "classDef graphRoot fill:#0b1f3a,stroke:#0b1f3a,color:#ffffff,stroke-width:2px".to_string(),
    );
    lines
        .push("classDef io fill:#fff7ed,stroke:#f97316,color:#7c2d12,stroke-width:2px".to_string());
    lines.push(
        "classDef ctx fill:#eef2ff,stroke:#4f46e5,color:#1e1b4b,stroke-width:2px".to_string(),
    );
    lines.push(
        "classDef stepNode fill:#ecfeff,stroke:#06b6d4,color:#083344,stroke-width:2px".to_string(),
    );
    lines.push("classDef stepNodeCtxRef stroke:#4f46e5,stroke-width:3px".to_string());
    lines.push("classDef stepNodeCtxMut stroke:#dc2626,stroke-width:3px".to_string());
    lines.push(
        "classDef stepGraph fill:#f1f5f9,stroke:#334155,color:#0f172a,stroke-width:2px,stroke-dasharray: 5 5"
            .to_string(),
    );
    lines.push(
        "classDef control fill:#fafafa,stroke:#64748b,color:#0f172a,stroke-width:2px".to_string(),
    );

    let root = next_id(&mut counter);
    lines.push(format!(
        r#"{root}(["{}"]):::graphRoot"#,
        escape_label(graph.name)
    ));

    let inputs_node = if graph.inputs.is_empty() {
        None
    } else {
        let node_id = next_id(&mut counter);
        lines.push(format!(
            r#"{node_id}(["{}"]):::io"#,
            escape_label(&format!("in: {}", graph.inputs.join(", ")))
        ));
        Some(node_id)
    };

    let ctx_node = if graph_uses_ctx(graph) {
        let node_id = next_id(&mut counter);
        let label = context_label
            .map(|ctx| format!("ctx: {ctx}"))
            .unwrap_or_else(|| "ctx".to_string());
        lines.push(format!(r#"{node_id}(["{}"]):::ctx"#, escape_label(&label)));
        Some(node_id)
    } else {
        None
    };

    if let Some(inputs_node) = &inputs_node {
        lines.push(format!("{root} --> {inputs_node}"));
    }
    if let Some(ctx_node) = &ctx_node {
        lines.push(format!("{root} --> {ctx_node}"));
    }

    let mut tracker = ArtifactTracker {
        inputs_node,
        ctx_node,
        ..Default::default()
    };
    if let Some(inputs_node) = &tracker.inputs_node {
        for input in &graph.inputs {
            tracker.owned.insert(input.to_string(), inputs_node.clone());
        }
    }

    if graph.steps.is_empty() {
        return lines.join("\n");
    }

    let rendered = append_steps(
        &graph.steps,
        &mut tracker,
        graph_id,
        linkable_graphs,
        &mut lines,
        &mut counter,
    );
    lines.push(format!("{root} --> {}", rendered.head));

    let outputs_node = if graph.outputs.is_empty() {
        None
    } else {
        let node_id = next_id(&mut counter);
        lines.push(format!(
            r#"{node_id}(["{}"]):::io"#,
            escape_label(&format!("out: {}", graph.outputs.join(", ")))
        ));
        Some(node_id)
    };

    if let Some(outputs_node) = &outputs_node {
        lines.push(format!("{} --> {outputs_node}", rendered.tail));
        // Add explicit data edges for declared graph outputs.
        for &output in graph.outputs.iter() {
            if let Some(src) = tracker.owned.get(output) {
                lines.push(format!(
                    r#"{src} -. "{}" .-> {outputs_node}"#,
                    escape_label(output)
                ));
            }
        }
    }

    lines.join("\n")
}

#[derive(Clone, Default)]
struct ArtifactTracker {
    owned: HashMap<String, String>,
    borrowed_live: HashSet<String>,
    inputs_node: Option<String>,
    ctx_node: Option<String>,
}

#[derive(Clone)]
struct RenderedSteps {
    head: String,
    tail: String,
}

fn append_steps(
    steps: &[GraphStep],
    tracker: &mut ArtifactTracker,
    graph_id: Option<&str>,
    linkable_graphs: &HashSet<String>,
    lines: &mut Vec<String>,
    counter: &mut usize,
) -> RenderedSteps {
    let mut head: Option<String> = None;
    let mut previous_tail: Option<String> = None;

    for step in steps {
        let rendered = render_step(step, tracker, graph_id, linkable_graphs, lines, counter);
        if head.is_none() {
            head = Some(rendered.head.clone());
        }
        if let Some(prev) = previous_tail {
            lines.push(format!("{prev} --> {}", rendered.head));
        }
        previous_tail = Some(rendered.tail);
    }

    RenderedSteps {
        head: head.unwrap_or_else(|| next_id(counter)),
        tail: previous_tail.unwrap_or_else(|| next_id(counter)),
    }
}

fn render_step(
    step: &GraphStep,
    tracker: &mut ArtifactTracker,
    graph_id: Option<&str>,
    linkable_graphs: &HashSet<String>,
    lines: &mut Vec<String>,
    counter: &mut usize,
) -> RenderedSteps {
    match step {
        GraphStep::Node {
            name,
            ctx,
            inputs,
            outputs,
        } => {
            let node_id = next_id(counter);
            let ctx_label = match ctx {
                graphium::CtxAccess::None => "",
                graphium::CtxAccess::Ref => " [ctx:&]",
                graphium::CtxAccess::Mut => " [ctx:&mut]",
            };
            let label = format!("{}{}", normalize_symbol(name), ctx_label);
            lines.push(format!(
                r#"{node_id}(["{}"]):::stepNode"#,
                escape_label(&label)
            ));
            match ctx {
                graphium::CtxAccess::Ref => {
                    lines.push(format!("class {node_id} stepNodeCtxRef"));
                }
                graphium::CtxAccess::Mut => {
                    lines.push(format!("class {node_id} stepNodeCtxMut"));
                }
                graphium::CtxAccess::None => {}
            }
            if let Some(graph_id) = graph_id {
                let node_slug = slugify(&normalize_symbol(name));
                lines.push(format!(
                    r#"click {node_id} "/node/{node_slug}?graph={graph_id}" "Open {}" _self"#,
                    escape_label(&normalize_symbol(name))
                ));
                lines.push(format!(r#"style {node_id} cursor:pointer"#));
            }
            emit_artifact_edges(tracker, &node_id, *ctx, inputs, outputs, None, lines);
            RenderedSteps {
                head: node_id.clone(),
                tail: node_id,
            }
        }
        GraphStep::Nested {
            graph,
            ctx,
            inputs,
            outputs,
        } => {
            // Keep nested graphs collapsed by default; expanding them inline makes
            // even simple graphs hard to read.
            let node_id = next_id(counter);
            let ctx_label = match ctx {
                graphium::CtxAccess::None => "",
                graphium::CtxAccess::Ref => " [ctx:&]",
                graphium::CtxAccess::Mut => " [ctx:&mut]",
            };
            lines.push(format!(
                r#"{node_id}[["{}"]]:::stepGraph"#,
                escape_label(&format!("{}{}", graph.name, ctx_label))
            ));
            emit_artifact_edges(tracker, &node_id, *ctx, inputs, outputs, None, lines);
            let nested_id = slugify(graph.name);
            if linkable_graphs.contains(&nested_id) {
                lines.push(format!(
                    r#"click {node_id} "/graph/{nested_id}" "Open {}" _self"#,
                    escape_label(graph.name)
                ));
                lines.push(format!(r#"style {node_id} cursor:pointer"#));
            }
            RenderedSteps {
                head: node_id.clone(),
                tail: node_id,
            }
        }
        GraphStep::Parallel {
            branches,
            inputs,
            outputs,
        } => {
            let fork = next_id(counter);
            let join = next_id(counter);
            lines.push(format!(r#"{fork}(("&")):::control"#));
            lines.push(format!(r#"{join}(("join")):::control"#));

            let fanout = parallel_fanout(branches);
            emit_artifact_edges(
                tracker,
                &fork,
                graphium::CtxAccess::None,
                inputs,
                &[],
                Some(&fanout),
                lines,
            );

            for (idx, branch) in branches.iter().enumerate() {
                if branch.is_empty() {
                    continue;
                }
                let mut branch_tracker = tracker.clone();
                let rendered = append_steps(
                    branch,
                    &mut branch_tracker,
                    graph_id,
                    linkable_graphs,
                    lines,
                    counter,
                );
                lines.push(format!(r#"{fork} -->|b{}| {}"#, idx + 1, rendered.head));
                lines.push(format!("{} --> {join}", rendered.tail));

                for &output in outputs.iter() {
                    let (base, borrowed) = parse_artifact(output);
                    if borrowed {
                        continue;
                    }
                    if let Some(src) = branch_tracker.owned.get(base) {
                        lines.push(format!(r#"{src} -. "{}" .-> {join}"#, escape_label(base)));
                    }
                }
            }

            // Join outputs are the union of branch exit artifacts.
            let mut borrowed_outputs: Vec<&str> = Vec::new();
            for &output in outputs.iter() {
                let (base, borrowed) = parse_artifact(output);
                if borrowed {
                    borrowed_outputs.push(base);
                } else {
                    tracker.owned.insert(base.to_string(), join.clone());
                }
            }
            apply_borrowed_lifetimes(tracker, &join, &borrowed_outputs, lines);

            RenderedSteps {
                head: fork,
                tail: join,
            }
        }
        GraphStep::Route {
            on,
            cases,
            inputs,
            outputs,
        } => {
            let decision = next_id(counter);
            let join = next_id(counter);

            lines.push(format!(
                r#"{decision}{{"{}"}}:::control"#,
                escape_label(&route_label(on, inputs))
            ));
            lines.push(format!(r#"{join}(("join")):::control"#));

            let fanout = route_selector_fanout(cases, inputs);
            emit_artifact_edges(
                tracker,
                &decision,
                graphium::CtxAccess::None,
                inputs,
                &[],
                Some(&fanout),
                lines,
            );

            for case in cases {
                if case.steps.is_empty() {
                    continue;
                }
                let mut case_tracker = tracker.clone();
                let rendered = append_steps(
                    &case.steps,
                    &mut case_tracker,
                    graph_id,
                    linkable_graphs,
                    lines,
                    counter,
                );
                lines.push(format!(
                    r#"{decision} -->|"{}"| {}"#,
                    escape_label(case.label),
                    rendered.head
                ));
                lines.push(format!("{} --> {join}", rendered.tail));

                for &output in outputs.iter() {
                    let (base, borrowed) = parse_artifact(output);
                    if borrowed {
                        continue;
                    }
                    if let Some(src) = case_tracker.owned.get(base) {
                        lines.push(format!(r#"{src} -. "{}" .-> {join}"#, escape_label(base)));
                    }
                }
            }

            let mut borrowed_outputs: Vec<&str> = Vec::new();
            for &output in outputs.iter() {
                let (base, borrowed) = parse_artifact(output);
                if borrowed {
                    borrowed_outputs.push(base);
                } else {
                    tracker.owned.insert(base.to_string(), join.clone());
                }
            }
            apply_borrowed_lifetimes(tracker, &join, &borrowed_outputs, lines);

            RenderedSteps {
                head: decision,
                tail: join,
            }
        }
        GraphStep::While {
            condition,
            body,
            inputs,
            outputs,
        } => {
            let cond = next_id(counter);
            let exit = next_id(counter);
            lines.push(format!(
                r#"{cond}{{"{}"}}:::control"#,
                escape_label(&format!("while {condition}"))
            ));
            lines.push(format!(r#"{exit}(("exit")):::control"#));

            emit_artifact_edges(
                tracker,
                &cond,
                graphium::CtxAccess::None,
                inputs,
                &[],
                None,
                lines,
            );

            if !body.is_empty() {
                let mut body_tracker = tracker.clone();
                let rendered = append_steps(
                    body,
                    &mut body_tracker,
                    graph_id,
                    linkable_graphs,
                    lines,
                    counter,
                );
                lines.push(format!(r#"{cond} -->|"true"| {}"#, rendered.head));
                lines.push(format!("{} --> {cond}", rendered.tail));
            }
            lines.push(format!(r#"{cond} -->|"false"| {exit}"#));

            let mut borrowed_outputs: Vec<&str> = Vec::new();
            for &output in outputs.iter() {
                let (base, borrowed) = parse_artifact(output);
                if borrowed {
                    borrowed_outputs.push(base);
                } else {
                    tracker.owned.insert(base.to_string(), exit.clone());
                }
            }
            apply_borrowed_lifetimes(tracker, &exit, &borrowed_outputs, lines);

            RenderedSteps {
                head: cond,
                tail: exit,
            }
        }
        GraphStep::Loop {
            body,
            inputs,
            outputs,
        } => {
            let start = next_id(counter);
            let exit = next_id(counter);
            lines.push(format!(r#"{start}(("loop")):::control"#));
            lines.push(format!(r#"{exit}(("exit")):::control"#));
            lines.push(format!(r#"{start} -->|"exit"| {exit}"#));

            emit_artifact_edges(
                tracker,
                &start,
                graphium::CtxAccess::None,
                inputs,
                &[],
                None,
                lines,
            );

            if !body.is_empty() {
                let mut body_tracker = tracker.clone();
                let rendered = append_steps(
                    body,
                    &mut body_tracker,
                    graph_id,
                    linkable_graphs,
                    lines,
                    counter,
                );
                lines.push(format!("{start} --> {}", rendered.head));
                lines.push(format!("{} --> {start}", rendered.tail));
            }

            // The macro's `Break` is modeled as a step; leave the explicit break
            // node to visually indicate exits.
            let mut borrowed_outputs: Vec<&str> = Vec::new();
            for &output in outputs.iter() {
                let (base, borrowed) = parse_artifact(output);
                if borrowed {
                    borrowed_outputs.push(base);
                } else {
                    tracker.owned.insert(base.to_string(), exit.clone());
                }
            }
            apply_borrowed_lifetimes(tracker, &exit, &borrowed_outputs, lines);

            RenderedSteps {
                head: start,
                tail: exit,
            }
        }
        GraphStep::Break => {
            let node_id = next_id(counter);
            lines.push(format!(r#"{node_id}(("break")):::control"#));
            RenderedSteps {
                head: node_id.clone(),
                tail: node_id,
            }
        }
    }
}

fn graph_uses_ctx(graph: &GraphDef) -> bool {
    steps_use_ctx(&graph.steps)
}

fn steps_use_ctx(steps: &[GraphStep]) -> bool {
    for step in steps {
        match step {
            GraphStep::Node {
                inputs,
                outputs,
                ctx,
                ..
            } => {
                if *ctx != graphium::CtxAccess::None {
                    return true;
                }
                if inputs.iter().any(|v| v.starts_with('&'))
                    || outputs.iter().any(|v| v.starts_with('&'))
                {
                    return true;
                }
            }
            GraphStep::Nested {
                inputs,
                outputs,
                ctx,
                ..
            } => {
                if *ctx != graphium::CtxAccess::None {
                    return true;
                }
                if inputs.iter().any(|v| v.starts_with('&'))
                    || outputs.iter().any(|v| v.starts_with('&'))
                {
                    return true;
                }
            }
            GraphStep::Parallel { branches, .. } => {
                if branches.iter().any(|b| steps_use_ctx(b)) {
                    return true;
                }
            }
            GraphStep::Route { cases, .. } => {
                if cases.iter().any(|c| steps_use_ctx(&c.steps)) {
                    return true;
                }
            }
            GraphStep::While { body, .. } | GraphStep::Loop { body, .. } => {
                if steps_use_ctx(body) {
                    return true;
                }
            }
            GraphStep::Break => {}
        }
    }
    false
}

fn route_label(on: &str, inputs: &[&'static str]) -> String {
    if inputs.len() == 1 {
        return format!("match {}", inputs[0]);
    }
    let trimmed = on.trim();
    let noisy = trimmed.contains('{') || trimmed.contains('|') || trimmed.len() > 60;
    if noisy {
        "match".to_string()
    } else {
        format!("match {trimmed}")
    }
}

fn emit_artifact_edges(
    tracker: &mut ArtifactTracker,
    step_node: &str,
    ctx_access: graphium::CtxAccess,
    inputs: &[&'static str],
    outputs: &[&'static str],
    owned_fanout: Option<&HashMap<String, usize>>,
    lines: &mut Vec<String>,
) {
    if let Some(ctx) = &tracker.ctx_node {
        let access_label = match ctx_access {
            graphium::CtxAccess::None => None,
            graphium::CtxAccess::Ref => Some("ctx access: &"),
            graphium::CtxAccess::Mut => Some("ctx access: &mut"),
        };
        if let Some(label) = access_label {
            // Use an undirected edge so it doesn't look like values "flow" through ctx.
            lines.push(format!(
                r#"{ctx} -. "{}" .- {step_node}"#,
                escape_label(label)
            ));
        }
    }

    let mut borrowed_inputs: Vec<&str> = Vec::new();
    let mut owned_inputs: Vec<&str> = Vec::new();
    for input in inputs {
        let (base, borrowed) = parse_artifact(input);
        if borrowed {
            borrowed_inputs.push(base);
        } else {
            owned_inputs.push(base);
        }
    }

    owned_inputs.sort();
    owned_inputs.dedup();
    for base in owned_inputs {
        let fanout = owned_fanout.and_then(|m| m.get(base)).copied().unwrap_or(1);
        let label = if fanout > 1 {
            format!("clone x{} + move: {base}", fanout - 1)
        } else {
            format!("move: {base}")
        };

        if let Some(src) = tracker.owned.get(base) {
            lines.push(format!(
                r#"{src} -. "{}" .-> {step_node}"#,
                escape_label(&label)
            ));
        } else if let Some(inputs_node) = &tracker.inputs_node {
            lines.push(format!(
                r#"{inputs_node} -. "{}" .-> {step_node}"#,
                escape_label(&label)
            ));
            tracker.owned.insert(base.to_string(), inputs_node.clone());
        }
    }

    if !borrowed_inputs.is_empty() {
        borrowed_inputs.sort();
        borrowed_inputs.dedup();
        if let Some(ctx) = &tracker.ctx_node {
            let label = if ctx_access == graphium::CtxAccess::Mut {
                format!("borrow (ctx:&mut): {}", borrowed_inputs.join(", "))
            } else {
                format!("borrow: {}", borrowed_inputs.join(", "))
            };
            lines.push(format!(
                r#"{ctx} -. "{}" .- {step_node}"#,
                escape_label(&label)
            ));
        }
    }

    let mut borrowed_outputs: Vec<&str> = Vec::new();
    for output in outputs {
        let (base, borrowed) = parse_artifact(output);
        if borrowed {
            borrowed_outputs.push(base);
            continue;
        }
        tracker
            .owned
            .insert(base.to_string(), step_node.to_string());
    }

    apply_borrowed_lifetimes(tracker, step_node, &borrowed_outputs, lines);
}

fn parallel_fanout(branches: &[Vec<GraphStep>]) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for branch in branches {
        let required = steps_owned_requirements(branch);
        for artifact in required {
            *counts.entry(artifact).or_insert(0) += 1;
        }
    }
    counts
}

fn route_selector_fanout(
    cases: &[GraphCase],
    selector_inputs: &[&'static str],
) -> HashMap<String, usize> {
    let mut branch_required: HashSet<String> = HashSet::new();
    for case in cases {
        branch_required.extend(steps_owned_requirements(&case.steps));
    }

    let mut out: HashMap<String, usize> = HashMap::new();
    for &input in selector_inputs {
        let (base, borrowed) = parse_artifact(input);
        if borrowed {
            continue;
        }
        if branch_required.contains(base) {
            // Selector needs a clone so the chosen branch can still consume the value.
            out.insert(base.to_string(), 2);
        }
    }
    out
}

fn steps_owned_requirements(steps: &[GraphStep]) -> HashSet<String> {
    let mut out = HashSet::new();
    collect_steps_owned_requirements(steps, &mut out);
    out
}

fn collect_steps_owned_requirements(steps: &[GraphStep], out: &mut HashSet<String>) {
    for step in steps {
        match step {
            GraphStep::Node { inputs, .. } | GraphStep::Nested { inputs, .. } => {
                for &input in inputs {
                    let (base, borrowed) = parse_artifact(input);
                    if !borrowed {
                        out.insert(base.to_string());
                    }
                }
            }
            GraphStep::Parallel { branches, .. } => {
                for branch in branches {
                    collect_steps_owned_requirements(branch, out);
                }
            }
            GraphStep::Route { inputs, cases, .. } => {
                for &input in inputs {
                    let (base, borrowed) = parse_artifact(input);
                    if !borrowed {
                        out.insert(base.to_string());
                    }
                }
                for case in cases {
                    collect_steps_owned_requirements(&case.steps, out);
                }
            }
            GraphStep::While { body, inputs, .. } | GraphStep::Loop { body, inputs, .. } => {
                for &input in inputs {
                    let (base, borrowed) = parse_artifact(input);
                    if !borrowed {
                        out.insert(base.to_string());
                    }
                }
                collect_steps_owned_requirements(body, out);
            }
            GraphStep::Break => {}
        }
    }
}

fn apply_borrowed_lifetimes(
    tracker: &mut ArtifactTracker,
    step_node: &str,
    borrowed_outputs: &[&str],
    lines: &mut Vec<String>,
) {
    let next_live: HashSet<String> = borrowed_outputs.iter().map(|v| (*v).to_string()).collect();
    if let Some(ctx) = &tracker.ctx_node {
        if !next_live.is_empty() {
            let mut introduced: Vec<&str> = next_live
                .iter()
                .filter(|artifact| !tracker.borrowed_live.contains(*artifact))
                .map(|s| s.as_str())
                .collect();
            introduced.sort();
            introduced.dedup();
            if !introduced.is_empty() {
                let label = format!("ctx set refs: {}", introduced.join(", "));
                lines.push(format!(
                    r#"{step_node} -. "{}" .- {ctx}"#,
                    escape_label(&label)
                ));
            }
        }

        if !tracker.borrowed_live.is_empty() {
            let mut dropped: Vec<&str> = tracker
                .borrowed_live
                .iter()
                .filter(|artifact| !next_live.contains(*artifact))
                .map(|s| s.as_str())
                .collect();
            dropped.sort();
            dropped.dedup();
            if !dropped.is_empty() {
                let label = format!("drop: {}", dropped.join(", "));
                lines.push(format!(
                    r#"{step_node} -. "{}" .- {ctx}"#,
                    escape_label(&label)
                ));
            }
        }
    }
    tracker.borrowed_live = next_live;
}
