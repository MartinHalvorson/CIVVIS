//! `builders-improve-the-idle-for-production`: a city's best idle tile that a
//! Mine or a Lumber Mill would lift clear of its weakest worked tile is priced
//! as if a citizen worked it, and counts toward the Builder backlog.
//!
//! ## Why (live Deity Rome, 6-player Continents, Online, 2026-11-15)
//!
//! - **Few Mines.** Seven non-Culture openings laid 0-2 Mines by turn 50 and
//!   0-7 by turn 80, out of 4-9 and 15-27 improvements; the rest were Farms,
//!   Plantations and Pastures on tiles citizens already worked.
//! - **Idle hills beside poor worked tiles.** `civvis-20261115T075055Z` at
//!   turn 50: six owned bare hills idle while citizens worked 3/0 marsh and
//!   2/0.5 flats (Food/Production); turn 80: seven idle, two Mines; turn 110:
//!   ten Mines, seven of them worked at 1.4/3.3. `T081820Z` at turn 80: six
//!   Mines, all worked, and eight bare hills idle while citizens worked 2.7/0.7
//!   floodplains, 2/0 Fishing Boats and a 3/0 Farm.
//! - **The citizens do take a Mine.** The host's governor works a mined hill;
//!   the Builders never mine one no citizen works yet.
//!
//! The trap is circular. Every Builder gene prices the tiles citizens work
//! (`builders-improve-the-worked-for-production`), and the backlog that trains
//! Builders counts worked tiles (`builders-cover-the-worked-backlog`,
//! `the-worked-backlog-gets-its-builder-first`). A bare hill makes 2/1 or 1/2,
//! loses the citizen to a Food tile, and so is never worked, never counted and
//! never mined. `worked_production.rs`'s own census found about five bare
//! hills and five forests unimproved at turn 100.
//!
//! The native sim shows the same trap, milder. 12 Rome Score games with the
//! live genome had 3.1 idle owned bare hills at turn 50 and 4.8 at turn 80,
//! against 1.1 and 4.0 Mines, 92% of them worked. About half the worked tiles
//! made 1 Production or less, and 1.1 / 3.0 idle hills a city would gain two
//! Production from at turns 50 and 80.
//!
//! ## What the gene does
//!
//! Off, an exact no-op. On, each turn every city of ours not attacked in the
//! last [`IDLE_ATTACK_MEMORY`] turns picks at most one idle tile
//! ([`AdvancedAi::idle_production_tile`]) that meets all of these:
//!
//! - it is owned by the city, within three tiles of its centre, and worked by
//!   no citizen;
//! - it holds no improvement, district, foundation or wonder;
//! - one of [`IDLE_PRODUCTION_IMPROVEMENTS`] is legal there for us now (a
//!   Mine needs Mining, a Lumber Mill its tree, and a wooded hill no Mine);
//! - that improvement's yield, with the tree's upgrades, beats the
//!   Production of the city's weakest worked tile by
//!   [`IDLE_PRODUCTION_MARGIN`] or more;
//! - the city would not starve if the citizen moved there:
//!   `builders-improve-the-worked-for-production`'s Food floor, applied to
//!   the surplus after the swap.
//!
//! The pick is the largest Production gain, ties by position. That one
//! tile's improvement earns the worked premium
//! ([`AdvancedAi::worked_production_premium`]) as if a citizen worked it, and
//! every city with a pick adds one tile to the worked backlog
//! ([`AdvancedAi::idle_production_jobs`]) that trains and buys Builders.
//!
//! Journal reasons start `builders-improve-the-idle-for-production:`.

use super::*;
use crate::ai::advanced::worked_production::WORKED_FOOD_SURPLUS_FLOOR;

/// The improvements the gene prices on an idle tile: the two whose Production
/// carries a citizen off a Food tile.
pub(crate) const IDLE_PRODUCTION_IMPROVEMENTS: [&str; 2] = ["mine", "lumber_mill"];

/// The Production an improved idle tile must make over the city's weakest
/// worked tile: two, so a citizen gains at least as much as a Mine's whole
/// yield on a plains hill before Apprenticeship.
pub(crate) const IDLE_PRODUCTION_MARGIN: f64 = 2.0;

/// The farthest an idle tile may stand from the city centre: the three-ring
/// work radius.
const IDLE_PRODUCTION_RADIUS: i32 = 3;

/// A city attacked within this many turns keeps its Builders on the tiles
/// it works.
pub(crate) const IDLE_ATTACK_MEMORY: u32 = 4;

/// The turn's pick per city, keyed by city id with the centre beside it: a
/// live mirror renumbers cities between frames, so a stale id never resolves
/// to another city's pick.
#[derive(Clone, Default)]
pub(crate) struct IdleProductionFrame {
    turn: Option<u32>,
    pid: usize,
    picks: BTreeMap<u32, (Pos, Option<(Pos, Name)>)>,
}

impl AdvancedAi {
    /// The idle tile `cid` prices as worked this turn and its improvement: see
    /// the module. `None` with the gene off.
    pub(super) fn idle_production_tile(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> Option<(Pos, Name)> {
        if !self.builders_improve_the_idle_for_production {
            return None;
        }
        let centre = g.cities.get(&cid)?.pos;
        let mut frame = self.idle_production_frame.borrow_mut();
        if frame.turn != Some(g.turn) || frame.pid != pid {
            *frame = IdleProductionFrame {
                turn: Some(g.turn),
                pid,
                picks: BTreeMap::new(),
            };
        }
        if let Some((pos, pick)) = frame.picks.get(&cid) {
            if *pos == centre {
                return *pick;
            }
        }
        let pick = Self::idle_production_pick(g, pid, cid);
        frame.picks.insert(cid, (centre, pick));
        pick
    }

    /// The pick itself, uncached.
    fn idle_production_pick(g: &Game, pid: usize, cid: u32) -> Option<(Pos, Name)> {
        let city = g.cities.get(&cid).filter(|city| city.owner == pid)?;
        if city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= IDLE_ATTACK_MEMORY
        {
            return None;
        }
        let worked: Vec<Pos> = match g.observed_city_worked_tiles.get(&cid) {
            Some(worked) => worked.to_vec(),
            None => g.city_citizen_plan(cid).worked_tiles,
        };
        let weakest = worked
            .iter()
            .filter(|pos| **pos != city.pos)
            .map(|pos| g.workable_tile_yields(*pos))
            .min_by(|a, b| {
                a.production
                    .total_cmp(&b.production)
                    .then(b.food.total_cmp(&a.food))
            })?;
        let surplus = g.city_yields(cid).food - 2.0 * city.pop as f64;
        let housed = g.city_housing_headroom(city) > 1.0;
        let mut best: Option<(f64, Pos, Name)> = None;
        for &pos in &city.owned_tiles {
            if pos == city.pos
                || worked.contains(&pos)
                || g.wdist(city.pos, pos) > IDLE_PRODUCTION_RADIUS
            {
                continue;
            }
            let Some(tile) = g.map.get(pos) else {
                continue;
            };
            if tile.improvement.is_some()
                || tile.district.is_some()
                || tile.district_foundation.is_some()
                || tile.wonder.is_some()
            {
                continue;
            }
            let legal = g.valid_improvements(pid, pos);
            for name in IDLE_PRODUCTION_IMPROVEMENTS {
                let improvement = Name::new(name);
                if !legal.contains(&improvement)
                    || !g
                        .rules
                        .improvements
                        .get(&improvement)
                        .is_some_and(|spec| spec.builder_buildable)
                {
                    continue;
                }
                let mut improved = g.workable_tile_yields(pos);
                improved.add(g.improvement_yield_change(pid, pos, improvement));
                let gain = improved.production - weakest.production;
                if gain < IDLE_PRODUCTION_MARGIN {
                    continue;
                }
                let after = surplus + improved.food - weakest.food;
                if after <= WORKED_FOOD_SURPLUS_FLOOR && housed {
                    continue;
                }
                if best
                    .as_ref()
                    .is_none_or(|(old, old_pos, _)| gain > *old || (gain == *old && pos < *old_pos))
                {
                    best = Some((gain, pos, improvement));
                }
            }
        }
        best.map(|(_, pos, improvement)| (pos, improvement))
    }

    /// Cities with an idle pick this turn: the tiles the gene adds to the
    /// worked backlog. Zero with the gene off.
    pub(super) fn idle_production_jobs(&self, g: &Game, pid: usize) -> usize {
        if !self.builders_improve_the_idle_for_production {
            return 0;
        }
        g.player_city_ids(pid)
            .into_iter()
            .filter(|cid| self.idle_production_tile(g, pid, *cid).is_some())
            .count()
    }

    /// The worked backlog with the gene's idle picks added: exactly
    /// `BasicAi::unimproved_worked_tiles` with the gene off.
    pub(super) fn worked_backlog_with_idle(&self, g: &Game, pid: usize) -> usize {
        BasicAi::unimproved_worked_tiles(g, pid) + self.idle_production_jobs(g, pid)
    }

    /// Whether `pos` is the idle pick of its city for `improvement`, so the
    /// worked premium prices it as if a citizen worked it. False with the gene
    /// off.
    pub(super) fn idle_production_prices_as_worked(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        pos: Pos,
        improvement: &str,
    ) -> bool {
        self.idle_production_tile(g, pid, cid)
            .is_some_and(|(pick, name)| pick == pos && name == improvement)
    }

    /// Hand `BasicAi`'s backlog step the turn's idle picks and journal any
    /// new one. Zero, and silent, with the gene off.
    pub(super) fn publish_idle_production_backlog(&mut self, g: &Game, pid: usize) {
        if !self.builders_improve_the_idle_for_production {
            self.base.idle_production_backlog = 0;
            return;
        }
        let mut picks = 0;
        for cid in g.player_city_ids(pid) {
            let Some((pos, improvement)) = self.idle_production_tile(g, pid, cid) else {
                continue;
            };
            picks += 1;
            let city = &g.cities[&cid].name;
            let yields = g.workable_tile_yields(pos);
            think!(self.journal(), Economy, Detail,
                "{city} wants a {improvement} on its idle tile ({}, {})", pos.0, pos.1;
                "builders-improve-the-idle-for-production: the tile makes {:.0}/{:.0} idle and \
                 beats the weakest worked tile by {IDLE_PRODUCTION_MARGIN:.0} Production or more \
                 improved", yields.food, yields.production);
        }
        self.base.idle_production_backlog = picks;
    }
}

#[cfg(test)]
mod tests;
