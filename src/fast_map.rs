//! A fast, deterministic hash map for the engine's point lookups: the
//! indexes of what stands where and the memos a read-only query scope keeps.

use std::hash::{BuildHasherDefault, Hasher};

/// Hashes small keys (map positions, ids, interned names and tuples of them)
/// a machine word at a time: a rotate, an xor and a multiply per word, and
/// MurmurHash3's 64-bit finalizer to spread the result over the whole word.
///
/// It has no random state, so it hashes the same everywhere it runs, wasm
/// included. A map it serves must never be iterated for anything a game can
/// observe: the order its buckets fall in is not the order of its keys.
#[derive(Default, Clone, Copy)]
pub(crate) struct FastHasher(u64);

impl FastHasher {
    #[inline]
    fn mix(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(29) ^ word).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}

impl Hasher for FastHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let (words, tail) = bytes.as_chunks::<8>();
        for word in words {
            self.mix(u64::from_le_bytes(*word));
        }
        let mut last = [0u8; 8];
        last[..tail.len()].copy_from_slice(tail);
        self.mix(u64::from_le_bytes(last) ^ ((tail.len() as u64) << 56));
    }

    #[inline]
    fn write_u8(&mut self, value: u8) {
        self.mix(u64::from(value));
    }

    #[inline]
    fn write_u16(&mut self, value: u16) {
        self.mix(u64::from(value));
    }

    #[inline]
    fn write_u32(&mut self, value: u32) {
        self.mix(u64::from(value));
    }

    #[inline]
    fn write_u64(&mut self, value: u64) {
        self.mix(value);
    }

    #[inline]
    fn write_usize(&mut self, value: usize) {
        self.mix(value as u64);
    }

    #[inline]
    fn write_i32(&mut self, value: i32) {
        self.mix(u64::from(value as u32));
    }

    #[inline]
    fn write_i64(&mut self, value: i64) {
        self.mix(value as u64);
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

/// A hash map keyed through [`FastHasher`]: for lookups by key, never for
/// walking in key order.
pub(crate) type FastMap<K, V> = std::collections::HashMap<K, V, BuildHasherDefault<FastHasher>>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::Hash;

    fn hash(value: impl Hash) -> u64 {
        let mut hasher = FastHasher::default();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn nearby_keys_hash_apart_and_the_same_key_hashes_alike() {
        let mut seen = std::collections::HashSet::new();
        for q in -40..40 {
            for r in -40..40 {
                assert!(seen.insert(hash((q, r))), "({q}, {r}) collided");
            }
        }
        assert_eq!(hash((3, -7)), hash((3, -7)));
        assert_ne!(hash((3, -7)), hash((-7, 3)));
        assert_ne!(hash("ab"), hash("ab\0"));
        let mut map: FastMap<(i32, i32), u32> = FastMap::default();
        map.insert((1, 2), 3);
        assert_eq!(map.get(&(1, 2)), Some(&3));
        assert_eq!(map.get(&(2, 1)), None);
    }
}
