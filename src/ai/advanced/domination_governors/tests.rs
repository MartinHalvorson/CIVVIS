use super::*;
use crate::ai::advanced::{GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::Action;

fn board() -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(2, 32, 20, 366900, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    let capital = g.found_city_for(0, (6, 8), None);
    let other = g.found_city_for(0, (12, 8), None);
    g.found_city_for(0, (18, 8), None);
    g.players[0]
        .counters
        .insert("district_governor_titles".into(), 7);
    for (governor, city, promotions) in [
        ("magnus", capital, vec!["provision", "surplus_logistics"]),
        (
            "moksha",
            other,
            vec!["grand_inquisitor", "laying_on_of_hands", "citadel_of_god"],
        ),
    ] {
        g.apply(
            0,
            &Action::AppointGovernor {
                governor: governor.into(),
                city,
            },
        )
        .unwrap();
        for promotion in promotions {
            g.apply(
                0,
                &Action::PromoteGovernor {
                    governor: governor.into(),
                    promotion: promotion.into(),
                },
            )
            .unwrap();
        }
    }
    g.players[1].religion = Some("faith_theirs".into());
    let city = g.cities.get_mut(&capital).unwrap();
    city.pressure.insert("faith_theirs".into(), 400.0);
    city.atheist_pressure = 0.0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_moksha_defends_the_faithless();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    assert!(ai.home_conversion_threat(&g, 0).is_some());
    (g, ai, plan)
}

#[test]
fn secured_defense_funds_pingala_and_both_yields_before_advanced_moksha() {
    let (mut g, ai, plan) = board();
    for titles in 8..=10 {
        g.players[0]
            .counters
            .insert("district_governor_titles".into(), titles);
        ai.strategic_governors(&mut g, 0, &plan);
        assert_eq!(g.governor_titles_available(0), 0);
        assert_eq!(g.players[0].governor_roster["moksha"].promotions.len(), 3);
    }
    let pingala = &g.players[0].governor_roster["pingala"];
    assert!(pingala.city.is_some());
    assert!(pingala.promotions.contains("researcher"));
    assert!(pingala.promotions.contains("connoisseur"));
}

#[test]
fn multiple_titles_stop_the_economic_detour_at_the_foundation() {
    let (mut g, ai, plan) = board();
    g.players[0]
        .counters
        .insert("district_governor_titles".into(), 11);
    ai.strategic_governors(&mut g, 0, &plan);
    assert_eq!(g.players[0].governor_roster["pingala"].promotions.len(), 2);
    assert!(g.players[0].governor_roster["moksha"]
        .promotions
        .contains("patron_saint"));
}

#[test]
fn unfinished_religious_defense_keeps_its_next_title() {
    let (mut g, ai, plan) = board();
    g.players[0]
        .governor_roster
        .get_mut("moksha")
        .unwrap()
        .promotions
        .remove("citadel_of_god");
    g.players[0].governor_titles_spent -= 1;
    ai.strategic_governors(&mut g, 0, &plan);
    assert!(g.players[0].governor_roster["moksha"]
        .promotions
        .contains("citadel_of_god"));
    assert!(!g.players[0].governor_roster.contains_key("pingala"));
}

#[test]
fn other_lanes_founders_and_disabled_defense_keep_their_priority() {
    let (mut g, mut ai, plan) = board();
    for target in [
        VictoryTarget::Science,
        VictoryTarget::Culture,
        VictoryTarget::Religion,
    ] {
        ai.victory_target = Some(target);
        assert_eq!(ai.governor_priority_for(&g, 0, plan.strategy)[0], "moksha");
    }
    ai.victory_target = Some(VictoryTarget::Domination);
    g.players[0].religion = Some("ours".into());
    assert_eq!(ai.governor_priority_for(&g, 0, plan.strategy)[0], "magnus");
    g.players[0].religion = None;
    ai.disable_moksha_defends_the_faithless();
    assert_eq!(ai.governor_priority_for(&g, 0, plan.strategy)[0], "magnus");
}

#[test]
fn a_temple_keeps_titles_available_for_apostle_promotions() {
    let (mut g, ai, plan) = board();
    let city = g.players[0].governor_roster["moksha"].city.unwrap();
    g.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("temple"));
    g.players[0]
        .counters
        .insert("district_governor_titles".into(), 8);
    ai.strategic_governors(&mut g, 0, &plan);
    assert!(!g.players[0].governor_roster.contains_key("pingala"));
    assert!(g.players[0].governor_roster["moksha"]
        .promotions
        .contains("patron_saint"));
}

/// `domination-seat-appoints-pingala-first`: a one-city opening with State
/// Workforce in, so one Governor title is ready.
fn opening_board(seed: u64) -> Game {
    let mut game = Game::new_full(1, 18, 10, seed, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    game.players[0]
        .civics
        .insert(crate::name!("state_workforce"));
    game
}

fn opening_plan(game: &Game, strategy: GrandStrategy) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    }
}

fn pingala_first_seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_domination_seat_appoints_pingala_first();
    }
    ai
}

fn opening_roster(game: &Game) -> Vec<String> {
    game.players[0]
        .governor_roster
        .keys()
        .map(|name| name.to_string())
        .collect()
}

#[test]
fn a_domination_opening_appoints_pingala_with_the_gene_and_magnus_without() {
    for strategy in [GrandStrategy::Expansion, GrandStrategy::Conquest] {
        let mut game = opening_board(9_201);
        let opening = opening_plan(&game, strategy);
        pingala_first_seat(VictoryTarget::Domination, true)
            .strategic_governors(&mut game, 0, &opening);
        assert_eq!(opening_roster(&game), ["pingala"], "{strategy:?}");
    }
    let mut game = opening_board(9_201);
    let opening = opening_plan(&game, GrandStrategy::Expansion);
    pingala_first_seat(VictoryTarget::Domination, false)
        .strategic_governors(&mut game, 0, &opening);
    assert_eq!(opening_roster(&game), ["magnus"]);
}

#[test]
fn pingala_first_leaves_other_targets_and_a_recovery_alone() {
    let mut game = opening_board(9_202);
    let opening = opening_plan(&game, GrandStrategy::Expansion);
    pingala_first_seat(VictoryTarget::Science, true).strategic_governors(&mut game, 0, &opening);
    assert_eq!(opening_roster(&game), ["magnus"]);

    let mut game = opening_board(9_203);
    let recovery = opening_plan(&game, GrandStrategy::Recovery);
    pingala_first_seat(VictoryTarget::Domination, true)
        .strategic_governors(&mut game, 0, &recovery);
    assert_eq!(opening_roster(&game), ["victor"]);
}

/// Two cities and no units, so a second Governor has a seat.
fn two_city_board(titles: i64) -> Game {
    let mut game = Game::new_full(2, 32, 20, 366900, 500, 0, false);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    game.found_city_for(0, (6, 8), None);
    game.found_city_for(0, (12, 8), None);
    game.players[0]
        .counters
        .insert("district_governor_titles".into(), titles);
    game
}

#[test]
fn researcher_is_the_second_title_and_magnus_the_third() {
    // Two titles: an appointment and one promotion.
    let mut game = two_city_board(2);
    let opening = opening_plan(&game, GrandStrategy::Expansion);
    let ai = pingala_first_seat(VictoryTarget::Domination, true);
    ai.strategic_governors(&mut game, 0, &opening);
    assert_eq!(game.governor_titles_available(0), 0);
    assert_eq!(opening_roster(&game), ["pingala"]);
    let pingala = &game.players[0].governor_roster["pingala"];
    assert!(pingala.city.is_some());
    assert!(
        pingala.promotions.contains("researcher") && pingala.promotions.len() == 1,
        "{:?}",
        pingala.promotions
    );
    // Researcher completes her foundation: the plan's own roster returns,
    // and the third title seats Magnus instead of her second promotion.
    assert!(!ai.domination_titles_follow_science(&game, 0, GrandStrategy::Expansion));
    game.players[0]
        .counters
        .insert("district_governor_titles".into(), 3);
    ai.strategic_governors(&mut game, 0, &opening);
    assert_eq!(game.governor_titles_available(0), 0);
    assert!(game.players[0].governor_roster.contains_key("magnus"));
    assert_eq!(
        game.players[0].governor_roster["pingala"].promotions.len(),
        1
    );
    // Without the gene the same three titles all go to Magnus.
    let mut game = two_city_board(3);
    pingala_first_seat(VictoryTarget::Domination, false)
        .strategic_governors(&mut game, 0, &opening);
    assert_eq!(opening_roster(&game), ["magnus"]);
    assert_eq!(
        game.players[0].governor_roster["magnus"].promotions.len(),
        2
    );
}

#[test]
fn a_domination_pingala_stays_seated_with_the_gene() {
    let game = opening_board(9_205);
    let off = pingala_first_seat(VictoryTarget::Domination, false);
    assert_eq!(off.culture_pingala_stays_home(&game), None);
    let on = pingala_first_seat(VictoryTarget::Domination, true);
    let (name, within) = on.culture_pingala_stays_home(&game).unwrap();
    assert_eq!(name, "pingala");
    assert!(within > 0.0);
    let science = pingala_first_seat(VictoryTarget::Science, true);
    assert_eq!(science.culture_pingala_stays_home(&game), None);
}

/// `conquest_or_score_seat`: a Score seat (an Expansion plan) seats Pingala
/// first as a Domination seat does, and takes the economic Governor after a
/// founded Moksha; a Culture, Science or Religion seat does with the gene
/// what it does without it.
#[test]
fn a_score_seat_appoints_pingala_first_and_other_targets_are_unchanged() {
    let opening = opening_plan(&opening_board(9_206), GrandStrategy::Expansion);
    for target in [VictoryTarget::Domination, VictoryTarget::Score] {
        let mut game = opening_board(9_206);
        pingala_first_seat(target, true).strategic_governors(&mut game, 0, &opening);
        assert_eq!(opening_roster(&game), ["pingala"], "{target:?}");
    }
    for target in [
        VictoryTarget::Culture,
        VictoryTarget::Science,
        VictoryTarget::Religion,
    ] {
        let mut on = opening_board(9_206);
        pingala_first_seat(target, true).strategic_governors(&mut on, 0, &opening);
        let mut off = opening_board(9_206);
        pingala_first_seat(target, false).strategic_governors(&mut off, 0, &opening);
        assert_eq!(opening_roster(&on), opening_roster(&off), "{target:?}");
    }

    let (g, mut ai, plan) = board();
    ai.victory_target = Some(VictoryTarget::Score);
    assert!(ai.domination_needs_economic_governor(&g, 0));
    assert_eq!(ai.governor_priority_for(&g, 0, plan.strategy)[0], "pingala");
    for target in [
        VictoryTarget::Culture,
        VictoryTarget::Science,
        VictoryTarget::Religion,
    ] {
        ai.victory_target = Some(target);
        assert!(!ai.domination_needs_economic_governor(&g, 0), "{target:?}");
    }
}
