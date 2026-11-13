//! Focused heap integrity checks, also callable in a real WebAssembly instance.
//!
//! `cargo build --locked --release --target wasm32-unknown-unknown
//! --example wasm_allocator_smoke` links the same global allocator as the engine.
//! A host can call `allocator_smoke` repeatedly; zero means every check passed.

use civvis as _;
use std::alloc::{alloc_zeroed, dealloc, realloc, Layout};
use std::ptr::NonNull;

struct Block {
    pointer: NonNull<u8>,
    layout: Layout,
    tag: u8,
}

impl Block {
    fn new(size: usize, align: usize, tag: u8) -> Result<Self, u32> {
        let layout = Layout::from_size_align(size, align).unwrap();
        // SAFETY: `layout` has positive size and valid power-of-two alignment.
        let pointer = NonNull::new(unsafe { alloc_zeroed(layout) }).ok_or(1u32)?;
        let mut block = Self {
            pointer,
            layout,
            tag,
        };
        if !(pointer.as_ptr() as usize).is_multiple_of(align) {
            return Err(2);
        }
        // SAFETY: this live allocation holds exactly `size` initialized bytes.
        let bytes = unsafe { std::slice::from_raw_parts_mut(pointer.as_ptr(), size) };
        if bytes.iter().any(|byte| *byte != 0) {
            return Err(3);
        }
        block.fill();
        Ok(block)
    }

    fn fill(&mut self) {
        for index in 0..self.layout.size() {
            // SAFETY: the block exclusively owns this live allocation. Write
            // through a raw pointer: a grown block's new bytes are uninitialized.
            unsafe {
                self.pointer
                    .as_ptr()
                    .add(index)
                    .write(self.tag.wrapping_add(index as u8));
            }
        }
    }

    fn check_prefix(&self, size: usize) -> bool {
        // SAFETY: every caller bounds `size` by the current allocation's size.
        let bytes = unsafe { std::slice::from_raw_parts(self.pointer.as_ptr(), size) };
        bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| *byte == self.tag.wrapping_add(index as u8))
    }

    fn resize(&mut self, size: usize) -> Result<(), u32> {
        let preserved = size.min(self.layout.size());
        // SAFETY: the pointer was allocated with this layout and the new size
        // is positive and fits Layout's range. Failure leaves the old block live.
        let pointer = NonNull::new(unsafe { realloc(self.pointer.as_ptr(), self.layout, size) })
            .ok_or(4u32)?;
        self.pointer = pointer;
        self.layout = Layout::from_size_align(size, self.layout.align()).unwrap();
        if !(pointer.as_ptr() as usize).is_multiple_of(self.layout.align()) {
            return Err(5);
        }
        if !self.check_prefix(preserved) {
            return Err(6);
        }
        self.fill();
        Ok(())
    }
}

impl Drop for Block {
    fn drop(&mut self) {
        // SAFETY: this block exclusively owns the pointer and its current layout.
        unsafe { dealloc(self.pointer.as_ptr(), self.layout) };
    }
}

fn exercise() -> Result<(), u32> {
    let sizes = [
        1, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65, 255, 256, 257, 4095, 4096, 4097, 16384,
        65535, 65536, 65537, 131089,
    ];
    let mut blocks = Vec::new();
    for align in [1, 8, 16, 64, 4096] {
        for size in sizes {
            blocks.push(Some(Block::new(size, align, blocks.len() as u8)?));
        }
    }
    // Free alternating sizes while other patterns remain live, then grow and
    // shrink their neighbors. Overlapping allocations corrupt the held patterns.
    for (index, slot) in blocks.iter_mut().enumerate() {
        if index.is_multiple_of(2) {
            *slot = None;
        }
    }
    for block in blocks.iter_mut().flatten() {
        if !block.check_prefix(block.layout.size()) {
            return Err(7);
        }
        block.resize(block.layout.size() * 2 + 19)?;
        block.resize(block.layout.size() / 3 + 1)?;
    }
    let mut growing = Vec::new();
    for index in 0..131_089u32 {
        growing.push(index ^ 0x5a5a_1234);
    }
    growing.shrink_to_fit();
    if growing
        .iter()
        .enumerate()
        .any(|(index, value)| *value != index as u32 ^ 0x5a5a_1234)
    {
        return Err(8);
    }
    for block in blocks.iter().flatten() {
        if !block.check_prefix(block.layout.size()) {
            return Err(9);
        }
    }
    Ok(())
}

#[no_mangle]
pub extern "C" fn allocator_smoke() -> u32 {
    exercise().err().unwrap_or(0)
}

fn main() {
    assert_eq!(allocator_smoke(), 0);
}

#[test]
fn allocation_patterns_survive_growth_shrink_and_fragmentation() {
    assert_eq!(allocator_smoke(), 0);
}
