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
//!
//! **`near-rival-deterrence-2` (2026-11-02).** Live Lv 6
//! civvis-20261102T185013Z read Babylon at 215 power against our 279 and
//! stood down; Babylon's Pikemen (45) took two cities from our Warriors (20)
//! in nine turns, and after turn 80 nothing watched its climb to 493 before
//! the second war took Moscow. Version 2 keeps v1's neighbour inside v1's
//! window and adds, from the window's start to
//! [`DETERRENCE_WATCH_END_STANDARD`] (live turn 150), any met major at peace
//! whose war forecast hazard reaches [`war_forecast::FORECAST_HAZARD_FLOOR`]
//! (see `advanced/war_forecast.rs` for the census). The watch stops there:
//! 13 of the census's 14 wars that cost us a city began by live turn 142,
//! and 1 of the 33 begun after turn 160 did; an all-game watch read 0.9
//! points of score share worse than v1 at Emperor (32 paired seeds) for an
//! army 10% larger and no city kept. Both sides are weighed in quality
//! power, so ten Warriors no longer read level with four Pikemen. While the neighbour
//! out-guns us, one obsolete land unit a turn is upgraded, best strength
//! gained per gold first, above [`DETERRENCE_UPGRADE_RESERVE_BASE`] plus
//! [`DETERRENCE_UPGRADE_RESERVE_PER_CITY`] a city.

use super::war_forecast::FORECAST_HAZARD_FLOOR;
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

/// Version 2's forecast watch ends here, in standard turns: live turn 150 at
/// Quick speed.
pub(crate) const DETERRENCE_WATCH_END_STANDARD: u32 = 225;

/// Land units in training at once under the claim.
pub(crate) const DETERRENCE_MAX_QUEUED: usize = 2;

/// The longest unit build, in standard turns, the claim will queue.
pub(crate) const DETERRENCE_MAX_BUILD: u32 = 18;

/// Gold kept back from a version 2 upgrade: this much…
pub(crate) const DETERRENCE_UPGRADE_RESERVE_BASE: f64 = 60.0;

/// …plus this much per city.
pub(crate) const DETERRENCE_UPGRADE_RESERVE_PER_CITY: f64 = 15.0;

impl AdvancedAi {
    /// The strongest met major at peace with a known city within
    /// [`DETERRENCE_NEAR_TILES`] of ours, and its power. `None` with the
    /// gene off, outside the window, and with no such neighbour.
    pub(super) fn deterrence_threat(&self, g: &Game, pid: usize) -> Option<(usize, f64)> {
        if !(self.near_rival_deterrence || self.near_rival_deterrence_2)
            || g.turn < g.standard_duration(DETERRENCE_START_STANDARD)
        {
            return None;
        }
        let in_window = g.turn < g.standard_duration(DETERRENCE_END_STANDARD);
        let watching = self.near_rival_deterrence_2
            && g.turn < g.standard_duration(DETERRENCE_WATCH_END_STANDARD);
        if !in_window && !watching {
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
        if self.near_rival_deterrence_2 {
            // Version 2: the forecast names the neighbour and prices both
            // armies by unit quality. `deterrence-trusts-a-friend` still
            // spares a friend.
            return self
                .threat_forecasts(g, pid)
                .into_iter()
                .filter(|forecast| !self.deterrence_spares_a_friend(g, pid, forecast.rival))
                .filter(|forecast| {
                    (in_window && forecast.nearest <= DETERRENCE_NEAR_TILES)
                        || forecast.hazard >= FORECAST_HAZARD_FLOOR
                })
                .map(|forecast| (forecast.rival, forecast.theirs))
                .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)));
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
                    // `deterrence-trusts-a-friend`: see
                    // `advanced/deterrence_trusts_a_friend.rs`.
                    && !self.deterrence_spares_a_friend(g, pid, other.id)
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

    /// Our power on the scale [`Self::deterrence_threat`] reports theirs:
    /// quality power under version 2, plain power under version 1.
    pub(super) fn deterrence_our_power(&self, g: &Game, pid: usize) -> f64 {
        if self.near_rival_deterrence_2 {
            Self::quality_power(g, pid, pid)
        } else {
            g.military_power(pid)
        }
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
        self.deterrence_threat(g, pid).is_some_and(|(_, theirs)| {
            self.deterrence_our_power(g, pid) < DETERRENCE_RATIO * theirs
        })
    }

    fn deterrence_land_unit(g: &Game, unit: &Name) -> bool {
        g.rules.units.get(unit).is_some_and(|spec| {
            spec.class == "military"
                && spec.domain.as_deref().is_none_or(|domain| domain == "land")
                && (spec.is_melee_capable() || spec.has_ranged_attack())
        })
    }

    /// Whether `cid`'s current queue head may give way to the deterrent.
    fn deterrence_may_displace(&self, g: &Game, pid: usize, cid: u32) -> bool {
        let Some(item) = g.cities[&cid].queue.first() else {
            return true;
        };
        // `deterrence-spares-the-faith-defence`: an uninvested district is
        // always displaceable, so a Holy Site re-queued every turn by the
        // religious defence never started while the deterrent took its slot.
        if self.deterrence_spares_the_faith_defence && Self::faith_defence_item(item) {
            return false;
        }
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

    /// A Holy Site, Shrine or Temple: the queue items the religious defence
    /// (counterweight, sanctuary, founder) relies on.
    pub(super) fn faith_defence_item(item: &Item) -> bool {
        match item {
            Item::District { district, .. } => district.as_str() == "holy_site",
            Item::Building { building } => matches!(building.as_str(), "shrine" | "temple"),
            _ => false,
        }
    }

    /// Version 2: upgrade the one land unit that gains the most strength per
    /// gold, while the gold left covers [`DETERRENCE_UPGRADE_RESERVE_BASE`]
    /// plus [`DETERRENCE_UPGRADE_RESERVE_PER_CITY`] a city and the upkeep
    /// rise fits the income.
    fn deterrence_upgrade(&self, g: &mut Game, pid: usize, rival: usize, ours: f64, theirs: f64) {
        let reserve = DETERRENCE_UPGRADE_RESERVE_BASE
            + DETERRENCE_UPGRADE_RESERVE_PER_CITY * g.player_city_ids(pid).len() as f64;
        let gold = g.players[pid].gold;
        let income = g.players[pid].gold_per_turn;
        let best = g
            .player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| {
                let unit = &g.units[&uid];
                let from = &g.rules.units[unit.kind];
                if from.class != "military"
                    || from.siege
                    || from.promotion_class == "recon"
                    || matches!(from.domain.as_deref(), Some("sea" | "air"))
                    || self.amphibious_locked(uid)
                {
                    return None;
                }
                let (target, price, _) = g.unit_gold_upgrade_offer(pid, uid)?;
                let to = &g.rules.units[target];
                let gain = to.strength.max(to.ranged_attack_strength())
                    - from.strength.max(from.ranged_attack_strength());
                let upkeep = (to.maintenance - from.maintenance).max(0.0);
                (gain > 0.0 && gold - price >= reserve && income >= upkeep).then_some((
                    gain / price.max(1.0),
                    std::cmp::Reverse(uid),
                    uid,
                    target,
                    price,
                ))
            })
            .max_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let Some((_, _, uid, target, price)) = best else {
            return;
        };
        let from = g.units[&uid].kind;
        if g.apply(pid, &Action::UpgradeUnit { unit: uid }).is_ok() {
            think!(self.journal(), Military, Decision,
                "Upgrading against {}", g.players[rival].civ;
                "near-rival-deterrence-2: {} to {} for {price:.0} gold; our quality power {ours:.0} \
                 against their {theirs:.0}",
                plain(&from), plain(&target));
        }
    }

    /// See the module: one land unit in the city that trains it soonest while
    /// a neighbour out-guns us. Exact no-op with the gene off.
    pub(super) fn claim_deterrence_unit(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let Some((rival, theirs)) = self.deterrence_threat(g, pid) else {
            return;
        };
        let ours = self.deterrence_our_power(g, pid);
        if ours >= DETERRENCE_RATIO * theirs {
            return;
        }
        if self.near_rival_deterrence_2 {
            self.deterrence_upgrade(g, pid, rival, ours, theirs);
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
                || !self.deterrence_may_displace(g, pid, cid)
                || g.cities[&cid]
                    .queue
                    .first()
                    .is_some_and(|head| self.bound_granary_holds(g, pid, cid, plan, head))
                || g.cities[&cid].queue.first().is_some_and(|head| {
                    self.capped_aqueduct_holds(g, pid, cid, plan, head)
                        || self.capped_neighborhood_holds(g, pid, cid, plan, head)
                })
                || g.cities[&cid]
                    .queue
                    .first()
                    .is_some_and(|head| self.producer_zone_claim_holds(g, pid, cid, plan, head))
                || g.cities[&cid]
                    .queue
                    .first()
                    .is_some_and(|head| self.factory_claim_holds(g, pid, cid, plan, head))
                || g.cities[&cid]
                    .queue
                    .first()
                    .is_some_and(|head| self.campus_building_claim_holds(g, pid, cid, plan, head))
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
        let forecast = self
            .near_rival_deterrence_2
            .then(|| self.threat_forecast(g, pid, rival))
            .flatten();
        if g.apply(pid, &Action::Produce { city: cid, item }).is_ok() {
            if let Some(forecast) = forecast {
                think!(self.journal(), Military, Decision,
                    "{} trains a deterrent against {}", g.cities[&cid].name, g.players[rival].civ;
                    "near-rival-deterrence-2: {} — forecast {:.0}% to declare within ten turns \
                     (nearest city {} tiles, {} soldiers at our door); their quality power \
                     {theirs:.1} (mean strength {:.0}) against our {ours:.1} ({:.0}); {turns:.1} \
                     turns{}",
                    unit.replace('_', " "), 100.0 * forecast.hazard, forecast.nearest,
                    forecast.massed, forecast.their_quality, forecast.our_quality,
                    displaced.map_or(String::new(), |item| format!(
                        "; {} keeps its progress", Self::plain_item(&item))));
            } else {
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
}

#[cfg(test)]
mod tests;
