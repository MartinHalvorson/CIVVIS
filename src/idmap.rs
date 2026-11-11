//! ID-keyed rosters with contiguous, numerically ordered iteration.
//!
//! The sorted entries determine iteration and save order. A separate hash
//! index answers point lookups without a tree walk; it never determines order.
//! IDs may be sparse or as large as `u32::MAX`, so the index scales with the
//! number of entries rather than the largest ID imported from a host game.

use std::collections::BTreeMap;
use std::fmt;
use std::iter::FusedIterator;
use std::slice;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

const EMPTY: u32 = u32::MAX;

#[derive(Clone)]
pub struct IdMap<T> {
    entries: Vec<(u32, T)>,
    index: Vec<u32>,
}

impl<T> Default for IdMap<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            index: Vec::new(),
        }
    }
}

impl<T> IdMap<T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[inline]
    fn hash(id: u32) -> usize {
        let mixed = (id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        (mixed ^ (mixed >> 32)) as usize
    }

    fn reindex(&mut self) {
        self.index.clear();
        if self.entries.is_empty() {
            return;
        }
        assert!(
            self.entries.len() < EMPTY as usize,
            "too many roster entries"
        );
        let capacity = (self.entries.len() * 2).next_power_of_two().max(8);
        self.index.resize(capacity, EMPTY);
        let mask = capacity - 1;
        for (position, (id, _)) in self.entries.iter().enumerate() {
            let mut slot = Self::hash(*id) & mask;
            while self.index[slot] != EMPTY {
                slot = (slot + 1) & mask;
            }
            self.index[slot] = position as u32;
        }
    }

    #[inline]
    fn position(&self, id: u32) -> Option<usize> {
        if self.index.is_empty() {
            return None;
        }
        let mask = self.index.len() - 1;
        let mut slot = Self::hash(id) & mask;
        loop {
            let position = self.index[slot];
            if position == EMPTY {
                return None;
            }
            if self.entries[position as usize].0 == id {
                return Some(position as usize);
            }
            slot = (slot + 1) & mask;
        }
    }

    #[inline]
    pub fn get(&self, id: &u32) -> Option<&T> {
        self.position(*id).map(|position| &self.entries[position].1)
    }

    #[inline]
    pub fn get_mut(&mut self, id: &u32) -> Option<&mut T> {
        self.position(*id)
            .map(|position| &mut self.entries[position].1)
    }

    #[inline]
    pub fn get_key_value(&self, id: &u32) -> Option<(&u32, &T)> {
        self.position(*id).map(|position| {
            let (id, value) = &self.entries[position];
            (id, value)
        })
    }

    #[inline]
    pub fn contains_key(&self, id: &u32) -> bool {
        self.position(*id).is_some()
    }

    pub fn insert(&mut self, id: u32, value: T) -> Option<T> {
        match self.entries.binary_search_by_key(&id, |(key, _)| *key) {
            Ok(position) => Some(std::mem::replace(&mut self.entries[position].1, value)),
            Err(position) => {
                self.entries.insert(position, (id, value));
                self.reindex();
                None
            }
        }
    }

    pub fn remove(&mut self, id: &u32) -> Option<T> {
        let position = self.position(*id)?;
        let (_, value) = self.entries.remove(position);
        self.reindex();
        Some(value)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&u32, &mut T) -> bool) {
        // Vec keeps pairs together even when the predicate unwinds. Rebuild
        // the index on that path too, so a caught panic cannot leave it stale.
        struct ReindexOnDrop<'a, T>(&'a mut IdMap<T>);
        impl<T> Drop for ReindexOnDrop<'_, T> {
            fn drop(&mut self) {
                self.0.reindex();
            }
        }
        let guard = ReindexOnDrop(self);
        guard.0.entries.retain_mut(|(id, value)| keep(id, value));
    }

    pub fn iter(&self) -> Iter<'_, T> {
        Iter(self.entries.iter())
    }
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut(self.entries.iter_mut())
    }
    pub fn keys(&self) -> Keys<'_, T> {
        Keys(self.iter())
    }
    pub fn values(&self) -> Values<'_, T> {
        Values(self.iter())
    }
    pub fn values_mut(&mut self) -> ValuesMut<'_, T> {
        ValuesMut(self.iter_mut())
    }
    pub fn into_values(self) -> IntoValues<T> {
        IntoValues(self.entries.into_iter())
    }
}

pub struct Iter<'a, T>(slice::Iter<'a, (u32, T)>);
impl<T> Clone for Iter<'_, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (&'a u32, &'a T);
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(id, value)| (id, value))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<T> DoubleEndedIterator for Iter<'_, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(id, value)| (id, value))
    }
}
impl<T> ExactSizeIterator for Iter<'_, T> {}
impl<T> FusedIterator for Iter<'_, T> {}

pub struct IterMut<'a, T>(slice::IterMut<'a, (u32, T)>);
impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (&'a u32, &'a mut T);
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(id, value)| (&*id, value))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<T> DoubleEndedIterator for IterMut<'_, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(id, value)| (&*id, value))
    }
}
impl<T> ExactSizeIterator for IterMut<'_, T> {}
impl<T> FusedIterator for IterMut<'_, T> {}

// Concrete iterator types preserve BTreeMap's borrowing behavior in tail
// expressions. An opaque `impl Iterator` makes drop checking conservative
// enough to reject callers that borrow a local roster for a final lookup.
pub struct Keys<'a, T>(Iter<'a, T>);
impl<T> Clone for Keys<'_, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<'a, T> Iterator for Keys<'a, T> {
    type Item = &'a u32;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(id, _)| id)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<T> DoubleEndedIterator for Keys<'_, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(id, _)| id)
    }
}
impl<T> ExactSizeIterator for Keys<'_, T> {}
impl<T> FusedIterator for Keys<'_, T> {}

pub struct Values<'a, T>(Iter<'a, T>);
impl<T> Clone for Values<'_, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<'a, T> Iterator for Values<'a, T> {
    type Item = &'a T;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, value)| value)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<T> DoubleEndedIterator for Values<'_, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(_, value)| value)
    }
}
impl<T> ExactSizeIterator for Values<'_, T> {}
impl<T> FusedIterator for Values<'_, T> {}

pub struct ValuesMut<'a, T>(IterMut<'a, T>);
impl<'a, T> Iterator for ValuesMut<'a, T> {
    type Item = &'a mut T;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, value)| value)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<T> DoubleEndedIterator for ValuesMut<'_, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(_, value)| value)
    }
}
impl<T> ExactSizeIterator for ValuesMut<'_, T> {}
impl<T> FusedIterator for ValuesMut<'_, T> {}

pub struct IntoValues<T>(std::vec::IntoIter<(u32, T)>);
impl<T> Iterator for IntoValues<T> {
    type Item = T;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, value)| value)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<T> DoubleEndedIterator for IntoValues<T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(_, value)| value)
    }
}
impl<T> ExactSizeIterator for IntoValues<T> {}
impl<T> FusedIterator for IntoValues<T> {}

impl<T> std::ops::Index<&u32> for IdMap<T> {
    type Output = T;
    fn index(&self, id: &u32) -> &T {
        self.get(id).expect("no entry found for key")
    }
}
impl<'a, T> IntoIterator for &'a IdMap<T> {
    type Item = (&'a u32, &'a T);
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, T> IntoIterator for &'a mut IdMap<T> {
    type Item = (&'a u32, &'a mut T);
    type IntoIter = IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
impl<T> IntoIterator for IdMap<T> {
    type Item = (u32, T);
    type IntoIter = std::vec::IntoIter<(u32, T)>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}
impl<T> From<BTreeMap<u32, T>> for IdMap<T> {
    fn from(map: BTreeMap<u32, T>) -> Self {
        let mut map = Self {
            entries: map.into_iter().collect(),
            index: Vec::new(),
        };
        map.reindex();
        map
    }
}
impl<T> FromIterator<(u32, T)> for IdMap<T> {
    fn from_iter<I: IntoIterator<Item = (u32, T)>>(items: I) -> Self {
        // Preserve BTreeMap's last-value-wins behavior for duplicate IDs.
        BTreeMap::from_iter(items).into()
    }
}
impl<T: PartialEq> PartialEq for IdMap<T> {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}
impl<T: Eq> Eq for IdMap<T> {}
impl<T: fmt::Debug> fmt::Debug for IdMap<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}
impl<T: Serialize> Serialize for IdMap<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.iter())
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for IdMap<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        BTreeMap::<u32, T>::deserialize(deserializer).map(Self::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::sync::Arc;

    fn agree(actual: &IdMap<i32>, expected: &BTreeMap<u32, i32>) {
        assert_eq!(
            actual.iter().collect::<Vec<_>>(),
            expected.iter().collect::<Vec<_>>()
        );
        assert_eq!(
            actual.iter().rev().collect::<Vec<_>>(),
            expected.iter().rev().collect::<Vec<_>>()
        );
        assert_eq!(actual.len(), expected.len());
        assert_eq!(actual.is_empty(), expected.is_empty());
        for id in expected.keys().chain([0, 1, u32::MAX].iter()) {
            assert_eq!(actual.get(id), expected.get(id));
            assert_eq!(actual.get_key_value(id), expected.get_key_value(id));
            assert_eq!(actual.contains_key(id), expected.contains_key(id));
        }
    }

    #[test]
    fn mutations_match_the_ordered_reference() {
        let mut actual = IdMap::new();
        let mut expected = BTreeMap::new();
        let mut random = 0x8319_752du32;
        for step in 0..10_000 {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            let id = if step % 97 == 0 {
                u32::MAX
            } else {
                random % 1024
            };
            match random % 7 {
                0..=2 => assert_eq!(actual.insert(id, step), expected.insert(id, step)),
                3 => assert_eq!(actual.remove(&id), expected.remove(&id)),
                4 => {
                    if let Some(value) = actual.get_mut(&id) {
                        *value += 1;
                    }
                    if let Some(value) = expected.get_mut(&id) {
                        *value += 1;
                    }
                }
                5 if step % 23 == 0 => {
                    actual.retain(|key, value| {
                        *value += 1;
                        key % 5 != id % 5
                    });
                    expected.retain(|key, value| {
                        *value += 1;
                        key % 5 != id % 5
                    });
                }
                _ => assert_eq!(actual.get(&id), expected.get(&id)),
            }
            if step % 31 == 0 {
                agree(&actual, &expected);
            }
        }
        agree(&actual, &expected);
        actual.clear();
        expected.clear();
        agree(&actual, &expected);
    }

    #[test]
    fn collisions_and_removals_keep_every_id_findable() {
        let ids: Vec<_> = (0..u32::MAX)
            .filter(|id| IdMap::<i32>::hash(*id) & 127 == 0)
            .take(50)
            .collect();
        let mut actual: IdMap<_> = ids.iter().rev().map(|id| (*id, *id as i32)).collect();
        let mut expected: BTreeMap<_, _> = ids.iter().map(|id| (*id, *id as i32)).collect();
        for id in [ids[0], ids[25], ids[49]] {
            assert_eq!(actual.remove(&id), expected.remove(&id));
            agree(&actual, &expected);
        }
        assert_eq!(actual.get(&u32::MAX), None);
    }

    #[test]
    fn sparse_ids_and_duplicate_entries_keep_the_save_format() {
        let items = [(u32::MAX, 7), (10, 1), (2, 2), (10, 3), (0, 4)];
        let actual: IdMap<_> = items.into_iter().collect();
        let expected: BTreeMap<_, _> = items.into_iter().collect();
        agree(&actual, &expected);
        let json = serde_json::to_string(&actual).unwrap();
        assert_eq!(json, serde_json::to_string(&expected).unwrap());
        let restored: IdMap<i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, actual);
        let duplicates: IdMap<i32> = serde_json::from_str(r#"{"12":1,"2":2,"12":3}"#).unwrap();
        assert_eq!(
            duplicates.into_iter().collect::<Vec<_>>(),
            [(2, 2), (12, 3)]
        );
    }

    #[test]
    fn iteration_preserves_order_and_exact_lengths() {
        let mut actual: IdMap<_> = [(9, 90), (1, 10), (5, 50)].into_iter().collect();
        let mut iter = actual.iter();
        assert_eq!(iter.len(), 3);
        assert_eq!(iter.next(), Some((&1, &10)));
        assert_eq!(iter.next_back(), Some((&9, &90)));
        assert_eq!(iter.size_hint(), (1, Some(1)));
        assert_eq!(iter.next(), Some((&5, &50)));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next_back(), None);
        for (id, value) in &mut actual {
            *value += *id as i32;
        }
        assert_eq!(actual.values().copied().collect::<Vec<_>>(), [11, 55, 99]);
        for value in actual.values_mut() {
            *value += 1;
        }
        assert_eq!(
            actual.clone().into_values().rev().collect::<Vec<_>>(),
            [100, 56, 12]
        );
        assert_eq!(actual.keys().copied().collect::<Vec<_>>(), [1, 5, 9]);
    }

    #[test]
    fn a_tail_expression_can_finish_borrowing_a_local_roster() {
        let best = {
            let roster: IdMap<_> = [(1, 10), (2, 20)].into_iter().collect();
            roster.values().copied().max().unwrap()
        };
        assert_eq!(best, 20);
    }

    #[test]
    fn copy_on_write_keeps_the_snapshot_and_untouched_units() {
        let mut branch = Arc::new(
            [(1, Arc::new(vec![10])), (2, Arc::new(vec![20]))]
                .into_iter()
                .collect::<IdMap<_>>(),
        );
        let snapshot = Arc::clone(&branch);
        assert!(Arc::ptr_eq(&branch, &snapshot));
        Arc::make_mut(Arc::make_mut(&mut branch).get_mut(&2).unwrap()).push(21);
        assert_eq!(snapshot[&2].as_slice(), [20]);
        assert_eq!(branch[&2].as_slice(), [20, 21]);
        assert!(Arc::ptr_eq(&branch[&1], &snapshot[&1]));
        Arc::make_mut(&mut branch).remove(&1);
        assert!(snapshot.contains_key(&1));
        assert!(!branch.contains_key(&1));
    }

    #[test]
    fn a_caught_retain_panic_leaves_a_valid_index() {
        let mut actual: IdMap<_> = (0..8).map(|id| (id, id as i32)).collect();
        let mut expected: BTreeMap<_, _> = actual.clone().into_iter().collect();
        assert!(catch_unwind(AssertUnwindSafe(|| actual.retain(|id, value| {
            if *id == 3 {
                panic!("predicate stopped");
            }
            *value += 1;
            *id >= 2
        })))
        .is_err());
        assert!(
            catch_unwind(AssertUnwindSafe(|| expected.retain(|id, value| {
                if *id == 3 {
                    panic!("predicate stopped");
                }
                *value += 1;
                *id >= 2
            })))
            .is_err()
        );
        agree(&actual, &expected);
        assert_eq!(actual.remove(&3), expected.remove(&3));
        agree(&actual, &expected);
    }
}
