//! Two territory genes: the district look-ahead a settler scores a site by,
//! and the priced tile purchase a treasury decides with.
//!
//! Both are opt-in (`PRODUCTION_OPT_INS`), ship off, and are priced by
//! `gene_screen` before any promotion question is asked — see
//! `docs/GENE_SCREEN.md`. They live in their own file because they are
//! one sentence each and `src/ai/advanced.rs` is the most contended file in
//! the repository (`tools/conflict_hotspots.py`).
//!
//! **`district-lookahead-settle`.** The shipped settle scorer prices a
//! site's district potential with `settlement_adjacency_summary`: every
//! adjacency-bearing family the ruleset knows, each at its own best plot,
//! summed and capped. That is the potential of a city that builds every
//! district at once, on plots that may all be the same hex. A settler is
//! choosing where a *particular* city will put its *first* one or two
//! districts, and the planner already knows which those are — the lane's
//! `strategic_family` table and the coverage terms in `production_value`
//! name them. With this on, the scorer asks that question instead: the
//! families the plan would build, in order, each assigned its own plot,
//! each priced at the lane's own yield weights. A Science seat's site is
//! its Campus plot; a Holy Site's +4 at the same site is nothing to it.
//!
//! **`priced-tile-purchase`.** `advanced_gold_spending` buys a border plot
//! when nothing else clears the bar, scoring it as a flat sum of yields,
//! resource class and a speculative district site, minus 0.70 of the
//! price, against a constant floor. Nothing in that asks the two questions
//! a purchase turns on: would a citizen actually *work* the plot (a
//! size-three town with eleven tiles works three of them), and would the
//! border take it for free next turn anyway? With this on the plot is
//! priced as an investment: the per-turn value it adds — the job it
//! improves, the resource it connects, the district plot it unlocks — over
//! a payback horizon cut short where culture would claim it, against the
//! Gold at the lane's own price for Gold. The purchase must clear the price
//! by a margin, and by an absolute surplus, before it is made.

use super::{AdvancedAi, GrandStrategy, StrategicPlan};
use crate::game::Game;
use crate::name::Name;
use crate::rules::Yields;
use crate::Pos;
use std::collections::BTreeMap;

/// Multiplier on a wanted district's priced adjacency in the settle score.
/// A +3 Campus on the Science lane (3 × 4.2 beakers) is 18.9 points, about
/// a third of a typical site; the same +3 on the Expansion lane, whose
/// beaker is worth 1.2, is 5.4. Capped by [`DISTRICT_LOOKAHEAD_CAP`] so a
/// mountain bowl cannot outbid a breadbasket outright.
pub(super) const DISTRICT_LOOKAHEAD_SCALE: f64 = 1.5;
/// The same ceiling `settlement_adjacency_value` uses for the summary.
pub(super) const DISTRICT_LOOKAHEAD_CAP: f64 = 24.0;
/// Points lost when the lane's first district has *no* legal plot inside
/// the two-ring disk, per unit of its weight — a site where the plan's
/// opening district cannot be built is a site the plan will resent.
pub(super) const DISTRICT_LOOKAHEAD_UNPLACEABLE: f64 = 6.0;

/// Standard-speed turns a plot purchase is asked to pay for itself in.
/// The price curve quadruples over the game while a worked plot's yield
/// does not, so the horizon is what makes late purchases stop clearing.
pub(super) const TILE_PURCHASE_PAYBACK_HORIZON: u32 = 30;
/// A plot's priced benefit must exceed its Gold at the lane's price by
/// this factor — "fairly positive", not merely positive.
pub(super) const TILE_PURCHASE_MARGIN: f64 = 1.5;
/// …and by at least this many yield-points outright, so a cheap marginal
/// plot does not clear on ratio alone.
pub(super) const TILE_PURCHASE_MIN_SURPLUS: f64 = 40.0;
/// A district the plot would host arrives some turns after the purchase;
/// its adjacency edge is discounted by this before the horizon is applied.
pub(super) const TILE_PURCHASE_DISTRICT_DISCOUNT: f64 = 0.6;
/// A plot the border will take in this many turns or fewer is not bought.
pub(super) const TILE_PURCHASE_FREE_SOON: f64 = 2.0;

/// Per-city facts a plot-purchase pass reuses across every border hex of
/// that city, so a large empire's scan does not re-plan citizens per plot.
#[derive(Default)]
pub(super) struct PlotPurchaseCache {
    cities: BTreeMap<u32, CityPurchaseFacts>,
}

struct CityPurchaseFacts {
    /// The lane's value of the weakest tile a citizen works now, or `None`
    /// when no tile is worked (every citizen a specialist).
    weakest_worked: Option<f64>,
    /// The lane's value of the best owned, unworked, workable tile — what
    /// the next citizen would take without a purchase.
    best_unworked: Option<f64>,
    /// The district this city would most likely build next (the first
    /// wished family it lacks), its civ variant, the plan's weight for it,
    /// and the best adjacency value any owned plot already offers it.
    next_district: Option<(Name, f64, f64)>,
    /// Turns until the border grows on its own, `None` when it never will.
    growth_turns: Option<f64>,
    /// The plots that growth would draw from.
    front: Vec<Pos>,
}

impl AdvancedAi {
    /// The district families a new city on this lane would build, first
    /// first, with the share of full weight each carries in the look-ahead.
    ///
    /// Mirrors `production_value`'s `strategic_family` table and its two
    /// coverage terms (`research_coverage`: a Campus is asked for in every
    /// city whatever the lane; `balanced_core`: Commercial Hub / Harbor /
    /// Industrial Zone for the first half of the empire), reduced to what a
    /// site can be chosen for: only adjacency-bearing families, only the
    /// first two or three. A family the site cannot place at all contributes
    /// nothing except, for the first one, [`DISTRICT_LOOKAHEAD_UNPLACEABLE`].
    pub(super) fn new_city_district_wishlist(strategy: GrandStrategy) -> Vec<(Name, f64)> {
        let rows: &[(&str, f64)] = match strategy {
            GrandStrategy::Science => &[
                ("campus", 1.0),
                ("commercial_hub", 0.5),
                ("industrial_zone", 0.5),
            ],
            GrandStrategy::Culture => &[
                ("theater_square", 1.0),
                ("campus", 0.6),
                ("commercial_hub", 0.4),
            ],
            GrandStrategy::Religion => {
                &[("holy_site", 1.0), ("campus", 0.5), ("commercial_hub", 0.3)]
            }
            GrandStrategy::Diplomacy => {
                &[("commercial_hub", 0.8), ("harbor", 0.6), ("campus", 0.6)]
            }
            // An Encampment has no adjacency to look ahead at and fits almost
            // anywhere; the Conquest lane's site is its Industrial Zone.
            GrandStrategy::Conquest => &[
                ("industrial_zone", 0.8),
                ("campus", 0.6),
                ("commercial_hub", 0.4),
            ],
            GrandStrategy::Recovery => &[("industrial_zone", 1.0), ("campus", 0.4)],
            GrandStrategy::Expansion => {
                &[("campus", 0.8), ("commercial_hub", 0.7), ("harbor", 0.5)]
            }
        };
        rows.iter()
            .map(|(family, weight)| (Name::new(family), *weight))
            .collect()
    }

    /// The lane's opinion of a district's adjacency, never below the settle
    /// scorer's own yield weights: the Expansion lane prices a beaker at
    /// 1.2 already, and no lane may price one below what every other
    /// settlement term pays for it.
    fn lookahead_yield_value(&self, adjacency: Yields, strategy: GrandStrategy) -> f64 {
        self.yield_value(adjacency, strategy).max(
            adjacency.food * 2.0
                + adjacency.production * 2.2
                + adjacency.gold * 0.7
                + adjacency.science * 1.2
                + adjacency.culture * 1.2
                + adjacency.faith * 0.4,
        )
    }

    /// The strategy the look-ahead plans for: the current plan's, or the
    /// opening lane before a plan exists (every seat settles before it has
    /// assessed anything).
    fn lookahead_strategy(&self) -> GrandStrategy {
        self.plan
            .as_ref()
            .map_or(GrandStrategy::Expansion, |plan| plan.strategy)
    }

    /// `district-lookahead-settle`'s term in `settlement_static_value`:
    /// the districts the plan would build at `pos`, each on its own plot,
    /// priced at the lane's weights. See the module header.
    pub(super) fn settlement_district_lookahead_value(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        positions: &[Pos],
    ) -> f64 {
        let strategy = self.lookahead_strategy();
        let wishlist = Self::new_city_district_wishlist(strategy);
        let families: Vec<Name> = wishlist.iter().map(|(family, _)| *family).collect();
        let sites = g.settlement_district_lookahead_from_positions(pid, pos, positions, &families);
        let mut value = 0.0;
        for (index, ((_, weight), site)) in wishlist.iter().zip(sites).enumerate() {
            match site {
                Some((_, adjacency)) => {
                    value += weight * self.lookahead_yield_value(adjacency, strategy);
                }
                None if index == 0 => value -= weight * DISTRICT_LOOKAHEAD_UNPLACEABLE,
                None => {}
            }
        }
        (value * DISTRICT_LOOKAHEAD_SCALE).min(DISTRICT_LOOKAHEAD_CAP)
    }

    /// What a tile pays a citizen of `pid` who works it, at the lane's
    /// weights, seeing only the resources the seat has the technology to
    /// see — the same visibility rule every purchase scorer here applies.
    fn visible_tile_value(&self, g: &Game, pid: usize, pos: Pos, strategy: GrandStrategy) -> f64 {
        let tile = &g.map.tiles[&pos];
        let revealed = tile
            .resource
            .as_ref()
            .and_then(|name| g.rules.resources.get(name))
            .is_some_and(|spec| {
                spec.tech
                    .as_ref()
                    .is_none_or(|tech| g.players[pid].techs.contains(tech))
                    && spec
                        .civic
                        .as_ref()
                        .is_none_or(|civic| g.players[pid].civics.contains(civic))
            });
        let yields = if tile.resource.is_some() && !revealed {
            let mut visible = tile.clone();
            visible.resource = None;
            g.rules.tile_yields(&visible)
        } else {
            g.modeled_tile_yields(pos)
        };
        self.yield_value(yields, strategy)
    }

    fn city_purchase_facts(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        cid: u32,
    ) -> CityPurchaseFacts {
        let city = &g.cities[&cid];
        let citizens = g.city_citizen_plan(cid);
        let weakest_worked = citizens
            .worked_tiles
            .iter()
            .map(|pos| self.visible_tile_value(g, pid, *pos, plan.strategy))
            .min_by(f64::total_cmp);
        let best_unworked = city
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
            .map(|pos| self.visible_tile_value(g, pid, pos, plan.strategy))
            .max_by(f64::total_cmp);
        let next_district = Self::new_city_district_wishlist(plan.strategy)
            .into_iter()
            .find(|(family, _)| {
                !city
                    .districts
                    .keys()
                    .any(|built| g.district_family(*built) == *family)
                    && !city.queue.iter().any(|item| {
                        matches!(item, crate::game::Item::District { district, .. }
                            if g.district_family(*district) == *family)
                    })
            })
            .map(|(family, weight)| {
                let variant = g.civ_district_variant(pid, family.as_str());
                let best_owned = g
                    .district_sites(cid, variant)
                    .into_iter()
                    .map(|site| self.yield_value(g.district_yields(variant, site), plan.strategy))
                    .fold(0.0, f64::max);
                (variant, weight, best_owned)
            });
        CityPurchaseFacts {
            weakest_worked,
            best_unworked,
            next_district,
            growth_turns: g.border_growth_turns(cid),
            front: g.border_growth_front(cid),
        }
    }

    /// `priced-tile-purchase`'s answer for one legal `BuyPlot`: the surplus
    /// of the plot's benefit over its Gold, or `None` when the purchase
    /// does not clear [`TILE_PURCHASE_MARGIN`] and
    /// [`TILE_PURCHASE_MIN_SURPLUS`]. See the module header; everything is
    /// in yield-points at the lane's weights, so the Gold is priced at what
    /// the lane thinks a Gold is worth.
    pub(super) fn priced_plot_purchase_score(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        cid: u32,
        pos: Pos,
        cost: f64,
        cache: &mut PlotPurchaseCache,
    ) -> Option<f64> {
        let strategy = plan.strategy;
        if !cache.cities.contains_key(&cid) {
            let facts = self.city_purchase_facts(g, pid, plan, cid);
            cache.cities.insert(cid, facts);
        }
        let facts = &cache.cities[&cid];
        let gold_loss = cost
            * self.yield_value(
                Yields {
                    gold: 1.0,
                    ..Yields::default()
                },
                strategy,
            );

        // The payback horizon: the shorter of the game left and the standard
        // window, cut to the border's own schedule when culture would claim
        // this very plot — buying it then buys only the turns in between.
        let remaining = g.max_turns.saturating_sub(g.turn).max(1) as f64;
        let mut horizon = remaining.min(g.standard_duration(TILE_PURCHASE_PAYBACK_HORIZON) as f64);
        if facts.front.contains(&pos) {
            if let Some(turns) = facts.growth_turns {
                if turns <= TILE_PURCHASE_FREE_SOON {
                    return None;
                }
                horizon = horizon.min(turns);
            }
        }

        // The job: what a citizen gains by moving to this plot now, or what
        // the next citizen gains by having it instead of the best idle
        // owned tile (halved — the growth is not here yet).
        let tile_value = self.visible_tile_value(g, pid, pos, strategy);
        let gain_now = facts
            .weakest_worked
            .map_or(tile_value, |weakest| (tile_value - weakest).max(0.0));
        let gain_growth = facts
            .best_unworked
            .map_or(tile_value, |best| (tile_value - best).max(0.0));
        let job = gain_now.max(0.5 * gain_growth);

        // The connection: a luxury or strategic the empire does not yet own
        // is worth an Amenity or a stockpile in every city; a second copy is
        // trade goods. Bonus resources already pay through the tile yields.
        let tile = &g.map.tiles[&pos];
        let resource = tile
            .resource
            .as_ref()
            .and_then(|name| g.rules.resources.get(name).map(|spec| (name, spec)))
            .filter(|(_, spec)| {
                spec.tech
                    .as_ref()
                    .is_none_or(|tech| g.players[pid].techs.contains(tech))
                    && spec
                        .civic
                        .as_ref()
                        .is_none_or(|civic| g.players[pid].civics.contains(civic))
            })
            .map(|(name, spec)| {
                let owned_already = g.cities.values().any(|other| {
                    other.owner == pid
                        && other.owned_tiles.iter().any(|owned| {
                            g.map
                                .get(*owned)
                                .is_some_and(|t| t.resource.as_ref() == Some(name))
                        })
                });
                match (spec.class.as_str(), owned_already) {
                    ("luxury", false) => 6.0,
                    ("luxury", true) => 1.5,
                    ("strategic", false) => 4.0,
                    ("strategic", true) => 1.0,
                    _ => 0.0,
                }
            })
            .unwrap_or(0.0);

        // The site: the district this city builds next, if this plot beats
        // every plot it already owns for it.
        let district = facts
            .next_district
            .filter(|(variant, _, _)| g.plot_fits_placement(pid, *variant, pos, g.cities[&cid].pos))
            .map(|(variant, weight, best_owned)| {
                let here = self.yield_value(g.district_yields(variant, pos), strategy);
                (here - best_owned).max(0.0) * weight * TILE_PURCHASE_DISTRICT_DISCOUNT
            })
            .unwrap_or(0.0);

        let wonder = if g.tile_is_natural_wonder(tile) {
            8.0
        } else {
            0.0
        };

        let benefit = (job + resource + district + wonder) * horizon;
        let surplus = benefit - gold_loss;
        (benefit + f64::EPSILON >= TILE_PURCHASE_MARGIN * gold_loss
            && surplus + f64::EPSILON >= TILE_PURCHASE_MIN_SURPLUS)
            .then_some(surplus)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::advanced::PRODUCTION_OPT_INS;
    use crate::game::Action;

    fn plan(strategy: GrandStrategy, turn: u32) -> StrategicPlan {
        StrategicPlan {
            strategy,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: turn,
            rush: false,
        }
    }

    /// A seat's capital on a disk of bare plains out to radius six, so
    /// every term but the one under test is the same for every plot.
    fn flat_capital() -> (Game, u32, Pos) {
        let mut game = Game::new_full(2, 28, 18, 91_779, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let center = game.units[&settler].pos;
        for pos in game.wdisk(center, 6) {
            if let Some(tile) = game.map.tiles.get_mut(&pos) {
                tile.terrain = crate::name!("plains");
                tile.feature = None;
                tile.hills = false;
                tile.resource = None;
                tile.improvement = None;
                tile.district = None;
                tile.wonder = None;
                tile.river_edges = [false; 6];
            }
        }
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        (game, city, center)
    }

    /// A settle site three rings from the capital and, beside it, a plot one
    /// ring further out: every neighbour of that plot is unowned, on the
    /// flattened disk, and inside the site's two-ring look-ahead.
    fn site_and_outer_anchor(game: &Game, center: Pos) -> (Pos, Pos) {
        for site in ring(game, center, 3) {
            let anchor = game.nbrs(site).into_iter().find(|pos| {
                game.wdist(*pos, center) == 4
                    && game.nbrs(*pos).iter().all(|p| {
                        game.map
                            .get(*p)
                            .is_some_and(|t| t.owner_city.is_none() && t.terrain == "plains")
                    })
            });
            if let Some(anchor) = anchor {
                return (site, anchor);
            }
        }
        panic!("a ring-three site with an open outer neighbour");
    }

    fn ring(game: &Game, center: Pos, radius: i32) -> Vec<Pos> {
        let mut ring: Vec<Pos> = game
            .wdisk(center, radius)
            .into_iter()
            .filter(|pos| game.wdist(*pos, center) == radius && game.map.get(*pos).is_some())
            .collect();
        ring.sort();
        ring
    }

    #[test]
    fn district_lookahead_settle_is_a_native_opt_in() {
        let mut ai = AdvancedAi::new();
        ai.enable_live_bridge_universe();
        assert!(!ai.district_lookahead_settle);
        let (_, _, enable) = PRODUCTION_OPT_INS
            .iter()
            .find(|(_, tag, _)| *tag == "district-lookahead-settle")
            .expect("an opt-in row");
        enable(&mut ai);
        assert!(ai.district_lookahead_settle);
        ai.disable_district_lookahead_settle();
        assert!(!ai.district_lookahead_settle);
    }

    #[test]
    fn priced_tile_purchase_is_a_native_opt_in_that_takes_over_the_base_buyer() {
        let mut ai = AdvancedAi::new();
        ai.enable_live_bridge_universe();
        assert!(!ai.priced_tile_purchase);
        assert!(!ai.base.plot_purchase_delegated);
        let (_, _, enable) = PRODUCTION_OPT_INS
            .iter()
            .find(|(_, tag, _)| *tag == "priced-tile-purchase")
            .expect("an opt-in row");
        enable(&mut ai);
        assert!(ai.priced_tile_purchase);
        assert!(ai.base.plot_purchase_delegated);
        ai.disable_priced_tile_purchase();
        assert!(!ai.priced_tile_purchase);
        assert!(!ai.base.plot_purchase_delegated);
    }

    #[test]
    fn every_lane_wishes_for_adjacency_bearing_districts_first_first() {
        let game = Game::new_full(2, 28, 18, 91_779, 200, 0, false);
        for strategy in [
            GrandStrategy::Science,
            GrandStrategy::Culture,
            GrandStrategy::Religion,
            GrandStrategy::Diplomacy,
            GrandStrategy::Conquest,
            GrandStrategy::Recovery,
            GrandStrategy::Expansion,
        ] {
            let wishlist = AdvancedAi::new_city_district_wishlist(strategy);
            assert!(!wishlist.is_empty());
            assert_eq!(
                wishlist[0].1,
                wishlist.iter().map(|(_, w)| *w).fold(0.0, f64::max),
                "{strategy:?}: the first family carries the full weight"
            );
            for (family, weight) in wishlist {
                let spec = game
                    .rules
                    .districts
                    .get(family.as_str())
                    .unwrap_or_else(|| panic!("{strategy:?} wishes for unknown {family}"));
                assert!(
                    !spec.adjacency.is_empty(),
                    "{family} has no adjacency to look ahead at"
                );
                assert!(weight > 0.0 && weight <= 1.0);
            }
        }
    }

    /// Woods feed a Holy Site and nothing a Campus wants. The Religion lane
    /// should see the grove; the Science lane should see bare plains — and
    /// the summary this gene replaces credited both lanes alike.
    #[test]
    fn the_lookahead_prices_only_the_districts_the_lane_would_build() {
        let (mut game, _, center) = flat_capital();
        let (site, anchor) = site_and_outer_anchor(&game, center);
        let grove: Vec<Pos> = game
            .nbrs(anchor)
            .into_iter()
            .filter(|pos| *pos != site && game.map.get(*pos).is_some())
            .take(4)
            .collect();
        assert_eq!(grove.len(), 4);
        let positions = game.wdisk(site, 2);

        let mut religion = AdvancedAi::new();
        religion.enable_district_lookahead_settle();
        religion.plan = Some(plan(GrandStrategy::Religion, game.turn));
        let mut science = AdvancedAi::new();
        science.enable_district_lookahead_settle();
        science.plan = Some(plan(GrandStrategy::Science, game.turn));

        let bare_religion =
            religion.settlement_district_lookahead_value(&game, 0, site, &positions);
        let bare_science = science.settlement_district_lookahead_value(&game, 0, site, &positions);
        for pos in &grove {
            game.map.tiles.get_mut(pos).unwrap().feature = Some(crate::name!("forest"));
        }
        let grove_religion =
            religion.settlement_district_lookahead_value(&game, 0, site, &positions);
        let grove_science = science.settlement_district_lookahead_value(&game, 0, site, &positions);

        assert!(
            grove_religion > bare_religion + 1.0,
            "the Religion lane sees its Holy Site grove: {bare_religion:.1} -> {grove_religion:.1}"
        );
        assert!(
            (grove_science - bare_science).abs() < 1e-9,
            "the Science lane's Campus does not care about woods: {bare_science:.1} -> {grove_science:.1}"
        );
        // The summary this gene replaces credits the grove whatever the lane.
        let summary = AdvancedAi::new();
        assert!(summary.adjacency_site_planning);
        let summary_grove =
            summary.settlement_adjacency_value_from_positions(&game, 0, site, &positions);
        for pos in &grove {
            game.map.tiles.get_mut(pos).unwrap().feature = None;
        }
        let summary_bare =
            summary.settlement_adjacency_value_from_positions(&game, 0, site, &positions);
        assert!(
            summary_grove > summary_bare,
            "the all-families summary credits every lane"
        );
    }

    #[test]
    fn the_lookahead_enters_the_settle_score_only_when_the_gene_is_on() {
        let (mut game, _, center) = flat_capital();
        let (site, anchor) = site_and_outer_anchor(&game, center);
        let peaks: Vec<Pos> = game
            .nbrs(anchor)
            .into_iter()
            .filter(|pos| *pos != site && game.map.get(*pos).is_some())
            .take(3)
            .collect();
        assert_eq!(peaks.len(), 3);
        for pos in &peaks {
            game.map.tiles.get_mut(pos).unwrap().terrain = crate::name!("mountain");
        }
        let mut off = AdvancedAi::new();
        off.plan = Some(plan(GrandStrategy::Science, game.turn));
        let mut on = off.clone();
        on.enable_district_lookahead_settle();
        let off_value = off.settlement_static_value_uncached(&game, 0, site);
        let on_value = on.settlement_static_value_uncached(&game, 0, site);
        let summary =
            off.settlement_adjacency_value_from_positions(&game, 0, site, &game.wdisk(site, 2));
        let lookahead =
            on.settlement_district_lookahead_value(&game, 0, site, &game.wdisk(site, 2));
        assert!(lookahead > summary, "a +3 Campus on the Science lane outprices the capped summary: {lookahead:.1} v {summary:.1}");
        assert!(
            ((on_value - off_value) - (lookahead - summary)).abs() < 1e-6,
            "the gene swaps exactly the adjacency term: {off_value:.2} -> {on_value:.2}"
        );
    }

    /// Lay out the purchase scenario: a rich unowned plot two rings out that
    /// a citizen would move to, and — when `decoy` — a luxury elsewhere on
    /// the ring so the border's next pick is not the plot under test.
    fn purchase_scenario(decoy: bool) -> (Game, u32, Pos) {
        let (mut game, city, center) = flat_capital();
        let ring2 = ring(&game, center, 2);
        let candidate = ring2[0];
        {
            let tile = game.map.tiles.get_mut(&candidate).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.hills = true;
            tile.feature = Some(crate::name!("forest"));
        }
        if decoy {
            let luxury = game
                .rules
                .resources
                .iter()
                .find(|(_, spec)| {
                    spec.class == "luxury" && spec.tech.is_none() && spec.civic.is_none()
                })
                .map(|(name, _)| *name)
                .expect("an ungated luxury");
            let other = ring2
                .iter()
                .copied()
                .find(|pos| game.wdist(*pos, candidate) >= 2)
                .unwrap();
            game.map.tiles.get_mut(&other).unwrap().resource = Some(luxury);
        }
        game.cities.get_mut(&city).unwrap().border_culture = 0.0;
        (game, city, candidate)
    }

    #[test]
    fn a_worked_plot_the_border_will_not_reach_soon_pays_for_itself() {
        let (game, city, candidate) = purchase_scenario(true);
        assert!(
            !game.border_growth_front(city).contains(&candidate),
            "the decoy draws the border"
        );
        let cost = game
            .plot_purchase_cost(0, city, candidate)
            .expect("a legal ring-two purchase");
        let ai = AdvancedAi::new();
        let plan = plan(GrandStrategy::Expansion, game.turn);
        let mut cache = PlotPurchaseCache::default();
        let score = ai
            .priced_plot_purchase_score(&game, 0, &plan, city, candidate, cost, &mut cache)
            .expect("a 2/2 plot beside a plains-only capital is worth its price");
        let gold_loss = cost
            * ai.yield_value(
                Yields {
                    gold: 1.0,
                    ..Yields::default()
                },
                plan.strategy,
            );
        assert!(score + gold_loss >= TILE_PURCHASE_MARGIN * gold_loss);
        assert!(score >= TILE_PURCHASE_MIN_SURPLUS);
    }

    #[test]
    fn a_plot_the_border_takes_next_turn_is_not_bought() {
        let (mut game, city, candidate) = purchase_scenario(false);
        assert!(
            game.border_growth_front(city).contains(&candidate),
            "the best plot is the border's next pick"
        );
        let need = 10.0;
        game.cities.get_mut(&city).unwrap().border_culture = need - 0.1;
        assert_eq!(game.border_growth_turns(city), Some(1.0));
        let cost = game.plot_purchase_cost(0, city, candidate).unwrap();
        let ai = AdvancedAi::new();
        let plan = plan(GrandStrategy::Expansion, game.turn);
        let mut cache = PlotPurchaseCache::default();
        assert_eq!(
            ai.priced_plot_purchase_score(&game, 0, &plan, city, candidate, cost, &mut cache),
            None
        );
    }

    #[test]
    fn a_plot_no_citizen_would_work_does_not_clear_its_price() {
        let (game, city, center) = flat_capital();
        // Bare plains like every tile the city already works: nothing gained.
        let candidate = ring(&game, center, 2)[0];
        let cost = game.plot_purchase_cost(0, city, candidate).unwrap();
        let ai = AdvancedAi::new();
        let plan = plan(GrandStrategy::Expansion, game.turn);
        let mut cache = PlotPurchaseCache::default();
        assert_eq!(
            ai.priced_plot_purchase_score(&game, 0, &plan, city, candidate, cost, &mut cache),
            None
        );
    }

    #[test]
    fn the_gold_pass_buys_the_priced_plot_and_leaves_the_worthless_one() {
        let (mut game, city, candidate) = purchase_scenario(true);
        game.players[0].gold = 1_500.0;
        let plan = plan(GrandStrategy::Expansion, game.turn);
        let mut ai = AdvancedAi::new();
        ai.enable_priced_tile_purchase();
        let before = game.cities[&city].owned_tiles.len();
        assert!(ai.advanced_gold_spending(&mut game, 0, &plan));
        assert!(
            game.cities[&city].owned_tiles.contains(&candidate),
            "the priced plot is the one bought"
        );
        assert_eq!(game.cities[&city].owned_tiles.len(), before + 1);
        // Nothing else on the ring clears its price; a second pass on the
        // same treasury buys no plains.
        let plains_before = game.cities[&city].owned_tiles.len();
        let _ = ai.advanced_gold_spending(&mut game, 0, &plan);
        let bought_plains = game.cities[&city]
            .owned_tiles
            .iter()
            .skip(plains_before)
            .any(|pos| game.map.tiles[pos].terrain == "plains" && !game.map.tiles[pos].hills);
        assert!(
            !bought_plains,
            "bare plains beside a plains capital are not worth their Gold"
        );
    }
}
