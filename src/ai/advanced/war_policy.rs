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
//!   unserved for [`DEFEND_UNSERVED_PATIENCE`] turns. When `one-war-at-a-time`
//!   owns the active front, its sustained-tide and rout rules own that tide
//!   decision; a single negative window here cannot abandon its siege. A
//!   defensive war with a served Defend row and an even tide is fought, not
//!   begged. A live Domination siege also defers recovery and fatigue peace.
//!   Religion, envoy reclaim and the Science lane's defensive peace keep
//!   their own rules; the tribute a `0.62` rout licensed is not claimed by
//!   this term. A present siege gets twelve standard turns to invest; a live
//!   breach through standing walls gets up to eighty before these terms may
//!   end it.
//!
//! Journal: "Not a target" (Detail) for an excluded rival, the declaration's
//! blocker in the existing "Holding off war" line, the peace reason in the
//! existing "Offering peace" line. `StrategyCensus` gains
//! `war_policy_declarations_held` and `war_policy_peace_offers`.

use std::collections::{BTreeMap, VecDeque};

use super::objective_board::{ObjectiveKey, ObjectiveKind};
use super::one_war::{ONE_WAR_CITY_WEIGHT, ONE_WAR_TIDE_WINDOW};
use super::siege_train::SiegeStage;
use super::{AdvancedAi, StrategicPlan, VictoryTarget};
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
            let ledger = self.one_war_ledger(g, pid, rival);
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
    /// is within the whole roster's strength, or its clock is short. A
    /// committed domination player may also name a preparation target before
    /// its army is ready. Always true with the gene off.
    pub(super) fn war_policy_target_feasible(&self, g: &Game, pid: usize, rival: usize) -> bool {
        if !self.war_policy_via_board || rival == pid {
            return true;
        }
        if g.is_at_war(pid, rival) || self.urgent_victory_threat(g, rival) {
            return true;
        }
        // A committed domination player needs an objective to mobilize:
        // assess names its city, production raises the army budget, and the
        // staging pass assembles it. Requiring that army here prevented the
        // preparation itself. The declaration still checks the full Siege
        // requirement, staging and unmet defensive needs.
        if self.active_victory_target(g) == Some(super::VictoryTarget::Domination) {
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
        // `one_war_second_front`: the rival the one-war gate admits beside
        // the burning war is not held by it here either.
        let second_front = self.one_war_second_front(g, pid) == Some(target);
        // `board-ignores-the-faith-war`: the war `faith-war-is-no-front`
        // already keeps out of `one_war_holds_declaration` does not hold the
        // board's declaration either. Live Immortal civvis-20261031T054450Z
        // held its war on Australia ("another major war, with Macedon") on
        // 49 turns from 101 at 6-12 times Australia's power: the Macedon war
        // was our own faith war, opened at 97, and Melbourne walled up by 123.
        let enemies = if self.board_ignores_the_faith_war {
            self.without_faith_war(g, pid, self.one_war_enemies(g, pid))
        } else {
            self.one_war_enemies(g, pid)
        };
        if let Some(enemy) = enemies
            .into_iter()
            .find(|enemy| *enemy != target && !second_front)
        {
            return Some(Err(format!(
                "another major war, with {}, is being fought",
                g.players[enemy].civ
            )));
        }
        // A Domination seat countering this rival's clock at twice its power
        // does not wait on a home Defend row. Live King
        // civvis-20261003T113755Z (game 34) held its war on the Khmer for a
        // short Defend row at turns 115, 125 and 135, the last at 438 power
        // against 138, and lost to Khmer Buddhism at 153 without a war.
        let counter_at_mercy = self.active_victory_target(g)
            == Some(super::VictoryTarget::Domination)
            && self.domination_counter_target(g, pid, target)
            && g.military_power(pid) >= 2.0 * g.military_power(target).max(1.0);
        // `prey-is-declared-on`: prey waits on neither a home Defend row nor
        // a staged ring. See `advanced/prey_declaration.rs`.
        if self.prey_is_declared_on_now(g, pid, target) {
            return Some(Ok(()));
        }
        if let Some(short) = self
            .requisitions()
            .into_iter()
            .find(|requisition| requisition.kind == ObjectiveKind::Defend)
            .filter(|_| !counter_at_mercy)
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
        // See `overwhelming_power_declares`: before a war the board writes no
        // Siege row, so nothing orders the army onto this ring.
        if self.overwhelming_power_declares(g, pid, target, city.pos) {
            return Some(Ok(()));
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
    pub(super) fn war_policy_siege_against(
        &self,
        g: &Game,
        pid: usize,
        other: usize,
    ) -> (bool, f64, f64) {
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

    /// A present Domination siege gets a short stage grace. A reducing
    /// column that has breached at least 20 wall HP gets a longer, bounded
    /// chance to finish the attack through standing walls.
    pub(super) fn domination_siege_has_grace(&self, g: &Game, pid: usize, other: usize) -> bool {
        // `score-seat-takes-what-pays`: or a Score seat's paying city.
        (self.active_victory_target(g) == Some(VictoryTarget::Domination)
            || self.score_take_names_a_city(g))
            && self.sieges.iter().any(|(cid, siege)| {
                matches!(
                    siege.stage,
                    SiegeStage::Invest | SiegeStage::Reduce | SiegeStage::Take
                ) && g.turn.saturating_sub(siege.assessed) <= 1
                    && g.cities.get(cid).is_some_and(|city| {
                        let breached_wall = siege.stage == SiegeStage::Reduce
                            && city.wall_hp > 0
                            && g.city_max_wall_hp(city).saturating_sub(city.wall_hp) >= 20;
                        let grace = if breached_wall {
                            g.standard_duration(80)
                        } else {
                            g.standard_duration(12)
                        };
                        if g.turn.saturating_sub(siege.entered) > grace {
                            return false;
                        }
                        city.owner == other
                            && (matches!(siege.stage, SiegeStage::Invest | SiegeStage::Reduce)
                                || city.hp < 200
                                || city.wall_hp < g.city_max_wall_hp(city))
                            && Self::domination_siege_present(g, pid, *cid)
                    })
            })
    }

    /// The gene's peace term for `other`: the reason, when no Siege row
    /// against them is feasible and either the tide has run against us over
    /// the window or an urgent Defend row has gone unserved for
    /// [`DEFEND_UNSERVED_PATIENCE`] turns. The active one-war front owns its
    /// own tide patience and rout decision. `None` with the gene off, and
    /// for a war the roster can still win.
    pub(super) fn war_policy_peace(&self, g: &Game, pid: usize, other: usize) -> Option<String> {
        if !self.war_policy_via_board || !g.is_at_war(pid, other) {
            return None;
        }
        let (feasible, need, roster) = self.war_policy_siege_against(g, pid, other);
        if feasible {
            return None;
        }
        if self.domination_siege_has_grace(g, pid, other) {
            return None;
        }
        // See `war_holds_the_road`.
        if self.war_holds_the_road(g, pid, other) {
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
        let one_war_owns_tide = self.one_war_at_a_time
            && self
                .one_war
                .as_ref()
                .is_some_and(|front| front.target == other);
        if tide < 0 && !one_war_owns_tide {
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
mod mobilization_tests;

#[cfg(test)]
mod tests;
