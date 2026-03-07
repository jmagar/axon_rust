#[test]
fn services_module_exports_exist() {
    let _ = axon::crates::services::events::ServiceEvent::Log {
        level: axon::crates::services::events::LogLevel::Info,
        message: "ok".to_string(),
    };
}
