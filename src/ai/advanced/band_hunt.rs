//! `war-kills-the-bands`: an enemy Rock Band in reach is run down.
//!
//! A Rock Band's concert is a lump of Tourism that never shows in its owner's
//! Tourism per turn, and the late culture surges that decide these games are
//! made of them. Live Emperor G409 (civvis-20261008T145658Z) passed the
//! production gate and lost to Spain's Culture Victory at turn 199: Spain's
//! Tourism read a flat 257-292 a turn from 185 to 199 while its visiting
//! tourists went 68 → 79 → 98 → 112 → 130 over turns 189-193, and two Spanish
//! bands stood beside Istanbul at 192 — the Ottoman staycation, the bar Spain
//! had to clear, fell 221 → 187 as they played. We were at war with Spain
//! from turn 130 and saw its bands on 59 band-turns from 153; none was ever
//! attacked.
//!
//! Every one of today's nine culture winners fielded bands we saw (2-10
//! each). We were at war with the band's owner on 202 such sightings across
//! seven of those games, and a land military unit of ours stood within three
//! tiles on 46 of them — 24 distinct bands, among them five of Japan's
//! (T083432Z), three of the Cree's (T090439Z) and nine of Spain's and the
//! Ottomans' in G409. A military
//! unit that enters an undefended band's tile destroys it
//! (`Game::resolve_entered_units`).
//!
//! **Bounded on purpose.** Only bands of a major we are at war with, only by
//! a land military unit that can reach the tile this turn with a move to
//! spare, never a body standing on the campaign objective's siege ring, and
//! never one the walk would leave exposed to half its health in blows. A band
//! escorted by a military unit cannot be entered and is left alone.

use std::collections::BTreeSet;

use super::{AdvancedAi, StrategicPlan};
use crate::game::{Action, Game};
use crate::think;
use crate::Pos;

/// A hunter keeps at least this much health.
const BAND_HUNTER_MIN_HP: i32 = 30;
/// How much of its health a hunter may be left exposed to after the kill.
const BAND_HUNTER_DANGER_SHARE: f64 = 0.5;
/// A land siege's ring: bodies this close to the campaign objective stay.
const BAND_HUNT_RING: i32 = 2;

impl AdvancedAi {
    /// Plan and apply this frame's band kills. Returns the hunters, which the
    /// ordinary unit loop must leave alone. Nothing with the gene off.
    pub(crate) fn plan_band_hunt(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        reserved: &BTreeSet<u32>,
    ) -> BTreeSet<u32> {
        let mut hunters = BTreeSet::new();
        if !self.war_kills_the_bands {
            return hunters;
        }
        let objective = plan
            .target_city
            .and_then(|cid| g.cities.get(&cid))
            .map(|city| city.pos);
        let mut bands: Vec<(u32, Pos)> = g
            .units
            .values()
            .filter(|unit| {
                unit.kind == "rock_band"
                    && unit.owner != pid
                    && !g.players[unit.owner].is_minor
                    && g.is_at_war(pid, unit.owner)
            })
            .map(|unit| (unit.id, unit.pos))
            .collect();
        bands.sort_by_key(|(id, _)| *id);
        for (band, at) in bands {
            if !g.units.contains_key(&band) {
                continue;
            }
            let mut candidates: Vec<(i32, u32)> = g
                .player_unit_ids(pid)
                .into_iter()
                .filter(|uid| {
                    let unit = &g.units[uid];
                    let spec = &g.rules.units[unit.kind];
                    spec.class == "military"
                        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                        && !reserved.contains(uid)
                        && !hunters.contains(uid)
                        && unit.hp >= BAND_HUNTER_MIN_HP
                        && unit.moves_left > 0.0
                        && !unit.acted
                        && unit.linked_to.is_none()
                        && !g.is_embarked(unit)
                        && objective.is_none_or(|ring| g.wdist(unit.pos, ring) > BAND_HUNT_RING)
                        && g.wdist(unit.pos, at) <= spec.moves.max(1.0) as i32
                })
                .map(|uid| (g.wdist(g.units[&uid].pos, at), uid))
                .collect();
            candidates.sort();
            for (_, uid) in candidates {
                let Some(step) = Self::band_hunt_step(g, pid, uid, band, at) else {
                    continue;
                };
                let owner = g.units[&band].owner;
                if g.apply(pid, &step).is_err() || g.units.contains_key(&band) {
                    continue;
                }
                hunters.insert(uid);
                *g.players[pid]
                    .counters
                    .entry("band_hunt:killed".to_string())
                    .or_insert(0) += 1;
                think!(self.journal(), Military, Decision,
                       "Running down a Rock Band of {}", g.players[owner].civ;
                       "war-kills-the-bands: a concert is a lump of Tourism toward a Culture Victory; the band stood in reach of our {}",
                       g.units.get(&uid).map(|unit| unit.kind.as_str()).unwrap_or("unit");
                       at);
                break;
            }
        }
        hunters
    }

    /// The walk onto the band's tile, simulated: it must land there, the band
    /// must be gone, and the hunter must not stand in reach of half its
    /// health.
    fn band_hunt_step(g: &Game, pid: usize, uid: u32, band: u32, at: Pos) -> Option<Action> {
        let mut board = g.speculative_clone();
        let step = Action::MoveTo { unit: uid, to: at };
        if board.apply(pid, &step).is_err()
            || board.units.get(&uid).is_none_or(|unit| unit.pos != at)
            || board.units.contains_key(&band)
        {
            return None;
        }
        let hp = f64::from(board.units.get(&uid)?.hp);
        (super::battle_planner::strike_danger(&board, pid, at, uid) < hp * BAND_HUNTER_DANGER_SHARE)
            .then_some(step)
    }
}

#[cfg(test)]
mod tests;
