pub mod crates;

use self::crates::cli::commands::{
    run_ask_native, run_batch, run_crawl, run_debug, run_dedupe_native, run_doctor,
    run_domains_native, run_embed, run_evaluate_native, run_extract, run_github, run_map,
    run_query_native, run_reddit, run_retrieve_native, run_scrape, run_search, run_sessions,
    run_sources_native, run_stats_native, run_status, run_suggest_native, run_youtube,
    start_url_from_cfg,
};
use self::crates::core::config::{parse_args, CommandKind, Config};
use self::crates::core::logging::{init_tracing, log_done, log_info, log_warn};
use sqlx::postgres::PgPoolOptions;
use std::error::Error;
use std::time::Duration;

async fn record_command_run(cfg: &Config) {
    if cfg.pg_url.is_empty() {
        return;
    }
    let attempt = async {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&cfg.pg_url)
            .await?;
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS axon_command_runs (
                id BIGSERIAL PRIMARY KEY,
                command TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&pool)
        .await?;
        sqlx::query("INSERT INTO axon_command_runs (command) VALUES ($1)")
            .bind(cfg.command.as_str())
            .execute(&pool)
            .await?;
        Ok::<(), sqlx::Error>(())
    };
    let _ = tokio::time::timeout(Duration::from_secs(2), attempt).await;
}

async fn run_once(cfg: &Config, start_url: &str) -> Result<(), Box<dyn Error>> {
    match cfg.command {
        CommandKind::Scrape => run_scrape(cfg, start_url).await?,
        CommandKind::Map => run_map(cfg, start_url).await?,
        CommandKind::Crawl => run_crawl(cfg, start_url).await?,
        CommandKind::Batch => run_batch(cfg).await?,
        CommandKind::Extract => run_extract(cfg).await?,
        CommandKind::Search => run_search(cfg).await?,
        CommandKind::Embed => run_embed(cfg).await?,
        CommandKind::Debug => run_debug(cfg).await?,
        CommandKind::Doctor => run_doctor(cfg).await?,
        CommandKind::Query => run_query_native(cfg).await?,
        CommandKind::Retrieve => run_retrieve_native(cfg).await?,
        CommandKind::Ask => run_ask_native(cfg).await?,
        CommandKind::Evaluate => run_evaluate_native(cfg).await?,
        CommandKind::Suggest => run_suggest_native(cfg).await?,
        CommandKind::Sources => run_sources_native(cfg).await?,
        CommandKind::Domains => run_domains_native(cfg).await?,
        CommandKind::Stats => run_stats_native(cfg).await?,
        CommandKind::Status => run_status(cfg).await?,
        CommandKind::Dedupe => run_dedupe_native(cfg).await?,
        CommandKind::Github => run_github(cfg).await?,
        CommandKind::Reddit => run_reddit(cfg).await?,
        CommandKind::Youtube => run_youtube(cfg).await?,
        CommandKind::Sessions => run_sessions(cfg).await?,
    }
    Ok(())
}

fn is_job_subcommand(cfg: &self::crates::core::config::Config) -> bool {
    matches!(
        cfg.positional.first().map(|s| s.as_str()),
        Some("status" | "cancel" | "errors" | "list" | "cleanup" | "clear" | "worker" | "recover")
    )
}

fn job_subcommand_name(cfg: &self::crates::core::config::Config) -> Option<&str> {
    cfg.positional.first().map(|s| s.as_str()).filter(|s| {
        matches!(
            *s,
            "status" | "cancel" | "errors" | "list" | "cleanup" | "clear" | "worker" | "recover"
        )
    })
}

fn is_async_enqueue_mode(cfg: &self::crates::core::config::Config) -> bool {
    !cfg.wait
        && matches!(
            cfg.command,
            CommandKind::Crawl
                | CommandKind::Batch
                | CommandKind::Extract
                | CommandKind::Embed
                | CommandKind::Github
                | CommandKind::Reddit
                | CommandKind::Youtube
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
    {
        let cfg_clone = cfg.clone();
        tokio::spawn(async move {
            record_command_run(&cfg_clone).await;
        });
    }

    if let Some(every_seconds) = cfg.cron_every_seconds {
        if is_job_subcommand(&cfg) {
            return Err(
                "--cron-every-seconds is not supported for job subcommands (status/cancel/list/etc)"
                    .into(),
            );
        }
        let max_runs = cfg.cron_max_runs.unwrap_or(usize::MAX);
        let mut run_count = 0usize;
        while run_count < max_runs {
            run_count += 1;
            log_info(&format!(
                "cron run {} command={} interval={}s",
                run_count,
                cfg.command.as_str(),
                every_seconds
            ));
            match run_once(&cfg, &start_url).await {
                Ok(_) => {}
                Err(e) => {
                    log_warn(&format!("cron run_once failed: {e:#}"));
                }
            }
            if run_count < max_runs {
                tokio::time::sleep(Duration::from_secs(every_seconds)).await;
            }
        }
        log_done(&format!(
            "command={} cron complete runs={}",
            cfg.command.as_str(),
            run_count
        ));
        return Ok(());
    }
    run_once(&cfg, &start_url).await?;

    if is_async_enqueue_mode(&cfg) {
        log_done(&format!("command={} enqueued", cfg.command.as_str()));
    } else if let Some(sub) = job_subcommand_name(&cfg) {
        log_done(&format!("command={} {} done", cfg.command.as_str(), sub));
    } else {
        log_done(&format!("command={} complete", cfg.command.as_str()));
    }
    Ok(())
}
