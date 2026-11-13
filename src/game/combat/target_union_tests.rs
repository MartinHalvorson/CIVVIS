//! Dense target unions retain every original attack tile and movement flood.

use super::*;

fn board(planet: bool) -> (Game, Pos) {
    let mut g = if planet {
        let topology = crate::setup::MapTopology::Planet;
        let (width, height) = crate::setup::MapSize::for_players(2).dimensions(topology);
        Game::new_with(GameOptions {
            map_topology: topology,
            barbarians: false,
            ..GameOptions::new(2, width, height, 5274, 100, 0)
        })
    } else {
        Game::new_full(2, 24, 16, 5274, 100, 0, false)
    };
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.clear_mirror_cities();
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
    for player in &mut g.players {
        player.techs.extend(g.rules.techs.keys().copied());
        player.civics.extend(g.rules.civics.keys().copied());
    }
    g.at_war.insert(pair(0, 1));
    let start = *g
        .map
        .tiles
        .keys()
        .find(|pos| g.wdisk(**pos, 4).len() >= 61)
        .unwrap();
    (g, start)
}

fn same(g: &Game, uid: u32) -> (Vec<Pos>, Vec<Pos>) {
    let actual = g.attack_reach_from_flood(uid);
    let original = g.original_attack_reach_from_flood(uid);
    assert_eq!(
        actual, original,
        "unit {uid} changed targets or flood order"
    );
    assert!(actual.0.windows(2).all(|pair| pair[0] < pair[1]));
    actual
}

#[test]
fn repeated_targets_and_off_map_coordinates_keep_exact_position_order() {
    for planet in [false, true] {
        let (g, start) = board(planet);
        let mut values = g.wdisk(start, 4);
        assert!(values.len() >= 61);
        values.reverse();
        values.extend(values.clone());
        values.extend([(-1000, 4000), (3000, -3000), start, (-1000, 4000)]);
        let mut expected = values.clone();
        expected.sort_unstable();
        expected.dedup();
        let mut union = AttackTargetUnion::new(&g.map.tiles);
        for pos in values {
            union.push(pos);
        }
        assert_eq!(union.into_sorted(), expected);
    }
}

#[test]
fn mismatched_tile_records_retain_the_original_input_coordinates() {
    let (mut g, start) = board(false);
    let adjacent = g.nbrs(start)[0];
    let altered = (3000, -3000);
    g.map.tiles.get_mut(&start).unwrap().pos = altered;
    // get_mut changes the record, while the authoritative offset index still
    // resolves the old coordinate. Both must remain selectable independently.
    assert!(g.map.tiles.index_of(start).is_some());
    let values = [start, adjacent, altered, start, adjacent];
    let mut expected = values.to_vec();
    expected.sort_unstable();
    expected.dedup();
    let mut union = AttackTargetUnion::new(&g.map.tiles);
    for pos in values {
        union.push(pos);
    }
    assert_eq!(union.into_sorted(), expected);
}

#[test]
fn stock_military_kinds_match_the_original_union_on_both_topologies() {
    for planet in [false, true] {
        let (g, start) = board(planet);
        let kinds: Vec<_> = g
            .rules
            .units
            .iter()
            .filter(|(_, spec)| spec.class == "military")
            .map(|(kind, _)| *kind)
            .collect();
        assert!(kinds.len() > 20);
        for kind in kinds {
            let mut sample = g.clone();
            let uid = sample.spawn_test_unit(kind.as_str(), 0, start);
            same(&sample, uid);
        }
    }
}

#[test]
fn ranged_and_melee_floods_keep_nonempty_complete_sorted_targets() {
    for planet in [false, true] {
        let (mut g, start) = board(planet);
        for kind in ["warrior", "archer", "catapult", "spec_ops"] {
            let uid = g.spawn_test_unit(kind, 0, start);
            let (targets, flood) = same(&g, uid);
            assert!(!targets.is_empty(), "{kind} must contribute real targets");
            assert!(flood.len() > 1, "{kind} must walk a real flood");
            g.remove_unit(uid);
        }
    }
}

#[test]
fn terrain_roads_water_enemy_zoc_and_relocation_keep_the_original_rules() {
    for planet in [false, true] {
        let (mut g, start) = board(planet);
        let positions = g.wdisk(start, 3);
        for (n, pos) in positions.iter().copied().enumerate() {
            let tile = g.map.tiles.get_mut(&pos).unwrap();
            tile.hills = n % 3 == 0;
            tile.road = if n % 4 == 0 { 2 } else { 0 };
            tile.feature = (n % 5 == 0).then_some(crate::name!("forest"));
            if n % 7 == 0 && pos != start {
                tile.terrain = crate::name!("coast");
                tile.feature = None;
                tile.hills = false;
            }
        }
        let enemy_pos = g.nbrs(start)[0];
        let enemy = g.spawn_test_unit("warrior", 1, enemy_pos);
        for kind in ["warrior", "archer", "catapult", "horseman", "galley"] {
            let uid = g.spawn_test_unit(kind, 0, start);
            same(&g, uid);
            g.relocate(uid, enemy_pos);
            same(&g, uid);
            g.remove_unit(uid);
        }
        g.remove_unit(enemy);
        let uid = g.spawn_test_unit("archer", 0, start);
        same(&g, uid);
    }
}

#[test]
fn original_policy_reads_and_opaque_attachments_remain_identical() {
    use crate::rules::{ModifierCollection, ModifierSpec};
    use std::sync::Arc;

    let (mut g, start) = board(false);
    g.players[0]
        .policies
        .extend(g.rules.policies.keys().copied());
    Arc::make_mut(&mut g.rules).modifiers.insert(
        "target_union_movement".to_owned(),
        ModifierSpec {
            effects: BTreeMap::from([("movement".to_owned(), 1.0)]),
            collection: ModifierCollection::Player,
            ..ModifierSpec::default()
        },
    );
    g.attach_modifier_to_player(0, "target_union_movement")
        .unwrap();
    for kind in ["warrior", "archer", "horseman", "catapult"] {
        let uid = g.spawn_test_unit(kind, 0, start);
        let original_trace = g.trace_policy_reads(0);
        let expected = g.original_attack_reach_from_flood(uid);
        let old = original_trace.finish();
        let new_trace = g.trace_policy_reads(0);
        let actual = g.attack_reach_from_flood(uid);
        let new = new_trace.finish();
        assert_eq!(actual, expected);
        assert_eq!(old.effects, new.effects);
        assert_eq!(old.cards, new.cards);
        assert_eq!(old.opaque, new.opaque);
        assert!(old.is_opaque() && new.is_opaque());
        g.remove_unit(uid);
    }
}

#[test]
fn missing_nonmilitary_air_clone_and_save_load_keep_original_results() {
    let (mut g, start) = board(false);
    assert_eq!(same(&g, u32::MAX), (vec![], vec![]));
    let builder = g.spawn_test_unit("builder", 0, start);
    assert_eq!(same(&g, builder), (vec![], vec![]));
    let archer = g.spawn_test_unit("archer", 0, start);
    let bomber = g.spawn_test_unit("bomber", 0, start);
    assert!(!same(&g, bomber).0.is_empty());
    let cloned = g.clone();
    let restored: Game = serde_json::from_value(serde_json::to_value(&g).unwrap()).unwrap();
    for uid in [builder, archer, bomber] {
        assert_eq!(same(&cloned, uid), same(&restored, uid));
    }
}

// Verbatim pre-change production method, independently retaining Vec sort/dedup.
impl Game {
    fn original_attack_reach_from_flood(&self, uid: u32) -> (Vec<Pos>, Vec<Pos>) {
        let Some(unit) = self.units.get(&uid) else {
            return (vec![], vec![]);
        };
        let spec = &self.rules.units[unit.kind];
        if spec.class != "military" || (!spec.is_melee_capable() && !spec.has_ranged_attack()) {
            return (vec![], vec![]);
        }

        let attack_range = self.unit_attack_range(uid);
        if spec.domain.as_deref() == Some("air") {
            let origin = self.air_operation_origin(uid);
            let mut targets = Vec::new();
            self.map.for_each_disk(origin, attack_range, |target| {
                if target != origin {
                    targets.push(target);
                }
            });
            targets.sort_unstable();
            targets.dedup();
            return (targets, vec![]);
        }

        let start = unit.pos;
        let max_moves = self.unit_max_moves(uid);
        // `targets` is sorted below and the sensitivity cache turns `flood`
        // into a set, so neither consumer observes the ordered-map contract of
        // `flow_past`. Keep this one flood in its already-dense discovery order
        // rather than allocating an intermediate `BTreeMap<Pos, f64>` merely to
        // immediately walk it again.
        let positions = self.flow_past_unordered(uid, start, max_moves, true);
        let flood: Vec<Pos> = positions.iter().map(|(position, _)| *position).collect();
        // ⚠ THIS WAS A `BTreeSet` AND IT ALLOCATED ONE NODE PER CANDIDATE.
        // A ranged unit offers the same tile from every stride it can shoot
        // from, so the set spent most of its work absorbing duplicates a node
        // at a time, and its caller then re-collected the result into a
        // second set. A `Vec` sorted once produces the identical ascending,
        // distinct sequence this function's contract promises, from one
        // buffer, and `BasicAi`'s envelope adopts it without copying.
        let radius = attack_range.max(0) as usize;
        let disk_capacity = 1usize
            .saturating_add(
                3usize
                    .saturating_mul(radius)
                    .saturating_mul(radius.saturating_add(1)),
            )
            .min(64);
        let per_position = if spec.has_ranged_attack() {
            disk_capacity
        } else {
            6
        };
        let mut targets: Vec<Pos> =
            Vec::with_capacity(positions.len().saturating_mul(per_position));
        for (from, remaining) in positions {
            if remaining <= 0.0 {
                continue;
            }

            if spec.has_ranged_attack()
                && !self.unit_is_embarked_at(unit, from)
                && (!spec.siege || from == start || self.siege_may_attack_after_moving(unit))
            {
                self.map.for_each_disk(from, attack_range, |target| {
                    if target != from && self.unit_has_line_of_sight_from(uid, from, target) {
                        targets.push(target);
                    }
                });
            }

            if spec.is_melee_capable() {
                for target in self.nbrs(from) {
                    if !self.unit_can_melee_target_domain(uid, target)
                        || !self.unit_can_cross_cliff(uid, from, target)
                    {
                        continue;
                    }
                    let fresh = from == start && remaining >= max_moves;
                    if fresh || remaining >= self.unit_step_cost(uid, from, target) {
                        targets.push(target);
                    }
                }
            }
        }
        targets.sort_unstable();
        targets.dedup();
        (targets, flood)
    }
}
