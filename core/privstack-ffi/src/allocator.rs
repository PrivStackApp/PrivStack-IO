//! Per-subsystem memory tracking allocator.
//!
//! Wraps the system allocator with a 16-byte header per allocation that stores
//! the original size (8 bytes) and subsystem ID (1 byte + 7 padding). Every
//! alloc/dealloc updates per-subsystem atomic counters.
//!
//! A thread-local `Cell<u8>` holds the current subsystem tag, set via
//! `with_subsystem(sub, || { ... })` scope guard.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

/// Subsystem IDs for Rust-side allocation tagging.
pub const SUB_UNTAGGED: u8 = 0;
pub const SUB_STORAGE: u8 = 1;
pub const SUB_SYNC: u8 = 2;
pub const SUB_CRYPTO: u8 = 3;
pub const SUB_CLOUD: u8 = 4;
pub const SUB_PLUGINS: u8 = 5;
pub const SUB_FFI: u8 = 6;
pub const SUB_RESERVED: u8 = 7;

const MAX_SUBSYSTEMS: usize = 8;
const HEADER_SIZE: usize = 16;

/// Per-subsystem counters: current bytes and total allocation count.
struct SubsystemCounters {
    bytes: AtomicI64,
    allocs: AtomicU64,
}

static COUNTERS: [SubsystemCounters; MAX_SUBSYSTEMS] = {
    // SAFETY: AtomicI64/AtomicU64 can be zero-initialized.
    const INIT: SubsystemCounters = SubsystemCounters {
        bytes: AtomicI64::new(0),
        allocs: AtomicU64::new(0),
    };
    [INIT; MAX_SUBSYSTEMS]
};

thread_local! {
    static CURRENT_SUBSYSTEM: Cell<u8> = const { Cell::new(SUB_UNTAGGED) };
}

/// Tracking allocator that prepends a 16-byte header to every allocation.
///
/// Header layout (16 bytes):
/// - bytes  0..8:  original requested size (u64, little-endian)
/// - byte   8:     subsystem ID
/// - bytes  9..16: padding
pub struct TrackingAllocator;

// SAFETY: We delegate to System allocator with an enlarged layout that
// includes a 16-byte header. The header stores the original size and
// subsystem ID so dealloc can update the correct counters.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let sub = CURRENT_SUBSYSTEM.with(|c| c.get());
        let idx = (sub as usize).min(MAX_SUBSYSTEMS - 1);

        let (full_layout, offset) = match padded_layout(layout) {
            Some(v) => v,
            None => return std::ptr::null_mut(),
        };

        // SAFETY: full_layout is valid (computed from a valid Layout + HEADER_SIZE).
        let ptr = unsafe { System.alloc(full_layout) };
        if ptr.is_null() {
            return ptr;
        }

        // Write header: size + subsystem ID.
        let size = layout.size() as u64;
        // SAFETY: ptr is valid for full_layout.size() bytes, and we write within the header.
        unsafe {
            std::ptr::copy_nonoverlapping(size.to_le_bytes().as_ptr(), ptr, 8);
            *ptr.add(8) = sub;
        }

        COUNTERS[idx].bytes.fetch_add(size as i64, Ordering::Relaxed);
        COUNTERS[idx].allocs.fetch_add(1, Ordering::Relaxed);

        // SAFETY: offset is HEADER_SIZE, which is within the allocated region.
        unsafe { ptr.add(offset) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let (full_layout, offset) = match padded_layout(layout) {
            Some(v) => v,
            None => return,
        };

        // SAFETY: ptr was returned by alloc with this layout, so ptr - offset is
        // the original allocation start.
        let header_ptr = unsafe { ptr.sub(offset) };

        // Read header.
        let mut size_bytes = [0u8; 8];
        // SAFETY: header_ptr is valid for HEADER_SIZE bytes.
        unsafe { std::ptr::copy_nonoverlapping(header_ptr, size_bytes.as_mut_ptr(), 8) };
        let size = u64::from_le_bytes(size_bytes);
        let sub = unsafe { *header_ptr.add(8) };
        let idx = (sub as usize).min(MAX_SUBSYSTEMS - 1);

        COUNTERS[idx].bytes.fetch_sub(size as i64, Ordering::Relaxed);

        // SAFETY: header_ptr is the original pointer from System.alloc with full_layout.
        unsafe { System.dealloc(header_ptr, full_layout) };
    }
}

/// Compute the full layout (header + payload) and the offset to the payload.
fn padded_layout(layout: Layout) -> Option<(Layout, usize)> {
    let header_layout = Layout::from_size_align(HEADER_SIZE, layout.align().max(8)).ok()?;
    let (full, offset) = header_layout.extend(layout).ok()?;
    Some((full.pad_to_align(), offset))
}

/// Execute `f` with the current thread's subsystem tag set to `sub`.
/// Restores the previous tag on return (including panic unwind).
pub fn with_subsystem<R>(sub: u8, f: impl FnOnce() -> R) -> R {
    struct Guard(u8);
    impl Drop for Guard {
        fn drop(&mut self) {
            CURRENT_SUBSYSTEM.with(|c| c.set(self.0));
        }
    }

    let prev = CURRENT_SUBSYSTEM.with(|c| c.replace(sub));
    let _guard = Guard(prev);
    f()
}

/// Snapshot of a single subsystem's counters.
#[derive(serde::Serialize)]
pub struct SubsystemMemorySnapshot {
    pub id: u8,
    pub bytes: i64,
    pub allocs: u64,
}

/// Return a snapshot of all subsystem counters.
pub fn snapshot_all() -> Vec<SubsystemMemorySnapshot> {
    (0..MAX_SUBSYSTEMS as u8)
        .map(|id| {
            let idx = id as usize;
            SubsystemMemorySnapshot {
                id,
                bytes: COUNTERS[idx].bytes.load(Ordering::Relaxed),
                allocs: COUNTERS[idx].allocs.load(Ordering::Relaxed),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_subsystem_restores_tag() {
        CURRENT_SUBSYSTEM.with(|c| c.set(SUB_UNTAGGED));
        with_subsystem(SUB_STORAGE, || {
            assert_eq!(CURRENT_SUBSYSTEM.with(|c| c.get()), SUB_STORAGE);
        });
        assert_eq!(CURRENT_SUBSYSTEM.with(|c| c.get()), SUB_UNTAGGED);
    }

    #[test]
    fn snapshot_returns_all_subsystems() {
        let snap = snapshot_all();
        assert_eq!(snap.len(), MAX_SUBSYSTEMS);
    }
}
