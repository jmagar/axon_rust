use super::*;

#[cfg(unix)]
#[tokio::test]
async fn oversized_initialization_line_is_rejected_by_byte_limit() {
    use std::process::Stdio;
    let mut child = tokio::process::Command::new("/bin/sh")
        .args(["-c", "head -c 1048577 /dev/zero"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let result = run_init_handshake(&LlmBackendConfig::default(), &mut stdin, &mut stdout).await;
    let _ = child.kill().await;
    let _ = child.wait().await;
    let error = result.unwrap_err();
    assert!(error.to_string().contains("byte limit"));
}

#[test]
fn oversized_protocol_frame_is_rejected_before_deserialization() {
    let mut state = CodexTurnState::new();
    let error = state
        .handle_line(&"x".repeat(1024 * 1024 + 1), &mut |_| Ok(()))
        .unwrap_err();
    assert!(error.to_string().contains("byte limit"), "{error}");
}

#[test]
fn cumulative_protocol_output_is_bounded_before_callback() {
    let mut state = CodexTurnState::new();
    let line = serde_json::json!({"method":"item/agentMessage/delta", "params":{"delta":"x".repeat(512 * 1024)}}).to_string();
    let mut delivered = 0;
    let mut callback = |delta: &str| {
        delivered += delta.len();
        Ok(())
    };
    let result = (0..33).try_for_each(|_| state.handle_line(&line, &mut callback).map(|_| ()));
    assert!(result.is_err(), "cumulative output must be bounded");
    assert!(result.unwrap_err().to_string().contains("byte limit"));
    assert!(delivered <= 16 * 1024 * 1024);
}
