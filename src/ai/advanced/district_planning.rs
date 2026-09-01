//! `district-planning`: the city plans its districts, sites and tile buys
//! together.
//!
//! Opt-in (`Kind::OptIn` in `genes.rs`), ships off, priced by `gene_screen`
//! before any promotion question is asked — see `docs/GENE_SCREEN.md` and
//! `docs/DISTRICT_PLANNING.md`. It lives in its own file because
//! `src/ai/advanced.rs` is the most contended file in the repository.
//!
//! Three shipped answers this plan replaces, and why each is the weak form:
//!
//! - **Which site.** `producible_items` offers each district's top TWO fresh
//!   sites ranked by *unweighted* `Yields::total()` (`src/game.rs`), so a
//!   Campus plot worth +3 Science loses its menu seat to one worth +1
//!   Science +3 Food, and the lane never gets to disagree. The plan prices
//!   every legal site at the lane's own weights and puts its choice on the
//!   menu directly — `can_produce` accepts any site `district_sites` allows,
//!   the shortlist was never the law.
//! - **Which plot belongs to whom.** Districts claim plots independently:
//!   the Commercial Hub that orders first takes the river-mountain hex the
//!   Campus wanted, because nothing reserves ground across districts. The
//!   plan assigns plots **once**, best marginal value first — the same
//!   greedy the settle look-ahead runs for a city that does not exist yet
//!   (`settlement_district_lookahead_from_positions`), run here for the
//!   city that does — and charges every site the worked tile it destroys.
//! - **Which tile to buy.** The best legal owned Campus plot measured ≤ +2
//!   across three seeds while plots at adjacency ≥ 4 are under 1% of the
//!   map (`campus_adjacency_threshold`'s survey): the ground worth planning
//!   for is almost never owned ground. `plot_purchase_cost` quotes rings
//!   1–3 (50/50/75 Gold, ×(1+4·progress)), and a bought plot is immediately
//!   `district_sites`-legal — but no shipped path buys FOR a district. The
//!   plan names the plot its most valuable site sits on and buys it when
//!   the site clears a raw-adjacency bar, beats the best owned alternative
//!   by a margin, and pays the Gold floor every other purchase pays.
//!
//! Early cities need no special case: a young city owns little beyond rings
//! 1–2, so its legal sites are rings 1–2, and ring-3 ground enters the plan
//! only through a purchase that actually clears.
//!
//! Off, every touched path is unchanged.

use super::{AdvancedAi, EmpireCounts, StrategicPlan};
use crate::game::{Game, Item};
use crate::name::Name;
use crate::Pos;
use std::collections::BTreeMap;

/// A purchased site must carry at least this much raw adjacency — Gold buys
/// ground for very valuable districts only, and plots at or above this bar
/// are under a few percent of any map.
pub(super) const PLAN_BUY_MIN_ADJACENCY: f64 = 3.0;
/// …and beat the best owned site for the same district by at least this
/// much raw adjacency, so a near-equal owned plot is used instead of Gold.
pub(super) const PLAN_BUY_MIN_EDGE: f64 = 2.0;
/// `district-planning-2` lowers both bars — adjacency 2 with a real edge of
/// 1 over the owned alternative is still ground worth Gold whenever the
/// score floor below clears; the floor, not the bar, arbitrates the price.
pub(super) const PLAN_BUY_MIN_ADJACENCY_2: f64 = 2.0;
/// The version-2 twin of [`PLAN_BUY_MIN_EDGE`].
pub(super) const PLAN_BUY_MIN_EDGE_2: f64 = 1.0;
/// The floor every Gold purchase in `advanced_gold_spending` clears.
pub(super) const PLAN_BUY_SCORE_FLOOR: f64 = 120.0;
/// The share of the district's production value a plot purchase may claim —
/// the discount `gold_plot_score` applies to a site a plot exposes.
pub(super) const PLAN_BUY_SITE_SHARE: f64 = 0.35;
/// Score charged per Gold of plot price, matching the shipped plot scorer.
pub(super) const PLAN_BUY_COST_CHARGE: f64 = 0.70;
/// How strongly an unowned plot's price counts against it while plots are
/// being assigned (lane yield-points per Gold): enough that free ground
/// wins a near-tie, small enough that a real adjacency edge survives.
pub(super) const PLAN_PURCHASE_WEIGHT: f64 = 0.04;

/// One planned district for one city.
#[derive(Clone, Debug)]
pub(super) struct PlannedDistrict {
    /// The civ's own variant (`seowon` for Korea's `campus`).
    pub district: Name,
    /// The base family the wishlist asked for.
    pub family: Name,
    /// The plot the plan reserved for it.
    pub pos: Pos,
    /// Weight × (lane-priced yields − the worked tile the site destroys),
    /// net of the amortized price where the plot must be bought.
    pub value: f64,
    /// `Some(cost)` when `pos` is unowned and the engine quotes a price.
    pub purchase: Option<f64>,
    /// The best owned site left to this district after every reservation,
    /// so an unbought head still leaves the menu a real candidate.
    pub owned_fallback: Option<Pos>,
}

/// Per-city plans memoized across one `advanced_gold_spending` pass, the
/// way `PlotPurchaseCache` memoizes the purchase facts.
#[derive(Default)]
pub(super) struct DistrictPlanCache {
    cities: BTreeMap<u32, Vec<PlannedDistrict>>,
}

impl AdvancedAi {
    /// The families this city still wants, at the lane's weights: the
    /// wishlist (`new_city_district_wishlist`, the same table the settler
    /// scores a site by) minus everything built, queued or already founded.
    fn city_missing_families(g: &Game, plan: &StrategicPlan, cid: u32) -> Vec<(Name, f64)> {
        let city = &g.cities[&cid];
        Self::new_city_district_wishlist(plan.strategy)
            .into_iter()
            .filter(|(family, _)| {
                !city
                    .districts
                    .keys()
                    .any(|built| g.district_family(*built) == *family)
                    && !city.queue.iter().any(|item| {
                        matches!(item, Item::District { district, .. }
                            if g.district_family(*district) == *family)
                    })
                    && !city.owned_tiles.iter().any(|pos| {
                        g.map.tiles[pos]
                            .district_foundation
                            .as_ref()
                            .is_some_and(|f| g.district_family(f.district) == *family)
                    })
            })
            .collect()
    }

    /// The plan: every wanted family × every sited plot — the city's owned
    /// legal sites plus the purchasable ground of rings 1–3 — scored at the
    /// lane's weights net of the worked tile a site destroys, then one
    /// greedy joint assignment: each plot given away at most once, each
    /// family placed at most once, best first.
    pub(super) fn city_district_plan(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        cid: u32,
    ) -> Vec<PlannedDistrict> {
        let city = &g.cities[&cid];
        let families = Self::city_missing_families(g, plan, cid);
        if families.is_empty() {
            return Vec::new();
        }
        let citizens = g.city_citizen_plan(cid);
        // What a displaced citizen falls back to: the best owned, unworked,
        // workable tile. A site on worked ground costs the difference.
        let best_idle = city
            .owned_tiles
            .iter()
            .copied()
            .filter(|pos| {
                *pos != city.pos
                    && !citizens.worked_tiles.contains(pos)
                    && g.map.get(*pos).is_some_and(|tile| {
                        tile.district.is_none()
                            && tile.district_foundation.is_none()
                            && tile.wonder.is_none()
                            && g.rules.is_passable(tile)
                    })
            })
            .map(|pos| self.yield_value(g.modeled_tile_yields(pos), plan.strategy))
            .max_by(f64::total_cmp);

        // Every candidate, scored. (family index, plot, score, quoted price)
        let mut candidates: Vec<(usize, Pos, f64, Option<f64>)> = Vec::new();
        let mut variants: Vec<Name> = Vec::with_capacity(families.len());
        for (index, (family, weight)) in families.iter().enumerate() {
            let variant = g.civ_district_variant(pid, family.as_str());
            variants.push(variant);
            let mut plots: Vec<(Pos, Option<f64>)> = g
                .district_sites(cid, variant)
                .into_iter()
                .filter(|pos| g.map.tiles[pos].district_foundation.is_none())
                .map(|pos| (pos, None))
                .collect();
            // Purchasable ground, only where owning one more plot could
            // open a site at all (the city-level caps `district_sites`
            // itself applies).
            if g.city_accepts_new_district_site(city, variant) {
                for pos in g.wdisk(city.pos, 3) {
                    if let Some(cost) = g.plot_purchase_cost(pid, cid, pos) {
                        if g.plot_fits_placement(pid, variant, pos, city.pos) {
                            plots.push((pos, Some(cost)));
                        }
                    }
                }
            }
            for (pos, price) in plots {
                let lane = self.yield_value(g.district_yields(variant, pos), plan.strategy);
                let lost = if citizens.worked_tiles.contains(&pos) {
                    let tile = self.yield_value(g.modeled_tile_yields(pos), plan.strategy);
                    best_idle.map_or(tile, |idle| (tile - idle).max(0.0))
                } else {
                    0.0
                };
                let score = weight * (lane - lost) - price.unwrap_or(0.0) * PLAN_PURCHASE_WEIGHT;
                candidates.push((index, pos, score, price));
            }
        }

        // The assignment: best first, deterministic ties (family order,
        // then plot order), one plot per family, one family per plot.
        candidates.sort_by(|a, b| {
            b.2.total_cmp(&a.2)
                .then_with(|| a.0.cmp(&b.0))
                .then_with(|| a.1.cmp(&b.1))
        });
        let mut placed = vec![false; families.len()];
        let mut reserved: Vec<Pos> = Vec::with_capacity(families.len());
        let mut plan_rows: Vec<PlannedDistrict> = Vec::new();
        for (index, pos, score, price) in &candidates {
            if placed[*index] || reserved.contains(pos) {
                continue;
            }
            placed[*index] = true;
            reserved.push(*pos);
            plan_rows.push(PlannedDistrict {
                district: variants[*index],
                family: families[*index].0,
                pos: *pos,
                value: *score,
                purchase: *price,
                owned_fallback: None,
            });
        }
        // A head on unowned ground still leaves the menu a real candidate:
        // the family's best owned plot no reservation holds.
        for row in &mut plan_rows {
            if row.purchase.is_none() {
                continue;
            }
            let index = families
                .iter()
                .position(|(family, _)| *family == row.family)
                .expect("a plan row names a wishlist family");
            row.owned_fallback = candidates
                .iter()
                .filter(|(fi, pos, _, price)| {
                    *fi == index && price.is_none() && !reserved.contains(pos)
                })
                .map(|(_, pos, _, _)| *pos)
                .next();
            if let Some(pos) = row.owned_fallback {
                reserved.push(pos);
            }
        }
        // Assignment order is score order already; sorting on the stored
        // value makes head-is-best a contract rather than a coincidence of
        // the loop above, for every consumer that reads only the head.
        plan_rows.sort_by(|a, b| b.value.total_cmp(&a.value).then_with(|| a.pos.cmp(&b.pos)));
        plan_rows
    }

    /// Menu shaping for `advanced_production`: the plan's sites join the
    /// menu, and a plot the plan reserved for one family is withdrawn from
    /// a rival district that still has a candidate of its own. The argmax
    /// and `production_value` are untouched — the plan only changes what
    /// they get to see.
    pub(super) fn district_plan_shape_menu(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        cid: u32,
        items: &mut Vec<Item>,
    ) {
        let planned = self.city_district_plan(g, pid, plan, cid);
        if planned.is_empty() {
            return;
        }
        let mut reserved: BTreeMap<Pos, Name> = BTreeMap::new();
        for row in &planned {
            reserved.insert(row.pos, row.family);
            if let Some(pos) = row.owned_fallback {
                reserved.insert(pos, row.family);
            }
            let site = if row.purchase.is_none() {
                Some(row.pos)
            } else {
                row.owned_fallback
            };
            if let Some(pos) = site {
                let item = Item::District {
                    district: row.district,
                    pos,
                };
                if !items.contains(&item) {
                    items.push(item);
                }
            }
        }
        let mut remaining: BTreeMap<Name, usize> = BTreeMap::new();
        for item in items.iter() {
            if let Item::District { district, .. } = item {
                *remaining.entry(*district).or_insert(0) += 1;
            }
        }
        items.retain(|item| {
            let Item::District { district, pos } = item else {
                return true;
            };
            // Committed ground (a laid foundation) is not re-litigated.
            if g.map.tiles[pos].district_foundation.is_some() {
                return true;
            }
            let held_for = reserved.get(pos);
            let squatting = held_for.is_some_and(|family| *family != g.district_family(*district));
            if squatting && remaining[district] > 1 {
                *remaining.get_mut(district).expect("counted above") -= 1;
                return false;
            }
            true
        });
    }

    /// The purchase: `Some(score)` when `pos` is the plot the plan wants
    /// bought for `city` and the buy clears the raw-adjacency bar, the edge
    /// over the best owned alternative, and the Gold floor every purchase
    /// clears. Priced as the share of the district's own production value
    /// the plot unlocks over its best owned site, less the Gold — the same
    /// scale `gold_purchase_score` floors at 120, so the buy competes as a
    /// strategic purchase, not a surplus one.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn district_plan_plot_score(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        counts: &EmpireCounts,
        city: u32,
        pos: Pos,
        cost: f64,
        cache: &mut DistrictPlanCache,
    ) -> Option<f64> {
        let planned = cache
            .cities
            .entry(city)
            .or_insert_with(|| self.city_district_plan(g, pid, plan, city));
        let row = planned
            .iter()
            .find(|row| row.pos == pos && row.purchase.is_some())?;
        let variant = row.district;
        let (min_adjacency, min_edge) = if self.district_planning_2 {
            (PLAN_BUY_MIN_ADJACENCY_2, PLAN_BUY_MIN_EDGE_2)
        } else {
            (PLAN_BUY_MIN_ADJACENCY, PLAN_BUY_MIN_EDGE)
        };
        let here = g
            .district_adjacency_assuming(variant, pos, None, None)
            .total();
        if here + f64::EPSILON < min_adjacency {
            return None;
        }
        let best_owned: Option<(Pos, f64)> = g
            .district_sites(city, variant)
            .into_iter()
            .filter(|site| g.map.tiles[site].district_foundation.is_none())
            .map(|site| {
                (
                    site,
                    g.district_adjacency_assuming(variant, site, None, None)
                        .total(),
                )
            })
            .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.cmp(&a.0)));
        if let Some((_, owned)) = best_owned {
            if here + f64::EPSILON < owned + min_edge {
                return None;
            }
        }
        let item = Item::District {
            district: variant,
            pos,
        };
        let unlocked = self.production_value(g, pid, city, &item, plan, counts);
        let already = best_owned
            .map(|(site, _)| {
                let owned_item = Item::District {
                    district: variant,
                    pos: site,
                };
                self.production_value(g, pid, city, &owned_item, plan, counts)
            })
            .unwrap_or(0.0)
            .max(0.0);
        // `production_value` is turns-normalized; un-normalize the marginal
        // the way `gold_purchase_score` prices what Gold buys, so the plot
        // competes on the same scale as every other strategic purchase.
        let production = g.city_yields(city).production.max(1.0);
        let turns = g.item_remaining_cost_for_city(pid, city, &item) / production;
        let positional = (unlocked - already) * (7.0 + turns.max(1.0));
        let score = PLAN_BUY_SITE_SHARE * positional - PLAN_BUY_COST_CHARGE * cost;
        (score >= PLAN_BUY_SCORE_FLOOR).then_some(score)
    }
}
