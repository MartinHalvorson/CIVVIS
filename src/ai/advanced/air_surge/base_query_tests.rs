//! Sparse aircraft-base candidates retain the full-map oracle and range rules.

use super::*;
use std::sync::Arc;

fn board() -> (Game, u32, u32) {
    let mut g = Game::new_full(2, 48, 24, 5268, 100, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.clear_mirror_cities();
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.improvement_owner = None;
        tile.district = None;
        tile.owner_city = None;
        tile.pillaged = false;
    }
    let own = g.found_city_for(0, (12, 10), None);
    let foreign = g.found_city_for(1, (24, 10), None);
    (g, own, foreign)
}

/// The original complete map scan is an independent oracle, not a second
/// implementation of the candidate rules.
fn scanned_bases(g: &Game, pid: usize) -> Vec<Pos> {
    g.map
        .tiles
        .keys()
        .copied()
        .filter(|position| g.air_capacity_at(pid, *position) > 0)
        .collect()
}

fn same_bases(g: &Game) {
    for pid in 0..g.players.len() {
        assert_eq!(
            AdvancedAi::air_surge_bases(g, pid),
            scanned_bases(g, pid),
            "player {pid}"
        );
    }
}

fn district(g: &mut Game, cid: u32, pos: Pos, kind: Name) {
    // Do not populate City::districts: mirrored tile state can arrive before
    // its city roster, and the engine's capacity query reads the tile itself.
    let tile = g.map.tiles.get_mut(&pos).unwrap();
    tile.owner_city = Some(cid);
    tile.district = Some(kind);
}

#[test]
fn centers_and_ordinary_units_match_the_original_sorted_scan() {
    let (mut g, _, _) = board();
    for owner in [0, 1] {
        g.spawn_test_unit("warrior", owner, (14, 10));
        g.spawn_test_unit("bomber", owner, (16, 10));
    }
    same_bases(&g);
    assert_eq!(AdvancedAi::air_surge_bases(&g, 0), vec![(12, 10)]);
    assert_eq!(AdvancedAi::air_surge_bases(&g, 1), vec![(24, 10)]);
}

#[test]
fn tile_districts_keep_replacements_pillage_and_incomplete_city_rosters() {
    let (mut g, own, foreign) = board();
    let kinds: Vec<_> = g.rules.districts.keys().copied().collect();
    for kind in kinds {
        for cid in [own, foreign] {
            for pillaged in [false, true] {
                district(&mut g, cid, (14, 10), kind);
                g.map.tiles.get_mut(&(14, 10)).unwrap().pillaged = pillaged;
                same_bases(&g);
            }
        }
    }
    let mut replacement = g.rules.districts["aerodrome"].clone();
    replacement.replaces = Some(crate::name!("aerodrome"));
    replacement.air_slots = 5;
    Arc::make_mut(&mut g.rules)
        .districts
        .insert("query_custom_field".to_string(), replacement);
    district(&mut g, own, (14, 10), crate::name!("query_custom_field"));
    g.map.tiles.get_mut(&(14, 10)).unwrap().pillaged = false;
    same_bases(&g);
    assert!(AdvancedAi::air_surge_bases(&g, 0).contains(&(14, 10)));
}

#[test]
fn neutral_airstrips_and_stale_city_owners_keep_engine_ownership_rules() {
    let (mut g, own, foreign) = board();
    for cid in [None, Some(own), Some(foreign), Some(u32::MAX)] {
        for owner in [None, Some(0), Some(1)] {
            for pillaged in [false, true] {
                let tile = g.map.tiles.get_mut(&(14, 10)).unwrap();
                tile.improvement = Some(crate::name!("airstrip"));
                tile.owner_city = cid;
                tile.improvement_owner = owner;
                tile.pillaged = pillaged;
                same_bases(&g);
            }
        }
    }
}

#[test]
fn carriers_overlap_existing_bases_and_move_without_retaining_an_answer() {
    let (mut g, own, _) = board();
    district(&mut g, own, (14, 10), crate::name!("aerodrome"));
    let carrier = g.spawn_test_unit("aircraft_carrier", 0, (14, 10));
    g.spawn_test_unit("aircraft_carrier", 0, (12, 10));
    g.spawn_test_unit("aircraft_carrier", 1, (16, 10));
    for (owner, pos) in [(0, (14, 10)), (0, (18, 10)), (1, (18, 10))] {
        let unit = g.units.get_mut(&carrier).unwrap();
        unit.owner = owner;
        unit.pos = pos;
        same_bases(&g);
    }
    g.remove_unit(carrier);
    same_bases(&g);
    let bases = AdvancedAi::air_surge_bases(&g, 0);
    assert_eq!(bases.iter().filter(|p| **p == (12, 10)).count(), 1);
}

#[test]
fn coordinates_absent_from_the_map_do_not_become_aircraft_bases() {
    let (mut g, own, _) = board();
    g.cities.get_mut(&own).unwrap().pos = (-1000, -1000);
    g.spawn_test_unit("aircraft_carrier", 0, (-999, -999));
    same_bases(&g);
    assert!(AdvancedAi::air_surge_base_candidates(&g, 0)
        .iter()
        .all(|pos| g.map.tiles.contains_key(pos)));
}

fn scanned_in_range(g: &Game, pid: usize, objective: Pos) -> bool {
    let bases = scanned_bases(g, pid);
    let reaches = |kind: Name| {
        bases
            .iter()
            .any(|base| g.wdist(*base, objective) <= g.rules.units[kind].range)
    };
    let wing: Vec<_> = g
        .units
        .values()
        .filter(|unit| unit.owner == pid && AdvancedAi::air_surge_is_bomber(g, unit.kind))
        .collect();
    if wing.len() >= AIR_SURGE_LAUNCH_BOMBERS {
        return wing.iter().filter(|unit| reaches(unit.kind)).count() >= AIR_SURGE_LAUNCH_BOMBERS;
    }
    let Some(bomber) = AdvancedAi::air_surge_bomber(g, pid) else {
        return false;
    };
    reaches(bomber)
}

#[test]
fn future_and_mixed_generation_wings_keep_every_range_decision() {
    let (mut g, own, _) = board();
    district(&mut g, own, (14, 10), crate::name!("aerodrome"));
    assert!(!g.players[0]
        .techs
        .contains(&crate::name!("advanced_flight")));
    assert!(AdvancedAi::air_surge_bomber(&g, 0).is_some());
    let mut wing = Vec::new();
    for generation in 0..4 {
        match generation {
            0 => {}
            1 => wing.push(g.spawn_test_unit("bomber", 0, (12, 10))),
            2 => wing.push(g.spawn_test_unit("jet_bomber", 0, (12, 10))),
            _ => g.units.get_mut(&wing[0]).unwrap().kind = crate::name!("jet_bomber"),
        }
        for objective in [(12, 10), (14, 10), (22, 10), (24, 10), (26, 10), (35, 15)] {
            assert_eq!(
                AdvancedAi::air_surge_in_range(&g, 0, objective),
                scanned_in_range(&g, 0, objective),
                "generation {generation}, objective {objective:?}"
            );
        }
    }
}

#[test]
fn cloning_and_save_restore_do_not_retain_base_answers() {
    let (mut g, own, _) = board();
    district(&mut g, own, (14, 10), crate::name!("aerodrome"));
    g.spawn_test_unit("aircraft_carrier", 0, (18, 10));
    same_bases(&g);
    let mut cloned = g.clone();
    cloned.map.tiles.get_mut(&(14, 10)).unwrap().pillaged = true;
    same_bases(&cloned);
    same_bases(&g);
    let restored: Game = serde_json::from_value(serde_json::to_value(&cloned).unwrap()).unwrap();
    same_bases(&restored);
    assert_eq!(
        AdvancedAi::air_surge_bases(&cloned, 0),
        AdvancedAi::air_surge_bases(&restored, 0)
    );
}

#[test]
fn ordinary_empty_tiles_never_need_a_full_capacity_query() {
    let (mut g, own, _) = board();
    district(&mut g, own, (14, 10), crate::name!("aerodrome"));
    let tile = g.map.tiles.get_mut(&(16, 10)).unwrap();
    tile.improvement = Some(crate::name!("airstrip"));
    tile.improvement_owner = Some(0);
    g.spawn_test_unit("aircraft_carrier", 0, (18, 10));
    let candidates = AdvancedAi::air_surge_base_candidates(&g, 0);
    assert!(
        candidates.len() < 16,
        "fixture has only a handful of possible bases"
    );
    assert!(candidates.len() * 20 < g.map.tiles.len());
    for pos in g.map.tiles.keys() {
        if g.air_capacity_at(0, *pos) > 0 {
            assert!(
                candidates.contains(pos),
                "a positive capacity must be queried: {pos:?}"
            );
        }
    }
    same_bases(&g);
}
