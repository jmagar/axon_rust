//! WebSocket event types for the `axon serve` execution bridge.
//!
//! All variants of [`WsEventV2`] are serialized as JSON with a `"type"` tag
//! and consumed by `apps/web`. Fields not constructed in Rust may still be
//! active wire protocol members.
use crate::crates::services::types::AcpBridgeEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandContext {
    pub exec_id: String,
    pub mode: String,
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobStatusPayload {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<BTreeMap<String, Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobProgressPayload {
    pub phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandDonePayload {
    pub exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandErrorPayload {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobCancelResponsePayload {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum WsEventV2 {
    #[serde(rename = "command.start")]
    CommandStart { ctx: CommandContext },
    #[serde(rename = "command.output.json")]
    CommandOutputJson { ctx: CommandContext, data: Value },
    #[serde(rename = "command.output.line")]
    CommandOutputLine { ctx: CommandContext, line: String },
    #[serde(rename = "command.done")]
    CommandDone {
        ctx: CommandContext,
        payload: CommandDonePayload,
    },
    #[serde(rename = "command.error")]
    CommandError {
        ctx: CommandContext,
        payload: CommandErrorPayload,
    },
    #[serde(rename = "job.status")]
    JobStatus {
        ctx: CommandContext,
        payload: JobStatusPayload,
    },
    #[serde(rename = "job.progress")]
    JobProgress {
        ctx: CommandContext,
        payload: JobProgressPayload,
    },
    #[serde(rename = "artifact.list")]
    ArtifactList {
        ctx: CommandContext,
        artifacts: Vec<ArtifactEntry>,
    },
    #[serde(rename = "artifact.content")]
    ArtifactContent {
        ctx: CommandContext,
        path: String,
        content: String,
    },
    #[serde(rename = "job.cancel.response")]
    JobCancelResponse {
        ctx: CommandContext,
        payload: JobCancelResponsePayload,
    },
}

pub(super) fn serialize_v2_event(event: WsEventV2) -> Option<String> {
    serde_json::to_string(&event).ok()
}

pub(super) fn acp_bridge_event_payload(event: &AcpBridgeEvent) -> Value {
    serde_json::to_value(event)
        .unwrap_or_else(|e| serde_json::json!({ "error": format!("serialization failed: {}", e) }))
}
