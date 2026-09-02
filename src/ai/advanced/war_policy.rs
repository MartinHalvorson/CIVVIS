//! War policy via the board: who may be a target, when a war is declared,
//! and when peace is sued for, read off the Objective Board's own
//! requirements instead of empire-wide power ratios. Opt-in gene
//! `war-policy-via-board` (field `war_policy_via_board`), off, an exact
//! no-op when off.
//!
//! **What the shipped layer does.** `assess` ranks rivals by
//! `rival_value_with_culture` — distance, power, score, victory pressure —
//! with no test that the army could ever take a city of theirs; the
//! elective declaration in `advanced_diplomacy` asks `my_power >
//! target_power * 1.32 + 12` (or the campaign's bill, or the domination
//! ratio) and a staged stack of three; and the peace chain sues the moment
//! `my_power < theirs * 0.62` — "outmatched" — whatever the front looks
//! like, so a defensive war whose threatened city is held by a served force
//! is begged out of on the empire total while the field is even.
//!
//! **What this does.**
//!
//! - **Feasibility** ([`AdvancedAi::war_policy_target_feasible`]): a rival
//!   whose nearest city's Siege requirement (`siege_requirement` — the bill
//!   the board writes for that city) is over the whole roster's strength
//!   (`campaign_field_army`) is not a target: `assess` drops it from the
//!   candidates `rival_value_with_culture` ranks and from the campaign's
//!   own choice. A rival whose clock is short (`urgent_victory_threat`) is
//!   untouched — denial is not elective.
//! - **Declaration** ([`AdvancedAi::war_policy_declaration`]): an elective
//!   or campaign war opens only when the strength on the staging ring of
//!   the objective city (`staged_campaign_units`, the 3–5 ring) meets that
//!   city's Siege requirement, no other major war is being fought, and
//!   every Defend row is served (no Defend requisition open). The
//!   `close_enough` and `staged` gates stand.
//! - **Peace** ([`AdvancedAi::war_policy_peace`]): the `0.62` term is
//!   replaced. Peace is sued for only when no Siege row against that rival
//!   is feasible — the board's Siege rows against them, else their nearest
//!   city — **and** either the tide ledger reads net negative over its
//!   window ([`Tide`], the same exchange `one_war.rs` keeps, here for every
//!   rival at war, gene or no gene) or an urgent Defend row has gone
//!   unserved for [`DEFEND_UNSERVED_PATIENCE`] turns. A defensive war with
//!   a served Defend row and an even tide is fought, not begged. Every other
//!   peace term (recovery, religion, fatigue, the envoy reclaim, one-war,
//!   the Science lane's defensive peace) is untouched; the tribute a `0.62`
//!   rout licensed is not claimed by this term.
//!
//! Journal: "Not a target" (Detail) for an excluded rival, the declaration's
//! blocker in the existing "Holding off war" line, the peace reason in the
//! existing "Offering peace" line. `StrategyCensus` gains
//! `war_policy_declarations_held` and `war_policy_peace_offers`.

use std::collections::{BTreeMap, VecDeque};

use super::objective_board::{ObjectiveKey, ObjectiveKind};
use super::one_war::{ONE_WAR_CITY_WEIGHT, ONE_WAR_TIDE_WINDOW};
use super::{AdvancedAi, StrategicPlan};
use crate::game::Game;
use crate::think;

/// An urgent Defend row unserved for this many turns sues for peace when
/// no siege is feasible.
pub const DEFEND_UNSERVED_PATIENCE: u32 = 3;

/// The exchange with one rival over the tide window: the war ledger's loss
/// counts at the last observation and the net per observation, newest
/// last — `one_war.rs`'s tide, kept here for every rival at war.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tide {
    /// (our units, their units, our cities, their cities) lost, at the last
    /// observation.
    ledger: (u32, u32, u32, u32),
    /// Net exchange per observation, positive ours.
    window: VecDeque<(u32, i32)>,
}

impl Tide {
    /// The net exchange over the window: positive is ours.
    pub fn window_net(&self) -> i32 {
        self.window.iter().map(|(_, net)| *net).sum()
    }
}

/// The gene's memory, kept on the controller.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WarPolicy {
    /// Per major we are at war with.
    tides: BTreeMap<usize, Tide>,
    /// Urgent Defend rows the allocation left short: city → the turn it was
    /// first seen short.
    defend_unserved_since: BTreeMap<u32, u32>,
    /// `(turn, seat)` of the last observation.
    observed: Option<(u32, usize)>,
}

impl AdvancedAi {
    /// The observation pass, once a turn: the tide with every major at
    /// war, and how long each urgent Defend row has gone unserved. Exact
    /// no-op with the gene off.
    pub(super) fn war_policy_observe(&mut self, g: &Game, pid: usize) {
        if !self.war_policy_via_board {
            return;
        }
        if self.war_policy.observed == Some((g.turn, pid)) {
            return;
        }
        self.war_policy.observed = Some((g.turn, pid));
        let enemies = self.one_war_enemies(g, pid);
        let window = g.standard_duration(ONE_WAR_TIDE_WINDOW).max(1);
        let mut tides = std::mem::take(&mut self.war_policy.tides);
        tides.retain(|rival, _| enemies.contains(rival));
        for rival in enemies {
            let ledger = Self::one_war_ledger(g, pid, rival);
            let tide = tides.entry(rival).or_insert_with(|| Tide {
                ledger,
                window: VecDeque::new(),
            });
            let (our_units, their_units, our_cities, their_cities) = (
                ledger.0.saturating_sub(tide.ledger.0) as i32,
                ledger.1.saturating_sub(tide.ledger.1) as i32,
                ledger.2.saturating_sub(tide.ledger.2) as i32,
                ledger.3.saturating_sub(tide.ledger.3) as i32,
            );
            tide.ledger = ledger;
            let net = their_units - our_units + ONE_WAR_CITY_WEIGHT * (their_cities - our_cities);
            tide.window.push_back((g.turn, net));
            while tide
                .window
                .front()
                .is_some_and(|(turn, _)| g.turn.saturating_sub(*turn) >= window)
            {
                tide.window.pop_front();
            }
        }
        self.war_policy.tides = tides;
        // The urgent Defend rows left short, as the board last wrote them.
        let short: Vec<u32> = if self.objective_board {
            let requisitions = self.requisitions();
            self.objective_board()
                .rows
                .iter()
                .filter(|row| row.kind == ObjectiveKind::Defend && row.urgent)
                .filter_map(|row| match row.key {
                    ObjectiveKey::Defend(cid) => Some(cid),
                    _ => None,
                })
                .filter(|cid| {
                    requisitions.iter().any(|requisition| {
                        requisition.kind == ObjectiveKind::Defend && requisition.city == Some(*cid)
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        self.war_policy
            .defend_unserved_since
            .retain(|cid, _| short.contains(cid));
        for cid in short {
            self.war_policy
                .defend_unserved_since
                .entry(cid)
                .or_insert(g.turn);
        }
    }

    /// The whole roster's strength: the field army, wounds priced.
    fn war_policy_roster_strength(&self, g: &Game, pid: usize) -> f64 {
        Self::campaign_strength_of(g, &self.campaign_field_army(g, pid))
    }

    /// The Siege requirement of `cid` — the bill the board writes — in
    /// strength.
    fn war_policy_siege_need(&self, g: &Game, pid: usize, cid: u32) -> f64 {
        self.siege_requirement(g, pid, cid).strength
    }

    /// Whether the whole roster could meet the Siege requirement of `cid`.
    #[cfg(test)]
    fn war_policy_siege_feasible(&self, g: &Game, pid: usize, cid: u32) -> bool {
        self.war_policy_roster_strength(g, pid) + 1e-9 >= self.war_policy_siege_need(g, pid, cid)
    }

    /// `rival`'s city nearest one of ours.
    fn nearest_city_of(&self, g: &Game, pid: usize, rival: usize) -> Option<u32> {
        let ours: Vec<crate::Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        g.player_city_ids(rival).into_iter().min_by_key(|cid| {
            let pos = g.cities[cid].pos;
            (
                ours.iter()
                    .map(|home| g.wdist(*home, pos))
                    .min()
                    .unwrap_or(0),
                *cid,
            )
        })
    }

    /// Whether `rival` may be a target: its nearest city's Siege requirement
    /// is within the whole roster's strength, or its clock is short. Always
    /// true with the gene off.
    pub(super) fn war_policy_target_feasible(&self, g: &Game, pid: usize, rival: usize) -> bool {
        if !self.war_policy_via_board || rival == pid {
            return true;
        }
        if g.is_at_war(pid, rival) || self.urgent_victory_threat(g, rival) {
            return true;
        }
        let Some(cid) = self.nearest_city_of(g, pid, rival) else {
            return true;
        };
        let need = self.war_policy_siege_need(g, pid, cid);
        let roster = self.war_policy_roster_strength(g, pid);
        if roster + 1e-9 >= need {
            return true;
        }
        if self.journal().wants(crate::reasoning::Level::Detail) {
            think!(self.journal(), Military, Detail,
                "Not a target: {}", g.players[rival].civ;
                "the Siege row for {} would ask {need:.0} strength and the whole roster is {roster:.0}",
                g.cities[&cid].name);
        }
        false
    }

    /// The declaration under the gene: `None` with the gene off; `Ok(())`
    /// when the strength staged on the objective city's ring meets its
    /// Siege requirement, no other major war is being fought and every
    /// Defend row is served; else the reason it waits.
    pub(super) fn war_policy_declaration(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        plan: &StrategicPlan,
    ) -> Option<Result<(), String>> {
        if !self.war_policy_via_board {
            return None;
        }
        let Some(city) = plan
            .target_city
            .and_then(|cid| g.cities.get(&cid))
            .filter(|city| city.owner == target)
        else {
            return Some(Err(
                "the plan names no city of theirs to besiege".to_string()
            ));
        };
        if let Some(enemy) = self
            .one_war_enemies(g, pid)
            .into_iter()
            .find(|enemy| *enemy != target)
        {
            return Some(Err(format!(
                "another major war, with {}, is being fought",
                g.players[enemy].civ
            )));
        }
        if let Some(short) = self
            .requisitions()
            .into_iter()
            .find(|requisition| requisition.kind == ObjectiveKind::Defend)
        {
            let name = short
                .city
                .and_then(|cid| g.cities.get(&cid))
                .map_or_else(|| "a city of ours".to_string(), |city| city.name.clone());
            return Some(Err(format!(
                "the Defend row at {name} is short {} unit(s)",
                short.count
            )));
        }
        let need = self.war_policy_siege_need(g, pid, city.id);
        let staged = self.staged_campaign_units(g, pid, target, city.pos);
        let strength = Self::campaign_strength_of(g, &staged);
        if strength + 1e-9 < need {
            return Some(Err(format!(
                "the Siege row for {} asks {need:.0} strength and {strength:.0} is staged on its ring in {} bodies",
                city.name,
                staged.len()
            )));
        }
        Some(Ok(()))
    }

    /// Whether any Siege row against `other` is feasible for the whole
    /// roster: the board's Siege rows against them when it has any, else
    /// their nearest city. `(feasible, the need read, the roster)`.
    fn war_policy_siege_against(&self, g: &Game, pid: usize, other: usize) -> (bool, f64, f64) {
        let roster = self.war_policy_roster_strength(g, pid);
        let rows: Vec<f64> = if self.objective_board {
            self.objective_board()
                .rows
                .iter()
                .filter(|row| row.kind == ObjectiveKind::Siege)
                .filter_map(|row| match row.key {
                    ObjectiveKey::Siege(cid) => g
                        .cities
                        .get(&cid)
                        .filter(|city| city.owner == other)
                        .map(|_| row.requirement.strength),
                    _ => None,
                })
                .collect()
        } else {
            Vec::new()
        };
        let needs = if rows.is_empty() {
            self.nearest_city_of(g, pid, other)
                .map(|cid| vec![self.war_policy_siege_need(g, pid, cid)])
                .unwrap_or_default()
        } else {
            rows
        };
        let least = needs.iter().copied().fold(f64::INFINITY, f64::min);
        let feasible = needs.iter().any(|need| roster + 1e-9 >= *need);
        (feasible, least, roster)
    }

    /// The gene's peace term for `other`: the reason, when no Siege row
    /// against them is feasible and either the tide has run against us over
    /// the window or an urgent Defend row has gone unserved for
    /// [`DEFEND_UNSERVED_PATIENCE`] turns. `None` with the gene off, and
    /// for a war the roster can still win.
    pub(super) fn war_policy_peace(&self, g: &Game, pid: usize, other: usize) -> Option<String> {
        if !self.war_policy_via_board || !g.is_at_war(pid, other) {
            return None;
        }
        let (feasible, need, roster) = self.war_policy_siege_against(g, pid, other);
        if feasible {
            return None;
        }
        let tide = self
            .war_policy
            .tides
            .get(&other)
            .map_or(0, Tide::window_net);
        let no_siege = if need.is_finite() {
            format!("no siege against them is feasible ({roster:.0} strength against a bill of {need:.0})")
        } else {
            format!("no siege against them is feasible ({roster:.0} strength, no city of theirs to bill)")
        };
        if tide < 0 {
            return Some(format!(
                "{no_siege} and the tide has run against us ({tide:+}) over the window"
            ));
        }
        let stale = self
            .war_policy
            .defend_unserved_since
            .iter()
            .map(|(cid, since)| (*cid, g.turn.saturating_sub(*since)))
            .filter(|(_, turns)| *turns >= DEFEND_UNSERVED_PATIENCE)
            .max_by_key(|(cid, turns)| (*turns, std::cmp::Reverse(*cid)));
        if let Some((cid, turns)) = stale {
            let name = g
                .cities
                .get(&cid)
                .map_or_else(|| format!("city {cid}"), |city| city.name.clone());
            return Some(format!(
                "{no_siege} and the urgent Defend row at {name} has gone unserved for {turns} turns"
            ));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::super::GrandStrategy;
    use super::*;
    use crate::game::{Game, WarLosses, WarRecord};
    use crate::name;
    use crate::Pos;

    /// A flat board, the shape `objective_board::tests` uses: every starting
    /// unit cleared, everyone met, turn 60, nobody at war.
    fn flat_board(seed: u64, capitals: &[Pos]) -> Game {
        let mut game = Game::new_full(capitals.len(), 36, 22, seed, 1_000, 0, false);
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        game.barb_camps.clear();
        game.barb_naval_camps.clear();
        for tile in game.map.tiles.values_mut() {
            tile.terrain = name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
        }
        for (pid, pos) in capitals.iter().enumerate() {
            game.found_city_for(pid, *pos, None);
        }
        for pid in 0..capitals.len() {
            for other in 0..capitals.len() {
                if pid != other {
                    game.players[pid].met.insert(other);
                }
            }
            game.players[pid]
                .explored
                .extend(game.map.tiles.keys().copied());
        }
        game.at_war.clear();
        game.turn = 60;
        game.current = 0;
        game
    }

    fn at(col: i32, row: i32) -> Pos {
        crate::hex::offset_to_axial(col, row)
    }

    fn war(g: &mut Game, a: usize, b: usize) {
        g.at_war.insert((a.min(b), a.max(b)));
    }

    fn spawn(g: &mut Game, kind: &str, pid: usize, pos: Pos) -> u32 {
        let uid = g.spawn_test_unit(kind, pid, pos);
        let moves = g.unit_max_moves(uid);
        let unit = g.units.get_mut(&uid).unwrap();
        unit.moves_left = moves;
        unit.attacks_left = 1;
        uid
    }

    fn conquest(g: &Game, target: Option<u32>) -> StrategicPlan {
        StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: target.map(|cid| g.cities[&cid].owner),
            target_city: target,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: g.turn,
            rush: false,
        }
    }

    fn on() -> AdvancedAi {
        let mut ai = AdvancedAi::new();
        ai.enable_objective_board();
        ai.enable_war_policy_via_board();
        ai
    }

    fn city_of(g: &Game, pid: usize, pos: Pos) -> u32 {
        g.city_at(pos)
            .filter(|cid| g.cities[cid].owner == pid)
            .expect("a city of ours")
    }

    /// A war record between 0 and `other` with `ours` units of ours lost.
    fn record_losses(g: &mut Game, other: usize, ours: u32, theirs: u32) {
        let key = (0usize, other);
        let mut losses = BTreeMap::new();
        losses.insert(
            0,
            WarLosses {
                units: ours,
                ..Default::default()
            },
        );
        losses.insert(
            other,
            WarLosses {
                units: theirs,
                ..Default::default()
            },
        );
        match g.wars.get_mut(&key) {
            Some(record) => record.losses = losses,
            None => {
                g.wars.insert(
                    key,
                    WarRecord {
                        conflict: 1,
                        declarer: other,
                        target: 0,
                        casus_belli: None,
                        joint_war_until: None,
                        aggressor: other,
                        defender: 0,
                        started: 50,
                        ended: None,
                        losses,
                        participants: Vec::new(),
                        peace_terms: Vec::new(),
                        highlights: Vec::new(),
                        theater: Vec::new(),
                    },
                );
            }
        }
    }

    #[test]
    fn the_gene_ships_off_and_is_registered() {
        let ai = AdvancedAi::new();
        assert!(!ai.war_policy_via_board, "an opt-in ships off");
        assert!(super::super::GENES.iter().any(|gene| gene.opt_in()
            && gene.tag == "war-policy-via-board"
            && gene.field == "war_policy_via_board"));
        let mut on = AdvancedAi::new();
        on.enable_war_policy_via_board();
        assert!(on.war_policy_via_board);
        on.disable_war_policy_via_board();
        assert!(!on.war_policy_via_board);
        super::super::test_support::opt_in_off_in_both_controllers("war-policy-via-board", |ai| {
            ai.war_policy_via_board
        });
        // Off: every rival is feasible, the declaration and the peace term
        // read nothing.
        let g = flat_board(31, &[at(6, 8), at(20, 8), at(30, 14)]);
        assert!(ai.war_policy_target_feasible(&g, 0, 1));
        assert!(ai
            .war_policy_declaration(&g, 0, 1, &conquest(&g, None))
            .is_none());
        assert!(ai.war_policy_peace(&g, 0, 1).is_none());
    }

    /// A rival whose nearest city's Siege requirement is over the whole
    /// roster is not a target; one the roster can take is.
    #[test]
    fn an_infeasible_rival_is_not_a_target() {
        let mut g = flat_board(32, &[at(6, 8), at(20, 8), at(30, 14)]);
        let fortress = city_of(&g, 1, at(20, 8));
        // Six warriors hold the fortress; the third player's capital stands
        // empty; our roster is two warriors.
        for pos in [
            at(21, 8),
            at(19, 8),
            at(20, 7),
            at(20, 9),
            at(21, 7),
            at(19, 9),
        ] {
            spawn(&mut g, "warrior", 1, pos);
        }
        for pos in [at(7, 8), at(5, 8)] {
            spawn(&mut g, "warrior", 0, pos);
        }
        let ai = on();
        assert!(
            !ai.war_policy_siege_feasible(&g, 0, fortress),
            "six warriors and a city are over two warriors"
        );
        assert!(!ai.war_policy_target_feasible(&g, 0, 1), "not a target");
        let open = city_of(&g, 2, at(30, 14));
        assert!(
            ai.war_policy_siege_feasible(&g, 0, open) || !ai.war_policy_target_feasible(&g, 0, 2),
            "the helper and the gate agree on the open capital"
        );
        // Eight swordsmen of ours and the fortress is within reach.
        for col in 0..8 {
            spawn(&mut g, "swordsman", 0, at(4 + col, 12));
        }
        assert!(ai.war_policy_target_feasible(&g, 0, 1));
        // Off, everyone is a target.
        let off = AdvancedAi::new();
        let mut poor = flat_board(33, &[at(6, 8), at(20, 8), at(30, 14)]);
        for pos in [
            at(21, 8),
            at(19, 8),
            at(20, 7),
            at(20, 9),
            at(21, 7),
            at(19, 9),
        ] {
            spawn(&mut poor, "warrior", 1, pos);
        }
        assert!(off.war_policy_target_feasible(&poor, 0, 1));
    }

    /// A declaration waits until the strength staged on the objective's
    /// ring meets its Siege requirement, and opens then.
    #[test]
    fn a_declaration_waits_until_staged_strength_meets_the_need() {
        let mut g = flat_board(34, &[at(6, 8), at(20, 8)]);
        let target = city_of(&g, 1, at(20, 8));
        let plan = conquest(&g, Some(target));
        let ai = on();
        assert!(
            matches!(ai.war_policy_declaration(&g, 0, 1, &plan), Some(Err(_))),
            "nothing staged: the declaration waits"
        );
        // One warrior on the ring: still short.
        spawn(&mut g, "warrior", 0, at(15, 8));
        let verdict = ai.war_policy_declaration(&g, 0, 1, &plan);
        let Some(Err(reason)) = verdict else {
            panic!("one warrior is not a siege: {verdict:?}");
        };
        assert!(reason.contains("Siege row"), "{reason}");
        // Bodies on the 3–5 ring until the bill is met.
        let ring: Vec<Pos> = g
            .wdisk(g.cities[&target].pos, 5)
            .into_iter()
            .filter(|pos| (3..=5).contains(&g.wdist(*pos, g.cities[&target].pos)))
            .filter(|pos| g.unit_ids_at(*pos).is_empty())
            .collect();
        let mut opened = false;
        for pos in ring {
            spawn(&mut g, "swordsman", 0, pos);
            if ai.war_policy_declaration(&g, 0, 1, &plan) == Some(Ok(())) {
                opened = true;
                break;
            }
        }
        assert!(opened, "the ring filled and the declaration never opened");
        // A second major war holds it.
        let mut two_fronts = flat_board(35, &[at(6, 8), at(20, 8), at(30, 14)]);
        war(&mut two_fronts, 0, 2);
        let plan = conquest(&two_fronts, Some(city_of(&two_fronts, 1, at(20, 8))));
        let Some(Err(reason)) = ai.war_policy_declaration(&two_fronts, 0, 1, &plan) else {
            panic!("a second front must hold the declaration");
        };
        assert!(reason.contains("another major war"), "{reason}");
    }

    /// Our capital at war and under pressure from three warriors, the
    /// Defend row served by three of ours beside it, their capital held by
    /// six: no siege is feasible, the tide is even — peace is not offered.
    fn defended_front(seed: u64) -> (Game, AdvancedAi) {
        let mut g = flat_board(seed, &[at(6, 8), at(24, 8)]);
        war(&mut g, 0, 1);
        for pos in [at(8, 8), at(7, 9), at(7, 7)] {
            spawn(&mut g, "warrior", 1, pos);
        }
        for pos in [at(5, 8), at(5, 9), at(4, 8)] {
            spawn(&mut g, "warrior", 0, pos);
        }
        for pos in [
            at(25, 8),
            at(23, 8),
            at(24, 7),
            at(24, 9),
            at(25, 7),
            at(23, 9),
        ] {
            spawn(&mut g, "warrior", 1, pos);
        }
        let mut ai = on();
        let plan = conquest(&g, None);
        ai.rebuild_force_groups(&g, 0, &plan);
        ai.war_policy_observe(&g, 0);
        (g, ai)
    }

    #[test]
    fn peace_is_not_offered_while_the_defend_row_is_served_and_the_tide_is_even() {
        let (g, ai) = defended_front(36);
        let home = city_of(&g, 0, at(6, 8));
        assert!(
            ai.requisitions()
                .iter()
                .all(|requisition| requisition.kind != ObjectiveKind::Defend),
            "the Defend row is served: {:?}",
            ai.requisitions()
        );
        assert!(ai
            .objective_board()
            .rows
            .iter()
            .any(|row| row.key == ObjectiveKey::Defend(home)));
        let (feasible, _, _) = ai.war_policy_siege_against(&g, 0, 1);
        assert!(!feasible, "six warriors and a city are over three warriors");
        assert_eq!(ai.war_policy.tides[&1].window_net(), 0);
        assert!(
            ai.war_policy_peace(&g, 0, 1).is_none(),
            "fought, not begged"
        );
    }

    #[test]
    fn peace_is_offered_when_the_tide_is_negative_and_no_siege_is_feasible() {
        let (mut g, mut ai) = defended_front(37);
        // Three of ours lost for none of theirs since the last observation.
        record_losses(&mut g, 1, 3, 0);
        g.turn += 1;
        ai.war_policy_observe(&g, 0);
        assert_eq!(ai.war_policy.tides[&1].window_net(), -3);
        let reason = ai
            .war_policy_peace(&g, 0, 1)
            .expect("no siege is feasible and the tide has turned");
        assert!(reason.contains("tide has run against us"), "{reason}");
        // Two of theirs fall: the window turns and the term stands down.
        record_losses(&mut g, 1, 3, 5);
        g.turn += 1;
        ai.war_policy_observe(&g, 0);
        assert!(ai.war_policy.tides[&1].window_net() > 0);
        assert!(ai.war_policy_peace(&g, 0, 1).is_none());
    }

    /// An urgent Defend row left short for the patience sues for peace
    /// when no siege is feasible, even with the tide even.
    #[test]
    fn an_urgent_defend_row_unserved_for_the_patience_sues_for_peace() {
        let mut g = flat_board(38, &[at(6, 8), at(24, 8)]);
        war(&mut g, 0, 1);
        // Three enemy warriors at the capital and nobody of ours anywhere:
        // an urgent Defend nobody can serve; their capital held by six.
        for pos in [at(8, 8), at(7, 9), at(7, 7)] {
            spawn(&mut g, "warrior", 1, pos);
        }
        for pos in [
            at(25, 8),
            at(23, 8),
            at(24, 7),
            at(24, 9),
            at(25, 7),
            at(23, 9),
        ] {
            spawn(&mut g, "warrior", 1, pos);
        }
        let mut ai = on();
        let plan = conquest(&g, None);
        ai.rebuild_force_groups(&g, 0, &plan);
        let home = city_of(&g, 0, at(6, 8));
        let row = ai
            .objective_board()
            .rows
            .iter()
            .find(|row| row.key == ObjectiveKey::Defend(home))
            .expect("a Defend row");
        assert!(row.urgent, "no force can relieve it: {row:?}");
        ai.war_policy_observe(&g, 0);
        assert!(ai.war_policy_peace(&g, 0, 1).is_none(), "not yet");
        for _ in 0..DEFEND_UNSERVED_PATIENCE {
            g.turn += 1;
            ai.rebuild_force_groups(&g, 0, &plan);
            ai.war_policy_observe(&g, 0);
        }
        let reason = ai
            .war_policy_peace(&g, 0, 1)
            .expect("the Defend row has gone unserved for the patience");
        assert!(reason.contains("unserved"), "{reason}");
    }
}
