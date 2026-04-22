use std::collections::HashMap;

#[derive(Clone)]
pub(crate) struct UiIndex<T> {
    pub(crate) ordered: Vec<T>,
    pub(crate) by_id: HashMap<String, T>,
}

impl<T: Clone> UiIndex<T> {
    pub(crate) fn from_ordered<F>(ordered: Vec<T>, id: F) -> Self
    where
        F: Fn(&T) -> &str,
    {
        let by_id = ordered
            .iter()
            .cloned()
            .map(|item| (id(&item).to_string(), item))
            .collect::<HashMap<_, _>>();
        Self { ordered, by_id }
    }

    pub(crate) fn insert(&mut self, id: impl Into<String>, item: T) {
        let id = id.into();
        if !self.by_id.contains_key(&id) {
            self.ordered.push(item.clone());
        }
        self.by_id.insert(id, item);
    }
}
