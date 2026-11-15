//! The numeric effects a ruleset entry carries, keyed by name.
//!
//! A building's or a policy card's `effects` used to be a
//! `BTreeMap<String, f64>`, and the engine asks those maps one question far
//! more than any other: "does this entry grant *that* effect?", for every
//! building in a city and every card on a seat, many times per city yield.
//! A tree answers it with a string *ordering* comparison per entry it passes,
//! and the answer is nearly always "no". Measured on the site's world (seed 7,
//! turns ~150-210), those lookups under `city_building_effect` and
//! `policy_effect` were 2.5% of the stepping thread.
//!
//! An entry carries a handful of effects. Kept as a short vector in the same
//! key order, a lookup by text is a scan whose comparisons are equality tests
//! (unequal lengths reject without reading a byte), and a lookup by an
//! interned [`Name`] compares one integer per entry. Iteration, serialization
//! and every value are what the tree gave, so nothing that reads the map can
//! tell the difference except by the clock.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::name::Name;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectMap {
    /// In ascending key order, as the `BTreeMap` iterated.
    entries: Vec<(String, f64)>,
    /// `Name::id` of each key, index for index.
    ids: Vec<u32>,
}

impl EffectMap {
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
    fn position(&self, key: &str) -> Option<usize> {
        self.entries.iter().position(|(entry, _)| entry == key)
    }

    #[inline]
    pub fn get(&self, key: &str) -> Option<&f64> {
        self.position(key).map(|index| &self.entries[index].1)
    }

    #[inline]
    pub fn get_mut(&mut self, key: &str) -> Option<&mut f64> {
        self.position(key).map(|index| &mut self.entries[index].1)
    }

    #[inline]
    pub fn contains_key(&self, key: &str) -> bool {
        self.position(key).is_some()
    }

    /// The value for an interned key: one integer comparison per entry.
    #[inline]
    pub fn get_name(&self, key: Name) -> Option<f64> {
        let id = key.id();
        self.ids
            .iter()
            .position(|entry| *entry == id)
            .map(|index| self.entries[index].1)
    }

    pub fn insert(&mut self, key: String, value: f64) -> Option<f64> {
        match self
            .entries
            .binary_search_by(|(entry, _)| entry.as_str().cmp(key.as_str()))
        {
            Ok(index) => Some(std::mem::replace(&mut self.entries[index].1, value)),
            Err(index) => {
                self.ids.insert(index, Name::new(&key).id());
                self.entries.insert(index, (key, value));
                None
            }
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<f64> {
        let index = self.position(key)?;
        self.ids.remove(index);
        Some(self.entries.remove(index).1)
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&String, &f64)> + ExactSizeIterator {
        self.entries.iter().map(|(key, value)| (key, value))
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &String> + ExactSizeIterator {
        self.entries.iter().map(|(key, _)| key)
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &f64> + ExactSizeIterator {
        self.entries.iter().map(|(_, value)| value)
    }
}

impl<'a> IntoIterator for &'a EffectMap {
    type Item = (&'a String, &'a f64);
    type IntoIter = std::iter::Map<
        std::slice::Iter<'a, (String, f64)>,
        fn(&'a (String, f64)) -> (&'a String, &'a f64),
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter().map(|(key, value)| (key, value))
    }
}

impl From<BTreeMap<String, f64>> for EffectMap {
    fn from(map: BTreeMap<String, f64>) -> Self {
        let ids = map.keys().map(|key| Name::new(key).id()).collect();
        EffectMap {
            entries: map.into_iter().collect(),
            ids,
        }
    }
}

impl FromIterator<(String, f64)> for EffectMap {
    fn from_iter<I: IntoIterator<Item = (String, f64)>>(iter: I) -> Self {
        EffectMap::from(iter.into_iter().collect::<BTreeMap<_, _>>())
    }
}

impl Extend<(String, f64)> for EffectMap {
    fn extend<I: IntoIterator<Item = (String, f64)>>(&mut self, iter: I) {
        for (key, value) in iter {
            self.insert(key, value);
        }
    }
}

impl std::ops::Index<&str> for EffectMap {
    type Output = f64;

    fn index(&self, key: &str) -> &f64 {
        self.get(key)
            .unwrap_or_else(|| panic!("no effect named {key}"))
    }
}

impl Serialize for EffectMap {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.iter())
    }
}

impl<'de> Deserialize<'de> for EffectMap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        BTreeMap::<String, f64>::deserialize(deserializer).map(EffectMap::from)
    }
}

#[cfg(test)]
mod tests {
    use super::EffectMap;
    use crate::name::Name;
    use std::collections::BTreeMap;

    fn tree() -> BTreeMap<String, f64> {
        [("zeta", 1.0), ("alpha", 2.5), ("mid_key", -3.0), ("b", 0.0)]
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect()
    }

    #[test]
    fn reads_iterates_and_serializes_as_the_tree_did() {
        let tree = tree();
        let map = EffectMap::from(tree.clone());
        assert_eq!(map.len(), tree.len());
        assert!(map.iter().eq(tree.iter()));
        assert!(map.keys().eq(tree.keys()));
        for key in ["zeta", "alpha", "mid_key", "b", "missing", ""] {
            assert_eq!(map.get(key), tree.get(key));
            assert_eq!(map.contains_key(key), tree.contains_key(key));
            assert_eq!(map.get_name(Name::new(key)), tree.get(key).copied());
        }
        assert_eq!(
            serde_json::to_string(&map).unwrap(),
            serde_json::to_string(&tree).unwrap()
        );
        let back: EffectMap = serde_json::from_str(&serde_json::to_string(&tree).unwrap()).unwrap();
        assert_eq!(back, map);
    }

    #[test]
    fn inserts_and_removes_keep_order_and_names_in_step() {
        let mut tree = tree();
        let mut map = EffectMap::from(tree.clone());
        assert_eq!(map.insert("c".into(), 9.0), tree.insert("c".into(), 9.0));
        assert_eq!(
            map.insert("alpha".into(), 7.0),
            tree.insert("alpha".into(), 7.0)
        );
        assert_eq!(map.remove("zeta"), tree.remove("zeta"));
        assert_eq!(map.remove("zeta"), tree.remove("zeta"));
        assert!(map.iter().eq(tree.iter()));
        for key in ["c", "alpha", "zeta", "b", "mid_key"] {
            assert_eq!(map.get_name(Name::new(key)), tree.get(key).copied());
        }
    }
}
