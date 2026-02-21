use crate::crates::core::config::Config;
use crate::crates::core::content::extract_links;
use crate::crates::core::http::{fetch_html, http_client};
use crate::crates::core::logging::log_done;
use crate::crates::core::ui::{muted, primary, print_option, print_phase};
use std::error::Error;

pub async fn run_search(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let query = if let Some(q) = &cfg.query {
        q.clone()
    } else if !cfg.positional.is_empty() {
        cfg.positional.join(" ")
    } else {
        return Err("search requires a query (positional or --query)".into());
    };

    let encoded: String = spider::url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    let url = format!("https://duckduckgo.com/html/?q={encoded}");
    print_phase("◐", "Searching", &query);
    println!("  {}", primary("Options:"));
    print_option("limit", &cfg.search_limit.to_string());
    println!();

    let client = http_client()?;
    let html = fetch_html(client, &url).await?;

    let mut links = extract_links(&html, cfg.search_limit * 5);
    links.retain(|l| !l.contains("duckduckgo.com") && !l.contains("javascript:void"));
    links.dedup();
    links.truncate(cfg.search_limit);

    println!("{}", primary(&format!("Search Results for \"{query}\"")));
    println!("{} {}", muted("Showing"), links.len());
    println!();

    for link in links {
        println!("  • {link}");
    }

    log_done("command=search complete");
    Ok(())
}
