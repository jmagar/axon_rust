//! Maps WS execute request fields to [`ConfigOverrides`] for future service dispatch.
//!
//! This module is the boundary between the WebSocket request surface and the
//! services layer. Only safe, non-filesystem fields are accepted. Any value that
//! cannot be validated is rejected with a descriptive error so the caller can
//! surface it back to the client without spawning a subprocess.
use crate::crates::core::config::ConfigOverrides;
use crate::crates::core::config::RenderMode;

/// Valid `render_mode` strings accepted from WS clients.
const VALID_RENDER_MODES: &[&str] = &["http", "chrome", "auto-switch"];

/// Valid `response_mode` strings accepted from WS clients.
const VALID_RESPONSE_MODES: &[&str] = &["inline", "path", "artifact"];

/// Map WS execute request fields to a [`ConfigOverrides`] for service dispatch.
///
/// Only maps safe, non-filesystem fields. Returns `Err(String)` with a
/// descriptive message when a field value fails validation.
///
/// # Fields mapped
///
/// | WS field       | `ConfigOverrides` field | Validation |
/// |----------------|------------------------|------------|
/// | `render_mode`  | `render_mode`          | must be `"http"`, `"chrome"`, or `"auto-switch"` |
/// | `max_pages`    | `max_pages`            | any `u32` (0 = uncapped) |
/// | `wait`         | `wait`                 | any `bool` |
/// | `response_mode`| `response_mode`        | must be `"inline"`, `"path"`, or `"artifact"` |
pub fn ws_request_to_overrides(
    render_mode: Option<&str>,
    max_pages: Option<u32>,
    wait: Option<bool>,
    response_mode: Option<&str>,
) -> Result<WsConfigOverrides, String> {
    let mapped_render_mode = match render_mode {
        None => None,
        Some("") => {
            return Err("render_mode: must not be empty".to_string());
        }
        Some(raw) if !VALID_RENDER_MODES.contains(&raw) => {
            return Err(format!(
                "render_mode: invalid value '{raw}'; must be one of: {}",
                VALID_RENDER_MODES.join(", ")
            ));
        }
        Some("http") => Some(RenderMode::Http),
        Some("chrome") => Some(RenderMode::Chrome),
        Some("auto-switch") => Some(RenderMode::AutoSwitch),
        Some(_) => unreachable!("render_mode validated against VALID_RENDER_MODES above"),
    };

    let mapped_response_mode = match response_mode {
        None => None,
        Some("") => {
            return Err("response_mode: must not be empty".to_string());
        }
        Some(raw) if !VALID_RESPONSE_MODES.contains(&raw) => {
            return Err(format!(
                "response_mode: invalid value '{raw}'; must be one of: {}",
                VALID_RESPONSE_MODES.join(", ")
            ));
        }
        Some(s) => Some(s.to_string()),
    };

    Ok(WsConfigOverrides {
        overrides: ConfigOverrides {
            render_mode: mapped_render_mode,
            max_pages,
            wait,
            ..ConfigOverrides::default()
        },
        response_mode: mapped_response_mode,
    })
}

/// `ConfigOverrides` enriched with WS-specific fields that have no analog in
/// the core [`ConfigOverrides`] struct (e.g. `response_mode`).
///
/// Tasks 5.2 and 5.3 consume this type when dispatching to services.
#[derive(Debug, Default, Clone)]
pub struct WsConfigOverrides {
    /// Core overrides — ready to be applied via [`Config::apply_overrides`].
    pub overrides: ConfigOverrides,
    /// Controls whether command output is returned inline, by path, or as an
    /// artifact download.  `None` means the service chooses its default.
    pub response_mode: Option<String>,
}

// Convenience accessors so callers can address fields directly.
impl WsConfigOverrides {
    /// `render_mode` from the wrapped `ConfigOverrides`.
    pub fn render_mode(&self) -> Option<RenderMode> {
        self.overrides.render_mode
    }

    /// `max_pages` from the wrapped `ConfigOverrides`.
    pub fn max_pages(&self) -> Option<u32> {
        self.overrides.max_pages
    }

    /// `wait` from the wrapped `ConfigOverrides`.
    pub fn wait(&self) -> Option<bool> {
        self.overrides.wait
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_none_is_ok() {
        let r = ws_request_to_overrides(None, None, None, None);
        assert!(r.is_ok());
    }

    #[test]
    fn valid_render_modes_all_parse() {
        for mode in VALID_RENDER_MODES {
            let r = ws_request_to_overrides(Some(mode), None, None, None);
            assert!(r.is_ok(), "expected Ok for render_mode={mode}");
        }
    }

    #[test]
    fn valid_response_modes_all_parse() {
        for mode in VALID_RESPONSE_MODES {
            let r = ws_request_to_overrides(None, None, None, Some(mode));
            assert!(r.is_ok(), "expected Ok for response_mode={mode}");
        }
    }

    #[test]
    fn invalid_render_mode_is_err_with_field_name() {
        let e = ws_request_to_overrides(Some("ftp"), None, None, None).unwrap_err();
        assert!(e.contains("render_mode"));
    }

    #[test]
    fn invalid_response_mode_is_err_with_field_name() {
        let e = ws_request_to_overrides(None, None, None, Some("stream")).unwrap_err();
        assert!(e.contains("response_mode"));
    }
}
