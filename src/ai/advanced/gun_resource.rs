//! `siege-buys-the-gun-resource`: the strategic resource our better siege gun
//! waits on, bought from a rival's trade screen.
//!
//! Live Emperor civvis-20261007T111017Z (game 343) stood before Angkor Thom's
//! 400 walls at turns 176-177 with `breakers-match-the-walls` holding: "our
//! best gun hits 2.3 a shot against 109; 6 guns cannot breach 400 walls". The
//! best gun was the Trebuchet (45). Metal Casting, Steel and Military Science
//! were all researched, but the seat owned no Niter or Oil tile: the Bombard
//! needs 20 Niter a gun up front and the Artillery 1 Oil and 1 a turn. That
//! same turn the Zulu offered 82 Niter on their trade screen and Arabia 70.
//! At turn 150 of 101 Emperor runs of October 6-7, 92 lacked Niter or Oil and
//! 53 had a met rival offering one they lacked; the offers
//! (`StateRival::tradeable_strategics`, exported since #3792) had no consumer.
//! -d8 measured 1,000 walled infinite siege-turns of 10-06/07: 22% were
//! resource-blocked (Oil 13%, Niter 9%), and in those the tech-best gun hits
//! a median 9.4 a shot against 4.1 for the buildable best.
//!
//! The want is sized two ways (-d8). An up-front resource (Niter) is bought
//! for the guns the better gun's blow needs: `breakers-match-the-walls`'
//! count at that blow when the gene is on, the shipped breaker count
//! otherwise, times the gun's cost. A fuel (Oil) is per-turn upkeep, so the
//! purchase carries [`GUN_RESOURCE_FUEL_TURNS`] of it, and the want comes back
//! as a renewal once the guns fielded or queued hold fewer than
//! [`GUN_RESOURCE_FUEL_LOW_TURNS`] turns of it with no income to cover them.
//! The bridge (`append_gun_resource_buy_order` in `civvis_orders`) chooses the
//! seller and the price.

use super::siege_production::{matched_guns, BREAKER_MATCH_DEFAULT_TURNS, BREAKER_MATCH_MIN_HIT};
use super::*;
use crate::game::expected_damage;

/// A shortfall that our own income makes up for one gun within this many
/// turns is not bought.
pub(crate) const GUN_RESOURCE_WAIT_TURNS: f64 = 5.0;
/// The turns of fuel a purchase carries for each gun it is sized to.
pub(crate) const GUN_RESOURCE_FUEL_TURNS: f64 = 10.0;
/// The turns of fuel under which the fielded and queued guns ask again, and
/// the fuel the smallest worthwhile block carries.
pub(crate) const GUN_RESOURCE_FUEL_LOW_TURNS: f64 = 3.0;

/// One strategic purchase the campaign's siege guns ask for.
#[derive(Clone, Debug, PartialEq)]
pub struct GunResourceWant {
    /// The resource as the host names it on a trade screen, `RESOURCE_NITER`.
    pub resource: String,
    /// The resource as CIVVIS names it, `niter`.
    pub resource_id: String,
    /// How much of it to buy.
    pub amount: u32,
    /// The smallest block worth a deal: one gun's cost, and
    /// [`GUN_RESOURCE_FUEL_LOW_TURNS`] of its fuel.
    pub minimum: u32,
    /// The gun it unlocks or keeps firing (CIVVIS id).
    pub unit: String,
    /// The guns the block is sized to.
    pub guns: usize,
    /// The gun's expected blow against the campaign target.
    pub hit: f64,
    /// Whether this renews the fuel of guns already fielded or queued.
    pub renewal: bool,
}

impl AdvancedAi {
    /// `siege-buys-the-gun-resource`: the strategic purchases the Domination
    /// campaign's walled target asks for, strongest gun first. Empty with the
    /// gene off, off a Domination seat, or without a walled campaign target
    /// whose owner we fight or the plan names.
    ///
    /// A gun asks when our tech unlocks it, it out-hits every land siege gun
    /// we can build or field now, its blow can take the city (the
    /// `breakers-match-the-walls` model), and its resource neither stands in
    /// the stockpile nor arrives within [`GUN_RESOURCE_WAIT_TURNS`] of income.
    /// A fuel-burning gun we field or queue asks again when its fuel runs
    /// under [`GUN_RESOURCE_FUEL_LOW_TURNS`] turns of upkeep.
    pub fn siege_gun_resource_wants(&self, g: &Game, pid: usize) -> Vec<GunResourceWant> {
        if !self.siege_buys_the_gun_resource
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return Vec::new();
        }
        let Some(plan) = self.plan.as_ref() else {
            return Vec::new();
        };
        let Some((cid, city)) = plan
            .target_city
            .and_then(|cid| g.cities.get(&cid).map(|city| (cid, city)))
        else {
            return Vec::new();
        };
        if city.owner == pid
            || city.wall_hp <= 0
            || !(g.is_at_war(pid, city.owner) || plan.target_player == Some(city.owner))
        {
            return Vec::new();
        }
        let land_gun = |spec: &crate::rules::UnitSpec| {
            spec.class == "military"
                && spec.siege
                && spec.has_ranged_attack()
                && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
        };
        // Our best gun now, read as `breakers_matched_to_walls` reads it.
        let buildable = g
            .player_city_ids(pid)
            .into_iter()
            .flat_map(|ours| g.producible_items(pid, ours))
            .filter_map(|item| match item {
                Item::Unit { unit } => {
                    let spec = &g.rules.units[&unit];
                    land_gun(spec).then(|| spec.ranged_attack_strength())
                }
                _ => None,
            });
        let fielded = g
            .units
            .values()
            .filter(|unit| unit.owner == pid && land_gun(&g.rules.units[unit.kind]))
            .map(|unit| g.rules.units[unit.kind].ranged_attack_strength());
        let best_now = buildable.chain(fielded).fold(0.0_f64, f64::max);
        let defense = g.city_strength(cid);
        let walls = f64::from(city.wall_hp);
        let health = f64::from(city.hp.max(0));
        let target_turns = self
            .breakers_matched_to_walls(g, pid, cid)
            .map_or(BREAKER_MATCH_DEFAULT_TURNS, |matched| matched.target_turns);
        let stock_guns = super::objective_board::breaker_guns_wanted(city);
        let player = &g.players[pid];
        let room = g.strategic_stockpile_capacity(pid);
        let mut wants = Vec::new();
        for (kind, spec) in g.rules.units.iter() {
            let Some(resource) = spec.requires_resource else {
                continue;
            };
            if !land_gun(spec)
                || !spec.buildable
                || spec.tech.is_some_and(|tech| !player.techs.contains(&tech))
                || spec
                    .civic
                    .is_some_and(|civic| !player.civics.contains(&civic))
                || spec
                    .unique_to
                    .as_deref()
                    .is_some_and(|civ| !g.owns_civ_unique(pid, civ))
                || g.player_unit_replacement(pid, *kind) != *kind
                || g.unit_is_obsolete(pid, *kind)
            {
                continue;
            }
            let hit = expected_damage(spec.ranged_attack_strength(), defense);
            let guns = if self.breakers_match_the_walls {
                matched_guns(hit, walls, health, target_turns).map(|(guns, _, _)| guns)
            } else {
                (hit > BREAKER_MATCH_MIN_HIT).then_some(stock_guns.max(1))
            };
            let Some(guns) = guns else {
                continue;
            };
            let stock = g.strategic_stockpile(pid, resource);
            let income = g.strategic_resource_rate(pid, resource.as_str());
            let cost = spec.resource_cost.max(0.0);
            let fuel = spec.resource_maintenance.max(0.0);
            let have = g
                .units
                .values()
                .filter(|unit| unit.owner == pid && unit.kind == *kind)
                .count()
                + g.cities
                    .values()
                    .filter(|ours| ours.owner == pid)
                    .flat_map(|ours| ours.queue.iter())
                    .filter(|item| matches!(item, Item::Unit { unit } if unit == kind))
                    .count();
            let upkeep = have as f64 * fuel;
            let (amount, minimum, renewal, sized) = if fuel > 0.0
                && have > 0
                && income < upkeep
                && stock < upkeep * GUN_RESOURCE_FUEL_LOW_TURNS
            {
                (
                    upkeep * GUN_RESOURCE_FUEL_TURNS - stock,
                    upkeep * GUN_RESOURCE_FUEL_LOW_TURNS - stock,
                    true,
                    have,
                )
            } else if spec.ranged_attack_strength() > best_now
                && stock + income * GUN_RESOURCE_WAIT_TURNS < cost.max(fuel)
            {
                let gun = cost + fuel * GUN_RESOURCE_FUEL_TURNS;
                (
                    guns as f64 * gun - stock,
                    cost + fuel * GUN_RESOURCE_FUEL_LOW_TURNS - stock,
                    false,
                    guns,
                )
            } else {
                continue;
            };
            let limit = (room - stock).max(0.0);
            let amount = amount.min(limit).ceil();
            let minimum = minimum.max(1.0).ceil();
            if amount < minimum {
                continue;
            }
            wants.push(GunResourceWant {
                resource: format!("RESOURCE_{}", resource.as_str().to_ascii_uppercase()),
                resource_id: resource.as_str().to_string(),
                amount: amount as u32,
                minimum: minimum as u32,
                unit: kind.as_str().to_string(),
                guns: sized,
                hit,
                renewal,
            });
        }
        wants.sort_by(|left, right| {
            right
                .hit
                .partial_cmp(&left.hit)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| right.renewal.cmp(&left.renewal))
                .then_with(|| left.unit.cmp(&right.unit))
        });
        if let Some(want) = wants.first() {
            think!(self.journal(), Military, Detail,
                "Siege of {}: the {} waits on {}", city.name, want.unit, want.resource_id;
                "{} {} for {} gun(s) that hit {:.1} a shot against {:.0}, where our best gun now hits {:.1}{}",
                want.amount, want.resource_id, want.guns, want.hit, defense,
                expected_damage(best_now, defense),
                if want.renewal { "; the fielded guns' fuel runs low" } else { "" };
                city.pos);
        }
        wants
    }
}

#[cfg(test)]
mod tests;
