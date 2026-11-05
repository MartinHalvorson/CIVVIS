use super::super::test_support::opt_in_off_in_both_controllers;
use super::*;
use std::sync::Arc;

/// Two majors with a capital each, met, no units; military power is the
/// observed figure (ours 577, theirs 3377, the 033244Z t168 reading).
fn board(war: bool) -> Game {
    let mut game = Game::new_full(2, 24, 16, 7_947, 300, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("every major starts with a settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
    }
    for pid in 0..2 {
        for unit in game.player_unit_ids(pid) {
            game.remove_unit(unit);
        }
    }
    game.record_contact(0, 1);
    Arc::make_mut(&mut game.observed_military_power).insert(0, 577.0);
    Arc::make_mut(&mut game.observed_military_power).insert(1, 3377.0);
    if war {
        game.at_war.insert((0, 1));
    }
    game.turn = 168;
    game.current = 0;
    game
}

fn plan(strategy: GrandStrategy) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: 0,
        rush: false,
    }
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.victory_target = Some(target);
    if gene {
        ai.enable_threatened_lane_lends_the_army();
    }
    ai
}

#[test]
fn threatened_lane_lends_the_army_is_opt_in() {
    opt_in_off_in_both_controllers("threatened-lane-lends-the-army", |ai| {
        ai.threatened_lane_lends_the_army
    });
}

/// 033244Z t168: a Science seat in Recovery, at war, outweighed. Off, the
/// governor keeps the genome's floor (no lend); on, it builds to at least
/// two land bodies a city.
#[test]
fn a_threatened_science_seat_at_war_borrows_the_war_target() {
    let game = board(true);
    let recovery = plan(GrandStrategy::Recovery);
    assert_eq!(
        seat(VictoryTarget::Science, false).domination_war_military_per_city(&game, 0, &recovery),
        None
    );
    let lent = seat(VictoryTarget::Science, true)
        .domination_war_military_per_city(&game, 0, &recovery)
        .expect("the threatened Science seat borrows the war target");
    assert!(lent >= 2.0, "lent {lent} a city");
}

/// A war with no home city threatened, or a Recovery plan at peace, lends
/// nothing: the lane keeps its economy.
#[test]
fn an_unthreatened_war_or_peace_lends_nothing() {
    let war = board(true);
    assert_eq!(
        seat(VictoryTarget::Science, true).domination_war_military_per_city(
            &war,
            0,
            &plan(GrandStrategy::Science)
        ),
        None
    );
    let peace = board(false);
    assert_eq!(
        seat(VictoryTarget::Science, true).domination_war_military_per_city(
            &peace,
            0,
            &plan(GrandStrategy::Recovery)
        ),
        None
    );
}

/// A city of ours attacked within four turns is a threatened home city too.
#[test]
fn a_city_attacked_this_turn_counts_as_threatened() {
    let mut game = board(true);
    let ours = game.player_city_ids(0)[0];
    game.cities.get_mut(&ours).unwrap().last_attacked = game.turn;
    assert!(seat(VictoryTarget::Science, true)
        .domination_war_military_per_city(&game, 0, &plan(GrandStrategy::Science))
        .is_some());
}

/// Domination keeps its own lend, gene or not.
#[test]
fn domination_is_unchanged() {
    let game = board(true);
    let science_plan = plan(GrandStrategy::Science);
    assert_eq!(
        seat(VictoryTarget::Domination, false).domination_war_military_per_city(
            &game,
            0,
            &science_plan
        ),
        seat(VictoryTarget::Domination, true).domination_war_military_per_city(
            &game,
            0,
            &science_plan
        )
    );
}
