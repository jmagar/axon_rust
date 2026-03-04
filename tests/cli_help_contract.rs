use std::process::Command;

fn run_help(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_axon"))
        .args(args)
        .output()
        .expect("failed to execute axon binary");
    assert!(
        output.status.success(),
        "axon command failed: status={:?} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn refresh_schedule_help_exposes_worker_subcommand() {
    let stdout = run_help(&["refresh", "schedule", "--help"]);
    assert!(
        stdout.contains("worker"),
        "expected refresh schedule help to include worker subcommand, got:\n{stdout}"
    );
}

#[test]
fn youtube_help_describes_video_url_or_id_only() {
    let stdout = run_help(&["youtube", "--help"]);
    assert!(
        stdout.contains("YouTube video URL or bare video ID"),
        "expected youtube help to describe video URL or bare ID input, got:\n{stdout}"
    );
}

#[test]
fn top_level_help_describes_http_mcp_runtime() {
    let stdout = run_help(&["--help"]);
    assert!(
        stdout.contains("Start MCP HTTP server runtime"),
        "expected top-level help to describe HTTP MCP runtime, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("Start MCP stdio server"),
        "top-level help still advertises stdio MCP runtime:\n{stdout}"
    );
}
