#[derive(Debug, Clone, Default, PartialEq)]
pub struct UpdateCheck {
    pub latest: String,
    pub notes: Option<String>,
    pub url: Option<String>,
    pub has_update: bool,
}
