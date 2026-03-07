// std::env::set_var / remove_var are unsafe in Rust 1.81+ (POSIX multi-thread constraint).
// These integration tests are the only place in the codebase that need this, and they
// hold a process-level Mutex to serialize access. Package-level `deny(unsafe_code)`
// is overridden at the file level here, which is permitted because `deny` (unlike
// `forbid`) can be narrowed by a child `allow`.
#![allow(unsafe_code)]

/// Regression tests: spawn_adapter() must strip specific env vars from the child.
///
/// Background:
/// - Claude Code sets `CLAUDECODE=1` in every child process it spawns.
/// - When axon runs inside a Claude Code session (local dev, pre-commit hooks),
///   `claude-agent-acp` inherits `CLAUDECODE` and the inner `claude` CLI detects
///   a nested session, printing "Claude Code cannot be launched inside another
///   Claude Code session" and exiting 1. This was the root cause of the
///   "Query closed before response received" error in Pulse Chat.
/// - `OPENAI_BASE_URL`, `OPENAI_API_KEY`, `OPENAI_MODEL` point at Axon's local LLM
///   proxy. If inherited, the claude/codex adapters would try to use the wrong
///   endpoint and authentication scheme.
///
/// Fix: `spawn_adapter()` calls `command.env_remove()` for each of these vars.
///
/// These tests inject poison values into the current process env, spawn a child
/// command that would expose those values if inherited, and assert the output is
/// empty. They use a process-level mutex so env mutations don't race with other
/// tests in the same binary.
use axon::crates::services::acp::AcpClientScaffold;
use axon::crates::services::types::AcpAdapterCommand;
use std::sync::Mutex;

/// Global lock: env var mutation is not thread-safe; serialize these tests.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Vars that spawn_adapter() MUST strip before exec-ing the adapter subprocess.
const STRIPPED_VARS: &[&str] = &[
    "CLAUDECODE",
    "OPENAI_BASE_URL",
    "OPENAI_API_KEY",
    "OPENAI_MODEL",
];

/// Spawn a shell that prints the concatenated values of all STRIPPED_VARS.
/// Returns the trimmed stdout. Empty string means all vars were absent/stripped.
async fn run_env_probe() -> String {
    // "printf '%s' $VAR1$VAR2..." — empty vars contribute nothing; non-empty
    // vars produce visible output that fails the assertion.
    let args_inner = STRIPPED_VARS
        .iter()
        .map(|v| format!("\"${v}\""))
        .collect::<Vec<_>>()
        .join("");
    let cmd = format!("printf '%s' {args_inner}");

    let adapter = AcpAdapterCommand {
        program: "sh".to_string(),
        args: vec!["-c".to_string(), cmd],
        cwd: None,
    };
    let scaffold = AcpClientScaffold::new(adapter);
    let child = scaffold
        .spawn_adapter()
        .expect("spawn_adapter should succeed");
    let output = child
        .wait_with_output()
        .await
        .expect("child should complete");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// When CLAUDECODE is set in the parent environment, the child must NOT inherit it.
///
/// Regression: if `command.env_remove("CLAUDECODE")` is removed from spawn_adapter(),
/// this test fails whenever it runs inside a Claude Code session (which pre-commit
/// hooks do by definition).
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn spawn_adapter_strips_claudecode_nested_session_guard() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    // SAFETY: ENV_LOCK is held; no concurrent env mutation in this process.
    unsafe { std::env::set_var("CLAUDECODE", "test_poison_nested_session") };

    let output = run_env_probe().await;

    // SAFETY: ENV_LOCK is held.
    unsafe { std::env::remove_var("CLAUDECODE") };
    assert!(
        output.is_empty(),
        "CLAUDECODE must be stripped from child env by spawn_adapter(), \
         but child still saw it (output: {output:?})"
    );
}

/// When Axon's LLM proxy vars are set in the parent, the child must NOT inherit them.
///
/// ACP adapters authenticate directly (OAuth / API keys stored in ~/.claude or
/// ~/.codex). If the Axon proxy vars leak in, the adapter calls the wrong endpoint.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn spawn_adapter_strips_llm_proxy_vars() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    // SAFETY: ENV_LOCK is held; no concurrent env mutation in this process.
    unsafe {
        std::env::set_var("OPENAI_BASE_URL", "http://poison.axon-proxy.test/v1");
        std::env::set_var("OPENAI_API_KEY", "sk-poison-axon-proxy-test");
        std::env::set_var("OPENAI_MODEL", "poison-axon-model");
    }

    let output = run_env_probe().await;

    // SAFETY: ENV_LOCK is held.
    unsafe {
        std::env::remove_var("OPENAI_BASE_URL");
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("OPENAI_MODEL");
    }
    assert!(
        output.is_empty(),
        "OPENAI_* proxy vars must be stripped from child env by spawn_adapter(), \
         but child still saw: {output:?}"
    );
}

/// Gemini auth vars (GEMINI_API_KEY, GOOGLE_API_KEY) must be passed through to the child,
/// not stripped. These are needed for Gemini CLI authentication.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn spawn_adapter_passes_through_gemini_auth_vars() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    const GEMINI_VARS: &[&str] = &["GEMINI_API_KEY", "GOOGLE_API_KEY"];
    const SENTINEL: &str = "gemini_test_sentinel";

    // SAFETY: ENV_LOCK is held; no concurrent env mutation in this process.
    unsafe {
        for v in GEMINI_VARS {
            std::env::set_var(v, SENTINEL);
        }
    }

    // Build a probe that prints the concatenated values of GEMINI_VARS.
    let args_inner = GEMINI_VARS
        .iter()
        .map(|v| format!("\"${v}\""))
        .collect::<Vec<_>>()
        .join("");
    let cmd = format!("printf '%s' {args_inner}");

    let adapter = AcpAdapterCommand {
        program: "sh".to_string(),
        args: vec!["-c".to_string(), cmd],
        cwd: None,
    };
    let scaffold = AcpClientScaffold::new(adapter);
    let child = scaffold
        .spawn_adapter()
        .expect("spawn_adapter should succeed");
    let output = child
        .wait_with_output()
        .await
        .expect("child should complete");
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // SAFETY: ENV_LOCK is held.
    unsafe {
        for v in GEMINI_VARS {
            std::env::remove_var(v);
        }
    }

    // Both vars should be present in the child's output (sentinel repeated twice).
    let expected = SENTINEL.repeat(GEMINI_VARS.len());
    assert_eq!(
        stdout, expected,
        "GEMINI_API_KEY and GOOGLE_API_KEY must be passed through to child env, \
         but child saw: {stdout:?} (expected: {expected:?})"
    );
}

/// All STRIPPED_VARS injected simultaneously — none must leak through.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn spawn_adapter_strips_all_isolation_vars_together() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    // SAFETY: ENV_LOCK is held; no concurrent env mutation in this process.
    unsafe {
        std::env::set_var("CLAUDECODE", "1");
        std::env::set_var("OPENAI_BASE_URL", "http://poison.test/v1");
        std::env::set_var("OPENAI_API_KEY", "sk-poison");
        std::env::set_var("OPENAI_MODEL", "poison");
    }

    let output = run_env_probe().await;

    // SAFETY: ENV_LOCK is held.
    unsafe {
        for v in STRIPPED_VARS {
            std::env::remove_var(v);
        }
    }
    assert!(
        output.is_empty(),
        "All isolation vars must be stripped together; child saw: {output:?}"
    );
}
