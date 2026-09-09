use super::*;
use std::fs;
use tempfile::TempDir;

const MCP_SOURCE: &str = include_str!("../../../crates/axon-cli/src/commands/mcp.rs");

fn write_all_required(root: &Path) {
    let mcp_rs = root.join("crates/axon-cli/src/commands/mcp.rs");
    fs::create_dir_all(mcp_rs.parent().unwrap()).unwrap();
    fs::write(
        &mcp_rs,
        "run_unified_server();\nlet stdio = axon_mcp::run_stdio_server_with_context;\n\
         match t { McpTransport::Both => {} }\n",
    )
    .unwrap();

    let cli_cfg = root.join("crates/axon-core/src/config/cli.rs");
    fs::create_dir_all(cli_cfg.parent().unwrap()).unwrap();
    fs::write(
        &cli_cfg,
        "pub struct C { pub transport: Option<McpTransport> }\n",
    )
    .unwrap();

    let build_cfg = root.join("crates/axon-core/src/config/parse/build_config/config_literal.rs");
    fs::create_dir_all(build_cfg.parent().unwrap()).unwrap();
    fs::write(
        &build_cfg,
        "let t = resolve_mcp_transport(mcp_transport, mcp_transport_default);\n",
    )
    .unwrap();

    let helpers = root.join("crates/axon-core/src/config/parse/helpers.rs");
    fs::create_dir_all(helpers.parent().unwrap()).unwrap();
    fs::write(&helpers, "// reads AXON_MCP_TRANSPORT env var\n").unwrap();
}

#[test]
fn passes_with_all_patterns_present() {
    let tmp = TempDir::new().unwrap();
    write_all_required(tmp.path());
    check(tmp.path()).expect("expected check to pass");
}

#[test]
fn accepts_current_shared_context_transport_wiring() {
    let tmp = TempDir::new().unwrap();
    write_all_required(tmp.path());
    fs::write(tmp.path().join(FILE_SPECS[0].0), MCP_SOURCE).unwrap();
    check(tmp.path()).expect("current shared-context transport wiring must pass");
}

#[test]
fn rejects_missing_transport_wiring() {
    for pattern in [
        "axon_mcp::run_stdio_server_with_context",
        "run_unified_server(",
        "McpTransport::Both =>",
    ] {
        let tmp = TempDir::new().unwrap();
        write_all_required(tmp.path());
        assert!(MCP_SOURCE.contains(pattern));
        fs::write(
            tmp.path().join(FILE_SPECS[0].0),
            MCP_SOURCE.replace(pattern, "removed_transport_wiring"),
        )
        .unwrap();
        check(tmp.path()).expect_err("missing transport wiring must fail");
    }
}

#[test]
fn fails_when_file_missing() {
    let tmp = TempDir::new().unwrap();
    write_all_required(tmp.path());
    fs::remove_file(tmp.path().join("crates/axon-cli/src/commands/mcp.rs")).unwrap();
    let err = check(tmp.path()).expect_err("expected missing file error");
    assert_eq!(
        err.to_string(),
        "ERROR: missing crates/axon-cli/src/commands/mcp.rs"
    );
}

#[test]
fn fails_when_pattern_missing() {
    let tmp = TempDir::new().unwrap();
    write_all_required(tmp.path());
    // Overwrite mcp.rs missing the `McpTransport::Both =>` arm. A bare `Both`
    // token (e.g., in a comment) must NOT satisfy the matcher.
    fs::write(
        tmp.path().join("crates/axon-cli/src/commands/mcp.rs"),
        "run_unified_server();\nlet stdio = axon_mcp::run_stdio_server_with_context;\n// keyword: Both\n",
    )
    .unwrap();
    let err = check(tmp.path()).expect_err("expected pattern error");
    assert!(
        err.to_string().contains("McpTransport::Both =>"),
        "error should reference the strengthened matcher, got: {err}"
    );
}

#[test]
fn pattern_table_is_canonical() {
    // Lock the table shape to catch accidental edits.
    let paths: Vec<&'static str> = FILE_SPECS.iter().map(|(p, _)| *p).collect();
    assert_eq!(
        paths,
        vec![
            "crates/axon-cli/src/commands/mcp.rs",
            "crates/axon-core/src/config/cli.rs",
            "crates/axon-core/src/config/parse/build_config/config_literal.rs",
            "crates/axon-core/src/config/parse/helpers.rs",
        ]
    );

    let mcp_patterns: Vec<&'static str> = FILE_SPECS[0].1.iter().map(|(p, _)| *p).collect();
    assert_eq!(
        mcp_patterns,
        vec![
            "run_unified_server(",
            "axon_mcp::run_stdio_server_with_context",
            "McpTransport::Both =>"
        ]
    );

    assert_eq!(FILE_SPECS[1].1.len(), 1);
    assert_eq!(FILE_SPECS[1].1[0].0, "transport: Option<McpTransport>");

    assert_eq!(FILE_SPECS[2].1.len(), 1);
    assert_eq!(
        FILE_SPECS[2].1[0].0,
        "resolve_mcp_transport(mcp_transport, mcp_transport_default)"
    );

    assert_eq!(FILE_SPECS[3].1.len(), 1);
    assert_eq!(FILE_SPECS[3].1[0].0, "AXON_MCP_TRANSPORT");
}
