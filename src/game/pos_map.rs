//! The game's index of what stands where, by map position. See
//! [`crate::fast_map`].

/// What stands at each position, found by hashing the position.
pub(crate) type PosMap<V> = crate::fast_map::FastMap<crate::Pos, V>;
