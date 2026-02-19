use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct QdrantPayload {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub chunk_text: String,
    #[serde(default)]
    pub text: String,
    pub chunk_index: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QdrantPoint {
    #[serde(default)]
    pub payload: QdrantPayload,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QdrantSearchHit {
    pub score: f64,
    #[serde(default)]
    pub payload: QdrantPayload,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QdrantSearchResponse {
    #[serde(default)]
    pub(crate) result: Vec<QdrantSearchHit>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QdrantScrollResult {
    #[serde(default)]
    pub(crate) points: Vec<QdrantPoint>,
    pub(crate) next_page_offset: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QdrantScrollResponse {
    pub(crate) result: QdrantScrollResult,
}

pub(crate) const RETRIEVE_MAX_POINTS_CEILING: usize = 500;
