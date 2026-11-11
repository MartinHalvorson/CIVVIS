//! `unique-unit-closes-the-dark-age`: a seat one to four Era Score short of a
//! Normal Age before the era closes researches its unique unit's unlock and
//! trains or buys the first one, a Historic Moment
//! (`MOMENT_UNIT_CREATED_FIRST_UNIQUE`, 4).
//!
//! ## What the live games say
//!
//! Deity Online, 49 games of 2026-11-10 from 14:00Z (measured by the session
//! holding the Pericles culture goal): Online eras end at t30, 50, 70, 90 and
//! 110. Entering the Classical era, 30 seats were Dark, 19 Normal, none
//! Golden; the Dark ones fell short by 1-6, 24 of them by four or less. A
//! first unique unit is +4, verified live (234921Z: 8 -> 12 at t36 with the
//! first Hoplite; 235747Z: 2 -> 6 at t31). In all seven Pericles games the
//! first Hoplite came at t31-59, after the Ancient era had closed; Bronze
//! Working came at t22-51.
//!
//! The Dark Age is what breaks the expansion: from the same two or three
//! cities at t30, Dark seats stood at 2.9 cities at t50 against 3.4, were
//! refused 4.4 Loyalty sites a game against 1.2, and lost a city in t30-50
//! in 6 of 30 games against 1 of 19. Across 25 finished games after 20Z, the
//! 14 that collapsed to two or fewer cities had 7 Loyalty flips, all in a
//! Dark Age, three of them two-to-four citizen cities at t38-42.
//!
//! ## What the gene does
//!
//! While the seat is 1 to 4 Era Score short of its Normal threshold, has
//! never made its unique unit, and the era's end is in sight (the host's
//! countdown, or the Online schedule before the countdown starts):
//! - research takes the unit's unlocking technology once its prerequisites
//!   are known (`AdvancedAi::unique_unit_age_tech`, in the goal list behind
//!   the endgame and ahead of the optional economic detours);
//! - the city that can finish the unit before the era ends starts it (the
//!   displaced head keeps its progress); if none can, it is bought with Gold
//!   above the upkeep floor.
//!
//! It never pulls the +4 forward when it would not turn a Dark Age into a
//! Normal one: after a Normal Age the next threshold is higher, so a +4 that
//! changes nothing now only moves the risk to the next era.

use super::*;

/// The Moment the first unique unit scores.
const UNIQUE_UNIT_MOMENT: &str = "MOMENT_UNIT_CREATED_FIRST_UNIQUE";

/// Standard turns of the first era. Online's first era closes at t30.
const FIRST_ERA_STANDARD: u32 = 45;

/// Standard turns of every later era. Online's later eras last 20 turns.
const LATER_ERA_STANDARD: u32 = 30;

impl AdvancedAi {
    /// The turn the world's current era is expected to close: the host's
    /// countdown when it runs, else the speed's era schedule.
    fn unique_unit_era_end(g: &Game) -> u32 {
        g.world_era_countdown_end.unwrap_or_else(|| {
            let length = if g.world_era == 0 {
                FIRST_ERA_STANDARD
            } else {
                LATER_ERA_STANDARD
            };
            g.world_era_since
                .saturating_add(g.standard_duration(length))
        })
    }

    /// The seat's own unique unit, cheapest first.
    fn seat_unique_unit(g: &Game, pid: usize) -> Option<Name> {
        g.rules
            .units
            .iter()
            .filter(|(_, spec)| {
                spec.unique_to
                    .as_deref()
                    .is_some_and(|civilization| g.owns_civ_unique(pid, civilization))
            })
            .min_by(|a, b| a.1.cost.total_cmp(&b.1.cost).then(a.0.cmp(b.0)))
            .map(|(name, _)| *name)
    }

    /// `unique-unit-closes-the-dark-age`: Era Score short and turns left,
    /// when the first unique unit would turn this era's Dark Age Normal.
    /// `None` with the gene off.
    pub(super) fn unique_unit_age_short(&self, g: &Game, pid: usize) -> Option<(i64, u32)> {
        if !self.unique_unit_closes_the_dark_age {
            return None;
        }
        let player = &g.players[pid];
        if player.is_minor || player.is_barbarian {
            return None;
        }
        let moment = g.rules.historic_moments.get(UNIQUE_UNIT_MOMENT)?;
        let short = player.normal_age_threshold - player.era_score;
        if !(1..=moment.era_score).contains(&short) {
            return None;
        }
        let awarded = player
            .counters
            .get(&format!("historic_moment_awards:{UNIQUE_UNIT_MOMENT}"))
            .copied()
            .unwrap_or(0)
            > 0;
        let unit = Self::seat_unique_unit(g, pid)?;
        let fielded = g
            .units
            .values()
            .any(|other| other.owner == pid && other.kind == unit);
        if awarded || fielded {
            return None;
        }
        let left = Self::unique_unit_era_end(g).checked_sub(g.turn)?;
        (left > 0).then_some((short, left))
    }

    /// `unique-unit-closes-the-dark-age`: the technology to research for the
    /// unit, when it is unknown and its prerequisites are known.
    pub(super) fn unique_unit_age_tech(&self, g: &Game, pid: usize) -> Option<String> {
        self.unique_unit_age_short(g, pid)?;
        let unit = Self::seat_unique_unit(g, pid)?;
        let tech = g.rules.units[&unit].tech?;
        let player = &g.players[pid];
        if player.techs.contains(&tech) {
            return None;
        }
        let spec = g.rules.techs.get(&tech)?;
        spec.requires
            .iter()
            .all(|prereq| player.techs.contains(prereq))
            .then(|| tech.to_string())
    }

    /// `unique-unit-closes-the-dark-age`: start or buy the first unique unit
    /// so it stands before the era closes. True when it acted.
    pub(super) fn claim_unique_unit_for_the_age(&self, g: &mut Game, pid: usize) -> bool {
        let Some((short, left)) = self.unique_unit_age_short(g, pid) else {
            return false;
        };
        let Some(unit) = Self::seat_unique_unit(g, pid) else {
            return false;
        };
        let item = Item::Unit { unit };
        let cities = g.player_city_ids(pid);
        if cities.iter().any(|cid| {
            g.cities[cid].queue.first() == Some(&item)
                && Self::unique_unit_turns(g, pid, *cid, &item) < left as f64
        }) {
            return false;
        }
        let best = cities
            .iter()
            .filter(|cid| g.can_produce(pid, **cid, &item))
            .map(|cid| (*cid, Self::unique_unit_turns(g, pid, *cid, &item)))
            .filter(|(_, turns)| turns.is_finite() && *turns < left as f64)
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        if let Some((city, turns)) = best {
            if g.apply(
                pid,
                &Action::Produce {
                    city,
                    item: item.clone(),
                },
            )
            .is_ok()
            {
                think!(self.journal(), Military, Decision,
                    "{} trains the first {} to close the Age", g.cities[&city].name,
                    crate::reasoning::plain(unit.as_str());
                    "unique-unit-closes-the-dark-age: {short} Era Score short of a Normal Age, \
                     the first unique unit is worth 4; {turns:.0} turns against {left} left in the era");
                return true;
            }
        }
        let floor = self.purchase_upkeep_floor(g, pid, 0.0);
        let budget = g.players[pid].gold - floor;
        let bought = cities
            .iter()
            .filter(|cid| g.can_produce(pid, **cid, &item) && !g.purchase_is_blocked(**cid, &item))
            .filter_map(|cid| {
                g.unit_purchase_cost(pid, *cid, unit.as_str(), "gold")
                    .map(|price| (*cid, price))
            })
            .filter(|(_, price)| *price <= budget + 1e-9)
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        let Some((city, price)) = bought else {
            return false;
        };
        if g.apply(
            pid,
            &Action::Buy {
                city,
                unit,
                formation: 0,
                currency: "gold".into(),
            },
        )
        .is_err()
        {
            return false;
        }
        think!(self.journal(), Military, Decision,
            "{} buys the first {} to close the Age", g.cities[&city].name,
            crate::reasoning::plain(unit.as_str());
            "unique-unit-closes-the-dark-age: {short} Era Score short of a Normal Age with \
             {left} turns left; {price:.0} Gold, {floor:.0} kept");
        true
    }

    fn unique_unit_turns(g: &Game, pid: usize, cid: u32, item: &Item) -> f64 {
        (g.item_cost_for_city(pid, cid, item) - g.item_invested_production(cid, item)).max(0.0)
            / g.city_yields(cid).production.max(0.5)
    }
}

#[cfg(test)]
#[path = "unique_unit_closes_the_age_tests.rs"]
mod tests;
