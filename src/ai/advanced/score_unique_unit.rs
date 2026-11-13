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
//! ⚠ Not here: improving or buying the resource the unit needs, and how
//! many are trained. `unique-unit-preference` already picks the unique unit
//! where a soldier is wanted, and `army-upgrade-materials` buys a missing
//! strategic resource for an upgrade.

use super::*;

/// The latest era (0 Ancient, 1 Classical) of a technology the gene makes a
/// goal. A later unique unit belongs to the plan's own research.
pub(crate) const SCORE_UNIQUE_UNIT_MAX_ERA: usize = 1;

/// Cities the seat holds before the goal opens: the first Settler is out and
/// the opening technologies are in.
pub(crate) const SCORE_UNIQUE_UNIT_MIN_CITIES: usize = 2;

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
}

#[cfg(test)]
mod tests;
