//! `first-ship-closes-the-dark-age` (opt-in): a seat one or two Era Score
//! short of a Normal Age before the era closes trains, or buys, its first
//! ship: a Historic Moment (`MOMENT_UNIT_CREATED_FIRST_DOMAIN_SEA`, 2; 3 when
//! no civilization has one yet).
//!
//! ## What the live games say (Tournament 6 and Lv 8 Deity, 2026-11-21/22)
//!
//! Over 51 recorded games the seat entered 128 Dark Ages; 71 of them were one
//! to three points short of the Normal threshold on the era's last turn, and
//! 41 of those 71 had never built a ship. Many are the first era (t30 at
//! Online, threshold 8): the openings research Sailing at t25-32 and a Galley
//! costs a few turns of a coastal capital. `MOMENT_UNIT_CREATED_FIRST_DOMAIN_SEA`
//! is EraScore 2 and `..._IN_WORLD` 3 in `Expansion1_Moments.xml`; the engine
//! books it in `Game::note_unit_created_moments` under the
//! `historic_moment:first_sea_unit` guard. The cost of the Dark Age itself is
//! measured in `advanced/unique_unit_closes_the_age.rs`: Dark seats stood at
//! fewer cities at t50 and lost a city in t30-50 six times as often.
//!
//! ## What the gene does
//!
//! While the seat is 1 to 2 Era Score short of its Normal threshold (the
//! ordinary Moment's value; the world-first bonus is not counted on), has
//! never earned the Moment (the award counters, which the mirror now fills
//! from the host's `historic_moments`, and no ship of ours on the map), and the
//! era's end is in sight (`unique_unit_era_end`): the coastal city that can
//! finish the cheapest ship it can build before the era ends starts it (the
//! displaced head keeps its progress); if none can, the ship is bought with
//! Gold above the upkeep floor. Nothing is researched for it: a seat without a
//! ship it can build is left alone.

use super::*;

/// The ordinary Moment, and the world-first one.
const FIRST_SHIP_MOMENT: &str = "MOMENT_UNIT_CREATED_FIRST_DOMAIN_SEA";
const FIRST_SHIP_IN_WORLD: &str = "MOMENT_UNIT_CREATED_FIRST_DOMAIN_SEA_IN_WORLD";

impl AdvancedAi {
    /// `first-ship-closes-the-dark-age`: how far short of a Normal Age the
    /// seat is and how many turns the era has left, while a first ship would
    /// close it. `None` when the gene is off or the ship would not help.
    pub(super) fn first_ship_age_short(&self, g: &Game, pid: usize) -> Option<(i64, u32)> {
        if !self.first_ship_closes_the_dark_age {
            return None;
        }
        let player = &g.players[pid];
        if player.is_minor || player.is_barbarian {
            return None;
        }
        let value = g.rules.historic_moments.get(FIRST_SHIP_MOMENT)?.era_score;
        let short = player.normal_age_threshold - player.era_score;
        if !(1..=value).contains(&short) {
            return None;
        }
        let awarded = [FIRST_SHIP_MOMENT, FIRST_SHIP_IN_WORLD]
            .iter()
            .any(|moment| {
                player
                    .counters
                    .get(&format!("historic_moment_awards:{moment}"))
                    .copied()
                    .unwrap_or(0)
                    > 0
            })
            || player
                .counters
                .contains_key("historic_moment:first_sea_unit");
        let afloat = g.units.values().any(|unit| {
            unit.owner == pid && g.rules.units[unit.kind].domain.as_deref() == Some("sea")
        });
        if awarded || afloat {
            return None;
        }
        let left = Self::unique_unit_era_end(g).checked_sub(g.turn)?;
        (left > 0).then_some((short, left))
    }

    /// The cheapest ship `city` can build now, with the turns it takes there.
    fn first_ship_for(g: &Game, pid: usize, city: u32) -> Option<(Item, f64)> {
        g.rules
            .units
            .iter()
            .filter(|(_, spec)| spec.domain.as_deref() == Some("sea"))
            .map(|(name, _)| Item::Unit { unit: *name })
            .filter(|item| g.can_produce(pid, city, item))
            .map(|item| {
                let left = (g.item_cost_for_city(pid, city, &item)
                    - g.item_invested_production(city, &item))
                .max(0.0);
                let turns = left / g.city_yields(city).production.max(0.5);
                (item, turns)
            })
            .filter(|(_, turns)| turns.is_finite())
            .min_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then_with(|| format!("{:?}", a.0).cmp(&format!("{:?}", b.0)))
            })
    }

    /// `first-ship-closes-the-dark-age`: start or buy the first ship so it
    /// stands before the era closes. True when it acted.
    pub(super) fn claim_first_ship_for_the_age(&self, g: &mut Game, pid: usize) -> bool {
        let Some((short, left)) = self.first_ship_age_short(g, pid) else {
            return false;
        };
        let cities = g.player_city_ids(pid);
        // Already under way somewhere and in time: nothing to do.
        let in_time = cities.iter().any(|cid| {
            matches!(g.cities[cid].queue.first(), Some(Item::Unit { unit })
                if g.rules.units.get(unit).is_some_and(|spec| spec.domain.as_deref() == Some("sea")))
                && Self::first_ship_for(g, pid, *cid).is_some_and(|(_, turns)| turns.ceil() < left as f64)
        });
        if in_time {
            return false;
        }
        // The first unique unit is worth 4 and claims its city first: never
        // displace it.
        let unique = Self::seat_unique_unit(g, pid).map(|unit| Item::Unit { unit });
        let best = cities
            .iter()
            .filter(|cid| unique.is_none() || g.cities[*cid].queue.first() != unique.as_ref())
            .filter_map(|cid| {
                Self::first_ship_for(g, pid, *cid).map(|(item, turns)| (*cid, item, turns))
            })
            .filter(|(_, _, turns)| turns.ceil() < left as f64)
            .min_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)));
        if let Some((city, item, turns)) = best {
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
                    "{} builds the first ship to close the Age", g.cities[&city].name;
                    "first-ship-closes-the-dark-age: {short} Era Score short of a Normal Age, \
                     the first ship is worth 2; {turns:.0} turns against {left} left in the era");
                return true;
            }
        }
        let floor = self.purchase_upkeep_floor(g, pid, 0.0);
        let budget = g.players[pid].gold - floor;
        let bought = cities
            .iter()
            .filter_map(|cid| {
                let (item, _) = Self::first_ship_for(g, pid, *cid)?;
                let Item::Unit { unit } = item else {
                    return None;
                };
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
            "{} buys the first ship to close the Age", g.cities[&city].name;
            "first-ship-closes-the-dark-age: {short} Era Score short of a Normal Age with \
             {left} turns left; {price:.0} Gold, {floor:.0} kept");
        true
    }
}

#[cfg(test)]
mod tests;
