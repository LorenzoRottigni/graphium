use crate::node::Context;
use graphio_macro::graph;

enum Status {
    Valid,
    Invalid,
    NeedsReview,
}

graph! {
    #[metadata(context = Context)]
    DataGraph1 {
        crate::node::GetDataNode() >>
        crate::node::ValidateDataNode() >>
        crate::node::NormalizeDataNode() >>
        crate::node::PrintDataNode() & crate::node::SendEmailNode() & crate::node::PublishEventNode() >>
        crate::node::DisconnectFromDbNode()
    }
}

graph! {
    #[metadata(context = Context)]
    DataGraph2 {
        crate::node::GetDataNode() >>
        crate::node::ValidateDataNode() >>
        crate::node::NormalizeDataNode() >>
        crate::node::PrintDataNode() & crate::node::SendEmailNode() & crate::node::PublishEventNode() >>
        crate::node::DisconnectFromDbNode() >>
        DataGraph1
    }
}

graph! {
    #[metadata(context = Context)]
    DataGraph {
        crate::node::GetDataNode() >>
        crate::node::ValidateDataNode() >>
        @route {
            on: |ctx: &mut Context| Status::Invalid,
            routes: {
                Status::Valid => crate::node::PrintDataNode() & crate::node::SendEmailNode(),
                Status::Invalid => crate::node::PrintErrorNode(),
                Status::NeedsReview => crate::node::SendReviewNode(),
            }
        }
        >>
        crate::node::DisconnectFromDbNode()
    }
}

graph! {
    #[metadata(context = Context)]
    PropsGraph {
        crate::node::Node1Node() -> (data1, data2, data3) >>
        crate::node::Node2Node(data1, data2) & crate::node::Node3Node(data2, data3)
    }
}

graph! {
    #[metadata(
        context = Context,
        inputs = (data2: String, data3: String),
        outputs = (data4: String),
    )]
    DataGraphWithProps {
        crate::node::Node3Node(data2, data3) >>
        crate::node::Node1Node() -> (data4, _unused_data2, _unused_data3)
    }
}

graph! {
    #[metadata(context = Context)]
    PropsNestedGraph {
        crate::node::Node1Node() -> (data1, data2, data3) >>
        crate::node::Node2Node(data1, data2) &
        DataGraphWithProps(data2, data3) -> (data4) >>
        crate::node::Node2Node(data4, data4)
    }
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
