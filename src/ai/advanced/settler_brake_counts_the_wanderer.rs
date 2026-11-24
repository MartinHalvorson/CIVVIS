//! `settler-brake-counts-the-wanderer`: a Settler that has been out of a city
//! for [`WANDERER_TURNS`] without founding closes the Settler pipeline, as a
//! parked one does under `settler-backlog-brake`, once the empire holds
//! [`BasicAi::SETTLER_BACKLOG_MIN_CITIES`] cities.
//!
//! **Why (live Tournament 7, 2026-11-22/23).** `settler-backlog-brake` counts
//! a Settler as parked only after it has stood on one tile for six turns
//! (`BasicAi::parked_settlers`). A Settler boxed in by rival Loyalty never
//! stands still: it marches to a site, refuses it ("inside that city's
//! nine-tile Loyalty reach"), retargets and marches again, so the brake never
//! sees it while the capital keeps training more. Live Ludwig
//! `civvis-t7-20261123T045817Z` (Lv 8): three cities from t30, 2 Settlers out
//! from t45, and Munich started a Settler at t85, t89, t92 and t101 with 2, 3,
//! 4 and 5 already out (100 to 145 Production each, a citizen apiece), 252
//! "Settler marching to" lines and no fourth city until t140. Across the 60
//! Tournament 7 games, 3 to 5 Settlers stood out for 40-60 turns while the
//! city count stayed flat in Barbarossa `083149Z` (6 cities), Catherine
//! `235852Z` (6), Matthias `101541Z` (1-2), Saladin `040607Z` (4) and Ludwig.
//!
//! The clock is `settler_walk_started` (the turn the Settler was first
//! stepped, which a return home does not reset). A wanderer still walks and
//! founds when a site opens; only the next Settler waits. Off, nothing
//! changes.
//!
//! **The empire clock (2026-11-23).** The first armed live game, Qin
//! `civvis-t7-20261123T080638Z` (Lv 8), held three cities from t50 to t150
//! with 2-5 Settlers out, "stranded | no legal site is reachable" from t95,
//! and trained Settlers again at t105, t109, t125, t128, t131 and t159: the
//! per-unit clock did not hold those Settlers (an id the board does not carry
//! from one turn to the next restarts it, and a Settler handled outside the
//! ordinary step is never stamped). So the brake also reads an empire-level
//! clock, [`AdvancedAi::note_settlers_out`]: the turn since which the empire
//! has had a Settler out without founding a city, reset when a city is founded
//! or no Settler is left. It is keyed to nothing a unit id can move.

use super::*;

/// Turns a Settler may be out of a city, founding nothing, before it closes
/// the pipeline: about twice the fast games' median walk.
pub(super) const WANDERER_TURNS: u32 = 15;

impl AdvancedAi {
    /// Our Settlers first stepped [`WANDERER_TURNS`] or more turns ago that
    /// have not founded. 0 with the gene off.
    pub(super) fn wandering_settlers(&self, g: &Game, pid: usize) -> usize {
        if !self.settler_brake_counts_the_wanderer {
            return 0;
        }
        g.player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                g.units
                    .get(uid)
                    .is_some_and(|unit| unit.kind.as_str() == "settler")
                    && self
                        .settler_walk_started
                        .get(uid)
                        .is_some_and(|started| g.turn.saturating_sub(*started) >= WANDERER_TURNS)
            })
            .count()
    }

    /// Keep the empire clock: the turn since which `pid` has had a Settler
    /// out and founded no city. Reset when the city count grows or no Settler
    /// is left; a lost city only lowers the count it compares against. Nothing
    /// with the gene off.
    pub(super) fn note_settlers_out(&mut self, g: &Game, pid: usize) {
        if !self.settler_brake_counts_the_wanderer {
            return;
        }
        let cities = g.player_city_ids(pid).len();
        let settlers = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                g.units
                    .get(uid)
                    .is_some_and(|unit| unit.kind.as_str() == "settler")
            })
            .count();
        match self.settlers_out_since {
            _ if settlers == 0 => self.settlers_out_since = None,
            Some((_, counted)) if cities > counted => {
                self.settlers_out_since = Some((g.turn, cities));
            }
            Some((since, counted)) if cities < counted => {
                self.settlers_out_since = Some((since, cities));
            }
            Some(_) => {}
            None => self.settlers_out_since = Some((g.turn, cities)),
        }
    }

    /// Whether the pipeline closes for a wanderer: the gene is on, the empire
    /// holds the backlog brake's city floor, and a Settler has wandered, by
    /// its own clock or the empire's.
    pub(super) fn wanderer_closes_the_pipeline(
        &self,
        g: &Game,
        pid: usize,
        city_count: usize,
    ) -> bool {
        self.settler_brake_counts_the_wanderer
            && city_count >= BasicAi::SETTLER_BACKLOG_MIN_CITIES
            && (self.wandering_settlers(g, pid) > 0
                || self
                    .settlers_out_since
                    .is_some_and(|(since, _)| g.turn.saturating_sub(since) >= WANDERER_TURNS))
    }
}

#[cfg(test)]
mod tests;
