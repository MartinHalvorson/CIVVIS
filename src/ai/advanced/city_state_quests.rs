//! City-state quests, played for the Envoy they pay (operator, 2026-08-26:
//! *"add a heuristic for trying to complete the city state challenges for an
//! additional envoy"*).
//!
//! ★★★ NOTHING IN THE AGENT HAS EVER READ A QUEST. `src/game/quests.rs` has
//! modelled the eight shipped `Quests.xml` rows since #430 — every met
//! city-state asks each civilization for one thing at a time and pays
//! `envoys_free += 1` when it is done (`Game::check_city_state_quests`) — and
//! `grep city_state_quest src/ai` was empty before this module. The seat
//! collected quest Envoys only by coincidence: it built a district the
//! city-state happened to want, or cleared a camp that happened to be the
//! named one. An Envoy is the resource the repository's own oracle calls the
//! largest subsystem headroom it has found, and eight of them are sitting on
//! the board being asked for by name.
//!
//! Four opt-in genes, one per decision surface that can *already* satisfy a
//! quest and simply does not know it is being asked. Every one is a
//! **reorder, not a spend**: the same Trader, the same queue slot, the same
//! errand-running soldier, priced with the Envoy the city-state has promised
//! for it. That is deliberate — the repository's own screens have twice
//! shown a gene that spends more losing share even when its idea was right.
//!
//! - **`quest-production`** — `production_value` pays the Envoy on the unit a
//!   `train_unit_type` quest names and on any district in the family a
//!   `zone_district_type` quest names. The queue reorders; nothing is added
//!   to it.
//! - **`quest-trade-route`** — `trade_route_destination_value` pays the Envoy
//!   on a city-state city whose outstanding quest for us is
//!   `send_trade_route`. The same Trader goes somewhere slightly different.
//! - **`quest-camp-errand`** — the camp errand (`BasicAi::camp_bounty_target`)
//!   prefers the exact outpost a `clear_barbarian_camp` quest names over a
//!   nearer unnamed one. ⚠ A PREFERENCE, NOT A REACH, and the first probe is
//!   why: an earlier form also walked six tiles beyond the errand's home ring
//!   for a named camp and read **HURTS ** (win −15.8 pp, z −3.71) over 24
//!   games — the one gene of the four that spent rather than reordered, and
//!   the seat paid for the soldiers it sent out of position. The errand's own
//!   radius, war gate, claim ledger and exchange threshold still decide which
//!   camps are eligible; a named camp outside them is not chased.
//! - **`quest-boost`** — a `trigger_tech_boost` or `trigger_civic_boost`
//!   quest is paid by whatever completes that boost, so the Envoy rides on
//!   the same trigger table `eureka-chasing-production` reads
//!   (`advanced/deity_habits.rs`). ⚠ The two genes price *different things*
//!   on the same items and compose: the eureka gene pays the RESEARCH the
//!   boost grants, this one pays the ENVOY the city-state grants, and either
//!   works with the other off.
//!
//! Two quests are deliberately left alone. `convert_capital_to_religion` and
//! `recruit_great_person_class` are reachable only by redirecting a
//! Missionary already scarce enough to have its own genes, or by re-ordering
//! Great People patronage the seat spends Faith on for their own sake; both
//! are a spend, not a reorder, and belong in their own PR with their own
//! probe. `send_trade_route`'s sibling — founding a route we would not
//! otherwise found — is excluded for the same reason.
//!
//! ⭐ WHAT AN ENVOY IS WORTH HERE. Not a new number:
//! [`AdvancedAi::quest_envoy_production_value`] is the seat's existing
//! `ENVOY_PRODUCTION_VALUE` (170, the price the Diplomatic Quarter's envoy
//! already carries, or `DIPLOMATIC_ENVOY_PRODUCTION_VALUE` in the Diplomacy
//! lane), scaled by [`AdvancedAi::quest_city_state_multiplier`] — how much
//! this particular city-state's next Envoy actually buys. An Envoy toward a
//! suzerainty one step away is worth more than the seventh Envoy poured into
//! a city-state already securely ours, and the multiplier says so, mirroring
//! the `needed` arithmetic `advanced_envoys` scores placements with.
//!
//! Off, every path is byte-identical: each entry point returns `0.0`/`false`
//! before reading the board.

use super::{AdvancedAi, GrandStrategy};
use crate::game::{Game, Item};
use crate::name::Name;
use crate::Pos;

/// `quest_*`: a city-state one Envoy short of a suzerainty is the one whose
/// quest is worth crossing the map for; one already secure past its paying
/// tier is not. The multiplier on [`super::ENVOY_PRODUCTION_VALUE`] by the
/// Envoys still needed to lead — the same `needed` the envoy scorer prices
/// placements with.
pub(super) const QUEST_NEEDED_ONE: f64 = 2.0;
/// One step behind that: the quest pays the first of two.
pub(super) const QUEST_NEEDED_TWO: f64 = 1.5;
/// A city-state at the standing three-Envoy floor with nothing invested.
pub(super) const QUEST_NEEDED_THREE: f64 = 1.0;
/// A suzerainty this far out is not what the quest is worth chasing for; the
/// Envoy is still fungible, so this is a floor rather than a refusal.
pub(super) const QUEST_NEEDED_FAR: f64 = 0.6;
/// A city-state already ours by more than the tie margin, past the tier its
/// yields step at: its next Envoy buys almost nothing, and the quest reward
/// is worth only what banking it is worth.
pub(super) const QUEST_ALREADY_SECURE: f64 = 0.25;
/// Envoys at which a secure city-state counts as padded — the tier ladder
/// (1/3/6) has no step left above it. Mirrors `bank_envoys`' own reading.
pub(super) const QUEST_PADDED_ENVOYS: i64 = 6;

/// `quest_production`: the ceiling on one item's quest premium, so a quest
/// can reorder a queue but never dominate it. A district is worth thousands
/// raw before the turns divisor; a Warrior is worth tens.
pub(super) const QUEST_PRODUCTION_CAP: f64 = 400.0;
/// `quest_trade_route`: the destination scale is yields-per-turn weighted
/// (a good route is ~30–60, the alliance premium 45), not the production
/// scale, so the Envoy is priced against it at this fraction.
pub(super) const QUEST_TRADE_ROUTE_SCALE: f64 = 0.25;

/// Every city-state currently asking `pid` for `kind`, with its quest. The
/// engine stores one quest per pair, so this is at most one row per met
/// city-state, and a city-state that has died holds none.
fn quests_of_kind<'a>(
    g: &'a Game,
    pid: usize,
    kind: &str,
) -> Vec<(usize, &'a crate::game::quests::CityStateQuest)> {
    let Some(player) = g.players.get(pid) else {
        return Vec::new();
    };
    player
        .quests
        .iter()
        .filter(|(_, quest)| quest.kind == kind)
        .filter(|(minor, _)| {
            g.players
                .get(**minor)
                .is_some_and(|state| state.alive && state.is_minor && !state.is_barbarian)
        })
        .map(|(minor, quest)| (*minor, quest))
        .collect()
}

impl AdvancedAi {
    /// What this city-state's next Envoy is worth, as a multiple of the
    /// seat's standing Envoy price. See the module header.
    ///
    /// Mirrors `advanced_envoys`' `needed` arithmetic: the Envoys between us
    /// and a strict lead over every other major, floored at Civilization VI's
    /// three-Envoy suzerainty threshold.
    pub(super) fn quest_city_state_multiplier(&self, g: &Game, pid: usize, minor: usize) -> f64 {
        let mine = g.envoys_at(pid, minor);
        let rival = g
            .players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian && player.id != pid)
            .map(|player| g.envoys_at(player.id, minor))
            .max()
            .unwrap_or(0);
        if g.suzerain_of(minor) == Some(pid) && mine > rival + 1 && mine >= QUEST_PADDED_ENVOYS {
            return QUEST_ALREADY_SECURE;
        }
        match (3_i64.max(rival + 1) - mine).max(1) {
            1 => QUEST_NEEDED_ONE,
            2 => QUEST_NEEDED_TWO,
            3 => QUEST_NEEDED_THREE,
            _ => QUEST_NEEDED_FAR,
        }
    }

    /// The Envoy a quest pays, on `production_value`'s raw scale, for the
    /// city-state that is asking. The seat's own Envoy price, scaled by what
    /// this city-state's next Envoy buys.
    pub(super) fn quest_envoy_production_value(
        &self,
        g: &Game,
        pid: usize,
        minor: usize,
        strategy: GrandStrategy,
    ) -> f64 {
        let envoy = match strategy {
            GrandStrategy::Diplomacy => super::DIPLOMATIC_ENVOY_PRODUCTION_VALUE,
            _ => super::ENVOY_PRODUCTION_VALUE,
        };
        envoy * self.quest_city_state_multiplier(g, pid, minor)
    }

    /// `quest_production`: the Envoy on an item a city-state is asking for by
    /// name — the unit of a `train_unit_type` quest, or any district in the
    /// family of a `zone_district_type` quest. Summed over the city-states
    /// asking for the same thing (two of them wanting a Campus pay twice) and
    /// capped at [`QUEST_PRODUCTION_CAP`].
    ///
    /// `0.0` with the gene off, and for every other item.
    pub(super) fn quest_production_premium(
        &self,
        g: &Game,
        pid: usize,
        item: &Item,
        strategy: GrandStrategy,
    ) -> f64 {
        if !self.quest_production {
            return 0.0;
        }
        let (kind, target) = match item {
            // A Corps or an Army is that unit trained: the quest reads the
            // units on the board, not how they were packaged.
            Item::Unit { unit } | Item::Formation { unit, .. } => {
                ("train_unit_type", unit.to_string())
            }
            Item::District { district, .. } => (
                "zone_district_type",
                g.district_family(*district).to_string(),
            ),
            _ => return 0.0,
        };
        let premium: f64 = quests_of_kind(g, pid, kind)
            .into_iter()
            .filter(|(_, quest)| quest.target == target)
            .map(|(minor, _)| self.quest_envoy_production_value(g, pid, minor, strategy))
            .sum();
        premium.min(QUEST_PRODUCTION_CAP)
    }

    /// `quest_trade_route`: the Envoy on a destination whose owner is asking
    /// us for a trade route. `0.0` with the gene off and for every city that
    /// is not a city-state asking for one.
    ///
    /// The quest is satisfied by *a* route to any of that city-state's
    /// cities, so the premium rides on each of them and the ordinary yield
    /// terms decide which.
    pub(super) fn quest_trade_route_premium(
        &self,
        g: &Game,
        pid: usize,
        owner: usize,
        strategy: GrandStrategy,
    ) -> f64 {
        if !self.quest_trade_route {
            return 0.0;
        }
        if !quests_of_kind(g, pid, "send_trade_route")
            .iter()
            .any(|(minor, _)| *minor == owner)
        {
            return 0.0;
        }
        self.quest_envoy_production_value(g, pid, owner, strategy) * QUEST_TRADE_ROUTE_SCALE
    }

    /// `quest_boost`: the Envoy on an item that completes the boost a
    /// `trigger_tech_boost` or `trigger_civic_boost` quest names.
    ///
    /// ⚠ This is the ENVOY, not the research. `eureka_chasing_production`
    /// prices the same items by the research their boost grants; the two are
    /// independent genes on one trigger table and compose additively.
    /// Off, or with no quest naming a node this item triggers, `0.0`.
    pub(super) fn quest_boost_premium(
        &self,
        g: &Game,
        pid: usize,
        item: &Item,
        strategy: GrandStrategy,
    ) -> f64 {
        if !self.quest_boost {
            return 0.0;
        }
        let key = match item {
            Item::Unit { unit } | Item::Formation { unit, .. } => format!("units_of:{unit}"),
            Item::Building { building } => format!("building:{building}"),
            Item::District { district, .. } => {
                format!("district:{}", g.district_family(*district))
            }
            _ => return 0.0,
        };
        self.quest_boost_premium_for_trigger(g, pid, &key, strategy)
    }

    /// `quest_boost`, on the Builder's scale: the Envoy on an improvement
    /// that completes a quest-named boost. The owning city supplies the
    /// civilization, exactly as `eureka_builder_premium` does, and the
    /// trigger vocabulary is that function's — `improvement:`,
    /// `improvement_on_resource:` and `improve_resource:`.
    pub(super) fn quest_boost_builder_premium(
        &self,
        g: &Game,
        pos: Pos,
        improvement: &str,
        strategy: GrandStrategy,
    ) -> f64 {
        if !self.quest_boost {
            return 0.0;
        }
        let tile = &g.map.tiles[&pos];
        let Some(pid) = tile
            .owner_city
            .and_then(|cid| g.cities.get(&cid))
            .map(|city| city.owner)
        else {
            return 0.0;
        };
        let asked = Self::quest_boost_nodes(g, pid);
        if asked.is_empty() {
            return 0.0;
        }
        let premium: f64 = self
            .eureka_chases(g, pid)
            .iter()
            .filter(|chase| {
                if let Some(named) = chase.trigger.strip_prefix("improvement:") {
                    named == improvement
                } else if let Some(named) = chase.trigger.strip_prefix("improvement_on_resource:") {
                    named == improvement && tile.resource.is_some()
                } else if let Some(resource) = chase.trigger.strip_prefix("improve_resource:") {
                    tile.resource.as_deref() == Some(resource)
                        && Self::improvement_connects(g, improvement, resource)
                } else {
                    false
                }
            })
            .filter_map(|chase| {
                let minor = asked
                    .iter()
                    .find(|(_, node)| *node == chase.node)
                    .map(|(minor, _)| *minor)?;
                Some(
                    self.quest_envoy_production_value(g, pid, minor, strategy)
                        / chase.remaining.max(1) as f64,
                )
            })
            .sum();
        premium.min(QUEST_PRODUCTION_CAP)
    }

    /// The city-states asking for a Eureka or an Inspiration, and the node
    /// each one named.
    fn quest_boost_nodes(g: &Game, pid: usize) -> Vec<(usize, Name)> {
        ["trigger_tech_boost", "trigger_civic_boost"]
            .into_iter()
            .flat_map(|kind| quests_of_kind(g, pid, kind))
            .map(|(minor, quest)| (minor, Name::new(&quest.target)))
            .collect()
    }

    /// The shared half of the item-side `quest_boost`: the Envoys owed by
    /// city-states whose named node this trigger still completes, each spread
    /// over the steps the trigger has left.
    fn quest_boost_premium_for_trigger(
        &self,
        g: &Game,
        pid: usize,
        trigger: &str,
        strategy: GrandStrategy,
    ) -> f64 {
        let asked = Self::quest_boost_nodes(g, pid);
        if asked.is_empty() {
            return 0.0;
        }
        let chases = self.eureka_chases(g, pid);
        asked
            .into_iter()
            .filter_map(|(minor, node)| {
                let chase = chases
                    .iter()
                    .find(|chase| chase.node == node && chase.trigger == trigger)?;
                // One step of several earns a fraction of the Envoy: three
                // Archers for Machinery are one Envoy for the third, not one
                // apiece.
                Some(
                    self.quest_envoy_production_value(g, pid, minor, strategy)
                        / chase.remaining.max(1) as f64,
                )
            })
            .sum::<f64>()
            .min(QUEST_PRODUCTION_CAP)
    }
}

impl crate::ai::BasicAi {
    /// `quest_camp_errand`: is this outpost the one a city-state named, and
    /// is it therefore worth the errand? `BasicAi::camp_bounty_target` reads
    /// this to prefer a named camp over a nearer unnamed one and to reach a
    /// little further for it. `false` with the gene off, and for minors and
    /// barbarians, which hold no quests.
    ///
    /// Civilization VI names ONE outpost within five tiles of the city-state
    /// and pays only for that one (`Game::quest_done`'s `clear_barbarian_camp`
    /// arm checks the named position and that we are the civilization whose
    /// camp counter moved), so "clear a camp" is not the errand — clearing
    /// *this* camp is.
    pub(crate) fn quest_camp_is_named(&self, g: &Game, pid: usize, camp: Pos) -> bool {
        self.quest_camp_errand
            && !self.minor
            && !self.barb
            && quests_of_kind(g, pid, "clear_barbarian_camp")
                .iter()
                .any(|(_, quest)| quest.pos == Some(camp))
    }
}

#[cfg(test)]
mod tests {

    use super::super::test_support::opt_in_off_in_both_controllers;
    use super::*;
    use crate::game::Game;
    use crate::name;

    #[test]
    fn quest_production_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("quest-production", |ai| ai.quest_production);
    }

    #[test]
    fn quest_trade_route_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("quest-trade-route", |ai| ai.quest_trade_route);
    }

    #[test]
    fn quest_camp_errand_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("quest-camp-errand", |ai| ai.base.quest_camp_errand);
    }

    #[test]
    fn quest_boost_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("quest-boost", |ai| ai.quest_boost);
    }

    /// A board with one met city-state, and the quest it is asking for
    /// replaced by the one under test.
    fn asked(seed: u64, quest: crate::game::quests::CityStateQuest) -> (Game, usize) {
        let mut g = Game::new(2, 24, 16, seed, 80, 4);
        let minor = g
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .expect("the fixture seats a city-state");
        g.record_contact(0, minor);
        g.players[0].quests.insert(minor, quest);
        (g, minor)
    }

    fn quest(kind: &str, target: &str) -> crate::game::quests::CityStateQuest {
        crate::game::quests::CityStateQuest {
            kind: kind.to_string(),
            target: target.to_string(),
            era: 0,
            pos: None,
            mark: 0,
        }
    }

    /// ★ The premium is the seat's own Envoy price, and it is paid only for
    /// the thing the city-state actually named.
    #[test]
    fn quest_production_pays_the_envoy_for_the_named_unit_and_nothing_else() {
        let (g, _) = asked(11, quest("train_unit_type", "archer"));
        let mut ai = AdvancedAi::new();
        let archer = Item::Unit {
            unit: name!("archer"),
        };
        let warrior = Item::Unit {
            unit: name!("warrior"),
        };
        assert_eq!(
            ai.quest_production_premium(&g, 0, &archer, GrandStrategy::Science),
            0.0,
            "off, the queue never sees the quest"
        );
        ai.enable_quest_production();
        let paid = ai.quest_production_premium(&g, 0, &archer, GrandStrategy::Science);
        assert!(paid > 0.0, "the named unit carries the Envoy: {paid}");
        assert_eq!(
            ai.quest_production_premium(&g, 0, &warrior, GrandStrategy::Science),
            0.0,
            "a unit nobody asked for carries nothing"
        );
        assert!(paid <= QUEST_PRODUCTION_CAP);
    }

    /// A Corps of the named unit is the named unit trained: `quest_done`
    /// reads the units on the board, so the queue must price the formation
    /// the same way.
    #[test]
    fn quest_production_pays_a_formation_of_the_named_unit() {
        let (g, _) = asked(12, quest("train_unit_type", "archer"));
        let mut ai = AdvancedAi::new();
        ai.enable_quest_production();
        let unit = ai.quest_production_premium(
            &g,
            0,
            &Item::Unit {
                unit: name!("archer"),
            },
            GrandStrategy::Science,
        );
        let corps = ai.quest_production_premium(
            &g,
            0,
            &Item::Formation {
                unit: name!("archer"),
                formation: 1,
            },
            GrandStrategy::Science,
        );
        assert_eq!(unit, corps);
    }

    /// A district quest names a FAMILY, so any member of it pays.
    #[test]
    fn quest_production_pays_any_district_in_the_named_family() {
        let (g, _) = asked(13, quest("zone_district_type", "campus"));
        let mut ai = AdvancedAi::new();
        ai.enable_quest_production();
        let campus = Item::District {
            district: name!("campus"),
            pos: (0, 0),
        };
        let holy = Item::District {
            district: name!("holy_site"),
            pos: (0, 0),
        };
        assert!(ai.quest_production_premium(&g, 0, &campus, GrandStrategy::Science) > 0.0);
        assert_eq!(
            ai.quest_production_premium(&g, 0, &holy, GrandStrategy::Science),
            0.0
        );
    }

    /// ★★ The Envoy is worth what the city-state's next Envoy buys: a
    /// suzerainty one step away outprices one already padded.
    #[test]
    fn the_quest_envoy_is_priced_by_what_the_city_states_next_envoy_buys() {
        let (mut g, minor) = asked(14, quest("train_unit_type", "archer"));
        let mut ai = AdvancedAi::new();
        ai.enable_quest_production();
        let item = Item::Unit {
            unit: name!("archer"),
        };
        // Two envoys in: one more takes the suzerainty.
        g.players[0].envoys = vec![(minor, 2)];
        let close = ai.quest_production_premium(&g, 0, &item, GrandStrategy::Science);
        // Padded well past the last tier, and already ours.
        g.players[0].envoys = vec![(minor, 8)];
        let padded = ai.quest_production_premium(&g, 0, &item, GrandStrategy::Science);
        assert!(
            close > padded,
            "an Envoy that finishes a suzerainty must outprice the ninth poured \
             into a secure one: close {close}, padded {padded}"
        );
        assert!(padded > 0.0, "the Envoy is still fungible");
        assert_eq!(
            ai.quest_city_state_multiplier(&g, 0, minor),
            QUEST_ALREADY_SECURE
        );
    }

    /// The Diplomacy lane already prices an Envoy higher, and the quest
    /// premium follows the seat's own price rather than inventing one.
    #[test]
    fn the_diplomacy_lane_pays_its_own_envoy_price() {
        let (g, _) = asked(15, quest("train_unit_type", "archer"));
        let mut ai = AdvancedAi::new();
        ai.enable_quest_production();
        let item = Item::Unit {
            unit: name!("archer"),
        };
        assert!(
            ai.quest_production_premium(&g, 0, &item, GrandStrategy::Diplomacy)
                > ai.quest_production_premium(&g, 0, &item, GrandStrategy::Science),
            "the Diplomacy lane's Envoy is the dearer one"
        );
    }

    #[test]
    fn quest_trade_route_pays_only_the_city_state_asking_for_a_route() {
        let (g, minor) = asked(16, quest("send_trade_route", ""));
        let mut ai = AdvancedAi::new();
        assert_eq!(
            ai.quest_trade_route_premium(&g, 0, minor, GrandStrategy::Science),
            0.0
        );
        ai.enable_quest_trade_route();
        assert!(ai.quest_trade_route_premium(&g, 0, minor, GrandStrategy::Science) > 0.0);
        let other = g
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian && player.id != minor)
            .map(|player| player.id);
        if let Some(other) = other {
            assert_eq!(
                ai.quest_trade_route_premium(&g, 0, other, GrandStrategy::Science),
                0.0,
                "a city-state asking for something else pays nothing"
            );
        }
    }

    #[test]
    fn quest_camp_errand_names_the_exact_outpost() {
        let camp = (7, 9);
        let mut asked_quest = quest("clear_barbarian_camp", "");
        asked_quest.pos = Some(camp);
        let (g, _) = asked(17, asked_quest);
        let mut ai = AdvancedAi::new();
        assert!(
            !ai.base.quest_camp_is_named(&g, 0, camp),
            "off, no camp is named"
        );
        ai.enable_quest_camp_errand();
        assert!(ai.base.quest_camp_is_named(&g, 0, camp));
        assert!(
            !ai.base.quest_camp_is_named(&g, 0, (8, 9)),
            "clearing a different camp does not finish this quest, so it is not the errand"
        );
    }

    /// A quest naming a node no trigger of ours completes pays nothing, and
    /// the gene never invents a chase the boost table does not have.
    #[test]
    fn quest_boost_pays_nothing_for_a_node_no_item_triggers() {
        let (g, _) = asked(18, quest("trigger_tech_boost", "no_such_tech"));
        let mut ai = AdvancedAi::new();
        ai.enable_quest_boost();
        assert_eq!(
            ai.quest_boost_premium(
                &g,
                0,
                &Item::Unit {
                    unit: name!("archer")
                },
                GrandStrategy::Science
            ),
            0.0
        );
    }

    /// ★ The two boost genes are independent prices on one table: the quest
    /// Envoy is paid with `eureka-chasing-production` off.
    #[test]
    fn quest_boost_pays_the_envoy_with_the_eureka_gene_off() {
        let mut g = Game::new(2, 24, 16, 19, 80, 4);
        let minor = g
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .expect("a city-state");
        g.record_contact(0, minor);
        let mut ai = AdvancedAi::new();
        ai.enable_quest_boost();
        // Whatever the archer trigger completes on this board, ask for it.
        let chase = ai
            .eureka_chases(&g, 0)
            .into_iter()
            .find(|chase| chase.trigger == "units_of:archer");
        let Some(chase) = chase else {
            return; // no archer boost on this ruleset; the other tests cover the path
        };
        g.players[0]
            .quests
            .insert(minor, quest("trigger_tech_boost", chase.node.as_ref()));
        assert!(!ai.eureka_chasing_production, "the eureka gene stays off");
        let paid = ai.quest_boost_premium(
            &g,
            0,
            &Item::Unit {
                unit: name!("archer"),
            },
            GrandStrategy::Science,
        );
        assert!(
            paid > 0.0,
            "the Envoy rides on the trigger whether or not the research gene is on"
        );
    }

    /// A board with two Barbarian Outposts, the nearer one unnamed and the
    /// farther one named by a city-state's quest.
    fn two_camps(seed: u64) -> (Game, usize, usize, Pos, Pos, u32) {
        // Two majors, two city-states (one of them will do the asking) and
        // barbarians, so the errand and the quest are both live.
        let mut game = Game::new_full(2, 30, 20, seed, 120, 2, true);
        for player in 0..2 {
            let settler = game
                .player_unit_ids(player)
                .into_iter()
                .find(|uid| game.units[uid].kind == "settler")
                .expect("each player opens with a settler");
            game.current = player;
            game.apply(player, &crate::game::Action::FoundCity { unit: settler })
                .unwrap();
        }
        for player in 0..2 {
            for uid in game.player_unit_ids(player) {
                game.remove_unit(uid);
            }
        }
        let barb = game.barb_pid.expect("a barbarian-seated game has barb_pid");
        for uid in game.units.keys().copied().collect::<Vec<_>>() {
            if game.units[&uid].owner == barb {
                game.remove_unit(uid);
            }
        }
        game.barb_camps.clear();
        game.current = 0;
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        let open = |game: &Game, distance: i32| -> Pos {
            let mut ring: Vec<Pos> = game
                .map
                .tiles
                .keys()
                .copied()
                .filter(|pos| {
                    game.wdist(*pos, home) == distance
                        && game.map.get(*pos).is_some_and(|tile| {
                            game.rules.is_passable(tile) && !game.rules.is_water(tile)
                        })
                        && game.city_at(*pos).is_none()
                        && game.unit_ids_at(*pos).is_empty()
                })
                .collect();
            ring.sort_unstable();
            ring.into_iter()
                .next()
                .expect("open ground at the distance")
        };
        let near = open(&game, 3);
        let far = open(&game, 5);
        for camp in [near, far] {
            game.barb_camps.insert(camp, game.turn + 1_000);
            game.map.tiles.get_mut(&camp).unwrap().improvement =
                Some(crate::name!("barbarian_camp"));
        }
        let hunter = game.spawn_test_unit("swordsman", 0, home);
        let minor = game
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .expect("the fixture seats a city-state");
        game.record_contact(0, minor);
        (game, 0, minor, near, far, hunter)
    }

    /// ★★ THE ERRAND GOES TO THE CAMP THAT PAYS. Civilization VI names one
    /// outpost and pays for that one; the stock errand takes the nearest camp
    /// and would clear the wrong one all game. Both camps here are inside the
    /// errand's own radius: the gene reorders what it would already do, and
    /// never sends the hunter past the ring.
    #[test]
    fn the_camp_errand_walks_past_a_nearer_camp_to_the_one_that_pays() {
        let (mut game, pid, minor, near, far, hunter) = two_camps(90_180);
        let mut named = quest("clear_barbarian_camp", "");
        named.pos = Some(far);
        game.players[pid].quests.insert(minor, named);

        let mut stock = AdvancedAi::new();
        stock.enable_camp_bounty();
        assert_eq!(
            stock.base.camp_bounty_target(&game, pid, hunter),
            Some(near),
            "the stock errand takes the nearest camp, whoever is paying"
        );

        let mut ai = AdvancedAi::new();
        ai.enable_camp_bounty();
        ai.enable_quest_camp_errand();
        assert_eq!(
            ai.base.camp_bounty_target(&game, pid, hunter),
            Some(far),
            "the named outpost is the errand even though it is farther"
        );
    }

    /// The gene never invents an errand: with the quest naming a camp that is
    /// not on the board, the nearest camp is still the errand.
    #[test]
    fn a_quest_naming_no_live_camp_leaves_the_errand_alone() {
        let (mut game, pid, minor, near, _far, hunter) = two_camps(90_181);
        let mut named = quest("clear_barbarian_camp", "");
        named.pos = Some((0, 0));
        game.players[pid].quests.insert(minor, named);
        let mut ai = AdvancedAi::new();
        ai.enable_camp_bounty();
        ai.enable_quest_camp_errand();
        assert_eq!(ai.base.camp_bounty_target(&game, pid, hunter), Some(near));
    }

    /// Nothing is paid for a city-state that is not asking, so a board with
    /// no quests is byte-identical with every gene on.
    #[test]
    fn a_board_with_no_quests_pays_nothing_with_every_gene_on() {
        let mut g = Game::new(2, 24, 16, 20, 80, 4);
        g.players[0].quests.clear();
        let mut ai = AdvancedAi::new();
        ai.enable_quest_production();
        ai.enable_quest_trade_route();
        ai.enable_quest_camp_errand();
        ai.enable_quest_boost();
        assert_eq!(
            ai.quest_production_premium(
                &g,
                0,
                &Item::Unit {
                    unit: name!("archer")
                },
                GrandStrategy::Science
            ),
            0.0
        );
        assert_eq!(
            ai.quest_trade_route_premium(&g, 0, 1, GrandStrategy::Science),
            0.0
        );
        assert!(!ai.base.quest_camp_is_named(&g, 0, (3, 3)));
    }
}
