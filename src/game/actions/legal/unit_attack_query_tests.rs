//! Selected-unit survival attacks retain full enumeration order and policy reads.

use super::*;
use crate::rules::{ModifierCollection, ModifierSpec};
use std::sync::Arc;

fn board() -> (Game, Vec<u32>, u32) {
    let mut g = Game::new_full(2, 32, 20, 5269, 100, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.clear_mirror_cities();
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    g.map.clear_rivers();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = None;
        tile.owner_city = None;
        tile.hills = false;
        tile.road = 0;
    }
    g.found_city_for(0, (4, 6), None);
    g.found_city_for(1, (22, 10), None);
    g.players[0].techs.extend(g.rules.techs.keys().copied());
    g.players[0].civics.extend(g.rules.civics.keys().copied());
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.current = 0;
    let front = (10, 8);
    let enemy_pos = g.nbrs(front)[0];
    let own = vec![
        g.spawn_test_unit("warrior", 0, front),
        g.spawn_test_unit("archer", 0, front),
        g.spawn_test_unit("spec_ops", 0, front),
        g.spawn_test_unit("builder", 0, (4, 6)),
        g.spawn_test_unit("military_engineer", 0, (4, 6)),
    ];
    let enemy = g.spawn_test_unit("warrior", 1, enemy_pos);
    g.spawn_test_unit("battering_ram", 1, enemy_pos);
    g.reveal(0, front, 5);
    (g, own, enemy)
}

/// Original full-roster enumeration and exact selection, kept independently.
fn original(g: &Game, pid: usize, uid: u32) -> Vec<Action> {
    g.legal_actions_within(pid, ActionFamilies::UNITS)
        .into_iter()
        .filter(|action| {
            matches!(
                action,
                Action::Attack { unit, .. }
                    | Action::Ranged { unit, .. }
                    | Action::PriorityTarget { unit, .. }
                    if *unit == uid
            )
        })
        .collect()
}

fn same(g: &Game, uid: u32) {
    assert_eq!(g.legal_survival_attack_actions(0, uid), original(g, 0, uid));
}

#[test]
fn melee_ranged_and_priority_attacks_keep_the_original_order() {
    let (g, own, _) = board();
    for &uid in &own {
        same(&g, uid);
    }
    assert!(original(&g, 0, own[0])
        .iter()
        .any(|a| matches!(a, Action::Attack { .. })));
    assert!(original(&g, 0, own[1])
        .iter()
        .any(|a| matches!(a, Action::Ranged { .. })));
    assert!(original(&g, 0, own[2])
        .iter()
        .any(|a| matches!(a, Action::PriorityTarget { .. })));
}

#[test]
fn spent_movement_attacks_and_different_unit_health_keep_exact_actions() {
    let (mut g, own, _) = board();
    for &uid in &own[..3] {
        for (moves, attacks, hp) in [(0.0, 0, 25), (0.0, 1, 100), (2.0, 0, 50), (2.0, 1, 100)] {
            let unit = g.units.get_mut(&uid).unwrap();
            unit.moves_left = moves;
            unit.attacks_left = attacks;
            unit.hp = hp;
            same(&g, uid);
        }
    }
}

#[test]
fn current_player_finished_missing_and_foreign_units_keep_the_old_gates() {
    let (mut g, own, enemy) = board();
    same(&g, u32::MAX);
    same(&g, enemy);
    assert!(g.legal_survival_attack_actions(0, enemy).is_empty());
    g.current = 1;
    same(&g, own[0]);
    assert!(g.legal_survival_attack_actions(0, own[0]).is_empty());
    g.current = 0;
    g.winner = Some(0);
    same(&g, own[0]);
    assert!(g.legal_survival_attack_actions(0, own[0]).is_empty());
}

#[test]
fn a_pending_city_disposition_remains_exclusive() {
    let (mut g, own, _) = board();
    let cid = g.player_city_ids(0)[0];
    g.cities.get_mut(&cid).unwrap().captured_from = Some(1);
    assert!(!g.pending_city_capture_actions(0).is_empty());
    for uid in own {
        same(&g, uid);
        assert!(g.legal_survival_attack_actions(0, uid).is_empty());
    }
}

#[test]
fn stock_military_kinds_keep_the_full_enumerator_subsequence() {
    let (g, _, _) = board();
    let kinds: Vec<_> = g
        .rules
        .units
        .iter()
        .filter(|(_, spec)| spec.class == "military")
        .map(|(kind, _)| *kind)
        .collect();
    assert!(kinds.len() > 20);
    for kind in kinds {
        let mut example = g.clone();
        let uid = example.spawn_test_unit(kind.as_str(), 0, (10, 8));
        same(&example, uid);
    }
}

fn traced(g: &Game, uid: u32, selected: bool) -> (Vec<Action>, PolicyReadSet) {
    let guard = g.trace_policy_reads(0);
    let actions = if selected {
        g.legal_survival_attack_actions(0, uid)
    } else {
        original(g, 0, uid)
    };
    let reads = guard.finish();
    (actions, reads)
}

#[test]
fn active_traces_keep_all_original_effect_card_and_inertness_reads() {
    let (mut g, own, _) = board();
    g.players[0]
        .policies
        .extend(g.rules.policies.keys().copied());
    for uid in own {
        let (old_actions, old) = traced(&g, uid, false);
        let (new_actions, new) = traced(&g, uid, true);
        assert_eq!(old_actions, new_actions);
        assert_eq!(old.effects, new.effects);
        assert_eq!(old.cards, new.cards);
        assert_eq!(old.opaque, new.opaque);
        assert!(
            !old.effects.is_empty(),
            "the full trace must read real policy effects"
        );
        for card in g.rules.policies.keys() {
            assert_eq!(
                old.card_is_inert(&g.rules, *card),
                new.card_is_inert(&g.rules, *card)
            );
        }
    }
}

#[test]
fn runtime_attachments_keep_the_original_opaque_trace_and_actions() {
    let (mut g, own, _) = board();
    Arc::make_mut(&mut g.rules).modifiers.insert(
        "survival_query_attachment".to_string(),
        ModifierSpec {
            effects: BTreeMap::from([("movement".to_string(), 1.0)]),
            collection: ModifierCollection::Player,
            ..ModifierSpec::default()
        },
    );
    g.attach_modifier_to_player(0, "survival_query_attachment")
        .unwrap();
    let (old_actions, old) = traced(&g, own[0], false);
    let (new_actions, new) = traced(&g, own[0], true);
    assert_eq!(old_actions, new_actions);
    assert!(old.is_opaque() && new.is_opaque());
    assert_eq!(old.effects, new.effects);
    assert_eq!(old.cards, new.cards);
    same(&g, own[0]);
}

#[test]
fn visibility_terrain_clone_and_save_load_keep_identical_ordered_attacks() {
    let (mut g, own, enemy) = board();
    let position = g.units[&enemy].pos;
    g.map.tiles.get_mut(&position).unwrap().hills = true;
    g.map.tiles.get_mut(&position).unwrap().feature = Some(crate::name!("woods"));
    g.relocate(enemy, (13, 8));
    for &uid in &own {
        same(&g, uid);
    }
    let cloned = g.clone();
    let restored: Game = serde_json::from_value(serde_json::to_value(&g).unwrap()).unwrap();
    for uid in own {
        same(&cloned, uid);
        same(&restored, uid);
        assert_eq!(
            cloned.legal_survival_attack_actions(0, uid),
            restored.legal_survival_attack_actions(0, uid)
        );
    }
}
