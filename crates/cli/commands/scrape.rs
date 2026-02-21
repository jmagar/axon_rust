use crate::crates::core::config::{Config, ScrapeFormat};
use crate::crates::core::content::{
    extract_meta_description, find_between, to_markdown, url_to_filename,
};
use crate::crates::core::http::{normalize_url, validate_url};
use crate::crates::core::logging::log_done;
use crate::crates::core::ui::{muted, primary, print_option, print_phase};
use crate::crates::vector::ops::embed_path_native;
use std::error::Error;
use std::time::Duration;

fn build_scrape_client(cfg: &Config) -> Result<reqwest::Client, Box<dyn Error>> {
    let timeout = Duration::from_millis(cfg.request_timeout_ms.unwrap_or(20_000).max(1_000));
    let mut builder = reqwest::Client::builder().timeout(timeout);

    if let Some(proxy_url) = cfg.chrome_proxy.as_deref() {
        builder = builder.proxy(reqwest::Proxy::all(proxy_url)?);
    }
    if let Some(user_agent) = cfg.chrome_user_agent.as_deref() {
        builder = builder.user_agent(user_agent);
    }

    Ok(builder.build()?)
}

async fn fetch_html_resilient(
    cfg: &Config,
    client: &reqwest::Client,
    url: &str,
) -> Result<String, Box<dyn Error>> {
    let normalized = normalize_url(url);
    validate_url(&normalized)?;

    let retries = cfg.fetch_retries;
    let mut attempt = 0usize;
    loop {
        let response = client.get(&normalized).send().await;
        match response {
            Ok(resp) if resp.status().is_success() => {
                return Ok(resp.text().await?);
            }
            Ok(resp) => {
                let retryable = resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
                    || resp.status().is_server_error();
                if !retryable || attempt >= retries {
                    return Err(format!(
                        "scrape request failed with HTTP status {} for {}",
                        resp.status(),
                        normalized
                    )
                    .into());
                }
            }
            Err(err) => {
                if attempt >= retries {
                    return Err(format!("scrape request failed for {}: {}", normalized, err).into());
                }
            }
        }

        attempt = attempt.saturating_add(1);
        let backoff = cfg
            .retry_backoff_ms
            .max(100)
            .saturating_mul(attempt as u64)
            .min(5_000);
        tokio::time::sleep(Duration::from_millis(backoff)).await;
    }
}

pub async fn run_scrape(cfg: &Config, url: &str) -> Result<(), Box<dyn Error>> {
    print_phase("◐", "Scraping", url);
    println!("  {}", primary("Options:"));
    print_option("format", &format!("{:?}", cfg.format));
    print_option("proxy", cfg.chrome_proxy.as_deref().unwrap_or("none"));
    print_option(
        "userAgent",
        cfg.chrome_user_agent.as_deref().unwrap_or("spider-default"),
    );
    print_option(
        "timeoutMs",
        &cfg.request_timeout_ms.unwrap_or(20_000).to_string(),
    );
    print_option("fetchRetries", &cfg.fetch_retries.to_string());
    print_option("retryBackoffMs", &cfg.retry_backoff_ms.to_string());
    print_option("chromeAntiBot", &cfg.chrome_anti_bot.to_string());
    print_option("chromeStealth", &cfg.chrome_stealth.to_string());
    print_option("chromeIntercept", &cfg.chrome_intercept.to_string());
    print_option("embed", &cfg.embed.to_string());
    println!();

    let client = build_scrape_client(cfg)?;
    let html = fetch_html_resilient(cfg, &client, url).await?;
    let markdown = to_markdown(&html);

    let output = match cfg.format {
        ScrapeFormat::Markdown => markdown.clone(),
        ScrapeFormat::Html | ScrapeFormat::RawHtml => html.clone(),
        ScrapeFormat::Json => serde_json::to_string_pretty(&serde_json::json!({
            "url": url,
            "markdown": markdown,
            "title": find_between(&html, "<title>", "</title>").unwrap_or(""),
            "description": extract_meta_description(&html).unwrap_or_default(),
        }))?,
    };

    if let Some(path) = &cfg.output_path {
        tokio::fs::write(path, &output).await?;
        log_done(&format!("wrote output: {}", path.to_string_lossy()));
    } else {
        println!("{} {}", primary("Scrape Results for"), url);
        println!("{}\n", muted("As of: now"));
        println!("{output}");
    }

    if cfg.embed {
        let embed_dir = cfg.output_dir.join("scrape-markdown");
        tokio::fs::create_dir_all(&embed_dir).await?;
        tokio::fs::write(embed_dir.join(url_to_filename(url, 1)), markdown).await?;
        embed_path_native(cfg, &embed_dir.to_string_lossy()).await?;
    }

    Ok(())
}
