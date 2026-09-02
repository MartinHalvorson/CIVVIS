//! `chase-every-boost`: one opt-in gene that hunts Eurekas AND Inspirations
//! everywhere the planner already decides something a boost trigger names.
//!
//! Measured on the live King/Emperor ladder (ledger runs of 2026-08-30 to
//! 2026-09-01, 32 runs past turn 100): of the technologies the seat
//! researched, **13–40 % had been boosted**; of the civics it adopted,
//! **0–26 %, typically 5–12 %**. Each boost is 40 % of the node's cost. The
//! one near-win on Emperor (run 20260901T132005Z) lost the space race by a
//! single turn with 29 of 72 techs boosted and 11 of 43 civics inspired. A
//! strong human science player boosts most of both trees; the gap is a
//! research-pace lever the size of the whole late-game deficit.
//!
//! Why the six existing boost genes did not close it: three of them only
//! re-order research (`boost-first-research`, `boost-wait-research`,
//! `boost-unlock-research`), two chase a trigger by building the thing it
//! names (`eureka-chasing-builder`, `eureka-chasing-production`), and all
//! five read [`AdvancedAi::eureka_chases`], whose progress reader knew **six
//! buildable trigger prefixes** and returned `None` for every other one —
//! kills, camps, contacts, trade routes, population, districts, wonders,
//! great people. Nothing anywhere steered a unit action toward a boost, and
//! the science lane's beeline (`goal_pick` in `plan_research`) walked the
//! cheapest prerequisite by printed cost, blind to which prerequisite was
//! already 40 % paid for — on the live seat the lane beelines nearly every
//! turn, so the ordering genes were inert exactly where they mattered.
//!
//! What this gene does, as one arm so the live ladder can screen it whole:
//!
//! 1. **Every trigger is a chase.** [`Game::boost_progress`] is the engine's
//!    own `(have, need)` for every one of the ~45 trigger forms, so the chase
//!    table covers both trees completely. Counter-backed triggers (`kills`,
//!    `barbs_killed`, `kill_with:*`, `camps`, …) read zero on the host seat,
//!    where the mirror never fills `player.counters`; that keeps them open as
//!    chases-by-action until the host reports the boost fired, which is the
//!    truthful answer.
//! 2. **Research and civics.** The in-hand scale (`boost_in_hand_scale`) is
//!    deliberately NOT armed (its isolation read negative, see that function);
//!    a node whose boost is
//!    **one actionable step away** is deferred (`boost_wait_penalty`, in its
//!    tight form); a node is credited the chases it opens, under a cap a
//!    third of the standalone gene's; and the **beeline** takes the goal's
//!    prerequisites by effective cost — printed cost less the boost in hand,
//!    or half the boost when one action away — which never lengthens the
//!    path because every prerequisite is walked anyway.
//! 3. **Production.** A unit, building, district or wonder that advances an
//!    open chase is worth the research per step it earns, discounted by the
//!    steps still to go, less what other cities already have queued for the
//!    same trigger, so two cities do not both build "the second Archer".
//! 4. **Builders** keep `eureka-chasing-builder`'s premium, the strongest of
//!    the old family (+11.7 pp wins in its own probe).
//! 5. **Kills.** A kill that fires `kill_with:<kind>`, `kills`, `barbs_killed`
//!    or clears a camp is worth the research it earns — only when the exact
//!    evaluation says the attacker survives, so the premium can raise a kill
//!    above the threshold but never make a losing exchange acceptable.
//!
//! Off, every path is byte-identical: each hook is gated on the flag and the
//! old genes keep their own flags and behaviour.
use super::deity_habits::EurekaChase;
use super::AdvancedAi;
use crate::game::{Game, Item};
use crate::Pos;

/// Kill premium: what one research point of an open boost is worth on the
/// tactical scale, where a clean kill of a cheap unit scores ~190. Archery
/// (50 × 0.4 = 20 points) pays a Slinger kill +20; Military Tactics (300 ×
/// 0.4 = 120) would pay +120 and is capped below.
pub(super) const CHASE_KILL_VALUE_PER_POINT: f64 = 1.0;
/// The ceiling on the kill premium: a third of a kill's own base value, so
/// the premium re-orders which kill is taken and never invents one.
pub(super) const CHASE_KILL_VALUE_CAP: f64 = 60.0;
/// Production premium per research point, on `production_value`'s raw
/// scale. ⚠ THAT SCALE IS SMALL: measured on the probe's own board at turns
/// 60–180, an Archer reads 68, a Campus 51, an Amphitheater 46, a Trebuchet
/// 95–161, a Crossbowman 180. `eureka-chasing-production`'s 4.0 per point
/// (capped at 400) added +234 to a Trebuchet and +400 to an Entertainment
/// Complex — three to eight times the item's own worth — and a two-city
/// empire built the trigger instead of the Settler; that is why that gene
/// measured a null. A quarter point per beaker makes a 40-point step worth
/// +10 against items worth 50–200: a nudge among comparable builds.
pub(super) const CHASE_PRODUCTION_VALUE_PER_POINT: f64 = 0.25;
/// The production premium's absolute ceiling, on the same scale.
pub(super) const CHASE_PRODUCTION_VALUE_CAP: f64 = 60.0;
/// The production premium's relative ceiling: never more than this share
/// of the item's own positive value, so a trigger can only re-order builds
/// of comparable worth and never lifts a marginal item over a Settler or a
/// Campus.
pub(super) const CHASE_PRODUCTION_RAW_FRACTION: f64 = 0.5;
/// The builder premium's relative ceiling under this gene, against the
/// improvement's own yield value: the same discipline as production.
pub(super) const CHASE_BUILDER_VALUE_FRACTION: f64 = 0.5;
/// How much of a step's value survives for each further step the trigger
/// still needs: the third Archer is worth its full step only once two are
/// standing, and a chase three builds out may never be finished.
pub(super) const CHASE_STEP_DISCOUNT: f64 = 0.8;
/// Beeline: a boost one actionable step away is priced as this much of a
/// boost in hand — the action is available now, and the discount is
/// credited mid-research if the node is still running when it lands.
pub(super) const CHASE_ONE_STEP_FRACTION: f64 = 0.5;
/// The cap, in turns of research, on the unlock credit under this gene.
/// `boost-unlock-research` capped at six turns (132 on the value scale
/// against a flat 28 for a boost in hand) and measured slightly negative;
/// two turns keeps the permission worth less than the boost it opens.
pub(super) const CHASE_UNLOCK_TURNS_CAP: f64 = 2.0;

impl AdvancedAi {
    /// The open chase for `node`, if its boost is still earnable.
    pub(super) fn chase_for(&self, g: &Game, pid: usize, node: &str) -> Option<EurekaChase> {
        self.eureka_chases(g, pid)
            .into_iter()
            .find(|chase| chase.node.as_str() == node)
    }

    /// Is this chase one actionable step from firing: exactly one trigger
    /// step left, and the thing that step names is something the empire can
    /// do right now?
    pub(super) fn chase_one_action_away(&self, g: &Game, pid: usize, chase: &EurekaChase) -> bool {
        chase.remaining == 1 && self.chase_actionable(g, pid, chase)
    }

    /// Can the empire act on this trigger now — build the thing, improve the
    /// tile, make the kill? Triggers that only time or growth advance
    /// (population, contacts, alliances, great people) are not actions and
    /// read `false`, so nothing is deferred waiting for them.
    pub(super) fn chase_actionable(&self, g: &Game, pid: usize, chase: &EurekaChase) -> bool {
        let trigger = chase.trigger.as_str();
        let owns_unit = |want: &dyn Fn(&str) -> bool| {
            g.units
                .values()
                .any(|unit| unit.owner == pid && want(unit.kind.as_str()))
        };
        let military = |kind: &str| {
            g.rules
                .units
                .get(kind)
                .is_some_and(|spec| spec.class == "military")
        };
        if let Some(kind) = trigger.strip_prefix("kill_with:") {
            return owns_unit(&|have| have == kind);
        }
        if matches!(trigger, "kills" | "barbs_killed" | "camps") {
            return owns_unit(&military);
        }
        if trigger == "cities" {
            return owns_unit(&|have| have == "settler");
        }
        if trigger == "trade_routes" {
            return owns_unit(&|have| have == "trader")
                || Self::trigger_gates(g, "units_of:trader")
                    .iter()
                    .all(|(gate, techs)| Self::node_known(g, pid, gate.as_str(), *techs));
        }
        if trigger == "wonders" {
            return g
                .cities
                .values()
                .filter(|city| city.owner == pid)
                .any(|city| {
                    city.queue
                        .iter()
                        .any(|item| matches!(item, Item::Wonder { .. }))
                });
        }
        let gates = Self::trigger_gates(g, trigger);
        if gates.is_empty() {
            return false;
        }
        gates
            .iter()
            .all(|(gate, techs)| Self::node_known(g, pid, gate.as_str(), *techs))
    }

    /// The cost the beeline should walk a prerequisite at: the printed cost
    /// less the boost in hand, or less half the boost when it is one action
    /// away; the printed cost with the gene off. Ordering the goal's own
    /// prerequisites this way cannot lengthen the path — every one of them is
    /// researched before the goal whatever the order.
    pub(super) fn beeline_step_cost(&self, g: &Game, pid: usize, node: &str, techs: bool) -> f64 {
        let cost = if techs {
            g.rules.techs[node].cost
        } else {
            g.rules.civics[node].cost
        };
        if !self.chase_every_boost {
            return cost;
        }
        let frac = Self::boost_frac(g, node, techs).clamp(0.0, 0.99);
        if Self::boost_in_hand(g, pid, node, techs) {
            return cost * (1.0 - frac);
        }
        match self.chase_for(g, pid, node) {
            Some(chase) if self.chase_one_action_away(g, pid, &chase) => {
                cost * (1.0 - frac * CHASE_ONE_STEP_FRACTION)
            }
            _ => cost,
        }
    }

    /// The production premium `production_value` adds for `item` in city
    /// `cid`: the old `eureka-chasing-production` rate when only that gene is
    /// on, this gene's chase-aware version when it is on. Zero with both off.
    pub(super) fn production_boost_premium(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        raw: f64,
    ) -> f64 {
        if self.chase_every_boost {
            self.chase_production_premium(g, pid, cid, item)
                .min(raw.max(0.0) * CHASE_PRODUCTION_RAW_FRACTION)
        } else {
            self.eureka_production_premium(g, pid, item)
        }
    }

    /// The builder premium `improvement_value_with_appeal` adds: the old
    /// `eureka-chasing-builder` rate when that gene is on, the same rate held
    /// under half the improvement's own value when only this gene is on.
    pub(super) fn builder_boost_premium(
        &self,
        g: &Game,
        pos: Pos,
        improvement: &str,
        value: f64,
    ) -> f64 {
        let premium = self.eureka_builder_premium(g, pos, improvement);
        if self.chase_every_boost && !self.eureka_chasing_builder {
            premium.min(value.max(0.0) * CHASE_BUILDER_VALUE_FRACTION)
        } else {
            premium
        }
    }

    /// The trigger key an item advances, in the chase table's own spelling.
    /// A wonder advances the tree-agnostic `wonders` trigger (Drama and
    /// Poetry's inspiration); projects and repairs advance nothing.
    fn item_trigger_key(g: &Game, item: &Item) -> Option<String> {
        Some(match item {
            Item::Unit { unit } => format!("units_of:{unit}"),
            Item::Building { building } => format!("building:{building}"),
            Item::District { district, .. } => {
                format!("district:{}", g.district_family(*district))
            }
            Item::Wonder { .. } => "wonders".to_string(),
            _ => return None,
        })
    }

    /// What `item` is worth toward open chases, less what the rest of the
    /// empire has already queued for the same trigger. A chase needing three
    /// Archers with two already queued elsewhere pays this city for the
    /// third and nothing for a fourth.
    pub(super) fn chase_production_premium(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> f64 {
        let Some(key) = Self::item_trigger_key(g, item) else {
            return 0.0;
        };
        let queued_elsewhere = g
            .cities
            .values()
            .filter(|city| city.owner == pid && city.id != cid)
            .flat_map(|city| city.queue.iter())
            .filter(|queued| Self::item_trigger_key(g, queued).as_deref() == Some(key.as_str()))
            .count() as i64;
        let premium: f64 = self
            .eureka_chases(g, pid)
            .iter()
            .filter(|chase| chase.trigger == key)
            .map(|chase| {
                let left = chase.remaining - queued_elsewhere;
                if left <= 0 {
                    return 0.0;
                }
                chase.per_step()
                    * CHASE_PRODUCTION_VALUE_PER_POINT
                    * CHASE_STEP_DISCOUNT.powi((left - 1) as i32)
            })
            .sum();
        premium.min(CHASE_PRODUCTION_VALUE_CAP)
    }

    /// What a kill by `uid` on `target` earns in open boosts: `kill_with:` the
    /// attacker's own kind, `kills`, `barbs_killed` against the Barbarian
    /// seat, and `camps` for a defender standing on a camp. Zero with the gene
    /// off. The caller adds this only to a kill the exact evaluation says the
    /// attacker survives.
    pub(super) fn chase_kill_premium(&self, g: &Game, pid: usize, uid: u32, target: Pos) -> f64 {
        if !self.chase_every_boost {
            return 0.0;
        }
        let Some(attacker) = g.units.get(&uid) else {
            return 0.0;
        };
        let kill_with = format!("kill_with:{}", attacker.kind);
        let barbarian = g
            .unit_ids_at(target)
            .iter()
            .any(|other| g.players[g.units[other].owner].is_barbarian);
        let camp = g
            .map
            .get(target)
            .is_some_and(|tile| tile.improvement.as_deref() == Some("barbarian_camp"));
        let premium: f64 = self
            .eureka_chases(g, pid)
            .iter()
            .filter(|chase| {
                let trigger = chase.trigger.as_str();
                trigger == kill_with
                    || trigger == "kills"
                    || (barbarian && trigger == "barbs_killed")
                    || (camp && trigger == "camps")
            })
            .map(|chase| chase.per_step() * CHASE_KILL_VALUE_PER_POINT)
            .sum();
        premium.min(CHASE_KILL_VALUE_CAP)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::opt_in_off_in_both_controllers;
    use super::super::AdvancedAi;
    use super::*;
    use crate::game::{Action, Game};
    use crate::name;

    #[test]
    fn chase_every_boost_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("chase-every-boost", |ai| ai.chase_every_boost);
    }

    /// One founded capital and nothing else on the board.
    fn capital_board(seed: u64) -> Game {
        let mut game = Game::new_full(1, 20, 14, seed, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| game.units[uid].kind == "settler")
            .expect("the player opens with a settler");
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        for uid in game.player_unit_ids(0) {
            game.remove_unit(uid);
        }
        game
    }

    /// A second city of ours, founded on the first legal site four or more
    /// tiles from the capital.
    fn second_city(game: &mut Game) -> u32 {
        let capital = game.cities[&game.player_city_ids(0)[0]].pos;
        let sites: Vec<Pos> = game.map.tiles.keys().copied().collect();
        for site in sites {
            if game.wdist(site, capital) < 4 {
                continue;
            }
            let settler = game.spawn_unit("settler", 0, site);
            if game.apply(0, &Action::FoundCity { unit: settler }).is_ok() {
                return *game
                    .player_city_ids(0)
                    .iter()
                    .find(|cid| game.cities[cid].pos == site)
                    .expect("the city stands where it was founded");
            }
            game.remove_unit(settler);
        }
        panic!("no legal second site on the board");
    }

    fn armed() -> AdvancedAi {
        let mut ai = AdvancedAi::new();
        ai.enable_chase_every_boost();
        ai
    }

    fn spec(game: &Game, node: &str, techs: bool) -> crate::rules::BoostSpec {
        let boost = if techs {
            game.rules.techs[node].boost.clone()
        } else {
            game.rules.civics[node].boost.clone()
        };
        boost.expect("the node carries a boost row")
    }

    #[test]
    fn boost_progress_is_the_engines_own_met_test_for_every_row() {
        let game = capital_board(57_001);
        let rows: Vec<(String, crate::rules::BoostSpec)> = game
            .rules
            .techs
            .iter()
            .filter_map(|(n, s)| s.boost.clone().map(|b| (n.to_string(), b)))
            .chain(
                game.rules
                    .civics
                    .iter()
                    .filter_map(|(n, s)| s.boost.clone().map(|b| (n.to_string(), b))),
            )
            .collect();
        assert!(
            rows.len() > 100,
            "both trees carry boost rows: {}",
            rows.len()
        );
        for (node, boost) in rows {
            let (have, need) = game.boost_progress(0, &boost);
            assert!(need >= 1, "{node}: need {need}");
            assert_eq!(
                game.boost_met(0, &boost),
                have >= need,
                "{node} ({}) met must be have >= need: {have} >= {need}",
                boost.trigger
            );
        }
    }

    #[test]
    fn boost_progress_counts_the_state_the_planner_can_change() {
        let mut game = capital_board(57_002);
        // Early Empire: six citizens across the empire, one city of one.
        let (have, need) = game.boost_progress(0, &spec(&game, "early_empire", false));
        assert_eq!((have, need), (1, 6));
        // Guilds: two Markets, none standing.
        assert_eq!(
            game.boost_progress(0, &spec(&game, "guilds", false)),
            (0, 2)
        );
        // Political Philosophy: three city-states met, none on a lone board.
        assert_eq!(
            game.boost_progress(0, &spec(&game, "political_philosophy", false)),
            (0, 3)
        );
        // Archery: a Slinger kill the counters have not recorded.
        assert_eq!(
            game.boost_progress(0, &spec(&game, "archery", true)),
            (0, 1)
        );
        // A yes/no trigger is (0 or 1, 1): Sailing wants a coastal city.
        let (have, need) = game.boost_progress(0, &spec(&game, "sailing", true));
        assert_eq!(need, 1);
        assert!(have == 0 || have == 1);
        // Bronze Working: three barbarians, and the counter moves the count.
        game.players[0]
            .counters
            .insert("barbs_killed".to_string(), 2);
        assert_eq!(
            game.boost_progress(0, &spec(&game, "bronze_working", true)),
            (2, 3)
        );
    }

    #[test]
    fn off_the_chase_table_knows_only_the_six_buildable_prefixes() {
        let game = capital_board(57_003);
        let plain = AdvancedAi::new();
        let chases = plain.eureka_chases(&game, 0);
        assert!(chases.iter().all(|chase| {
            let t = chase.trigger.as_str();
            t.starts_with("improvement")
                || t.starts_with("improve_resource:")
                || t.starts_with("units_of:")
                || t.starts_with("building:")
                || t.starts_with("district:")
        }));
        assert!(!chases.iter().any(|chase| chase.node == name!("archery")));
        assert!(!chases
            .iter()
            .any(|chase| chase.node == name!("political_philosophy")));
    }

    #[test]
    fn on_every_trigger_in_reach_is_a_chase_in_both_trees() {
        let game = capital_board(57_004);
        let chases = armed().eureka_chases(&game, 0);
        let chase = |node: &str| {
            chases
                .iter()
                .find(|chase| chase.node.as_str() == node)
                .unwrap_or_else(|| panic!("{node} is chased"))
                .clone()
        };
        // A kill trigger the counters cannot yet show: one step open.
        assert_eq!(chase("archery").remaining, 1);
        assert_eq!(chase("bronze_working").remaining, 3);
        // Inspirations from contacts, wonders, districts and population.
        assert_eq!(chase("political_philosophy").remaining, 3);
        assert_eq!(chase("drama_poetry").remaining, 1);
        assert_eq!(chase("military_training").remaining, 1);
        assert_eq!(chase("early_empire").remaining, 5);
        // The old buildable prefixes are still there.
        assert_eq!(chase("guilds").remaining, 2);
        assert_eq!(chase("masonry").remaining, 1);
        // Research is the boost's own fraction of the node's cost.
        let guilds = chase("guilds");
        assert!((guilds.research - game.game_speed.scale(420.0) * 0.4).abs() < 1e-9);
    }

    #[test]
    fn a_banked_boost_drops_out_of_the_chase_table() {
        let mut game = capital_board(57_005);
        game.players[0].boosted_civics.insert(name!("drama_poetry"));
        game.players[0].boosted_techs.insert(name!("archery"));
        let chases = armed().eureka_chases(&game, 0);
        assert!(!chases
            .iter()
            .any(|chase| chase.node == name!("drama_poetry")));
        assert!(!chases.iter().any(|chase| chase.node == name!("archery")));
    }

    #[test]
    fn a_kill_trigger_is_actionable_only_with_the_named_unit_in_hand() {
        let mut game = capital_board(57_006);
        let ai = armed();
        let archery = ai
            .chase_for(&game, 0, "archery")
            .expect("archery is chased");
        assert!(
            !ai.chase_one_action_away(&game, 0, &archery),
            "no Slinger, no action"
        );
        let capital = game.cities[&game.player_city_ids(0)[0]].pos;
        game.spawn_unit("slinger", 0, capital);
        assert!(ai.chase_one_action_away(&game, 0, &archery));
        // Growth is not an action: Early Empire is never "one step away".
        let early = ai
            .chase_for(&game, 0, "early_empire")
            .expect("early empire is chased");
        assert!(!ai.chase_actionable(&game, 0, &early));
    }

    #[test]
    fn the_beeline_walks_the_boosted_prerequisite_first() {
        let mut game = capital_board(57_007);
        let plain = AdvancedAi::new();
        let ai = armed();
        let printed = game.rules.techs["masonry"].cost;
        assert_eq!(plain.beeline_step_cost(&game, 0, "masonry", true), printed);
        assert_eq!(ai.beeline_step_cost(&game, 0, "masonry", true), printed);
        game.players[0].boosted_techs.insert(name!("masonry"));
        assert_eq!(
            plain.beeline_step_cost(&game, 0, "masonry", true),
            printed,
            "off, the beeline is blind to the boost"
        );
        assert!((ai.beeline_step_cost(&game, 0, "masonry", true) - printed * 0.6).abs() < 1e-9);
        // The civic half through the same code path.
        let civic = game.rules.civics["drama_poetry"].cost;
        game.players[0].boosted_civics.insert(name!("drama_poetry"));
        assert!((ai.beeline_step_cost(&game, 0, "drama_poetry", false) - civic * 0.6).abs() < 1e-9);
        assert_eq!(
            plain.beeline_step_cost(&game, 0, "drama_poetry", false),
            civic
        );
    }

    #[test]
    fn a_boost_one_action_away_halves_its_discount_on_the_beeline() {
        let mut game = capital_board(57_008);
        let ai = armed();
        let capital = game.cities[&game.player_city_ids(0)[0]].pos;
        game.spawn_unit("slinger", 0, capital);
        let printed = game.rules.techs["archery"].cost;
        let cost = ai.beeline_step_cost(&game, 0, "archery", true);
        assert!((cost - printed * (1.0 - 0.4 * CHASE_ONE_STEP_FRACTION)).abs() < 1e-9);
    }

    #[test]
    fn production_pays_an_inspiration_trigger_and_stops_when_others_have_it_queued() {
        let mut game = capital_board(57_009);
        let ai = armed();
        let cid = game.player_city_ids(0)[0];
        let encampment = Item::District {
            district: name!("encampment"),
            pos: game.cities[&cid].pos,
        };
        let market = Item::Building {
            building: name!("market"),
        };
        assert!(
            ai.chase_production_premium(&game, 0, cid, &encampment) > 0.0,
            "Military Training's Encampment is priced"
        );
        assert!(
            ai.chase_production_premium(&game, 0, cid, &market) > 0.0,
            "Guilds' Market is priced"
        );
        assert_eq!(
            AdvancedAi::new().production_boost_premium(&game, 0, cid, &market, 100.0),
            0.0,
            "off, the premium is the old genes' zero"
        );
        // A second city with two Markets queued exhausts the two-Market chase.
        let second = second_city(&mut game);
        game.cities
            .get_mut(&second)
            .unwrap()
            .queue
            .push(market.clone());
        game.cities
            .get_mut(&second)
            .unwrap()
            .queue
            .push(market.clone());
        assert_eq!(ai.chase_production_premium(&game, 0, cid, &market), 0.0);
        // With one queued elsewhere, this city is paid for the second, at
        // the full step (nothing left after it).
        game.cities.get_mut(&second).unwrap().queue.pop();
        let one_left = ai.chase_production_premium(&game, 0, cid, &market);
        game.cities.get_mut(&second).unwrap().queue.clear();
        let two_left = ai.chase_production_premium(&game, 0, cid, &market);
        assert!(
            one_left > two_left,
            "{one_left} > {two_left}: the further step is discounted"
        );
    }

    #[test]
    fn the_premiums_never_exceed_half_the_items_own_value() {
        let game = capital_board(57_013);
        let ai = armed();
        let cid = game.player_city_ids(0)[0];
        let market = Item::Building {
            building: name!("market"),
        };
        let open = ai.chase_production_premium(&game, 0, cid, &market);
        assert!(open > 0.0);
        // A marginal item (raw 10) can gain at most 5; a strong one keeps
        // the whole premium, under the absolute cap.
        assert!(
            (ai.production_boost_premium(&game, 0, cid, &market, 10.0) - open.min(5.0)).abs()
                < 1e-9
        );
        assert!((ai.production_boost_premium(&game, 0, cid, &market, 1_000.0) - open).abs() < 1e-9);
        assert!(open <= CHASE_PRODUCTION_VALUE_CAP);
        // A refused item (raw below zero) gains nothing.
        assert_eq!(
            ai.production_boost_premium(&game, 0, cid, &market, -5.0),
            0.0
        );
        // The builder premium under this gene alone is held the same way.
        let capital = game.cities[&cid].pos;
        let tile = game.cities[&cid]
            .owned_tiles
            .iter()
            .copied()
            .find(|pos| *pos != capital)
            .expect("the capital owns a work tile");
        let raw = ai.eureka_builder_premium(&game, tile, "farm");
        let held = ai.builder_boost_premium(&game, tile, "farm", 4.0);
        assert!(held <= raw && held <= 2.0);
        let mut both = armed();
        both.enable_eureka_chasing_builder();
        assert!((both.builder_boost_premium(&game, tile, "farm", 4.0) - raw).abs() < 1e-9);
    }

    #[test]
    fn a_wonder_is_worth_the_inspiration_it_completes() {
        let game = capital_board(57_010);
        let ai = armed();
        let cid = game.player_city_ids(0)[0];
        let wonder = Item::Wonder {
            wonder: name!("pyramids"),
            pos: game.cities[&cid].pos,
        };
        assert!(ai.chase_production_premium(&game, 0, cid, &wonder) > 0.0);
        assert_eq!(
            AdvancedAi::new().production_boost_premium(&game, 0, cid, &wonder, 100.0),
            0.0
        );
    }

    #[test]
    fn a_slinger_kill_on_a_barbarian_earns_archery_and_bronze_working() {
        // A barbarian-seated board, emptied of every unit and camp.
        let mut game = Game::new_full(1, 20, 14, 57_011, 200, 0, true);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| game.units[uid].kind == "settler")
            .expect("the player opens with a settler");
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        for uid in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(uid);
        }
        game.barb_camps.clear();
        let ai = armed();
        let capital = game.cities[&game.player_city_ids(0)[0]].pos;
        let slinger = game.spawn_unit("slinger", 0, capital);
        let warrior = game.spawn_unit("warrior", 0, capital);
        let barbarian = game.barb_pid.expect("a barbarian seat");
        let camp = (capital.0 + 3, capital.1);
        game.spawn_unit("warrior", barbarian, camp);
        let slinger_pays = ai.chase_kill_premium(&game, 0, slinger, camp);
        let warrior_pays = ai.chase_kill_premium(&game, 0, warrior, camp);
        assert!(
            slinger_pays > warrior_pays,
            "{slinger_pays} > {warrior_pays}"
        );
        assert!(
            warrior_pays > 0.0,
            "Bronze Working's three barbarians pay any kill"
        );
        assert!(slinger_pays <= CHASE_KILL_VALUE_CAP);
        assert_eq!(
            AdvancedAi::new().chase_kill_premium(&game, 0, slinger, camp),
            0.0
        );
        // Archery banked: the Slinger's edge is gone. A fresh controller, because
        // the chase table is memoised per turn and the board changed under it.
        game.players[0].boosted_techs.insert(name!("archery"));
        let ai = armed();
        let banked = ai.chase_kill_premium(&game, 0, slinger, camp);
        assert!((banked - ai.chase_kill_premium(&game, 0, warrior, camp)).abs() < 1e-9);
    }

    /// Diagnostic, never in CI: one screen-shaped game (the probe's own
    /// board) with the gene on seat 0 against the same seed with it off,
    /// printing the boosted share and every unboosted node's trigger, plus
    /// the chase table at three turns. `cargo test --profile ci --lib --
    /// --ignored diagnostic_boost_share --nocapture`.
    #[test]
    #[ignore]
    fn diagnostic_boost_share_on_vs_off() {
        use crate::ai::Ai;
        use crate::game::GameOptions;
        use crate::setup::MapScript;
        let seed: u64 = std::env::var("CIVVIS_DIAG_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(26090140);
        for on in [false, true] {
            if !on && std::env::var("CIVVIS_DIAG_SKIP_OFF").is_ok() {
                continue;
            }
            let mut world = Game::new_with(GameOptions {
                speed: "online".to_string(),
                map_script: MapScript::Continents,
                difficulty: "prince".to_string(),
                ..GameOptions::new(6, 74, 46, seed, 250, 9)
            });
            let mut ais: Vec<AdvancedAi> = (0..world.players.len())
                .map(|pid| {
                    let mut ai = AdvancedAi::new();
                    if on && pid == 0 {
                        ai.enable_chase_every_boost();
                    }
                    ai
                })
                .collect();
            world.set_fog_memory(false);
            world.set_war_ledger(false);
            let mut reported = std::collections::BTreeSet::new();
            while world.winner.is_none() && world.turn <= world.max_turns {
                let pid = world.current;
                if pid == 0
                    && [60u32, 120, 180].contains(&world.turn)
                    && !reported.contains(&world.turn)
                {
                    reported.insert(world.turn);
                    let chases = ais[0].eureka_chases(&world, 0);
                    let mut brief: Vec<String> = chases
                        .iter()
                        .map(|c| format!("{}[{} left {}]", c.node, c.trigger, c.remaining))
                        .collect();
                    brief.sort();
                    if std::env::var("CIVVIS_DIAG_PRODUCTION").is_ok() {
                        let cid = world.player_city_ids(0)[0];
                        let plan =
                            ais[0]
                                .current_plan()
                                .cloned()
                                .unwrap_or(super::super::StrategicPlan {
                                    strategy: super::super::GrandStrategy::Science,
                                    target_player: None,
                                    target_city: None,
                                    threatened_city: None,
                                    desired_cities: 4,
                                    assessed_turn: world.turn,
                                    rush: false,
                                });
                        let counts = ais[0].counts(&world, 0);
                        let items = world.producible_items(0, cid);
                        let values =
                            ais[0].production_values(&world, 0, cid, &items, &plan, counts);
                        let mut table: Vec<(f64, f64, String)> = items
                            .iter()
                            .zip(values.iter())
                            .map(|(item, value)| {
                                (
                                    *value,
                                    ais[0].chase_production_premium(&world, 0, cid, item),
                                    format!("{item:?}"),
                                )
                            })
                            .collect();
                        table.sort_by(|a, b| b.0.total_cmp(&a.0));
                        let brief: Vec<String> = table
                            .iter()
                            .take(14)
                            .map(|(v, prem, name)| {
                                format!(
                                    "{}={:.0}(+{:.0})",
                                    name.replace("Item::", "").replace(' ', ""),
                                    v,
                                    prem
                                )
                            })
                            .collect();
                        eprintln!("on={on} t{} PRODUCTION {}", world.turn, brief.join(" "));
                    }
                    let p = &world.players[0];
                    eprintln!(
                        "on={on} t{} cities {} score {} techs {} boosted∩ {} | civics {} inspired∩ {} | chases {}: {}",
                        world.turn,
                        world.player_city_ids(0).len(),
                        world.score(0),
                        p.techs.len(),
                        p.techs.iter().filter(|t| p.boosted_techs.contains(t)).count(),
                        p.civics.len(),
                        p.civics.iter().filter(|c| p.boosted_civics.contains(c)).count(),
                        chases.len(),
                        brief.join(" ")
                    );
                }
                ais[pid].take_turn(&mut world, pid);
                if world.winner.is_none() && world.current == pid {
                    let _ = world.apply(pid, &Action::EndTurn);
                }
            }
            world.finish_at_turn_limit();
            let p = &world.players[0];
            let mut unboosted: Vec<String> = p
                .techs
                .iter()
                .filter(|t| !p.boosted_techs.contains(t))
                .map(|t| {
                    let trig = world.rules.techs[t]
                        .boost
                        .as_ref()
                        .map(|b| format!("{}x{}", b.trigger, b.count))
                        .unwrap_or_else(|| "no-boost".into());
                    format!("{t}[{trig}]")
                })
                .collect();
            unboosted.sort();
            let mut unboosted_civics: Vec<String> = p
                .civics
                .iter()
                .filter(|c| !p.boosted_civics.contains(c))
                .map(|c| {
                    let trig = world.rules.civics[c]
                        .boost
                        .as_ref()
                        .map(|b| format!("{}x{}", b.trigger, b.count))
                        .unwrap_or_else(|| "no-boost".into());
                    format!("{c}[{trig}]")
                })
                .collect();
            unboosted_civics.sort();
            eprintln!(
                "on={on} END t{} score {} cities {} techs {} boosted∩ {} civics {} inspired∩ {}",
                world.turn,
                world.score(0),
                world.player_city_ids(0).len(),
                p.techs.len(),
                p.techs
                    .iter()
                    .filter(|t| p.boosted_techs.contains(t))
                    .count(),
                p.civics.len(),
                p.civics
                    .iter()
                    .filter(|c| p.boosted_civics.contains(c))
                    .count(),
            );
            eprintln!("on={on} UNBOOSTED TECHS: {}", unboosted.join(" "));
            eprintln!("on={on} UNINSPIRED CIVICS: {}", unboosted_civics.join(" "));
        }
    }

    #[test]
    fn the_research_hooks_read_the_gene_as_the_union_of_the_old_family() {
        let mut game = capital_board(57_012);
        game.players[0].boosted_techs.insert(name!("masonry"));
        game.players[0]
            .boosted_civics
            .insert(name!("craftsmanship"));
        let plain = AdvancedAi::new();
        let ai = armed();
        assert_eq!(plain.boost_in_hand_scale(&game, 0, "masonry", true), 1.0);
        // The in-hand scale is the one old-family hook the gene does NOT arm:
        // see `boost_in_hand_scale` for the isolation that removed it.
        assert_eq!(ai.boost_in_hand_scale(&game, 0, "masonry", true), 1.0);
        assert!(
            (ai.boost_in_hand_scale(&game, 0, "craftsmanship", false) - 1.0).abs()
                < 1e-9
        );
    }
}
