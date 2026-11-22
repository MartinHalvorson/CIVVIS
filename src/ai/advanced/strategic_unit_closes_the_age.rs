//! `strategic-unit-closes-the-dark-age` (opt-in): a seat one Era Score short
//! of a Normal Age before the era closes trains, or buys, the first unit of a
//! strategic resource it has not used yet: a Historic Moment
//! (`MOMENT_UNIT_CREATED_FIRST_REQUIRING_STRATEGIC`, 1; 2 first in the world)
//! that pays once per resource.
//!
//! ## What the live games say (Tournament 6/7 and Lv 8 Deity, 2026-11-21/22)
//!
//! Over 51 recorded games the seat entered 128 Dark Ages, 26 of them exactly
//! one point short of the Normal threshold on the era's last turn. The Moment
//! is per resource type: the host's `historic_moments` reads it 4 or 5 times a
//! game (Horses, Iron, Niter, Coal, Oil...), so a seat that holds a resource no
//! unit of its own has used yet has a one-point Moment one unit away.
//! `Expansion1_Moments.xml`: `MOMENT_UNIT_CREATED_FIRST_REQUIRING_STRATEGIC`
//! EraScore 1, `..._IN_WORLD` 2; the engine books it in
//! `Game::note_unit_created_moments` under `historic_moment:first_unit_using_resource:<resource>`.
//! What a Dark Age costs: `advanced/unique_unit_closes_the_age.rs`.
//!
//! ## What the gene does
//!
//! While the seat is exactly 1 Era Score short of its Normal threshold and the
//! era's end is in sight (`unique_unit_era_end`), the cheapest unit it can
//! build that needs a strategic resource no unit of ours has used (the
//! engine's per-resource guard, or a unit of ours on the map that needs it) is
//! started in the city that finishes it in whole turns strictly inside the
//! era; else it is bought with Gold above the upkeep floor. It leaves a city
//! alone whose head is the seat's unique unit or a ship, the other age closers'
//! claims, and does nothing while a first ship is already under way to close
//! the same gap.

use super::*;

const STRATEGIC_MOMENT: &str = "MOMENT_UNIT_CREATED_FIRST_REQUIRING_STRATEGIC";

impl AdvancedAi {
    /// Turns left in the era while one strategic-unit Moment would close the
    /// Age. `None` when the gene is off or it would not help.
    pub(super) fn strategic_unit_age_short(&self, g: &Game, pid: usize) -> Option<u32> {
        if !self.strategic_unit_closes_the_dark_age {
            return None;
        }
        let player = &g.players[pid];
        if player.is_minor || player.is_barbarian {
            return None;
        }
        let value = g.rules.historic_moments.get(STRATEGIC_MOMENT)?.era_score;
        let short = player.normal_age_threshold - player.era_score;
        if short < 1 || short > value {
            return None;
        }
        let left = Self::unique_unit_era_end(g).checked_sub(g.turn)?;
        (left > 0).then_some(left)
    }

    /// Whether a unit of ours has already used `resource`, by the engine's own
    /// guard or a unit on the map that needs it.
    fn strategic_resource_used(g: &Game, pid: usize, resource: &Name) -> bool {
        g.players[pid].counters.contains_key(&format!(
            "historic_moment:first_unit_using_resource:{resource}"
        )) || g.units.values().any(|unit| {
            unit.owner == pid
                && g.rules.units[unit.kind].requires_resource.as_ref() == Some(resource)
        })
    }

    /// The cheapest unit `city` can build now that needs a strategic resource
    /// no unit of ours has used, with its whole production turns there.
    fn strategic_unit_for(g: &Game, pid: usize, city: u32) -> Option<(Name, f64)> {
        g.rules
            .units
            .iter()
            .filter(|(_, spec)| {
                spec.requires_resource
                    .as_ref()
                    .is_some_and(|resource| !Self::strategic_resource_used(g, pid, resource))
            })
            .map(|(name, _)| *name)
            .filter(|unit| g.can_produce(pid, city, &Item::Unit { unit: *unit }))
            .map(|unit| {
                let item = Item::Unit { unit };
                let left = (g.item_cost_for_city(pid, city, &item)
                    - g.item_invested_production(city, &item))
                .max(0.0);
                (
                    unit,
                    (left / g.city_yields(city).production.max(0.5)).ceil(),
                )
            })
            .filter(|(_, turns)| turns.is_finite())
            .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
    }

    /// `strategic-unit-closes-the-dark-age`: start or buy that unit so it
    /// stands before the era closes. True when it acted.
    pub(super) fn claim_strategic_unit_for_the_age(&self, g: &mut Game, pid: usize) -> bool {
        let Some(left) = self.strategic_unit_age_short(g, pid) else {
            return false;
        };
        let cities = g.player_city_ids(pid);
        let unique = Self::seat_unique_unit(g, pid);
        let held_by_another_closer = |g: &Game, cid: u32| match g.cities[&cid].queue.first() {
            Some(Item::Unit { unit }) => {
                Some(*unit) == unique
                    || g.rules
                        .units
                        .get(unit)
                        .is_some_and(|spec| spec.domain.as_deref() == Some("sea"))
            }
            _ => false,
        };
        // A first ship already under way closes the same point.
        if self.first_ship_age_short(g, pid).is_some()
            && cities.iter().any(|cid| held_by_another_closer(g, *cid))
        {
            return false;
        }
        // Already under way somewhere and in time: nothing to do.
        let in_time = cities.iter().any(|cid| {
            matches!(g.cities[cid].queue.first(), Some(Item::Unit { unit })
                if g.rules.units.get(unit).and_then(|spec| spec.requires_resource.as_ref())
                    .is_some_and(|resource| !Self::strategic_resource_used(g, pid, resource)))
                && Self::strategic_unit_for(g, pid, *cid)
                    .is_some_and(|(_, turns)| turns < left as f64)
        });
        if in_time {
            return false;
        }
        let best = cities
            .iter()
            .filter(|cid| !held_by_another_closer(g, **cid))
            .filter_map(|cid| {
                Self::strategic_unit_for(g, pid, *cid).map(|(unit, turns)| (*cid, unit, turns))
            })
            .filter(|(_, _, turns)| *turns < left as f64)
            .min_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)));
        if let Some((city, unit, turns)) = best {
            if g.apply(
                pid,
                &Action::Produce {
                    city,
                    item: Item::Unit { unit },
                },
            )
            .is_ok()
            {
                think!(self.journal(), Military, Decision,
                    "{} trains a first {} to close the Age", g.cities[&city].name,
                    crate::reasoning::plain(unit.as_str());
                    "strategic-unit-closes-the-dark-age: 1 Era Score short of a Normal Age, \
                     the first unit of a strategic resource is worth 1; {turns:.0} turns \
                     against {left} left in the era");
                return true;
            }
        }
        let floor = self.purchase_upkeep_floor(g, pid, 0.0);
        let budget = g.players[pid].gold - floor;
        let bought = cities
            .iter()
            .filter(|cid| !held_by_another_closer(g, **cid))
            .filter_map(|cid| {
                let (unit, _) = Self::strategic_unit_for(g, pid, *cid)?;
                if g.purchase_is_blocked(*cid, &Item::Unit { unit }) {
                    return None;
                }
                g.unit_purchase_cost(pid, *cid, unit.as_str(), "gold")
                    .map(|price| (*cid, unit, price))
            })
            .filter(|(_, _, price)| *price <= budget + 1e-9)
            .min_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)));
        let Some((city, unit, price)) = bought else {
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
            "{} buys a first {} to close the Age", g.cities[&city].name,
            crate::reasoning::plain(unit.as_str());
            "strategic-unit-closes-the-dark-age: 1 Era Score short of a Normal Age with \
             {left} turns left; {price:.0} Gold, {floor:.0} kept");
        true
    }
}

#[cfg(test)]
mod tests;
