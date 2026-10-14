//! `siege-budget-projects-the-seal`: a city whose ring is sealed does not
//! heal (`Game::city_under_siege`), so on the siege's entry reads a train
//! with the melee bodies to seal the ring prices the city at no heal. Live
//! Immortal Jenne (civvis-20261013T184508Z) held turns 111-117 with seven to
//! nine units at 23.7-65.0 turns, its ring 0/6 sealed.

use super::tests::{at_distance, ring_of, walled_city};
use super::*;

/// `walled_city` on flat dry grassland with no river near, at war.
fn open_ground() -> (Game, u32) {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    for pos in g.wdisk(city, 4) {
        if let Some(tile) = g.map.tiles.get_mut(&pos) {
            if pos != city {
                tile.terrain = crate::name!("grassland");
                tile.feature = None;
                tile.hills = false;
            }
            tile.river_edges = [false; 6];
        }
    }
    g.at_war.insert((0, 1));
    (g, cid)
}

/// `open_ground` with no wall pool and a City Center of 45, so one
/// Crossbowman's shot (23 after the district penalty) falls short of the
/// open ring's heal of 20 and the shipped budget reads it infinite.
fn open_city() -> (Game, u32) {
    let (mut g, cid) = open_ground();
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, 0);
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(cid, 45.0);
    std::sync::Arc::make_mut(&mut g.observed_city_ranged_strength).insert(cid, 40.0);
    (g, cid)
}

fn budget_reader(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_siege_budget_counts_what_fires();
    ai.enable_siege_positive_damage_budget();
    if gene {
        ai.enable_siege_budget_projects_the_seal();
    }
    ai
}

/// `count` Men-at-Arms three tiles out, off the ring.
fn melee(g: &mut Game, cid: u32, count: usize) -> Vec<u32> {
    let spots = at_distance(g, cid, 3);
    (0..count)
        .map(|i| g.spawn_unit("man_at_arms", 0, spots[i * 3]))
        .collect()
}

#[test]
fn two_bodies_seal_an_open_six_tile_ring() {
    let (mut g, cid) = open_ground();
    assert_eq!(ring_of(&g, cid).len(), 6, "fixture: six dry sides");
    assert_eq!(seal_bodies_needed(&g, 0, cid), Some(2));
    // One of ours already on the ring covers itself and both neighbours.
    let ring = ring_of(&g, cid);
    g.spawn_unit("man_at_arms", 0, ring[0]);
    assert_eq!(seal_bodies_needed(&g, 0, cid), Some(1));
}

#[test]
fn a_water_side_cannot_be_sealed_from_land() {
    let (mut g, cid) = open_ground();
    let side = ring_of(&g, cid)[0];
    g.map.tiles.get_mut(&side).unwrap().terrain = crate::name!("coast");
    assert_eq!(seal_bodies_needed(&g, 0, cid), None);
}

#[test]
fn a_river_between_sides_asks_another_body() {
    let (mut g, cid) = open_ground();
    let city = g.cities[&cid].pos;
    // A river on every edge between neighbouring ring tiles: each body
    // covers only its own side.
    let ring = ring_of(&g, cid);
    for a in &ring {
        for b in &ring {
            if a < b && g.wdist(*a, *b) == 1 {
                assert!(g.map.set_river_edge(*a, *b, true));
            }
        }
    }
    assert!(g.wdist(ring[0], city) == 1);
    assert_eq!(seal_bodies_needed(&g, 0, cid), Some(6));
}

#[test]
fn a_seal_capable_train_prices_the_city_at_no_heal() {
    let (mut g, cid) = open_city();
    let mut force = vec![g.spawn_unit("crossbowman", 0, at_distance(&g, cid, 2)[0])];
    force.extend(melee(&mut g, cid, 2));
    let off = budget_reader(false);
    let shipped = off
        .conversion_siege_entry_budget(&g, 0, cid, &force, 5)
        .unwrap();
    assert!(
        shipped.0.is_infinite(),
        "fixture: the open ring's heal out-heals the shot: {shipped:?}"
    );
    let on = budget_reader(true);
    assert!(on.train_seals_the_ring(&g, 0, cid, &force, 5));
    let (turns, endurance) = on
        .conversion_siege_entry_budget(&g, 0, cid, &force, 5)
        .unwrap();
    assert!(
        turns.is_finite(),
        "no heal: the shot takes the city in time: {turns}"
    );
    assert_eq!(
        endurance, shipped.1,
        "the seal changes the heal, not the endurance"
    );
    assert_eq!(
        on.conversion_siege_budget_within(&g, 0, cid, &force, 5),
        off.conversion_siege_budget_within(&g, 0, cid, &force, 5),
        "the production reads keep the shipped heal"
    );
}

#[test]
fn too_few_bodies_keep_the_shipped_heal() {
    let (mut g, cid) = open_city();
    let mut force = vec![g.spawn_unit("crossbowman", 0, at_distance(&g, cid, 2)[0])];
    force.extend(melee(&mut g, cid, 1));
    let on = budget_reader(true);
    assert!(
        !on.train_seals_the_ring(&g, 0, cid, &force, 5),
        "one body seals two sides of six"
    );
    assert_eq!(
        on.conversion_siege_entry_budget(&g, 0, cid, &force, 5),
        budget_reader(false).conversion_siege_entry_budget(&g, 0, cid, &force, 5)
    );
}

#[test]
fn a_wounded_body_does_not_count_toward_the_seal() {
    let (mut g, cid) = open_city();
    g.tactics.heal = true;
    let force = melee(&mut g, cid, 2);
    let on = budget_reader(true);
    assert!(on.train_seals_the_ring(&g, 0, cid, &force, 5));
    g.units.get_mut(&force[1]).unwrap().hp = super::super::battle_planner::ROTATE_HP - 1;
    assert!(
        !on.train_seals_the_ring(&g, 0, cid, &force, 5),
        "a member rotating out holds no side"
    );
}

#[test]
fn a_walled_target_with_a_breaker_prices_its_city_phase_at_no_heal() {
    let (mut g, cid) = open_ground();
    assert!(g.cities[&cid].wall_hp > 0, "fixture: walls stand");
    let mut force: Vec<u32> = at_distance(&g, cid, 2)
        .into_iter()
        .take(2)
        .map(|pos| g.spawn_unit("catapult", 0, pos))
        .collect();
    force.extend(melee(&mut g, cid, 2));
    let (shipped, _) = budget_reader(false)
        .conversion_siege_entry_budget(&g, 0, cid, &force, 5)
        .unwrap();
    let (turns, _) = budget_reader(true)
        .conversion_siege_entry_budget(&g, 0, cid, &force, 5)
        .unwrap();
    assert!(turns.is_finite(), "{turns}");
    assert!(
        turns < shipped,
        "the city phase loses its heal: {turns} against {shipped}"
    );
}

/// The gene is a native opt-in, off in both controllers.
#[test]
fn siege_budget_projects_the_seal_is_a_native_opt_in_off_in_both_controllers() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "siege-budget-projects-the-seal",
        |ai| ai.siege_budget_projects_the_seal,
    );
}
