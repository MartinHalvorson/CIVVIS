use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, StrategicPlan};
use crate::name::Name;

/// Seat p3 ("moonshot", Cree, Science target) of `hotseat-20261128T031456Z`
/// as it chose its research: one city, `techs` known, nothing in progress,
/// one rival met far away, nobody at war.
fn moonshot(techs: &[&str], turn: u32) -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(2, 40, 24, 936012, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.players[0].civ = "Cree".to_string();
    g.found_city_for(0, (6, 12), None);
    g.found_city_for(1, (25, 12), None);
    g.spawn_test_unit("warrior", 0, (6, 12));
    g.players[0]
        .techs
        .extend(techs.iter().copied().map(Name::new));
    g.players[0].research = None;
    g.at_war.clear();
    g.current = 0;
    g.turn = turn;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.base.journal = crate::reasoning::Journal::recording();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 10,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan)
}

/// The research the seat starts, and the gene's journal lines.
fn research(
    techs: &[&str],
    turn: u32,
    target: Option<VictoryTarget>,
    on: bool,
) -> (Option<String>, Vec<String>) {
    let (mut g, mut ai, plan) = moonshot(techs, turn);
    if let Some(target) = target {
        ai.victory_target = Some(target);
    }
    if on {
        ai.enable_science_seat_writes_first();
    }
    ai.advanced_research(&mut g, 0, &plan);
    let lines = ai
        .base
        .journal
        .since(u64::MAX)
        .thoughts
        .into_iter()
        .filter(|thought| thought.detail.contains("science-seat-writes-first:"))
        .map(|thought| format!("{} | {}", thought.headline, thought.detail))
        .collect();
    (g.players[0].research.clone(), lines)
}

const TURN_12: [&str; 3] = ["animal_husbandry", "mining", "pottery"];

#[test]
fn science_seat_writes_first_ships_off_and_is_registered() {
    opt_in_off_in_both_controllers("science-seat-writes-first", |ai| {
        ai.science_seat_writes_first
    });
}

#[test]
fn moonshot_turn_12_takes_writing_where_it_took_another_step() {
    let science = Some(VictoryTarget::Science);
    let (off, off_lines) = research(&TURN_12, 12, science, false);
    let (on, on_lines) = research(&TURN_12, 12, science, true);
    assert!(off.is_some(), "off, the seat still researches something");
    assert_ne!(
        off.as_deref(),
        Some("writing"),
        "off, the Rocketry beeline takes another step first"
    );
    assert!(off_lines.is_empty(), "{off_lines:#?}");
    assert_eq!(on.as_deref(), Some("writing"));
    assert_eq!(on_lines.len(), 1, "{on_lines:#?}");
    assert!(
        on_lines[0].contains("Campus and Library unlock"),
        "{on_lines:#?}"
    );
}

#[test]
fn with_two_techs_and_no_pottery_the_walk_takes_pottery_first() {
    // Turn 10 of the same seat: Animal Husbandry and Mining.
    let (on, on_lines) = research(
        &["animal_husbandry", "mining"],
        10,
        Some(VictoryTarget::Science),
        true,
    );
    assert_eq!(on.as_deref(), Some("pottery"), "Writing needs Pottery");
    assert_eq!(on_lines.len(), 1, "{on_lines:#?}");
}

#[test]
fn the_goal_holds_its_scope() {
    let (mut g, mut ai, _) = moonshot(&TURN_12, 12);
    assert_eq!(ai.science_seat_writes_first_goal(&g, 0), None, "off");
    ai.enable_science_seat_writes_first();
    assert_eq!(ai.science_seat_writes_first_goal(&g, 0), Some("writing"));
    // One tech known: the opening's first steps are its own.
    g.players[0].techs.clear();
    g.players[0].techs.insert(crate::name!("mining"));
    assert_eq!(ai.science_seat_writes_first_goal(&g, 0), None, "one tech");
    g.players[0].techs.insert(crate::name!("animal_husbandry"));
    assert_eq!(
        ai.science_seat_writes_first_goal(&g, 0),
        Some("writing"),
        "two techs"
    );
    // Writing known.
    g.players[0].techs.insert(crate::name!("pottery"));
    g.players[0].techs.insert(crate::name!("writing"));
    assert_eq!(ai.science_seat_writes_first_goal(&g, 0), None, "known");
    g.players[0].techs.remove(&crate::name!("writing"));
    // Another target, no target, or a game with no Science victory.
    for target in [
        Some(VictoryTarget::Culture),
        Some(VictoryTarget::Domination),
        Some(VictoryTarget::Religion),
        None,
    ] {
        ai.victory_target = target;
        assert_eq!(ai.science_seat_writes_first_goal(&g, 0), None, "{target:?}");
    }
    ai.victory_target = Some(VictoryTarget::Science);
    g.victory_conditions.science = false;
    assert_eq!(ai.science_seat_writes_first_goal(&g, 0), None, "disabled");
}

#[test]
fn other_victory_lanes_keep_their_research_choices() {
    for target in [VictoryTarget::Culture, VictoryTarget::Domination] {
        let (off, _) = research(&TURN_12, 12, Some(target), false);
        let (on, on_lines) = research(&TURN_12, 12, Some(target), true);
        assert_eq!(off, on, "{target:?}");
        assert!(on_lines.is_empty(), "{target:?}: {on_lines:#?}");
    }
}

#[test]
fn a_tech_in_progress_finishes_first() {
    let pick = |on: bool| {
        let (mut g, mut ai, plan) = moonshot(&TURN_12, 13);
        g.players[0].research = Some("sailing".to_string());
        if on {
            ai.enable_science_seat_writes_first();
        }
        ai.advanced_research(&mut g, 0, &plan);
        g.players[0].research.clone()
    };
    assert_eq!(pick(false).as_deref(), Some("sailing"));
    assert_eq!(pick(true).as_deref(), Some("sailing"));
}
