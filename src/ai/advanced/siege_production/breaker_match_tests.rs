use super::super::*;
use super::tests::siege_gap_case;
use super::{BREAKER_MATCH_DEFAULT_TURNS, SHORTFALL_SIEGE_CAP};

/// The planned target walled 300 at a city strength of `defense`, none of
/// our units near it: the matched count is read at the default turns.
fn walled_case(defense: f64) -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let (mut g, mut ai, plan, home, target) = siege_gap_case();
    ai.enable_lane_delegates_production_2();
    ai.enable_siege_positive_damage_budget();
    g.cities
        .get_mut(&target)
        .unwrap()
        .buildings
        .push(crate::name!("renaissance_walls"));
    g.cities.get_mut(&target).unwrap().wall_hp = 300;
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(target, defense);
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    (g, ai, plan, home, target)
}

/// A land gun of ours `distance` tiles or more from the target.
fn far_gun(g: &mut Game, home: u32, target: u32) -> u32 {
    let pos = g.cities[&home].pos;
    assert!(g.wdist(pos, g.cities[&target].pos) > super::super::siege_train::MUSTER_BREACH_FAR);
    g.spawn_unit("catapult", 0, pos)
}

/// Off, nothing is matched and the shipped count stands.
#[test]
fn off_the_walls_ask_nothing() {
    let (g, ai, _, _, target) = walled_case(50.0);
    assert_eq!(ai.breakers_matched_to_walls(&g, 0, target), None);
    assert_eq!(ai.breakers_match_the_row(&g, 0, target, 3), 3);
}

/// Catapults (35) against a city of 50 strike ~16.5 a shot: three take
/// ~13.9 turns to breach 300 walls and take 200 health, four ~9.9, so four
/// are asked within the default twelve.
#[test]
fn weak_guns_against_heavy_walls_ask_more_than_three() {
    let (g, mut ai, _, _, target) = walled_case(50.0);
    ai.enable_breakers_match_the_walls();
    let matched = ai
        .breakers_matched_to_walls(&g, 0, target)
        .expect("a buildable catapult and standing walls");
    assert!((matched.hit - 30.0 * (-15.0_f64 / 25.0).exp()).abs() < 1e-9);
    assert_eq!(matched.target_turns, BREAKER_MATCH_DEFAULT_TURNS);
    let (guns, breach, take) = matched.ask.expect("four guns finish in time");
    assert_eq!(guns, 4);
    assert!(
        breach < take && take <= BREAKER_MATCH_DEFAULT_TURNS,
        "{breach} {take}"
    );
    assert_eq!(ai.breakers_match_the_row(&g, 0, target, 3), 4);
    assert_eq!(
        ai.breakers_match_the_row(&g, 0, target, 5),
        5,
        "never below the shipped count"
    );
}

/// Against a city of 70 a catapult strikes ~7.4: six take ~16 turns, so the
/// ask holds at the shipped count; against 100 it strikes ~2.2, under the
/// floor, and holds too (game 141).
#[test]
fn walls_six_guns_cannot_take_hold_the_shipped_count() {
    for defense in [70.0, 100.0] {
        let (g, mut ai, _, _, target) = walled_case(defense);
        ai.enable_breakers_match_the_walls();
        let matched = ai
            .breakers_matched_to_walls(&g, 0, target)
            .expect("walls stand");
        assert_eq!(matched.ask, None, "{defense}");
        assert_eq!(ai.breakers_match_the_row(&g, 0, target, 3), 3, "{defense}");
    }
}

/// No wall standing asks nothing of the gene.
#[test]
fn a_breached_city_asks_nothing() {
    let (mut g, mut ai, _, _, target) = walled_case(50.0);
    ai.enable_breakers_match_the_walls();
    g.cities.get_mut(&target).unwrap().wall_hp = 0;
    assert_eq!(ai.breakers_matched_to_walls(&g, 0, target), None);
}

/// The delegated reservation follows the matched count: with three guns
/// fielded the shipped reservation is spent, the gene orders a fourth, and
/// a fourth fielded closes it.
#[test]
fn the_reservation_follows_the_matched_count() {
    let case = |gene: bool, guns: usize| {
        let (mut g, mut ai, plan, home, target) = walled_case(50.0);
        if gene {
            ai.enable_breakers_match_the_walls();
        }
        for _ in 0..guns {
            far_gun(&mut g, home, target);
        }
        assert_eq!(ai.counts(&g, 0).siege, guns);
        let claim = ai.reserve_delegated_domination_siege(&mut g, 0, &plan);
        (claim.is_some(), g, home)
    };
    let (claimed, _, _) = case(false, SHORTFALL_SIEGE_CAP);
    assert!(!claimed, "off, three guns spend the reservation");
    let (claimed, g, home) = case(true, SHORTFALL_SIEGE_CAP);
    assert!(claimed, "on, the walls ask a fourth");
    assert!(matches!(
        g.cities[&home].queue.first(),
        Some(Item::Unit { unit }) if g.rules.units[unit].siege
    ));
    let (claimed, _, _) = case(true, 4);
    assert!(!claimed, "the fourth closes it");
}
