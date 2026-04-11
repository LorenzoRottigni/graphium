use crate::node::Context;
use graphio::Node;
use graphio_macro::graph;

enum Status {
    Valid,
    Invalid,
    NeedsReview,
}

graph! {
    name: DataGraph1,
    context: Context,
    schema: [crate::node::GetDataNode >> crate::node::ValidateDataNode >> crate::node::NormalizeDataNode >> crate::node::PrintDataNode & crate::node::SendEmailNode & crate::node::PublishEventNode >> crate::node::DisconnectFromDbNode]
}

graph! {
    name: DataGraph2,
    context: Context,
    schema: [crate::node::GetDataNode >> crate::node::ValidateDataNode >> crate::node::NormalizeDataNode >> crate::node::PrintDataNode & crate::node::SendEmailNode & crate::node::PublishEventNode >> crate::node::DisconnectFromDbNode >> DataGraph1::run]
}

graph! {
    name: DataGraph,
    context: Context,
    schema: [
        crate::node::GetDataNode >>
        crate::node::ValidateDataNode >>
        @route {
            on: |ctx: &mut Context| Status::Invalid,
            routes: {
                Status::Valid => crate::node::PrintDataNode & crate::node::SendEmailNode,
                Status::Invalid => crate::node::PrintErrorNode,
                Status::NeedsReview => crate::node::SendReviewNode,
            }
        }
        >>
        crate::node::DisconnectFromDbNode
    ]
}

graph! {
    name: PropsGraph,
    context: Context,
    schema: [
        crate::node::Node1Node >>
        crate::node::Node2Node
    ]
}

/*
graph! {
    name: DataGraph,
    context: Context,
    schema: [
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
