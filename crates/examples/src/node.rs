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
    #[outputs(data1, data2, data3)]
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
