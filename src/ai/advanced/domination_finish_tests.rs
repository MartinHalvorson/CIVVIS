use super::*;
use crate::{game::Action, name};

fn at(col: i32, row: i32) -> Pos {
    crate::hex::offset_to_axial(col, row)
}

fn board() -> (Game, u32, u32) {
    let mut g = Game::new_full(3, 40, 24, 109_106_000, 300, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
    }
    let home = g.found_city_for(0, at(4, 8), None);
    let target = g.found_city_for(1, at(24, 8), None);
    let third = g.found_city_for(2, at(4, 18), None);
    g.cities.get_mut(&third).unwrap().owner = 0;
    let pressure = g.found_city_for(1, at(28, 8), None);
    g.cities.get_mut(&pressure).unwrap().pop = 40;
    g.cities.get_mut(&target).unwrap().pop = 1;
    g.cities.get_mut(&target).unwrap().hp = 0;
    g.cities.get_mut(&target).unwrap().wall_hp = 0;
    g.current = 0;
    g.turn = 90;
    g.at_war.insert((0, 1));
    g.record_contact(0, 1);
    g.record_contact(0, 2);
    assert!(AdvancedAi::population_loyalty_delta_with_capture(&g, 0, target, true) <= -12.5);
    (g, home, target)
}

/// Exercise the actual capture/keep path, so a mirrored AI predicate cannot
/// claim a win merely because its own assertions agree with one another.
fn take(mut g: Game, target: u32) -> Game {
    let pos = g.cities[&target].pos;
    let unit = g.spawn_test_unit("giant_death_robot", 0, at(23, 8));
    g.apply(0, &Action::Attack { unit, target: pos }).unwrap();
    assert_eq!(g.cities[&target].owner, 0);
    g.apply(0, &Action::KeepCity { city: target }).unwrap();
    g
}

#[test]
fn a_lost_home_capital_preserves_occupation_safety_on_the_last_foreign_capital() {
    let (mut g, home, target) = board();
    assert!(AdvancedAi::capture_completes_domination(&g, 0, target));
    assert!(!AdvancedAi::should_defer_city_capture(&g, 0, target));
    assert_eq!(take(g.clone(), target).winner, Some(0));

    g.cities.get_mut(&home).unwrap().owner = 2;
    assert!(!AdvancedAi::capture_completes_domination(&g, 0, target));
    assert!(AdvancedAi::should_defer_city_capture(&g, 0, target));
    assert_eq!(take(g, target).winner, None);
}

#[test]
fn disabled_domination_cannot_waive_occupation_safety() {
    let (mut g, _, target) = board();
    g.victory_conditions.domination = false;
    assert!(!AdvancedAi::capture_completes_domination(&g, 0, target));
    assert!(AdvancedAi::should_defer_city_capture(&g, 0, target));
    assert_eq!(take(g, target).winner, None);
}

#[test]
fn a_domination_milestone_only_waives_safety_when_it_completes_require_n() {
    let (mut g, _, target) = board();
    g.required_victory_types = 2;
    assert!(!AdvancedAi::capture_completes_domination(&g, 0, target));
    assert!(AdvancedAi::should_defer_city_capture(&g, 0, target));
    let banked = take(g.clone(), target);
    assert_eq!(banked.winner, None);
    assert!(banked.victories_won[&0].contains("domination"));

    g.victories_won
        .entry(0)
        .or_default()
        .insert("science".into());
    assert!(AdvancedAi::capture_completes_domination(&g, 0, target));
    assert!(!AdvancedAi::should_defer_city_capture(&g, 0, target));
    assert_eq!(take(g, target).winner, Some(0));
}

#[test]
fn an_unfounded_living_major_is_still_a_remaining_domination_opponent() {
    let (mut g, _, target) = board();
    let third = g
        .cities
        .values()
        .find(|city| city.original_owner == 2)
        .unwrap()
        .id;
    g.cities.remove(&third);
    g.players[2].alive = true;
    assert!(!AdvancedAi::capture_completes_domination(&g, 0, target));
    assert!(AdvancedAi::should_defer_city_capture(&g, 0, target));
    g.players[2].alive = false;
    assert!(AdvancedAi::capture_completes_domination(&g, 0, target));
}

#[test]
fn team_completion_keeps_its_own_capitals_and_only_needs_opponents_to_lose_theirs() {
    let (mut g, _, target) = board();
    g.players[0].team = Some(1);
    g.players[2].team = Some(1);
    let third = g
        .cities
        .values()
        .find(|city| city.original_owner == 2)
        .unwrap()
        .id;
    g.cities.get_mut(&third).unwrap().owner = 2;
    assert!(AdvancedAi::capture_completes_domination(&g, 0, target));
    assert_eq!(take(g.clone(), target).winner, Some(0));
    g.cities.get_mut(&third).unwrap().owner = 1;
    assert!(!AdvancedAi::capture_completes_domination(&g, 0, target));
    assert!(AdvancedAi::should_defer_city_capture(&g, 0, target));
    assert_eq!(take(g, target).winner, None);
}

#[test]
fn team_require_n_uses_the_engines_first_qualifying_member() {
    let (mut g, _, target) = board();
    g.players[0].team = Some(1);
    g.players[2].team = Some(1);
    let third = g
        .cities
        .values()
        .find(|city| city.original_owner == 2)
        .unwrap()
        .id;
    g.cities.get_mut(&third).unwrap().owner = 2;
    g.required_victory_types = 2;
    g.victories_won
        .entry(2)
        .or_default()
        .insert("science".into());
    assert!(!AdvancedAi::capture_completes_domination(&g, 0, target));
    assert_eq!(take(g.clone(), target).winner, None);
    g.victories_won
        .entry(0)
        .or_default()
        .insert("science".into());
    assert!(AdvancedAi::capture_completes_domination(&g, 0, target));
    assert!(AdvancedAi::capture_completes_domination(&g, 2, target));
    let result = take(g, target);
    assert_eq!(result.winner, Some(0));
    assert!(result.winning_players().contains(&2));
}

#[test]
fn a_diplomatic_match_clock_routes_the_army_to_its_last_required_capital() {
    let (mut g, home, capital) = board();
    // The old front has already yielded its original capital, but still has
    // a town and an active war. This is the live failure shape: the ordinary
    // denial response switches to Diplomacy while the army mops up that town.
    let old_front_town = g.found_city_for(2, at(14, 18), None);
    g.cities.get_mut(&old_front_town).unwrap().is_capital = false;
    g.at_war.remove(&(0, 1));
    g.at_war.insert((0, 2));
    g.players[1].dvp = 16;

    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_deny_while_targeted();
    ai.enable_stock_denial_lead_time();
    ai.enable_denial_outranks_expansion();
    assert_eq!(
        ai.actionable_victory_denial(&g, 0),
        Some((1, GrandStrategy::Diplomacy)),
        "the public 16-point clock must be the actual competing plan"
    );
    assert_eq!(ai.domination_finishing_capital_for(&g, 0, 1), Some(capital));
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.strategy, GrandStrategy::Conquest);
    assert_eq!(plan.target_player, Some(1));
    assert_eq!(g.cities[&plan.target_city.unwrap()].owner, 1);

    let mut forced = ai.clone();
    forced.forced_target_player = Some(2);
    assert_eq!(forced.assess(&g, 0).target_player, Some(2));

    let mut one_war = ai.clone();
    one_war.enable_one_war_at_a_time();
    one_war.one_war_observe(&g, 0);
    assert_eq!(one_war.assess(&g, 0).target_player, Some(2));

    let mut threatened = g.clone();
    threatened.spawn_test_unit("giant_death_robot", 2, at(5, 8));
    let defensive = ai.assess(&threatened, 0);
    assert_eq!(defensive.strategy, GrandStrategy::Recovery);
    assert_eq!(defensive.target_player, Some(2));

    g.cities.get_mut(&home).unwrap().owner = 2;
    assert_eq!(ai.domination_finishing_capital_for(&g, 0, 1), None);
    assert_eq!(ai.assess(&g, 0).strategy, GrandStrategy::Diplomacy);
}

/// See `finishing_capital_in_reach`: the capital whose capture completes
/// Domination is the first objective at twice the first-capture march when
/// we outgun its owner twice over, ahead of a nearer walled town.
#[test]
fn the_finishing_capital_is_the_first_objective_when_we_outgun_its_owner() {
    let build = |strong: bool| {
        let mut g = Game::new_full(3, 40, 24, 109_106_001, 300, 0, false);
        for id in g.units.keys().copied().collect::<Vec<_>>() {
            g.remove_unit(id);
        }
        for tile in g.map.tiles.values_mut() {
            tile.terrain = name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
        }
        g.found_city_for(0, at(4, 8), None);
        let third = g.found_city_for(2, at(4, 18), None);
        g.cities.get_mut(&third).unwrap().owner = 0;
        let capital = g.found_city_for(1, at(17, 8), None);
        let town = g.found_city_for(1, at(10, 8), None);
        for cid in [capital, town] {
            g.cities.get_mut(&cid).unwrap().wall_hp = 100;
        }
        g.record_contact(0, 1);
        g.record_contact(0, 2);
        g.current = 0;
        g.turn = 150;
        // Strong: eight armies against a warrior. Weak: two against six.
        let (ours, theirs) = if strong { (8, 0) } else { (2, 6) };
        for y in 0..ours {
            g.spawn_test_unit("modern_armor", 0, at(5, 6 + y % 6));
        }
        for y in 0..theirs {
            g.spawn_test_unit("modern_armor", 1, at(18, 6 + y % 6));
        }
        g.spawn_test_unit("warrior", 1, at(17, 9));
        (g, capital, town)
    };
    for strong in [false, true] {
        let (g, capital, town) = build(strong);
        assert!(AdvancedAi::capture_completes_domination(&g, 0, capital));
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        let plan = ai.assess(&g, 0);
        assert_eq!(plan.target_player, Some(1));
        assert_eq!(
            plan.target_city,
            Some(if strong { capital } else { town }),
            "strong {strong}"
        );
    }
}

/// `domination-finish-holds-the-front`: at war with the owner of the capital
/// that completes Domination, a committed objective at full health yields to
/// that capital; one the siege has damaged keeps the army. Live King
/// civvis-20261005T141932Z (game 135) stayed committed to three other cities
/// for 18 turns beside Madrid, the last capital it needed.
#[test]
fn the_finishing_capital_holds_the_front_and_outranks_an_untouched_commitment() {
    let mut g = Game::new_full(3, 40, 24, 109_106_001, 300, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
    }
    g.found_city_for(0, at(4, 8), None);
    let third = g.found_city_for(2, at(4, 18), None);
    g.cities.get_mut(&third).unwrap().owner = 0;
    let capital = g.found_city_for(1, at(17, 8), None);
    let town = g.found_city_for(1, at(10, 8), None);
    for cid in [capital, town] {
        g.cities.get_mut(&cid).unwrap().wall_hp = 100;
    }
    g.record_contact(0, 1);
    g.record_contact(0, 2);
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 150;
    for y in 0..8 {
        g.spawn_test_unit("modern_armor", 0, at(5, 6 + y % 6));
    }
    g.spawn_test_unit("warrior", 1, at(17, 9));
    assert!(AdvancedAi::capture_completes_domination(&g, 0, capital));
    let committed = |ai: &mut AdvancedAi, g: &Game| {
        ai.plan = Some(StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(town),
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: g.turn,
            rush: false,
        });
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert_eq!(ai.domination_finish_front(&g, 0), None, "off");
    ai.enable_siege_commitment();
    committed(&mut ai, &g);
    assert_eq!(
        ai.assess(&g, 0).target_city,
        Some(town),
        "off: the commitment holds"
    );
    ai.enable_domination_finish_holds_the_front();
    assert_eq!(ai.domination_finish_front(&g, 0), Some((1, capital)));
    committed(&mut ai, &g);
    assert_eq!(
        ai.assess(&g, 0).target_city,
        Some(capital),
        "the finish outranks it"
    );
    // A siege that has the town below full health finishes it first.
    g.cities.get_mut(&town).unwrap().hp = 120;
    committed(&mut ai, &g);
    assert_eq!(
        ai.assess(&g, 0).target_city,
        Some(town),
        "a damaged siege finishes"
    );
}

/// See `domination_finish_at_war`: at war with both the owner of the last
/// capital Domination needs and an old front, the campaign goes for that
/// capital, whoever the denial layer names (Maori, game 65).
#[test]
fn the_last_capital_is_the_objective_when_its_owner_is_already_at_war() {
    let (mut g, _, capital) = board();
    let old_front_town = g.found_city_for(2, at(10, 8), None);
    g.cities.get_mut(&old_front_town).unwrap().is_capital = false;
    g.at_war.insert((0, 2));
    assert!(g.is_at_war(0, 1));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_one_war_at_a_time();
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.domination_finish_at_war(&g, 0), Some((1, capital)));
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.target_player, Some(1));
    assert_eq!(plan.target_city, Some(capital));
}

/// `recovery-needs-the-deficit`: a threatened city puts a war into Recovery
/// only while the army is short of twice the strongest opponent. Live King
/// civvis-20261005T141932Z (game 135) went into Recovery at 1848 power
/// against 645, one capital from Domination.
#[test]
fn a_threatened_city_leaves_a_winning_war_out_of_recovery_under_the_gene() {
    let (mut g, _, _) = board();
    g.at_war.insert((0, 2));
    g.spawn_test_unit("giant_death_robot", 2, at(5, 8));
    for row in 0..6 {
        g.spawn_test_unit("giant_death_robot", 0, at(30, 14 + row));
    }
    assert!(g.military_power(0) >= 2.0 * g.military_power(2));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert_eq!(
        ai.assess(&g, 0).strategy,
        GrandStrategy::Recovery,
        "off: the threatened city"
    );
    ai.enable_recovery_needs_the_deficit();
    assert_ne!(ai.assess(&g, 0).strategy, GrandStrategy::Recovery);
}

/// `science-ladder-reads-the-clock`: each launch reads on the culture
/// race's finish clock -- 58, 67, 84, 94 -- against the ladder's 25, 45, 65
/// and 78 with the gene off.
#[test]
fn the_science_ladder_reads_the_finish_clock_under_the_gene() {
    let (mut g, _, _) = board();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    for (project, off, on) in [
        ("launch_earth_satellite", 25, 58),
        ("launch_moon_landing", 45, 67),
        ("launch_mars_colony", 65, 84),
        ("exoplanet_expedition", 78, 94),
    ] {
        g.players[2].science_projects.insert(project.to_string());
        ai.disable_science_ladder_reads_the_clock();
        assert_eq!(ai.science_race_pressure(&g, 2), off, "{project} off");
        ai.enable_science_ladder_reads_the_clock();
        assert_eq!(ai.science_race_pressure(&g, 2), on, "{project} on");
    }
}
