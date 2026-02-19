#[test]
fn crawl_jobs_v2_module_has_no_legacy_function_calls() {
    let src = include_str!("../crates/jobs/crawl_jobs_v2/mod.rs");

    let forbidden = [
        "crawl_jobs_legacy::doctor",
        "crawl_jobs_legacy::start_crawl_job",
        "crawl_jobs_legacy::get_job",
        "crawl_jobs_legacy::list_jobs",
        "crawl_jobs_legacy::cancel_job",
        "crawl_jobs_legacy::cleanup_jobs",
        "crawl_jobs_legacy::clear_jobs",
        "crawl_jobs_legacy::recover_stale_crawl_jobs",
        "crawl_jobs_legacy::run_worker",
    ];

    for needle in forbidden {
        assert!(
            !src.contains(needle),
            "v2 module still references legacy function: {needle}"
        );
    }
}
