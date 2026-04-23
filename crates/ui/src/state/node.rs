#[derive(Clone)]
pub(crate) struct UiNode {
    pub(crate) dto: graphium::dto::NodeDto,
    pub(crate) graph_id: String,
}
