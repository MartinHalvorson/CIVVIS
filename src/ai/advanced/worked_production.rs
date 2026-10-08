//! `builders-improve-the-worked-for-production`: the Builder prices the
//! Production an improvement adds to a tile a city is already working.
//!
//! **The finding (10-06/07/08 Emperor runs, 40-46 games).** The operator
//! retires a game at turn 150 that is not top 2 of 4 by Production. Worked
//! tiles are about 90% of our Production, and they are what separates our top
//! third of games by Production per citizen from the bottom third: 1.66
//! Production a worked tile against 1.22 at turn 100 (2.17 against 1.46 at
//! turn 150), while cards, governors, trade and, until turn 150, Industrial
//! Zones are level. The top third's improved tiles make **2.11** Production
//! against **1.43**: its Builders laid 5.3 Mines and 3.6 Lumber Mills by turn
//! 100 against 3.5 and 2.2, with the same nine Builders and the same five
//! charges in hand. Both thirds still owned about five bare hills and five
//! forests unimproved at turn 100. From turn 30 to 110 the median game laid
//! 4.3 Farms on flat ground no citizen worked, its largest single kind of
//! job, against 1.6 Mines on worked hills and 1.2 Lumber Mills on worked
//! forests.
//!
//! **The mechanism.** `AdvancedAi::improvement_value_with_appeal` prices an
//! improvement by its printed yields, the same whether a citizen works the
//! tile today or not, and `growth-prices-the-farm` adds a Farm's Housing in
//! every small city within one citizen of its Housing — 52-60% of our cities.
//! A Farm on an idle flat therefore outbids a Mine under a citizen.
//!
//! **The gene.** On a tile one of our cities works, an improvement's
//! Production (with the tree's upgrades, Apprenticeship's +1 on a Mine) is
//! priced again at `WORKED_PRODUCTION_PREMIUM` times the lane's Production
//! weight, scaled like the growth premium by the city's production-foundation
//! shortfall. A city whose Food surplus is at most `WORKED_FOOD_SURPLUS_FLOOR`
//! with Housing to grow into is starving and pays nothing, so it still farms;
//! a city within one citizen of its Housing pays the premium, because the Food
//! a Farm adds there is mostly wasted. Off, every value is unchanged.

use super::*;

/// The top third's Production per improved tile over the bottom third's at
/// turn 100 (2.11 / 1.43): a worked tile's Production point is priced this
/// many times the lane's Production weight again, on top of its printed value.
/// Under Conquest a worked Mine then reads 6.9 against a Housing-bound city's
/// Farm at 4.2-5.8.
pub(super) const WORKED_PRODUCTION_PREMIUM: f64 = 2.11 / 1.43;
/// A city with at most this Food surplus and Housing to grow into is starving
/// and keeps the stock pricing.
pub(super) const WORKED_FOOD_SURPLUS_FLOOR: f64 = 1.0;

/// The tiles each city works this turn, for a sweep that asks about every
/// owned tile. A live mirror's observed assignment is read directly and never
/// cached; this holds the simulator's citizen plan, which is costly to derive.
#[derive(Clone, Default)]
pub(crate) struct WorkedTilesFrame {
    turn: Option<u32>,
    pid: usize,
    worked: BTreeMap<u32, Vec<Pos>>,
}

impl AdvancedAi {
    /// Whether city `cid` of `pid` works `pos` this turn.
    fn city_works_the_tile(&self, g: &Game, pid: usize, cid: u32, pos: Pos) -> bool {
        if let Some(worked) = g.observed_city_worked_tiles.get(&cid) {
            return worked.contains(&pos);
        }
        let mut frame = self.builder_worked_frame.borrow_mut();
        if frame.turn != Some(g.turn) || frame.pid != pid {
            *frame = WorkedTilesFrame {
                turn: Some(g.turn),
                pid,
                worked: BTreeMap::new(),
            };
        }
        frame
            .worked
            .entry(cid)
            .or_insert_with(|| g.city_citizen_plan(cid).worked_tiles)
            .contains(&pos)
    }

    /// `builders-improve-the-worked-for-production`: the premium `improvement`
    /// earns on `pos` for the Production it adds under a citizen. Zero with the
    /// gene off, for an improvement with no Production, on a tile no city of
    /// `pid` owns or works, and in a starving city with Housing to grow into.
    pub(super) fn worked_production_premium(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        improvement: &str,
        strategy: GrandStrategy,
    ) -> f64 {
        if !self.builders_improve_the_worked_for_production {
            return 0.0;
        }
        let Some(cid) = g.map.tiles.get(&pos).and_then(|tile| tile.owner_city) else {
            return 0.0;
        };
        let Some(city) = g.cities.get(&cid).filter(|city| city.owner == pid) else {
            return 0.0;
        };
        let Some(spec) = g.rules.improvements.get(improvement) else {
            return 0.0;
        };
        let production =
            spec.yields.production + Self::improvement_tree_yields(g, pid, improvement).production;
        if production <= 0.0 || !self.city_works_the_tile(g, pid, cid, pos) {
            return 0.0;
        }
        let surplus = g.city_yields(cid).food - 2.0 * city.pop as f64;
        if surplus <= WORKED_FOOD_SURPLUS_FLOOR && g.city_housing_headroom(city) > 1.0 {
            return 0.0;
        }
        self.yield_value(
            Yields {
                production,
                ..Yields::default()
            },
            strategy,
        ) * WORKED_PRODUCTION_PREMIUM
            * (1.0
                + self.city_production_foundation_shortfall(g, pid, cid)
                    * PRODUCTION_FOUNDATION_IMPROVEMENT_MULTIPLIER)
    }
}
