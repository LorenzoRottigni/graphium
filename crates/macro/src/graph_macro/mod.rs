//! Code generation for the `graph!` procedural macro.
//!
//! The graph expander is split by concern so the hop-based orchestration rules
//! are easier to navigate and test.

mod analysis;
mod dispatch;
mod expand;
mod execution;
mod flow;
mod loops;
mod parallel;
mod payload;
mod route;
mod selector;
mod single;

pub use expand::expand;

use analysis::{
    analyze_expr, collect_parallel_borrowed, collect_parallel_outputs, collect_route_borrowed,
    collect_route_outputs, required_artifacts, required_borrowed,
};
use dispatch::{capture_outputs, contains_break, get_node_expr};
use loops::{get_loop_node_expr, get_while_node_expr, loop_exit_outputs};
use parallel::{collect_parallel_entry_usage, get_parallel_nodes_expr, get_sequence_nodes_expr};
use payload::{
    assign_outputs_to_slots, prepare_move_payload, prepare_output_slots, prepare_parallel_payload,
};
use route::{get_route_node_expr, route_exit_outputs};
use selector::{
    SelectorParam, build_condition_bindings, build_condition_call, build_selector_bindings,
    build_selector_call, selector_params_for_on_expr,
};
use single::{get_single_node_expr, graph_type_path};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_expand_entrypoint() {
        let _entry: fn(proc_macro::TokenStream) -> proc_macro::TokenStream = expand;
    }
}
