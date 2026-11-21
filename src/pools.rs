//! The browser build's allocator: a small block comes off a free list kept
//! for its size, and goes back onto it.
//!
//! Every native binary allocates through mimalloc (`lib.rs`), which has no
//! wasm32 target, so the browser build was left on the allocator Rust gives
//! that target, `dlmalloc`. It is a careful general allocator and pays for
//! it on every call: a profile of 140 debugger turns on a published build
//! put 5.5% of all samples in its `malloc` and `free` alone. The engine
//! clones boards to think and builds a JSON document for every frame, so
//! nearly all of what it asks for is small and is given back soon.
//!
//! Here a block of up to [`LARGEST`] bytes is one pop from the list for its
//! size and freeing it is one push. Lists are fed from slabs taken from the
//! allocator underneath; a pooled block is never handed back to it, only
//! reused for the same size. Anything larger, or aligned past [`STEP`], goes
//! to the allocator underneath as before.
//!
//! One thread only. The browser build has one, and that is the only place
//! this is the global allocator (`lib.rs`); the type is not `Sync` anywhere
//! else.

use std::alloc::{GlobalAlloc, Layout};
use std::cell::UnsafeCell;
use std::ptr;

/// Sizes are rounded up to a multiple of this. It is also the alignment of
/// every pooled block.
const STEP: usize = 16;
/// The largest block a pool serves.
const LARGEST: usize = 2048;
const POOLS: usize = LARGEST / STEP;
/// What the allocator underneath is asked for when a list is empty and the
/// slab in hand cannot fit the block.
const SLAB: usize = 256 * 1024;

struct Lists {
    /// The first free block of each size; a free block's first word is the
    /// next one.
    free: [*mut u8; POOLS],
    /// The part of the newest slab nothing has been cut from yet.
    next: *mut u8,
    left: usize,
}

/// An allocator that pools small blocks over the allocator `under` it.
pub struct Pooled<A> {
    lists: UnsafeCell<Lists>,
    under: A,
}

// SAFETY: sound only where there is one thread, which is what this is gated
// to: wasm32 without the threads proposal. Everywhere else the raw pointers
// inside keep the type `!Sync`, and it cannot be a `static`.
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
unsafe impl<A: Sync> Sync for Pooled<A> {}

impl<A: GlobalAlloc> Pooled<A> {
    pub const fn new(under: A) -> Pooled<A> {
        Pooled {
            lists: UnsafeCell::new(Lists {
                free: [ptr::null_mut(); POOLS],
                next: ptr::null_mut(),
                left: 0,
            }),
            under,
        }
    }

    /// Which list serves this layout, if one does.
    #[inline]
    fn pool(layout: Layout) -> Option<usize> {
        (layout.size() <= LARGEST && layout.align() <= STEP)
            .then(|| (layout.size().max(1) - 1) / STEP)
    }

    /// A new block of `size` bytes cut from a slab. Whatever is left of a
    /// slab too short for it is let go: at most one block's worth in each.
    #[cold]
    unsafe fn cut(&self, lists: &mut Lists, size: usize) -> *mut u8 {
        if lists.left < size {
            let slab = self
                .under
                .alloc(Layout::from_size_align_unchecked(SLAB, STEP));
            if slab.is_null() {
                return slab;
            }
            lists.next = slab;
            lists.left = SLAB;
        }
        let block = lists.next;
        lists.next = block.add(size);
        lists.left -= size;
        block
    }
}

// SAFETY: a pooled block is at least the size asked for, aligned to `STEP`
// (slabs are, and every block is cut at a multiple of it), and on no list
// while it is out. `dealloc` and `realloc` are told the layout the block was
// made with, which names the list it belongs to. Everything else is passed
// to `under` unchanged.
unsafe impl<A: GlobalAlloc> GlobalAlloc for Pooled<A> {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(pool) = Self::pool(layout) else {
            return self.under.alloc(layout);
        };
        let lists = &mut *self.lists.get();
        let block = lists.free[pool];
        if block.is_null() {
            return self.cut(lists, (pool + 1) * STEP);
        }
        lists.free[pool] = block.cast::<*mut u8>().read();
        block
    }

    #[inline]
    unsafe fn dealloc(&self, block: *mut u8, layout: Layout) {
        let Some(pool) = Self::pool(layout) else {
            return self.under.dealloc(block, layout);
        };
        let lists = &mut *self.lists.get();
        block.cast::<*mut u8>().write(lists.free[pool]);
        lists.free[pool] = block;
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if Self::pool(layout).is_none() {
            return self.under.alloc_zeroed(layout);
        }
        let block = self.alloc(layout);
        if !block.is_null() {
            // A block off a list holds whatever its last owner left.
            ptr::write_bytes(block, 0, layout.size());
        }
        block
    }

    unsafe fn realloc(&self, block: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let wanted = Layout::from_size_align_unchecked(new_size, layout.align());
        match (Self::pool(layout), Self::pool(wanted)) {
            // Large stays large: the allocator underneath may grow it in place.
            (None, None) => self.under.realloc(block, layout, new_size),
            // The block already is the size its new length rounds up to.
            (Some(old), Some(new)) if old == new => block,
            _ => {
                let moved = self.alloc(wanted);
                if !moved.is_null() {
                    ptr::copy_nonoverlapping(block, moved, layout.size().min(new_size));
                    self.dealloc(block, layout);
                }
                moved
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::System;

    fn layout(size: usize) -> Layout {
        Layout::from_size_align(size, 8).unwrap()
    }

    /// A block's contents: one byte value all the way through.
    unsafe fn fill(block: *mut u8, size: usize, with: u8) {
        ptr::write_bytes(block, with, size);
    }

    unsafe fn holds(block: *mut u8, size: usize, with: u8) -> bool {
        std::slice::from_raw_parts(block, size)
            .iter()
            .all(|byte| *byte == with)
    }

    #[test]
    fn a_freed_block_is_the_next_one_of_its_size() {
        let pools = Pooled::new(System);
        unsafe {
            let first = pools.alloc(layout(40));
            let other = pools.alloc(layout(40));
            assert_ne!(first, other);
            pools.dealloc(first, layout(40));
            // 33 to 48 bytes are one size here.
            assert_eq!(pools.alloc(layout(33)), first);
            // And no other size is.
            let small = pools.alloc(layout(32));
            let large = pools.alloc(layout(49));
            assert!(small != first && large != first && small != other && large != other);
            pools.dealloc(other, layout(40));
            pools.dealloc(first, layout(33));
            // Last freed, first reused.
            assert_eq!(pools.alloc(layout(48)), first);
            assert_eq!(pools.alloc(layout(48)), other);
        }
    }

    #[test]
    fn blocks_out_at_once_never_share_a_byte() {
        let pools = Pooled::new(System);
        let sizes = [1, 7, 16, 17, 100, 640, 2047, 2048, 2049, 5000];
        unsafe {
            let mut out: Vec<(*mut u8, usize, u8)> = Vec::new();
            // Enough of them to run through several slabs.
            for round in 0..400usize {
                let size = sizes[round % sizes.len()];
                let with = (round % 251) as u8;
                let block = pools.alloc(layout(size));
                assert!(!block.is_null());
                assert_eq!(block as usize % 8, 0);
                if size <= LARGEST {
                    assert_eq!(block as usize % STEP, 0, "a pooled block of {size}");
                }
                fill(block, size, with);
                out.push((block, size, with));
                // Give every third one back as it goes, so lists are in use too.
                if round % 3 == 2 {
                    let (block, size, with) = out.swap_remove(round % out.len());
                    assert!(
                        holds(block, size, with),
                        "a block of {size} was written over"
                    );
                    pools.dealloc(block, layout(size));
                }
            }
            for (block, size, with) in out {
                assert!(
                    holds(block, size, with),
                    "a block of {size} was written over"
                );
                pools.dealloc(block, layout(size));
            }
        }
    }

    #[test]
    fn a_zeroed_block_is_zero_whatever_was_there() {
        let pools = Pooled::new(System);
        unsafe {
            for size in [24, 2048, 4096] {
                let block = pools.alloc(layout(size));
                fill(block, size, 0xAB);
                pools.dealloc(block, layout(size));
                let again = pools.alloc_zeroed(layout(size));
                assert!(holds(again, size, 0), "{size} bytes asked for zeroed");
                pools.dealloc(again, layout(size));
            }
        }
    }

    #[test]
    fn a_block_keeps_what_it_held_when_it_changes_size() {
        let pools = Pooled::new(System);
        unsafe {
            // Within one size the block stays where it is.
            let block = pools.alloc(layout(33));
            fill(block, 33, 7);
            let same = pools.realloc(block, layout(33), 48);
            assert_eq!(same, block);
            assert!(holds(same, 33, 7));
            // Up through the sizes, past the pools, and back down again.
            let mut at = same;
            let mut size = 48;
            fill(at, size, 9);
            for next in [49, 600, 2048, 2049, 70_000, 3000, 2048, 500, 16, 1] {
                let moved = pools.realloc(at, layout(size), next);
                assert!(!moved.is_null());
                assert!(holds(moved, size.min(next), 9), "{size} to {next}");
                fill(moved, next, 9);
                at = moved;
                size = next;
            }
            pools.dealloc(at, layout(size));
        }
    }

    #[test]
    fn what_a_pool_cannot_align_is_left_to_the_allocator_underneath() {
        let pools = Pooled::new(System);
        let wide = Layout::from_size_align(64, 64).unwrap();
        assert_eq!(Pooled::<System>::pool(wide), None);
        assert_eq!(Pooled::<System>::pool(layout(LARGEST + 1)), None);
        assert_eq!(Pooled::<System>::pool(layout(LARGEST)), Some(POOLS - 1));
        assert_eq!(Pooled::<System>::pool(layout(1)), Some(0));
        assert_eq!(Pooled::<System>::pool(layout(STEP)), Some(0));
        assert_eq!(Pooled::<System>::pool(layout(STEP + 1)), Some(1));
        unsafe {
            let block = pools.alloc(wide);
            assert_eq!(block as usize % 64, 0);
            fill(block, 64, 3);
            let grown = pools.realloc(block, wide, 128);
            assert_eq!(grown as usize % 64, 0);
            assert!(holds(grown, 64, 3));
            pools.dealloc(grown, Layout::from_size_align(128, 64).unwrap());
        }
    }
}
