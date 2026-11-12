//! `strike_reach_of` reads the board it is given. It used to be handed a
//! copy of the game, write the hostile's turn state fresh on it, flood, and
//! write it back; `strike_reach_by_standing_the_hostile_up` is that route,
//! kept for these tests. The two must return the same tiles for every unit
//! in every state a game leaves one in.

use super::*;
use crate::ai::{run_game_observed, Ai, BasicAi};

/// The world at five turns of one game: three majors seated as the site
/// seats them, two city-states and the barbarians. The observer sees each
/// turn before its first seat acts, so every other seat's units carry
/// whatever their last turn left on them.
fn played_boards() -> Vec<Game> {
    let mut game = Game::new_with(crate::game::GameOptions::new(3, 40, 26, 20_261_112, 80, 2));
    let mut ais: Vec<Box<dyn Ai + Send>> = game
        .players
        .iter()
        .map(|player| -> Box<dyn Ai + Send> {
            if player.is_minor || player.is_barbarian {
                Box::new(BasicAi::new())
            } else {
                let mut ai = AdvancedAi::new();
                ai.enable_site_deployment();
                Box::new(ai)
            }
        })
        .collect();
    let mut boards = Vec::new();
    run_game_observed(&mut game, &mut ais, |board| {
        if [25, 40, 55, 70, 80].contains(&board.turn) {
            boards.push(board.clone());
        }
    });
    assert_eq!(boards.len(), 5, "fixture: the game reaches turn 80");
    boards
}

fn fighters(g: &Game) -> Vec<u32> {
    g.units
        .values()
        .filter(|unit| {
            let spec = &g.rules.units[unit.kind];
            spec.class == "military" && (spec.is_melee_capable() || spec.has_ranged_attack())
        })
        .map(|unit| unit.id)
        .collect()
}

/// Both routes for one unit, compared. Returns the reach.
fn both_routes(g: &Game, uid: u32, what: &str) -> Vec<Pos> {
    let owner = g.units[&uid].owner;
    let viewer = g
        .players
        .iter()
        .map(|player| player.id)
        .find(|pid| *pid != owner)
        .expect("a second seat");
    let read = strike_reach_of(g, viewer, uid);
    let mut copy = g.clone();
    let stood = strike_reach_by_standing_the_hostile_up(&mut copy, viewer, uid);
    assert_eq!(
        read,
        stood,
        "turn {}: {} {uid} of seat {owner} at {:?}, {what}",
        g.turn,
        g.units[&uid].kind.as_str(),
        g.units[&uid].pos
    );
    read
}

/// Every fighting unit on every sampled board, exactly as the game left it:
/// movement spent, stopped by a zone of control, having attacked, embarked.
#[test]
fn the_board_read_matches_standing_the_hostile_up_for_every_unit_of_a_game() {
    let (mut units, mut reaching, mut spent) = (0, 0, 0);
    for board in played_boards() {
        for uid in fighters(&board) {
            let reach = both_routes(&board, uid, "as played");
            let unit = &board.units[&uid];
            units += 1;
            reaching += usize::from(!reach.is_empty());
            spent += usize::from(
                unit.moves_left < board.unit_max_moves(uid)
                    || unit.acted
                    || unit.moved
                    || unit.zoc_stopped,
            );
        }
    }
    assert!(units >= 30, "fixture: only {units} fighting units");
    assert!(reaching >= 10, "fixture: {reaching} of {units} reach");
    assert!(spent >= 1, "fixture: no unit had spent a turn");
}

/// The lock a linked peer puts on its leader is the one part of the turn
/// state the board read still consults, and it is read off the *peer* as the
/// peer stands. Link a civilian under each land fighter and set the peer's
/// and the leader's flags every way round.
#[test]
fn a_linked_peers_zone_of_control_lock_is_read_the_same_both_ways() {
    let (mut linked, mut locked_by_the_peer) = (0, 0);
    for board in played_boards() {
        for uid in fighters(&board).into_iter().take(24) {
            let leader = board.units[&uid].clone();
            // A land unit's domain is unnamed in the rules; sea and air are.
            if board.rules.units[leader.kind].domain.is_some()
                || leader.linked_to.is_some()
                || board
                    .map
                    .get(leader.pos)
                    .is_none_or(|tile| board.rules.is_water(tile))
            {
                continue;
            }
            let mut game = board.clone();
            let peer = game.spawn_unit("builder", leader.owner, leader.pos);
            if game.units[&peer].pos != leader.pos {
                continue;
            }
            game.units.get_mut(&uid).unwrap().linked_to = Some(peer);
            game.units.get_mut(&peer).unwrap().linked_to = Some(uid);
            linked += 1;
            let free = both_routes(&game, uid, "linked, nothing set");
            // (peer zoc_stopped, peer began in a zone and acted, peer moved,
            //  leader's own four flags set)
            for (stopped, acted_in_zone, moved, own) in [
                (true, false, false, false),
                (false, true, false, false),
                (false, true, true, false),
                (true, true, false, true),
                (false, false, false, true),
                (false, true, false, true),
            ] {
                let mut varied = game.clone();
                {
                    let peer = varied.units.get_mut(&peer).unwrap();
                    peer.zoc_stopped = stopped;
                    peer.started_turn_in_zoc = acted_in_zone;
                    peer.acted = acted_in_zone;
                    peer.moved = moved;
                }
                if own {
                    let leader = varied.units.get_mut(&uid).unwrap();
                    leader.zoc_stopped = true;
                    leader.started_turn_in_zoc = true;
                    leader.acted = true;
                    leader.moved = false;
                    leader.moves_left = 0.0;
                }
                let reach = both_routes(
                    &varied,
                    uid,
                    &format!("linked, peer stopped {stopped} acted in a zone {acted_in_zone} moved {moved}, own flags {own}"),
                );
                if stopped && !own {
                    locked_by_the_peer += usize::from(reach.len() < free.len());
                }
            }
        }
    }
    assert!(
        linked >= 10,
        "fixture: only {linked} formations were linked"
    );
    assert!(
        locked_by_the_peer >= 1,
        "fixture: a stopped peer narrowed no reach, so the lock was not exercised"
    );
}
