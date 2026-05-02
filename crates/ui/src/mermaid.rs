use std::collections::{HashMap, HashSet};

use graphium::dto::{CtxAccessDto, GraphCaseDto, GraphDto, GraphStepDto};

use crate::util::{
    ArtifactAccess, escape_label, next_id, normalize_symbol, parse_artifact, slugify,
};

pub(crate) fn to_mermaid(
    graph: &GraphDto,
    context_label: Option<&str>,
    linkable_graphs: &HashSet<String>,
) -> String {
    let mut lines = Vec::new();
    let mut counter = 0usize;

    // Tune layout a bit so linear graphs read cleanly and complex graphs don't
    // feel as cramped by default.
    lines.push(
        r#"%%{init: {"flowchart": {"curve":"basis","nodeSpacing":56,"rankSpacing":80}} }%%"#
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
        "classDef lifetime fill:#0b1220,stroke:#22c55e,color:#dcfce7,stroke-width:2px"
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

    let rendered = append_steps(
        &graph.flow.steps,
        &mut tracker,
        linkable_graphs,
        &mut lines,
        &mut counter,
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

#[derive(Clone, Default)]
struct ArtifactTracker {
    owned: HashMap<ArtifactKey, String>,
    inputs_node: Option<String>,
    ctx_node: Option<String>,
    lifetime_nodes: HashMap<String, String>,
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
) -> RenderedSteps {
    let mut head: Option<String> = None;
    let mut previous_tail: Option<String> = None;

    for step in steps {
        let rendered = render_step(step, tracker, linkable_graphs, lines, counter);
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
            emit_artifact_edges(tracker, &node_id, *ctx, inputs, outputs, None, lines, counter);
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
            emit_artifact_edges(tracker, &node_id, *ctx, inputs, outputs, None, lines, counter);
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
            emit_artifact_edges(
                tracker,
                &fork,
                CtxAccessDto::None,
                inputs,
                &[],
                Some(&fanout),
                lines,
                counter,
            );

            for (idx, branch) in branches.iter().enumerate() {
                if branch.is_empty() {
                    continue;
                }
                let mut branch_tracker = tracker.clone();
                let rendered =
                    append_steps(branch, &mut branch_tracker, linkable_graphs, lines, counter);
                lines.push(format!(r#"{fork} -->|b{}| {}"#, idx + 1, rendered.head));
                lines.push(format!("{} --> {join}", rendered.tail));

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
            apply_lifetime_updates(tracker, &join, outputs, lines, counter);

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
            emit_artifact_edges(
                tracker,
                &decision,
                CtxAccessDto::None,
                inputs,
                &[],
                Some(&fanout),
                lines,
                counter,
            );

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
                );
                lines.push(format!(
                    r#"{decision} -->|"{}"| {}"#,
                    escape_label(&case.label),
                    rendered.head
                ));
                lines.push(format!("{} --> {join}", rendered.tail));

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
            apply_lifetime_updates(tracker, &join, outputs, lines, counter);

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

            emit_artifact_edges(
                tracker,
                &cond,
                CtxAccessDto::None,
                inputs,
                &[],
                None,
                lines,
                counter,
            );

            if !body.is_empty() {
                let mut body_tracker = tracker.clone();
                let rendered =
                    append_steps(body, &mut body_tracker, linkable_graphs, lines, counter);
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
            apply_lifetime_updates(tracker, &exit, outputs, lines, counter);

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

            emit_artifact_edges(
                tracker,
                &start,
                CtxAccessDto::None,
                inputs,
                &[],
                None,
                lines,
                counter,
            );

            if !body.is_empty() {
                let mut body_tracker = tracker.clone();
                let rendered =
                    append_steps(body, &mut body_tracker, linkable_graphs, lines, counter);
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
            apply_lifetime_updates(tracker, &exit, outputs, lines, counter);

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
    counter: &mut usize,
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

    emit_lifetime_edges(tracker, step_node, inputs, lines, counter);

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

    apply_lifetime_updates(tracker, step_node, outputs, lines, counter);
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

fn ensure_lifetime_node(
    tracker: &mut ArtifactTracker,
    lifetime: &str,
    lines: &mut Vec<String>,
    counter: &mut usize,
) -> String {
    if let Some(existing) = tracker.lifetime_nodes.get(lifetime) {
        return existing.clone();
    }
    let node_id = next_id(counter);
    lines.push(format!(
        r#"{node_id}{{"{}"}}:::lifetime"#,
        escape_label(&format!("lifetime {lifetime}"))
    ));
    tracker
        .lifetime_nodes
        .insert(lifetime.to_string(), node_id.clone());
    node_id
}

fn emit_lifetime_edges(
    tracker: &mut ArtifactTracker,
    step_node: &str,
    inputs: &[String],
    lines: &mut Vec<String>,
    counter: &mut usize,
) {
    #[derive(Default)]
    struct Ops {
        borrowed: Vec<String>,
        borrowed_mut: Vec<String>,
        taken: Vec<String>,
    }

    let mut per_lt: HashMap<String, Ops> = HashMap::new();
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
                    ops.borrowed_mut.push(parsed.name.to_string());
                } else {
                    ops.borrowed.push(parsed.name.to_string());
                }
            }
            ArtifactAccess::Taken => ops.taken.push(parsed.name.to_string()),
            ArtifactAccess::Owned => {}
        }
    }

    for (lt, mut ops) in per_lt {
        let lifetime_node = ensure_lifetime_node(tracker, &lt, lines, counter);

        ops.borrowed.sort();
        ops.borrowed.dedup();
        if !ops.borrowed.is_empty() {
            let refs: Vec<String> = ops
                .borrowed
                .iter()
                .map(|name| format!("&{lt} {name}"))
                .collect();
            let label = format!("borrow: {}", refs.join(", "));
            lines.push(format!(
                r#"{lifetime_node} -. "{}" .-> {step_node}"#,
                escape_label(&label)
            ));
        }

        ops.borrowed_mut.sort();
        ops.borrowed_mut.dedup();
        if !ops.borrowed_mut.is_empty() {
            let refs: Vec<String> = ops
                .borrowed_mut
                .iter()
                .map(|name| format!("&{lt} mut {name}"))
                .collect();
            let label = format!("borrow mut: {}", refs.join(", "));
            lines.push(format!(
                r#"{lifetime_node} -. "{}" .-> {step_node}"#,
                escape_label(&label)
            ));
        }

        ops.taken.sort();
        ops.taken.dedup();
        if !ops.taken.is_empty() {
            let moved: Vec<String> = ops
                .taken
                .iter()
                .map(|name| format!("*{lt} {name}"))
                .collect();
            let label = format!("move from lifetime: {}", moved.join(", "));
            lines.push(format!(
                r#"{lifetime_node} -. "{}" .-> {step_node}"#,
                escape_label(&label)
            ));
        }
    }
}

fn apply_lifetime_updates(
    tracker: &mut ArtifactTracker,
    step_node: &str,
    outputs: &[String],
    lines: &mut Vec<String>,
    counter: &mut usize,
) {
    let mut per_lt: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();
    for output in outputs {
        let parsed = parse_artifact(output);
        if parsed.access != ArtifactAccess::Borrowed {
            continue;
        }
        let Some(lt) = parsed.lifetime else {
            continue;
        };

        let entry = per_lt.entry(lt.to_string()).or_default();
        if parsed.mutable {
            entry.1.push(parsed.name.to_string());
        } else {
            entry.0.push(parsed.name.to_string());
        }
    }

    for (lt, (mut shared, mut mutable)) in per_lt {
        let lifetime_node = ensure_lifetime_node(tracker, &lt, lines, counter);

        shared.sort();
        shared.dedup();
        if !shared.is_empty() {
            let assigned: Vec<String> = shared
                .iter()
                .map(|name| format!("&{lt} {name}"))
                .collect();
            let label = format!("assign to lifetime: {}", assigned.join(", "));
            lines.push(format!(
                r#"{step_node} -. "{}" .-> {lifetime_node}"#,
                escape_label(&label)
            ));
        }

        mutable.sort();
        mutable.dedup();
        if !mutable.is_empty() {
            let assigned: Vec<String> = mutable
                .iter()
                .map(|name| format!("&{lt} mut {name}"))
                .collect();
            let label = format!("assign mut to lifetime: {}", assigned.join(", "));
            lines.push(format!(
                r#"{step_node} -. "{}" .-> {lifetime_node}"#,
                escape_label(&label)
            ));
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

        let rendered = to_mermaid(&graph, None, &HashSet::new());
        assert!(rendered.contains("lifetime 'a"));
        assert!(!rendered.contains(":::ctx"));
        assert!(!rendered.contains("ctx set:"));
        assert!(rendered.contains("borrow: &'a x"));
        assert!(rendered.contains("assign to lifetime: &'a x"));
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

        let rendered = to_mermaid(&graph, None, &HashSet::new());
        assert!(rendered.contains(":::ctx"));
        assert!(!rendered.contains("lifetime '"));
    }
}
