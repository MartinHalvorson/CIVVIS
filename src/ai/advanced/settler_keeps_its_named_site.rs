//! `settler-keeps-its-named-site`: a Settler is sent only to a site it can
//! keep, and every change of its destination is journalled with its cause.
//!
//! ## What the live runs say (27 Deity runs of 2026-11-15, 229 Settlers)
//!
//! The march already keeps a cached site against a better-scoring one: the
//! cached site is re-checked every frame and kept unless a check fails
//! (`target_drop_reason` in `settling.rs`). With `settler-target-hysteresis`
//! and `settler-stall-counts-turns` live, nearly every drop is journalled:
//! of 284 changes of destination, 277 sit next to a line that names the old
//! or the new site. What wastes the walk is which checks fail.
//!
//! - **The site was a plot the seat had never seen.** 153 of 486 named sites
//!   (31%) were unrevealed when named. The board marks an unrevealed plot
//!   `unknown` and the frontier prior lets the scan rank it; most sit on the
//!   edge of a coast. Of the 141 later revealed, 79 were Coast or Ocean and 5
//!   Mountains. 15 of the 153 were founded. "The cached site stopped passing
//!   its checks (water)" or "(impassable)" ended 82 walks, and 275 of the 1,044
//!   tiles walked toward sites later dropped. Run civvis-20261115T071444Z t55:
//!   (-4, 20) was named unrevealed, revealed as Coast on t56 and set aside on
//!   t57; a Fishing Boats was built there on t84.
//! - **Two of our Settlers named sites within three tiles of each other.**
//!   The reservation (`settler_target_reserved_by_other`) covers the exact
//!   plot only. 32 of 40 "a city within three tiles" drops were our own new
//!   city, 21 of them an ordinary founding at its own site, 1 to 3 tiles from
//!   a sibling's site.
//! - The rest are the stall limit (60 changes), the threat detour (28), the
//!   Score seat's near-site founding (17), the retreat limit (12) and the
//!   arrival Loyalty check (5): each is an event the march should answer.
//!
//! ## What the gene does
//!
//! 1. A plot the board marks `unknown` is not a site in the ranked pick
//!    (`best_reachable_settle_site_except_cached`). When the ranked pick finds
//!    no seen site, the exhaustion and stranded fallbacks still look into
//!    the fog, as before.
//! 2. A site within three tiles (the founding spacing) of a site another
//!    live Settler of ours has named is not offered to the ranked pick, nor
//!    to the two exhaustion fallbacks of `settler-never-idles`. A cached site
//!    is never dropped for this, so two Settlers whose sites already overlap
//!    do not drop each other's. The founding-now rules (walk deadline,
//!    stalled Settler, the Score seat's near site, shelter) are untouched.
//! 3. Every change of destination gets one line, `Settler changes its
//!    destination from A to B`, naming what happened to A and keyed by the
//!    Settler's stable id. The comparison is against the last site this
//!    Settler named, so a change made anywhere (between frames, by an escort
//!    or watchdog rule) is caught at the Settler's next step.
//!
//! The capture and hostile-reach holds are untouched. Off (the default), no
//! filter applies, the memory stays empty and no line is written.
//!
//! The native sim plays on known terrain (no `unknown` plot), so it can judge
//! only parts 2 and 3; part 1 is a live-board effect.
//!
//! ## Measured (2026-11-15, PR #5340)
//!
//! - Paired sim, 60 seeds, Rome Score seat, Deity 6p Continents Online:
//!   destination changes per Settler 2.93 -> 2.42 (z -2.6), tiles walked
//!   237 -> 203 (z -2.6), Settler-turns per founded city 16.1 -> 14.0
//!   (z -2.4), cities at t100 9.00 -> 9.32 (z +1.6). Drops for a sibling's
//!   site or our own new city fell from 466 to 115.
//! - One-frame `civvis_orders` replays of 152 live frames where a Settler
//!   named an unseen plot: unseen sites named fell from 76 of 102 marching
//!   Settlers to 35; the 26 passed over were half water or Mountain, but the
//!   seen site taken instead was farther (8.2 tiles against 5.7) and worth
//!   less (74.8 against 86.0). 26 others kept the fog site through the
//!   exhaustion fallback.

use super::{hostile_memory_key, AdvancedAi, Game};
use crate::think;
use crate::Pos;

/// Two city centres must stand at least this many tiles apart; a site
/// closer than this to another named site cannot be founded beside it.
const FOUNDING_SPACING: i32 = 4;

impl AdvancedAi {
    /// The Settler's id across the live mirror's per-frame board rebuilds:
    /// the host's Civ VI id when the board carries one, else the board id.
    /// `None` with the gene off or the unit gone.
    pub(super) fn named_site_key(&self, g: &Game, uid: u32) -> Option<i64> {
        if !self.settler_keeps_its_named_site {
            return None;
        }
        g.units.get(&uid).map(|unit| hostile_memory_key(g, unit))
    }

    /// The site another live Settler of ours has named within the founding
    /// spacing of `pos`, if any.
    pub(super) fn sibling_site_within_spacing(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        pos: Pos,
    ) -> Option<Pos> {
        self.settler_targets
            .iter()
            .find(|(other, site)| {
                **other != uid
                    && g.wdist(**site, pos) < FOUNDING_SPACING
                    && g.units
                        .get(other)
                        .is_some_and(|unit| unit.owner == pid && unit.kind == "settler")
            })
            .map(|(_, site)| *site)
    }

    /// Whether `pos` lies within the founding spacing of a site another live
    /// Settler of ours has named, so at most one of the two can be founded.
    /// The exhaustion fallbacks ask this too. `false` with the gene off.
    pub(super) fn named_site_spaced_out(&self, g: &Game, pid: usize, uid: u32, pos: Pos) -> bool {
        self.settler_keeps_its_named_site
            && self.sibling_site_within_spacing(g, pid, uid, pos).is_some()
    }

    /// Whether the ranked pick must pass over `pos` for this Settler: the
    /// plot is unseen, or it lies within the founding spacing of another
    /// Settler's named site. `false` with the gene off.
    pub(super) fn named_site_is_unkeepable(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        pos: Pos,
    ) -> bool {
        if !self.settler_keeps_its_named_site {
            return false;
        }
        g.map.get(pos).is_some_and(|tile| g.rules.is_unknown(tile))
            || self.named_site_spaced_out(g, pid, uid, pos)
    }

    /// After the Settler's step: remember the site it names, and journal a
    /// change from the one it named before. `key` is
    /// [`Self::named_site_key`] read before the step; `None` (the gene off)
    /// does nothing.
    pub(super) fn note_named_site(&mut self, g: &Game, pid: usize, uid: u32, key: Option<i64>) {
        let Some(key) = key else {
            return;
        };
        if !g.units.get(&uid).is_some_and(|unit| unit.owner == pid) {
            // Founded, lost or gone: nothing walks to the old site any more.
            self.settler_named_sites.remove(&key);
            return;
        }
        let Some(site) = self.settler_targets.get(&uid).copied() else {
            // No site this frame: the next one named is compared with the last.
            return;
        };
        let Some(old) = self.settler_named_sites.insert(key, site) else {
            return;
        };
        if old == site {
            return;
        }
        let here = g.units[&uid].pos;
        let why = self.named_site_change_cause(g, pid, uid, old);
        think!(self.journal(), Expansion, Detail,
               "Settler changes its destination from {old:?} to {site:?}";
               "settler-keeps-its-named-site: {why}; the new site is {} tiles from the old \
                and {} from the Settler; settler {key}",
               g.wdist(old, site), g.wdist(here, site); site);
    }

    /// What happened to `old`, the site this Settler named before, read from
    /// the board and the march's own memory after the change.
    pub(super) fn named_site_change_cause(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        old: Pos,
    ) -> String {
        let Some(tile) = g.map.get(old) else {
            return "the old site left the board".to_string();
        };
        if g.rules.is_water(tile) {
            return "the old site is water".to_string();
        }
        if !g.rules.is_passable(tile) {
            return "the old site is impassable".to_string();
        }
        if let Some(city) = g
            .cities
            .values()
            .filter(|city| g.wdist(city.pos, old) < FOUNDING_SPACING)
            .min_by_key(|city| (g.wdist(city.pos, old), city.pos))
        {
            let whose = if city.owner == pid {
                "our own".to_string()
            } else {
                format!("{}'s", g.players[city.owner].civ)
            };
            return format!(
                "{whose} city at {:?} stands {} tiles from the old site",
                city.pos,
                g.wdist(city.pos, old)
            );
        }
        if tile
            .owner_city
            .is_some_and(|cid| g.cities.get(&cid).is_none_or(|city| city.owner != pid))
        {
            return "the old site is another empire's ground".to_string();
        }
        if g.blocked_city_sites.contains(&old) {
            return "the host refused a city on the old site".to_string();
        }
        if self.settler_capture_scars.contains_key(&old) {
            return "a Settler was lost within three tiles of the old site".to_string();
        }
        if self.settler_site_is_dead(uid, old) {
            return "the old site was retired for this Settler (it could not found there, its \
                    Loyalty forecast failed, or its approach kept forcing retreats)"
                .to_string();
        }
        // Before the set-aside: hysteresis also sets a deferred site aside.
        if self.settler_threat_deferrals.contains_key(&old) {
            return "the approach to the old site is deferred for a visible threat".to_string();
        }
        if self.settler_avoid.get(&uid).map(|(site, _)| *site) == Some(old) {
            return "the old site was set aside (the stall limit, or a check it stopped passing)"
                .to_string();
        }
        if let Some(sibling) = self.sibling_site_within_spacing(g, pid, uid, old) {
            return format!(
                "another Settler names {sibling:?}, within three tiles of the old site"
            );
        }
        "the old site still passes its checks; a founding, shelter, detour or Score rule named \
         the new one"
            .to_string()
    }
}

#[cfg(test)]
mod tests;
