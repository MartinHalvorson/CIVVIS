//! `growth-prices-the-farm`: the Builder prices the Housing and the Food a
//! small city cannot grow without.
//!
//! **The finding (122 Emperor runs of 2026-10-06/07, 308 city pairs 20 turns
//! apart).** The best rival's production leads ours 2.3× at turn 100, and
//! once the AI's +40% production handicap is taken out the biggest part left
//! is city SIZE: 5.1 citizens a city against 8.6 at the same city count.
//! Our cities grow 1.25 citizens in 20 turns. A city with Housing to spare and
//! Food to grow grows 1.91; one within a citizen of its Housing (52-60% of our
//! cities at turns 60-120) grows 1.19; one with no Food surplus (surplus at
//! most one, 19-30%) grows 0.50.
//!
//! **The mechanism.** `AdvancedAi::improvement_value_with_appeal` prices an
//! improvement by its printed yields alone. Under Conquest, the seat's usual
//! lane, Food weighs 1.2 and Production 2.8, so a Mine outbids a Farm on every
//! hill, and the Farm's half point of Housing is not priced at all. By turn 100
//! the Builders had laid a median 6.1 Mines, 4.2 Lumber Mills and 1.5 Quarries
//! a run against 4.3 Farms: half a Farm a city. Housing from improvements was
//! 0.3-0.65 a city, and 40% of our cities sit on sites without fresh water
//! (2-3 Housing from water).
//!
//! **The gene.** In one of our cities below `GROWTH_CITY_POP_MAX` citizens, an
//! improvement's Housing is worth `GROWTH_HOUSING_VALUE` a point while the city
//! is within one citizen of its Housing, and its Food is worth a further
//! `GROWTH_FOOD_VALUE` a point while the city's Food surplus is at most one.
//! In a city below its production foundation the premium is scaled by the
//! same multiplier the foundation pays a Mine's production, since the citizen
//! it buys works a tile there too. A Farm in a Housing-bound city then outbids
//! a Mine; a city with room and Food keeps its Mines. Off, every value is
//! unchanged.
//!
//! ## `growth-prices-the-farm-2`
//!
//! **The finding (2026-10-13, 5 Immortal runs of the day and 33 surviving
//! 10-10/11 copies, `tiles` + `state` records).** Version 1 left the half of
//! our cities that still have Housing room at the stock price, and there the
//! Mine wins every tile: a worked Mine in a city making 5 Production scores
//! about 18 against a Farm's 2. The Builder journal of the day's five games
//! laid 93 Mines (median worth 15.5) and 63 Lumber Mills (13.9) against 40
//! Farms (6.2). Farms are 9-10% of our worked tiles and 14-25% of our
//! improvements; on the rivals' visible land they are 45-56%. So our worked
//! land yields 2.04 Food a tile at turn 60 and 1.80 at 100 against the two a
//! citizen eats: every citizen we add feeds only itself, a city's surplus
//! stays the +3-4 of its centre, and growth stays linear while the rivals'
//! compounds (Food a citizen 2.74 against 3.06 at turn 60). Cities with room
//! working at least 2.2 Food a tile at turn 60 grew 4.0 citizens and 7.4
//! Production by turn 100; those under 1.8 grew 1.9 and 4.2. And the Farm is
//! a Housing source we skip: improvements give our cities 0.29 Housing at
//! turn 60 and 0.66 at 100 (+0.5 a Farm, Pasture or Plantation; a Mine,
//! Quarry or Lumber Mill gives none), while 45-50% of our cities grow at x0.5
//! or x0.25 from their cap at every mark from turn 40 to 125.
//!
//! **The gene.** In one of our cities below `GROWTH_CITY_POP_MAX` citizens
//! with at least two Housing of room, an improvement's Food is priced as the
//! citizens it buys: `growth_citizens` over `GROWTH_HORIZON_STANDARD_TURNS`
//! (the speed's growth cost at this size, capped one below the Housing), half
//! of them on average across the horizon, each worth
//! `CITIZEN_PRODUCTION_POINTS` of Production. That Production is priced the
//! way a Mine's is — the lane's Production weight, times the city's
//! production-foundation multiplier, and again by `WORKED_PRODUCTION_PREMIUM`
//! on a worked tile while `builders-improve-the-worked-for-production` is on —
//! less the Food weight the stock value already paid. Version 1's Housing
//! price now ramps in from `HOUSING_RAMP_HEADROOM` instead of starting at the
//! cap, since a Farm laid then is standing when the city arrives there. A
//! small worked Farm then meets a worked Mine at near parity; a large city,
//! one without room, or a Mine after Apprenticeship keeps the Mine.

use super::*;

/// The value of a point of Housing in a Housing-bound city: about one worked
/// tile's yield, the citizen it lets the city grow into.
pub(super) const GROWTH_HOUSING_VALUE: f64 = 6.0;
/// The extra value of a point of Food in a city whose surplus no longer grows
/// it, on top of the lane's own Food weight.
pub(super) const GROWTH_FOOD_VALUE: f64 = 1.6;
/// A city this large has outgrown the small-city deficit the gene answers.
pub(super) const GROWTH_CITY_POP_MAX: i32 = 12;
/// `growth-prices-the-farm-2`: the horizon, in Standard turns, over which a
/// point of Food is credited with the citizens it grows (40 Online turns).
pub(super) const GROWTH_HORIZON_STANDARD_TURNS: u32 = 60;
/// `growth-prices-the-farm-2`: one citizen grown, in Production points. Our
/// worked land made 1.25 Production a tile at turn 60 and the next tile a
/// city takes about 0.6, plus the citizen's own Science and Culture.
pub(super) const CITIZEN_PRODUCTION_POINTS: f64 = 1.0;
/// `growth-prices-the-farm-2`: Housing is priced in full within one citizen of
/// the cap, a third less for each citizen of room above that, and not at all
/// from this much room.
pub(super) const HOUSING_RAMP_HEADROOM: f64 = 3.0;

impl AdvancedAi {
    /// `growth-prices-the-farm`: the premium the city working `pos` pays for
    /// `improvement`'s Housing and Food. Zero with the gene off, on a tile no
    /// city of `pid` owns, and in a city of `GROWTH_CITY_POP_MAX` or more.
    pub(super) fn growth_farm_premium(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        improvement: &str,
    ) -> f64 {
        if !self.growth_prices_the_farm && !self.growth_prices_the_farm_2 {
            return 0.0;
        }
        let Some(cid) = g.map.tiles.get(&pos).and_then(|tile| tile.owner_city) else {
            return 0.0;
        };
        let Some(city) = g.cities.get(&cid).filter(|city| city.owner == pid) else {
            return 0.0;
        };
        if city.pop >= GROWTH_CITY_POP_MAX {
            return 0.0;
        }
        let Some(spec) = g.rules.improvements.get(improvement) else {
            return 0.0;
        };
        let mut premium = spec.housing
            * GROWTH_HOUSING_VALUE
            * self.housing_price_share(g.city_housing_headroom(city));
        let surplus = g.city_yields(cid).food - 2.0 * city.pop as f64;
        if surplus <= 1.0 && spec.yields.food > 0.0 {
            premium += spec.yields.food * GROWTH_FOOD_VALUE;
        }
        // The citizen this buys works a tile in a city below its production
        // foundation, so it earns the same multiplier the foundation pays a
        // Mine's printed production (`production_foundation_improvement_bonus`).
        // Live G317-G319 (civvis-20261007T060649Z..T063028Z) showed why: the
        // gene fired, but a weak city's Mine read 15-49 to a Farm's 1.8-5, and
        // 7 of the triggered cities' 19 production improvements were laid
        // while a farmable flat stood unimproved beside them.
        premium
            * (1.0
                + self.city_production_foundation_shortfall(g, pid, cid)
                    * PRODUCTION_FOUNDATION_IMPROVEMENT_MULTIPLIER)
    }

    /// The share of `GROWTH_HOUSING_VALUE` a point of Housing earns at this
    /// room: all of it within one citizen of the cap; with version 2 also a
    /// ramp down to nothing at `HOUSING_RAMP_HEADROOM + 1` citizens of room.
    fn housing_price_share(&self, headroom: f64) -> f64 {
        if headroom <= 1.0 {
            1.0
        } else if self.growth_prices_the_farm_2 {
            ((HOUSING_RAMP_HEADROOM + 1.0 - headroom) / HOUSING_RAMP_HEADROOM).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// `growth-prices-the-farm-2`: the citizens a point of Food a turn grows
    /// in a city of `pop` with `headroom` Housing of room over the horizon,
    /// stopping one below the cap where the host halves growth.
    pub(super) fn growth_citizens(g: &Game, pop: i32, headroom: f64) -> f64 {
        let horizon = g.standard_duration(GROWTH_HORIZON_STANDARD_TURNS) as f64;
        (horizon / g.growth_cost(pop).max(1.0)).min((headroom - 1.0).max(0.0))
    }

    /// `growth-prices-the-farm-2`: the premium a growing city of `pid` pays
    /// for `improvement`'s Food on `pos`, priced as the Production of the
    /// citizens it grows and multiplied as a Mine's Production is. Zero with
    /// the gene off, on a tile no city of `pid` owns, in a city of
    /// `GROWTH_CITY_POP_MAX` or more, with under two Housing of room, and for
    /// an improvement without Food.
    pub(super) fn growing_city_food_premium(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        improvement: &str,
        strategy: GrandStrategy,
    ) -> f64 {
        if !self.growth_prices_the_farm_2 {
            return 0.0;
        }
        let Some(cid) = g.map.tiles.get(&pos).and_then(|tile| tile.owner_city) else {
            return 0.0;
        };
        let Some(city) = g.cities.get(&cid).filter(|city| city.owner == pid) else {
            return 0.0;
        };
        if city.pop >= GROWTH_CITY_POP_MAX {
            return 0.0;
        }
        let Some(spec) = g.rules.improvements.get(improvement) else {
            return 0.0;
        };
        let food = spec.yields.food + Self::improvement_tree_yields(g, pid, improvement).food;
        let headroom = g.city_housing_headroom(city);
        if food <= 0.0 || headroom < 2.0 {
            return 0.0;
        }
        let citizens = Self::growth_citizens(g, city.pop, headroom);
        let production = self.yield_value(
            Yields {
                production: food * CITIZEN_PRODUCTION_POINTS * citizens / 2.0,
                ..Yields::default()
            },
            strategy,
        );
        let worked = if self.builders_improve_the_worked_for_production
            && self.city_works_the_tile(g, pid, cid, pos)
        {
            worked_production::WORKED_PRODUCTION_PREMIUM
        } else {
            0.0
        };
        let priced = production
            * (1.0
                + self.city_production_foundation_shortfall(g, pid, cid)
                    * PRODUCTION_FOUNDATION_IMPROVEMENT_MULTIPLIER)
            * (1.0 + worked);
        // The stock value already paid the printed Food at the lane's weight.
        let paid = self.yield_value(
            Yields {
                food: spec.yields.food,
                ..Yields::default()
            },
            strategy,
        );
        (priced - paid).max(0.0)
    }
}
