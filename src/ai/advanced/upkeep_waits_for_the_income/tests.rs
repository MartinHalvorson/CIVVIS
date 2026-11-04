use super::*;

/// Two majors on grassland; we hold three cities, the rival one, and the
/// plan is a Domination conquest of the rival's city.
fn fixture() -> (Game, StrategicPlan, Vec<u32>) {
    let mut g = Game::new_full(2, 24, 16, 366_401, 250, 0, false);
    g.units.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (4, 4), None);
    g.found_city_for(0, (4, 10), None);
    g.found_city_for(0, (10, 4), None);
    g.found_city_for(1, (18, 10), None);
    g.current = 0;
    g.turn = g.standard_duration(UPKEEP_FROM_STANDARD_TURN) + 10;
    g.players[0].gold = 10.0;
    g.players[0].gold_per_turn = 3.0;
    let ours = g.player_city_ids(0);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(g.player_city_ids(1)[0]),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, plan, ours)
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: crate::name::Name::new(name),
    }
}

fn on() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_upkeep_waits_for_the_income();
    ai
}

#[test]
fn added_upkeep_reads_units_buildings_and_income_districts() {
    let (g, _, _) = fixture();
    let tank = g.rules.units["tank"].maintenance;
    assert!(tank > 0.0, "the fixture needs a unit with upkeep");
    assert_eq!(AdvancedAi::item_added_upkeep(&g, &unit("tank")), tank);
    assert_eq!(
        AdvancedAi::item_added_upkeep(
            &g,
            &Item::Formation {
                unit: crate::name!("tank"),
                formation: 1,
            }
        ),
        2.0 * tank,
        "a Corps is two units' upkeep"
    );
    let market = &g.rules.buildings["market"];
    assert_eq!(
        AdvancedAi::item_added_upkeep(
            &g,
            &Item::Building {
                building: crate::name!("market"),
            }
        ),
        (market.maintenance - market.yields.gold).max(0.0),
        "a building's upkeep is net of its own Gold"
    );
    assert_eq!(
        AdvancedAi::item_added_upkeep(
            &g,
            &Item::District {
                district: crate::name!("commercial_hub"),
                pos: (5, 5),
            }
        ),
        0.0,
        "an income district never waits"
    );
}

#[test]
fn a_new_unit_waits_while_income_and_bank_cannot_carry_it() {
    let (mut g, plan, ours) = fixture();
    let tank = unit("tank");
    // Income 3 against the Tank's upkeep: the deficit outlasts a 10 Gold bank.
    assert!(on().upkeep_waits_for(&g, 0, ours[0], &tank, false, &plan));
    // Off, or for another target, nothing waits.
    let off = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(!off.upkeep_waits_for(&g, 0, ours[0], &tank, false, &plan));
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_upkeep_waits_for_the_income();
    assert!(!science.upkeep_waits_for(&g, 0, ours[0], &tank, false, &plan));
    // A bank that covers fifteen turns of the deficit pays for it.
    g.players[0].gold = 1000.0;
    assert!(!on().upkeep_waits_for(&g, 0, ours[0], &tank, false, &plan));
    // An item with no upkeep never waits.
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = -20.0;
    assert!(!on().upkeep_waits_for(&g, 0, ours[0], &unit("builder"), false, &plan));
}

#[test]
fn the_other_queues_upkeep_is_counted_before_it_comes_due() {
    let (mut g, plan, ours) = fixture();
    let tank = unit("tank");
    let upkeep = AdvancedAi::item_added_upkeep(&g, &tank);
    // Income carries this Tank and one more, not a third.
    g.players[0].gold_per_turn = 2.0 * upkeep + 0.5;
    g.cities.get_mut(&ours[1]).unwrap().queue = vec![tank.clone()];
    assert!(!on().upkeep_waits_for(&g, 0, ours[0], &tank, false, &plan));
    g.cities.get_mut(&ours[2]).unwrap().queue = vec![tank.clone()];
    assert_eq!(
        AdvancedAi::committed_queue_upkeep(&g, 0, Some(ours[0])),
        2.0 * upkeep
    );
    assert!(on().upkeep_waits_for(&g, 0, ours[0], &tank, false, &plan));
    // The city's own queue head is not counted against itself.
    g.cities.get_mut(&ours[0]).unwrap().queue = vec![tank.clone()];
    assert_eq!(
        AdvancedAi::committed_queue_upkeep(&g, 0, Some(ours[0])),
        2.0 * upkeep
    );
}

#[test]
fn a_threatened_city_still_trains_its_land_defender() {
    let (g, plan, ours) = fixture();
    assert!(!on().upkeep_waits_for(&g, 0, ours[0], &unit("tank"), true, &plan));
    assert!(
        on().upkeep_waits_for(&g, 0, ours[0], &unit("bomber"), true, &plan),
        "an air unit is no city's defender"
    );
}

#[test]
fn the_governor_sets_aside_only_a_queued_item_under_half_paid() {
    let (mut g, plan, ours) = fixture();
    let tank = unit("tank");
    g.cities.get_mut(&ours[0]).unwrap().queue = vec![tank.clone()];
    g.cities.get_mut(&ours[0]).unwrap().production = 0.0;
    assert!(on().upkeep_wait_preempts(&g, 0, ours[0], &tank, &plan));
    let cost = g.item_cost_for_city(0, ours[0], &tank);
    g.cities.get_mut(&ours[0]).unwrap().production = 0.6 * cost;
    assert!(
        !on().upkeep_wait_preempts(&g, 0, ours[0], &tank, &plan),
        "a Tank 60% paid finishes"
    );
    // The plan's threatened city keeps its land defender.
    g.cities.get_mut(&ours[0]).unwrap().production = 0.0;
    let mut threatened = plan.clone();
    threatened.threatened_city = Some(ours[0]);
    assert!(!on().upkeep_wait_preempts(&g, 0, ours[0], &tank, &threatened));
}

#[test]
fn the_shut_gate_is_journalled_once_a_pass() {
    let (mut g, _, _) = fixture();
    let shut = |ai: &AdvancedAi| {
        ai.journal()
            .since(0)
            .thoughts
            .iter()
            .filter(|thought| thought.detail.contains("upkeep-waits-for-the-income"))
            .count()
    };
    let mut ai = on();
    ai.base.journal = crate::reasoning::Journal::recording();
    // Income 3 carries a 1-Gold item: the gate is open, nothing is written.
    ai.note_upkeep_wait_state(&g, 0);
    assert_eq!(shut(&ai), 0);
    // Income 0 with 10 Gold banked does not: a 1-Gold deficit needs 15.
    g.players[0].gold_per_turn = 0.0;
    ai.note_upkeep_wait_state(&g, 0);
    assert_eq!(shut(&ai), 1);
    // Off, silent.
    let mut off = AdvancedAi::targeting(VictoryTarget::Domination);
    off.base.journal = crate::reasoning::Journal::recording();
    off.note_upkeep_wait_state(&g, 0);
    assert_eq!(shut(&off), 0);
}

#[test]
fn nothing_waits_before_the_late_game() {
    let (mut g, plan, ours) = fixture();
    let tank = unit("tank");
    assert!(on().upkeep_waits_for(&g, 0, ours[0], &tank, false, &plan));
    g.turn = g.standard_duration(UPKEEP_FROM_STANDARD_TURN) - 1;
    assert!(
        !on().upkeep_waits_for(&g, 0, ours[0], &tank, false, &plan),
        "live 052705Z: a 1-Gold Spearman at t45 is no bankruptcy"
    );
    let mut ai = on();
    ai.base.journal = crate::reasoning::Journal::recording();
    g.players[0].gold_per_turn = 0.0;
    ai.note_upkeep_wait_state(&g, 0);
    assert!(ai.journal().since(0).thoughts.is_empty());
}

#[test]
fn a_recovery_plan_keeps_its_land_defenders() {
    let (g, mut plan, ours) = fixture();
    plan.strategy = GrandStrategy::Recovery;
    assert!(!on().upkeep_waits_for(&g, 0, ours[0], &unit("spearman"), false, &plan));
    assert!(
        on().upkeep_waits_for(&g, 0, ours[0], &unit("bomber"), false, &plan),
        "an air unit is no Recovery defender"
    );
}
