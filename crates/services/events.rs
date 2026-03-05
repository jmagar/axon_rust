use crate::crates::services::types::AcpBridgeEvent;
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceEvent {
    Log { level: String, message: String },
    AcpBridge { event: AcpBridgeEvent },
}

pub fn emit(tx: &Option<mpsc::Sender<ServiceEvent>>, event: ServiceEvent) {
    if let Some(sender) = tx {
        let _ = sender.try_send(event);
    }
}
