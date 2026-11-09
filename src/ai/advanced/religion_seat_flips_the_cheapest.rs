//! `religion-seat-flips-the-cheapest`: a Religion-target founder's spreaders
//! go first where a charge buys the most of the victory.
//!
//! A Religious victory counts each major's cities once, however large they
//! are: it asks for a strict majority of cities in every living major. The
//! shipped target score pays `pop x 12` and `capital x 18`, so the corps walks
//! to the largest cities, which hold the most followers to convert, and it
//! does not read how near any civilization is to its majority.
//!
//! With the gene on, for a foreign city this seat's spreader may target:
//! - the shipped population and capital terms are taken back and a small city
//!   is preferred;
//! - a city with no majority religion is preferred, since only atheist
//!   pressure holds it;
//! - the civilization with the fewest cities still to convert comes first, so
//!   a majority lands sooner and its cities start pressing on their neighbours.
//!
//! City counts prefer the host's public figure, because the board holds only
//! the cities this seat has seen.
//!
//! Off, for a seat aiming elsewhere, or for a city of ours, nothing changes.

use super::*;

/// The shipped target score's weight on population (`advanced_missionary_step`).
const SHIPPED_POP_WEIGHT: i32 = 12;

/// The shipped target score's bonus for a capital.
const SHIPPED_CAPITAL_BONUS: i32 = 18;

/// Paid against each citizen, on top of taking the shipped weight back.
const POP_CHARGE: i32 = 6;

/// Paid for each city its owner still needs for our majority.
const FLIP_NEEDED_CHARGE: i32 = 30;

/// More cities still to convert than this all read alike.
const FLIP_NEEDED_CAP: i32 = 8;

/// Paid to a city no religion holds a majority of.
const NO_MAJORITY_BONUS: i32 = 100;

impl AdvancedAi {
    /// How many more of `owner`'s cities must follow `religion` for a strict
    /// majority.
    fn flips_needed(g: &Game, owner: usize, religion: &str) -> i32 {
        let known = g.player_city_ids(owner);
        let cities = g
            .observed_public_empire_stats
            .get(&owner)
            .and_then(|stats| stats.city_count)
            .map_or(known.len(), |count| count.max(known.len()));
        let following = known
            .iter()
            .filter(|cid| g.city_religion(&g.cities[cid]) == Some(religion))
            .count();
        (cities / 2 + 1).saturating_sub(following) as i32
    }

    /// The adjustment to the spreader's target score. Zero with the gene off,
    /// on another target, for another faith's spreader or for our own city.
    pub(super) fn religion_seat_flip_adjustment(
        &self,
        g: &Game,
        pid: usize,
        city: &crate::game::City,
        religion: &str,
    ) -> i32 {
        if !self.religion_seat_flips_the_cheapest
            || city.owner == pid
            || self.active_victory_target(g) != Some(VictoryTarget::Religion)
            || g.players[pid].religion.as_deref() != Some(religion)
        {
            return 0;
        }
        let pop = city.pop.max(0);
        let shipped = pop * SHIPPED_POP_WEIGHT + i32::from(city.is_capital) * SHIPPED_CAPITAL_BONUS;
        let unclaimed = if g.city_religion(city).is_none() {
            NO_MAJORITY_BONUS
        } else {
            0
        };
        unclaimed
            - shipped
            - pop * POP_CHARGE
            - Self::flips_needed(g, city.owner, religion).min(FLIP_NEEDED_CAP) * FLIP_NEEDED_CHARGE
    }
}

#[cfg(test)]
mod tests;
