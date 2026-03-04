use axon::crates::mcp::schema::SearchTimeRange;
use axon::crates::mcp::server::common::{
    to_map_options, to_pagination, to_retrieve_options, to_search_options, to_service_time_range,
};
use axon::crates::services::types::{MapOptions, Pagination, RetrieveOptions, ServiceTimeRange};

// --- to_pagination ---

#[test]
fn pagination_default_limit_when_none() {
    let p = to_pagination(None, None);
    assert_eq!(
        p,
        Pagination {
            limit: 10,
            offset: 0
        }
    );
}

#[test]
fn pagination_custom_limit_and_offset() {
    let p = to_pagination(Some(25), Some(50));
    assert_eq!(
        p,
        Pagination {
            limit: 25,
            offset: 50
        }
    );
}

#[test]
fn pagination_offset_passthrough_zero() {
    let p = to_pagination(Some(5), Some(0));
    assert_eq!(p.offset, 0);
    assert_eq!(p.limit, 5);
}

#[test]
fn pagination_limit_clamped_at_500() {
    let p = to_pagination(Some(9999), None);
    assert_eq!(p.limit, 500);
}

#[test]
fn pagination_limit_clamped_at_1_minimum() {
    let p = to_pagination(Some(0), None);
    assert_eq!(p.limit, 1);
}

// --- to_retrieve_options ---

#[test]
fn retrieve_options_none_passthrough() {
    let r = to_retrieve_options(None);
    assert_eq!(r, RetrieveOptions { max_points: None });
}

#[test]
fn retrieve_options_some_passthrough() {
    let r = to_retrieve_options(Some(42));
    assert_eq!(
        r,
        RetrieveOptions {
            max_points: Some(42)
        }
    );
}

// --- to_service_time_range ---

#[test]
fn time_range_day() {
    assert_eq!(
        to_service_time_range(SearchTimeRange::Day),
        ServiceTimeRange::Day
    );
}

#[test]
fn time_range_week() {
    assert_eq!(
        to_service_time_range(SearchTimeRange::Week),
        ServiceTimeRange::Week
    );
}

#[test]
fn time_range_month() {
    assert_eq!(
        to_service_time_range(SearchTimeRange::Month),
        ServiceTimeRange::Month
    );
}

#[test]
fn time_range_year() {
    assert_eq!(
        to_service_time_range(SearchTimeRange::Year),
        ServiceTimeRange::Year
    );
}

// --- to_search_options ---

#[test]
fn search_options_no_time_range() {
    let opts = to_search_options(Some(20), Some(5), None);
    assert_eq!(opts.limit, 20);
    assert_eq!(opts.offset, 5);
    assert!(opts.time_range.is_none());
}

#[test]
fn search_options_with_time_range_maps_correctly() {
    let opts = to_search_options(None, None, Some(SearchTimeRange::Month));
    assert!(opts.time_range.is_some());
    assert_eq!(opts.time_range.unwrap(), ServiceTimeRange::Month);
}

// --- to_map_options ---

#[test]
fn map_options_default_limit_when_none() {
    let m = to_map_options(None, None);
    assert_eq!(
        m,
        MapOptions {
            limit: 10,
            offset: 0
        }
    );
}

#[test]
fn map_options_limit_clamped_at_500() {
    let m = to_map_options(Some(10_000), Some(3));
    assert_eq!(
        m,
        MapOptions {
            limit: 500,
            offset: 3
        }
    );
}
