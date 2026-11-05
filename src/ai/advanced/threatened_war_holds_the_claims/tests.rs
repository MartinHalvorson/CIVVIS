use super::super::test_support::opt_in_off_in_both_controllers;
use super::*;

/// Two majors with a capital each, met, no units.
fn board(war: bool) -> Game {
    let mut game = Game::new_full(2, 24, 16, 7_953, 300, 0, false);
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
    if war {
        game.at_war.insert((0, 1));
    }
    game.turn = 161;
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

fn holding() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_threatened_war_holds_the_claims();
    ai
}

#[test]
fn threatened_war_holds_the_claims_is_opt_in() {
    opt_in_off_in_both_controllers("threatened-war-holds-the-claims", |ai| {
        ai.threatened_war_holds_the_claims
    });
}

/// At war with a major and in Recovery: the claims hold; off, they never do.
#[test]
fn a_threatened_war_holds_the_claims() {
    let game = board(true);
    let recovery = plan(GrandStrategy::Recovery);
    assert!(holding().threatened_war_holds_the_claims_holds(&game, 0, &recovery));
    assert!(!AdvancedAi::new().threatened_war_holds_the_claims_holds(&game, 0, &recovery));
}

/// A named threatened city, or a city attacked this turn, is a threat too.
#[test]
fn a_named_or_attacked_city_counts_as_threatened() {
    let mut game = board(true);
    let ours = game.player_city_ids(0)[0];
    let mut named = plan(GrandStrategy::Science);
    named.threatened_city = Some(ours);
    assert!(holding().threatened_war_holds_the_claims_holds(&game, 0, &named));
    game.cities.get_mut(&ours).unwrap().last_attacked = game.turn;
    assert!(holding().threatened_war_holds_the_claims_holds(
        &game,
        0,
        &plan(GrandStrategy::Science)
    ));
}

/// A war with no home city threatened, or a Recovery plan at peace, leaves
/// the claims alone.
#[test]
fn an_unthreatened_war_or_peace_leaves_the_claims() {
    assert!(!holding().threatened_war_holds_the_claims_holds(
        &board(true),
        0,
        &plan(GrandStrategy::Science)
    ));
    assert!(!holding().threatened_war_holds_the_claims_holds(
        &board(false),
        0,
        &plan(GrandStrategy::Recovery)
    ));
}
