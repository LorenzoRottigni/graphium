use graphio_macro::node;

use crate::data::ctx::Context;

node! {
    pub fn get_number(_ctx: &mut Context) -> u32 {
        42
    }
}

node! {
    pub fn add(_ctx: &mut Context, a: u32, b: u32) -> u32 {
        a + b
    }
}

node! {
    pub fn subtract(_ctx: &mut Context, a: u32, b: u32) -> u32 {
        a - b
    }
}

node! {
    pub fn multiply(_ctx: &mut Context, a: u32, b: u32) -> u32 {
        a * b
    }
}

node! {
    pub fn modulo(_ctx: &mut Context, a: u32, b: u32) -> u32 {
        assert_ne!(b, 0, "modulo by zero");
        a % b
    }
}

node! {
    pub fn divide(_ctx: &mut Context, a: u32, b: u32) -> u32 {
        assert_ne!(b, 0, "division by zero");
        a / b
    }
}

node! {
    pub fn duplicate(_ctx: &mut Context, a: u32) -> (u32, u32) {
        (a, a)
    }
}

node! {
    pub fn equal(_ctx: &mut Context, a: u32, b: u32) -> bool {
        a == b
    }
}

node! {
    pub fn print_number(_ctx: &mut Context, a: u32) {
        println!("{}", a)
    }
}

node! {
    pub fn panic_with(_ctx: &mut Context, a: u32) {
        panic!("Panic with {}", a)
    }
}

node! {
    pub fn pipe_number(_ctx: &mut Context, a: u32) -> u32 {
        a
    }
}

node! {
    pub fn store_number(ctx: &mut Context, a: u32) {
        ctx.a = a
    }
}

node! {
    pub fn take_ownership(ctx: &mut Context) -> u32 {
        ctx.a
    }
}
