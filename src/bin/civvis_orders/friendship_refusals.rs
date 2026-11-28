//! Friendship asks the host refused, handed to the planner (gene
//! `refused-friendship-waits`, `src/ai/advanced/refused_friendship_waits.rs`).
//!
//! The order channel already keeps the turn of every order's latest failure
//! (`HostOrderRefusals::failed_on`); a `friendship` order's subject is the
//! host player it asked. This reads those turns and puts them on the board's
//! seats, so the planner can pass over a major that refused it instead of
//! asking it again on the next turn.

use std::collections::BTreeMap;

/// The turn of each major's latest refused friendship ask, by board seat,
/// within [`super::ORDER_REFUSAL_COOLDOWN_TURNS`] of `turn`. `seats` is the
/// host player id to board seat map (`mirror::host_major_seat_map`).
pub(super) fn refused_seats(
    seats: &BTreeMap<usize, usize>,
    turn: u32,
    refusals: &super::HostOrderRefusals,
) -> BTreeMap<usize, u32> {
    let mut refused: BTreeMap<usize, u32> = BTreeMap::new();
    for ((kind, _, subject, _), failed) in &refusals.failed_on {
        if kind != "friendship"
            || turn.saturating_sub(*failed) >= super::ORDER_REFUSAL_COOLDOWN_TURNS
        {
            continue;
        }
        let Some(seat) = subject
            .and_then(|host| usize::try_from(host).ok())
            .and_then(|host| seats.get(&host))
        else {
            continue;
        };
        let latest = refused.entry(*seat).or_insert(*failed);
        *latest = (*latest).max(*failed);
    }
    refused
}

#[cfg(test)]
mod tests {
    use super::super::{HostOrderRefusals, IssuedOrder, OrderCheck, Verdict};
    use super::*;

    fn friendship(player: i64) -> IssuedOrder {
        IssuedOrder {
            kind: "friendship".to_string(),
            subject: Some(player),
            verb: Some("DECLARE_FRIEND".to_string()),
            pos: None,
        }
    }

    fn check(player: i64, verdict: Verdict) -> OrderCheck {
        OrderCheck {
            order: friendship(player),
            verdict,
        }
    }

    /// `selfplay-duel-20261127T0654Z` seat p1 is host player 1; host player
    /// 0 sits on board seat 1.
    fn duel_seats() -> BTreeMap<usize, usize> {
        BTreeMap::from([(1, 0), (0, 1)])
    }

    #[test]
    fn a_refused_ask_is_handed_to_the_planner_by_seat_until_the_cooldown() {
        let mut refusals = HostOrderRefusals::default();
        refusals.observe(&[check(0, Verdict::Failed("not_friends".into()))], 51);
        assert_eq!(
            refused_seats(&duel_seats(), 52, &refusals),
            BTreeMap::from([(1, 51)])
        );
        assert!(refused_seats(
            &duel_seats(),
            51 + super::super::ORDER_REFUSAL_COOLDOWN_TURNS,
            &refusals
        )
        .is_empty());
    }

    #[test]
    fn a_verified_ask_and_other_orders_are_not_refusals() {
        let mut refusals = HostOrderRefusals::default();
        refusals.observe(&[check(0, Verdict::Failed("not_friends".into()))], 51);
        refusals.observe(&[check(0, Verdict::Verified)], 52);
        refusals.observe(
            &[OrderCheck {
                order: IssuedOrder {
                    kind: "delegation".to_string(),
                    subject: Some(0),
                    verb: Some("DIPLOMATIC_DELEGATION".to_string()),
                    pos: None,
                },
                verdict: Verdict::Failed("delegation_not_offered".into()),
            }],
            52,
        );
        assert!(refused_seats(&duel_seats(), 53, &refusals).is_empty());
    }

    #[test]
    fn an_unmapped_player_is_left_out() {
        let mut refusals = HostOrderRefusals::default();
        refusals.observe(&[check(7, Verdict::Failed("not_friends".into()))], 51);
        assert!(refused_seats(&duel_seats(), 52, &refusals).is_empty());
    }
}
