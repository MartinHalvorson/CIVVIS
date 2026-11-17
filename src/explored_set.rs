//! A civilization's explored ground: ordered for iteration, hashed for lookup.
//!
//! `Player::explored` is asked "has this tile been seen?" from every corner of
//! the AI — exploration goals, naval recon, scout inference, the visibility
//! refresh — and as a `BTreeSet<Pos>` each answer was a tree descent. A
//! native profile of the site game put `BTreeSet<Pos>::contains` at 4.6% of
//! all self time, the largest single leaf in the engine.
//!
//! The ordered set stays: iteration, set algebra and saves read it exactly as
//! before, so nothing that depends on order can change. Beside it sits a
//! hashed copy that answers [`ExploredSet::contains`]. There is deliberately
//! no `DerefMut`: every write goes through a method here that updates both, so
//! the copy can never drift from the set it mirrors.

use std::collections::{BTreeSet, HashSet};
use std::hash::BuildHasherDefault;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::fast_map::FastHasher;
use crate::Pos;

#[derive(Clone, Default)]
pub struct ExploredSet {
    ordered: BTreeSet<Pos>,
    index: HashSet<Pos, BuildHasherDefault<FastHasher>>,
}

impl ExploredSet {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn contains(&self, pos: &Pos) -> bool {
        self.index.contains(pos)
    }

    pub fn insert(&mut self, pos: Pos) -> bool {
        if self.ordered.insert(pos) {
            self.index.insert(pos);
            true
        } else {
            false
        }
    }

    pub fn remove(&mut self, pos: &Pos) -> bool {
        if self.ordered.remove(pos) {
            self.index.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        self.ordered.clear();
        self.index.clear();
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&Pos) -> bool) {
        let index = &mut self.index;
        self.ordered.retain(|pos| {
            let kept = keep(pos);
            if !kept {
                index.remove(pos);
            }
            kept
        });
    }

    /// The ordered set, for callers that need a `BTreeSet` by value.
    pub fn to_btree(&self) -> BTreeSet<Pos> {
        self.ordered.clone()
    }
}

impl std::ops::Deref for ExploredSet {
    type Target = BTreeSet<Pos>;

    fn deref(&self) -> &BTreeSet<Pos> {
        &self.ordered
    }
}

impl From<BTreeSet<Pos>> for ExploredSet {
    fn from(ordered: BTreeSet<Pos>) -> Self {
        let index = ordered.iter().copied().collect();
        Self { ordered, index }
    }
}

impl FromIterator<Pos> for ExploredSet {
    fn from_iter<I: IntoIterator<Item = Pos>>(iter: I) -> Self {
        Self::from(iter.into_iter().collect::<BTreeSet<Pos>>())
    }
}

impl Extend<Pos> for ExploredSet {
    fn extend<I: IntoIterator<Item = Pos>>(&mut self, iter: I) {
        for pos in iter {
            self.insert(pos);
        }
    }
}

impl<'a> Extend<&'a Pos> for ExploredSet {
    fn extend<I: IntoIterator<Item = &'a Pos>>(&mut self, iter: I) {
        for pos in iter {
            self.insert(*pos);
        }
    }
}

impl<'a> IntoIterator for &'a ExploredSet {
    type Item = &'a Pos;
    type IntoIter = std::collections::btree_set::Iter<'a, Pos>;

    fn into_iter(self) -> Self::IntoIter {
        self.ordered.iter()
    }
}

impl PartialEq for ExploredSet {
    fn eq(&self, other: &Self) -> bool {
        self.ordered == other.ordered
    }
}

impl Eq for ExploredSet {}

impl std::fmt::Debug for ExploredSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.ordered.fmt(f)
    }
}

impl Serialize for ExploredSet {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.ordered.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ExploredSet {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        BTreeSet::<Pos>::deserialize(deserializer).map(Self::from)
    }
}

#[cfg(test)]
mod tests {
    use super::ExploredSet;
    use std::collections::BTreeSet;

    #[test]
    fn every_write_keeps_the_index_in_step_with_the_set() {
        let mut set = ExploredSet::new();
        assert!(set.insert((3, 4)));
        assert!(!set.insert((3, 4)));
        set.extend([(1, 1), (2, 2), (5, 5)]);
        set.extend(&[(7, 7)]);
        assert!(set.remove(&(2, 2)));
        assert!(!set.remove(&(2, 2)));
        set.retain(|pos| pos.0 != 5);
        for probe in [(1, 1), (2, 2), (3, 4), (5, 5), (7, 7), (9, 9)] {
            assert_eq!(set.contains(&probe), set.iter().any(|pos| *pos == probe));
        }
        assert_eq!(
            set.iter().copied().collect::<Vec<_>>(),
            vec![(1, 1), (3, 4), (7, 7)]
        );
        set.clear();
        assert!(!set.contains(&(1, 1)));
    }

    #[test]
    fn serializes_exactly_as_the_ordered_set() {
        let ordered: BTreeSet<_> = [(4, -1), (0, 2), (-3, 9)].into_iter().collect();
        let set = ExploredSet::from(ordered.clone());
        let json = serde_json::to_string(&set).unwrap();
        assert_eq!(json, serde_json::to_string(&ordered).unwrap());
        let back: ExploredSet = serde_json::from_str(&json).unwrap();
        assert_eq!(back, set);
        assert!(back.contains(&(0, 2)));
    }
}
