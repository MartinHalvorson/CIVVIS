use super::*;
use crate::name;

/// Two founded empires at turn 110 in world era `era`; ours knows Mining and
/// Bronze Working (Iron Working is one step away) and fields no soldier but
/// the `army` it is given, placed beside its capital.
fn board(era: usize, army: &[&str]) -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(2, 30, 20, 110_211_000, 300, 0, false);
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.found_city_for(pid, g.units[&settler].pos, None);
    }
    g.current = 0;
    g.turn = 110;
    g.world_era = era;
    g.record_contact(0, 1);
    for tech in ["mining", "bronze_working"] {
        g.players[0].techs.insert(Name::new(tech));
    }
    let soldiers: Vec<u32> = g
        .player_unit_ids(0)
        .into_iter()
        .filter(|id| g.rules.units[g.units[id].kind].class == "military")
        .collect();
    for id in soldiers {
        g.remove_unit(id);
    }
    let capital = g.cities[&g.player_city_ids(0)[0]].pos;
    for (i, kind) in army.iter().enumerate() {
        g.spawn_unit(
            kind,
            0,
            (capital.0 + 1 + i as i32 % 2, capital.1 + i as i32 / 2),
        );
    }
    let ai = AdvancedAi::new();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan)
}

#[test]
fn warriors_wait_on_iron_working_once_the_world_leaves_its_era() {
    // Iron Working is Classical (era 1); the world is Medieval (era 2).
    let (g, mut ai, _) = board(2, &["warrior", "warrior", "warrior"]);
    assert!(
        ai.old_techs_before_the_beeline_goal(&g, 0, false).is_none(),
        "off by default"
    );
    ai.enable_old_techs_before_the_beeline();
    let pick = ai
        .old_techs_before_the_beeline_goal(&g, 0, false)
        .expect("three Warriors wait on a Classical technology in the Medieval era");
    assert_eq!(pick.tech, name!("iron_working"));
    assert_eq!(
        pick.reason,
        OldTechReason::ArmyUpgrade {
            held: name!("warrior"),
            // The seat's own Swordsman (a Legion for Rome).
            successor: g.player_unit_replacement(0, name!("swordsman")),
            units: 3,
        }
    );
    ai.disable_old_techs_before_the_beeline();
    assert!(ai.old_techs_before_the_beeline_goal(&g, 0, false).is_none());
}

#[test]
fn an_upgrade_from_the_current_era_is_not_yet_due() {
    // The world is still Classical: Iron Working is a current technology.
    let (g, mut ai, _) = board(1, &["warrior", "warrior"]);
    ai.enable_old_techs_before_the_beeline();
    assert!(AdvancedAi::overdue_army_upgrade_tech(&g, 0).is_none());
    // And nothing is two eras behind a Classical world.
    assert!(ai.old_techs_before_the_beeline_goal(&g, 0, false).is_none());
}

#[test]
fn recon_units_do_not_pull_research() {
    let (g, _, _) = board(3, &["scout", "scout", "scout"]);
    assert!(
        AdvancedAi::overdue_army_upgrade_tech(&g, 0).is_none(),
        "Scouts upgrade on the explorers' schedule"
    );
}

#[test]
fn archers_wait_on_machinery_from_the_renaissance() {
    let (mut g, mut ai, _) = board(3, &["archer", "archer"]);
    ai.enable_old_techs_before_the_beeline();
    let pick = AdvancedAi::overdue_army_upgrade_tech(&g, 0).expect("Machinery is Medieval");
    assert_eq!(pick.tech, name!("machinery"));
    // Once Machinery is known the Archers can upgrade: no army goal is left.
    g.players[0].techs.insert(name!("machinery"));
    assert!(AdvancedAi::overdue_army_upgrade_tech(&g, 0).is_none());
}

#[test]
fn the_unit_count_picks_between_two_locked_upgrades() {
    let (g, _, _) = board(3, &["archer", "warrior", "warrior"]);
    let pick = AdvancedAi::overdue_army_upgrade_tech(&g, 0).expect("both are overdue");
    assert_eq!(
        pick.tech,
        name!("iron_working"),
        "two Warriors outvote one Archer"
    );
}

#[test]
fn the_backlog_comes_due_two_eras_behind_the_world() {
    // No army at all: only the backlog arm can fire.
    let (g, mut ai, _) = board(2, &[]);
    ai.enable_old_techs_before_the_beeline();
    let pick = ai
        .old_techs_before_the_beeline_goal(&g, 0, false)
        .expect("Ancient technologies are two eras behind a Medieval world");
    let spec = &g.rules.techs[&pick.tech];
    assert_eq!(spec.era, 0);
    assert_eq!(pick.reason, OldTechReason::Backlog { eras: 2 });
    let cheapest = g
        .available_techs(0)
        .into_iter()
        .filter(|tech| g.rules.techs[tech].era == 0)
        .map(|tech| g.rules.techs[&tech].cost)
        .fold(f64::INFINITY, f64::min);
    assert_eq!(spec.cost, cheapest, "cheapest first");
    // One era behind is still inside the grace.
    let (g, _, _) = board(1, &[]);
    assert!(AdvancedAi::overdue_backlog_tech(&g, 0).is_none());
}

#[test]
fn the_army_upgrade_ranks_ahead_of_a_cheaper_backlog() {
    let (g, mut ai, _) = board(2, &["warrior", "warrior"]);
    ai.enable_old_techs_before_the_beeline();
    assert!(AdvancedAi::overdue_backlog_tech(&g, 0).is_some());
    let pick = ai.old_techs_before_the_beeline_goal(&g, 0, false).unwrap();
    assert_eq!(pick.tech, name!("iron_working"));
}

#[test]
fn a_committed_launch_chain_keeps_the_slot() {
    let (g, mut ai, _) = board(2, &["warrior", "warrior"]);
    ai.enable_old_techs_before_the_beeline();
    assert!(ai.old_techs_before_the_beeline_goal(&g, 0, true).is_none());
}

#[test]
fn the_pick_owns_the_research_slot() {
    let (mut g, mut ai, plan) = board(2, &["warrior", "warrior", "warrior"]);
    ai.enable_old_techs_before_the_beeline();
    g.players[0].research = None;
    ai.advanced_research(&mut g, 0, &plan);
    let research = g.players[0]
        .research
        .clone()
        .expect("a technology is chosen");
    let research = Name::new(&research);
    assert!(
        research == name!("iron_working") || ai.tech_leads_to(&g, &research, "iron_working"),
        "research {research:?} walks toward Iron Working"
    );
}
