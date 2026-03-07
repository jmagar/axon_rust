use agent_client_protocol::ProtocolVersion;
use axon::crates::services::acp::{
    AcpClientScaffold, AcpSessionSetupRequest, validate_prompt_turn_request, validate_session_cwd,
};
use axon::crates::services::types::{AcpAdapterCommand, AcpPromptTurnRequest};
use std::path::Path;

fn test_scaffold() -> AcpClientScaffold {
    AcpClientScaffold::new(AcpAdapterCommand {
        program: "cat".to_string(),
        args: vec!["--stdio".to_string()],
        cwd: None,
    })
}

#[test]
fn prepare_initialize_builds_latest_protocol_request() {
    let scaffold = test_scaffold();
    let request = scaffold
        .prepare_initialize()
        .expect("initialize request should be constructible");
    assert_eq!(request.protocol_version, ProtocolVersion::LATEST);
}

#[test]
fn prepare_initialize_fails_for_invalid_adapter() {
    let scaffold = AcpClientScaffold::new(AcpAdapterCommand {
        program: "   ".to_string(),
        args: vec![],
        cwd: None,
    });
    let err = scaffold
        .prepare_initialize()
        .expect_err("invalid adapter should fail");
    assert!(err.to_string().contains("cannot be empty"));
}

#[test]
fn prepare_session_setup_builds_new_session_when_no_session_id() {
    let scaffold = test_scaffold();
    let req = AcpPromptTurnRequest {
        session_id: None,
        prompt: vec!["hello".to_string()],
        model: None,
        mcp_servers: vec![],
    };
    let setup = scaffold
        .prepare_session_setup(&req, "/tmp")
        .expect("new session setup should build");

    match setup {
        AcpSessionSetupRequest::New(new_req) => {
            assert_eq!(new_req.cwd, Path::new("/tmp"));
            assert!(new_req.mcp_servers.is_empty());
        }
        AcpSessionSetupRequest::Load(_) => {
            panic!("expected new session request");
        }
    }
}

#[test]
fn prepare_session_setup_builds_load_session_when_session_id_present() {
    let scaffold = test_scaffold();
    let req = AcpPromptTurnRequest {
        session_id: Some("session-42".to_string()),
        prompt: vec!["continue".to_string()],
        model: None,
        mcp_servers: vec![],
    };
    let setup = scaffold
        .prepare_session_setup(&req, "/tmp")
        .expect("load session setup should build");

    match setup {
        AcpSessionSetupRequest::Load(load_req) => {
            assert_eq!(load_req.cwd, Path::new("/tmp"));
            assert_eq!(load_req.session_id.0.as_ref(), "session-42");
            assert!(load_req.mcp_servers.is_empty());
        }
        AcpSessionSetupRequest::New(_) => {
            panic!("expected load session request");
        }
    }
}

#[test]
fn prepare_session_setup_rejects_blank_session_id() {
    let scaffold = test_scaffold();
    let req = AcpPromptTurnRequest {
        session_id: Some("   ".to_string()),
        prompt: vec!["continue".to_string()],
        model: None,
        mcp_servers: vec![],
    };
    let err = scaffold
        .prepare_session_setup(&req, "/tmp")
        .expect_err("blank session id should fail");
    assert!(err.to_string().contains("session_id cannot be blank"));
}

#[test]
fn prepare_session_setup_rejects_relative_cwd() {
    let scaffold = test_scaffold();
    let req = AcpPromptTurnRequest {
        session_id: None,
        prompt: vec!["hello".to_string()],
        model: None,
        mcp_servers: vec![],
    };
    let err = scaffold
        .prepare_session_setup(&req, "relative/path")
        .expect_err("relative cwd should fail");
    assert!(err.to_string().contains("must be an absolute path"));
}

#[test]
fn validate_prompt_turn_request_rejects_empty_prompt() {
    let req = AcpPromptTurnRequest {
        session_id: None,
        prompt: vec![],
        model: None,
        mcp_servers: vec![],
    };
    let err = validate_prompt_turn_request(&req).expect_err("empty prompt should fail");
    assert!(err.to_string().contains("at least one prompt block"));
}

#[test]
fn validate_session_cwd_rejects_relative_path() {
    let err = validate_session_cwd(Path::new("relative")).expect_err("relative path should fail");
    assert!(err.to_string().contains("must be an absolute path"));
}
