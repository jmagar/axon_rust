//! Thread-local work counters for deterministic performance regressions. This
//! allocator only exists in unit-test binaries and delegates storage to System.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

#[derive(Clone, Copy, Default)]
pub(crate) struct Work {
    pub(crate) allocated_bytes: usize,
    pub(crate) range_scan_bytes: usize,
}

thread_local! {
    static CURRENT: Cell<Option<Work>> = const { Cell::new(None) };
}

fn record(update: impl FnOnce(&mut Work)) {
    let _ = CURRENT.try_with(|current| {
        if let Some(mut work) = current.get() {
            update(&mut work);
            current.set(Some(work));
        }
    });
}

pub(crate) fn range_scan(bytes: usize) {
    record(|work| work.range_scan_bytes = work.range_scan_bytes.saturating_add(bytes));
}

pub(crate) fn measure<T>(operation: impl FnOnce() -> T) -> (T, Work) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            CURRENT.set(None);
        }
    }
    assert!(
        CURRENT.replace(Some(Work::default())).is_none(),
        "nested measurement"
    );
    let _reset = Reset;
    let result = operation();
    (result, CURRENT.get().expect("measurement active"))
}

struct CountingAllocator;
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

// SAFETY: All allocation operations forward the original pointer and layout to
// System. The non-allocating thread-local counter never changes storage ownership.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(|work| work.allocated_bytes = work.allocated_bytes.saturating_add(layout.size()));
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(|work| work.allocated_bytes = work.allocated_bytes.saturating_add(layout.size()));
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(|work| work.allocated_bytes = work.allocated_bytes.saturating_add(size));
        unsafe { System.realloc(pointer, layout, size) }
    }
}
