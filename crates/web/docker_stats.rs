use bollard::Docker;
use bollard::container::{ListContainersOptions, StatsOptions};
use futures_util::StreamExt;
use serde_json::json;
use std::collections::HashMap;
use tokio::sync::broadcast;
use tracing::warn;

const CONTAINER_PREFIX: &str = "axon-";
const POLL_INTERVAL_MS: u64 = 500;

/// Previous absolute counters for rate calculations.
///
/// Per-container absolute I/O counters from the previous poll cycle, used to
/// compute byte-per-second rates via `(current - prev) / dt`.
struct PreviousSnapshot {
    timestamp: std::time::Instant,
    net_rx: HashMap<String, u64>,
    net_tx: HashMap<String, u64>,
    block_read: HashMap<String, u64>,
    block_write: HashMap<String, u64>,
}

/// Computed per-container metrics for a single poll cycle.
struct ContainerMetrics {
    name: String,
    cpu_percent: f64,
    memory_percent: f64,
    memory_usage_mb: f64,
    memory_limit_mb: f64,
    net_rx_rate: f64,
    net_tx_rate: f64,
    block_read_rate: f64,
    block_write_rate: f64,
    status: String,
}

/// Main stats polling loop — broadcasts to all WS clients via the channel.
pub(super) async fn run_stats_loop(tx: broadcast::Sender<String>) {
    let docker = match Docker::connect_with_local_defaults() {
        Ok(d) => d,
        Err(e) => {
            warn!("Docker not available for stats: {e} — stats broadcasting disabled");
            // Don't crash the server — just stop the stats poller
            return;
        }
    };

    let mut prev = PreviousSnapshot {
        timestamp: std::time::Instant::now(),
        net_rx: HashMap::new(),
        net_tx: HashMap::new(),
        block_read: HashMap::new(),
        block_write: HashMap::new(),
    };

    loop {
        let metrics = poll_once(&docker, &mut prev).await;
        let message = build_stats_message(&metrics);

        // If nobody is listening, that's fine — just drop the message
        let _ = tx.send(message);

        tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
    }
}

async fn poll_once(docker: &Docker, prev: &mut PreviousSnapshot) -> Vec<ContainerMetrics> {
    let now = std::time::Instant::now();
    let dt = now.duration_since(prev.timestamp).as_secs_f64().max(0.1);
    prev.timestamp = now;

    let filters: HashMap<String, Vec<String>> = HashMap::from([
        ("name".to_string(), vec![CONTAINER_PREFIX.to_string()]),
        ("status".to_string(), vec!["running".to_string()]),
    ]);

    let containers = match docker
        .list_containers(Some(ListContainersOptions {
            all: false,
            filters,
            ..Default::default()
        }))
        .await
    {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to list containers: {e}");
            return vec![];
        }
    };

    let mut results = Vec::new();

    for container in containers {
        let name = container
            .names
            .as_ref()
            .and_then(|n| n.first())
            .map(|n| n.trim_start_matches('/').to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let id = match &container.id {
            Some(id) => id.clone(),
            None => continue,
        };

        let stats = match docker
            .stats(
                &id,
                Some(StatsOptions {
                    stream: false,
                    one_shot: true,
                }),
            )
            .next()
            .await
        {
            Some(Ok(s)) => s,
            Some(Err(e)) => {
                tracing::debug!("Stats failed for {name}: {e}");
                continue;
            }
            None => continue,
        };

        // CPU %
        let cpu_delta = stats
            .cpu_stats
            .cpu_usage
            .total_usage
            .saturating_sub(stats.precpu_stats.cpu_usage.total_usage);
        let system_delta = stats
            .cpu_stats
            .system_cpu_usage
            .unwrap_or(0)
            .saturating_sub(stats.precpu_stats.system_cpu_usage.unwrap_or(0));
        let online_cpus = stats.cpu_stats.online_cpus.unwrap_or(1).max(1);
        let cpu_percent = if system_delta > 0 {
            (cpu_delta as f64 / system_delta as f64) * online_cpus as f64 * 100.0
        } else {
            0.0
        };

        // Memory
        let mem_usage = stats.memory_stats.usage.unwrap_or(0);
        let mem_cache = stats
            .memory_stats
            .stats
            .as_ref()
            .map(|s| match s {
                bollard::container::MemoryStatsStats::V1(v1) => v1.cache,
                bollard::container::MemoryStatsStats::V2(v2) => v2.inactive_file,
            })
            .unwrap_or(0);
        let mem_actual = mem_usage.saturating_sub(mem_cache);
        let mem_limit = stats.memory_stats.limit.unwrap_or(1).max(1);
        let mem_usage_mb = mem_actual as f64 / (1024.0 * 1024.0);
        let mem_limit_mb = mem_limit as f64 / (1024.0 * 1024.0);
        let mem_percent = (mem_actual as f64 / mem_limit as f64) * 100.0;

        // Network I/O
        let (net_rx, net_tx) = stats
            .networks
            .as_ref()
            .map(|nets| {
                nets.values().fold((0u64, 0u64), |(rx, tx), iface| {
                    (rx + iface.rx_bytes, tx + iface.tx_bytes)
                })
            })
            .unwrap_or((0, 0));

        // Block I/O
        let (blk_read, blk_write) = stats
            .blkio_stats
            .io_service_bytes_recursive
            .as_ref()
            .map(|entries| {
                entries.iter().fold((0u64, 0u64), |(r, w), e| {
                    match e.op.to_lowercase().as_str() {
                        "read" => (r + e.value, w),
                        "write" => (r, w + e.value),
                        _ => (r, w),
                    }
                })
            })
            .unwrap_or((0, 0));

        // Rate calculations
        let prev_rx = *prev.net_rx.get(&name).unwrap_or(&net_rx);
        let prev_tx = *prev.net_tx.get(&name).unwrap_or(&net_tx);
        let prev_br = *prev.block_read.get(&name).unwrap_or(&blk_read);
        let prev_bw = *prev.block_write.get(&name).unwrap_or(&blk_write);

        let net_rx_rate = (net_rx.saturating_sub(prev_rx) as f64 / dt).max(0.0);
        let net_tx_rate = (net_tx.saturating_sub(prev_tx) as f64 / dt).max(0.0);
        let blk_read_rate = (blk_read.saturating_sub(prev_br) as f64 / dt).max(0.0);
        let blk_write_rate = (blk_write.saturating_sub(prev_bw) as f64 / dt).max(0.0);

        results.push(ContainerMetrics {
            name,
            cpu_percent: round2(cpu_percent),
            memory_percent: round2(mem_percent),
            memory_usage_mb: round1(mem_usage_mb),
            memory_limit_mb: round1(mem_limit_mb),
            net_rx_rate: round1(net_rx_rate),
            net_tx_rate: round1(net_tx_rate),
            block_read_rate: round1(blk_read_rate),
            block_write_rate: round1(blk_write_rate),
            status: "running".to_string(),
        });
        if let Some(m) = results.last() {
            let key = m.name.clone();
            prev.net_rx.insert(key.clone(), net_rx);
            prev.net_tx.insert(key.clone(), net_tx);
            prev.block_read.insert(key.clone(), blk_read);
            prev.block_write.insert(key, blk_write);
        }
    }

    results
}

fn build_stats_message(metrics: &[ContainerMetrics]) -> String {
    let mut containers = serde_json::Map::new();
    let mut total_cpu = 0.0f64;
    let mut total_mem_pct = 0.0f64;
    let mut total_net_rx_rate = 0.0f64;
    let mut total_net_tx_rate = 0.0f64;
    let mut total_blk_read_rate = 0.0f64;
    let mut total_blk_write_rate = 0.0f64;

    for m in metrics {
        containers.insert(
            m.name.clone(),
            json!({
                "cpu_percent": m.cpu_percent,
                "memory_percent": m.memory_percent,
                "memory_usage_mb": m.memory_usage_mb,
                "memory_limit_mb": m.memory_limit_mb,
                "net_rx_rate": m.net_rx_rate,
                "net_tx_rate": m.net_tx_rate,
                "block_read_rate": m.block_read_rate,
                "block_write_rate": m.block_write_rate,
                "status": m.status,
            }),
        );
        total_cpu += m.cpu_percent;
        total_mem_pct += m.memory_percent;
        total_net_rx_rate += m.net_rx_rate;
        total_net_tx_rate += m.net_tx_rate;
        total_blk_read_rate += m.block_read_rate;
        total_blk_write_rate += m.block_write_rate;
    }

    let count = metrics.len().max(1) as f64;

    json!({
        "type": "stats",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64(),
        "container_count": metrics.len(),
        "containers": containers,
        "aggregate": {
            "cpu_percent": round2(total_cpu),
            "avg_cpu_percent": round2(total_cpu / count),
            "avg_memory_percent": round2(total_mem_pct / count),
            "total_net_rx_rate": round1(total_net_rx_rate),
            "total_net_tx_rate": round1(total_net_tx_rate),
            "total_net_io_rate": round1(total_net_rx_rate + total_net_tx_rate),
            "total_block_read_rate": round1(total_blk_read_rate),
            "total_block_write_rate": round1(total_blk_write_rate),
        },
    })
    .to_string()
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
