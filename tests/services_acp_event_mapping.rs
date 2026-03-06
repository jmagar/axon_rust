use agent_client_protocol::{
    ContentChunk, PermissionOption, PermissionOptionId, PermissionOptionKind,
    RequestPermissionRequest, SessionConfigKind, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOption, SessionConfigSelectOptions, SessionNotification, SessionUpdate,
    ToolCallId, ToolCallUpdate, ToolCallUpdateFields,
};
use axon::crates::services::acp::{
    map_permission_request, map_permission_request_event, map_session_notification,
    map_session_notification_event, map_session_update_kind,
};
use axon::crates::services::events::ServiceEvent;
use axon::crates::services::types::{AcpBridgeEvent, AcpSessionUpdateKind};

#[test]
fn map_session_notification_agent_delta_extracts_text_and_kind() {
    let notification = SessionNotification::new(
        "session-123",
        SessionUpdate::AgentMessageChunk(ContentChunk::new("delta text".into())),
    );
    let mapped = map_session_notification(&notification);
    assert_eq!(mapped.session_id, "session-123");
    assert_eq!(mapped.kind, AcpSessionUpdateKind::AssistantDelta);
    assert_eq!(mapped.text_delta.as_deref(), Some("delta text"));
    assert!(mapped.tool_call_id.is_none());
}

#[test]
fn map_session_update_kind_tool_call_update_is_typed() {
    let update = SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
        ToolCallId::new("tool-1"),
        ToolCallUpdateFields::new().title("Read file"),
    ));
    assert_eq!(
        map_session_update_kind(&update),
        AcpSessionUpdateKind::ToolCallUpdated
    );
}

#[test]
fn map_permission_request_extracts_option_ids_and_tool_call() {
    let permission_request = RequestPermissionRequest::new(
        "session-77",
        ToolCallUpdate::new(
            ToolCallId::new("tool-77"),
            ToolCallUpdateFields::new().title("Write file"),
        ),
        vec![
            PermissionOption::new(
                PermissionOptionId::new("allow-once"),
                "Allow once",
                PermissionOptionKind::AllowOnce,
            ),
            PermissionOption::new(
                PermissionOptionId::new("reject-once"),
                "Reject once",
                PermissionOptionKind::RejectOnce,
            ),
        ],
    );

    let mapped = map_permission_request(&permission_request);
    assert_eq!(mapped.session_id, "session-77");
    assert_eq!(mapped.tool_call_id, "tool-77");
    assert_eq!(
        mapped.option_ids,
        vec!["allow-once".to_string(), "reject-once".to_string()]
    );
}

#[test]
fn map_session_notification_event_wraps_bridge_event() {
    let notification = SessionNotification::new(
        "session-wrap",
        SessionUpdate::AgentMessageChunk(ContentChunk::new("wrapped delta".into())),
    );

    let event = map_session_notification_event(&notification);
    match event {
        ServiceEvent::AcpBridge {
            event: AcpBridgeEvent::SessionUpdate(update),
        } => {
            assert_eq!(update.session_id, "session-wrap");
            assert_eq!(update.kind, AcpSessionUpdateKind::AssistantDelta);
            assert_eq!(update.text_delta.as_deref(), Some("wrapped delta"));
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn map_session_notification_event_emits_config_options_update_for_config_changes() {
    let option = SessionConfigOption::new(
        "choice",
        "Model Selector",
        SessionConfigKind::Select(agent_client_protocol::SessionConfigSelect::new(
            "gpt-5.4",
            SessionConfigSelectOptions::Ungrouped(vec![
                SessionConfigSelectOption::new("gpt-5.4", "GPT 5.4"),
                SessionConfigSelectOption::new("gpt-5.3-codex", "GPT 5.3 Codex"),
            ]),
        )),
    )
    .category(SessionConfigOptionCategory::Model);
    let notification = SessionNotification::new(
        "session-config",
        SessionUpdate::ConfigOptionUpdate(agent_client_protocol::ConfigOptionUpdate::new(vec![
            option,
        ])),
    );

    let event = map_session_notification_event(&notification);
    match event {
        ServiceEvent::AcpBridge {
            event: AcpBridgeEvent::ConfigOptionsUpdate(options),
        } => {
            assert_eq!(options.len(), 1);
            assert_eq!(options[0].id, "choice");
            assert_eq!(options[0].category.as_deref(), Some("model"));
            assert_eq!(options[0].options.len(), 2);
            assert_eq!(options[0].options[0].value, "gpt-5.4");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn map_permission_request_event_wraps_bridge_event() {
    let permission_request = RequestPermissionRequest::new(
        "session-wrap-2",
        ToolCallUpdate::new(
            ToolCallId::new("tool-wrap"),
            ToolCallUpdateFields::new().title("Wrapped request"),
        ),
        vec![PermissionOption::new(
            PermissionOptionId::new("allow"),
            "Allow",
            PermissionOptionKind::AllowOnce,
        )],
    );

    let event = map_permission_request_event(&permission_request);
    match event {
        ServiceEvent::AcpBridge {
            event: AcpBridgeEvent::PermissionRequest(req),
        } => {
            assert_eq!(req.session_id, "session-wrap-2");
            assert_eq!(req.tool_call_id, "tool-wrap");
            assert_eq!(req.option_ids, vec!["allow".to_string()]);
        }
        other => panic!("unexpected event: {other:?}"),
    }
}
