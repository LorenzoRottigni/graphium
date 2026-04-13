use crate::node::Context;
use graphio_macro::{graph, graph_runtime};

enum Status {
    Valid,
    Invalid,
    NeedsReview,
}

graph! {
    #[metadata(context = Context)]
    DataGraph1 {
        crate::node::GetData() >>
        crate::node::ValidateData() >>
        crate::node::NormalizeData() >>
        crate::node::PrintData() & crate::node::SendEmail() & crate::node::PublishEvent() >>
        crate::node::DisconnectFromDb()
    }
}

graph_runtime! {
    #[metadata(context = Context)]
    RuntimeDataGraph {
        crate::node::GetData() >>
        crate::node::ValidateData() >>
        crate::node::NormalizeData() >>
        crate::node::DisconnectFromDb()
    }
}

graph! {
    #[metadata(context = Context)]
    DataGraph2 {
        crate::node::GetData() >>
        crate::node::ValidateData() >>
        crate::node::NormalizeData() >>
        crate::node::PrintData() & crate::node::SendEmail() & crate::node::PublishEvent() >>
        crate::node::DisconnectFromDb() >>
        DataGraph1
    }
}

graph! {
    #[metadata(context = Context)]
    DataGraph {
        crate::node::GetData() >>
        crate::node::ValidateData() >>
        @route {
            on: |ctx: &mut Context| Status::Invalid,
            routes: {
                Status::Valid => crate::node::PrintData() & crate::node::SendEmail(),
                Status::Invalid => crate::node::PrintError(),
                Status::NeedsReview => crate::node::SendReview(),
            }
        }
        >>
        crate::node::DisconnectFromDb()
    }
}

graph! {
    #[metadata(context = Context)]
    PropsGraph {
        crate::node::Node1() -> (data1, data2, data3) >>
        crate::node::Node2(data1, data2) & crate::node::Node3(data2, data3)
    }
}

graph! {
    #[metadata(
        context = Context,
        inputs = (data2: String, data3: String),
        outputs = (data4: String),
    )]
    DataGraphWithProps {
        crate::node::Node3(data2, data3) >>
        crate::node::Node1() -> (data4, _unused_data2, _unused_data3)
    }
}

graph! {
    #[metadata(context = Context)]
    PropsNestedGraph {
        crate::node::Node1() -> (data1, data2, data3) >>
        crate::node::Node2(data1, data2) &
        DataGraphWithProps(data2, data3) -> (data4) >>
        crate::node::Node2(data4, data4)
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
