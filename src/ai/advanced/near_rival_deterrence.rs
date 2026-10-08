//! `near-rival-deterrence`: from live turn 30 to 80, while a met major at
//! peace has a city within [`DETERRENCE_NEAR_TILES`] of ours and our military
//! power is below its own, one idle or routine queue a turn trains a land
//! unit (at most [`DETERRENCE_MAX_QUEUED`] in training), until we stand level.
//! The early city floor's Settler, Builders, Walls, wonders and investments in
//! progress keep their queues.
//!
//! **Why (10-08 census, 76 live Emperor runs).** 35 wars opened between
//! turns 30 and 80; the AI declared 23 of them, and 12 of those 23 cost a city
//! within 30 turns (14 cities), the empire holding a median 4 at t75. Every
//! one of the 23 came at a power ratio of 1.32 or less (median 0.59), 22 below
//! parity. Over 6,757 rival-turns at peace the declaration rate falls with
//! our power: per 100 turns against a rival with a city within ten tiles,
//!
//! | our power / theirs | rival-turns | declared | per 100 |
//! |---|---:|---:|---:|
//! | < 0.5 | 394 | 5 | 1.27 |
//! | 0.5-0.7 | 597 | 6 | 1.01 |
//! | 0.7-0.9 | 652 | 5 | 0.77 |
//! | 0.9-1.1 | 488 | 2 | 0.41 |
//! | 1.1-1.5 | 478 | 1 | 0.21 |
//! | ≥ 1.5 | 671 | 0 | 0 |
//!
//! and no rival whose nearest city stood more than ten tiles off declared at
//! all (0 of 1,588). The seven-city openings held a median 1.02 of the near
//! rival's power over turns 35-55; the four-or-fewer openings 0.76. Walls and
//! units per city separated the rates far less than power did.
//!
//! Parity is the line: the rate halves from 0.7-0.9 to 0.9-1.1, and the cost of
//! the next band (1.5) is half again the army. The claim runs after the
//! early city floor's Settler and before the Prophet race and the routine
//! claims; the end-of-turn defence redirects keep the last word.

use super::*;

/// A rival whose nearest known city stands within this many tiles of one of
/// ours is a neighbour. None farther declared in the census.
pub(crate) const DETERRENCE_NEAR_TILES: i32 = 10;

/// Our power against the strongest neighbour's the claim builds toward.
pub(crate) const DETERRENCE_RATIO: f64 = 1.0;

/// The window, in standard turns: 45 and 120 are live turns 30 and 80 at the
/// seat's Quick speed.
pub(crate) const DETERRENCE_START_STANDARD: u32 = 45;
pub(crate) const DETERRENCE_END_STANDARD: u32 = 120;

/// Land units in training at once under the claim.
pub(crate) const DETERRENCE_MAX_QUEUED: usize = 2;

/// The longest unit build, in standard turns, the claim will queue.
pub(crate) const DETERRENCE_MAX_BUILD: u32 = 18;

impl AdvancedAi {
    /// The strongest met major at peace with a known city within
    /// [`DETERRENCE_NEAR_TILES`] of ours, and its power. `None` with the
    /// gene off, outside the window, and with no such neighbour.
    pub(super) fn deterrence_threat(&self, g: &Game, pid: usize) -> Option<(usize, f64)> {
        if !self.near_rival_deterrence
            || g.turn < g.standard_duration(DETERRENCE_START_STANDARD)
            || g.turn >= g.standard_duration(DETERRENCE_END_STANDARD)
        {
            return None;
        }
        let ours: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        if ours.is_empty() {
            return None;
        }
        g.players
            .iter()
            .filter(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && !other.is_free_city
                    && g.has_met(pid, other.id)
                    && !g.is_at_war(pid, other.id)
            })
            .filter(|other| {
                g.cities.values().any(|city| {
                    city.owner == other.id
                        && ours
                            .iter()
                            .any(|home| g.wdist(*home, city.pos) <= DETERRENCE_NEAR_TILES)
                })
            })
            .map(|other| (other.id, g.military_power(other.id)))
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
    }

    /// Whether the claim still wants the land unit at the head of `cid`'s
    /// queue: a neighbour still out-guns us.
    pub(super) fn deterrence_unit_holds(&self, g: &Game, pid: usize, item: &Item) -> bool {
        let Item::Unit { unit } = item else {
            return false;
        };
        if !Self::deterrence_land_unit(g, unit) {
            return false;
        }
        self.deterrence_threat(g, pid)
            .is_some_and(|(_, theirs)| g.military_power(pid) < DETERRENCE_RATIO * theirs)
    }

    fn deterrence_land_unit(g: &Game, unit: &Name) -> bool {
        g.rules.units.get(unit).is_some_and(|spec| {
            spec.class == "military"
                && spec.domain.as_deref().is_none_or(|domain| domain == "land")
                && (spec.is_melee_capable() || spec.has_ranged_attack())
        })
    }

    /// Whether `cid`'s current queue head may give way to the deterrent.
    fn deterrence_may_displace(g: &Game, pid: usize, cid: u32) -> bool {
        let Some(item) = g.cities[&cid].queue.first() else {
            return true;
        };
        let remaining =
            g.item_cost_for_city(pid, cid, item) - g.item_invested_production(cid, item);
        if remaining <= g.city_yields(cid).production {
            return false;
        }
        match item {
            Item::Building { building } => g.rules.buildings.get(building).is_some_and(|spec| {
                !spec.wonder
                    && !matches!(
                        building.as_str(),
                        "walls" | "medieval_walls" | "renaissance_walls"
                    )
            }),
            Item::District { .. } => g.item_invested_production(cid, item) <= 0.0,
            Item::Unit { unit } => matches!(unit.as_str(), "scout" | "trader"),
            _ => false,
        }
    }

    /// See the module: one land unit in the city that trains it soonest while
    /// a neighbour out-guns us. Exact no-op with the gene off.
    pub(super) fn claim_deterrence_unit(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let Some((rival, theirs)) = self.deterrence_threat(g, pid) else {
            return;
        };
        let ours = g.military_power(pid);
        if ours >= DETERRENCE_RATIO * theirs {
            return;
        }
        let city_ids = g.player_city_ids(pid);
        let queued = city_ids
            .iter()
            .filter(|cid| {
                matches!(
                    g.cities[*cid].queue.first(),
                    Some(Item::Unit { unit }) if Self::deterrence_land_unit(g, unit)
                )
            })
            .count();
        if queued >= DETERRENCE_MAX_QUEUED {
            return;
        }
        let counts = self.counts(g, pid);
        let rival_cities: Vec<Pos> = g
            .cities
            .values()
            .filter(|city| city.owner == rival)
            .map(|city| city.pos)
            .collect();
        let max_turns = g.standard_duration(DETERRENCE_MAX_BUILD) as f64;
        let mut best: Option<(f64, i32, u32, String)> = None;
        for &cid in &city_ids {
            if plan.threatened_city == Some(cid)
                || self.early_settler_floor_holds(g, pid, cid, plan)
                || !Self::deterrence_may_displace(g, pid, cid)
            {
                continue;
            }
            let Some(unit) = self
                .base
                .combined_arms_unit(g, pid, cid, counts.melee, counts.ranged)
                .filter(|unit| Self::deterrence_land_unit(g, &Name::new(unit)))
            else {
                continue;
            };
            let item = Item::Unit {
                unit: Name::new(&unit),
            };
            if !g.can_produce(pid, cid, &item) {
                continue;
            }
            let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                g.item_cost_for_city(pid, cid, &item) / g.city_yields(cid).production.max(0.5)
            });
            if turns > max_turns {
                continue;
            }
            let home = g.cities[&cid].pos;
            let border = rival_cities
                .iter()
                .map(|pos| g.wdist(home, *pos))
                .min()
                .unwrap_or(i32::MAX);
            if best.as_ref().is_none_or(|(old, old_border, old_city, _)| {
                turns + 1e-9 < *old
                    || ((turns - old).abs() <= 1e-9 && (border, cid) < (*old_border, *old_city))
            }) {
                best = Some((turns, border, cid, unit));
            }
        }
        let Some((turns, _, cid, unit)) = best else {
            return;
        };
        let displaced = g.cities[&cid].queue.first().cloned();
        let item = Item::Unit {
            unit: Name::new(&unit),
        };
        if g.apply(pid, &Action::Produce { city: cid, item }).is_ok() {
            think!(self.journal(), Military, Decision,
                "{} trains a deterrent against {}", g.cities[&cid].name, g.players[rival].civ;
                "near-rival-deterrence: {} for {ours:.0} power against their {theirs:.0}, a city \
                 within {DETERRENCE_NEAR_TILES} tiles; {turns:.1} turns{}",
                unit.replace('_', " "),
                displaced.map_or(String::new(), |item| format!(
                    "; {} keeps its progress", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
