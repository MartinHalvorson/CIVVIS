//! `near-rival-deterrence`: from live turn 30 to 80, while a met major at
//! peace has a city within [`DETERRENCE_NEAR_TILES`] of ours and our military
//! power is below its own, one idle or routine queue a turn trains a land
//! unit, never a siege unit (at most [`DETERRENCE_MAX_QUEUED`] in training),
//! until we stand level.
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
//! `near-rival-deterrence-holds-late` carries the same claim, its holds and
//! its exemptions past the window at a lower target; see
//! `advanced/near_rival_deterrence_holds_late.rs`.
//! `near-rival-deterrence-opens-early` opens the window at live turn 10 and
//! counts a soldier at the capital's door as a neighbour; see
//! `advanced/near_rival_deterrence_opens_early.rs`.

use super::near_rival_deterrence_holds_late::DeterrencePhase;
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
        // `near-rival-deterrence-holds-late`: past the window the late gene
        // answers the same neighbour. See
        // `advanced/near_rival_deterrence_holds_late.rs`.
        let phase = self.deterrence_phase(g)?;
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
                    // `deterrence-trusts-a-friend`: see
                    // `advanced/deterrence_trusts_a_friend.rs`.
                    && !self.deterrence_spares_a_friend(g, pid, other.id)
            })
            .filter(|other| {
                Self::deterrence_city_near(g, &ours, other.id)
                    // `near-rival-deterrence-opens-early`: in the opening, a
                    // soldier at the capital's door makes a neighbour too. See
                    // `advanced/near_rival_deterrence_opens_early.rs`.
                    || (phase == DeterrencePhase::Opening
                        && self.deterrence_door_neighbour(g, other.id))
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
        self.deterrence_threat(g, pid).is_some_and(|(_, theirs)| {
            g.military_power(pid) < self.deterrence_ratio(g) * theirs
                // `deterrence-stops-at-a-garrison`: out of reach, only a
                // garrisonless city's ranged garrison is held.
                && (!self.deterrence_out_of_reach(g, pid, theirs)
                    || Self::deterrence_garrison_head(g, pid, item))
        })
    }

    /// A land unit that can fight and is not a siege gun: a siege unit is a
    /// poor deterrent and no city defender.
    pub(super) fn deterrence_land_unit(g: &Game, unit: &Name) -> bool {
        g.rules.units.get(unit).is_some_and(|spec| {
            spec.class == "military"
                && !spec.siege
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

    /// Whether `cid`'s queue is spared the deterrent: the city is under
    /// attack, the early city floor's Settler, Walls, a wonder or an
    /// investment in progress holds its head (`deterrence_may_displace`), or
    /// another claim's hold keeps the head. Shared by `near-rival-deterrence`
    /// and `near-rival-deterrence-holds-late`.
    pub(super) fn deterrence_spares_the_city(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        plan.threatened_city == Some(cid)
            || self.early_settler_floor_holds(g, pid, cid, plan)
            || !self.deterrence_may_displace(g, pid, cid)
            // `culture-seat-builds-past-the-deterrent`: see
            // `advanced/culture_past_the_deterrent.rs`.
            || self.deterrent_spares_the_culture_build(g, pid, cid, plan)
            || self.first_seowon_holds_parity(g, pid, cid, plan)
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
            // `campus-and-library-before-the-claims`: its first Campus goes
            // first too.
            || g.cities[&cid]
                .queue
                .first()
                .is_some_and(|head| self.campus_and_library_holds(g, pid, cid, plan, head))
            // `domination-seat-raises-its-culture`: its Monument goes first
            // at peace.
            || g.cities[&cid]
                .queue
                .first()
                .is_some_and(|head| self.domination_monument_holds(g, pid, cid, plan, head))
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

    /// See the module: one land unit in the city that trains it soonest while
    /// a neighbour out-guns us, then (`near-rival-deterrence-holds-late`
    /// only) the next tier of Walls. Exact no-op with both genes off.
    pub(super) fn claim_deterrence_unit(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        self.train_deterrence_unit(g, pid, plan);
        self.claim_late_deterrence_walls(g, pid, plan);
    }

    /// The unit leg of [`Self::claim_deterrence_unit`].
    fn train_deterrence_unit(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let Some(phase) = self.deterrence_phase(g) else {
            return;
        };
        let Some((rival, theirs)) = self.deterrence_threat(g, pid) else {
            return;
        };
        let ours = g.military_power(pid);
        let ratio = self.deterrence_ratio(g);
        if ours >= ratio * theirs {
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
        // `deterrence-stops-at-a-garrison`: against twice our power or more,
        // a city with a ranged garrison trains no deterrent and one without
        // trains only that garrison.
        let out_of_reach = self.deterrence_out_of_reach(g, pid, theirs);
        let mut garrisoned = 0usize;
        let mut best: Option<(f64, i32, u32, String)> = None;
        for &cid in &city_ids {
            if self.deterrence_spares_the_city(g, pid, cid, plan)
                // `near-rival-deterrence-holds-late`: a Culture seat's Theater
                // Square work keeps its queue past the window.
                || (phase == DeterrencePhase::Late && self.late_deterrent_spares_the_theater(g, cid))
            {
                continue;
            }
            if out_of_reach && Self::city_has_ranged_garrison(g, pid, cid) {
                garrisoned += 1;
                continue;
            }
            let pick = if out_of_reach {
                Self::garrison_shooter_for(g, pid, cid).map(|unit| unit.to_string())
            } else {
                // `best_military`'s ranged pick is the highest ranged
                // strength, so a Catapult beat an Archer: when the army is due
                // a ranged unit the deterrent is the strongest non-siege
                // shooter, or the melee pick when none can be trained.
                self.base
                    .combined_arms_unit(g, pid, cid, counts.melee, counts.ranged)
                    .and_then(|unit| {
                        if !g
                            .rules
                            .units
                            .get(&Name::new(&unit))
                            .is_some_and(|spec| spec.has_ranged_attack())
                        {
                            return Some(unit);
                        }
                        Self::garrison_shooter_for(g, pid, cid)
                            .map(|unit| unit.to_string())
                            .or_else(|| self.base.best_military(g, pid, cid, Some(false)))
                    })
            };
            let Some(unit) = pick.filter(|unit| Self::deterrence_land_unit(g, &Name::new(unit)))
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
            if out_of_reach && garrisoned > 0 {
                think!(self.journal(), Military, Detail,
                    "No deterrent against {}: it is out of reach", g.players[rival].civ;
                    "deterrence-stops-at-a-garrison: {theirs:.0} power against our {ours:.0}, \
                     {:.1}x or more; {garrisoned} of our cities already hold a ranged garrison",
                    super::deterrence_stops_at_a_garrison::DETERRENCE_OUT_OF_REACH);
            }
            return;
        };
        let displaced = g.cities[&cid].queue.first().cloned();
        // `near-rival-deterrence-opens-early`: why this gene, not the opening
        // gene alone, armed the claim. `None` with the gene off.
        let early = (phase == DeterrencePhase::Opening)
            .then(|| self.deterrence_opens_early_reason(g, pid, rival))
            .flatten();
        let item = Item::Unit {
            unit: Name::new(&unit),
        };
        if g.apply(pid, &Action::Produce { city: cid, item }).is_ok() {
            if let Some(reason) = early {
                think!(self.journal(), Military, Decision,
                "{} trains a deterrent against {}", g.cities[&cid].name, g.players[rival].civ;
                "near-rival-deterrence-opens-early: {} for {ours:.0} power against their \
                 {theirs:.0}, {reason}; {turns:.1} turns{}{}",
                unit.replace('_', " "),
                displaced.map_or(String::new(), |item| format!(
                    "; {} keeps its progress", Self::plain_item(&item))),
                if out_of_reach {
                    "; deterrence-stops-at-a-garrison: out of reach, a ranged garrison only"
                } else {
                    ""
                });
                return;
            }
            if phase == DeterrencePhase::Late {
                think!(self.journal(), Military, Decision,
                "{} trains a deterrent against {}", g.cities[&cid].name, g.players[rival].civ;
                "near-rival-deterrence-holds-late: {} for {ours:.0} power against their \
                 {theirs:.0}, target {:.0} ({ratio:.1} of theirs), a city within \
                 {DETERRENCE_NEAR_TILES} tiles; {turns:.1} turns{}",
                unit.replace('_', " "),
                ratio * theirs,
                displaced.map_or(String::new(), |item| format!(
                    "; {} keeps its progress", Self::plain_item(&item))));
                return;
            }
            think!(self.journal(), Military, Decision,
            "{} trains a deterrent against {}", g.cities[&cid].name, g.players[rival].civ;
            "near-rival-deterrence: {} for {ours:.0} power against their {theirs:.0}, a city \
             within {DETERRENCE_NEAR_TILES} tiles; {turns:.1} turns{}{}",
            unit.replace('_', " "),
            displaced.map_or(String::new(), |item| format!(
                "; {} keeps its progress", Self::plain_item(&item))),
            if out_of_reach {
                "; deterrence-stops-at-a-garrison: out of reach, a ranged garrison only"
            } else {
                ""
            });
        }
    }
}

#[cfg(test)]
mod tests;
