pub trait Artifact: Clone + 'static {}

impl<T> Artifact for T where T: Clone + 'static {}

pub fn clone_artifact<T: Artifact>(value: &T) -> T {
    value.clone()
}
