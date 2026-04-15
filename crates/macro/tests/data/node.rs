use graphium_macro::node;

use crate::data::ctx::Context;

node! {
    pub fn get_number() -> u32 {
        42
    }
}

node! {
    pub fn add(a: u32, b: u32) -> u32 {
        a + b
    }
}

node! {
    pub fn subtract(a: u32, b: u32) -> u32 {
        a - b
    }
}

node! {
    pub fn multiply(a: u32, b: u32) -> u32 {
        a * b
    }
}

node! {
    pub fn modulo(a: u32, b: u32) -> u32 {
        assert_ne!(b, 0, "modulo by zero");
        a % b
    }
}

node! {
    pub fn divide(a: u32, b: u32) -> u32 {
        assert_ne!(b, 0, "division by zero");
        a / b
    }
}

node! {
    pub fn duplicate(a: u32) -> (u32, u32) {
        (a, a)
    }
}

node! {
    pub fn equal(a: u32, b: u32) -> bool {
        a == b
    }
}

node! {
    pub fn print_number(a: u32) {
        println!("{}", a)
    }
}

node! {
    pub fn panic_with(a: u32) {
        panic!("Panic with {}", a)
    }
}

node! {
    pub fn pipe_number(a: u32) -> u32 {
        a
    }
}

node! {
    pub fn store_number(_a: u32) {
    }
}

node! {
    pub fn take_ownership(_ctx: &Context, a: &u32) -> u32 {
        *a
    }
}
