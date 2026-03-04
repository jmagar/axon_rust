use super::constants::{ALLOWED_FLAGS, ASYNC_MODES, NO_JSON_MODES};

fn evaluate_events_mode(flags: &serde_json::Value) -> bool {
    flags
        .as_object()
        .and_then(|obj| obj.get("responses_mode"))
        .and_then(serde_json::Value::as_str)
        .map(|value| value.eq_ignore_ascii_case("events"))
        .unwrap_or(false)
}

pub(super) fn build_args(mode: &str, input: &str, flags: &serde_json::Value) -> Vec<String> {
    let is_async = ASYNC_MODES.contains(&mode);
    let mut args: Vec<String> = vec![mode.to_string()];

    let trimmed = input.trim();
    if !trimmed.is_empty() {
        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        let is_job_subcmd = matches!(
            parts[0],
            "cancel" | "status" | "errors" | "list" | "cleanup" | "clear" | "worker" | "recover"
        );
        if is_job_subcmd {
            for part in parts {
                let p = part.trim().trim_start_matches('-');
                if !p.is_empty() {
                    args.push(p.to_string());
                }
            }
        } else {
            let sanitized = trimmed.trim_start_matches('-');
            if !sanitized.is_empty() {
                args.push(sanitized.to_string());
            }
        }
    }

    let disable_json_for_evaluate_events = mode == "evaluate" && evaluate_events_mode(flags);
    if !NO_JSON_MODES.contains(&mode) && !disable_json_for_evaluate_events {
        args.push("--json".to_string());
    }
    if mode == "scrape" {
        args.push("--embed".to_string());
        args.push("false".to_string());
    }

    if let Some(obj) = flags.as_object() {
        for (json_key, cli_flag) in ALLOWED_FLAGS {
            if is_async && *json_key == "wait" {
                continue;
            }
            if let Some(val) = obj.get(*json_key) {
                match val {
                    serde_json::Value::Bool(true) => {
                        args.push(cli_flag.to_string());
                    }
                    serde_json::Value::Bool(false) => {
                        args.push(cli_flag.to_string());
                        args.push("false".to_string());
                    }
                    serde_json::Value::Number(n) => {
                        args.push(cli_flag.to_string());
                        args.push(n.to_string());
                    }
                    serde_json::Value::String(s) if !s.is_empty() => {
                        // Guard output-dir values against path traversal attacks.
                        // Any value containing a `..` component is rejected before it
                        // reaches the subprocess, preventing a caller from redirecting
                        // output outside the expected output root.
                        if cli_flag.contains("output") && cli_flag.contains("dir") {
                            let p = std::path::Path::new(s.as_str());
                            if p.components().any(|c| c == std::path::Component::ParentDir) {
                                log::warn!("rejecting output-dir with path traversal: {s}");
                                continue;
                            }
                        }
                        args.push(cli_flag.to_string());
                        args.push(s.clone());
                    }
                    _ => {}
                }
            }
        }
    }

    args
}
