use graphio_macro::graph;

graph! {
    name: DataGraph,
    context: crate::node::Context,
    nodes: [crate::node::get_data >> crate::node::validate_data >> crate::node::normalize_data >> crate::node::print_data & crate::node::send_email & crate::node::publish_event >> crate::node::disconnect_from_db]
}