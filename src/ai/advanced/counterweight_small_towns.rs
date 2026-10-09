//! `counterweight-flips-the-small-towns`: a defensive spreader takes a rival
//! faith's hold on our empire apart from its smallest towns up.
//!
//! A Religious Victory needs more than half of our *cities* in the winner's
//! faith; a town of two counts exactly as much as a capital of ten. The
//! shipped spreader target list (`advanced_missionary_step`) adds twelve a
//! citizen to every target, so a defensive Missionary walks to the largest
//! converted city near it — the one whose majority has had the longest to
//! pile up and needs the most charges to break — and the veto count does
//! not move.
//!
//! Live Emperor G211317Z (pin fb5ac2506) lost on Religion to Orthodoxy at
//! turn 105 with 7 of our 9 cities Orthodox. Bogotá turned Catholic at 81
//! and bought three Catholic Missionaries (81, 86, 104); five of their
//! charges went into Maracaibo (pop 7 → 10, Orthodox since turn 30) beside
//! the capital, which never flipped, and the rest into Bogotá itself. At
//! turn 95 Cuenca and Angostura (pop 2 each, Orthodox only since 90 and 95)
//! and Guayaquil (pop 3) were the three flips that would have left Orthodoxy
//! four of nine — no majority, no victory. Over the October 8 Emperor
//! Religious defeats with logs, 5 of 12 were faithless seats whose only
//! counter was a Missionary of another faith, and the faith itself (a city
//! of ours on another religion with a Shrine) was the scarce part; this gene
//! spends those charges where the count moves.
//!
//! Under the gene, while a rival-founded faith other than the spreader's
//! holds at least one of our cities (the "majority threat", the faith
//! holding the most of them), our own cities not on the spreader's faith
//! are ranked by what one charge buys toward breaking that majority:
//! - the twelve-a-citizen preference is reversed — the smaller the town,
//!   the earlier it is taken;
//! - a city the threat holds, or a city on no faith the threat is closing
//!   on (it shows the threat's pressure), gains a flat bonus: flipping or
//!   holding it moves the veto count by one;
//! - a city on a third faith loses a penalty: converting it leaves the
//!   threat's count alone.
//!
//! Foreign targets, the spreader's own cities and the recruitment priority
//! (`counterfaith_recruitment_targets`) are untouched; so is everything
//! with no rival faith in our cities. Off: unchanged.

use super::{AdvancedAi, VictoryTarget};
use crate::game::{City, Game};

/// What flipping, or keeping, one city out of the threat faith is worth on
/// top of the shipped defensive weight: it moves the veto count by one.
const SMALL_TOWN_VETO_BONUS: i32 = 30;

/// Taken from an own city on a third faith: converting it leaves the
/// threat's count where it was.
const SMALL_TOWN_IDLE_PENALTY: i32 = 60;

/// The shipped list's weight per citizen, which the gene turns around.
const SHIPPED_POP_WEIGHT: i32 = 12;

impl AdvancedAi {
    /// The rival-founded faith other than `faith` that holds the most of our
    /// cities, then the most other majors, then by name. `None` with the
    /// gene off, outside the Domination lane, with Religious Victory off, or
    /// while no such faith holds a city of ours.
    pub(super) fn small_town_threat(&self, g: &Game, pid: usize, faith: &str) -> Option<String> {
        if !self.counterweight_flips_the_small_towns
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !g.victory_conditions.religious
        {
            return None;
        }
        let cities = g.player_city_ids(pid);
        let own = g.players[pid].religion.as_deref();
        let mut best: Option<(usize, usize, String)> = None;
        for founder in g
            .players
            .iter()
            .filter(|p| p.id != pid && p.alive && !p.is_minor && !p.is_barbarian)
        {
            let Some(rival) = founder.religion.as_deref() else {
                continue;
            };
            if rival == faith || Some(rival) == own {
                continue;
            }
            let held = cities
                .iter()
                .filter(|cid| g.city_religion(&g.cities[cid]) == Some(rival))
                .count();
            if held == 0 {
                continue;
            }
            let dominated = g
                .players
                .iter()
                .filter(|p| {
                    p.alive
                        && !p.is_minor
                        && !p.is_barbarian
                        && p.id != pid
                        && p.id != founder.id
                        && g.civ_follows_religion(p.id, rival)
                })
                .count();
            let better = best
                .as_ref()
                .is_none_or(|(other_held, other_dominated, name)| {
                    held > *other_held
                        || (held == *other_held
                            && (dominated > *other_dominated
                                || (dominated == *other_dominated && rival < name.as_str())))
                });
            if better {
                best = Some((held, dominated, rival.to_owned()));
            }
        }
        best.map(|(_, _, rival)| rival)
    }

    /// What the gene adds to the shipped score of a spreader of `faith`
    /// aiming at `city`, against `threat` (from [`Self::small_town_threat`]).
    /// Zero without a threat, for a foreign city, and for a city already on
    /// the spreader's faith.
    pub(super) fn small_town_target_adjustment(
        g: &Game,
        pid: usize,
        city: &City,
        faith: &str,
        threat: Option<&str>,
    ) -> i32 {
        let Some(threat) = threat else {
            return 0;
        };
        if city.owner != pid {
            return 0;
        }
        let held = g.city_religion(city);
        if held == Some(faith) {
            return 0;
        }
        // Undo the shipped `+ pop × 12` and charge it instead.
        let resize = -2 * city.pop.max(0) * SHIPPED_POP_WEIGHT;
        let closing = city.pressure.get(threat).copied().unwrap_or(0.0) > 0.0;
        match held {
            Some(religion) if religion == threat => resize + SMALL_TOWN_VETO_BONUS,
            None if closing => resize + SMALL_TOWN_VETO_BONUS,
            None => resize,
            Some(_) => resize - SMALL_TOWN_IDLE_PENALTY,
        }
    }
}

#[cfg(test)]
mod tests;
