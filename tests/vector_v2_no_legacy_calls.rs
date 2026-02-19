#[test]
fn ops_v2_modules_do_not_call_legacy_functions() {
    let files = [
        include_str!("../crates/vector/ops_v2/commands.rs"),
        include_str!("../crates/vector/ops_v2/qdrant.rs"),
        include_str!("../crates/vector/ops_v2/tei.rs"),
        include_str!("../crates/vector/ops_v2/stats.rs"),
    ];

    for source in files {
        assert!(
            !source.contains("ops_legacy::"),
            "ops_v2 source still contains ops_legacy function calls"
        );
    }
}
