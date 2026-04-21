#[derive(Debug)]
pub enum UiError {
    EmptyGraphs,
    Bind(std::io::Error),
}

impl std::fmt::Display for UiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiError::EmptyGraphs => write!(f, "graphium-ui-next config requires at least one graph"),
            UiError::Bind(err) => write!(f, "failed to bind graphium-ui-next server: {err}"),
        }
    }
}

impl std::error::Error for UiError {}
