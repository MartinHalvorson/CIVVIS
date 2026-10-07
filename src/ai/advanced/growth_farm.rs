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

use super::*;

/// The value of a point of Housing in a Housing-bound city: about one worked
/// tile's yield, the citizen it lets the city grow into.
pub(super) const GROWTH_HOUSING_VALUE: f64 = 6.0;
/// The extra value of a point of Food in a city whose surplus no longer grows
/// it, on top of the lane's own Food weight.
pub(super) const GROWTH_FOOD_VALUE: f64 = 1.6;
/// A city this large has outgrown the small-city deficit the gene answers.
pub(super) const GROWTH_CITY_POP_MAX: i32 = 12;

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
        if !self.growth_prices_the_farm {
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
        let mut premium = 0.0;
        if g.city_housing_headroom(city) <= 1.0 {
            premium += spec.housing * GROWTH_HOUSING_VALUE;
        }
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
}
