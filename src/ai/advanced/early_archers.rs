//! `early-archers`: an Archer for every city, the frontier city first, while
//! the world is Ancient and Classical (operator, 2026-08-26: *"prioritize
//! building out archers in the early game. Archers are fairly powerful
//! units, especially with the step and shoot ability and being able to fire
//! from within cities. We can defend our frontier cities with an archer.
//! Archers are very solid on defense."*).
//!
//! What the shipped picker does with an Archer today, read off
//! `production_value`'s military arm and `tech_value`:
//!
//! - **The army is sized by bodies, not by role.** `desired_military` is one
//!   land unit per city under every peacetime plan, and the census that
//!   fills it (`EmpireCounts::add_unit`) files the opening Scout under
//!   `melee`. A one-city empire holding its Scout and two Warriors is at the
//!   ceiling, and a third body is refused outright at `-2_000` — the same
//!   veto `early-contact-window` had to lift for a second Scout.
//! - **The role balance is symmetric and empire-wide.** `role_gap` pays a
//!   shooter `+55` only while `counts.melee > counts.ranged`, and pays a
//!   melee body the same `+55` the moment shooters draw level: the target
//!   composition is one shooter for every melee unit, counted from the
//!   Scout up, and nothing asks for a shooter in any particular city.
//! - **Nothing prices the tile the shooter stands on.** A ranged unit fires
//!   from a city centre like any other tile — `Game::legal_actions` builds
//!   `Action::Ranged` off `unit_attack_range` from the unit's own position —
//!   and an Archer's shot reaches two, so a garrisoned Archer answers
//!   anything that steps up to the walls without leaving them; a shooter
//!   that has moved may still fire (`!spec.siege || !u.moved`), which is the
//!   step-and-shoot the operator names. The picker does not know a frontier
//!   city from an interior one when it trains a unit; only
//!   `contested-land-first` does, and only for Walls.
//! - **Archery is a fifty-beaker node the research picker rates below its
//!   neighbours.** `tech_value` credits it the Archer's power at `1.1` and a
//!   stranded Slinger's upgrade at `1.4`: `(27.5 + 35) / sqrt(50) ≈ 8.8`
//!   against a Pottery or a Mining at 13–17. `tech_value`'s own note names
//!   Archery first among the nodes an upgrade waited on while empires
//!   holding ten to thirty technologies still fielded Slingers.
//!
//! What the gene changes, all of it inert while the flag is off:
//!
//! 1. **One shooter per city is the target** — a land, non-siege unit with a
//!    ranged attack of range [`EARLY_ARCHERS_MIN_RANGE`] or more
//!    (`EmpireCounts::archers`; a Slinger is range one and does not count, a
//!    Pítati Archer or a Crossbowman does). While the empire holds fewer
//!    than it has cities, a producible shooter earns [`EARLY_ARCHERS_BASE`]
//!    plus [`EARLY_ARCHERS_PER_MISSING`] for every further shooter missing,
//!    and — like a Scout inside the contact window — it is not a body the
//!    army ceiling may refuse. The census includes queues and is advanced as
//!    each city commits, so two cities reviewed in one turn do not both
//!    start the same missing Archer.
//! 2. **The frontier city builds it first.** A city within
//!    `CONTESTED_LAND_FRONTIER_RADIUS` of a met major's city — the geometry
//!    `contested-land-first` reads, without its flag — with no shooter on or
//!    beside it earns [`EARLY_ARCHERS_FRONTIER`] on top, the credit that
//!    gene pays a frontier city's first Walls. A unit is trained onto the
//!    city centre and the base planner fortifies a unit with nothing to do
//!    (`BasicAi::fortify_or_stop`), so the Archer a frontier city trains is
//!    the Archer that holds it; walking one there from elsewhere is not
//!    this gene's business.
//! 3. **Archery is chased.** While the empire lacks the node that unlocks
//!    its cheapest shooter, that node and every node on the way earn
//!    [`EARLY_ARCHERS_RESEARCH`] in `tech_value` — the beeline idiom the
//!    water goals use at 150–230, at less than half their weight, because a
//!    fifty-beaker node needs no more than that to come next.
//! 4. **The window closes with the Classical era.** Every term above is zero
//!    once the empire holds a technology past [`EARLY_ARCHERS_LAST_ERA`] on
//!    `rules::ERA_NAMES`: the Archer is the shooter of the Ancient and
//!    Classical eras and the Crossbowman replaces it at Machinery, and a
//!    standing appetite for shooters past that point would be a different
//!    gene with a different measurement.
//!
//! What it deliberately does not do: it does not count Slingers toward the
//! target (a range-one shot cannot cover a city's ring from the centre),
//! does not train them ahead of Archery (the research credit is the answer
//! to a missing node, not a cheaper unit), does not change how many melee
//! bodies the plan wants, and does not move a unit.

use super::{AdvancedAi, EmpireCounts};
use crate::ai::BasicAi;
use crate::game::Game;
use crate::name::Name;
use crate::rules::UnitSpec;
use crate::Pos;

/// The last era index on `rules::ERA_NAMES` the window is open in:
/// Classical. The first Medieval technology closes it.
pub(crate) const EARLY_ARCHERS_LAST_ERA: usize = 1;
/// A shooter's reach for it to count: range two covers a city's ring from
/// the centre; a Slinger's one does not.
pub(crate) const EARLY_ARCHERS_MIN_RANGE: i32 = 2;
/// What the first missing shooter is worth to a city that can train it:
/// above a Builder's 260–295 and an early Monument's 240 once the ranking
/// has divided every arm by `7 + turns` (an Archer's 60 Production pays more
/// of that divisor than a Builder's 50, so 300 lands just under the Builder
/// at four Production a turn — 14.4 against 15.1), and far below any Settler
/// with a site (920 and up).
pub(crate) const EARLY_ARCHERS_BASE: f64 = 360.0;
/// For every further shooter the empire is short.
pub(crate) const EARLY_ARCHERS_PER_MISSING: f64 = 60.0;
/// On top, for a frontier city with no shooter on or beside it — the same
/// credit `contested-land-first` pays that city's first Walls.
pub(crate) const EARLY_ARCHERS_FRONTIER: f64 = 240.0;
/// Paid to the node that unlocks the cheapest shooter and to every node on
/// the way, while the empire lacks it.
pub(crate) const EARLY_ARCHERS_RESEARCH: f64 = 90.0;

impl AdvancedAi {
    /// A land, non-siege unit with a ranged attack that reaches
    /// `EARLY_ARCHERS_MIN_RANGE`: what this gene counts and trains.
    pub(crate) fn early_archers_shooter(spec: &UnitSpec) -> bool {
        spec.class == "military"
            && matches!(spec.domain.as_deref(), None | Some("land"))
            && !spec.siege
            && spec.has_ranged_attack()
            && spec.range >= EARLY_ARCHERS_MIN_RANGE
    }

    /// Whether the empire is still in the Archer's eras: it holds no
    /// technology past `EARLY_ARCHERS_LAST_ERA`. Read off the tree nodes the
    /// way `Game::player_era` reads them; an Advanced Start that hands out a
    /// later era's technologies closes the window by the same reading.
    pub(crate) fn early_archers_window_open(g: &Game, pid: usize) -> bool {
        g.players[pid]
            .techs
            .iter()
            .filter_map(|tech| g.rules.techs.get(tech))
            .all(|spec| spec.era <= EARLY_ARCHERS_LAST_ERA)
    }

    /// Whether a shooter of ours stands on `city` or beside it.
    fn early_archers_shooter_near(g: &Game, pid: usize, city: Pos) -> bool {
        g.units.values().any(|unit| {
            unit.owner == pid
                && Self::early_archers_shooter(&g.rules.units[unit.kind])
                && g.wdist(unit.pos, city) <= 1
        })
    }

    /// `early-archers`: what training `spec` in `cid` is worth on top of the
    /// military arm's own sum. Zero with the gene off, outside the window,
    /// for anything but a shooter, and once the empire holds a shooter per
    /// city. `city_count` is the caller's, so the arm allocates nothing new.
    pub(super) fn early_archers_value(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        spec: &UnitSpec,
        counts: &EmpireCounts,
        city_count: usize,
    ) -> f64 {
        if !self.early_archers
            || !Self::early_archers_shooter(spec)
            || !Self::early_archers_window_open(g, pid)
        {
            return 0.0;
        }
        let missing = city_count.saturating_sub(counts.archers);
        if missing == 0 {
            return 0.0;
        }
        let city = g.cities[&cid].pos;
        let frontier = BasicAi::contested_frontier_distance_inner(g, pid, city).is_some()
            && !Self::early_archers_shooter_near(g, pid, city);
        EARLY_ARCHERS_BASE
            + EARLY_ARCHERS_PER_MISSING * (missing - 1) as f64
            + if frontier {
                EARLY_ARCHERS_FRONTIER
            } else {
                0.0
            }
    }

    /// The technology that unlocks the cheapest shooter this civilization
    /// may train — the Pítati Archer's for Nubia, the Archer's for everyone
    /// else — read off the rules; `None` when the rules hold no such unit.
    fn early_archers_node(g: &Game, pid: usize) -> Option<Name> {
        let civ = &g.players[pid].civ;
        g.rules
            .units
            .values()
            .filter(|spec| spec.buildable && Self::early_archers_shooter(spec))
            .filter(|spec| spec.unique_to.as_ref().is_none_or(|unique| unique == civ))
            .filter_map(|spec| spec.tech.map(|tech| (spec.cost, tech)))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, tech)| tech)
    }

    /// `early-archers`: what `tech` is worth in `tech_value` for leading to
    /// the shooter's node. Zero with the gene off, outside the window, and
    /// once the empire holds that node.
    pub(super) fn early_archers_research_value(&self, g: &Game, pid: usize, tech: &str) -> f64 {
        if !self.early_archers || !Self::early_archers_window_open(g, pid) {
            return 0.0;
        }
        let Some(node) = Self::early_archers_node(g, pid) else {
            return 0.0;
        };
        if g.players[pid].techs.contains(&node) || !self.tech_leads_to(g, tech, &node) {
            return 0.0;
        }
        EARLY_ARCHERS_RESEARCH
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::advanced::{GrandStrategy, StrategicPlan};
    use crate::game::Item;

    /// Two cities of ours twelve and six tiles from a met neighbour's, no
    /// units on the map, Archery known.
    fn board() -> Game {
        let mut g = Game::new_full(3, 40, 20, 7_802, 250, 0, false);
        g.current = 0;
        for uid in g.units.keys().copied().collect::<Vec<_>>() {
            g.remove_unit(uid);
        }
        g.found_city_for(0, (10, 10), None);
        g.found_city_for(0, (16, 10), None);
        g.found_city_for(1, (22, 10), None);
        g.record_contact(0, 1);
        g.players[0].techs.insert(crate::name!("archery"));
        assert_eq!(g.wdist((16, 10), (22, 10)), 6, "fixture: the frontier gap");
        g
    }

    fn city_at(g: &Game, pos: Pos) -> u32 {
        g.player_city_ids(0)
            .into_iter()
            .find(|cid| g.cities[cid].pos == pos)
            .unwrap()
    }

    fn spec<'a>(g: &'a Game, unit: &str) -> &'a UnitSpec {
        &g.rules.units[&Name::new(unit)]
    }

    fn short(archers: usize) -> EmpireCounts {
        EmpireCounts {
            archers,
            ..EmpireCounts::default()
        }
    }

    #[test]
    fn off_by_default_and_toggles() {
        let ai = AdvancedAi::new();
        assert!(!ai.early_archers, "an opt-in ships off");
        assert!(!AdvancedAi::legacy().early_archers);
        let mut ai = AdvancedAi::new();
        ai.enable_early_archers();
        assert!(ai.early_archers);
        ai.disable_early_archers();
        assert!(!ai.early_archers);
    }

    #[test]
    fn a_shooter_is_a_range_two_land_unit_that_is_not_siege() {
        let g = board();
        assert!(AdvancedAi::early_archers_shooter(spec(&g, "archer")));
        assert!(AdvancedAi::early_archers_shooter(spec(&g, "crossbowman")));
        assert!(AdvancedAi::early_archers_shooter(spec(&g, "pitati_archer")));
        assert!(
            !AdvancedAi::early_archers_shooter(spec(&g, "slinger")),
            "range one cannot cover a city's ring from the centre"
        );
        assert!(
            !AdvancedAi::early_archers_shooter(spec(&g, "catapult")),
            "siege is not a garrison"
        );
        assert!(!AdvancedAi::early_archers_shooter(spec(&g, "warrior")));
        assert!(!AdvancedAi::early_archers_shooter(spec(&g, "quadrireme")));
        let mut counts = EmpireCounts::default();
        for unit in ["archer", "slinger", "warrior", "scout", "catapult"] {
            counts.add_unit(&g, unit);
        }
        assert_eq!(counts.archers, 1, "the census counts the Archer alone");
        assert_eq!(counts.ranged, 3, "the shipped tally is untouched");
    }

    #[test]
    fn the_window_closes_with_the_first_medieval_technology() {
        let mut g = board();
        assert!(AdvancedAi::early_archers_window_open(&g, 0));
        g.players[0].techs.insert(crate::name!("currency"));
        assert!(
            AdvancedAi::early_archers_window_open(&g, 0),
            "a Classical node keeps it open"
        );
        g.players[0].techs.insert(crate::name!("machinery"));
        assert!(
            !AdvancedAi::early_archers_window_open(&g, 0),
            "the Crossbowman's node closes it"
        );
    }

    #[test]
    fn a_city_short_of_a_shooter_prices_one_and_the_frontier_city_prices_it_higher() {
        let g = board();
        let capital = city_at(&g, (10, 10));
        let frontier = city_at(&g, (16, 10));
        let archer = spec(&g, "archer");
        let off = AdvancedAi::new();
        let mut on = AdvancedAi::new();
        on.enable_early_archers();

        assert_eq!(
            off.early_archers_value(&g, 0, capital, archer, &short(0), 2),
            0.0
        );
        assert_eq!(
            off.early_archers_value(&g, 0, frontier, archer, &short(0), 2),
            0.0
        );
        assert_eq!(
            on.early_archers_value(&g, 0, capital, archer, &short(0), 2),
            EARLY_ARCHERS_BASE + EARLY_ARCHERS_PER_MISSING,
            "two cities, no shooter: the base and one more missing"
        );
        assert_eq!(
            on.early_archers_value(&g, 0, frontier, archer, &short(0), 2),
            EARLY_ARCHERS_BASE + EARLY_ARCHERS_PER_MISSING + EARLY_ARCHERS_FRONTIER,
            "six tiles from a met neighbour's city, and unguarded"
        );
        assert_eq!(
            on.early_archers_value(&g, 0, frontier, archer, &short(1), 2),
            EARLY_ARCHERS_BASE + EARLY_ARCHERS_FRONTIER,
            "one shooter held somewhere: one still missing"
        );
        assert_eq!(
            on.early_archers_value(&g, 0, frontier, archer, &short(2), 2),
            0.0,
            "a shooter per city: nothing more asked"
        );
        assert_eq!(
            on.early_archers_value(&g, 0, frontier, spec(&g, "slinger"), &short(0), 2),
            0.0,
            "a Slinger is not the shooter asked for"
        );
        assert_eq!(
            on.early_archers_value(&g, 0, frontier, spec(&g, "warrior"), &short(0), 2),
            0.0
        );
    }

    #[test]
    fn a_shooter_beside_the_frontier_city_and_an_unmet_neighbour_waive_the_frontier_credit() {
        let mut on = AdvancedAi::new();
        on.enable_early_archers();

        let mut g = board();
        let frontier = city_at(&g, (16, 10));
        g.spawn_unit("archer", 0, (16, 11));
        assert_eq!(
            on.early_archers_value(&g, 0, frontier, spec(&g, "archer"), &short(1), 2),
            EARLY_ARCHERS_BASE,
            "guarded: the empire is still one short, the frontier is not"
        );
        let mut g = board();
        g.players[0].met.remove(&1);
        g.players[1].met.remove(&0);
        assert_eq!(
            on.early_archers_value(&g, 0, frontier, spec(&g, "archer"), &short(0), 2),
            EARLY_ARCHERS_BASE + EARLY_ARCHERS_PER_MISSING,
            "a neighbour we have not met is no frontier"
        );
        let mut g = board();
        g.players[0].techs.insert(crate::name!("machinery"));
        assert_eq!(
            on.early_archers_value(&g, 0, frontier, spec(&g, "archer"), &short(0), 2),
            0.0,
            "the window is shut"
        );
    }

    #[test]
    fn archery_and_its_prerequisite_are_credited_until_the_node_is_held() {
        let mut g = board();
        g.players[0].techs.remove(&crate::name!("archery"));
        let off = AdvancedAi::new();
        let mut on = AdvancedAi::new();
        on.enable_early_archers();

        assert_eq!(off.early_archers_research_value(&g, 0, "archery"), 0.0);
        assert_eq!(
            on.early_archers_research_value(&g, 0, "archery"),
            EARLY_ARCHERS_RESEARCH
        );
        assert_eq!(
            on.early_archers_research_value(&g, 0, "animal_husbandry"),
            EARLY_ARCHERS_RESEARCH,
            "the node on the way is credited the same"
        );
        assert_eq!(on.early_archers_research_value(&g, 0, "mining"), 0.0);
        assert_eq!(
            on.early_archers_research_value(&g, 0, "bronze_working"),
            0.0
        );

        g.players[0].techs.insert(crate::name!("archery"));
        assert_eq!(
            on.early_archers_research_value(&g, 0, "animal_husbandry"),
            0.0,
            "held: nothing to chase"
        );
        g.players[0].techs.remove(&crate::name!("archery"));
        g.players[0].techs.insert(crate::name!("machinery"));
        assert_eq!(on.early_archers_research_value(&g, 0, "archery"), 0.0);
    }

    /// The whole arm: a one-city empire at its army ceiling — a Scout and
    /// two Warriors on the centre — is refused a third body outright on the
    /// shipped picker, and prices a missing Archer above its Builder with
    /// the gene on.
    #[test]
    fn a_saturated_empire_still_trains_its_first_archer_ahead_of_a_builder() {
        let mut g = Game::new_full(2, 40, 20, 7_802, 250, 0, false);
        g.current = 0;
        for uid in g.units.keys().copied().collect::<Vec<_>>() {
            g.remove_unit(uid);
        }
        let city = g.found_city_for(0, (10, 10), None);
        g.players[0].techs.insert(crate::name!("archery"));
        g.spawn_unit("scout", 0, (10, 10));
        g.spawn_unit("warrior", 0, (10, 10));
        g.spawn_unit("warrior", 0, (10, 10));
        // Keep this unit-saturation fixture outside the new development half;
        // the test is about the opt-in Archer exception, not the phase floor
        // that asks a peaceful opening to hold two bodies per city.
        g.turn = g.max_turns / 2;
        let archer = Item::Unit {
            unit: crate::name!("archer"),
        };
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        assert!(g.can_produce(0, city, &archer));
        let plan = StrategicPlan {
            strategy: GrandStrategy::Expansion,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: g.turn,
            rush: false,
        };
        let off = AdvancedAi::new();
        let mut on = AdvancedAi::new();
        on.enable_early_archers();
        let counts = off.counts(&g, 0);
        assert_eq!(counts.archers, 0);

        assert_eq!(
            off.production_value(&g, 0, city, &archer, &plan, &counts),
            -2_000.0,
            "three land bodies against a target of one: the ceiling refuses"
        );
        // The ranking is the arm's sum over `7 + turns`, so the credit is
        // read against the Builder the same city would otherwise start,
        // not against the raw constant.
        let priced = on.production_value(&g, 0, city, &archer, &plan, &counts);
        let builder_priced = on.production_value(&g, 0, city, &builder, &plan, &counts);
        assert!(
            priced > 0.0,
            "the gene lifts the veto and pays the credit: {priced}"
        );
        assert!(
            priced > builder_priced,
            "ahead of the Builder: {priced} v {builder_priced}"
        );
        assert_eq!(
            builder_priced,
            off.production_value(&g, 0, city, &builder, &plan, &counts),
            "and nothing else moved"
        );
    }
}
