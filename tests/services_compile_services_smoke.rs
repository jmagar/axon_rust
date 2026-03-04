#[test]
fn services_module_exports_exist() {
    let _ = axon::crates::services::events::ServiceEvent::Log {
        level: "info".to_string(),
        message: "ok".to_string(),
    };
}
