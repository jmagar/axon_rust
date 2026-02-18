use crate::axon_cli::crates::core::config::{Config, ScrapeFormat};
use crate::axon_cli::crates::core::content::{
    extract_meta_description, find_between, to_markdown, url_to_filename,
};
use crate::axon_cli::crates::core::http::{build_client, fetch_html};
use crate::axon_cli::crates::core::logging::log_done;
use crate::axon_cli::crates::core::ui::{muted, primary, print_option, print_phase};
use crate::axon_cli::crates::vector::ops::embed_path_native;
use std::error::Error;

pub async fn run_scrape(cfg: &Config, url: &str) -> Result<(), Box<dyn Error>> {
    print_phase("◐", "Scraping", url);
    println!("  {}", primary("Options:"));
    print_option("format", &format!("{:?}", cfg.format));
    print_option("embed", &cfg.embed.to_string());
    println!();

    let client = build_client(20)?;
    let html = fetch_html(&client, url).await?;
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
