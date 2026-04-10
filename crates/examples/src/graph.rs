use graphio_macro::graph;
use crate::node::Context;

enum Status {
    Valid,
    Invalid,
    NeedsReview,
}

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


graph! {
    name: DataGraph,
    context: Context,
    nodes: [
        crate::node::get_data >>
        crate::node::validate_data >>
        @route {
            on: |ctx: &mut Context| Status::Invalid,
            routes: {
                Status::Valid => crate::node::print_data & crate::node::send_email,
                Status::Invalid => crate::node::print_error,
                Status::NeedsReview => crate::node::send_review,
            }
        }
        >>
        crate::node::disconnect_from_db
    ]
}

/*
graph! {
    name: DataGraph,
    context: Context,
    nodes: [
        crate::node::get_data >>
        crate::node::validate_data >>
        @route {
            on: |ctx: &mut Context| Status::Invalid,
            routes: {
                Status::Valid => crate::node::print_data & crate::node::send_email,
                Status::Invalid => crate::node::print_error,
                Status::NeedsReview => crate::node::send_review,
            }
        }
        >>
        @loop {
            condition: |ctx: &mut Context, i: usize| i < 3,
            body: crate::node::print_data
        }
        >>
        crate::node::disconnect_from_db
    ]
}
*/