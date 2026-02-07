//! Fast bump allocator for short-lived objects
//!
//! Uses thread-local bump allocation for fast object creation.
//! Objects allocated here are not individually freed - the entire arena
//! can be reset at once (e.g., at end of program or during GC).

use std::cell::UnsafeCell;
use std::alloc::{alloc, Layout};

/// Size of each arena block (8MB)
const BLOCK_SIZE: usize = 8 * 1024 * 1024;

/// Create a block of at least the given size (for oversized allocations)
fn alloc_block(min_size: usize) -> ArenaBlock {
    let size = if min_size <= BLOCK_SIZE { BLOCK_SIZE } else {
        // Round up to next multiple of BLOCK_SIZE
        ((min_size + BLOCK_SIZE - 1) / BLOCK_SIZE) * BLOCK_SIZE
    };
    let layout = Layout::from_size_align(size, 16).unwrap();
    let data = unsafe { alloc(layout) };
    if data.is_null() {
        panic!("Failed to allocate arena block of {} bytes", size);
    }
    ArenaBlock {
        data,
        size,
        offset: 0,
    }
}

/// A single arena block
struct ArenaBlock {
    data: *mut u8,
    size: usize,
    offset: usize,
}

impl ArenaBlock {
    fn new() -> Self {
        alloc_block(BLOCK_SIZE)
    }

    /// Try to allocate within this block, respecting alignment
    #[inline]
    fn alloc(&mut self, size: usize, align: usize) -> Option<*mut u8> {
        // Align offset up
        let aligned_offset = (self.offset + align - 1) & !(align - 1);
        if aligned_offset + size > self.size {
            return None;
        }

        let ptr = unsafe { self.data.add(aligned_offset) };
        self.offset = aligned_offset + size;
        Some(ptr)
    }
}

/// Thread-local arena allocator
struct Arena {
    blocks: Vec<ArenaBlock>,
    current: usize,
}

impl Arena {
    fn new() -> Self {
        Arena {
            blocks: vec![ArenaBlock::new()],
            current: 0,
        }
    }

    #[inline]
    fn alloc(&mut self, size: usize, align: usize) -> *mut u8 {
        // Try current block first
        if let Some(ptr) = self.blocks[self.current].alloc(size, align) {
            return ptr;
        }

        // Need a new block — sized to fit the allocation
        self.blocks.push(alloc_block(size));
        self.current += 1;

        self.blocks[self.current].alloc(size, align)
            .expect("Fresh block should have space")
    }
}

thread_local! {
    static ARENA: UnsafeCell<Arena> = UnsafeCell::new(Arena::new());
}

/// Allocate memory from the thread-local arena
/// This is very fast - just a pointer bump in the common case
#[inline]
pub fn arena_alloc(size: usize, align: usize) -> *mut u8 {
    ARENA.with(|arena| {
        let arena = unsafe { &mut *arena.get() };
        arena.alloc(size, align)
    })
}

/// Allocate an object of known size from the arena
/// Returns a properly aligned pointer
#[no_mangle]
pub extern "C" fn js_arena_alloc(size: u32) -> *mut u8 {
    arena_alloc(size as usize, 8)
}

/// Get arena memory statistics: (heap_used, heap_total)
/// heap_used = total bytes allocated across all blocks
/// heap_total = total bytes reserved across all blocks
#[no_mangle]
pub extern "C" fn js_arena_stats(out_used: *mut u64, out_total: *mut u64) {
    ARENA.with(|arena| {
        let arena = unsafe { &*arena.get() };
        let mut used: u64 = 0;
        let mut total: u64 = 0;
        for block in &arena.blocks {
            used += block.offset as u64;
            total += block.size as u64;
        }
        unsafe {
            *out_used = used;
            *out_total = total;
        }
    });
}
