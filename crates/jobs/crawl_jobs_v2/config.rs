#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CrawlJobsV2Mode {
    Enabled,
}

pub(crate) fn mode() -> CrawlJobsV2Mode {
    CrawlJobsV2Mode::Enabled
}
