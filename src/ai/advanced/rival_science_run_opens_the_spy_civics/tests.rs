use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::Action;
use std::sync::Arc;

/// Two majors, one city each, met and at peace; ours holds every civic
/// through era 2, so Diplomatic Service (era 3, after Guilds) is available.
fn board() -> Game {
    let mut g = Game::new_full(2, 34, 24, 79_208, 250, 0, false);
    for pid in 0..2 {
        g.current = pid;
        let unit = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("every major starts with a settler");
        g.apply(pid, &Action::FoundCity { unit }).unwrap();
    }
    let early: Vec<String> = g
        .rules
        .civics
        .iter()
        .filter(|(_, spec)| spec.era <= 2)
        .map(|(name, _)| name.to_string())
        .collect();
    for civic in early {
        g.players[0].civics.insert(Name::new(&civic));
    }
    g.players[0].civic = None;
    g.current = 0;
    g.turn = 110;
    g.at_war.clear();
    g.record_contact(0, 1);
    g
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

fn plan(strategy: GrandStrategy) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 8,
        assessed_turn: 110,
        rush: false,
    }
}

fn religion_seat(on: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    if on {
        ai.enable_rival_science_run_opens_the_spy_civics();
    }
    ai
}

/// Tournament 8's shape: the rival makes 2.7 times our Science.
fn running_board() -> Game {
    let mut g = board();
    science(&mut g, 0, 70.0);
    science(&mut g, 1, 190.0);
    g
}

#[test]
fn rival_science_run_opens_the_spy_civics_is_a_native_opt_in() {
    opt_in_off_in_both_controllers("rival-science-run-opens-the-spy-civics", |ai| {
        ai.rival_science_run_opens_the_spy_civics
    });
}

#[test]
fn a_religion_seat_watching_a_science_run_adopts_diplomatic_service() {
    let religion = plan(GrandStrategy::Religion);
    let mut off_board = running_board();
    religion_seat(false).advanced_research(&mut off_board, 0, &religion);
    let off_pick = off_board.players[0].civic.clone();
    assert!(off_pick.is_some(), "the stock seat picks a civic");
    assert_ne!(
        off_pick.as_deref(),
        Some("diplomatic_service"),
        "the stock seat takes its government ladder first (the live loss)"
    );

    let mut on_board = running_board();
    religion_seat(true).advanced_research(&mut on_board, 0, &religion);
    assert_eq!(
        on_board.players[0].civic.as_deref(),
        Some("diplomatic_service")
    );
}

#[test]
fn diplomatic_service_held_the_goal_is_nationalism() {
    let mut g = running_board();
    g.players[0]
        .civics
        .insert(crate::name!("diplomatic_service"));
    let ai = religion_seat(true);
    assert_eq!(
        ai.rival_science_run_spy_civic_goal(&g, 0, &plan(GrandStrategy::Religion)),
        Some("nationalism")
    );
    g.players[0].civics.insert(crate::name!("nationalism"));
    assert_eq!(
        ai.rival_science_run_spy_civic_goal(&g, 0, &plan(GrandStrategy::Religion)),
        None
    );
}

#[test]
fn no_run_no_goal() {
    let ai = religion_seat(true);
    let religion = plan(GrandStrategy::Religion);
    // Under the ratio.
    let mut g = board();
    science(&mut g, 0, 100.0);
    science(&mut g, 1, 140.0);
    assert_eq!(AdvancedAi::rival_science_run(&g, 0), None);
    assert_eq!(ai.rival_science_run_spy_civic_goal(&g, 0, &religion), None);
    // Over the ratio but under the floor: early-game noise.
    science(&mut g, 0, 10.0);
    science(&mut g, 1, 40.0);
    assert_eq!(ai.rival_science_run_spy_civic_goal(&g, 0, &religion), None);
    // The Tournament 8 shape is read.
    assert_eq!(AdvancedAi::rival_science_run(&running_board(), 0), Some(1));
}

#[test]
fn a_science_aim_and_the_gene_off_are_untouched() {
    let g = running_board();
    let religion = plan(GrandStrategy::Religion);
    assert_eq!(
        religion_seat(false).rival_science_run_spy_civic_goal(&g, 0, &religion),
        None
    );
    assert_eq!(
        religion_seat(true).rival_science_run_spy_civic_goal(&g, 0, &plan(GrandStrategy::Science)),
        None,
        "a Science counterplan is left to spies-steal-the-lead"
    );
    let mut science_seat = AdvancedAi::targeting(VictoryTarget::Science);
    science_seat.enable_rival_science_run_opens_the_spy_civics();
    assert_eq!(
        science_seat.rival_science_run_spy_civic_goal(&g, 0, &religion),
        None
    );
}
