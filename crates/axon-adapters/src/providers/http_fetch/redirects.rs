use super::*;

impl HttpFetchProvider {
    pub(super) async fn fetch_redirected(&self, request: FetchRequest) -> Result<FetchedResource> {
        let (response, chain) = self.follow_redirects(&request).await?;
        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            self.record_rate_limited().await;
            return Err(self.error("fetch.rate_limited", "provider returned HTTP 429"));
        }
        if status.is_server_error() {
            self.record_fatal().await;
            return Err(self.error(
                "fetch.server_error",
                format!("provider returned HTTP {}", status.as_u16()),
            ));
        }
        self.finish_success(request, response, status, chain).await
    }

    async fn follow_redirects(
        &self,
        request: &FetchRequest,
    ) -> Result<(reqwest::Response, Vec<String>)> {
        let original = reqwest::Url::parse(&request.uri)
            .map_err(|error| self.error("fetch.invalid_uri", error.to_string()))?;
        let credentialed = request_carries_credentials(request)
            || !original.username().is_empty()
            || original.password().is_some();
        let client = self.build_client()?;
        let mut url = original.clone();
        let mut method = self.method(&request.method)?;
        let mut body = request
            .body
            .as_ref()
            .map(|body| self.encode_body(body))
            .transpose()?;
        let mut chain = Vec::new();
        let mut drop_entity_headers = false;
        loop {
            validate_url(url.as_str())
                .map_err(|error| self.error("fetch.invalid_uri", error.to_string()))?;
            let mut outgoing = client.request(method.clone(), url.clone());
            for header in &request.headers.headers {
                if drop_entity_headers
                    && ["content-length", "content-type", "transfer-encoding"]
                        .iter()
                        .any(|name| header.name.eq_ignore_ascii_case(name))
                {
                    continue;
                }
                outgoing = outgoing.header(&header.name, &header.value);
            }
            if let Some(body) = &body {
                outgoing = outgoing.body(body.clone());
            }
            let response = match outgoing.send().await {
                Ok(response) => response,
                Err(error) => {
                    self.record_fatal().await;
                    return Err(self.error("fetch.transport", error.to_string()));
                }
            };
            if !matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
                return Ok((response, chain));
            }
            let Some(location) = response.headers().get(reqwest::header::LOCATION) else {
                return Ok((response, chain));
            };
            let next = location
                .to_str()
                .ok()
                .and_then(|value| url.join(value).ok())
                .ok_or_else(|| self.error("fetch.transport", "invalid redirect location"))?;
            validate_url(next.as_str())
                .map_err(|error| self.error("fetch.invalid_uri", error.to_string()))?;
            // Preserve the previous policy's bound (previous includes the
            // original request), as well as its conservative credential rule.
            if chain.len() + 1 >= MAX_REDIRECTS {
                return Err(self.error("fetch.transport", "too many redirects"));
            }
            if credentialed && !redirect_can_forward_credentials(&original, &next) {
                return Err(self.error(
                    "fetch.transport",
                    "refusing to follow a credentialed redirect across an origin boundary",
                ));
            }
            if matches!(response.status().as_u16(), 301..=303)
                && method != Method::GET
                && method != Method::HEAD
            {
                method = Method::GET;
                body = None;
                drop_entity_headers = true;
            }
            chain.push(next.to_string());
            url = next;
        }
    }
}
