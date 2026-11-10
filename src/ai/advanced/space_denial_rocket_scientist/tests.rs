use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// Two majors at peace and in contact; seat 1 holds Rocketry and has landed
/// its Earth Satellite. A level-one Spy of seat 0 stands in seat 1's capital
/// with a promotion owed, and the host offers Ace Driver, Quartermaster and
/// Rocket Scientist.
fn board() -> (Game, u32) {
    let mut g = Game::new_full(2, 30, 20, 41_830, 250, 0, false);
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("each fixture major begins with a Settler");
        let position = g.units[&settler].pos;
        g.found_city_for(pid, position, None);
        for uid in g.player_unit_ids(pid) {
            g.remove_unit(uid);
        }
    }
    g.current = 0;
    g.turn = 150;
    g.at_war.clear();
    g.record_contact(0, 1);
    g.players[1].techs.insert(crate::name!("rocketry"));
    g.players[1]
        .science_projects
        .insert("launch_earth_satellite".to_string());
    let capital = g.player_city_ids(1)[0];
    let spy = g.next_id;
    g.next_id += 1;
    g.spies.insert(
        spy,
        crate::game::Spy {
            id: spy,
            owner: 0,
            level: 1,
            promotions: BTreeSet::new(),
            city: Some(capital),
            ready_turn: g.turn,
            mission: None,
            sources_city: None,
            sources_until: 0,
            captured_by: None,
        },
    );
    let menu: BTreeSet<Name> = ["ace_driver", "quartermaster", "rocket_scientist"]
        .into_iter()
        .map(Name::new)
        .collect();
    Arc::make_mut(&mut g.host_unit_facts).insert(
        spy,
        crate::game::HostUnitFacts {
            spy_promotions: Some(menu),
            ..Default::default()
        },
    );
    (g, spy)
}

fn plan(strategy: GrandStrategy) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

/// The live arming this gene joins: the Rocketry-trigger network.
fn live(target: VictoryTarget) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_space_denial_spies_from_rocketry();
    ai.enable_racer_keeps_the_spies();
    ai
}

fn promoted(g: &Game, spy: u32) -> Vec<String> {
    g.spies[&spy]
        .promotions
        .iter()
        .map(|name| name.to_string())
        .collect()
}

#[test]
fn the_gene_ships_off() {
    opt_in_off_in_both_controllers("space-denial-promotes-the-rocket-scientist", |ai| {
        ai.space_denial_promotes_the_rocket_scientist
    });
}

#[test]
fn rocket_scientist_goes_first_and_the_list_keeps_its_order() {
    assert_eq!(
        AdvancedAi::rocket_scientist_first(&["cat_burglar", "con_artist", "disguise"]),
        vec!["rocket_scientist", "cat_burglar", "con_artist", "disguise"]
    );
    assert_eq!(
        AdvancedAi::rocket_scientist_first(&["technologist", "rocket_scientist", "linguist"]),
        vec!["rocket_scientist", "technologist", "linguist"]
    );
}

#[test]
fn a_culture_seat_takes_rocket_scientist_only_with_the_gene() {
    let (g, spy) = board();
    let mut off = g.clone();
    live(VictoryTarget::Culture).advanced_spies(&mut off, 0, &plan(GrandStrategy::Culture));
    assert_eq!(
        promoted(&off, spy),
        vec!["ace_driver"],
        "off: the first offered"
    );

    let mut ai = live(VictoryTarget::Culture);
    ai.enable_space_denial_promotes_the_rocket_scientist();
    assert!(ai.denial_wants_the_rocket_scientist(&g, 0));
    let mut on = g.clone();
    ai.advanced_spies(&mut on, 0, &plan(GrandStrategy::Culture));
    assert_eq!(promoted(&on, spy), vec!["rocket_scientist"]);
}

#[test]
fn a_domination_seat_takes_rocket_scientist_over_ace_driver() {
    let (g, spy) = board();
    let mut off = g.clone();
    live(VictoryTarget::Domination).advanced_spies(&mut off, 0, &plan(GrandStrategy::Conquest));
    assert_eq!(
        promoted(&off, spy),
        vec!["ace_driver"],
        "off: Conquest list"
    );

    let mut ai = live(VictoryTarget::Domination);
    ai.enable_space_denial_promotes_the_rocket_scientist();
    let mut on = g.clone();
    ai.advanced_spies(&mut on, 0, &plan(GrandStrategy::Conquest));
    assert_eq!(promoted(&on, spy), vec!["rocket_scientist"]);
}

#[test]
fn a_science_seat_and_a_board_without_a_racer_read_as_off() {
    let (g, _) = board();
    let mut science = live(VictoryTarget::Science);
    science.enable_space_denial_promotes_the_rocket_scientist();
    assert!(!science.denial_wants_the_rocket_scientist(&g, 0));

    let mut culture = live(VictoryTarget::Culture);
    culture.enable_space_denial_promotes_the_rocket_scientist();
    let mut quiet = g.clone();
    quiet.players[1].techs.clear();
    quiet.players[1].science_projects.clear();
    assert!(culture.denial_spy_racers(&quiet, 0).is_empty());
    assert!(!culture.denial_wants_the_rocket_scientist(&quiet, 0));

    let mut no_science = g.clone();
    no_science.victory_conditions.science = false;
    assert!(!culture.denial_wants_the_rocket_scientist(&no_science, 0));
}
