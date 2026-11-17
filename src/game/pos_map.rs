//! A hash index by map position, for the game's point lookups of what stands
//! where.

use std::hash::{BuildHasherDefault, Hasher};

/// Hashes a map position: its two coordinates, a multiply each, and a
/// finalizer that spreads them over the whole word.
///
/// `Pos` is `(i32, i32)`, which hashes as two `write_i32` calls; anything else
/// folds its bytes. The indexes it serves are only ever asked about one
/// position at a time and never iterated, so the order their buckets fall in
/// reaches nothing.
#[derive(Default, Clone, Copy)]
pub(crate) struct PosHasher(u64);

impl Hasher for PosHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    #[inline]
    fn write_i32(&mut self, value: i32) {
        self.0 =
            (self.0.rotate_left(29) ^ u64::from(value as u32)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }

    /// MurmurHash3's 64-bit finalizer.
    #[inline]
    fn finish(&self) -> u64 {
        let mut hash = self.0;
        hash ^= hash >> 33;
        hash = hash.wrapping_mul(0xff51_afd7_ed55_8ccd);
        hash ^= hash >> 33;
        hash = hash.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        hash ^ (hash >> 33)
    }
}

/// What stands at each position, found by hashing the position.
pub(crate) type PosMap<V> = std::collections::HashMap<crate::Pos, V, BuildHasherDefault<PosHasher>>;
