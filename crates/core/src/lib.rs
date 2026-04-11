use std::any::{Any, TypeId};
use std::collections::HashMap;

#[derive(Default)]
pub struct Store {
    values: HashMap<TypeId, Box<dyn Any>>,
    named_values: HashMap<&'static str, Box<dyn Any>>,
}

impl Store {
    pub fn insert<T: 'static>(&mut self, value: T) {
        self.values.insert(TypeId::of::<T>(), Box::new(value));
    }

    pub fn insert_named<T: 'static>(&mut self, name: &'static str, value: T) {
        self.named_values.insert(name, Box::new(value));
    }

    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.values
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref::<T>())
    }

    pub fn get_named<T: 'static>(&self, name: &'static str) -> Option<&T> {
        self.named_values
            .get(name)
            .and_then(|boxed| boxed.downcast_ref::<T>())
    }

    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.values
            .get_mut(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_mut::<T>())
    }

    pub fn get_named_mut<T: 'static>(&mut self, name: &'static str) -> Option<&mut T> {
        self.named_values
            .get_mut(name)
            .and_then(|boxed| boxed.downcast_mut::<T>())
    }

    pub fn take<T: 'static>(&mut self) -> Option<T> {
        self.values
            .remove(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast::<T>().ok())
            .map(|boxed| *boxed)
    }

    pub fn take_named<T: 'static>(&mut self, name: &'static str) -> Option<T> {
        self.named_values
            .remove(name)
            .and_then(|boxed| boxed.downcast::<T>().ok())
            .map(|boxed| *boxed)
    }
}

pub trait Node<Ctx> {
    fn run(ctx: &mut Ctx, store: &mut Store);
}
