//! `holdout-keeps-the-faith-war`: no peace with a faith's founder while we
//! are that faith's last holdout and it is already taking our cities.
//!
//! A Religious Victory needs every living major's majority. When the founder's
//! faith holds every other major and we are the one still out, the war on the
//! founder is the only thing slowing its spreaders: our units condemn the
//! Missionaries and Apostles that enter our borders, and the founder sends its
//! spreaders to the front instead of past it. Across the religious losses of
//! 2026-10-13/14 the racer's spreaders reached our cities at 0.26 a turn at
//! war against 1.47 at peace.
//!
//! - Live Emperor 110038Z: at war with the Khmer (Buddhism, already holding
//!   Mali and Nubia, and 3 of our 7 cities) for 33 turns; at turn 114 we
//!   offered "the last window was a rout: 274 power against their 390", the
//!   Khmer took it, and Buddhism won at 116.
//! - Live Immortal 193955Z: peace with Kongo at 110, the religious loss at 120.
//!
//! The gene withholds every peace offer to, and refuses every peace offer
//! from, such a founder, whatever the reason the shipped clauses give. The
//! exit a besieged empire still needs stays: once a city of ours is falling
//! (`our_cities_falling`), peace is allowed again. A founder whose faith holds
//! none of our cities is spared: four match-point peace offers of the same
//! fortnight that the holdout survived had no city of ours on the racer's
//! faith.
//!
//! "Taking our cities" is a city of ours whose majority follows the faith, or
//! one it is converting: the faith at [`CONVERTING_SHARE`] of the city's
//! strongest pressure, the same line `home_conversion_threat` reacts to (on a
//! mirrored board, the host's next faith within its conversion clock).
//!
//! Off: unchanged.

use super::*;

/// A faith at this share of a city's strongest pressure is converting it.
const CONVERTING_SHARE: f64 = 0.6;

impl AdvancedAi {
    /// Our cities following `faith` or being converted to it.
    pub(super) fn our_cities_on_the_faith(g: &Game, pid: usize, faith: &str) -> usize {
        g.player_city_ids(pid)
            .iter()
            .filter_map(|cid| g.cities.get(cid))
            .filter(|city| {
                if g.city_religion(city) == Some(faith) {
                    return true;
                }
                let top = city.pressure.values().fold(0.0f64, |a, b| a.max(*b));
                top > 0.0
                    && city
                        .pressure
                        .get(faith)
                        .is_some_and(|pressure| *pressure + 1e-9 >= top * CONVERTING_SHARE)
            })
            .count()
    }

    /// `holdout-keeps-the-faith-war`: whether the war with `rival` is kept —
    /// we are its founded faith's last holdout, a city of ours follows or is
    /// converting to that faith, and no city of ours is falling. `false` with
    /// the gene off.
    pub(crate) fn holdout_keeps_the_faith_war_with(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> bool {
        if !self.holdout_keeps_the_faith_war || !g.victory_conditions.religious {
            return false;
        }
        let Some(player) = g.players.get(rival) else {
            return false;
        };
        if !player.alive || player.is_minor || player.is_barbarian {
            return false;
        }
        let Some(faith) = player.religion.as_deref() else {
            return false;
        };
        Self::we_are_the_last_holdout(g, pid, rival)
            && Self::our_cities_on_the_faith(g, pid, faith) > 0
            && !Self::our_cities_falling(g, pid)
    }

    /// The journal line for a withheld offer or a refused one.
    pub(crate) fn journal_holdout_keeps_the_faith_war(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        refused: bool,
    ) {
        if !self.journal().wants(crate::reasoning::Level::Decision) {
            return;
        }
        let faith = g.players[rival].religion.clone().unwrap_or_default();
        let on_it = Self::our_cities_on_the_faith(g, pid, &faith);
        let cities = g.player_city_ids(pid).len();
        let what = if refused {
            "Refusing peace with"
        } else {
            "Keeping the war with"
        };
        think!(self.journal(), Diplomacy, Decision,
               "{what} {}", g.players[rival].civ;
               "holdout-keeps-the-faith-war: we are {}'s last holdout and {on_it} of our {cities} cities follow it or are converting; at war our soldiers condemn its spreaders",
               faith);
    }
}

#[cfg(test)]
mod tests;
