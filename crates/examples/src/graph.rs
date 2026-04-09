use graphio_macro::graph;
use crate::node::Context;

graph! {
    name: DataGraph1,
    context: Context,
    nodes: [crate::node::get_data >> crate::node::validate_data >> crate::node::normalize_data >> crate::node::print_data & crate::node::send_email & crate::node::publish_event >> crate::node::disconnect_from_db]
}

graph! {
    name: DataGraph2,
    context: Context,
    nodes: [crate::node::get_data >> crate::node::validate_data >> crate::node::normalize_data >> crate::node::print_data & crate::node::send_email & crate::node::publish_event >> crate::node::disconnect_from_db >> DataGraph1::run]
}