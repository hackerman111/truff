use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

pub struct CountingAllocator;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

fn record(size: usize) {
    ALLOCATIONS.fetch_add(1, Relaxed);
    let live = LIVE.fetch_add(size, Relaxed) + size;
    PEAK.fetch_max(live, Relaxed);
}

// SAFETY: Every allocation operation is forwarded unchanged to System.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: GlobalAlloc's caller supplies a valid allocation layout.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            record(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: The caller supplies a live System allocation and its original layout.
        unsafe { System.dealloc(ptr, layout) };
        LIVE.fetch_sub(layout.size(), Relaxed);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: The caller supplies a live allocation, its layout, and a valid new size.
        let result = unsafe { System.realloc(ptr, layout, size) };
        if !result.is_null() {
            LIVE.fetch_sub(layout.size(), Relaxed);
            record(size);
        }
        result
    }
}

pub fn measure<T>(name: &str, operation: impl FnOnce() -> T) -> T {
    let before = LIVE.load(Relaxed);
    let allocations = ALLOCATIONS.load(Relaxed);
    PEAK.store(before, Relaxed);
    let start = std::time::Instant::now();
    let result = std::hint::black_box(operation());
    let ns = start.elapsed().as_nanos();
    let live = LIVE.load(Relaxed) as i128 - before as i128;
    let peak = PEAK.load(Relaxed).saturating_sub(before);
    let count = ALLOCATIONS.load(Relaxed) - allocations;
    println!("{name},{live},{peak},{count},{ns}");
    result
}
