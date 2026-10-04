//! Corps and Armies the host refused, handed to the planner (gene
//! `formations-heed-refusals`).
//!
//! The planner combines two units on its own board (`do_combine_units`
//! spends both units' turn) and sends `FORM_CORPS` or `FORM_ARMY`. When the
//! host refuses, the next fresh board has the two units apart again, the
//! planner combines them again, and the pair loses another turn. After
//! [`super::ORDER_REFUSAL_STRIKES`] refusals the order is withheld, but the
//! combine is still planned, so both units stay idle. Live King
//! civvis-20261004T100903Z (game 52): two Rocket Artillery in Loja were
//! refused "military_formation_tier_too_low" on turns 241-243 and stood
//! there until the game was lost at 257, during the war on India.

use std::collections::{BTreeMap, BTreeSet};

/// The board's unit pairs, lower id first, whose `FORM_CORPS` or
/// `FORM_ARMY` the host refused within
/// [`super::ORDER_REFUSAL_COOLDOWN_TURNS`] of `turn`. `mapped` is the
/// board's unit id to host id map.
pub(super) fn refused_pairs(
    mapped: &BTreeMap<u32, i64>,
    turn: u32,
    refusals: &super::HostOrderRefusals,
) -> BTreeSet<(u32, u32)> {
    let uid_of: BTreeMap<i64, u32> = mapped.iter().map(|(uid, host)| (*host, *uid)).collect();
    refusals
        .failed_on
        .iter()
        .filter_map(|((kind, verb, subject, pos), failed)| {
            if kind != "unit"
                || !matches!(verb.as_deref(), Some("FORM_CORPS" | "FORM_ARMY"))
                || turn.saturating_sub(*failed) >= super::ORDER_REFUSAL_COOLDOWN_TURNS
            {
                return None;
            }
            // `CombineUnits` sends the partner's host id as the order's y.
            let a = *uid_of.get(&(*subject)?)?;
            let b = *uid_of.get(&i64::from((*pos)?.1))?;
            Some((a.min(b), a.max(b)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::{HostOrderRefusals, IssuedOrder, OrderCheck, Verdict};
    use super::*;

    fn corps(subject: i64, partner: i32) -> IssuedOrder {
        IssuedOrder {
            kind: "unit".to_string(),
            subject: Some(subject),
            verb: Some("FORM_CORPS".to_string()),
            pos: Some((0, partner)),
        }
    }

    #[test]
    fn a_refused_corps_is_handed_to_the_planner_until_the_cooldown() {
        let mapped = BTreeMap::from([(7, 18939942), (3, 19988543), (5, 111)]);
        let mut refusals = HostOrderRefusals::default();
        refusals.observe(
            &[OrderCheck {
                order: corps(18939942, 19988543),
                verdict: Verdict::Failed("military_formation_tier_too_low".to_string()),
            }],
            241,
        );
        assert_eq!(
            refused_pairs(&mapped, 242, &refusals),
            BTreeSet::from([(3, 7)]),
            "one refusal is enough: each costs both units a turn"
        );
        assert!(refused_pairs(
            &mapped,
            241 + super::super::ORDER_REFUSAL_COOLDOWN_TURNS,
            &refusals
        )
        .is_empty());
        // A verified corps clears the record.
        refusals.observe(
            &[OrderCheck {
                order: corps(18939942, 19988543),
                verdict: Verdict::Verified,
            }],
            242,
        );
        assert!(refused_pairs(&mapped, 243, &refusals).is_empty());
    }
}
