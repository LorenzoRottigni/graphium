use graphio::Node;
use graphio_macro::node;
#[derive(Default)]
pub struct Context {
    pub data: String,
}

node! {
    pub fn get_data(ctx: &mut Context) {
        ctx.data = "raw data".to_string();
        println!("getData -> {}", ctx.data);
    }
}

node! {
    pub fn validate_data(ctx: &mut Context) {
        ctx.data = format!("validated({})", ctx.data);
        println!("validateData -> {}", ctx.data);
    }
}

node! {
    pub fn normalize_data(ctx: &mut Context) {
        ctx.data = format!("normalized({})", ctx.data);
        println!("normalizeData -> {}", ctx.data);
    }
}

node! {
    pub fn print_data(ctx: &mut Context) {
        println!("printData -> {}", ctx.data);
    }
}

node! {
    pub fn print_error(ctx: &mut Context) {
        println!("printError -> ERROR");
    }
}

node! {
    pub fn send_review(ctx: &mut Context) {
        println!("sendReview -> {}", ctx.data);
    }
}

node! {
    pub fn send_email(ctx: &mut Context) {
        println!("sendEmailWithData -> {}", ctx.data);
    }
}

node! {
    pub fn publish_event(ctx: &mut Context) {
        println!("publishDataEvent -> {}", ctx.data);
    }
}

node! {
    pub fn disconnect_from_db(_ctx: &mut Context) {
        println!("disconnectFromDb");
    }
}

node! {
    #[artifacts(data1, data2, data3)]
    pub fn node1(_ctx: &mut Context) -> (String, String, String) {
        let data1 = "data1value".to_string();
        let data2 = "data2value".to_string();
        let data3 = "data3value".to_string();
        (data1, data2, data3)
    }
}

node! {
    pub fn node2(_ctx: &mut Context, data1: String, data3: String) {
        println!("node2 -> {}, {}", data1, data3);
    }
}

/*
 - node1 can produce multiple artifacts exposing them to the next node.
 - artifacts can only be owned values, so its produced from nodeB and then cloned/copied only for next nodes (one or more) that need it.
   Mimic rust move semantics and ownership rules. (if there is only 1 next node the value is moved, otherwise is cloned/copied for each next node that needs it, using a trait Artifact we can ensure user passes values implementing clone and copy trait).
 - If borrowing is needed, this must happens through the context since the borrowed values must live somewhere and previous node isn't still alive.
 */