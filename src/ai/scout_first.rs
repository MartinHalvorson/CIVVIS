//! An independent, bounded opening experiment: buy eyes before expansion.
//!
//! `opening-scout-comes-back` (opt-in) keeps those eyes. Two decisions:
//!
//! 1. When the opening's barbarian defence takes a city's queue from a Scout
//!    (`Action::Produce` replaces the whole queue, so the Scout is gone and
//!    the book has already moved past it), the Scout is owed: it is the
//!    next build in that city once the defender leaves the queue, unless a
//!    recon unit of ours stands or is queued by then. The debt is kept on
//!    the city's tile, never its id.
//! 2. When a Scout of ours has stood on the board, none stands or is queued
//!    now, the turn is before [`OPENING_SCOUT_LOSS_TURNS`] standard turns
//!    (turn 25 at Online) and no living major rival's original capital is
//!    charted, the capital starts a Scout at once, unless it is training a
//!    soldier. Once a game.
//!
//! Live Gran Colombia, 24 Deity games of 2026-11-11 from 19:57Z: in
//! 200336Z, 222533Z and 230722Z "slinger takes the queue before scout" at
//! turns 2-4 and the empire had no Scout until turns 23, 31 and 46 (230722Z
//! met no rival by turn 45). In 223549Z the Scout was lost at about turn 7
//! and replaced at turn 29. All four were among the twelve games whose
//! nearest rival capital, 13-20 tiles away, was not charted by turn 20.
use super::{BasicAi, UnitDoctrine};
use crate::game::{Action, Game, Item};
use crate::think;

/// `opening-scout-comes-back`: a Scout lost before this many standard turns
/// (turn 25 at Online) is replaced at once.
pub(crate) const OPENING_SCOUT_LOSS_TURNS: u32 = 37;

fn scout_item() -> Item {
    Item::Unit {
        unit: crate::name!("scout"),
    }
}

impl BasicAi {
    pub(super) fn play_scout_first_opening(&mut self, g: &mut Game, pid: usize, cid: u32) -> bool {
        if !self.scout_first_opening || self.minor || self.barb || self.book_pos != 0 {
            return false;
        }
        let city = &g.cities[&cid];
        if !city.is_capital || city.owner != pid || !city.queue.is_empty() {
            return false;
        }
        let home = city.pos;
        // A first Scout must not substitute for the starting defender, or
        // duplicate recon already standing or ordered elsewhere.
        let has_defender = g.player_unit_ids(pid).into_iter().any(|uid| {
            let unit = &g.units[&uid];
            let spec = &g.rules.units[&unit.kind];
            spec.class == "military"
                && spec.domain.as_deref().unwrap_or("land") == "land"
                && spec.promotion_class != "recon"
                && g.wdist(home, unit.pos) <= 5
        });
        let has_recon = g
            .player_unit_ids(pid)
            .into_iter()
            .any(|uid| Self::unit_doctrine(g, uid) == UnitDoctrine::Recon)
            || g.player_city_ids(pid).into_iter().any(|city| {
                g.cities[&city].queue.iter().any(|item| match item {
                    Item::Unit { unit } | Item::Formation { unit, .. } => g
                        .rules
                        .units
                        .get(unit)
                        .is_some_and(|spec| spec.promotion_class == "recon"),
                    _ => false,
                })
            });
        // Only observed pressure counts. Even a hostile Scout can interfere
        // with the first settlers, so do not exclude recon from this veto.
        let threatened = g.units.values().any(|unit| {
            unit.owner != pid
                && g.is_at_war(pid, unit.owner)
                && g.players[pid].turn_visible.contains(&unit.pos)
                && g.wdist(home, unit.pos) <= 5
                && g.rules.units[&unit.kind].class == "military"
        });
        // Ask about a known land frontier, never the terrain under the fog.
        let frontier = g.players[pid].explored.iter().any(|pos| {
            g.wdist(home, *pos) <= 5
                && g.map
                    .get(*pos)
                    .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
                && g.nbrs(*pos).into_iter().any(|next| {
                    g.map.tiles.contains_key(&next) && !g.players[pid].explored.contains(&next)
                })
        });
        let item = Item::Unit {
            unit: crate::name!("scout"),
        };
        if !has_defender || has_recon || threatened || !frontier || !g.can_produce(pid, cid, &item)
        {
            return false;
        }
        if g.apply(pid, &Action::Produce { city: cid, item }).is_err() {
            return false;
        }
        self.book_pos = 1;
        think!(self.journal, Cities, Decision,
               "{} opens with a Scout", g.cities[&cid].name;
               "scout-first-opening: a nearby defender covers the capital, no hostile is visible within five tiles, and land frontier remains; this replaces the first opening slot");
        true
    }

    /// Whether `item` trains a recon unit.
    fn is_recon_item(g: &Game, item: &Item) -> bool {
        match item {
            Item::Unit { unit } | Item::Formation { unit, .. } => g
                .rules
                .units
                .get(unit)
                .is_some_and(|spec| spec.promotion_class == "recon"),
            _ => false,
        }
    }

    /// Recon of ours on the board, and recon in any of our queues.
    fn opening_recon(g: &Game, pid: usize) -> (bool, bool) {
        let fielded = g
            .player_unit_ids(pid)
            .into_iter()
            .any(|uid| Self::unit_doctrine(g, uid) == UnitDoctrine::Recon);
        let queued = g.player_city_ids(pid).into_iter().any(|cid| {
            g.cities[&cid]
                .queue
                .iter()
                .any(|item| Self::is_recon_item(g, item))
        });
        (fielded, queued)
    }

    /// Whether a living major rival's original capital stands on ground we
    /// have charted.
    fn rival_capital_charted(g: &Game, pid: usize) -> bool {
        g.players.iter().any(|rival| {
            rival.id != pid
                && rival.alive
                && !rival.is_minor
                && !rival.is_barbarian
                && g.player_city_ids(rival.id).into_iter().any(|cid| {
                    let city = &g.cities[&cid];
                    city.is_capital
                        && city.original_owner == rival.id
                        && g.players[pid].explored.contains(&city.pos)
                })
        })
    }

    /// `opening-scout-comes-back`, decision 1 recorded: the barbarian
    /// defence just took city `cid`'s queue from `displaced`. A recon unit
    /// displaced is owed, on the city's tile. No-op with the gene off.
    pub(super) fn note_opening_scout_displaced(
        &mut self,
        g: &Game,
        cid: u32,
        displaced: Option<&Item>,
    ) {
        if !self.opening_scout_comes_back
            || !displaced.is_some_and(|item| Self::is_recon_item(g, item))
        {
            return;
        }
        if let Some(city) = g.cities.get(&cid) {
            self.opening_scout_owed_at = Some(city.pos);
        }
    }

    /// `opening-scout-comes-back`: city `cid` trains the Scout it owes once
    /// its queue is empty (decision 1), or the capital replaces a lost early
    /// Scout at once (decision 2). True when the city took a Scout.
    pub(super) fn play_opening_scout_comes_back(
        &mut self,
        g: &mut Game,
        pid: usize,
        cid: u32,
    ) -> bool {
        if !self.opening_scout_comes_back
            || self.minor
            || self.barb
            || (self.opening_scout_owed_at.is_none()
                && (self.opening_scout_replaced
                    || g.turn >= g.standard_duration(OPENING_SCOUT_LOSS_TURNS)))
        {
            return false;
        }
        let Some(city) = g.cities.get(&cid).filter(|city| city.owner == pid) else {
            return false;
        };
        let (pos, is_capital, head) = (city.pos, city.is_capital, city.queue.first().cloned());
        let (fielded, queued) = Self::opening_recon(g, pid);
        if fielded {
            self.opening_recon_fielded = true;
        }
        let item = scout_item();
        if self.opening_scout_owed_at == Some(pos) {
            if fielded || queued {
                // The eyes came another way; nothing is owed.
                self.opening_scout_owed_at = None;
                return false;
            }
            if head.is_some()
                || !g.can_produce(pid, cid, &item)
                || g.apply(pid, &Action::Produce { city: cid, item }).is_err()
            {
                return false;
            }
            self.opening_scout_owed_at = None;
            think!(self.journal, Cities, Decision,
                   "{} trains the Scout the barbarian defence displaced", g.cities[&cid].name;
                   "opening-scout-comes-back: the defender has left the queue and the empire has \
                    no recon unit standing or queued");
            return true;
        }
        let defending = head.as_ref().is_some_and(|head| match head {
            Item::Unit { unit } | Item::Formation { unit, .. } => g
                .rules
                .units
                .get(unit)
                .is_some_and(|spec| spec.class == "military" && spec.promotion_class != "recon"),
            _ => false,
        });
        if !is_capital
            || !self.opening_recon_fielded
            || self.opening_scout_replaced
            || fielded
            || queued
            || defending
            || g.turn >= g.standard_duration(OPENING_SCOUT_LOSS_TURNS)
            || Self::rival_capital_charted(g, pid)
            || !g.can_produce(pid, cid, &item)
            || g.apply(pid, &Action::Produce { city: cid, item }).is_err()
        {
            return false;
        }
        self.opening_scout_replaced = true;
        think!(self.journal, Cities, Decision,
               "{} replaces the lost Scout at once", g.cities[&cid].name;
               "opening-scout-comes-back: our Scout is gone before turn {} and no rival capital \
                is charted; the one replacement this game",
               g.standard_duration(OPENING_SCOUT_LOSS_TURNS));
        true
    }
}

#[cfg(test)]
mod tests;
