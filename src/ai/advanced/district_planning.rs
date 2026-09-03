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

use super::{AdvancedAi, EmpireCounts, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::adjacency::PlanAssumption;
use crate::game::{Action, Game, Item};
use crate::name::Name;
use crate::rules::Yields;
use crate::world::DistrictFoundation;
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
/// A tile earning this much Science on its own is a durable science asset,
/// rather than ordinary surplus ground. Five is the yield of one tile beside
/// the Bermuda Triangle; ordinary one- to three-Science ground still follows
/// the normal reserve and fallback rules.
pub(super) const EXCEPTIONAL_SCIENCE_TILE_MINIMUM: f64 = 5.0;
/// How strongly an unowned plot's price counts against it while plots are
/// being assigned (lane yield-points per Gold): enough that free ground
/// wins a near-tie, small enough that a real adjacency edge survives.
pub(super) const PLAN_PURCHASE_WEIGHT: f64 = 0.04;

/// A standing Industrial Zone is not enough: its productive adjacency is
/// often deliberately created by an Aqueduct, Dam, or Canal.  This is the
/// lane-priced marginal yield at a Zone site after a support district is
/// reserved. It keeps a zero-yield Aqueduct from being treated as a filler
/// when it is actually the first half of a +2/+4 cluster.
const PLAN_INDUSTRIAL_SUPPORT_MULTIPLIER: f64 = 1.0;
/// The standing-city planner must carry an Industrial Zone even on lanes
/// whose settlement wishlist is dominated by another district.  Production
/// is the common input to every lane; this small foundation value breaks a
/// tie against an otherwise equivalent generic district without displacing a
/// strong Campus site.
const PLAN_INDUSTRIAL_FOUNDATION_VALUE: f64 = 0.75;

/// Final (turn-normalized) score floors applied after the ordinary production
/// evaluator.  They order a safe, idle city through its coordinated plan:
/// Campus first, the Industrial Zone's legal support before the Zone, then
/// the remaining core.  Emergency production remains much larger than these
/// floors and is never displaced by this helper.
const PLAN_CAMPUS_SCORE_FLOOR: f64 = 90.0;
const PLAN_INDUSTRIAL_SUPPORT_SCORE_FLOOR: f64 = 84.0;
const PLAN_INDUSTRIAL_ZONE_SCORE_FLOOR: f64 = 78.0;
const PLAN_CORE_SCORE_FLOOR: f64 = 66.0;
/// A low-priority specialty district must not consume the last slot that the
/// coordinated core has already reserved.  This is a refusal sentinel in the
/// same scale as the production picker, not merely a small preference.
const PLAN_RESERVED_SPECIALTY_VETO: f64 = -10_000.0;
/// A serious Amenity collapse is a real exception to a development layout.
/// A city one or two Amenities short keeps its Campus/Industrial slots; a
/// city at this deficit may still take an Entertainment Complex to recover.
const PLAN_SEVERE_AMENITY_DEFICIT: i64 = -3;

/// Core districts score their direct lane output. Industrial support scores
/// the Zone adjacency it creates; the Hansa's Commercial Hub is a partner of
/// the latter kind even though it still consumes a specialty slot. Capacity
/// always comes from the district rule rather than this planning label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlannedDistrictRole {
    Core,
    IndustrialSupport,
    IndustrialPartner,
}

#[derive(Clone, Copy, Debug)]
struct PlannedFamily {
    family: Name,
    weight: f64,
    role: PlannedDistrictRole,
}

/// One planned district for one city.
#[derive(Clone, Debug)]
pub(super) struct PlannedDistrict {
    /// The civ's own variant (`seowon` for Korea's `campus`).
    pub district: Name,
    /// The base family the wishlist asked for.
    pub family: Name,
    /// The plot the plan reserved for it.
    pub pos: Pos,
    /// `true` for an Aqueduct, Dam, Canal, or Hansa Commercial Hub whose
    /// purpose is to lift the linked Industrial Zone's eventual adjacency.
    /// The capacity check still reads the district's own specialty flag.
    pub support: bool,
    /// The coordinated construction order. An Aqueduct can have lower direct
    /// yield than the Zone it enables, yet has to stand first for the Zone to
    /// receive its adjacency.
    pub order: usize,
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
    /// settlement wishlist minus everything built, queued or already
    /// founded.  A city plan additionally carries an Industrial Zone as a
    /// production foundation on every lane, then adds only the districts that
    /// the civilization's actual Zone can turn into adjacency.
    ///
    /// That last distinction matters.  An Aqueduct does not appear on a
    /// normal lane wishlist because it has no raw yield, but it is not a
    /// generic filler when it makes a neighboring Industrial Zone +2
    /// Production.  Conversely, an unrelated Aqueduct stays out of the plan.
    fn city_missing_families(
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        cid: u32,
    ) -> Vec<PlannedFamily> {
        let city = &g.cities[&cid];
        let missing = |family: Name| {
            !city
                .districts
                .keys()
                .any(|built| g.district_family(*built) == family)
                && !city.queue.iter().any(|item| {
                    matches!(item, Item::District { district, .. }
                        if g.district_family(*district) == family)
                })
                && !city.owned_tiles.iter().any(|pos| {
                    g.map.tiles[pos]
                        .district_foundation
                        .as_ref()
                        .is_some_and(|foundation| g.district_family(foundation.district) == family)
                })
        };
        let mut families: Vec<PlannedFamily> = Self::new_city_district_wishlist(plan.strategy)
            .into_iter()
            .filter(|(family, _)| missing(*family))
            .map(|(family, weight)| PlannedFamily {
                family,
                weight,
                role: PlannedDistrictRole::Core,
            })
            .collect();

        let industrial_zone = crate::name!("industrial_zone");
        // The lane table has an Industrial Zone in only some rows.  The
        // standing city must develop a production base on every route, so
        // insert a modest core request when the table did not.
        if missing(industrial_zone)
            && !families
                .iter()
                .any(|planned| planned.family == industrial_zone)
        {
            families.push(PlannedFamily {
                family: industrial_zone,
                weight: 0.65,
                role: PlannedDistrictRole::Core,
            });
        }

        let industrial_variant = g.civ_district_variant(pid, "industrial_zone");
        let industrial_present = !missing(industrial_zone);
        let industrial_planned = families
            .iter()
            .any(|planned| planned.family == industrial_zone);
        if !industrial_present && !industrial_planned {
            return families;
        }

        let industrial_spec = &g.rules.districts[industrial_variant];
        // Germany's Hansa pays +2 for its Commercial Hub. The Hub becomes an
        // adjacency partner even if the normal lane had already requested it:
        // it must be able to stand before the Hansa, not trail it as a plain
        // low-weight Gold district.
        if industrial_spec.adjacency.contains_key("commercial_hub")
            && missing(crate::name!("commercial_hub"))
        {
            if let Some(planned) = families
                .iter_mut()
                .find(|planned| planned.family == crate::name!("commercial_hub"))
            {
                planned.weight = planned.weight.max(0.60);
                planned.role = PlannedDistrictRole::IndustrialPartner;
            } else {
                families.push(PlannedFamily {
                    family: crate::name!("commercial_hub"),
                    weight: 0.60,
                    role: PlannedDistrictRole::IndustrialPartner,
                });
            }
        }

        for support in ["aqueduct", "dam", "canal"] {
            let family = Name::new(support);
            if industrial_spec.adjacency.contains_key(family.as_str()) && missing(family) {
                families.push(PlannedFamily {
                    family,
                    weight: 1.0,
                    role: PlannedDistrictRole::IndustrialSupport,
                });
            }
        }
        families
    }

    /// District yields with every foundation in the temporary city plan
    /// treated as its completed family.  Live yield calculations intentionally
    /// ignore unfinished districts; this is a planning-only counterfactual so
    /// the Aqueduct placed first can pay the Industrial Zone it unlocks.
    fn projected_district_yields(g: &Game, district: Name, pos: Pos) -> Yields {
        let mut yields = g.rules.districts[district].yields;
        yields.add(g.district_adjacency_assuming(
            district,
            pos,
            Some(&PlanAssumption {
                city_center: None,
                foundations: true,
            }),
            None,
        ));
        yields
    }

    /// Reserve a foundation on the planner's private clone.  This follows the
    /// production placement path closely enough for every later adjacency
    /// calculation: a district displaces its improvement and, except for
    /// Vietnam's specialty districts, clears its removable feature.
    fn reserve_plan_foundation(g: &mut Game, pid: usize, cid: u32, pos: Pos, district: Name) {
        let preserve_feature =
            g.players[pid].civ == "Vietnam" && g.rules.districts[district].specialty;
        // A purchased candidate is still unowned on the live board.  The
        // private board needs to treat it as owned, both so the city's
        // specialty-capacity accounting sees its foundation and so a later
        // district can receive adjacency from it.  This clone never escapes
        // the planner; no live ownership is changed here.
        let city = &mut g.cities.get_mut(&cid).expect("a planned city exists");
        if !city.owned_tiles.contains(&pos) {
            city.owned_tiles.push(pos);
        }
        let tile = g
            .map
            .tiles
            .get_mut(&pos)
            .expect("a planned district site is on the map");
        tile.owner_city = Some(cid);
        tile.district_foundation = Some(DistrictFoundation {
            district,
            cost: 0.0,
        });
        tile.improvement = None;
        tile.pillaged = false;
        if !preserve_feature {
            tile.feature = None;
        }
    }

    /// Every Industrial Zone position that can still benefit from a support
    /// district or Hansa Commercial Hub partner: a standing/started Zone plus
    /// the fresh legal candidates.
    fn industrial_zone_positions(g: &Game, pid: usize, cid: u32) -> Vec<Pos> {
        let city = &g.cities[&cid];
        let industrial_zone = crate::name!("industrial_zone");
        let variant = g.civ_district_variant(pid, "industrial_zone");
        let mut positions: Vec<Pos> = city
            .districts
            .iter()
            .filter_map(|(district, pos)| {
                (g.district_family(*district) == industrial_zone).then_some(*pos)
            })
            .chain(city.owned_tiles.iter().copied().filter(|pos| {
                g.map.tiles[pos]
                    .district_foundation
                    .as_ref()
                    .is_some_and(|foundation| {
                        g.district_family(foundation.district) == industrial_zone
                    })
            }))
            .collect();
        if g.city_accepts_new_district_site(city, variant) {
            positions.extend(
                g.district_sites(cid, variant)
                    .into_iter()
                    .filter(|pos| g.map.tiles[pos].district_foundation.is_none()),
            );
            // A purchased Industrial Zone is a real future destination for
            // an owned support or Hansa partner. Include only plots the engine
            // would sell and whose physical placement rule accepts, so a
            // support never chases fog or somebody else's land.
            positions.extend(g.wdisk(city.pos, 3).into_iter().filter(|pos| {
                g.plot_purchase_cost(pid, cid, *pos).is_some()
                    && g.plot_fits_placement(pid, variant, *pos, city.pos)
            }));
        }
        positions.sort();
        positions.dedup();
        positions
    }

    /// What this support site adds to the *same* Industrial Zone position
    /// before and after it is reserved.  Comparing an individual position is
    /// intentional: a city's already-best Zone can otherwise hide a real +2
    /// Dam beside its second-best site, even though that Dam and the Zone form
    /// the cluster the plan needs to construct. A support with no positive
    /// marginal effect is not admitted at all, which prevents random
    /// zero-yield Aqueducts and Dams from becoming fake development goals.
    fn industrial_support_value(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        support: Name,
        pos: Pos,
    ) -> f64 {
        let variant = g.civ_district_variant(pid, "industrial_zone");
        let positions = Self::industrial_zone_positions(g, pid, cid);
        let mut after = g.speculative_clone();
        Self::reserve_plan_foundation(&mut after, pid, cid, pos, support);
        positions
            .into_iter()
            // The support itself may also have been an otherwise legal IZ
            // tile. Once it becomes an Aqueduct/Dam/Canal/Commercial Hub it
            // is no longer a candidate for the Zone it supports.
            .filter(|zone| *zone != pos)
            .map(|zone| {
                let before = self.yield_value(
                    Self::projected_district_yields(g, variant, zone),
                    plan.strategy,
                );
                let after = self.yield_value(
                    Self::projected_district_yields(&after, variant, zone),
                    plan.strategy,
                );
                (after - before).max(0.0)
            })
            .max_by(f64::total_cmp)
            .unwrap_or(0.0)
            * PLAN_INDUSTRIAL_SUPPORT_MULTIPLIER
    }

    /// The plan is a small, private construction board.  It places one
    /// district at a time, marks that foundation on a speculative clone, and
    /// scores every next choice with the foundations already reserved.  That
    /// makes build order part of the answer: an Aqueduct or Dam can win before
    /// its Industrial Zone because the latter's projected adjacency sees it.
    ///
    /// A Science lane claims its Campus before the general greedy pass.  The
    /// remaining core districts and the Industrial support pieces then compete
    /// on their actual marginal city value, with one plot and one family used
    /// at most once.
    pub(super) fn city_district_plan(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        cid: u32,
    ) -> Vec<PlannedDistrict> {
        let city = &g.cities[&cid];
        let mut families = Self::city_missing_families(g, pid, plan, cid);
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

        let science_campus_first = (plan.strategy == GrandStrategy::Science
            || self.active_victory_target(g) == Some(VictoryTarget::Science))
            && families
                .iter()
                .any(|planned| planned.family == crate::name!("campus"));
        let campus = crate::name!("campus");
        let industrial_zone = crate::name!("industrial_zone");
        let mut planned_game = g.speculative_clone();
        let mut plan_rows: Vec<PlannedDistrict> = Vec::with_capacity(families.len());

        while !families.is_empty() {
            // Until the Campus has a site, no smaller science-lane request
            // may take either its premium ground or its specialty slot.
            let campus_due =
                science_campus_first && families.iter().any(|planned| planned.family == campus);
            // (family index, variant, position, score, quoted price)
            let mut best: Option<(usize, Name, Pos, f64, Option<f64>)> = None;
            for (index, requested) in families.iter().enumerate() {
                if campus_due && requested.family != campus {
                    continue;
                }
                let variant = planned_game.civ_district_variant(pid, requested.family.as_str());
                let planning_city = &planned_game.cities[&cid];
                if !planned_game.city_accepts_new_district_site(planning_city, variant) {
                    continue;
                }
                let mut plots: Vec<(Pos, Option<f64>)> = planned_game
                    .district_sites(cid, variant)
                    .into_iter()
                    .filter(|pos| planned_game.map.tiles[pos].district_foundation.is_none())
                    .map(|pos| (pos, None))
                    .collect();
                // Core districts and a Hansa Commercial Hub partner can name
                // an unowned site that a city will buy. Aqueduct/Dam/Canal
                // support stays owned-only: its value is the IZ cluster it
                // unlocks, not an excuse to buy filler ground.
                if requested.role != PlannedDistrictRole::IndustrialSupport {
                    for pos in planned_game.wdisk(planning_city.pos, 3) {
                        if let Some(cost) = planned_game.plot_purchase_cost(pid, cid, pos) {
                            if planned_game.plot_fits_placement(
                                pid,
                                variant,
                                pos,
                                planning_city.pos,
                            ) {
                                plots.push((pos, Some(cost)));
                            }
                        }
                    }
                }
                for (pos, price) in plots {
                    let lane = self.yield_value(
                        Self::projected_district_yields(&planned_game, variant, pos),
                        plan.strategy,
                    );
                    let lost = if citizens.worked_tiles.contains(&pos) {
                        let tile = self.yield_value(g.modeled_tile_yields(pos), plan.strategy);
                        best_idle.map_or(tile, |idle| (tile - idle).max(0.0))
                    } else {
                        0.0
                    };
                    let supports_industrial = matches!(
                        requested.role,
                        PlannedDistrictRole::IndustrialSupport
                            | PlannedDistrictRole::IndustrialPartner
                    );
                    let support_value = if supports_industrial {
                        self.industrial_support_value(
                            &planned_game,
                            pid,
                            cid,
                            plan,
                            requested.family,
                            pos,
                        )
                    } else {
                        0.0
                    };
                    // A support district must create real Industrial value;
                    // without that proof it is not a plan candidate at all.
                    if supports_industrial && support_value <= f64::EPSILON {
                        continue;
                    }
                    let industrial_foundation = if requested.family == industrial_zone {
                        PLAN_INDUSTRIAL_FOUNDATION_VALUE
                    } else {
                        0.0
                    };
                    let score =
                        requested.weight * (lane - lost) + support_value + industrial_foundation
                            - price.unwrap_or(0.0) * PLAN_PURCHASE_WEIGHT;
                    let replace =
                        best.as_ref()
                            .is_none_or(|(old_index, _, old_pos, old_score, _)| {
                                score > *old_score + f64::EPSILON
                                    || ((score - *old_score).abs() <= f64::EPSILON
                                        && (index, pos) < (*old_index, *old_pos))
                            });
                    if replace {
                        best = Some((index, variant, pos, score, price));
                    }
                }
            }
            let Some((index, district, pos, _value, purchase)) = best else {
                // An illegal Campus does not strand the city's whole plan;
                // discard just that currently impossible request and let the
                // next legal core proceed deterministically.
                let unavailable = if campus_due {
                    families
                        .iter()
                        .position(|planned| planned.family == campus)
                        .expect("the due Campus is still requested")
                } else {
                    0
                };
                families.remove(unavailable);
                continue;
            };
            let requested = families.remove(index);
            let order = plan_rows.len();
            plan_rows.push(PlannedDistrict {
                district,
                family: requested.family,
                pos,
                support: matches!(
                    requested.role,
                    PlannedDistrictRole::IndustrialSupport | PlannedDistrictRole::IndustrialPartner
                ),
                order,
                purchase,
                owned_fallback: None,
            });
            Self::reserve_plan_foundation(&mut planned_game, pid, cid, pos, district);
        }

        // A head on unowned ground still leaves the menu a real candidate:
        // the family's best owned plot no reservation holds.
        let mut reserved: Vec<Pos> = plan_rows.iter().map(|row| row.pos).collect();
        for row in &mut plan_rows {
            if row.purchase.is_none() {
                continue;
            }
            row.owned_fallback = g
                .district_sites(cid, row.district)
                .into_iter()
                .filter(|pos| {
                    g.map.tiles[pos].district_foundation.is_none() && !reserved.contains(pos)
                })
                .map(|pos| {
                    let lane = self.yield_value(
                        Self::projected_district_yields(g, row.district, pos),
                        plan.strategy,
                    );
                    let lost = if citizens.worked_tiles.contains(&pos) {
                        let tile = self.yield_value(g.modeled_tile_yields(pos), plan.strategy);
                        best_idle.map_or(tile, |idle| (tile - idle).max(0.0))
                    } else {
                        0.0
                    };
                    (pos, lane - lost)
                })
                .max_by(|left, right| {
                    left.1
                        .total_cmp(&right.1)
                        .then_with(|| right.0.cmp(&left.0))
                })
                .map(|(pos, _)| pos);
            if let Some(pos) = row.owned_fallback {
                reserved.push(pos);
            }
        }
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
    ) -> Vec<PlannedDistrict> {
        let planned = self.city_district_plan(g, pid, plan, cid);
        if planned.is_empty() {
            return planned;
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
        planned
    }

    /// Let a coordinated plan influence the ordinary production ranking once
    /// the menu has been scored.  Site shaping alone stops a rival district
    /// from stealing a reserved hex, but it still leaves a cheap Entertainment
    /// Complex free to consume the last specialty slot ahead of the Campus or
    /// Industrial Zone.  These modest floors make an idle, safe city start the
    /// plan in construction order; its normal emergency, military, wonder and
    /// project scores remain authoritative whenever they are higher.
    pub(super) fn district_plan_adjust_menu_scores(
        &self,
        g: &Game,
        plan: &StrategicPlan,
        cid: u32,
        planned: &[PlannedDistrict],
        items: &[Item],
        scores: &mut [f64],
    ) {
        debug_assert_eq!(items.len(), scores.len());
        // A city under immediate pressure must retain every defensive option;
        // layout development resumes on the next peaceful production pass.
        if plan.threatened_city == Some(cid) {
            return;
        }
        if planned.is_empty() {
            return;
        }
        let city = &g.cities[&cid];
        let specialty_capacity = 1 + (city.pop.max(1) - 1) as usize / 3;
        let used_specialty = city
            .districts
            .keys()
            .filter(|district| g.rules.districts[district].specialty)
            .count()
            + city
                .owned_tiles
                .iter()
                .filter_map(|pos| g.map.tiles[pos].district_foundation.as_ref())
                .filter(|foundation| g.rules.districts[foundation.district].specialty)
                .count();
        let planned_specialty = planned
            .iter()
            .filter(|row| {
                g.rules.districts[row.district].specialty
                    && (row.purchase.is_none() || row.owned_fallback.is_some())
            })
            .count();
        let reserve_specialty_slots =
            planned_specialty >= specialty_capacity.saturating_sub(used_specialty);

        for (item, score) in items.iter().zip(scores.iter_mut()) {
            let Item::District { district, pos } = item else {
                continue;
            };
            let family = g.district_family(*district);
            let row = planned.iter().find(|row| {
                row.district == *district
                    && match row.purchase {
                        None => row.pos == *pos,
                        Some(_) => row.owned_fallback == Some(*pos),
                    }
            });
            if let Some(row) = row {
                let floor = if family == crate::name!("campus") {
                    PLAN_CAMPUS_SCORE_FLOOR
                } else if row.support {
                    PLAN_INDUSTRIAL_SUPPORT_SCORE_FLOOR
                } else if family == crate::name!("industrial_zone") {
                    PLAN_INDUSTRIAL_ZONE_SCORE_FLOOR
                } else {
                    PLAN_CORE_SCORE_FLOOR
                };
                // The family floor protects the type of infrastructure; this
                // tiny offset protects the sequence among equal types. It is
                // deliberately much smaller than the gap between Campus,
                // support, Zone, and other core floors.
                let order_offset = (row.order as f64 * 0.25).min(3.0);
                *score = score.max(floor - order_offset);
                continue;
            }
            // Encampments, Aerodromes, and Spaceports have tactical or
            // end-game duties outside this economic layout, so this narrow
            // specialty reservation deliberately leaves them available.
            let tactical_family =
                matches!(family.as_str(), "encampment" | "aerodrome" | "spaceport");
            if reserve_specialty_slots
                && g.rules.districts[*district].specialty
                && !tactical_family
                && g.city_amenity_surplus(city) > PLAN_SEVERE_AMENITY_DEFICIT
            {
                *score = PLAN_RESERVED_SPECIALTY_VETO;
            }
        }
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
        let row_index = planned
            .iter()
            .position(|row| row.pos == pos && row.purchase.is_some())?;
        // Version 3 only funds the first district the coordinated plan would
        // ask an idle city to start now. V2 could buy a lower-priority site
        // while the city was committed elsewhere, then also spend through the
        // reserve on its independent high-Science asset path. A purchased
        // plot is useful only once its district can enter the next production
        // decision, and this preserves the full ordinary reserve until then.
        if self.district_planning_3 && (row_index != 0 || !g.cities[&city].queue.is_empty()) {
            return None;
        }
        let row = &planned[row_index];
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

    /// Price a very high-Science workable tile — or the one cheap border hex
    /// that opens it — as a strategic asset rather than a surplus plot. This
    /// is deliberately the version-two route: the original district-planning
    /// behaviour remains byte-for-byte unchanged.
    ///
    /// The generic working reserve protects prospective defenders and future
    /// deficit recovery across the whole empire. A five-plus-Science plot is
    /// worth letting a Science seat acquire now, but it may never consume an
    /// appointed war package or the Gold for an immediate threatened-city
    /// defender. `workable_tile_yields` is essential here: a live mirror's
    /// observed host yield, including a natural-wonder correction, is what
    /// the citizen will actually collect. Civ VI only sells connected land,
    /// so a promising ring-three tile often needs a low-yield ring-two bridge;
    /// the bridge is priced with its immediately unlocked science tile and the
    /// two quotes together, rather than as the empty ground it happens to be.
    pub(super) fn exceptional_science_plot_score(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        action: &Action,
    ) -> Option<f64> {
        let science_lane = plan.strategy == GrandStrategy::Science
            || (plan.strategy == GrandStrategy::Expansion
                && self.active_victory_target(g) == Some(VictoryTarget::Science));
        if !self.district_planning_2 || !science_lane {
            return None;
        }
        let Action::BuyPlot { city, pos, cost } = action else {
            return None;
        };
        let direct_yields = g.workable_tile_yields(*pos);
        let direct = direct_yields.science + f64::EPSILON >= EXCEPTIONAL_SCIENCE_TILE_MINIMUM;
        // A purchase only opens ground touching itself. This cheap prefilter
        // avoids cloning every ordinary border tile just to discover it does
        // not lead to a high-Science one.
        let bridge_might_open_science = !direct
            && g.nbrs(*pos).into_iter().any(|neighbor| {
                g.map.tiles[&neighbor].owner_city.is_none()
                    && g.workable_tile_yields(neighbor).science + f64::EPSILON
                        >= EXCEPTIONAL_SCIENCE_TILE_MINIMUM
            });
        if !direct && !bridge_might_open_science {
            return None;
        }
        let mut after = g.speculative_clone();
        after.apply(pid, action).ok()?;
        let remaining = after.players[pid].gold;
        let emergency_floor = self
            .war_treasury_floor(g, pid)
            .max(self.threatened_city_gold_floor(g, pid, plan));
        if remaining + f64::EPSILON < emergency_floor {
            crate::think!(self.journal(), Economy, Detail,
                "Keeping Gold instead of buying high-Science tile at {pos:?}";
                "{:.1} Science, {cost:.0} Gold; {remaining:.0} left is below the {:.0} emergency floor",
                direct_yields.science, emergency_floor);
            return None;
        }
        let target = if direct {
            Some((*pos, direct_yields, *cost, remaining))
        } else {
            after
                .nbrs(*pos)
                .into_iter()
                .filter_map(|science_pos| {
                    let science_cost = after.plot_purchase_cost(pid, *city, science_pos)?;
                    let science_yields = after.workable_tile_yields(science_pos);
                    (science_yields.science + f64::EPSILON >= EXCEPTIONAL_SCIENCE_TILE_MINIMUM
                        && remaining + f64::EPSILON >= emergency_floor + science_cost)
                        .then_some((
                            science_pos,
                            science_yields,
                            *cost + science_cost,
                            remaining - science_cost,
                        ))
                })
                .max_by(|left, right| {
                    let left_score = self.yield_value(left.1, GrandStrategy::Science) * 24.0
                        - PLAN_BUY_COST_CHARGE * left.2;
                    let right_score = self.yield_value(right.1, GrandStrategy::Science) * 24.0
                        - PLAN_BUY_COST_CHARGE * right.2;
                    left_score
                        .total_cmp(&right_score)
                        .then_with(|| right.0.cmp(&left.0))
                })
        };
        let (science_pos, science_yields, total_cost, final_gold) = target?;
        let score = self.yield_value(science_yields, GrandStrategy::Science) * 24.0
            - PLAN_BUY_COST_CHARGE * total_cost;
        if score + f64::EPSILON < PLAN_BUY_SCORE_FLOOR {
            crate::think!(self.journal(), Economy, Detail,
                "Leaving high-Science ground at {science_pos:?} unbought";
                "{:.1} Science, {total_cost:.0} Gold through {pos:?}; score {score:.0} is below the {:.0} floor",
                science_yields.science, PLAN_BUY_SCORE_FLOOR);
            return None;
        }
        if science_pos == *pos {
            crate::think!(self.journal(), Economy, Detail,
                "Pricing high-Science tile at {pos:?}";
                "{:.1} Science, {total_cost:.0} Gold, score {score:.0}; {final_gold:.0} left above the {:.0} emergency floor",
                science_yields.science, emergency_floor);
        } else {
            crate::think!(self.journal(), Economy, Detail,
                "Pricing bridge tile at {pos:?} for high-Science ground at {science_pos:?}";
                "{:.1} Science, {total_cost:.0} Gold for both plots, score {score:.0}; {final_gold:.0} left above the {:.0} emergency floor",
                science_yields.science, emergency_floor);
        }
        Some(score)
    }
}
