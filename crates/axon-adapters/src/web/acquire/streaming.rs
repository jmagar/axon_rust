//! Independently polled, bounded streaming acquisition.

use super::*;

pub(in crate::web) struct StreamedItemOutcome {
    pub(in crate::web) ordinal: usize,
    pub(in crate::web) is_final: bool,
    pub(in crate::web) item: Option<AcquiredSourceItem>,
    pub(in crate::web) warnings: Vec<SourceWarning>,
}

#[async_trait::async_trait]
pub(in crate::web) trait StreamingItemSink: Send + Sync {
    async fn send(&self, outcome: StreamedItemOutcome) -> Result<()>;
}

pub(super) fn independent_acquisition<F>(
    operation: F,
) -> tokio_util::task::AbortOnDropHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    // Buffered acquisition may stop being polled while downstream writes to
    // SQLite. In-flight provider operations must keep running to release their
    // writer gates. Dropping the stream still aborts every owned operation.
    tokio_util::task::AbortOnDropHandle::new(tokio::spawn(operation))
}

#[cfg(test)]
#[path = "streaming_tests.rs"]
mod polling_tests;

pub(in crate::web) async fn acquire_changed_items_streaming(
    plan: &SourcePlan,
    manifest_items: &[ManifestItem],
    fetch: std::sync::Arc<dyn FetchProvider>,
    render: std::sync::Arc<dyn RenderProvider>,
    progress: Option<&dyn AcquisitionProgressSink>,
    sink: &dyn StreamingItemSink,
) -> Result<()> {
    let opts = std::sync::Arc::new(acquire_options(plan));
    if opts.cache_policy == CachePolicy::Offline {
        return Err(ApiError::new(
            "web.cache.offline_miss",
            ErrorStage::Fetching,
            "offline cache policy cannot acquire changed web items",
        ));
    }
    let batch_started = Instant::now();
    let concurrency = acquire_concurrency().min(manifest_items.len().max(1));
    let mut pending = stream::iter(manifest_items.iter().cloned().enumerate())
        .map(|(ordinal, item)| {
            let fetch = fetch.clone();
            let render = render.clone();
            let opts = opts.clone();
            independent_acquisition(async move {
                let started = Instant::now();
                let outcome = acquire_item(fetch.as_ref(), render.as_ref(), &item, &opts).await;
                (ordinal, item, outcome, started.elapsed())
            })
        })
        // Preserve source identity in ordinal, but deliver ready work immediately
        // so one slow page cannot strand fetch capacity or downstream preparation.
        .buffer_unordered(concurrency);
    let mut timings = Vec::with_capacity(manifest_items.len());
    let mut documents = 0usize;
    while let Some(result) = pending.next().await {
        let (ordinal, manifest_item, outcome, elapsed) = result.map_err(|_| {
            ApiError::new(
                "web.acquire.task_failed",
                ErrorStage::Fetching,
                "acquisition task failed",
            )
        })?;
        timings.push(ItemTiming {
            elapsed,
            completed_at: batch_started.elapsed(),
        });
        let mut warnings = Vec::new();
        let item = resolve_item_outcome(
            outcome,
            manifest_item.source_item_key,
            &manifest_item.canonical_uri,
            &mut warnings,
        );
        documents += usize::from(item.is_some());
        report_progress(progress, manifest_items.len(), timings.len(), documents).await;
        sink.send(StreamedItemOutcome {
            ordinal,
            is_final: timings.len() == manifest_items.len(),
            item,
            warnings,
        })
        .await?;
    }
    log_acquisition_timings(
        "streaming",
        manifest_items.len(),
        concurrency,
        batch_started.elapsed(),
        &timings,
    );
    Ok(())
}

pub(super) fn acquire_options(plan: &SourcePlan) -> AcquireOptions {
    let values = &plan.route.validated_options.values;
    AcquireOptions {
        job_id: plan.job_id,
        mode: effective_render_mode(values),
        min_markdown_chars: min_markdown_chars(values),
        automation_script: automation_script_ref(values),
        headers: headers(values),
        cache_policy: cache_policy(values),
        render_metadata: render_metadata(values),
        vertical: VerticalOptions {
            enabled: verticals_enabled(values),
            auto_dispatch_skip: auto_dispatch_skip(values),
            user_agent: user_agent(values),
            cache_ttl_secs: values
                .get("vertical_cache_ttl_secs")
                .and_then(Value::as_object)
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|(name, value)| value.as_u64().map(|ttl| (name.clone(), ttl)))
                        .collect()
                })
                .unwrap_or_default(),
        },
    }
}
