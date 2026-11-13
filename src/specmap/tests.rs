//! Lookup collisions, mutation and serialization against the ordered map contract.

use super::{hash_key, LookupSlot, Name, SpecMap, EMPTY};
use std::collections::BTreeMap;

#[test]
fn keeps_entries_in_key_order() {
    let mut map = SpecMap::new();
    map.insert("warrior".to_string(), 1);
    map.insert("archer".to_string(), 2);
    map.insert("slinger".to_string(), 3);
    let keys: Vec<&str> = map.keys().map(|name| name.as_str()).collect();
    assert_eq!(keys, ["archer", "slinger", "warrior"]);
    assert_eq!(map.values().copied().collect::<Vec<_>>(), [2, 3, 1]);
}

#[test]
fn finds_replaces_and_removes() {
    let mut map = SpecMap::new();
    for name in ["a", "bb", "ccc", "dddddddd", "eeeeeeeee", "f"] {
        map.insert(name.to_string(), name.len());
    }
    for name in ["a", "bb", "ccc", "dddddddd", "eeeeeeeee", "f"] {
        assert_eq!(map.get(name), Some(&name.len()));
        assert_eq!(
            map.get_key_value(name),
            Some((&Name::new(name), &name.len()))
        );
        assert!(map.contains_key(name));
    }
    assert_eq!(map.get("missing"), None);
    assert_eq!(map.get_key_value("missing"), None);
    assert_eq!(map.insert("bb".to_string(), 99), Some(2));
    assert_eq!(map["bb"], 99);
    assert_eq!(map.get_key_value("bb"), Some((&Name::new("bb"), &99)));
    assert_eq!(map.remove("bb"), Some(99));
    assert_eq!(map.get("bb"), None);
    assert_eq!(map.get_key_value("bb"), None);
    assert_eq!(map.len(), 5);
}

fn colliding_keys(bucket: usize, mask: usize, count: usize) -> Vec<String> {
    (0..)
        .map(|i| format!("lookup_collision_{i}"))
        .filter(|key| hash_key(key) as usize & mask == bucket)
        .take(count)
        .collect()
}

#[test]
fn a_cluster_wraps_and_survives_replacement_and_removal() {
    let keys = colliding_keys(7, 7, 4);
    let mut map: SpecMap<usize> = keys[..3]
        .iter()
        .enumerate()
        .map(|(i, key)| (key.clone(), i))
        .collect();
    assert_eq!(map.table.len(), 8);
    assert!(map.table[7].index != EMPTY && map.table[0].index != EMPTY);
    for (i, key) in keys[..3].iter().enumerate() {
        assert_eq!(map.get(key), Some(&i));
        assert_eq!(map.get_interned(Name::new(key)), Some(&i));
    }
    assert_eq!(map.get(&keys[3]), None);
    assert_eq!(map.insert(keys[1].clone(), 19), Some(1));
    assert_eq!(map.remove(&keys[0]), Some(0));
    assert_eq!(map.get(&keys[0]), None);
    assert_eq!(map.get(&keys[1]), Some(&19));
    assert_eq!(map.get(&keys[2]), Some(&2));
    assert_eq!(map.get(&keys[3]), None);
}

#[test]
fn an_equal_fingerprint_still_requires_equal_text() {
    let keys = colliding_keys(7, 7, 3);
    let mut map: SpecMap<i32> = [(keys[0].clone(), 11), (keys[1].clone(), 22)]
        .into_iter()
        .collect();
    // Deliberately simulate a hash fingerprint collision at the first probe.
    // It must compare the full key and continue to the later real match.
    map.table[7].fingerprint = (hash_key(&keys[1]) >> 32) as u32;
    assert_eq!(map.get(&keys[1]), Some(&22));
    // The same collision on an absent key must remain a miss, too.
    for slot in &mut map.table {
        if slot.index != EMPTY {
            slot.fingerprint = (hash_key(&keys[2]) >> 32) as u32;
        }
    }
    assert_eq!(map.get(&keys[2]), None);
    assert_eq!(map.get_key_value(&keys[2]), None);
}

#[test]
fn fingerprints_never_order_or_serialize_entries() {
    let entries: BTreeMap<String, i32> = [
        ("".into(), 7),
        ("alpha".into(), 8),
        ("alpha\0suffix".into(), 9),
        ("αθήνα".into(), 10),
        ("long_shared_prefix_one".into(), 11),
        ("long_shared_prefix_two".into(), 12),
    ]
    .into_iter()
    .collect();
    let map = SpecMap::from(entries.clone());
    let encoded = serde_json::to_string(&map).unwrap();
    assert_eq!(encoded, serde_json::to_string(&entries).unwrap());
    let restored: SpecMap<i32> = serde_json::from_str(&encoded).unwrap();
    for copy in [map.clone(), restored] {
        assert_eq!(copy, map);
        for (key, value) in &entries {
            assert_eq!(copy.get(key), Some(value));
            assert_eq!(copy.get_interned(Name::new(key)), Some(value));
            assert_eq!(copy.get_key_value(key), Some((&Name::new(key), value)));
        }
    }
    assert_eq!(std::mem::size_of::<LookupSlot>(), 8);
}

#[test]
fn mixed_mutations_match_btree_results_and_wire_bytes() {
    let keys: Vec<_> = (0..96)
        .map(|i| format!("rules_lookup_{i:03}_shared_prefix"))
        .collect();
    let mut map = SpecMap::new();
    let mut ordered = BTreeMap::new();
    let mut random = 0x517c_c1b7_u32;
    for step in 0..768_i32 {
        random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let key = &keys[(random >> 8) as usize % keys.len()];
        match random % 4 {
            0 | 1 => assert_eq!(
                map.insert(key.clone(), step),
                ordered.insert(key.clone(), step)
            ),
            2 => assert_eq!(map.remove(key), ordered.remove(key)),
            _ => {
                if let Some(value) = map.get_mut(key) {
                    *value -= 1;
                }
                if let Some(value) = ordered.get_mut(key) {
                    *value -= 1;
                }
            }
        }
        for key in &keys {
            assert_eq!(map.get(key), ordered.get(key));
            assert_eq!(map.contains_key(key), ordered.contains_key(key));
            assert_eq!(map.get_interned(Name::new(key)), ordered.get(key));
        }
        if step % 64 == 0 {
            assert_eq!(
                serde_json::to_vec(&map).unwrap(),
                serde_json::to_vec(&ordered).unwrap()
            );
            let copy = map.clone();
            assert_eq!(copy, map);
        }
    }
}

#[test]
fn an_empty_reindexed_table_and_late_interned_names_still_miss() {
    let mut map = SpecMap::new();
    assert_eq!(map.get("never_inserted"), None);
    map.insert("only_entry".to_string(), 1);
    assert_eq!(map.remove("only_entry"), Some(1));
    assert!(map.is_empty());
    assert_eq!(map.get("only_entry"), None);
    let late = Name::new("rules_lookup_name_interned_after_empty_table");
    assert_eq!(map.get_interned(late), None);
    assert_eq!(map.get(late.as_str()), None);
}
