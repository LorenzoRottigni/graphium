// Core runtime helpers shared by generated graphs.
// The runtime surface is intentionally tiny: the macro emits mostly plain Rust
// and only relies on this trait/function pair to express "this value can be
// duplicated when a hop fans out to multiple consumers".

pub trait Artifact: Clone + 'static {}

impl<T> Artifact for T where T: Clone + 'static {}

pub fn clone_artifact<T: Artifact>(value: &T) -> T {
    value.clone()
}
