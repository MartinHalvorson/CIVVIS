use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, StrategicPlan};
use std::sync::Arc;

/// 155202Z's shape on open grassland: player 1's original capital is ours
/// and we fight its remnant town; player 3's original capital is ours too;
/// player 2, at peace and in reach, holds the last original capital and has
/// launched its Exoplanet Expedition. Returns the board, the controller and
/// player 2's capital.
fn launched() -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(4, 36, 22, 91_120, 500, 0, false);
    let ids: Vec<_> = g.units.keys().copied().collect();
    for id in ids {
        g.remove_unit(id);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.found_city_for(0, (0, 8), None);
    let beaten = g.found_city_for(0, (5, 8), None);
    {
        let city = g.cities.get_mut(&beaten).unwrap();
        city.is_capital = true;
        city.original_owner = 1;
        city.loyalty = 100.0;
    }
    let town = g.found_city_for(1, (8, 8), None);
    g.cities.get_mut(&town).unwrap().is_capital = false;
    let racer_capital = g.found_city_for(2, (16, 8), None);
    g.found_city_for(2, (21, 8), None);
    let third = g.found_city_for(3, (25, 16), None);
    {
        let city = g.cities.get_mut(&third).unwrap();
        city.owner = 0;
        city.original_owner = 3;
        city.is_capital = true;
        city.loyalty = 100.0;
    }
    g.players[3].alive = true;
    for other in [1, 2, 3] {
        g.record_contact(0, other);
    }
    for _ in 0..4 {
        g.spawn_test_unit("modern_armor", 0, (5, 8));
    }
    for _ in 0..2 {
        g.spawn_test_unit("modern_armor", 2, (18, 8));
    }
    Arc::make_mut(&mut g.observed_military_power).insert(0, 4000.0);
    for project in [
        "launch_earth_satellite",
        "launch_moon_landing",
        "launch_mars_colony",
        "exoplanet_expedition",
    ] {
        g.players[2].science_projects.insert(project.to_string());
    }
    g.players[0].gold = 3000.0;
    g.at_war.insert((0, 1));
    g.turn = 250;
    g.current = 0;
    let mut ai = AdvancedAi::new();
    ai.retarget(VictoryTarget::Domination);
    ai.enable_one_war_at_a_time();
    ai.coalition_before_war = false;
    ai.coalition_before_war_2 = false;
    ai.coalition_before_war_3 = false;
    ai.one_war_observe(&g, 0);
    (g, ai, racer_capital)
}

fn aimed_at(capital: u32, g: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(2),
        target_city: Some(capital),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    }
}

/// Our treasury below the war reserve and falling, as at 155202Z t217.
fn broke(g: &mut Game) {
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = -40.0;
}

#[test]
fn launched_racer_yields_the_capital_is_opt_in() {
    opt_in_off_in_both_controllers("launched-racer-yields-the-capital", |ai| {
        ai.launched_racer_yields_the_capital
    });
}

#[test]
fn the_launched_racer_holding_the_finishing_capital_is_read() {
    let (mut g, mut ai, capital) = launched();
    assert!(
        AdvancedAi::capture_completes_domination(&g, 0, capital),
        "fixture: the racer's capital is the last one Domination needs"
    );
    assert_eq!(ai.launched_racer_finish(&g, 0), None, "gene off");
    ai.enable_launched_racer_yields_the_capital();
    assert_eq!(ai.launched_racer_finish(&g, 0), Some((2, capital)));

    // Not launched: the pads still matter, and the launcher war answers it.
    let mut unlaunched = g.clone();
    unlaunched.players[2]
        .science_projects
        .remove("exoplanet_expedition");
    assert_eq!(
        ai.launched_racer_finish(&unlaunched, 0),
        None,
        "not launched"
    );

    // Its capital is not the finishing one: another capital still stands.
    let third = g
        .cities
        .values()
        .find(|city| city.original_owner == 3)
        .unwrap()
        .id;
    g.cities.get_mut(&third).unwrap().owner = 3;
    assert!(!AdvancedAi::capture_completes_domination(&g, 0, capital));
    assert_eq!(
        ai.launched_racer_finish(&g, 0),
        None,
        "not the finishing capital"
    );
}

#[test]
fn the_launched_racer_is_the_second_front_before_the_front_refuses() {
    let (g, mut ai, _) = launched();
    assert!(
        ai.one_war
            .as_ref()
            .and_then(|front| front.closure_wanted_since)
            .is_none_or(|since| since == g.turn),
        "fixture: the front's peace is offered this turn at most, not yet refused"
    );
    assert_ne!(
        ai.one_war_second_front(&g, 0),
        Some(2),
        "gene off: the refusal gate holds it"
    );
    ai.enable_launched_racer_yields_the_capital();
    assert_eq!(ai.one_war_second_front(&g, 0), Some(2));
}

#[test]
fn the_finishing_capital_is_the_objective_at_peace() {
    let (g, mut ai, capital) = launched();
    let off = ai.assess(&g, 0);
    assert_ne!(
        off.target_city,
        Some(capital),
        "gene off: the capital waits for a war"
    );
    ai.enable_launched_racer_yields_the_capital();
    let on = ai.assess(&g, 0);
    assert_eq!(on.target_player, Some(2));
    assert_eq!(on.target_city, Some(capital));
}

#[test]
fn the_declaration_skips_the_treasury_and_the_staging_holds() {
    for gene in [false, true] {
        let (mut g, mut ai, capital) = launched();
        ai.enable_war_needs_a_treasury();
        broke(&mut g);
        assert!(
            !ai.war_is_affordable(&g, 0),
            "fixture: the treasury holds an ordinary war"
        );
        if gene {
            ai.enable_launched_racer_yields_the_capital();
        }
        let plan = aimed_at(capital, &g);
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert_eq!(
            g.is_at_war(0, 2),
            gene,
            "gene {gene}: the launched racer is declared on through the holds"
        );
        assert!(g.is_at_war(0, 1), "the running war is kept");
    }
    // Not launched: the same board holds, gene on.
    let (mut g, mut ai, capital) = launched();
    ai.enable_war_needs_a_treasury();
    ai.enable_launched_racer_yields_the_capital();
    broke(&mut g);
    g.players[2].science_projects.remove("exoplanet_expedition");
    let plan = aimed_at(capital, &g);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        !g.is_at_war(0, 2),
        "not launched: no declaration through the holds"
    );
}

#[test]
fn a_treasury_hold_says_so() {
    let (mut g, mut ai, capital) = launched();
    g.at_war.clear();
    ai.one_war_observe(&g, 0);
    ai.enable_war_needs_a_treasury();
    broke(&mut g);
    ai.base.journal = crate::reasoning::Journal::recording();
    let headline = format!("Holding the war on {}", g.players[2].civ);
    let lines = |ai: &AdvancedAi| -> Vec<String> {
        ai.base
            .journal
            .since(0)
            .thoughts
            .iter()
            .filter(|thought| thought.headline == headline)
            .map(|thought| thought.detail.clone())
            .collect()
    };
    let plan = aimed_at(capital, &g);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 2), "held by the treasury");
    let held = lines(&ai);
    assert_eq!(held.len(), 1, "journalled once: {held:?}");
    let reserve = 100 + 25 * g.player_city_ids(0).len();
    assert_eq!(
        held[0],
        format!("the treasury reserve: 0 gold of {reserve} needed, at -40.0 a turn")
    );

    // A treasury that carries the war leaves no such line.
    let (mut g, mut ai, capital) = launched();
    g.at_war.clear();
    ai.one_war_observe(&g, 0);
    ai.enable_war_needs_a_treasury();
    ai.base.journal = crate::reasoning::Journal::recording();
    let plan = aimed_at(capital, &g);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        lines(&ai).is_empty(),
        "no treasury line when the war is affordable"
    );
}
