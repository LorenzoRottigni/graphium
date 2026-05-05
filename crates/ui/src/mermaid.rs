use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use graphium::dto::{CtxAccessDto, GraphCaseDto, GraphDto, GraphStepDto};

use crate::util::{
    ArtifactAccess, escape_label, next_id, normalize_symbol, parse_artifact, slugify,
};

pub(crate) fn to_mermaid(
    graph: &GraphDto,
    context_label: Option<&str>,
    linkable_graphs: &HashSet<String>,
    show_artifacts: bool,
) -> String {
    let mut lines = Vec::new();
    let mut counter = 0usize;

    // Tune layout a bit so linear graphs read cleanly and complex graphs don't
    // feel as cramped by default.
    lines.push(
        r#"%%{init: {"flowchart": {"curve":"linear","nodeSpacing":56,"rankSpacing":80}} }%%"#
            .to_string(),
    );
    // Prefer a horizontal layout so execution reads left-to-right; the UI wraps
    // the SVG in a horizontal scroller.
    lines.push("flowchart LR".to_string());

    lines.push(
        "classDef graphRoot fill:#121214,stroke:#f97316,color:#ffffff,stroke-width:3px".to_string(),
    );
    lines
        .push("classDef io fill:#1a1410,stroke:#f97316,color:#ffedd5,stroke-width:2px".to_string());
    lines.push(
        "classDef ctx fill:#13112b,stroke:#6366f1,color:#e0e7ff,stroke-width:2px".to_string(),
    );
    lines.push(
        // Lifetimes are rendered as separate "rails" (straight lines) so we can
        // connect steps to the rail with short labeled links without cluttering
        // the main execution edges.
        // Keep contrast low: the rail is a guide, not the focus.
        "classDef lifetime fill:#0b0f14,stroke:#334155,color:#cbd5e1,stroke-width:1px"
            .to_string(),
    );
    lines.push(
        "classDef stepNode fill:#121214,stroke:#06b6d4,color:#ecfeff,stroke-width:2px".to_string(),
    );
    lines.push("classDef stepNodeCtxRef stroke:#6366f1,stroke-width:4px".to_string());
    lines.push("classDef stepNodeCtxMut stroke:#ef4444,stroke-width:4px".to_string());
    lines.push("classDef stepGraph fill:#0b0b0c,stroke:#94a3b8,color:#e2e8f0,stroke-width:2px,stroke-dasharray: 6 4".to_string());
    lines.push(
        "classDef control fill:#0b0b0c,stroke:#9ca3af,color:#ffffff,stroke-width:2px".to_string(),
    );
    let root = next_id(&mut counter);
    lines.push(format!(
        r#"{root}(["{}"]):::graphRoot"#,
        escape_label(&graph.name)
    ));

    let inputs_node = if graph.flow.inputs.is_empty() {
        None
    } else {
        let node_id = next_id(&mut counter);
        lines.push(format!(
            r#"{node_id}(["{}"]):::io"#,
            escape_label(&format!("in: {}", graph.flow.inputs.join(", ")))
        ));
        Some(node_id)
    };

    let ctx_node = if graph_uses_ctx(&graph.flow.steps) {
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
        lifetimes: Arc::new(RefCell::new(HashMap::new())),
        lifetime_order: Arc::new(Vec::new()),
        ..Default::default()
    };
    if let Some(inputs_node) = &tracker.inputs_node {
        for input in &graph.flow.inputs {
            let parsed = parse_artifact(input);
            // Owned artifacts are normal data-flow values; don't tie them to a
            // lifetime abstraction in the UI.
            tracker
                .owned
                .insert(ArtifactKey::new(None, parsed.name), inputs_node.clone());
        }
    }

    if graph.flow.steps.is_empty() {
        return lines.join("\n");
    }

    if show_artifacts {
        // Pre-create lifetime rails so they exist from the beginning of the diagram.
        init_lifetime_rails(&mut tracker, &graph.flow.steps, &mut counter);
        // Add an initial tick so rails start aligned with the graph root.
        advance_lifetime_rails(&mut tracker, &mut counter);
    }
    // Note: Mermaid flowcharts don't guarantee exact placement; we render rails
    // after the main chain so they typically appear below it.

    let rendered = append_steps(
        &graph.flow.steps,
        &mut tracker,
        linkable_graphs,
        &mut lines,
        &mut counter,
        show_artifacts,
    );
    lines.push(format!("{root} --> {}", rendered.head));

    let outputs_node = if graph.flow.outputs.is_empty() {
        None
    } else {
        let node_id = next_id(&mut counter);
        lines.push(format!(
            r#"{node_id}(["{}"]):::io"#,
            escape_label(&format!("out: {}", graph.flow.outputs.join(", ")))
        ));
        Some(node_id)
    };

    if let Some(outputs_node) = &outputs_node {
        lines.push(format!("{} --> {outputs_node}", rendered.tail));
        // Add explicit data edges for declared graph outputs.
        for output in graph.flow.outputs.iter() {
            let parsed = parse_artifact(output);
            if let Some(src) = tracker
                .owned
                .get(&ArtifactKey::new(None, parsed.name))
            {
                lines.push(format!(
                    r#"{src} -. "{}" .-> {outputs_node}"#,
                    escape_label(output.trim())
                ));
            }
        }
    }

    if show_artifacts {
        // Extend rails one extra tick so they visually reach "the end" even when
        // the last step doesn't touch a given lifetime.
        advance_lifetime_rails(&mut tracker, &mut counter);
        emit_lifetime_rails(&tracker, &mut lines);
    }

    lines.join("\n")
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct ArtifactKey {
    lifetime: Option<String>,
    name: String,
}

impl ArtifactKey {
    fn new(lifetime: Option<&str>, name: &str) -> Self {
        Self {
            lifetime: lifetime.map(|v| v.to_string()),
            name: name.to_string(),
        }
    }
}

#[derive(Clone)]
struct ArtifactTracker {
    owned: HashMap<ArtifactKey, String>,
    inputs_node: Option<String>,
    ctx_node: Option<String>,
    lifetimes: Arc<RefCell<HashMap<String, LifetimeRail>>>,
    lifetime_order: Arc<Vec<String>>,
}

impl Default for ArtifactTracker {
    fn default() -> Self {
        Self {
            owned: HashMap::new(),
            inputs_node: None,
            ctx_node: None,
            lifetimes: Arc::new(RefCell::new(HashMap::new())),
            lifetime_order: Arc::new(Vec::new()),
        }
    }
}

#[derive(Clone, Default)]
struct LifetimeRail {
    /// Mermaid node id of the rail's start point.
    start: String,
    /// Mermaid node id of the last point added to the rail.
    last: String,
    /// Tap ids aligned to the main graph "positions" (one per rendered step).
    taps: Vec<String>,
    /// Lines that define nodes/edges inside the lifetime subgraph.
    lines: Vec<String>,
}

#[derive(Clone)]
struct RenderedSteps {
    head: String,
    tail: String,
}

fn append_steps(
    steps: &[GraphStepDto],
    tracker: &mut ArtifactTracker,
    linkable_graphs: &HashSet<String>,
    lines: &mut Vec<String>,
    counter: &mut usize,
    show_artifacts: bool,
) -> RenderedSteps {
    let mut head: Option<String> = None;
    let mut previous_tail: Option<String> = None;

    for step in steps {
        if show_artifacts {
            // Keep the lifetime rails in sync with the main graph progression.
            // This ensures each lifetime line spans the full diagram.
            advance_lifetime_rails(tracker, counter);
        }
        let rendered = render_step(step, tracker, linkable_graphs, lines, counter, show_artifacts);
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
    step: &GraphStepDto,
    tracker: &mut ArtifactTracker,
    linkable_graphs: &HashSet<String>,
    lines: &mut Vec<String>,
    counter: &mut usize,
    show_artifacts: bool,
) -> RenderedSteps {
    match step {
        GraphStepDto::Node {
            name,
            ctx,
            inputs,
            outputs,
        } => {
            let node_id = next_id(counter);
            let ctx_label = match ctx {
                CtxAccessDto::None => "",
                CtxAccessDto::Ref => " [ctx:&]",
                CtxAccessDto::Mut => " [ctx:&mut]",
            };
            let label = format!("{}{}", normalize_symbol(name), ctx_label);
            lines.push(format!(
                r#"{node_id}(["{}"]):::stepNode"#,
                escape_label(&label)
            ));
            match ctx {
                CtxAccessDto::Ref => {
                    lines.push(format!("class {node_id} stepNodeCtxRef"));
                }
                CtxAccessDto::Mut => {
                    lines.push(format!("class {node_id} stepNodeCtxMut"));
                }
                CtxAccessDto::None => {}
            }
            let node_slug = slugify(&normalize_symbol(name));
            lines.push(format!(
                r#"click {node_id} "/node/{node_slug}" "Open {}" _self"#,
                escape_label(&normalize_symbol(name))
            ));
            lines.push(format!(r#"style {node_id} cursor:pointer"#));
            if show_artifacts {
                emit_artifact_edges(tracker, &node_id, *ctx, inputs, outputs, None, lines);
                emit_lifetime_rail_links(tracker, &node_id, inputs, outputs, lines);
            } else {
                // Still track owned outputs so the rest of the graph renders.
                track_owned_outputs(tracker, &node_id, outputs);
            }
            RenderedSteps {
                head: node_id.clone(),
                tail: node_id,
            }
        }
        GraphStepDto::Nested {
            graph,
            ctx,
            inputs,
            outputs,
        } => {
            // Keep nested graphs collapsed by default; expanding them inline makes
            // even simple graphs hard to read.
            let node_id = next_id(counter);
            let ctx_label = match ctx {
                CtxAccessDto::None => "",
                CtxAccessDto::Ref => " [ctx:&]",
                CtxAccessDto::Mut => " [ctx:&mut]",
            };
            lines.push(format!(
                r#"{node_id}[["{}"]]:::stepGraph"#,
                escape_label(&format!("{}{}", graph.name, ctx_label))
            ));
            if show_artifacts {
                emit_artifact_edges(tracker, &node_id, *ctx, inputs, outputs, None, lines);
                emit_lifetime_rail_links(tracker, &node_id, inputs, outputs, lines);
            } else {
                track_owned_outputs(tracker, &node_id, outputs);
            }
            if linkable_graphs.contains(&graph.id) {
                lines.push(format!(
                    r#"click {node_id} "/graph/{}" "Open {}" _self"#,
                    graph.id,
                    escape_label(&graph.name)
                ));
                lines.push(format!(r#"style {node_id} cursor:pointer"#));
            }
            RenderedSteps {
                head: node_id.clone(),
                tail: node_id,
            }
        }
        GraphStepDto::Parallel {
            branches,
            inputs,
            outputs,
        } => {
            let fork = next_id(counter);
            let join = next_id(counter);
            lines.push(format!(r#"{fork}(("&")):::control"#));
            lines.push(format!(r#"{join}(("join")):::control"#));

            let fanout = parallel_fanout(branches);
            if show_artifacts {
                emit_artifact_edges(
                    tracker,
                    &fork,
                    CtxAccessDto::None,
                    inputs,
                    &[],
                    Some(&fanout),
                    lines,
                );
            }

            for (idx, branch) in branches.iter().enumerate() {
                if branch.is_empty() {
                    continue;
                }
                let mut branch_tracker = tracker.clone();
                let rendered =
                    append_steps(branch, &mut branch_tracker, linkable_graphs, lines, counter, show_artifacts);
                lines.push(format!(r#"{fork} -->|b{}| {}"#, idx + 1, rendered.head));
                lines.push(format!("{} --> {join}", rendered.tail));

                if show_artifacts {
                    for output in outputs.iter() {
                        let parsed = parse_artifact(output);
                        if parsed.access != ArtifactAccess::Owned {
                            continue;
                        }
                        let key = ArtifactKey::new(None, parsed.name);
                        if let Some(src) = branch_tracker.owned.get(&key) {
                            lines.push(format!(
                                r#"{src} -. "{}" .-> {join}"#,
                                escape_label(parsed.name)
                            ));
                        }
                    }
                }
            }

            // Join outputs are the union of branch exit artifacts.
            for output in outputs.iter() {
                let parsed = parse_artifact(output);
                match parsed.access {
                    ArtifactAccess::Borrowed => {}
                    ArtifactAccess::Owned | ArtifactAccess::Taken => {
                        tracker.owned.insert(
                            ArtifactKey::new(None, parsed.name),
                            join.clone(),
                        );
                    }
                }
            }

            RenderedSteps {
                head: fork,
                tail: join,
            }
        }
        GraphStepDto::Route {
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
            if show_artifacts {
                emit_artifact_edges(
                    tracker,
                    &decision,
                    CtxAccessDto::None,
                    inputs,
                    &[],
                    Some(&fanout),
                    lines,
                );
            }

            for case in cases {
                if case.steps.is_empty() {
                    continue;
                }
                let mut case_tracker = tracker.clone();
                let rendered = append_steps(
                    &case.steps,
                    &mut case_tracker,
                    linkable_graphs,
                    lines,
                    counter,
                    show_artifacts,
                );
                lines.push(format!(
                    r#"{decision} -->|"{}"| {}"#,
                    escape_label(&case.label),
                    rendered.head
                ));
                lines.push(format!("{} --> {join}", rendered.tail));

                if show_artifacts {
                    for output in outputs.iter() {
                        let parsed = parse_artifact(output);
                        if parsed.access != ArtifactAccess::Owned {
                            continue;
                        }
                        let key = ArtifactKey::new(None, parsed.name);
                        if let Some(src) = case_tracker.owned.get(&key) {
                            lines.push(format!(
                                r#"{src} -. "{}" .-> {join}"#,
                                escape_label(parsed.name)
                            ));
                        }
                    }
                }
            }

            for output in outputs.iter() {
                let parsed = parse_artifact(output);
                match parsed.access {
                    ArtifactAccess::Borrowed => {}
                    ArtifactAccess::Owned | ArtifactAccess::Taken => {
                        tracker.owned.insert(
                            ArtifactKey::new(None, parsed.name),
                            join.clone(),
                        );
                    }
                }
            }

            RenderedSteps {
                head: decision,
                tail: join,
            }
        }
        GraphStepDto::While {
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

            if show_artifacts {
                emit_artifact_edges(
                    tracker,
                    &cond,
                    CtxAccessDto::None,
                    inputs,
                    &[],
                    None,
                    lines,
                );
            }

            if !body.is_empty() {
                let mut body_tracker = tracker.clone();
                let rendered =
                    append_steps(body, &mut body_tracker, linkable_graphs, lines, counter, show_artifacts);
                lines.push(format!(r#"{cond} -->|"true"| {}"#, rendered.head));
                lines.push(format!("{} --> {cond}", rendered.tail));
            }
            lines.push(format!(r#"{cond} -->|"false"| {exit}"#));

            for output in outputs.iter() {
                let parsed = parse_artifact(output);
                match parsed.access {
                    ArtifactAccess::Borrowed => {}
                    ArtifactAccess::Owned | ArtifactAccess::Taken => {
                        tracker.owned.insert(
                            ArtifactKey::new(None, parsed.name),
                            exit.clone(),
                        );
                    }
                }
            }

            RenderedSteps {
                head: cond,
                tail: exit,
            }
        }
        GraphStepDto::Loop {
            body,
            inputs,
            outputs,
        } => {
            let start = next_id(counter);
            let exit = next_id(counter);
            lines.push(format!(r#"{start}(("loop")):::control"#));
            lines.push(format!(r#"{exit}(("exit")):::control"#));
            lines.push(format!(r#"{start} -->|"exit"| {exit}"#));

            if show_artifacts {
                emit_artifact_edges(
                    tracker,
                    &start,
                    CtxAccessDto::None,
                    inputs,
                    &[],
                    None,
                    lines,
                );
            }

            if !body.is_empty() {
                let mut body_tracker = tracker.clone();
                let rendered =
                    append_steps(body, &mut body_tracker, linkable_graphs, lines, counter, show_artifacts);
                lines.push(format!("{start} --> {}", rendered.head));
                lines.push(format!("{} --> {start}", rendered.tail));
            }

            // The macro's `Break` is modeled as a step; leave the explicit break
            // node to visually indicate exits.
            for output in outputs.iter() {
                let parsed = parse_artifact(output);
                match parsed.access {
                    ArtifactAccess::Borrowed => {}
                    ArtifactAccess::Owned | ArtifactAccess::Taken => {
                        tracker.owned.insert(
                            ArtifactKey::new(None, parsed.name),
                            exit.clone(),
                        );
                    }
                }
            }

            RenderedSteps {
                head: start,
                tail: exit,
            }
        }
        GraphStepDto::Break => {
            let node_id = next_id(counter);
            lines.push(format!(r#"{node_id}(("break")):::control"#));
            RenderedSteps {
                head: node_id.clone(),
                tail: node_id,
            }
        }
    }
}

fn graph_uses_ctx(steps: &[GraphStepDto]) -> bool {
    for step in steps {
        match step {
            GraphStepDto::Node { ctx, .. } => {
                if *ctx != CtxAccessDto::None {
                    return true;
                }
            }
            GraphStepDto::Nested { ctx, .. } => {
                if *ctx != CtxAccessDto::None {
                    return true;
                }
            }
            GraphStepDto::Parallel { branches, .. } => {
                if branches.iter().any(|b| graph_uses_ctx(b)) {
                    return true;
                }
            }
            GraphStepDto::Route { cases, .. } => {
                if cases.iter().any(|c| graph_uses_ctx(&c.steps)) {
                    return true;
                }
            }
            GraphStepDto::While { body, .. } | GraphStepDto::Loop { body, .. } => {
                if graph_uses_ctx(body) {
                    return true;
                }
            }
            GraphStepDto::Break => {}
        }
    }
    false
}

fn route_label(on: &str, inputs: &[String]) -> String {
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
    ctx_access: CtxAccessDto,
    inputs: &[String],
    outputs: &[String],
    owned_fanout: Option<&HashMap<String, usize>>,
    lines: &mut Vec<String>,
) {
    if let Some(ctx) = &tracker.ctx_node {
        let access_label = match ctx_access {
            CtxAccessDto::None => None,
            CtxAccessDto::Ref => Some("ctx access: &"),
            CtxAccessDto::Mut => Some("ctx access: &mut"),
        };
        if let Some(label) = access_label {
            // Use an undirected edge so it doesn't look like values "flow" through ctx.
            lines.push(format!(
                r#"{ctx} -. "{}" .- {step_node}"#,
                escape_label(label)
            ));
        }
    }

    let mut owned_inputs: Vec<&str> = Vec::new();
    for input in inputs {
        let parsed = parse_artifact(input);
        if parsed.access == ArtifactAccess::Owned {
            owned_inputs.push(parsed.name);
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

        if let Some(src) = tracker.owned.get(&ArtifactKey::new(None, base)) {
            lines.push(format!(
                r#"{src} -. "{}" .-> {step_node}"#,
                escape_label(&label)
            ));
        } else if let Some(inputs_node) = &tracker.inputs_node {
            lines.push(format!(
                r#"{inputs_node} -. "{}" .-> {step_node}"#,
                escape_label(&label)
            ));
            tracker
                .owned
                .insert(ArtifactKey::new(None, base), inputs_node.clone());
        }
    }

    for output in outputs {
        let parsed = parse_artifact(output);
        match parsed.access {
            ArtifactAccess::Borrowed => {
                continue;
            }
            ArtifactAccess::Owned | ArtifactAccess::Taken => {}
        }
        tracker.owned.insert(
            // Owned/Taken outputs are normal data-flow values; don't associate
            // them with a lifetime abstraction node.
            ArtifactKey::new(None, parsed.name),
            step_node.to_string(),
        );
    }
}

fn track_owned_outputs(tracker: &mut ArtifactTracker, step_node: &str, outputs: &[String]) {
    for output in outputs {
        let parsed = parse_artifact(output);
        if parsed.access == ArtifactAccess::Borrowed {
            continue;
        }
        tracker.owned.insert(
            ArtifactKey::new(None, parsed.name),
            step_node.to_string(),
        );
    }
}

fn parallel_fanout(branches: &[Vec<GraphStepDto>]) -> HashMap<String, usize> {
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
    cases: &[GraphCaseDto],
    selector_inputs: &[String],
) -> HashMap<String, usize> {
    let mut branch_required: HashSet<String> = HashSet::new();
    for case in cases {
        branch_required.extend(steps_owned_requirements(&case.steps));
    }

    let mut out: HashMap<String, usize> = HashMap::new();
    for input in selector_inputs {
        let parsed = parse_artifact(input);
        if parsed.access != ArtifactAccess::Owned {
            continue;
        }
        if branch_required.contains(parsed.name) {
            // Selector needs a clone so the chosen branch can still consume the value.
            out.insert(parsed.name.to_string(), 2);
        }
    }
    out
}

fn steps_owned_requirements(steps: &[GraphStepDto]) -> HashSet<String> {
    let mut out = HashSet::new();
    collect_steps_owned_requirements(steps, &mut out);
    out
}

fn collect_steps_owned_requirements(steps: &[GraphStepDto], out: &mut HashSet<String>) {
    for step in steps {
        match step {
            GraphStepDto::Node { inputs, .. } | GraphStepDto::Nested { inputs, .. } => {
                for input in inputs {
                    let parsed = parse_artifact(input);
                    if parsed.access == ArtifactAccess::Owned {
                        out.insert(parsed.name.to_string());
                    }
                }
            }
            GraphStepDto::Parallel { branches, .. } => {
                for branch in branches {
                    collect_steps_owned_requirements(branch, out);
                }
            }
            GraphStepDto::Route { inputs, cases, .. } => {
                for input in inputs {
                    let parsed = parse_artifact(input);
                    if parsed.access == ArtifactAccess::Owned {
                        out.insert(parsed.name.to_string());
                    }
                }
                for case in cases {
                    collect_steps_owned_requirements(&case.steps, out);
                }
            }
            GraphStepDto::While { body, inputs, .. } | GraphStepDto::Loop { body, inputs, .. } => {
                for input in inputs {
                    let parsed = parse_artifact(input);
                    if parsed.access == ArtifactAccess::Owned {
                        out.insert(parsed.name.to_string());
                    }
                }
                collect_steps_owned_requirements(body, out);
            }
            GraphStepDto::Break => {}
        }
    }
}

#[derive(Clone, Copy, Default)]
struct LifetimeOps {
    reads: usize,
    edits: usize,
    takes: usize,
    stores: usize,
}

fn collect_lifetime_ops(inputs: &[String], outputs: &[String]) -> HashMap<String, LifetimeOps> {
    let mut per_lt: HashMap<String, LifetimeOps> = HashMap::new();

    for input in inputs {
        let parsed = parse_artifact(input);
        if parsed.access == ArtifactAccess::Owned {
            continue;
        }
        let Some(lt) = parsed.lifetime else {
            continue;
        };

        let ops = per_lt.entry(lt.to_string()).or_default();
        match parsed.access {
            ArtifactAccess::Borrowed => {
                if parsed.mutable {
                    ops.edits += 1;
                } else {
                    ops.reads += 1;
                }
            }
            ArtifactAccess::Taken => ops.takes += 1,
            ArtifactAccess::Owned => {}
        }
    }

    for output in outputs {
        let parsed = parse_artifact(output);
        if parsed.access != ArtifactAccess::Borrowed {
            continue;
        }
        let Some(lt) = parsed.lifetime else {
            continue;
        };

        let ops = per_lt.entry(lt.to_string()).or_default();
        // Any `&'a ...` output means the step stores (assigns) an artifact
        // into the graph-managed lifetime scope.
        ops.stores += 1;
    }

    per_lt
}

fn lifetime_in_label(ops: LifetimeOps) -> String {
    let mut actions: Vec<String> = Vec::new();
    if ops.reads > 0 {
        actions.push(if ops.reads == 1 {
            "read".to_string()
        } else {
            format!("read x{}", ops.reads)
        });
    }
    if ops.edits > 0 {
        actions.push(if ops.edits == 1 {
            "edit".to_string()
        } else {
            format!("edit x{}", ops.edits)
        });
    }
    if ops.takes > 0 {
        actions.push(if ops.takes == 1 {
            "take".to_string()
        } else {
            format!("take x{}", ops.takes)
        });
    }
    actions.join(", ")
}

fn lifetime_out_label(ops: LifetimeOps) -> String {
    if ops.stores == 0 {
        return String::new();
    }
    if ops.stores == 1 {
        "store".to_string()
    } else {
        format!("store x{}", ops.stores)
    }
}

fn ensure_lifetime_rail(tracker: &mut ArtifactTracker, lt: &str, counter: &mut usize) {
    // We store rails behind a RefCell so cloned trackers (for parallel branches)
    // all contribute to the same set of rail nodes. This prevents "dangling"
    // tap references like `n8`/`n10` that Mermaid otherwise renders as unlabeled
    // squares.
    let mut rails = tracker.lifetimes.borrow_mut();
    if !rails.contains_key(lt) {
        let start = next_id(counter);
        let mut lines = Vec::new();
        // Start node is labeled so the rail is identifiable even when zoomed out.
        lines.push(format!(
            r#"{start}(["{}"]):::lifetime"#,
            escape_label(&format!("{lt}"))
        ));
        rails.insert(
            lt.to_string(),
            LifetimeRail {
                start: start.clone(),
                last: start,
                taps: Vec::new(),
                lines,
            },
        );
    }
}

fn with_lifetime_rail_mut<F: FnOnce(&mut LifetimeRail)>(
    tracker: &mut ArtifactTracker,
    lt: &str,
    counter: &mut usize,
    f: F,
) {
    ensure_lifetime_rail(tracker, lt, counter);
    let mut rails = tracker.lifetimes.borrow_mut();
    let rail = rails
        .get_mut(lt)
        .expect("lifetime rail must exist after ensure");
    f(rail);
}

fn init_lifetime_rails(tracker: &mut ArtifactTracker, steps: &[GraphStepDto], counter: &mut usize) {
    let mut lts: HashSet<String> = HashSet::new();
    collect_lifetimes_in_steps(steps, &mut lts);
    let mut order: Vec<String> = lts.into_iter().collect();
    order.sort();

    tracker.lifetime_order = Arc::new(order.clone());
    for lt in order {
        ensure_lifetime_rail(tracker, &lt, counter);
    }
}

fn advance_lifetime_rails(tracker: &mut ArtifactTracker, counter: &mut usize) {
    let order: Vec<String> = tracker.lifetime_order.iter().cloned().collect();
    for lt in order {
        let tap = next_id(counter);
        with_lifetime_rail_mut(tracker, &lt, counter, |rail| {
            // Timeline look: |---|---|...|
            // Mermaid can sometimes render a lone `|` label oddly; use the HTML entity
            // so it consistently shows as a visible tick mark instead of falling back
            // to the node id.
            rail.lines
                .push(format!(r#"{tap}(["&#124;"]):::lifetime"#));
            rail.lines.push(format!("{} --- {tap}", rail.last));
            rail.last = tap.clone();
            rail.taps.push(tap);
        });
    }
}

fn emit_lifetime_rail_links(
    tracker: &mut ArtifactTracker,
    step_node: &str,
    inputs: &[String],
    outputs: &[String],
    lines: &mut Vec<String>,
) {
    let mut per_lt: Vec<(String, LifetimeOps)> = collect_lifetime_ops(inputs, outputs)
        .into_iter()
        .collect();
    per_lt.sort_by(|(a, _), (b, _)| a.cmp(b));

    for (lt, ops) in per_lt {
        let rails = tracker.lifetimes.borrow();
        let Some(rail) = rails.get(&lt) else {
            continue;
        };
        let Some(tap) = rail.taps.last() else {
            continue;
        };

        // Read/edit/take: lifetime -> step (arrow into the node)
        let in_label = lifetime_in_label(ops);
        if !in_label.is_empty() {
            lines.push(format!(
                r#"{tap} -. "{}" .-> {step_node}"#,
                escape_label(&in_label)
            ));
        }

        // Store: step -> lifetime (arrow into the rail)
        let out_label = lifetime_out_label(ops);
        if !out_label.is_empty() {
            lines.push(format!(
                r#"{step_node} -. "{}" .-> {tap}"#,
                escape_label(&out_label)
            ));
        }
    }
}

fn emit_lifetime_rails(tracker: &ArtifactTracker, lines: &mut Vec<String>) {
    if tracker.lifetimes.borrow().is_empty() {
        return;
    }

    // Group all rails under one container to encourage Mermaid to keep them
    // together (typically below the main execution chain).
    lines.push(r#"subgraph lifetimes[" "]"#.to_string());
    lines.push("direction TB".to_string());

    // Render each lifetime as its own LR subgraph (a straight rail).
    for lt in tracker.lifetime_order.iter() {
        let rails = tracker.lifetimes.borrow();
        let rail = &rails[lt];
        let sg_id = format!("lt_{}", slugify(lt));
        // Unlabeled subgraph keeps contrast low; the start node carries the label.
        lines.push(format!(r#"subgraph {sg_id}[" "]"#));
        lines.push("direction LR".to_string());
        // Keep the rail visually close to the main graph by chaining a hidden
        // ordering edge from the graph root area via the first tap.
        // (Mermaid doesn't guarantee placement, but this helps.)
        for l in &rail.lines {
            lines.push(l.clone());
        }
        lines.push("end".to_string());
    }

    lines.push("end".to_string());
}

fn collect_lifetimes_in_steps(steps: &[GraphStepDto], out: &mut HashSet<String>) {
    for step in steps {
        match step {
            GraphStepDto::Node { inputs, outputs, .. }
            | GraphStepDto::Nested { inputs, outputs, .. } => {
                for v in inputs.iter().chain(outputs.iter()) {
                    let parsed = parse_artifact(v);
                    if parsed.access == ArtifactAccess::Owned {
                        continue;
                    }
                    if let Some(lt) = parsed.lifetime {
                        out.insert(lt.to_string());
                    }
                }
            }
            GraphStepDto::Parallel { branches, inputs, outputs } => {
                for v in inputs.iter().chain(outputs.iter()) {
                    let parsed = parse_artifact(v);
                    if parsed.access == ArtifactAccess::Owned {
                        continue;
                    }
                    if let Some(lt) = parsed.lifetime {
                        out.insert(lt.to_string());
                    }
                }
                for b in branches {
                    collect_lifetimes_in_steps(b, out);
                }
            }
            GraphStepDto::Route { cases, inputs, outputs, .. } => {
                for v in inputs.iter().chain(outputs.iter()) {
                    let parsed = parse_artifact(v);
                    if parsed.access == ArtifactAccess::Owned {
                        continue;
                    }
                    if let Some(lt) = parsed.lifetime {
                        out.insert(lt.to_string());
                    }
                }
                for c in cases {
                    collect_lifetimes_in_steps(&c.steps, out);
                }
            }
            GraphStepDto::While { body, inputs, outputs, .. }
            | GraphStepDto::Loop { body, inputs, outputs } => {
                for v in inputs.iter().chain(outputs.iter()) {
                    let parsed = parse_artifact(v);
                    if parsed.access == ArtifactAccess::Owned {
                        continue;
                    }
                    if let Some(lt) = parsed.lifetime {
                        out.insert(lt.to_string());
                    }
                }
                collect_lifetimes_in_steps(body, out);
            }
            GraphStepDto::Break => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphium::dto::{GraphFlowDto, GraphStepDto};

    #[test]
    fn mermaid_uses_frame_for_borrowed_artifacts_not_ctx() {
        let graph = GraphDto {
            id: "g".to_string(),
            name: "G".to_string(),
            flow: GraphFlowDto {
                inputs: vec![],
                outputs: vec![],
                steps: vec![
                    GraphStepDto::Node {
                        name: "A".to_string(),
                        ctx: CtxAccessDto::None,
                        inputs: vec![],
                        outputs: vec!["&'a x".to_string()],
                    },
                    GraphStepDto::Node {
                        name: "B".to_string(),
                        ctx: CtxAccessDto::None,
                        inputs: vec!["&'a x".to_string()],
                        outputs: vec![],
                    },
                ],
            },
            ..Default::default()
        };

        let rendered = to_mermaid(&graph, None, &HashSet::new(), true);
        assert!(!rendered.contains(":::ctx"));
        assert!(!rendered.contains("ctx set:"));
        // Lifetimes are rendered as rails with tap links.
        assert!(rendered.contains("subgraph lt_a"));
        assert!(rendered.contains("'a"));
        assert!(rendered.contains("store"));
        assert!(rendered.contains("read"));
    }

    #[test]
    fn mermaid_uses_ctx_only_when_ctx_access_present() {
        let graph = GraphDto {
            id: "g".to_string(),
            name: "G".to_string(),
            flow: GraphFlowDto {
                inputs: vec![],
                outputs: vec![],
                steps: vec![GraphStepDto::Node {
                    name: "A".to_string(),
                    ctx: CtxAccessDto::Ref,
                    inputs: vec![],
                    outputs: vec![],
                }],
            },
            ..Default::default()
        };

        let rendered = to_mermaid(&graph, None, &HashSet::new(), true);
        assert!(rendered.contains(":::ctx"));
        assert!(!rendered.contains("subgraph lt-"));
    }
}
