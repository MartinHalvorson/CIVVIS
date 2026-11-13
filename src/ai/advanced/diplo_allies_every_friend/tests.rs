use super::super::test_support::opt_in_off_in_both_controllers;
use super::*;
use crate::ai::advanced::VictoryTarget;
use crate::game::{Action, Game};

/// Four majors, every pair met, Civil Service everywhere, nobody holding the
/// Research kind's technology, seats 1 and 2 our declared friends for the
/// whole test and seat 3 a stranger.
fn board() -> Game {
    let mut game = Game::new_full(4, 32, 20, 7_923, 300, 0, false);
    for pid in 0..4 {
        for other in pid + 1..4 {
            game.record_contact(pid, other);
        }
    }
    for player in game.players.iter_mut().filter(|p| !p.is_minor) {
        player.civics.insert(crate::name!("civil_service"));
    }
    game.turn = 100;
    game.current = 0;
    befriend(&mut game, 1);
    befriend(&mut game, 2);
    game
}

fn befriend(game: &mut Game, other: usize) {
    game.players[0].friends_until.insert(other, 280);
    game.players[other].friends_until.insert(0, 280);
}

fn seat() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_diplo_seat_allies_every_friend();
    ai.victory_target = Some(VictoryTarget::Diplomacy);
    ai
}

fn asks(game: &Game) -> Vec<(usize, Option<String>, bool, bool)> {
    game.pending_deals
        .iter()
        .filter(|deal| deal.from == 0)
        .map(|deal| {
            (
                deal.to,
                deal.alliance.clone(),
                deal.friendship,
                deal.open_borders,
            )
        })
        .collect()
}

fn answer(game: &mut Game, accept: bool) {
    let deal = game
        .pending_deals
        .iter()
        .find(|deal| deal.from == 0)
        .expect("an ask is pending");
    let (id, to) = (deal.id, deal.to);
    game.current = to;
    let action = if accept {
        Action::AcceptDeal { deal: id }
    } else {
        Action::RejectDeal { deal: id }
    };
    game.apply(to, &action).expect("the partner answers");
    game.current = 0;
}

#[test]
fn diplo_seat_allies_every_friend_is_opt_in() {
    opt_in_off_in_both_controllers("diplo-seat-allies-every-friend", |ai| {
        ai.diplo_seat_allies_every_friend
    });
}

/// Both declared friends end up allied, each with a different kind, and the
/// stranger is never asked. Off the gene, off the Diplomatic lane, on a lost
/// lane and before Civil Service nothing is asked.
#[test]
fn every_declared_friend_is_asked_for_a_kind_of_its_own() {
    let mut game = board();
    let mut off = AdvancedAi::new();
    off.victory_target = Some(VictoryTarget::Diplomacy);
    assert!(!off.diplo_allies_every_friend_step(&mut game, 0, None));
    let mut science = seat();
    science.victory_target = Some(VictoryTarget::Science);
    assert!(!science.diplo_allies_every_friend_step(&mut game, 0, None));
    let mut lost = seat();
    lost.lane_lost = true;
    assert!(!lost.diplo_allies_every_friend_step(&mut game, 0, None));
    let mut early = game.clone();
    early.players[0]
        .civics
        .remove(&crate::name!("civil_service"));
    assert!(!seat().diplo_allies_every_friend_step(&mut early, 0, None));
    assert!(asks(&game).is_empty() && asks(&early).is_empty());

    // Research is legal for nobody here, so the first kind is Economic. The
    // friendship is bundled, as every desk does; passage is not.
    let mut ai = seat();
    assert!(ai.diplo_allies_every_friend_step(&mut game, 0, None));
    assert_eq!(
        asks(&game),
        vec![(1, Some("economic".to_string()), true, false)]
    );
    assert_eq!(
        game.players[0]
            .counters
            .get("diplo_allies_every_friend:asked")
            .copied(),
        Some(1)
    );
    // An ask in flight: nothing more this turn or the next.
    assert!(!ai.diplo_allies_every_friend_step(&mut game, 0, None));
    game.turn += 1;
    assert!(!ai.diplo_allies_every_friend_step(&mut game, 0, None));
    answer(&mut game, true);
    assert_eq!(
        game.alliance_with(0, 1).map(|a| a.kind.as_str()),
        Some("economic")
    );

    // The second friend: Economic is ours already, so Cultural.
    assert!(ai.diplo_allies_every_friend_step(&mut game, 0, None));
    assert_eq!(
        asks(&game),
        vec![(2, Some("cultural".to_string()), true, false)]
    );
    answer(&mut game, true);
    assert_eq!(
        game.alliance_with(0, 2).map(|a| a.kind.as_str()),
        Some("cultural")
    );

    // Both friends allied; the stranger is not asked.
    game.turn += 1;
    assert!(!ai.diplo_allies_every_friend_step(&mut game, 0, None));
    assert!(asks(&game).is_empty());
    assert_eq!(ai.diplo_ally_candidate(&game, 0, None), None);
}

/// Research comes first where both trees allow it, and the plan's denial
/// partner is never asked.
#[test]
fn research_is_the_first_kind_where_both_sides_can_sign_it() {
    let mut game = board();
    for pid in [0, 2] {
        game.players[pid]
            .techs
            .insert(crate::name!("scientific_theory"));
    }
    let ai = seat();
    // Friend 1 cannot sign Research and is offered Economic; friend 2 can,
    // and the earlier kind is asked first.
    assert_eq!(
        ai.diplo_ally_candidate(&game, 0, None),
        Some((2, "research"))
    );
    assert_eq!(
        ai.diplo_ally_candidate(&game, 0, Some(2)),
        Some((1, "economic"))
    );
    assert_eq!(
        ai.diplo_ally_candidate(&game, 0, Some(1)),
        Some((2, "research"))
    );
}

/// A friend that did not ally is asked again fifteen turns later, for the
/// next kind; the kind it turned down waits thirty turns. The other friend
/// is asked the turn after the first.
#[test]
fn a_friend_that_refused_is_asked_later_for_another_kind() {
    let mut game = board();
    let mut ai = seat();
    assert!(ai.diplo_allies_every_friend_step(&mut game, 0, None));
    assert_eq!(asks(&game)[0].0, 1);
    answer(&mut game, false);
    // The same turn, a second decision (the live board carries no pending
    // deal): one ask a turn.
    assert!(!ai.diplo_allies_every_friend_step(&mut game, 0, None));
    game.turn = 101;
    assert!(ai.diplo_allies_every_friend_step(&mut game, 0, None));
    assert_eq!(
        asks(&game),
        vec![(2, Some("economic".to_string()), true, false)]
    );
    answer(&mut game, false);
    for turn in 102..115 {
        game.turn = turn;
        assert!(
            !ai.diplo_allies_every_friend_step(&mut game, 0, None),
            "turn {turn}: both friends were asked inside fifteen turns"
        );
    }
    game.turn = 115;
    assert!(ai.diplo_allies_every_friend_step(&mut game, 0, None));
    assert_eq!(
        asks(&game),
        vec![(1, Some("cultural".to_string()), true, false)],
        "Economic was turned down fifteen turns ago and waits thirty"
    );
    answer(&mut game, false);
    game.turn = 130;
    assert_eq!(
        ai.diplo_ally_candidate(&game, 0, None),
        Some((1, "economic")),
        "thirty turns after the Economic ask it is open again"
    );
}

/// A turn on which the stock desk asked is not a turn for a second ask, and
/// a partner the stock desk asked waits the same fifteen turns.
#[test]
fn it_stands_down_on_a_turn_another_desk_asked() {
    let mut game = board();
    let mut ai = seat();
    ai.enable_alliance_remembers_the_refusal();
    ai.alliance_note_asked(&game, 0, 1);
    assert!(!ai.diplo_allies_every_friend_step(&mut game, 0, None));
    assert!(asks(&game).is_empty());
    game.turn = 101;
    assert!(ai.diplo_allies_every_friend_step(&mut game, 0, None));
    assert_eq!(asks(&game)[0].0, 2, "friend 1 was asked a turn ago");
    answer(&mut game, false);
    game.turn = 115;
    assert_eq!(
        ai.diplo_ally_candidate(&game, 0, None).map(|ask| ask.0),
        Some(1)
    );
}

/// On the live board the host's own reading of the leader decides: a rival
/// the mirror holds a friendship clock for but the host reads as Friendly is
/// not asked, and one it reads as a declared friend is.
#[test]
fn the_hosts_reading_of_the_friendship_decides() {
    let mut game = board();
    game.players[1]
        .observed_diplomatic_state
        .insert(0, "FRIENDLY".to_string());
    game.players[2]
        .observed_diplomatic_state
        .insert(0, "DECLARED_FRIEND".to_string());
    game.players[3]
        .observed_diplomatic_state
        .insert(0, "DECLARED_FRIEND".to_string());
    let ai = seat();
    assert!(!AdvancedAi::diplo_ally_declared_friend(&game, 0, 1));
    assert!(AdvancedAi::diplo_ally_declared_friend(&game, 0, 3));
    assert_eq!(
        ai.diplo_ally_candidate(&game, 0, None),
        Some((2, "economic"))
    );
    assert_eq!(
        ai.diplo_ally_candidate(&game, 0, Some(2)),
        Some((3, "economic"))
    );
    // At war, denounced or already allied: not a candidate.
    game.at_war.insert((0, 2));
    game.players[3].denounced_until.insert(0, 140);
    assert_eq!(ai.diplo_ally_candidate(&game, 0, None), None);
}

/// The renewal: while the alliance stands the partner is left alone, and the
/// turn it lapses the same friend is asked again, the same kind included.
#[test]
fn a_lapsed_alliance_is_asked_again_at_once() {
    let mut game = board();
    game.players[0].friends_until.remove(&2);
    game.players[2].friends_until.remove(&0);
    let mut ai = seat();
    assert!(ai.diplo_allies_every_friend_step(&mut game, 0, None));
    answer(&mut game, true);
    let ends = game.alliance_with(0, 1).expect("allied").ends;
    game.turn = ends - 1;
    assert!(!ai.diplo_allies_every_friend_step(&mut game, 0, None));
    game.turn = ends;
    assert!(game.alliance_with(0, 1).is_none());
    befriend(&mut game, 1);
    assert!(ai.diplo_allies_every_friend_step(&mut game, 0, None));
    assert_eq!(
        asks(&game),
        vec![(1, Some("economic".to_string()), true, false)]
    );
}
