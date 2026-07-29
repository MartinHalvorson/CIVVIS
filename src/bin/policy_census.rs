//! Count the policy cards the AI actually plays, against the ones the game offers.
//!
//! `data/policies.json` carries 125 cards. `ai::POLICY_PRIORITY` names twenty,
//! in a fixed order, and the slotting loop runs only while a slot is empty --
//! so a deck fills once and `policies_fit` then rejects every later card for
//! the rest of the game. Nothing in the repository has ever measured what that
//! costs, because no instrument reported on the card layer at all.
//!
//! This is that instrument. It plays whole games with the stock fleet and, at
//! every turn, records each major's slate. Three numbers come out:
//!
//! - **reach** -- distinct cards ever slotted, against the 125 that exist and
//!   against the cards that were legally available at some point in that game.
//!   A card the empire unlocked and never played is a measured miss, not a
//!   design choice; the priority list simply does not name it.
//! - **churn** -- turns on which a slate changed. A deck that answers the game
//!   changes when the game changes. A deck that is a constant does not.
//! - **idle slots** -- slot-turns left empty while a legal card was available.
//!   This separates "the list is short" from "the list ran out".
//!
//! Every number is per major player per game, then pooled. Minors are excluded:
//! `available_policies` returns nothing for them.
//!
//! ```text
//! policy_census --players 4 --maps 12
//! ```
//!
//! Like `search_probe` this is a screen, not a verdict. It measures what the
//! agent does, never whether doing it wins -- that costs a paired `ai_eval`.
use std::collections::{BTreeMap, BTreeSet};

use civvis::ai::{Ai, AdvancedAi};
use civvis::game::{Action, Game};
use civvis::parallel;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// One major player's card history over one game.
#[derive(Default)]
struct Seat {
    slotted: BTreeSet<String>,
    offered: BTreeSet<String>,
    /// The slate as of the previous turn, so a change can be seen as a change.
    last: BTreeSet<String>,
    changes: usize,
    slot_turns: i64,
    filled_turns: i64,
    /// Slot-turns that sat empty while a legal card was waiting to fill them.
    idle_with_offer: i64,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 12);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 200) as u32;
    let seed0 = number(&args, "--seed", 200_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());

    let games = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let mut game = Game::new(players, width, height, seed, turns, 0);
        let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet(&game);
        let majors: Vec<usize> = (0..game.players.len())
            .filter(|pid| !game.players[*pid].is_minor)
            .collect();
        let mut seats: BTreeMap<usize, Seat> = majors
            .iter()
            .map(|pid| (*pid, Seat::default()))
            .collect();

        for _ in 0..turns {
            if game.winner.is_some() {
                break;
            }
            for pid in 0..game.players.len() {
                if game.winner.is_some() {
                    break;
                }
                fleet[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }
            for pid in &majors {
                let held: BTreeSet<String> = game.players[*pid].policies.iter().cloned().collect();
                let offered = game.available_policies(*pid);
                let slots = game.gov_slots(*pid);
                let total = slots.military + slots.economic + slots.diplomatic + slots.wildcard;
                let seat = seats.get_mut(pid).expect("major seat recorded at setup");
                seat.slot_turns += total;
                seat.filled_turns += held.len() as i64;
                let empty = total - held.len() as i64;
                if empty > 0 && !offered.is_empty() {
                    seat.idle_with_offer += empty.min(offered.len() as i64);
                }
                for card in &offered {
                    seat.offered.insert(card.clone());
                }
                for card in &held {
                    seat.offered.insert(card.clone());
                }
                if held != seat.last {
                    if !seat.last.is_empty() || !held.is_empty() {
                        seat.changes += 1;
                    }
                    seat.last = held.clone();
                }
                for card in held {
                    seat.slotted.insert(card);
                }
            }
        }
        seats.into_values().collect::<Vec<Seat>>()
    });

    let seats: Vec<Seat> = games.into_iter().flatten().collect();
    let n = seats.len().max(1) as f64;
    let catalogue = Game::new(2, 20, 14, 1, 10, 0).rules.policies.len();

    let reach: f64 = seats.iter().map(|s| s.slotted.len() as f64).sum::<f64>() / n;
    let offered: f64 = seats.iter().map(|s| s.offered.len() as f64).sum::<f64>() / n;
    let changes: f64 = seats.iter().map(|s| s.changes as f64).sum::<f64>() / n;
    let fill: f64 = seats.iter().map(|s| s.filled_turns as f64).sum::<f64>()
        / seats.iter().map(|s| s.slot_turns.max(1) as f64).sum::<f64>();
    let idle: f64 = seats.iter().map(|s| s.idle_with_offer as f64).sum::<f64>() / n;

    let mut ever: BTreeSet<&str> = BTreeSet::new();
    for seat in &seats {
        for card in &seat.slotted {
            ever.insert(card.as_str());
        }
    }

    println!("policy census over {} seat-games", seats.len());
    println!("  catalogue                {catalogue} cards");
    println!("  ever slotted, any seat   {} cards", ever.len());
    println!("  distinct cards per seat  {reach:.2}");
    println!("  cards unlocked per seat  {offered:.2}");
    println!("  unlocked and never played {:.2}", offered - reach);
    println!("  slate changes per game   {changes:.2}");
    println!("  slot occupancy           {:.1}%", fill * 100.0);
    println!("  idle slot-turns w/ offer {idle:.1}");
}
