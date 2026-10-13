#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Share {
    pub id: String,
    pub url: String,
    pub description: Option<String>,
    pub expires_at: Option<String>,
}
