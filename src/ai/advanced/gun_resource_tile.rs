//! `gun-resource-buys-the-tile`: the Domination seat buys the tile of the
//! Niter or Oil its siege guns wait on, or the tile that brings that deposit
//! within a city's reach.
//!
//! Seven live Emperor Domination games of 2026-11-16 (Gilgamesh) spent 274
//! turns with the gun's technology in hand and its resource missing (no
//! income, under one gun's block): 71 Niter turns after Metal Casting, 203
//! Oil turns after Steel. A rival at peace offered a block on 217 of them,
//! but `siege-buys-the-gun-resource`'s asks closed 2 of the 25 the host
//! answered (23 REJECTED, 17 of the 22 Niter or Oil answers from a rival that
//! had denounced us), and over 11-15/16 no Gold-only Niter or Oil ask closed
//! (0 of 8, 200 Gold for 10 Niter among them). Our own cities' purchase menus
//! were the sure source the seat left on the table:
//! - civvis-20261116T035719Z: Metal Casting at turn 75, a Niter deposit
//!   unowned in Akshak's third ring until turn 103, one 50-Gold tile from
//!   its menu ((21,32), Gold 104-213 those turns); the seat drew no Niter
//!   until 121, and fielded its first Bombard after that.
//! - civvis-20261116T023139Z: Steel at 176, an Oil deposit in Ur's third ring
//!   one 90-Gold tile away, then on Ur's own menu at 140 Gold from 186 to the
//!   end (Gold 180-317); the seat never drew Oil, built no Artillery.
//! - civvis-20261116T024741Z: Lagash's menu quoted its Oil at 125 at 142-144.
//!
//! Over the seven games a tile buy (direct, or the first of two) was
//! affordable on 57 of the 274 starved turns (Niter 31 of 71, Oil 26 of 203),
//! against 0 tile buys made.
//!
//! The rule: while a land siege gun we can build needs a revealed strategic
//! resource we draw none of and hold less than one gun's block of
//! (`gun_tile_resources_wanted`), the purchase pass buys, out of the upkeep
//! floor (the threatened city's defender when that reserve is on), the
//! cheapest land deposit of it a city can buy; failing one, the cheapest
//! tile a city can buy beside an unowned land deposit in that city's third
//! ring, so the deposit is on the city's menu the next turn. One tile a
//! turn, and none once a workable land deposit of it lies inside our borders
//! (a sea deposit is not workable before Plastics: civvis-20261116T024741Z
//! owned an offshore Oil deposit from 144 while Lagash's menu quoted a land
//! one).

use super::*;
use crate::name::Name;

/// The farthest ring a city buys a plot in: the host quotes plots up to
/// three tiles from the centre, beside the city's own territory.
const GUN_TILE_RING: i32 = 3;

/// A land siege gun, read as `siege-buys-the-gun-resource` reads one.
fn land_gun(spec: &crate::rules::UnitSpec) -> bool {
    spec.class == "military"
        && spec.siege
        && spec.has_ranged_attack()
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
}

impl AdvancedAi {
    /// `gun-resource-buys-the-tile`: the strategic resources a land siege
    /// gun we can build now needs (tech and civic in hand, our own version,
    /// not obsolete), revealed to us, of which the empire draws no income,
    /// holds less than one gun's block (its cost plus
    /// [`super::gun_resource::GUN_RESOURCE_FUEL_TURNS`] of its upkeep) and
    /// owns no deposit yet. Empty off a Domination seat.
    pub(super) fn gun_tile_resources_wanted(&self, g: &Game, pid: usize) -> BTreeSet<Name> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination) {
            return BTreeSet::new();
        }
        let player = &g.players[pid];
        let cities: BTreeSet<u32> = g.player_city_ids(pid).into_iter().collect();
        // A workable land deposit inside our borders already: its Builder
        // brings the income, and another tile would only wait beside it. A
        // sea deposit is not one (its Offshore Oil Rig waits on Plastics).
        let owned = |resource: Name| {
            g.map.tiles.values().any(|tile| {
                tile.resource == Some(resource)
                    && !g.rules.is_water(tile)
                    && tile.district.is_none()
                    && tile.wonder.is_none()
                    && tile.owner_city.is_some_and(|cid| cities.contains(&cid))
            })
        };
        let mut wanted = BTreeSet::new();
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
            let revealed = g.rules.resources.get(&resource).is_some_and(|res| {
                res.class == "strategic"
                    && res
                        .tech
                        .as_ref()
                        .is_none_or(|tech| player.techs.contains(tech))
            });
            let block = spec.resource_cost.max(0.0)
                + spec.resource_maintenance.max(0.0) * super::gun_resource::GUN_RESOURCE_FUEL_TURNS;
            if revealed
                && g.strategic_resource_rate(pid, resource.as_str()) <= 0.0
                && g.strategic_stockpile(pid, resource) < block
                && !wanted.contains(&resource)
                && !owned(resource)
            {
                wanted.insert(resource);
            }
        }
        wanted
    }

    /// `gun-resource-buys-the-tile`: the reserve the tile buy keeps, given
    /// the purchase pass's `reserve`: the army's upkeep floor (and the
    /// threatened city's defender, `threatened-city-reserve`), never more
    /// than `reserve`. The tile is what the war's guns wait on, as
    /// `aluminum-tile-spends-the-war-chest` reads the Bomber's.
    pub(super) fn gun_resource_tile_reserve(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        reserve: f64,
    ) -> f64 {
        let upkeep = super::gold_and_cards::upgrade_floor_from(0.0, g.unit_gold_maintenance(pid));
        self.reserve_for_the_threatened_city(g, pid, plan, upkeep)
            .min(reserve)
    }

    /// `gun-resource-buys-the-tile`: buy the deposit, or the step to it.
    /// True when a tile was bought. An exact no-op with the gene off. See the
    /// module header.
    pub(super) fn gun_resource_tile_purchase(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        reserve: f64,
    ) -> bool {
        if !self.gun_resource_buys_the_tile {
            return false;
        }
        let wanted = self.gun_tile_resources_wanted(g, pid);
        if wanted.is_empty() {
            return false;
        }
        // An unowned land deposit of a wanted resource: a Builder can work it.
        let deposit_of = |g: &Game, pos: Pos| -> Option<Name> {
            g.map.get(pos).and_then(|tile| {
                tile.resource.filter(|resource| {
                    tile.owner_city.is_none()
                        && !g.rules.is_water(tile)
                        && wanted.contains(resource)
                })
            })
        };
        // (step, cost, pos, the deposit, its resource, the buying city, action)
        let mut best: Option<(bool, f64, Pos, Pos, Name, u32, Action)> = None;
        for action in self.legal_purchase_actions(g, pid) {
            let Action::BuyPlot { city, pos, cost } = action else {
                continue;
            };
            let candidate = if let Some(resource) = deposit_of(g, pos) {
                (false, cost, pos, pos, resource)
            } else {
                let Some(centre) = g.cities.get(&city).map(|c| c.pos) else {
                    continue;
                };
                let step = g
                    .nbrs(pos)
                    .into_iter()
                    .filter(|deposit| {
                        g.wdist(centre, *deposit) <= GUN_TILE_RING
                            && g.players[pid].explored.contains(deposit)
                    })
                    .filter_map(|deposit| deposit_of(g, deposit).map(|r| (deposit, r)))
                    .min_by_key(|(deposit, _)| *deposit);
                let Some((deposit, resource)) = step else {
                    continue;
                };
                (true, cost, pos, deposit, resource)
            };
            let (step, cost, pos, deposit, resource) = candidate;
            // A deposit before a step, then the cheaper, then the lower plot.
            let better = best.as_ref().is_none_or(|(s, c, p, ..)| {
                step.cmp(s).then(cost.total_cmp(c)).then(pos.cmp(p)).is_lt()
            });
            if better {
                best = Some((step, cost, pos, deposit, resource, city, action));
            }
        }
        let Some((step, cost, pos, deposit, resource, city, action)) = best else {
            return false;
        };
        let keep = self.gun_resource_tile_reserve(g, pid, plan, reserve);
        let name = g
            .cities
            .get(&city)
            .map(|c| c.name.clone())
            .unwrap_or_default();
        let what = if step {
            "the tile beside"
        } else {
            "the tile of"
        };
        if g.players[pid].gold + f64::EPSILON < keep + cost {
            think!(self.journal(), Economy, Detail,
                   "gun-resource-buys-the-tile: {} waits on the treasury for {} the {}", name, what, resource;
                   "gun-resource-buys-the-tile: {cost:.0} Gold for {pos:?} (deposit {deposit:?}) would leave less than the {keep:.0} reserve";
                   pos);
            return false;
        }
        if g.apply(pid, &action).is_err() {
            return false;
        }
        think!(self.journal(), Economy, Decision,
               "gun-resource-buys-the-tile: {} buys {} the {}", name, what, resource;
               "gun-resource-buys-the-tile: {cost:.0} Gold for {pos:?}, the {resource} deposit at {deposit:?}{}; the siege guns draw none and hold under one gun's block",
               if step { ", which the city can buy next turn" } else { "" };
               pos);
        true
    }
}

#[cfg(test)]
mod tests;
