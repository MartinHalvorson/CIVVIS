//! Interned effect indices preserve sums, dynamic modifiers, traces and reindexing.

use super::*;
use crate::rules::{ModifierCollection, ModifierSpec, TreeEffectSources};

fn game() -> Game {
    Game::new_full(2, 16, 12, 5259, 40, 0, false)
}

fn policy_keys(g: &Game) -> Vec<String> {
    let mut keys: BTreeSet<String> = g
        .rules
        .policies
        .values()
        .flat_map(|card| card.effects.keys().cloned())
        .collect();
    keys.extend([String::new(), "named_effect_unknown".to_string()]);
    keys.into_iter().collect()
}

fn same_policy_values(g: &Game, keys: &[String]) {
    for pid in 0..g.players.len() {
        for key in keys {
            assert_eq!(
                g.policy_effect(pid, key).to_bits(),
                g.policy_effect_named(pid, Name::new(key)).to_bits(),
                "policy {pid} / {key}"
            );
        }
    }
}

#[test]
fn named_presence_matches_every_shipped_policy_effect_and_unknowns() {
    let g = game();
    for key in policy_keys(&g) {
        assert_eq!(
            g.rules.effect_index.policies(&key),
            g.rules.effect_index.policies_named(Name::new(&key)),
            "{key}"
        );
    }
}

#[test]
fn named_tree_reads_follow_research_changes_without_a_cache() {
    let mut g = game();
    let mut keys: Vec<Name> = g.rules.tree_effects.keys().copied().collect();
    keys.push(crate::name!("named_tree_unknown"));
    for researched in [false, true, false] {
        for pid in 0..g.players.len() {
            g.players[pid].techs.clear();
            g.players[pid].civics.clear();
            if researched {
                g.players[pid].techs.extend(g.rules.techs.keys().copied());
                g.players[pid].civics.extend(g.rules.civics.keys().copied());
            }
            for key in &keys {
                assert_eq!(
                    g.tree_effect(pid, key.as_str()).to_bits(),
                    g.tree_effect_named(pid, *key).to_bits(),
                    "tree {pid} / {key} / {researched}"
                );
            }
        }
    }
}

#[test]
fn named_tree_sum_keeps_source_order_and_civics_after_technologies() {
    let mut g = game();
    let nodes = [
        crate::name!("named_order_a"),
        crate::name!("named_order_b"),
        crate::name!("named_order_c"),
    ];
    let civic = crate::name!("named_order_civic");
    g.players[0].techs.extend(nodes);
    g.players[0].civics.insert(civic);
    Arc::make_mut(&mut g.rules).tree_effects.insert(
        "named_order_effect".to_string(),
        TreeEffectSources {
            techs: vec![(nodes[0], 1e16), (nodes[1], 1.0), (nodes[2], -1e16)],
            civics: vec![(civic, 2.0)],
        },
    );
    assert_eq!(
        g.tree_effect(0, "named_order_effect").to_bits(),
        2.0f64.to_bits()
    );
    assert_eq!(
        g.tree_effect_named(0, crate::name!("named_order_effect"))
            .to_bits(),
        2.0f64.to_bits()
    );
}

#[test]
fn named_policy_sums_follow_deck_and_anarchy_changes_bit_for_bit() {
    let mut g = game();
    let keys = policy_keys(&g);
    same_policy_values(&g, &keys);
    let cards: Vec<Name> = g.rules.policies.keys().copied().collect();
    for pid in 0..g.players.len() {
        g.players[pid].policies.extend(cards.iter().copied());
    }
    same_policy_values(&g, &keys);
    g.players[0].anarchy_turns = 3;
    same_policy_values(&g, &keys);
    g.players[0].anarchy_turns = 0;
    g.players[0].policies.clear();
    same_policy_values(&g, &keys);
}

fn attach_late_modifier(g: &mut Game, name: &str, collection: ModifierCollection, value: f64) {
    Arc::make_mut(&mut g.rules).modifiers.insert(
        name.to_string(),
        ModifierSpec {
            effects: BTreeMap::from([("named_runtime_effect".to_string(), value)]),
            collection,
            ..ModifierSpec::default()
        },
    );
    g.attach_modifier_to_player(0, name).unwrap();
}

#[test]
fn late_runtime_attachments_keep_collections_and_anarchy_semantics() {
    let mut g = game();
    let key = crate::name!("named_runtime_effect");
    assert!(!g.rules.effect_index.policies_named(key));
    assert_eq!(g.policy_effect_named(0, key), 0.0);
    attach_late_modifier(&mut g, "named_player", ModifierCollection::Player, 7.0);
    attach_late_modifier(
        &mut g,
        "named_cities",
        ModifierCollection::PlayerCities,
        3.0,
    );
    attach_late_modifier(&mut g, "named_units", ModifierCollection::PlayerUnits, 11.0);
    // Runtime bundles were installed after indexing. Only the same two
    // collections the original generic policy consumer reads contribute.
    assert!(!g.rules.effect_index.policies_named(key));
    for anarchy in [0, 3] {
        g.players[0].anarchy_turns = anarchy;
        assert_eq!(g.policy_effect(0, key.as_str()), 10.0);
        assert_eq!(g.policy_effect_named(0, key), 10.0);
    }
    g.detach_modifier_from_player(0, "named_cities");
    assert_eq!(g.policy_effect_named(0, key), 7.0);
    Arc::make_mut(&mut g.rules)
        .modifiers
        .get_mut("named_player")
        .unwrap()
        .effects
        .insert(key.as_str().to_string(), 17.0);
    assert_eq!(g.policy_effect(0, key.as_str()), 17.0);
    assert_eq!(g.policy_effect_named(0, key), 17.0);
}

fn trace(g: &Game, keys: &[String], named: bool) -> PolicyReadSet {
    let guard = g.trace_policy_reads(0);
    for key in keys {
        if named {
            g.policy_effect_named(0, Name::new(key));
        } else {
            g.policy_effect(0, key);
        }
    }
    g.has_policy(0, "urban_planning");
    guard.finish()
}

#[test]
fn named_queries_record_identical_policy_reads_and_card_inertness() {
    let mut g = game();
    let keys = policy_keys(&g);
    for attached in [false, true] {
        if attached {
            attach_late_modifier(&mut g, "named_trace", ModifierCollection::Player, 1.0);
        }
        let raw = trace(&g, &keys, false);
        let named = trace(&g, &keys, true);
        assert_eq!(raw.effects, named.effects);
        assert_eq!(raw.cards, named.cards);
        assert_eq!(raw.opaque, named.opaque);
        assert_eq!(raw.is_opaque(), attached);
        for card in g.rules.policies.keys() {
            assert_eq!(
                raw.card_is_inert(&g.rules, *card),
                named.card_is_inert(&g.rules, *card)
            );
        }
    }
}

#[test]
fn late_names_follow_table_reindexing_and_removal() {
    let mut g = game();
    let key = Name::new("named_late_key");
    let node = Name::new("named_late_node");
    assert!(!g.rules.effect_index.policies_named(key));
    assert_eq!(g.tree_effect_named(0, key), 0.0);
    let mut card = g.rules.policies.get("urban_planning").unwrap().clone();
    card.effects = BTreeMap::from([(key.as_str().to_string(), 9.125)]);
    {
        let rules = Arc::make_mut(&mut g.rules);
        rules.policies.insert("named_late_card".to_string(), card);
        rules
            .effect_index
            .policies
            .insert(key.as_str().to_string(), ());
        rules.tree_effects.insert(
            key.as_str().to_string(),
            TreeEffectSources {
                techs: vec![(node, 4.25)],
                civics: Vec::new(),
            },
        );
    }
    g.players[0].policies.clear();
    g.players[0]
        .policies
        .insert(crate::name!("named_late_card"));
    g.players[0].techs.insert(node);
    assert!(g.rules.effect_index.policies_named(key));
    assert_eq!(g.policy_effect_named(0, key), 9.125);
    assert_eq!(g.policy_effect(0, key.as_str()), 9.125);
    assert_eq!(g.tree_effect_named(0, key), 4.25);
    assert_eq!(g.tree_effect(0, key.as_str()), 4.25);
    {
        let rules = Arc::make_mut(&mut g.rules);
        rules.policies.remove("named_late_card");
        rules.effect_index.policies.remove(key.as_str());
        rules.tree_effects.remove(key.as_str());
    }
    assert!(!g.rules.effect_index.policies_named(key));
    assert_eq!(g.policy_effect_named(0, key), 0.0);
    assert_eq!(g.tree_effect_named(0, key), 0.0);
}

#[test]
fn saved_game_restores_interned_lookup_results() {
    let mut g = game();
    g.players[0].policies.insert(crate::name!("urban_planning"));
    g.players[0].techs.extend(g.rules.techs.keys().copied());
    let keys = policy_keys(&g);
    let restored: Game = serde_json::from_value(serde_json::to_value(&g).unwrap()).unwrap();
    same_policy_values(&restored, &keys);
    for key in g.rules.tree_effects.keys() {
        assert_eq!(
            g.tree_effect(0, key).to_bits(),
            restored.tree_effect_named(0, *key).to_bits()
        );
    }
}
