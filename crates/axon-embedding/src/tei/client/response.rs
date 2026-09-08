//! Keep response-body I/O inside the embedding attempt's retry boundary.

pub(super) enum EmbedResponse {
    Vectors(Vec<Vec<f32>>),
    Status(reqwest::Response),
}

pub(super) async fn send_with_body(
    request: reqwest::RequestBuilder,
) -> Result<EmbedResponse, reqwest::Error> {
    let response = request.send().await?;
    if response.status().is_success() {
        Ok(EmbedResponse::Vectors(response.json().await?))
    } else {
        Ok(EmbedResponse::Status(response))
    }
}
