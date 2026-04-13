pub mod data;

use data::ctx::Context;
use graphio_macro::graph;

#[test]
fn e2e_graph_macro_simple() {
    let mut ctx = Context::default();

    graph! {
        #[metadata(context = Context)]
        PrintGraph {
            data::node::GetNumber() -> (number) >>
            data::node::PanicWith(number)
        }    
    }
    PrintGraph::__graphio_run(&mut ctx);
    // let y = AddGraph::__graphio_run(&mut ctx, 4, 3);

    // assert_eq!(y, 7);
}
