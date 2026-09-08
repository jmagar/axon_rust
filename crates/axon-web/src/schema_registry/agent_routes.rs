//! Durable agent turn lifecycle routes, separate from the Codex control plane.

use super::{RestRouteSpec, read, write};

pub(super) static AGENT_ROUTES: &[RestRouteSpec] = &[
    read(
        "GET",
        "/v1/agent/turns/{id}",
        "v1_agent_status",
        "AgentTurnResult",
    ),
    read(
        "GET",
        "/v1/agent/turns/{id}/events",
        "v1_agent_events",
        "serde_json::Value",
    ),
    write(
        "POST",
        "/v1/agent/turns/{id}/cancel",
        "v1_agent_cancel",
        None,
        "AgentTurnResult",
    ),
    write(
        "POST",
        "/v1/agent/turns/{id}/resume",
        "v1_agent_resume",
        Some("AgentResumeRequest"),
        "AgentTurnResult",
    ),
];
