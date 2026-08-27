//! Yield floors: two opt-in genes for the two yields the King seat never
//! builds — culture and gold.
//!
//! **The finding (live King seat `civvis-20260826T184456Z`, and every King
//! game of 2026-08-26 that reached turn 100).** At turn 150 the seat held
//! nine cities, 96 science, 57 culture and one wonder against the Aztec
//! leader's eleven cities, 148 science, 183 culture and seven wonders — 411
//! score to 894, 46% of the leader, under the line the ladder abandons at.
//! Gaul made 180 culture from ONE city. Across the fifteen King runs of the
//! day that reached turn 100 the picture is the same in every one: culture
//! 16–62 against the best rival's 71–133, ZERO Amphitheatres, 0–2 Theatre
//! Squares, ZERO Markets or Lighthouses, trade capacity 1 with one route,
//! and six of the fifteen bankrupt (treasury at 0) for 31–73 turns. In the
//! studied game the empire's seven cities made 14 Gold a turn between them
//! at turn 100 (Rome 6.3, every other city under 3); income went negative at
//! t80, the treasury hit 0 at t92 and stayed there to t131 while the engine
//! disbanded the army it could not pay for. In 132 production orders over
//! 150 turns not one Market, Amphitheatre, Lighthouse or Water Mill was ever
//! ordered; the first Commercial Hub stood at t111 and had no Market 39
//! turns later.
//!
//! **Three mechanisms in `production_value`, none of them a tuning knob:**
//!
//! 1. The Great Work veto: any building with a Great Work slot returns
//!    −10,000 whenever the victory target is not Culture. The Amphitheatre
//!    (+2 culture, two writing slots) is the cheapest culture building in the
//!    game and the seat plays `--victory diplomatic`, so it can NEVER be
//!    built — the veto was written against Museums and takes the +2 culture
//!    with the slots. `lane-release-when-hopeless` lifts it only past half
//!    the clock and under a third of the lane's progress; by then the civic
//!    tree is eleven civics behind and the government is still Classical
//!    Republic (all fifteen runs at turn 100).
//! 2. The lane bonus for a district is a table keyed on the grand strategy,
//!    and the seat spends the game in Expansion (83 readings) and Recovery
//!    (82) — 165 of 184 — where the Theatre Square's bonus is 0 and the
//!    Commercial Hub's is 90 and 0, while Recovery pays the Industrial Zone
//!    190, the Dam 180 and the Aqueduct 120. Six Industrial Zones and five
//!    Baths were built; two Commercial Hubs at t111/t134; the one early
//!    Theatre (Arpinum, t63) fell to a city-state with its city at t89.
//! 3. Nothing prices a gold building or district by the income it is
//!    missing. The empire's response to a deficit is a policy card and a stop
//!    on unit production; the Market (+2 gold, +1 trade route) prices at
//!    `2 × 0.9 × 42 ≈ 76` under Expansion, under a Granary, with the treasury
//!    at 0 and the army disbanding.
//!
//! **Two opt-in genes, off by default, byte-identical while off:**
//!
//! - `culture-floor`: while the empire's culture a turn is under
//!   [`CULTURE_FLOOR_RATIO`] of the strongest major's, a culture-yielding
//!   building is exempt from the Great Work veto and earns
//!   [`CULTURE_FLOOR_BUILDING`] × shortfall × (its culture / 2, capped at 1),
//!   and a Theatre Square earns [`CULTURE_FLOOR_DISTRICT`] × shortfall. The
//!   shortfall is `1 − ours / (ratio × best)`, so the bonus fades to nothing
//!   as the floor is reached and never touches a seat that is keeping up.
//! - `gold-income-floor`: while net income is under [`GOLD_FLOOR_PER_CITY`]
//!   Gold a turn per city, a Market or Lighthouse (each +1 trade route in the
//!   engine, see `Game::trade_capacity`) or any gold-yielding building earns
//!   [`GOLD_FLOOR_BUILDING`] × shortfall, and a Commercial Hub or Harbor
//!   earns [`GOLD_FLOOR_DISTRICT`] × shortfall while fewer than half the
//!   cities have one of that family. The shortfall is `1 − income / target`,
//!   1 at or below zero income.
//!
//! Both read only the seat's own board and the rivals' public culture the
//! culture-lane forecast already reads (`culture_lane_forecast_score`),
//! plus the host's per-seat yield correction (`observed_yield_adjustments`)
//! that carries a rival's culture on the live board.
//! Neither touches the Trader: the first slot is
//! `solvency-first-trade-slot`'s (+4.65 pp) and filling every slot measured
//! −2.80. Two shortfalls are computed once a seat-turn and cached in
//! [`YieldFloorFrame`], so the per-item price reads a number.

use super::AdvancedAi;
use crate::game::Game;
use crate::name::Name;
use crate::rules::BuildingSpec;

/// The culture floor: our culture a turn against this share of the strongest
/// major's. 0.7 puts the measured 0.30–0.50 King ratios at a shortfall of
/// 0.29–0.57 and a seat at parity at zero.
pub(crate) const CULTURE_FLOOR_RATIO: f64 = 0.7;
/// A Theatre Square's bonus at a full shortfall. What it has to beat is not
/// a lane row but the Campus's `RESEARCH_CAMPUS_COVERAGE` (300 × the research
/// horizon) and an Industrial Zone's adjacency yields (≈260 for +2 under
/// Recovery's 2.2 production weight): at the measured King shortfalls of
/// 0.29–0.57 this pays 175–340, level with a mid-game Campus and above the
/// lane's pet district outside Recovery. At 360 the square bound once in
/// nineteen sampled turns of the studied game.
pub(crate) const CULTURE_FLOOR_DISTRICT: f64 = 600.0;
/// A +2-culture building's bonus at a full shortfall. An Amphitheatre's own
/// price under Expansion is `2 × 1.2 × 42 ≈ 101` plus 50 for its slots; a
/// Library with `research_economy` carries a 190-point debt on top of its
/// yield. The floor puts the Amphitheatre level with the Library at the
/// measured ratios and above it below them.
pub(crate) const CULTURE_FLOOR_BUILDING: f64 = 420.0;
/// The gold floor: net income of this much a turn per city. Two Gold a city
/// is one unit's upkeep a city; the studied seat made 14 for seven cities
/// and then less than nothing.
pub(crate) const GOLD_FLOOR_PER_CITY: f64 = 2.0;
/// A Commercial Hub's or Harbor's bonus at a full shortfall — the district
/// is a trade route and the seat for a Market or Lighthouse, which is
/// another. The Diplomacy lane pays the hub 150; a bankrupt empire pays it
/// twice that.
pub(crate) const GOLD_FLOOR_DISTRICT: f64 = 300.0;
/// A Market's or Lighthouse's bonus at a full shortfall; a gold-yielding
/// building without a route (a Bank, +5) earns it by `gold / 2`, capped at
/// one. Against the Market's own ≈76 under Expansion.
pub(crate) const GOLD_FLOOR_BUILDING: f64 = 360.0;

/// The two shortfalls for one seat-turn. `production_value` is called once
/// per item per city; the culture shortfall walks every major's cities, so
/// it is computed once and read many times. A city founded or lost within
/// the turn re-reads it.
#[derive(Clone, Default)]
pub(crate) struct YieldFloorFrame {
    turn: Option<u32>,
    pid: usize,
    cities: usize,
    culture: f64,
    gold: f64,
}

/// `1 − ours / (ratio × best)`, clamped to `0..=1`; zero when nobody sets a
/// bar.
pub(crate) fn culture_shortfall_from(ours: f64, best: f64) -> f64 {
    let floor = best * CULTURE_FLOOR_RATIO;
    if floor <= 0.0 {
        return 0.0;
    }
    (1.0 - ours / floor).clamp(0.0, 1.0)
}

/// `1 − income / (per-city floor × cities)`, clamped to `0..=1`; zero with
/// no cities to pay for.
pub(crate) fn gold_income_shortfall_from(income: f64, cities: usize) -> f64 {
    if cities == 0 {
        return 0.0;
    }
    let target = GOLD_FLOOR_PER_CITY * cities as f64;
    (1.0 - income / target).clamp(0.0, 1.0)
}

impl AdvancedAi {
    fn yield_floor_shortfalls(&self, g: &Game, pid: usize) -> (f64, f64) {
        if self.base.minor || self.base.barb {
            return (0.0, 0.0);
        }
        let cities = g.player_city_ids(pid).len();
        {
            let frame = self.yield_floor_frame.borrow();
            if frame.turn == Some(g.turn) && frame.pid == pid && frame.cities == cities {
                return (frame.culture, frame.gold);
            }
        }
        // A seat's culture a turn is what its cities make plus the host's
        // correction for that seat (`observed_yield_adjustments`): on the
        // live board a rival's cities are reconstructed from what the seat
        // has seen and the host's public culture figure lands in the
        // adjustment, so reading the cities alone puts every rival at ~0
        // and the floor never fires. Empty on a native board.
        let culture_per_turn = |seat: usize| {
            g.player_city_ids(seat)
                .into_iter()
                .map(|city| g.city_yields(city).culture)
                .sum::<f64>()
                + g.observed_yield_adjustments
                    .get(&seat)
                    .map_or(0.0, |adjustment| adjustment.culture)
        };
        let best = g
            .players
            .iter()
            .filter(|player| {
                player.id != pid && player.alive && !player.is_minor && !player.is_barbarian
            })
            .map(|player| culture_per_turn(player.id))
            .fold(0.0_f64, f64::max);
        let culture = culture_shortfall_from(culture_per_turn(pid), best);
        let gold = gold_income_shortfall_from(g.players[pid].gold_per_turn, cities);
        *self.yield_floor_frame.borrow_mut() = YieldFloorFrame {
            turn: Some(g.turn),
            pid,
            cities,
            culture,
            gold,
        };
        (culture, gold)
    }

    /// How far the empire's culture is under the floor, `0..=1`; zero while
    /// `culture-floor` is off.
    pub(super) fn culture_floor_shortfall(&self, g: &Game, pid: usize) -> f64 {
        if !self.culture_floor {
            return 0.0;
        }
        self.yield_floor_shortfalls(g, pid).0
    }

    /// How far the empire's income is under the floor, `0..=1`; zero while
    /// `gold-income-floor` is off.
    pub(super) fn gold_income_shortfall(&self, g: &Game, pid: usize) -> f64 {
        if !self.gold_income_floor {
            return 0.0;
        }
        self.yield_floor_shortfalls(g, pid).1
    }

    /// `culture-floor`: a building that yields culture is not a Great Work
    /// slot to be vetoed while the empire is under the floor.
    pub(super) fn culture_floor_lifts_veto(
        &self,
        g: &Game,
        pid: usize,
        spec: &BuildingSpec,
    ) -> bool {
        self.culture_floor
            && spec.yields.culture > 0.0
            && self.culture_floor_shortfall(g, pid) > 0.0
    }

    /// `culture-floor`: the bonus for a culture-yielding building.
    pub(super) fn culture_floor_building_bonus(
        &self,
        g: &Game,
        pid: usize,
        spec: &BuildingSpec,
    ) -> f64 {
        if !self.culture_floor || spec.yields.culture <= 0.0 {
            return 0.0;
        }
        CULTURE_FLOOR_BUILDING
            * self.culture_floor_shortfall(g, pid)
            * (spec.yields.culture / 2.0).clamp(0.0, 1.0)
    }

    /// `culture-floor`: the bonus for a Theatre Square.
    pub(super) fn culture_floor_district_bonus(&self, g: &Game, pid: usize, family: Name) -> f64 {
        if !self.culture_floor || family != "theater_square" {
            return 0.0;
        }
        CULTURE_FLOOR_DISTRICT * self.culture_floor_shortfall(g, pid)
    }

    /// `gold-income-floor`: the bonus for a Market, a Lighthouse or a
    /// gold-yielding building.
    pub(super) fn gold_floor_building_bonus(
        &self,
        g: &Game,
        pid: usize,
        building: Name,
        spec: &BuildingSpec,
    ) -> f64 {
        if !self.gold_income_floor {
            return 0.0;
        }
        let route = g.building_is_family(building, crate::name!("market"))
            || g.building_is_family(building, crate::name!("lighthouse"));
        let share = (spec.yields.gold / 2.0)
            .clamp(0.0, 1.0)
            .max(if route { 1.0 } else { 0.0 });
        if share <= 0.0 {
            return 0.0;
        }
        GOLD_FLOOR_BUILDING * self.gold_income_shortfall(g, pid) * share
    }

    /// `gold-income-floor`: the bonus for a Commercial Hub or Harbor while
    /// fewer than half the cities hold one of that family.
    pub(super) fn gold_floor_district_bonus(
        &self,
        g: &Game,
        pid: usize,
        family: Name,
        district_count: usize,
        city_count: usize,
    ) -> f64 {
        if !self.gold_income_floor
            || !(family == "commercial_hub" || family == "harbor")
            || district_count * 2 >= city_count
        {
            return 0.0;
        }
        GOLD_FLOOR_DISTRICT * self.gold_income_shortfall(g, pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Action;

    /// Two majors; only the second founds a city, so the first has zero
    /// culture against a real bar.
    fn board() -> Game {
        let mut g = Game::new(2, 24, 16, 71, 250, 0);
        let settler = g
            .player_unit_ids(1)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        g.current = 1;
        g.apply(1, &Action::FoundCity { unit: settler }).unwrap();
        g.current = 0;
        g
    }

    #[test]
    fn off_by_default_and_toggles() {
        let ai = AdvancedAi::new();
        assert!(!ai.culture_floor, "an opt-in ships off");
        assert!(!ai.gold_income_floor, "an opt-in ships off");
        let legacy = AdvancedAi::legacy();
        assert!(!legacy.culture_floor);
        assert!(!legacy.gold_income_floor);
        let mut ai = AdvancedAi::new();
        ai.enable_culture_floor();
        ai.enable_gold_income_floor();
        assert!(ai.culture_floor && ai.gold_income_floor);
        ai.disable_culture_floor();
        ai.disable_gold_income_floor();
        assert!(!ai.culture_floor && !ai.gold_income_floor);
    }

    #[test]
    fn the_shortfall_laws() {
        assert_eq!(
            culture_shortfall_from(0.0, 0.0),
            0.0,
            "no bar, no shortfall"
        );
        assert_eq!(
            culture_shortfall_from(100.0, 100.0),
            0.0,
            "parity clears the floor"
        );
        assert_eq!(
            culture_shortfall_from(70.0, 100.0),
            0.0,
            "the floor itself clears it"
        );
        let king = culture_shortfall_from(37.0, 90.0);
        assert!(
            (king - (1.0 - 37.0 / 63.0)).abs() < 1e-9,
            "the studied seat at t100: {king}"
        );
        assert_eq!(
            culture_shortfall_from(0.0, 50.0),
            1.0,
            "nothing against something"
        );
        assert_eq!(
            gold_income_shortfall_from(30.0, 0),
            0.0,
            "no cities, no floor"
        );
        assert_eq!(
            gold_income_shortfall_from(14.0, 7),
            0.0,
            "fourteen for seven is the floor"
        );
        assert_eq!(
            gold_income_shortfall_from(-12.9, 6),
            1.0,
            "a deficit is the full shortfall"
        );
        assert_eq!(
            gold_income_shortfall_from(7.0, 7),
            0.5,
            "half the floor, half the shortfall"
        );
    }

    #[test]
    fn the_veto_and_the_bonuses_are_zero_while_off() {
        let g = board();
        let ai = AdvancedAi::new();
        let amphitheater = &g.rules.buildings["amphitheater"];
        let market = &g.rules.buildings["market"];
        assert!(!ai.culture_floor_lifts_veto(&g, 0, amphitheater));
        assert_eq!(ai.culture_floor_building_bonus(&g, 0, amphitheater), 0.0);
        assert_eq!(
            ai.culture_floor_district_bonus(&g, 0, crate::name!("theater_square")),
            0.0
        );
        assert_eq!(
            ai.gold_floor_building_bonus(&g, 0, crate::name!("market"), market),
            0.0
        );
        assert_eq!(
            ai.gold_floor_district_bonus(&g, 0, crate::name!("commercial_hub"), 0, 3),
            0.0
        );
    }

    #[test]
    fn a_seat_with_no_culture_against_a_rival_is_at_the_full_shortfall() {
        let g = board();
        let mut ai = AdvancedAi::new();
        ai.enable_culture_floor();
        assert_eq!(
            ai.culture_floor_shortfall(&g, 0),
            1.0,
            "zero culture against a capital"
        );
        let amphitheater = &g.rules.buildings["amphitheater"];
        assert!(
            ai.culture_floor_lifts_veto(&g, 0, amphitheater),
            "the Amphitheatre is culture"
        );
        assert_eq!(
            ai.culture_floor_building_bonus(&g, 0, amphitheater),
            CULTURE_FLOOR_BUILDING
        );
        let monument = &g.rules.buildings["monument"];
        assert_eq!(
            ai.culture_floor_building_bonus(&g, 0, monument),
            CULTURE_FLOOR_BUILDING / 2.0,
            "a +1 building earns half"
        );
        let library = &g.rules.buildings["library"];
        assert!(
            !ai.culture_floor_lifts_veto(&g, 0, library),
            "no culture, no exemption"
        );
        assert_eq!(ai.culture_floor_building_bonus(&g, 0, library), 0.0);
        assert_eq!(
            ai.culture_floor_district_bonus(&g, 0, crate::name!("theater_square")),
            CULTURE_FLOOR_DISTRICT
        );
        assert_eq!(
            ai.culture_floor_district_bonus(&g, 0, crate::name!("campus")),
            0.0
        );
        // The rival with the capital sets the bar for itself and clears it.
        assert_eq!(
            ai.culture_floor_shortfall(&g, 1),
            0.0,
            "the leader is over the floor"
        );
        // On the live board a rival's culture arrives as the host's per-seat
        // correction, not as modelled cities: the floor reads it.
        let mut g = g;
        g.observed_yield_adjustments.insert(
            0,
            crate::rules::Yields {
                culture: 1_000.0,
                ..crate::rules::Yields::default()
            },
        );
        g.turn += 1;
        assert_eq!(
            ai.culture_floor_shortfall(&g, 0),
            0.0,
            "the correction counts as culture"
        );
        assert!(
            ai.culture_floor_shortfall(&g, 1) > 0.99,
            "and sets the bar for the rival"
        );
    }

    #[test]
    fn the_gold_floor_prices_routes_and_gold_until_half_the_cities_have_a_hub() {
        let mut g = board();
        let mut ai = AdvancedAi::new();
        ai.enable_gold_income_floor();
        assert_eq!(
            ai.gold_income_shortfall(&g, 0),
            0.0,
            "no cities, nothing to pay for"
        );
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        g.players[0].gold_per_turn = -3.0;
        assert_eq!(
            ai.gold_income_shortfall(&g, 0),
            1.0,
            "a deficit is the full shortfall"
        );
        let market = &g.rules.buildings["market"];
        let lighthouse = &g.rules.buildings["lighthouse"];
        let bank = &g.rules.buildings["bank"];
        let library = &g.rules.buildings["library"];
        assert_eq!(
            ai.gold_floor_building_bonus(&g, 0, crate::name!("market"), market),
            GOLD_FLOOR_BUILDING
        );
        assert_eq!(
            ai.gold_floor_building_bonus(&g, 0, crate::name!("lighthouse"), lighthouse),
            GOLD_FLOOR_BUILDING,
            "a Lighthouse is a route with no gold yield"
        );
        assert_eq!(
            ai.gold_floor_building_bonus(&g, 0, crate::name!("bank"), bank),
            GOLD_FLOOR_BUILDING
        );
        assert_eq!(
            ai.gold_floor_building_bonus(&g, 0, crate::name!("library"), library),
            0.0
        );
        let hub = crate::name!("commercial_hub");
        assert_eq!(
            ai.gold_floor_district_bonus(&g, 0, hub, 0, 3),
            GOLD_FLOOR_DISTRICT
        );
        assert_eq!(
            ai.gold_floor_district_bonus(&g, 0, crate::name!("harbor"), 1, 3),
            GOLD_FLOOR_DISTRICT
        );
        assert_eq!(
            ai.gold_floor_district_bonus(&g, 0, hub, 2, 3),
            0.0,
            "half the cities hold one"
        );
        assert_eq!(
            ai.gold_floor_district_bonus(&g, 0, crate::name!("campus"), 0, 3),
            0.0
        );
        // The frame is per turn: a new turn re-reads the income.
        g.players[0].gold_per_turn = 10.0;
        assert_eq!(
            ai.gold_income_shortfall(&g, 0),
            1.0,
            "cached within the turn"
        );
        g.turn += 1;
        assert_eq!(
            ai.gold_income_shortfall(&g, 0),
            0.0,
            "re-read on the next turn"
        );
    }
}
