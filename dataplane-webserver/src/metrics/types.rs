use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct RangeQuery {
    pub query: String,
    pub start: f64,
    pub end: Option<f64>,
    pub step: Option<String>,
}
