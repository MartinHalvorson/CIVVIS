use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::Action;
use std::sync::Arc;

/// Four majors, one city each, every pair met and at peace, Civil Service on
/// every tree. Turn 60 is our cadence turn (`60 % 12 == 0`).
fn board() -> Game {
    let mut g = Game::new_full(4, 34, 24, 79_208, 250, 0, false);
    for pid in 0..4 {
        g.current = pid;
        let unit = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("every major starts with a settler");
        g.apply(pid, &Action::FoundCity { unit }).unwrap();
        g.players[pid].gold = 1_000.0;
        g.players[pid].civics.insert(crate::name!("early_empire"));
        g.players[pid].civics.insert(crate::name!("civil_service"));
    }
    g.current = 0;
    g.turn = 60;
    g.at_war.clear();
    for other in 1..4 {
        g.record_contact(0, other);
    }
    g.record_contact(1, 2);
    g.record_contact(1, 3);
    g.record_contact(2, 3);
    g
}

fn culture_plan(turn: u32) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: turn,
        rush: false,
    }
}

fn stock() -> AdvancedAi {
    AdvancedAi::targeting(VictoryTarget::Culture)
}

fn allies() -> AdvancedAi {
    let mut ai = stock();
    ai.enable_culture_seat_allies();
    ai
}

/// `seat`'s Science a turn set to `science` through the host's correction.
fn science(g: &mut Game, seat: usize, science: f64) {
    let derived = AdvancedAi::seat_science_per_turn(g, seat)
        - g.observed_yield_adjustments
            .get(&seat)
            .map_or(0.0, |adjustment| adjustment.science);
    Arc::make_mut(&mut g.observed_yield_adjustments)
        .entry(seat)
        .or_default()
        .science = science - derived;
}

fn domestic(g: &mut Game, seat: usize, tourists: u32) {
    Arc::make_mut(&mut g.observed_public_empire_stats)
        .entry(seat)
        .or_default()
        .domestic_tourists = Some(tourists as _);
}

/// The partner this empire's open proposal names, if any.
fn proposed(g: &Game, pid: usize) -> Option<(usize, Option<String>)> {
    g.pending_deals
        .iter()
        .find(|deal| deal.from == pid && deal.expires >= g.turn)
        .map(|deal| (deal.to, deal.alliance.clone()))
}

fn capital(g: &Game, seat: usize) -> Pos {
    let cid = g
        .player_city_ids(seat)
        .into_iter()
        .find(|cid| g.cities[cid].is_capital)
        .expect("every fixture major holds its capital");
    g.cities[&cid].pos
}

#[test]
fn culture_seat_allies_is_opt_in() {
    opt_in_off_in_both_controllers("culture-seat-allies", |ai| ai.culture_seat_allies);
}

#[test]
fn only_a_culture_seat_is_armed() {
    assert!(!stock().culture_seat_allies_armed());
    assert!(allies().culture_seat_allies_armed());
    let mut domination = AdvancedAi::targeting(VictoryTarget::Domination);
    domination.enable_culture_seat_allies();
    assert!(!domination.culture_seat_allies_armed());
    assert_eq!(
        domination.culture_seat_ally_nearness(&board(), 0, 1),
        0.0,
        "off the Culture target the ranking is unchanged"
    );
}

#[test]
fn the_science_racer_and_the_bar_holder_are_never_asked() {
    let run = |ai: AdvancedAi| {
        let mut g = board();
        // Seat 1 makes the most Science, seat 2 has landed a project, seat 3
        // holds the most domestic tourists: none is a partner under the gene.
        science(&mut g, 1, 90.0);
        g.players[2]
            .science_projects
            .insert("launch_earth_satellite".to_string());
        domestic(&mut g, 3, 400);
        let mut ai = ai;
        let plan = culture_plan(g.turn);
        ai.propose_strategic_alliance(&mut g, 0, &plan, None);
        proposed(&g, 0)
    };
    assert!(run(stock()).is_some(), "stock: one of the three is asked");
    assert_eq!(run(allies()), None, "under the gene, none of them is");
}

#[test]
fn the_nearest_safe_rival_is_asked() {
    let mut g = board();
    // One rival is both the science leader and the bar holder; of the other
    // two, the nearer capital is asked.
    let ours = capital(&g, 0);
    let mut rivals = vec![1, 2, 3];
    rivals.sort_by_key(|seat| (g.wdist(ours, capital(&g, *seat)), *seat));
    let (nearest, farthest) = (rivals[0], rivals[2]);
    let barred = rivals[1];
    science(&mut g, barred, 90.0);
    domestic(&mut g, barred, 400);
    let mut ai = allies();
    let plan = culture_plan(g.turn);
    ai.propose_strategic_alliance(&mut g, 0, &plan, None);
    assert_eq!(
        proposed(&g, 0).map(|(to, _)| to),
        Some(nearest),
        "the nearest of {nearest} and {farthest} ({} and {} tiles)",
        g.wdist(ours, capital(&g, nearest)),
        g.wdist(ours, capital(&g, farthest))
    );
    assert!(
        ai.culture_seat_ally_nearness(&g, 0, nearest)
            >= ai.culture_seat_ally_nearness(&g, 0, farthest)
    );
}

#[test]
fn a_culture_seat_does_not_wait_for_scientific_theory() {
    // `research-alliance-first` holds the slot for a Research Alliance until
    // our tree has Scientific Theory; a Culture seat under the gene asks for
    // its stock kind meanwhile.
    let run = |mut ai: AdvancedAi| {
        let mut g = board();
        let barred = 1;
        science(&mut g, barred, 90.0);
        domestic(&mut g, barred, 400);
        ai.enable_research_alliance_first();
        let plan = culture_plan(g.turn);
        ai.propose_strategic_alliance(&mut g, 0, &plan, None);
        proposed(&g, 0)
    };
    assert_eq!(run(stock()), None, "research-alliance-first waits");
    let asked = run(allies()).expect("the gene asks");
    assert_eq!(asked.1.as_deref(), Some("cultural"));
}
