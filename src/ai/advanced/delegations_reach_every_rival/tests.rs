use super::super::{AdvancedAi, GrandStrategy, StrategicPlan};
use super::MISSION_RETRY_TURNS;
use crate::game::{Action, Game};

/// Three majors in contact, each with a capital and Gold to spend.
fn three_majors() -> Game {
    let mut game = Game::new_full(3, 24, 16, 7_931, 300, 0, false);
    for pid in 0..3 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("every major starts with a settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
        game.players[pid].gold = 300.0;
    }
    for pid in 0..3 {
        for other in pid + 1..3 {
            game.record_contact(pid, other);
        }
    }
    game.turn = 16;
    game
}

#[test]
fn a_rival_hosting_our_delegation_is_skipped_for_the_next() {
    let mut game = three_majors();
    game.apply(0, &Action::SendDelegation { player: 1 })
        .expect("a delegation to major 1");
    let mut ai = AdvancedAi::new();
    assert!(
        ai.mission_partner_open(&game, 0, 1),
        "off, nobody is skipped"
    );
    assert!(!ai.mission_cadence_due(&game, 0), "off, no extra turns");

    ai.enable_delegations_reach_every_rival();
    assert!(!ai.mission_partner_open(&game, 0, 1), "major 1 holds ours");
    assert!(ai.mission_partner_open(&game, 0, 2));
    assert!(ai.mission_cadence_due(&game, 0));

    // With Diplomatic Service the Embassy supersedes the Delegation.
    game.players[0]
        .civics
        .insert(crate::name!("diplomatic_service"));
    assert!(ai.mission_partner_open(&game, 0, 1));
}

#[test]
fn a_rival_just_asked_waits_before_the_next_ask() {
    let mut game = three_majors();
    let mut ai = AdvancedAi::new();
    ai.record_mission_ask(&game, &Action::SendDelegation { player: 2 });
    ai.enable_delegations_reach_every_rival();
    assert!(ai.mission_partner_open(&game, 0, 2), "off, no ask was kept");

    ai.record_mission_ask(&game, &Action::SendDelegation { player: 2 });
    assert!(!ai.mission_partner_open(&game, 0, 2), "asked this turn");
    game.turn += game.standard_duration(MISSION_RETRY_TURNS);
    assert!(ai.mission_partner_open(&game, 0, 2), "the wait is over");
}

fn peaceful_culture_plan(game: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    }
}

#[test]
fn a_neutral_contact_gets_a_mission_during_its_only_odd_turn_window() {
    for enabled in [false, true] {
        let mut game = three_majors();
        game.turn = 17;
        game.players[1]
            .observed_diplomatic_state
            .insert(0, "UNFRIENDLY".into());
        game.players[2]
            .observed_diplomatic_state
            .insert(0, "NEUTRAL".into());
        let plan = peaceful_culture_plan(&game);
        let mut ai = AdvancedAi::new();
        if enabled {
            ai.enable_delegations_reach_every_rival();
        }

        ai.advanced_diplomacy(&mut game, 0, &plan);

        assert_eq!(game.diplomatic_mission_to(0, 2).is_some(), enabled);
        assert!(game.diplomatic_mission_to(0, 1).is_none());
        // By the next turn the live host can already refuse this rival. The
        // delegation must have been sent while the Neutral window existed.
        game.turn += 1;
        game.players[2]
            .observed_diplomatic_state
            .insert(0, "UNFRIENDLY".into());
        let next_plan = peaceful_culture_plan(&game);
        ai.advanced_diplomacy(&mut game, 0, &next_plan);
        assert_eq!(game.diplomatic_mission_to(0, 2).is_some(), enabled);
        assert!(game.diplomatic_mission_to(0, 1).is_none());
    }
}

#[test]
fn host_refused_partners_do_not_consume_the_mission_or_retry_clock() {
    for mood in ["UNFRIENDLY", "DENOUNCED", "WAR"] {
        let mut game = three_majors();
        for other in [1, 2] {
            game.players[other]
                .observed_diplomatic_state
                .insert(0, mood.into());
        }
        let plan = peaceful_culture_plan(&game);
        let mut ai = AdvancedAi::new();
        ai.enable_delegations_reach_every_rival();

        ai.advanced_diplomacy(&mut game, 0, &plan);

        assert!(game.diplomatic_mission_to(0, 1).is_none());
        assert!(game.diplomatic_mission_to(0, 2).is_none());
        assert!(ai.delegations_asked.is_empty());
    }
}
