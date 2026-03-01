use crate::crates::jobs::crawl::CrawlJob;
use crate::crates::jobs::extract::ExtractJob;
use crate::crates::jobs::ingest::IngestJob;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct JobStatusResponse {
    pub id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urls: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urls_json: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_json: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_json: Option<serde_json::Value>,
}

impl JobStatusResponse {
    pub fn from_crawl(job: &CrawlJob) -> Self {
        Self {
            id: job.id,
            status: job.status.clone(),
            created_at: job.created_at,
            updated_at: job.updated_at,
            started_at: job.started_at,
            finished_at: job.finished_at,
            error: job.error_text.clone(),
            error_text: job.error_text.clone(),
            url: Some(job.url.clone()),
            source_type: None,
            target: None,
            urls: None,
            urls_json: None,
            metrics: job.result_json.clone(),
            result_json: job.result_json.clone(),
            config_json: None,
        }
    }

    pub fn from_extract(job: &ExtractJob) -> Self {
        Self {
            id: job.id,
            status: job.status.clone(),
            created_at: job.created_at,
            updated_at: job.updated_at,
            started_at: job.started_at,
            finished_at: job.finished_at,
            error: job.error_text.clone(),
            error_text: job.error_text.clone(),
            url: None,
            source_type: None,
            target: None,
            urls: Some(job.urls_json.clone()),
            urls_json: Some(job.urls_json.clone()),
            metrics: job.result_json.clone(),
            result_json: job.result_json.clone(),
            config_json: None,
        }
    }

    pub fn from_ingest(job: &IngestJob) -> Self {
        Self {
            id: job.id,
            status: job.status.clone(),
            created_at: job.created_at,
            updated_at: job.updated_at,
            started_at: job.started_at,
            finished_at: job.finished_at,
            error: job.error_text.clone(),
            error_text: job.error_text.clone(),
            url: None,
            source_type: Some(job.source_type.clone()),
            target: Some(job.target.clone()),
            urls: None,
            urls_json: None,
            metrics: job.result_json.clone(),
            result_json: job.result_json.clone(),
            config_json: Some(job.config_json.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct JobCancelResponse {
    pub id: Uuid,
    pub canceled: bool,
    pub source: &'static str,
}

impl JobCancelResponse {
    pub fn new(id: Uuid, canceled: bool) -> Self {
        Self {
            id,
            canceled,
            source: "rust",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct JobErrorsResponse {
    pub id: Uuid,
    pub status: String,
    pub error: Option<String>,
}

impl JobErrorsResponse {
    pub fn from_job(id: Uuid, status: String, error: Option<String>) -> Self {
        Self { id, status, error }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct JobSummaryEntry {
    pub id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urls: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urls_json: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_json: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_json: Option<serde_json::Value>,
}

impl JobSummaryEntry {
    pub fn from_crawl(job: &CrawlJob) -> Self {
        Self {
            id: job.id,
            status: job.status.clone(),
            created_at: job.created_at,
            updated_at: job.updated_at,
            started_at: job.started_at,
            finished_at: job.finished_at,
            error: job.error_text.clone(),
            error_text: job.error_text.clone(),
            url: Some(job.url.clone()),
            source_type: None,
            target: None,
            urls: None,
            urls_json: None,
            metrics: job.result_json.clone(),
            result_json: job.result_json.clone(),
            config_json: None,
        }
    }

    pub fn from_extract(job: &ExtractJob) -> Self {
        Self {
            id: job.id,
            status: job.status.clone(),
            created_at: job.created_at,
            updated_at: job.updated_at,
            started_at: job.started_at,
            finished_at: job.finished_at,
            error: job.error_text.clone(),
            error_text: job.error_text.clone(),
            url: None,
            source_type: None,
            target: None,
            urls: Some(job.urls_json.clone()),
            urls_json: Some(job.urls_json.clone()),
            metrics: job.result_json.clone(),
            result_json: job.result_json.clone(),
            config_json: None,
        }
    }

    pub fn from_ingest(job: &IngestJob) -> Self {
        Self {
            id: job.id,
            status: job.status.clone(),
            created_at: job.created_at,
            updated_at: job.updated_at,
            started_at: job.started_at,
            finished_at: job.finished_at,
            error: job.error_text.clone(),
            error_text: job.error_text.clone(),
            url: None,
            source_type: Some(job.source_type.clone()),
            target: Some(job.target.clone()),
            urls: None,
            urls_json: None,
            metrics: job.result_json.clone(),
            result_json: job.result_json.clone(),
            config_json: Some(job.config_json.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn test_ts() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 2, 24, 12, 0, 0)
            .single()
            .expect("valid timestamp")
    }

    fn test_crawl_job() -> CrawlJob {
        CrawlJob {
            id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("valid uuid"),
            url: "https://example.com".to_string(),
            status: "running".to_string(),
            created_at: test_ts(),
            updated_at: test_ts(),
            started_at: Some(test_ts()),
            finished_at: None,
            error_text: None,
            result_json: Some(serde_json::json!({"pages_crawled": 3})),
        }
    }

    fn test_extract_job() -> ExtractJob {
        ExtractJob {
            id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("valid uuid"),
            status: "pending".to_string(),
            created_at: test_ts(),
            updated_at: test_ts(),
            started_at: None,
            finished_at: None,
            error_text: Some("boom".to_string()),
            urls_json: serde_json::json!(["https://example.com"]),
            result_json: Some(serde_json::json!({"items": 2})),
        }
    }

    fn test_ingest_job() -> IngestJob {
        IngestJob {
            id: Uuid::parse_str("33333333-3333-3333-3333-333333333333").expect("valid uuid"),
            status: "completed".to_string(),
            source_type: "github".to_string(),
            target: "owner/repo".to_string(),
            created_at: test_ts(),
            updated_at: test_ts(),
            started_at: Some(test_ts()),
            finished_at: Some(test_ts()),
            error_text: None,
            result_json: Some(serde_json::json!({"chunks": 99})),
            config_json: serde_json::json!({"collection": "cortex"}),
        }
    }

    fn serialize_list(entries: Vec<JobSummaryEntry>) -> serde_json::Value {
        let serialized = serde_json::to_string(&entries).expect("serialize");
        serde_json::from_str(&serialized).expect("parse")
    }

    #[test]
    fn crawl_status_includes_shared_and_legacy_metric_fields() {
        let json = serde_json::to_value(JobStatusResponse::from_crawl(&test_crawl_job()))
            .expect("serialize");
        assert_eq!(json["url"], "https://example.com");
        assert_eq!(json["status"], "running");
        assert_eq!(json["metrics"], serde_json::json!({"pages_crawled": 3}));
        assert_eq!(json["result_json"], serde_json::json!({"pages_crawled": 3}));
        assert_eq!(json["error"], serde_json::Value::Null);
        assert_eq!(json["error_text"], serde_json::Value::Null);
    }

    #[test]
    fn extract_status_includes_auditable_urls_aliases() {
        let json = serde_json::to_value(JobStatusResponse::from_extract(&test_extract_job()))
            .expect("serialize");
        let expected_urls = serde_json::json!(["https://example.com"]);
        assert_eq!(json["urls"], expected_urls);
        assert_eq!(json["urls_json"], expected_urls);
        assert_eq!(json["metrics"], serde_json::json!({"items": 2}));
        assert_eq!(json["result_json"], serde_json::json!({"items": 2}));
        assert_eq!(json["error"], "boom");
        assert_eq!(json["error_text"], "boom");
    }

    #[test]
    fn ingest_status_includes_shared_and_legacy_config_fields() {
        let json = serde_json::to_value(JobStatusResponse::from_ingest(&test_ingest_job()))
            .expect("serialize");
        assert_eq!(json["source_type"], "github");
        assert_eq!(json["target"], "owner/repo");
        assert_eq!(json["metrics"], serde_json::json!({"chunks": 99}));
        assert_eq!(json["result_json"], serde_json::json!({"chunks": 99}));
        assert_eq!(
            json["config_json"],
            serde_json::json!({"collection": "cortex"})
        );
    }

    #[test]
    fn list_payload_serialization_keeps_crawl_metrics() {
        let payload = serialize_list(vec![JobSummaryEntry::from_crawl(&test_crawl_job())]);
        let item = &payload[0];
        assert_eq!(item["url"], "https://example.com");
        assert_eq!(item["metrics"], serde_json::json!({"pages_crawled": 3}));
        assert_eq!(item["result_json"], serde_json::json!({"pages_crawled": 3}));
    }

    #[test]
    fn list_payload_serialization_keeps_extract_urls_and_metrics() {
        let payload = serialize_list(vec![JobSummaryEntry::from_extract(&test_extract_job())]);
        let item = &payload[0];
        let expected_urls = serde_json::json!(["https://example.com"]);
        assert_eq!(item["urls"], expected_urls);
        assert_eq!(item["urls_json"], expected_urls);
        assert_eq!(item["metrics"], serde_json::json!({"items": 2}));
        assert_eq!(item["result_json"], serde_json::json!({"items": 2}));
    }

    #[test]
    fn list_payload_serialization_keeps_ingest_source_target_and_config() {
        let payload = serialize_list(vec![JobSummaryEntry::from_ingest(&test_ingest_job())]);
        let item = &payload[0];
        assert_eq!(item["source_type"], "github");
        assert_eq!(item["target"], "owner/repo");
        assert_eq!(item["metrics"], serde_json::json!({"chunks": 99}));
        assert_eq!(item["result_json"], serde_json::json!({"chunks": 99}));
        assert_eq!(
            item["config_json"],
            serde_json::json!({"collection": "cortex"})
        );
    }

    #[test]
    fn cancel_and_errors_contracts_stay_stable() {
        let errors = serde_json::to_value(JobErrorsResponse::from_job(
            Uuid::nil(),
            "failed".to_string(),
            Some("boom".to_string()),
        ))
        .expect("serialize");
        assert_eq!(errors["id"], Uuid::nil().to_string());
        assert_eq!(errors["status"], "failed");
        assert_eq!(errors["error"], "boom");

        let cancel =
            serde_json::to_value(JobCancelResponse::new(Uuid::nil(), true)).expect("serialize");
        assert_eq!(cancel["id"], Uuid::nil().to_string());
        assert_eq!(cancel["canceled"], true);
        assert_eq!(cancel["source"], "rust");
    }
}
