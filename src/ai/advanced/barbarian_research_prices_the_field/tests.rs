use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

fn board() -> (Game, AdvancedAi, u32, u32, u32) {
    let mut g = Game::new_full(1, 24, 16, 96_170, 250, 0, true);
    let units: Vec<_> = g.units.keys().copied().collect();
    for unit in units {
        g.remove_unit(unit);
    }
    g.barb_camps.clear();
    g.barb_camp_guards.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
    }
    let city = g.found_city_for(0, (5, 5), None);
    g.turn = 70;
    g.world_era = 2;
    g.players[0].civ = "Indonesia".to_string();
    g.players[0].techs = g
        .rules
        .techs
        .iter()
        .filter(|(_, spec)| spec.era < 2)
        .map(|(name, _)| *name)
        .collect();
    g.players[0].research = None;
    for _ in 0..3 {
        g.spawn_test_unit("archer", 0, (5, 5));
    }
    let horse = g.spawn_test_unit("horseman", 0, (5, 5));
    g.units.get_mut(&horse).unwrap().hp = 11;
    g.spawn_test_unit("catapult", 0, (5, 5));
    let barb = g.barb_pid.unwrap();
    let raider = g.spawn_test_unit("man_at_arms", barb, (6, 5));
    g.units.get_mut(&raider).unwrap().hp = 70;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_barbarian_research_prices_the_field();
    assert!(g.player_can_see(0, (6, 5)));
    (g, ai, city, horse, raider)
}

#[test]
fn ships_off() {
    opt_in_off_in_both_controllers("barbarian-research-prices-the-field", |ai| {
        ai.barbarian_research_prices_the_field
    });
}

#[test]
fn wounded_horse_and_catapult_do_not_cancel_archer_modernization() {
    let (g, ai, _, _, _) = board();
    assert!(!BasicAi::barbarian_military_gap(&g, 0));
    assert!(BasicAi::barbarian_military_research_goal(&g, 0).is_none());
    assert_eq!(
        ai.barbarian_research_upgrade_goal(&g, 0).as_deref(),
        Some("machinery")
    );
    let mut off = ai.clone();
    off.disable_barbarian_research_prices_the_field();
    assert_eq!(
        off.barbarian_research_upgrade_goal(&g, 0),
        BasicAi::barbarian_military_research_goal(&g, 0)
    );
}

#[test]
fn a_healthy_horse_retains_the_existing_quality_threshold() {
    let (mut g, ai, _, horse, _) = board();
    g.units.get_mut(&horse).unwrap().hp = 100;
    assert!(ai.barbarian_research_upgrade_goal(&g, 0).is_none());
}

#[test]
fn a_badly_wounded_raider_does_not_force_a_research_detour() {
    let (mut g, ai, _, horse, raider) = board();
    g.remove_unit(horse);
    let catapults: Vec<_> = g
        .units
        .values()
        .filter(|u| u.kind == "catapult")
        .map(|u| u.id)
        .collect();
    for unit in catapults {
        g.remove_unit(unit);
    }
    g.units.get_mut(&raider).unwrap().hp = 1;
    assert!(BasicAi::barbarian_military_research_goal(&g, 0).is_some());
    assert!(ai.barbarian_research_upgrade_goal(&g, 0).is_none());
}

#[test]
fn bombard_field_power_agrees_with_engine_land_target_penalty() {
    let (g, _, _, _, raider) = board();
    let catapult = g
        .units
        .values()
        .find(|u| u.owner == 0 && u.kind == "catapult")
        .unwrap();
    let defender = &g.units[&raider];
    let (shot, _) = g
        .ranged_strike_strengths(catapult.id, raider, defender.pos)
        .unwrap();
    assert_eq!(shot, 18.0);
    assert_eq!(
        AdvancedAi::barbarian_field_power(&g, catapult),
        effective_strength(g.rules.units[catapult.kind].strength, catapult.hp).max(shot)
    );
    assert_eq!(AdvancedAi::barbarian_field_power(&g, catapult), 25.0);
}

#[test]
fn a_camp_guard_and_a_distant_raider_do_not_raise_the_alarm() {
    let (mut g, ai, _, _, raider) = board();
    g.barb_camp_guards.insert((6, 5), raider);
    assert!(ai.barbarian_research_upgrade_goal(&g, 0).is_none());
    g.barb_camp_guards.clear();
    g.relocate(raider, (17, 12));
    assert!(ai.barbarian_research_upgrade_goal(&g, 0).is_none());
}

#[test]
fn the_research_chooser_takes_the_legal_machinery_step() {
    let (mut g, ai, city, _, raider) = board();
    assert!(ai.base.barbarian_tactics_enabled());
    assert!(
        !AdvancedAi::imminent_city_attack(&g, 0, city, &g.player_vision_frame(0)),
        "raider reach {:?}, center {:?}, city strength {}, war {}",
        g.attack_reach(raider),
        g.cities[&city].pos,
        g.city_strength(city),
        g.is_at_war(0, g.barb_pid.unwrap())
    );
    assert!(AdvancedAi::barbarian_field_attack_is_imminent(&g, 0));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    let mut off_game = g.clone();
    let mut off = ai.clone();
    off.disable_barbarian_research_prices_the_field();
    off.advanced_research(&mut off_game, 0, &plan);
    ai.advanced_research(&mut g, 0, &plan);
    assert_ne!(off_game.players[0].research.as_deref(), Some("machinery"));
    assert_eq!(g.players[0].research.as_deref(), Some("machinery"));
}

#[test]
fn a_visible_raider_that_cannot_reach_a_city_does_not_preempt_science() {
    let (mut g, ai, _, _, raider) = board();
    g.relocate(raider, (10, 5));
    g.spawn_test_unit("scout", 0, (10, 6));
    assert!(g.player_can_see(0, (10, 5)));
    assert!(AdvancedAi::barbarian_field_quality_gap(&g, 0));
    assert!(!AdvancedAi::barbarian_field_attack_is_imminent(&g, 0));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    ai.advanced_research(&mut g, 0, &plan);
    assert_ne!(g.players[0].research.as_deref(), Some("machinery"));
}
