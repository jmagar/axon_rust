#[allow(dead_code)]
pub(crate) const STAGE_NAME: &str = "processor";

use crate::axon_cli::crates::core::config::RenderMode;
use crate::axon_cli::crates::jobs::crawl_jobs_v2::sitemap;
use std::error::Error;

#[derive(Debug, Clone)]
pub(crate) struct StartPlan {
    pub start_url: String,
    pub initial_mode: RenderMode,
}

pub(crate) fn resolve_initial_mode(
    render_mode: RenderMode,
    cache_skip_browser: bool,
) -> RenderMode {
    if cache_skip_browser {
        return RenderMode::Http;
    }
    match render_mode {
        RenderMode::AutoSwitch => RenderMode::Http,
        mode => mode,
    }
}

pub(crate) fn build_start_plan(
    start_url: &str,
    render_mode: RenderMode,
    cache_skip_browser: bool,
    exclude_path_prefix: &[String],
) -> Result<StartPlan, Box<dyn Error>> {
    let canonical_start_url =
        sitemap::canonicalize_url(start_url).ok_or("invalid crawl start URL")?;
    if sitemap::is_excluded_url_path(&canonical_start_url, exclude_path_prefix) {
        return Err("crawl start URL is excluded by configured path prefixes".into());
    }
    Ok(StartPlan {
        start_url: canonical_start_url,
        initial_mode: resolve_initial_mode(render_mode, cache_skip_browser),
    })
}

#[cfg(test)]
mod tests {
    use super::build_start_plan;
    use crate::axon_cli::crates::core::config::RenderMode;

    #[test]
    fn build_start_plan_normalizes_url_and_resolves_initial_mode() {
        let plan = build_start_plan(
            "https://example.com/path/#frag",
            RenderMode::AutoSwitch,
            false,
            &[],
        )
        .expect("build plan");

        assert_eq!(plan.start_url, "https://example.com/path".to_string());
        assert!(matches!(plan.initial_mode, RenderMode::Http));
    }

    #[test]
    fn build_start_plan_rejects_excluded_start_url() {
        let err = build_start_plan(
            "https://example.com/private/area",
            RenderMode::Chrome,
            false,
            &["/private".to_string()],
        )
        .expect_err("excluded start URL must fail");
        assert!(err
            .to_string()
            .contains("excluded by configured path prefixes"));
    }
}
