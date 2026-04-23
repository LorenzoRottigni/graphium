//! Expression expansion helpers for the graph macro.
//!
//! This module groups the building blocks used to expand `NodeExpr` trees into
//! generated Rust execution code. It keeps looping, sequencing, branching, and
//! selector logic together so the macro can treat expression expansion as a
//! cohesive concern.

mod dispatch;
mod loops;
mod parallel;
mod payload;
mod route;
mod selector;
mod single;

pub(super) use dispatch::{capture_outputs, contains_break, get_node_expr};
pub(super) use loops::{get_loop_node_expr, get_while_node_expr, loop_exit_outputs};
pub(super) use parallel::{
    collect_parallel_entry_usage, get_parallel_nodes_expr, get_sequence_nodes_expr,
};
pub(super) use payload::{
    assign_outputs_to_slots, prepare_move_payload, prepare_output_slots, prepare_parallel_payload,
};
pub(super) use route::{get_route_node_expr, route_exit_outputs};
pub(super) use selector::{
    SelectorParam, build_condition_bindings, build_condition_call, build_selector_bindings,
    build_selector_call, selector_params_for_on_expr,
};
pub(super) use single::{get_single_node_expr, graph_type_path};
