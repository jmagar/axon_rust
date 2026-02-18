pub mod crates;
pub mod axon_cli {
    pub use super::crates;
}

use self::crates::cli::commands::{
    run_ask_native, run_batch, run_crawl, run_doctor, run_domains_native, run_embed, run_extract,
    run_map, run_query_native, run_retrieve_native, run_scrape, run_search, run_sources_native,
    run_stats_native, run_status, start_url_from_cfg,
};
use self::crates::core::config::{parse_args, CommandKind};
use self::crates::core::logging::{init_tracing, log_done, log_info};
use std::error::Error;

fn is_job_subcommand(cfg: &self::crates::core::config::Config) -> bool {
    matches!(
        cfg.positional.first().map(|s| s.as_str()),
        Some("status" | "cancel" | "errors" | "list" | "cleanup" | "clear" | "worker" | "doctor")
    )
}

fn job_subcommand_name(cfg: &self::crates::core::config::Config) -> Option<&str> {
    cfg.positional.first().map(|s| s.as_str()).filter(|s| {
        matches!(
            *s,
            "status" | "cancel" | "errors" | "list" | "cleanup" | "clear" | "worker" | "doctor"
        )
    })
}

fn is_async_enqueue_mode(cfg: &self::crates::core::config::Config) -> bool {
    !cfg.wait
        && matches!(
            cfg.command,
            CommandKind::Crawl | CommandKind::Batch | CommandKind::Extract | CommandKind::Embed
        )
        && !is_job_subcommand(cfg)
}

pub async fn run() -> Result<(), Box<dyn Error>> {
    init_tracing();
    let cfg = parse_args();

    let start_url = start_url_from_cfg(&cfg);

    log_info(&format!(
        "command={} start_url={} render_mode={:?} embed={} collection={} profile={:?}",
        cfg.command.as_str(),
        start_url,
        cfg.render_mode,
        cfg.embed,
        cfg.collection,
        cfg.performance_profile
    ));

    match cfg.command {
        CommandKind::Scrape => run_scrape(&cfg, &start_url).await?,
        CommandKind::Map => run_map(&cfg, &start_url).await?,
        CommandKind::Crawl => run_crawl(&cfg, &start_url).await?,
        CommandKind::Batch => run_batch(&cfg).await?,
        CommandKind::Extract => run_extract(&cfg).await?,
        CommandKind::Search => run_search(&cfg).await?,
        CommandKind::Embed => run_embed(&cfg).await?,
        CommandKind::Doctor => run_doctor(&cfg).await?,
        CommandKind::Query => run_query_native(&cfg).await?,
        CommandKind::Retrieve => run_retrieve_native(&cfg).await?,
        CommandKind::Ask => run_ask_native(&cfg).await?,
        CommandKind::Sources => run_sources_native(&cfg).await?,
        CommandKind::Domains => run_domains_native(&cfg).await?,
        CommandKind::Stats => run_stats_native(&cfg).await?,
        CommandKind::Status => run_status(&cfg).await?,
    }

    if is_async_enqueue_mode(&cfg) {
        log_done(&format!("command={} enqueued", cfg.command.as_str()));
    } else if let Some(sub) = job_subcommand_name(&cfg) {
        log_done(&format!("command={} {} done", cfg.command.as_str(), sub));
    } else {
        log_done(&format!("command={} complete", cfg.command.as_str()));
    }
    Ok(())
}
