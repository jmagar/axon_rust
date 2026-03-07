use super::context::ExecCommandContext;
use super::events::{CommandDonePayload, CommandErrorPayload, WsEventV2, serialize_v2_event};
use tokio::sync::mpsc;

pub(super) async fn send_command_start(tx: &mpsc::Sender<String>, context: &ExecCommandContext) {
    if let Some(v2) = serialize_v2_event(WsEventV2::CommandStart {
        ctx: context.to_ws_ctx(),
    }) {
        let _ = tx.send(v2).await;
    }
}

pub(super) async fn send_command_output_line(
    tx: &mpsc::Sender<String>,
    context: &super::events::CommandContext,
    line: String,
) {
    if let Some(v2) = serialize_v2_event(WsEventV2::CommandOutputLine {
        ctx: context.clone(),
        line,
    }) {
        let _ = tx.send(v2).await;
    }
}

pub(super) async fn send_done_dual(
    tx: &mpsc::Sender<String>,
    context: &super::events::CommandContext,
    exit_code: i32,
    elapsed_ms: Option<u64>,
) {
    if let Some(v2) = serialize_v2_event(WsEventV2::CommandDone {
        ctx: context.clone(),
        payload: CommandDonePayload {
            exit_code,
            elapsed_ms,
        },
    }) {
        let _ = tx.send(v2).await;
    }
}

pub(super) async fn send_error_dual(
    tx: &mpsc::Sender<String>,
    context: &super::events::CommandContext,
    message: String,
    elapsed_ms: Option<u64>,
) {
    if let Some(v2) = serialize_v2_event(WsEventV2::CommandError {
        ctx: context.clone(),
        payload: CommandErrorPayload {
            message,
            elapsed_ms,
        },
    }) {
        let _ = tx.send(v2).await;
    }
}
