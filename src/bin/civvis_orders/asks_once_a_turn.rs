//! `asks-go-once-a-turn`: a diplomatic ask, a deal or a Trader route goes to
//! the host once in a host turn, not once in every frame of it.
//!
//! A turn the mod replans (`replan_frame`: new tiles revealed, a blow landed)
//! asks the decider again, and the decider re-derives every order from a
//! board on which the first frame's asks have not changed anything it reads.
//! For a unit's move that is right: the board moved. For these orders it is
//! not. A delegation or embassy the host accepted in the first frame is no
//! longer offered (`IsDiplomaticActionValid("DIPLOACTION_" .. verb)` is the
//! mod's own gate, `CivvisControlAgent.lua`, `delegation_not_offered`); a
//! deal sits pending or in the mod's retry window (`buy_host_pending`,
//! `buy_cooldown`); a friendship has its five-turn window; a refused one is
//! refused for the same reason again. Nothing in the turn's second frame can
//! change those answers, and each repeat costs the frame and a refusal.
//!
//! **Why (hotseat `hotseat-20261128T031456Z`, 2026-11-27).** On turn 16 three
//! seats sent a delegation in frame 0 and the same one in frame 1: p2 to
//! seat 5 and p4 to seat 0 had theirs accepted in frame 0 (`orders` `by`
//! `delegation: 1`) and refused `delegation_not_offered` in frame 1; p5's to
//! seat 2 was refused in both. p2 asked seat 5 for a Great Work on turn 15 in
//! both frames (refused `buy_no_great_work`, then `buy_cooldown`; two
//! `no_deal_response` verdicts). p3 sent its Trader to (12,21) in both frames
//! of turn 12 (`trade_route_refused` twice, `no_route` twice). In the 119-turn
//! self-play duel `selfplay-duel-20261127T0654Z` seat p1 asked the other seat
//! for a friendship two or three times a turn from turn 51 (105 journal
//! offers, 32 refused `friendship_not_offered` by the mod).
//!
//! `suppress_same_turn_replays` already kept a second deal per rival out of
//! a turn, but the Great Work purchase is appended after it runs, and it
//! covers no other ask.
//!
//! **The rule.** Each frame, after every other filter, an ask whose key an
//! earlier frame of the same host turn already sent is dropped, and the reply
//! names it (`asks_sent_this_turn=`). The keys: a delegation, friendship,
//! denouncement or alliance by its kind, verb and rival; any deal (`buy` or
//! `sell`) by its rival alone, because the host holds one working deal per
//! rival; a Trader's route by its unit and destination. Envoys are left out
//! (two to one city-state are two real orders), as are war and peace (the
//! peace arm has its own host cooldown, `HostPeaceRetries`) and every other
//! unit order (a move or a blow on a replanned board is a new order). A new
//! host turn starts the memory again. Off, nothing is read or kept.

use super::*;

/// The diplomatic kinds a later frame of one turn never asks again.
const ONCE_A_TURN_KINDS: &[&str] = &["delegation", "friendship", "denounce", "alliance"];

/// What an ask is remembered by: kind, verb, rival or unit, destination.
type AskKey = (
    &'static str,
    Option<String>,
    Option<i64>,
    Option<(i32, i32)>,
);

/// The key `order` is remembered by, or `None` for an order this rule
/// leaves alone.
fn ask_key(order: &Order) -> Option<AskKey> {
    match order.kind {
        kind if ONCE_A_TURN_KINDS.contains(&kind) => {
            Some((kind, order.verb.clone(), order.subject, None))
        }
        // One working deal per rival: whatever is asked of them is one ask.
        "buy" | "sell" => Some(("deal", None, order.subject, None)),
        "unit" if order.verb.as_deref() == Some("TRADE_ROUTE") => {
            Some(("unit", order.verb.clone(), order.subject, order.pos))
        }
        _ => None,
    }
}

/// The asks sent so far in one host turn.
#[derive(Default)]
pub(super) struct Asks {
    turn: Option<u32>,
    sent: std::collections::BTreeSet<AskKey>,
}

impl Asks {
    /// Drop from `orders` every ask an earlier frame of host turn `turn`
    /// already sent, then remember the asks this frame sends. Returns one
    /// `kind:verb subject` line per dropped order.
    pub(super) fn keep_once(&mut self, orders: &mut Vec<Order>, turn: u32) -> Vec<String> {
        if self.turn != Some(turn) {
            self.turn = Some(turn);
            self.sent.clear();
        }
        let sent = &self.sent;
        let mut dropped = Vec::new();
        orders.retain(|order| match ask_key(order) {
            Some(key) if sent.contains(&key) => {
                dropped.push(format!(
                    "{}:{} {}",
                    order.kind,
                    order.verb.as_deref().unwrap_or("-"),
                    order
                        .subject
                        .map_or_else(|| "-".to_string(), |s| s.to_string())
                ));
                false
            }
            _ => true,
        });
        dropped
    }
}

#[cfg(test)]
#[path = "asks_once_a_turn/tests.rs"]
mod tests;
