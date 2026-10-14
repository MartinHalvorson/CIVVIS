//! `war-target-prices-the-walls`: see the module notes.

use super::*;
use crate::ai::advanced::{ForceDomain, ForceGroup, GrandStrategy, StrategicPlan};
use crate::hex::offset_to_axial;
use std::sync::Arc;

/// Us at peace with two met rivals on open grassland, both capitals eight
/// tiles out: player 1's behind Urban Defenses (400 walls, strength 100),
/// player 2's behind Ancient Walls (100, strength 60). We field one Bombard
/// and a scout beside each capital.
fn two_capitals() -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(3, 44, 26, 113_150, 650, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let ours = g.found_city_for(0, offset_to_axial(10, 12), None);
    let walled = g.found_city_for(1, offset_to_axial(18, 12), None);
    let open = g.found_city_for(2, offset_to_axial(10, 20), None);
    assert!(g.cities[&walled].is_capital && g.cities[&open].is_capital);
    g.record_contact(0, 1);
    g.record_contact(0, 2);
    g.cities.get_mut(&walled).unwrap().wall_hp = 400;
    g.cities.get_mut(&open).unwrap().wall_hp = 100;
    let strength = Arc::make_mut(&mut g.observed_city_strength);
    strength.insert(walled, 100.0);
    strength.insert(open, 60.0);
    let home = g.cities[&ours].pos;
    g.spawn_test_unit("bombard", 0, home);
    for cid in [walled, open] {
        let beside = g
            .nbrs(g.cities[&cid].pos)
            .into_iter()
            .min()
            .expect("a neighbour");
        g.spawn_test_unit("scout", 0, beside);
    }
    g.turn = 120;
    g.current = 0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_war_target_prices_the_walls();
    (g, ai, walled, open)
}

#[test]
fn a_breachable_near_capital_beats_a_near_walled_one() {
    let (g, ai, walled, open) = two_capitals();
    let land = AdvancedAi::land_steps_from_our_cities(&g, 0);
    let guns = AdvancedAi::fielded_siege_guns(&g, 0);
    let behind = ai
        .capital_capture_estimate(&g, 0, walled, &land, guns)
        .unwrap();
    let ahead = ai
        .capital_capture_estimate(&g, 0, open, &land, guns)
        .unwrap();
    assert!(
        behind.hit >= WALL_PRICE_MIN_HIT && behind.breach > 60.0,
        "a Bombard against 400 walls at 100: {behind:?}"
    );
    assert!(ahead.turns() < 10.0, "100 walls at 60: {ahead:?}");
    assert_eq!(ai.wall_priced_war_target(&g, 0, Some(1)), Some(2));
    assert_eq!(
        ai.wall_priced_war_target(&g, 0, Some(2)),
        None,
        "the default already names it"
    );
}

#[test]
fn a_slow_breach_never_wins_even_against_an_unknown_default() {
    let (mut g, ai, _, open) = two_capitals();
    assert_eq!(ai.wall_priced_war_target(&g, 0, None), Some(2));
    g.cities.get_mut(&open).unwrap().wall_hp = 400;
    Arc::make_mut(&mut g.observed_city_strength).insert(open, 100.0);
    assert_eq!(
        ai.wall_priced_war_target(&g, 0, None),
        None,
        "Urban Defenses at a Bombard's blow ask more than {WALL_PRICE_BREACH_MAX} turns"
    );
}

#[test]
fn an_island_capital_with_no_amphibious_train_does_not_win() {
    let (mut g, mut ai, _, open) = two_capitals();
    let city = g.cities[&open].pos;
    for pos in g.wdisk(city, 2) {
        if pos != city {
            g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
        }
    }
    // A ship keeps the capital in sight.
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        if g.units[&id].kind == "scout" && g.wdist(g.units[&id].pos, city) <= 1 {
            g.remove_unit(id);
        }
    }
    let beside = g.nbrs(city).into_iter().min().unwrap();
    g.spawn_test_unit("galley", 0, beside);
    let land = AdvancedAi::land_steps_from_our_cities(&g, 0);
    let guns = AdvancedAi::fielded_siege_guns(&g, 0);
    assert!(
        AdvancedAi::seen_wall_pool(&g, 0, open).is_some(),
        "fixture: in sight"
    );
    assert_eq!(ai.capital_capture_estimate(&g, 0, open, &land, guns), None);
    assert_eq!(ai.wall_priced_war_target(&g, 0, Some(1)), None);
    ai.enable_amphibious_siege_train_2();
    let crossing = ai
        .capital_capture_estimate(&g, 0, open, &land, guns)
        .unwrap();
    assert!(crossing.march >= WALL_PRICE_CROSSING_TURNS, "{crossing:?}");
}

#[test]
fn a_victory_counter_keeps_precedence_over_the_walls() {
    let (mut g, mut ai, _, _) = two_capitals();
    // Player 1, behind 400 walls, converts the world: the religious counter.
    g.players[1].religion = Some("islam".to_string());
    for city in g.cities.values_mut() {
        city.pressure.clear();
        city.pressure.insert("islam".to_string(), 10_000.0);
    }
    ai.deny_leaders = true;
    ai.deny_while_targeted = true;
    assert_eq!(
        ai.actionable_victory_denial(&g, 0).map(|(rival, _)| rival),
        Some(1),
        "fixture: the counter names the walled rival"
    );
    assert_eq!(
        ai.wall_priced_war_target(&g, 0, Some(1)),
        Some(2),
        "the walls alone would name the open capital"
    );
    assert_eq!(ai.assess(&g, 0).target_player, Some(1));
}

#[test]
fn a_mustered_train_or_an_invested_siege_keeps_its_war() {
    let (mut g, mut ai, walled, _) = two_capitals();
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(walled),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    });
    assert_eq!(ai.wall_priced_war_target(&g, 0, Some(1)), Some(2));
    let near = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| g.wdist(*pos, g.cities[&walled].pos) == MUSTER_BREACH_FAR)
        .unwrap();
    let soldier = g.spawn_test_unit("warrior", 0, near);
    assert_eq!(
        ai.wall_priced_war_target(&g, 0, Some(1)),
        Some(2),
        "a body of ours near the objective is no train"
    );
    ai.force_groups.push(ForceGroup {
        id: soldier,
        domain: ForceDomain::Land,
        units: vec![soldier],
        anchor: near,
        objective: g.cities[&walled].pos,
        focus_target: None,
        posture: crate::ai::advanced::ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 2.0,
    });
    assert_eq!(
        ai.wall_priced_war_target(&g, 0, Some(1)),
        None,
        "a train aimed at the plan's objective, a member within the muster radius"
    );
    ai.force_groups.clear();
    ai.sieges.insert(
        walled,
        super::super::siege_train::Siege {
            stage: SiegeStage::Invest,
            taker: None,
            entered: g.turn,
            assessed: g.turn,
            posts: BTreeMap::new(),
            short_since: None,
        },
    );
    assert_eq!(
        ai.wall_priced_war_target(&g, 0, Some(1)),
        None,
        "an invested siege"
    );
}

#[test]
fn off_the_gene_changes_nothing() {
    let (g, mut ai, _, _) = two_capitals();
    ai.disable_war_target_prices_the_walls();
    assert_eq!(ai.wall_priced_war_target(&g, 0, Some(1)), None);
    let off = ai.assess(&g, 0).target_player;
    ai.enable_war_target_prices_the_walls();
    let on = ai.assess(&g, 0).target_player;
    assert!(
        off == on || on == Some(2),
        "the gene moves the plan only to the breachable capital: {off:?} -> {on:?}"
    );
}

/// The gene is a native opt-in, off in both controllers.
#[test]
fn war_target_prices_the_walls_is_a_native_opt_in_off_in_both_controllers() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "war-target-prices-the-walls",
        |ai| ai.war_target_prices_the_walls,
    );
}
