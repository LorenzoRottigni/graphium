use graphium_macro::node;

node! {
    pub async fn create_product() -> String {
        return "created".into()
    }
}