use super::super::AdvancedAi;
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
