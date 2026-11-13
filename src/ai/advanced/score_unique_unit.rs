//! `score-seat-fields-its-unique-unit`: a Score seat whose civilization has an
//! Ancient or Classical unique land soldier researches that soldier's
//! technology as a goal from its second city, so the unit can be trained
//! while it is still the best on the board.
//!
//! ## Why (33 live Deity Rome Score games past turn 100, 2026-11-12/13)
//!
//! Operator, 2026-11-13 04:32Z: *"rome is a very strong civ ... when needed,
//! wipe out our neighbors."* Rome's strength in the first third of the game
//! is the Legion (`data/units.json`: strength 40 against the Swordsman's 35,
//! `iron_working`, 10 Iron). The live seat does not field it:
//!
//! - **Five of 33 games ever held a Legion, and none more than one.** The
//!   army at turn 60 is Archers, Warriors, Spearmen and Heavy Chariots.
//! - **Iron Working lands at a median turn 48** (37-67) and Bronze Working,
//!   which reveals Iron, at about turn 34. Iron income reads 0 at turn 40 in
//!   every game and above 0 at turn 60 in five of the last 22.
//! - **The window is closed before the Iron is banked.** The host hides a
//!   unit once its upgrade can be trained: in 033756Z Iron Working came at
//!   turn 44 with no Iron, Rome's build list at turn 50 offered Warrior,
//!   Archer, Spearman and Battering Ram, and at turn 60, with 18 Iron, it
//!   offered the Man-at-Arms (Apprenticeship) and no Legion.
//! - **The research scorer does not see the unit.** 033756Z turn 39:
//!   "Researching astrology | worth 33 to the expansion plan, ahead of iron
//!   working at 23".
//! - **What it costs us.** Military at turn 40 reads about 100 against the
//!   neighbours' 250-500 (the host's figure is close to the sum of unit
//!   strengths: a Legion adds 40, an Archer 25). Rome itself fell in 5 of 14
//!   games; a rival's declaration at under 0.55 of its military cost a city
//!   in 8 of 9 cases and none in 9 at 0.85 or more (session -c2's table).
//!
//! ## What the gene does
//!
//! On a seat whose assigned victory target is Score, from its second city,
//! while the unique soldier's technology is unknown and the unit it upgrades
//! to is not yet trainable, that technology is a forced research goal: the
//! research step picks the cheapest step toward it (Bronze Working, then
//! Iron Working, for Rome). It sits behind the defensive goals (the opening
//! Archery, Walls under a threat) and ahead of every economic beeline. Off,
//! and for a civilization with no such unit, an exact no-op.
//!
//! ⚠ Not here: improving or buying the resource the unit needs.
//! `unique-unit-preference` already picks the unique unit where a soldier is
//! wanted, and `army-upgrade-materials` buys a missing strategic resource
//! for an upgrade.
//!
//! # `score-seat-raises-its-unique-corps`
//!
//! A second opt-in gene, for the same seat: once the unique soldier can be
//! trained and the seat holds [`SCORE_UNIQUE_CORPS_MIN_CITIES`] cities, it
//! keeps a corps of them, one a city, at least [`SCORE_UNIQUE_CORPS_MIN`]
//! and at most [`SCORE_UNIQUE_CORPS_MAX`] (the unit it upgrades to counts).
//! While the corps is short, the city that trains one soonest puts it at
//! the head of its queue, one in production for every three cities.
//!
//! **Why.** The assault census of 40 live Rome games (`civvis_orders
//! --explain-assault`, 690 game-frames) read a rival major's city takeable
//! (calibrated 0.5 within eight turns) in 0-5% of games at turns 30-60 with
//! the army the seat holds, in 33-40% with six Legions and a Battering Ram,
//! and in 57-68% at turns 40-60 with eight, a Ram and two Catapults. The
//! shipped board (`score-seat-takes-what-pays`) named a city in three games
//! and mustered "1 taker(s) ... 20 strength" for it: Warriors. A rival's
//! declaration at under 0.55 of its military cost us a city in 8 of 9
//! cases and none in 9 at 0.85 or more; the host's military figure is close
//! to the sum of unit strengths, so eight Legions add about 320 to a seat
//! that reads 120-200 at turns 40-60.
//!
//! **What may give way** to a corps unit: an idle queue, a building that is
//! no wonder and no Walls, a district at most half paid (never the empire's
//! first Campus), a Scout, a Trader, a Builder at most half paid, and a
//! weaker land soldier. A Settler, Walls, a wonder and work that finishes
//! next turn keep their queue. The unit it queues is kept through
//! `early_settler_floor_holds`, as the floor's Settler is.

use super::*;

/// The latest era (0 Ancient, 1 Classical) of a technology the gene makes a
/// goal. A later unique unit belongs to the plan's own research.
pub(crate) const SCORE_UNIQUE_UNIT_MAX_ERA: usize = 1;

/// Cities the seat holds before the goal opens: the first Settler is out and
/// the opening technologies are in.
pub(crate) const SCORE_UNIQUE_UNIT_MIN_CITIES: usize = 2;

/// Cities the seat holds before it raises the corps: the land grab's first
/// Settlers come first.
pub(crate) const SCORE_UNIQUE_CORPS_MIN_CITIES: usize = 3;

/// The smallest and the largest corps; between them, one unit a city.
pub(crate) const SCORE_UNIQUE_CORPS_MIN: usize = 4;
pub(crate) const SCORE_UNIQUE_CORPS_MAX: usize = 8;

/// Cities for each corps unit that may be in production at once.
pub(crate) const SCORE_UNIQUE_CORPS_CITIES_PER_BUILD: usize = 3;

/// The longest a corps unit may take, in standard turns (ten at Online).
pub(crate) const SCORE_UNIQUE_CORPS_MAX_BUILD: u32 = 15;

/// The most of a district's or a Builder's cost that may already be paid
/// for a corps unit to take its queue.
pub(crate) const SCORE_UNIQUE_CORPS_DISPLACE_SHARE: f64 = 0.5;

impl AdvancedAi {
    /// The civilization's earliest unique land soldier of the Ancient or
    /// Classical era that a technology unlocks, with that technology.
    pub(super) fn early_unique_soldier(g: &Game, pid: usize) -> Option<(Name, Name)> {
        let civ = g.players.get(pid)?.civ.as_str();
        g.rules
            .units
            .iter()
            .filter(|(_, spec)| {
                spec.unique_to.as_deref() == Some(civ)
                    && spec.class == "military"
                    && spec.strength > 0.0
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            })
            .filter_map(|(unit, spec)| {
                let tech = spec.tech?;
                let tech_spec = g.rules.techs.get(&tech)?;
                (tech_spec.era <= SCORE_UNIQUE_UNIT_MAX_ERA).then_some((
                    tech_spec.cost,
                    *unit,
                    tech,
                ))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, unit, tech)| (unit, tech))
    }

    /// See the module: the unique soldier's technology while it is unknown,
    /// the seat holds two cities, and the unit's window has not closed.
    /// `None` with the gene off or off a Score target.
    pub(super) fn score_unique_unit_research_goal(&self, g: &Game, pid: usize) -> Option<Name> {
        if !self.score_seat_fields_its_unique_unit
            || self.victory_target != Some(VictoryTarget::Score)
            || self.base.minor
            || self.base.barb
            || g.player_city_ids(pid).len() < SCORE_UNIQUE_UNIT_MIN_CITIES
        {
            return None;
        }
        let (unit, tech) = Self::early_unique_soldier(g, pid)?;
        let techs = &g.players[pid].techs;
        if techs.contains(&tech) {
            return None;
        }
        let spec = &g.rules.units[&unit];
        // The window is shut once the unit that replaces it can be trained,
        // or once it is obsolete: the host no longer offers it.
        let upgrade_known = spec
            .upgrade_to
            .and_then(|upgrade| g.rules.units.get(&upgrade))
            .and_then(|upgrade| upgrade.tech)
            .is_some_and(|upgrade_tech| techs.contains(&upgrade_tech));
        let obsolete = spec
            .obsolete_tech
            .is_some_and(|obsolete| techs.contains(&obsolete));
        (!upgrade_known && !obsolete).then_some(tech)
    }

    /// `score-seat-raises-its-unique-corps` is on and the seat aims at Score.
    fn score_unique_corps_on(&self) -> bool {
        self.score_seat_raises_its_unique_corps
            && self.victory_target == Some(VictoryTarget::Score)
            && !self.base.minor
            && !self.base.barb
    }

    /// The corps unit, how many the seat holds (the unit it upgrades to
    /// counts), how many stand at the head of a queue, and the corps size.
    /// `None` with the gene off, under three cities, or before the unit's
    /// technology is known.
    pub(super) fn score_unique_corps_census(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<(Name, usize, usize, usize)> {
        if !self.score_unique_corps_on() {
            return None;
        }
        let cities = g.player_city_ids(pid);
        if cities.len() < SCORE_UNIQUE_CORPS_MIN_CITIES {
            return None;
        }
        let (unit, tech) = Self::early_unique_soldier(g, pid)?;
        if !g.players[pid].techs.contains(&tech) {
            return None;
        }
        let upgrade = g.rules.units[&unit].upgrade_to;
        let held = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let kind = g.units[uid].kind;
                kind == unit || Some(kind) == upgrade
            })
            .count();
        let queued = cities
            .iter()
            .filter(|cid| {
                matches!(
                    g.cities[*cid].queue.first(),
                    Some(Item::Unit { unit: head }) if *head == unit
                )
            })
            .count();
        let size = cities
            .len()
            .clamp(SCORE_UNIQUE_CORPS_MIN, SCORE_UNIQUE_CORPS_MAX);
        Some((unit, held, queued, size))
    }

    /// Whether the corps unit at the head of `cid`'s queue is still wanted:
    /// the routine claims leave it alone, as they leave the floor's Settler.
    pub(super) fn score_unique_corps_holds(&self, g: &Game, pid: usize, cid: u32) -> bool {
        let Some((unit, held, queued, size)) = self.score_unique_corps_census(g, pid) else {
            return false;
        };
        matches!(
            g.cities[&cid].queue.first(),
            Some(Item::Unit { unit: head }) if *head == unit
        ) && held + queued <= size
    }

    /// Whether the head of `cid`'s queue may give way to a corps unit of
    /// `strength`. See the module.
    pub(super) fn score_unique_corps_may_displace(
        g: &Game,
        pid: usize,
        cid: u32,
        strength: f64,
    ) -> bool {
        let Some(item) = g.cities[&cid].queue.first() else {
            return true;
        };
        let cost = g.item_cost_for_city(pid, cid, item);
        let invested = g.item_invested_production(cid, item);
        if cost - invested <= g.city_yields(cid).production {
            return false;
        }
        match item {
            Item::Building { building } => g.rules.buildings.get(building).is_some_and(|spec| {
                !spec.wonder
                    && !matches!(
                        building.as_str(),
                        "walls" | "medieval_walls" | "renaissance_walls" | "ancestral_hall"
                    )
            }),
            Item::District { district, .. } => {
                let campus = crate::name!("campus");
                let first_campus = g.district_family(*district) == campus
                    && !g.player_city_ids(pid).iter().any(|other| {
                        g.cities[other]
                            .districts
                            .keys()
                            .any(|built| g.district_family(*built) == campus)
                    });
                !first_campus && invested <= SCORE_UNIQUE_CORPS_DISPLACE_SHARE * cost
            }
            Item::Unit { unit } => match unit.as_str() {
                "settler" => false,
                "scout" | "trader" => true,
                "builder" => invested <= SCORE_UNIQUE_CORPS_DISPLACE_SHARE * cost,
                _ => g.rules.units.get(unit).is_some_and(|spec| {
                    spec.class == "military"
                        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                        && spec.bombard_strength <= 0.0
                        && spec.strength < strength
                }),
            },
            _ => false,
        }
    }

    /// See the module: while the corps is short, the city that trains a
    /// corps unit soonest puts one at the head of its queue. Exact no-op
    /// with the gene off.
    pub(super) fn claim_score_unique_corps(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let Some((unit, held, queued, size)) = self.score_unique_corps_census(g, pid) else {
            return;
        };
        let city_ids = g.player_city_ids(pid);
        let lanes = (city_ids.len() / SCORE_UNIQUE_CORPS_CITIES_PER_BUILD).max(1);
        if held + queued >= size || queued >= lanes {
            return;
        }
        let item = Item::Unit { unit };
        let strength = g.rules.units[&unit].strength;
        let max_turns = g.standard_duration(SCORE_UNIQUE_CORPS_MAX_BUILD) as f64;
        let mut best: Option<(f64, u32)> = None;
        for &cid in &city_ids {
            let city = &g.cities[&cid];
            if city.queue.first() == Some(&item)
                || !g.can_produce(pid, cid, &item)
                // The floor's Settler and its like keep their queue.
                || self.early_settler_floor_holds(g, pid, cid, plan)
                || !Self::score_unique_corps_may_displace(g, pid, cid, strength)
            {
                continue;
            }
            let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                g.item_cost_for_city(pid, cid, &item) / g.city_yields(cid).production.max(0.5)
            });
            if turns > max_turns {
                continue;
            }
            if best.is_none_or(|(old, old_city)| {
                turns + 1e-9 < old || ((turns - old).abs() <= 1e-9 && cid < old_city)
            }) {
                best = Some((turns, cid));
            }
        }
        let Some((turns, cid)) = best else {
            return;
        };
        let displaced = g.cities[&cid].queue.first().cloned();
        let name = g.cities[&cid].name.clone();
        if g.apply(pid, &Action::Produce { city: cid, item }).is_ok() {
            think!(self.journal(), Military, Decision,
                "{name} trains a {} for the corps", Self::plain_item(&Item::Unit { unit });
                "score-seat-raises-its-unique-corps: {held} held and {queued} in production \
                 against a corps of {size}, {turns:.1} turns{}",
                displaced.map_or(String::new(), |item| format!(
                    "; {} keeps its progress", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
