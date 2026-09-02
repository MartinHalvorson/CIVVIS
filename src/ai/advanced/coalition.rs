//! Coalition before war: recruit the target's neighbours before the
//! declaration, so the war it gets is a war on several fronts.
//!
//! ★★★ THE WAR DESK PREPARES AN ARMY AND NOTHING ELSE. An appointed war
//! (`WarPlan`) spends its Research / Mobilize / Stage phases on a
//! breakthrough tech, a package of units and a march, and an elective war
//! (`advanced_diplomacy`'s Conquest branch) waits for `ready && staged`;
//! neither spends one turn of the lead time on who else will be fighting the
//! target. `propose_strategic_alliance` picks its partner from the empire's
//! own grand strategy on a 12-turn cadence and never looks at the target;
//! the envoy scorer reads a city-state's place only relative to OUR cities
//! (`flip-nearby-city-states`); and `Action::ProposeJointWar` — the one
//! engine action that makes a second empire declare on the same turn we do —
//! is constructed by `legal_actions` and by no controller at all. The
//! operator's rule (2026-08-25): *before we go to war, recruit allies, a
//! few turns in advance: when we fight someone, as many of that civ's
//! neighbours on our side as we can get — alliances, particularly military
//! alliances, with the target's other neighbours — and flip as many of its
//! city-states as we can, so that when we go to war the civ is facing wars
//! on several fronts.*
//!
//! What the gene does, all of it inert while the flag is off:
//!
//! 1. **A coalition window.** From the turn the war desk holds a target we
//!    are at peace with — the appointed war's target while the package is
//!    short of Exploit, else the Conquest plan's — until the war starts or
//!    the target changes, `Coalition` remembers the target and who has been
//!    asked what. The window IS the lead time: the appointment's own phases
//!    for a timed war, the readiness wait for an elective one.
//! 2. **Alliances with the target's neighbours.** Every turn of the window
//!    one alliance is proposed to the best neighbour of the target
//!    (a major with a city within [`COALITION_NEIGHBOUR_REACH`] of one of
//!    the target's): `military` when that kind is free on both sides, else
//!    the first free kind, friendship bundled as the stock proposal does.
//!    Neighbours who already resent the target and neighbours not yet bound
//!    to it by friendship or alliance rank first; a refused partner is asked
//!    again after [`COALITION_REASK_TURNS`]. The stock cadence and strategy
//!    match are not waited for. A typed alliance obliges nobody to fight —
//!    only a Defensive Pact does — but an ally shares the target's aggression
//!    as grievance (`ALLIED_GRIEVANCE_SHARE`), cannot be the target's ally of
//!    the same kind, and is the partner a joint war is proposed to.
//! 3. **Envoys to the target's city-states.** One more term in
//!    `advanced_envoys`' score: a city-state within
//!    [`COALITION_CITY_STATE_RADIUS`] tiles of a city of the target is worth
//!    its proximity, plus [`COALITION_TARGET_CLIENT`] when the target is its
//!    suzerain (a client the target loses AND a front it gains — a suzerain's
//!    clients fight its wars, `Game::is_at_war` derives it) or
//!    [`COALITION_OPEN_CITY_STATE`] when nobody holds it; amortised over the
//!    envoys the suzerainty still needs, zero for one we hold securely.
//! 4. **Joint-war invitations at the strike.** The turn the war desk would
//!    declare — the appointed package complete, or the elective gates
//!    `close_enough && ready && staged` — every eligible neighbour of the
//!    target is sent `Action::ProposeJointWar` first and the declaration is
//!    held while an answer is due, at most [`COALITION_JOINT_WAR_PATIENCE`]
//!    turns. An accepted invitation starts the war for both of us at once
//!    (`do_accept_deal` → `start_war(from, target, joint_war, Some(to))`),
//!    so there is nothing left for us to declare; a refused one costs no
//!    grievance and the desk declares alone the next turn. Native empires
//!    accept when they resent the target (≥ 20 grievances) and the joint
//!    power clears 1.2× the target's; an advanced one when the target is
//!    already its own plan's target.
//!
//! Counters: `coalition:opened`, `coalition:alliances_proposed`,
//! `coalition:envoys`, `coalition:joint_wars_proposed`,
//! `coalition:held_declarations`.
//!
//! Version 2 keeps only the part with a bounded, immediate payoff. At the
//! ready strike it invites one neighbour whose acceptance is supported by the
//! public board: either the target is already at terminal victory pressure,
//! or that neighbour has the grievance and combined power the Basic
//! controller requires. It consumes the invitation turn, then never retries.
//! Before the strike it proposes only a military alliance to such a partner,
//! and adds Envoy value only when that directly unseats the target from a
//! nearby client city-state.

use std::collections::BTreeMap;

use super::{AdvancedAi, GrandStrategy, StrategicPlan, WarPhase};
use crate::game::{Action, Game};
use crate::think;

/// A major is a neighbour of the target when one of its cities is within
/// this many tiles of one of the target's: the same reach the city campaign
/// uses to call a rival attackable at all.
pub(crate) const COALITION_NEIGHBOUR_REACH: i32 = super::city_campaign::CAMPAIGN_REACH;
/// A city-state this close to a city of the target is a front against it.
pub(crate) const COALITION_CITY_STATE_RADIUS: i32 = 9;
/// Envoy score per tile of proximity inside the radius.
pub(crate) const COALITION_CITY_STATE_PER_TILE: i64 = 10;
/// Envoy score for unseating the target as suzerain: its client, our front.
pub(crate) const COALITION_TARGET_CLIENT: i64 = 200;
/// Envoy score for a city-state near the target that nobody holds.
pub(crate) const COALITION_OPEN_CITY_STATE: i64 = 60;
/// A neighbour who refused, or has not answered, is asked again after this.
pub(crate) const COALITION_REASK_TURNS: u32 = 10;
/// The declaration is held this many turns at most for a joint-war answer.
/// Every empire answers its incoming deals on its own turn, so one full
/// round is enough; the second turn covers a partner seated before us.
pub(crate) const COALITION_JOINT_WAR_PATIENCE: u32 = 2;
/// Alliance kinds in the order the coalition asks for them: military first.
pub(crate) const COALITION_ALLIANCE_KINDS: [&str; 5] =
    ["military", "economic", "cultural", "religious", "research"];
/// A neighbour not bound to the target by friendship or alliance can still
/// join a joint war against it, and ranks this much higher as a partner.
pub(crate) const COALITION_UNBOUND_BONUS: f64 = 60.0;
/// Our grievances against a neighbour above this and it is no partner (the
/// stock alliance filter's ceiling).
pub(crate) const COALITION_GRIEVANCE_CEILING: f64 = 75.0;

/// The coalition being built for one target, carried from the turn the war
/// desk first holds the target until the war starts or the target changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Coalition {
    /// The empire the coming war is against.
    pub(crate) target: usize,
    /// The turn the window opened.
    pub(crate) opened: u32,
    /// Neighbours asked for an alliance, by the turn they were last asked.
    pub(crate) alliance_asked: BTreeMap<usize, u32>,
    /// Neighbours invited to a joint war, by the turn they were last invited.
    pub(crate) joint_war_asked: BTreeMap<usize, u32>,
    /// The turn the last round of joint-war invitations went out, if any.
    pub(crate) invited: Option<u32>,
}

impl AdvancedAi {
    /// The war the coalition is for: the appointed war's target while the
    /// package is short of Exploit, else the Conquest plan's target — and
    /// only while it is a met, living major we are neither at war with nor
    /// bound to. `None` with the gene off.
    pub(crate) fn coalition_target(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<usize> {
        if !self.coalition_before_war && !self.coalition_before_war_2 {
            return None;
        }
        let appointed = self
            .war_plan
            .as_ref()
            .filter(|war| war.phase != WarPhase::Exploit)
            .map(|war| war.target_player);
        let target = appointed.or_else(|| {
            (plan.strategy == GrandStrategy::Conquest)
                .then_some(plan.target_player)
                .flatten()
        })?;
        let other = g.players.get(target)?;
        (target != pid
            && other.alive
            && !other.is_minor
            && !other.is_barbarian
            && g.has_met(pid, target)
            && !g.is_at_war(pid, target)
            && !g.are_friends(pid, target)
            && !g.are_allied(pid, target))
        .then_some(target)
    }

    /// Open, keep or close the window for this turn's plan. Exact no-op with
    /// the gene off (the window is dropped).
    pub(crate) fn coalition_observe(&mut self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let target = self.coalition_target(g, pid, plan);
        match (target, self.coalition.as_ref()) {
            (None, _) => self.coalition = None,
            (Some(target), Some(current)) if current.target == target => {}
            (Some(target), _) => {
                self.coalition = Some(Coalition {
                    target,
                    opened: g.turn,
                    ..Coalition::default()
                });
                *g.players[pid]
                    .counters
                    .entry("coalition:opened".to_string())
                    .or_insert(0) += 1;
                if self.journal().wants(crate::reasoning::Level::Strategy) {
                    let neighbours = self.coalition_neighbours(g, pid, target).len();
                    think!(self.journal(), Diplomacy, Strategy,
                           "Building a coalition against {}", g.players[target].civ;
                           "{neighbours} other major{} border{} them; alliances, their \
                            city-states and a joint war are asked for before the declaration",
                           if neighbours == 1 { "" } else { "s" },
                           if neighbours == 1 { "s" } else { "" });
                }
            }
        }
    }

    /// The living majors other than us and the target with a city within
    /// [`COALITION_NEIGHBOUR_REACH`] of one of the target's cities.
    pub(crate) fn coalition_neighbours(&self, g: &Game, pid: usize, target: usize) -> Vec<usize> {
        let theirs: Vec<crate::Pos> = g
            .player_city_ids(target)
            .into_iter()
            .filter_map(|cid| g.cities.get(&cid).map(|city| city.pos))
            .collect();
        if theirs.is_empty() {
            return Vec::new();
        }
        g.players
            .iter()
            .filter(|p| p.id != pid && p.id != target && p.alive && !p.is_minor && !p.is_barbarian)
            .filter(|p| {
                g.player_city_ids(p.id).into_iter().any(|cid| {
                    g.cities.get(&cid).is_some_and(|city| {
                        theirs
                            .iter()
                            .any(|pos| g.wdist(city.pos, *pos) <= COALITION_NEIGHBOUR_REACH)
                    })
                })
            })
            .map(|p| p.id)
            .collect()
    }

    /// One alliance proposal per turn of the window, to the best neighbour
    /// of the target: `military` when free on both sides, else the first
    /// free kind. Nothing outside the window or before Civil Service.
    pub(crate) fn coalition_alliance_step(&mut self, g: &mut Game, pid: usize) {
        if !self.coalition_before_war && !self.coalition_before_war_2 {
            return;
        }
        let Some(target) = self.coalition.as_ref().map(|c| c.target) else {
            return;
        };
        if !g.players[pid]
            .civics
            .contains(&crate::name!("civil_service"))
        {
            return;
        }
        let turn = g.turn;
        let pending_with = |partner: usize| {
            g.pending_deals.iter().any(|deal| {
                deal.expires >= turn
                    && ((deal.from == pid && deal.to == partner)
                        || (deal.from == partner && deal.to == pid))
            })
        };
        let asked_recently = |partner: usize| {
            self.coalition
                .as_ref()
                .and_then(|c| c.alliance_asked.get(&partner))
                .is_some_and(|asked| turn < asked + COALITION_REASK_TURNS)
        };
        let kind_taken = |who: usize, kind: &str| {
            g.players[who]
                .alliances
                .values()
                .any(|alliance| alliance.ends > turn && alliance.kind == kind)
        };
        let free_kind = |partner: usize| {
            COALITION_ALLIANCE_KINDS.iter().copied().find(|kind| {
                (!self.coalition_before_war_2 || *kind == "military")
                    && !kind_taken(pid, kind)
                    && !kind_taken(partner, kind)
                    && (*kind != "research"
                        || (g.tree_effect(pid, "research_agreements") > 0.0
                            && g.tree_effect(partner, "research_agreements") > 0.0))
            })
        };
        let grievance = |holder: usize, against: usize| {
            g.players[holder]
                .grievances
                .get(&against)
                .copied()
                .unwrap_or(0.0)
        };
        let score = |partner: usize| {
            let friendship = if g.are_friends(pid, partner) {
                180.0
            } else {
                0.0
            };
            let unbound = if g.are_friends(partner, target) || g.are_allied(partner, target) {
                0.0
            } else {
                COALITION_UNBOUND_BONUS
            };
            g.military_power(partner).min(250.0) * 0.25
                + friendship
                + unbound
                + grievance(partner, target)
                - grievance(pid, partner)
        };
        let choice = self
            .coalition_neighbours(g, pid, target)
            .into_iter()
            .filter(|partner| {
                g.has_met(pid, *partner)
                    && !g.is_at_war(pid, *partner)
                    && g.players[*partner]
                        .civics
                        .contains(&crate::name!("civil_service"))
                    && g.alliance_with(pid, *partner).is_none()
                    && !pending_with(*partner)
                    && !asked_recently(*partner)
                    && grievance(pid, *partner) < COALITION_GRIEVANCE_CEILING
                    && self.rival_victory_pressure(g, *partner).progress < 82
                    && (!self.coalition_before_war_2
                        || self.coalition_credible_partner(g, pid, *partner, target))
            })
            .filter_map(|partner| free_kind(partner).map(|kind| (partner, kind)))
            .max_by(|left, right| {
                score(left.0)
                    .partial_cmp(&score(right.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| right.0.cmp(&left.0))
            });
        let Some((partner, kind)) = choice else {
            return;
        };
        let proposal = Action::ProposeDeal {
            player: partner,
            give_gold: 0.0,
            request_gold: 0.0,
            // `no_free_passage`: passage is sold, not bundled.
            open_borders: !self.base.no_free_passage
                && g.players[pid]
                    .civics
                    .contains(&crate::name!("early_empire")),
            friendship: true,
            peace: false,
            alliance: Some(kind.to_string()),
        };
        if g.apply(pid, &proposal).is_err() {
            return;
        }
        if let Some(coalition) = self.coalition.as_mut() {
            coalition.alliance_asked.insert(partner, turn);
        }
        *g.players[pid]
            .counters
            .entry("coalition:alliances_proposed".to_string())
            .or_insert(0) += 1;
        if self.journal().wants(crate::reasoning::Level::Decision) {
            think!(self.journal(), Diplomacy, Decision,
                   "Proposing a {kind} alliance to {}", g.players[partner].civ;
                   "they border {}, the empire the war desk is preparing against; \
                    an ally on their other frontier is a second front",
                   g.players[target].civ);
        }
    }

    /// The envoy term: what a city-state's place NEXT TO THE TARGET is worth
    /// — proximity, plus the target as its suzerain to unseat, or nobody —
    /// amortised over the envoys the suzerainty still needs. Zero outside
    /// the window, beyond the radius, or for one we hold securely.
    pub(crate) fn coalition_city_state_bonus(
        &self,
        g: &Game,
        pid: usize,
        minor: usize,
        needed: i64,
    ) -> i64 {
        if !self.coalition_before_war && !self.coalition_before_war_2 {
            return 0;
        }
        let Some(target) = self.coalition.as_ref().map(|c| c.target) else {
            return 0;
        };
        let Some(seat) = g
            .player_city_ids(minor)
            .into_iter()
            .next()
            .and_then(|cid| g.cities.get(&cid))
            .map(|city| city.pos)
        else {
            return 0;
        };
        let Some(near) = g
            .player_city_ids(target)
            .into_iter()
            .filter_map(|cid| g.cities.get(&cid))
            .map(|city| g.wdist(city.pos, seat))
            .min()
        else {
            return 0;
        };
        if near > COALITION_CITY_STATE_RADIUS {
            return 0;
        }
        let holder = g.suzerain_of(minor);
        if self.coalition_before_war_2 && holder != Some(target) {
            return 0;
        }
        if holder == Some(pid) {
            let mine = g.envoys_at(pid, minor);
            let rival = g
                .players
                .iter()
                .filter(|p| !p.is_minor && !p.is_barbarian && p.id != pid)
                .map(|p| g.envoys_at(p.id, minor))
                .max()
                .unwrap_or(0);
            if mine > rival + 1 {
                return 0;
            }
        }
        let proximity =
            i64::from(COALITION_CITY_STATE_RADIUS + 1 - near) * COALITION_CITY_STATE_PER_TILE;
        let side = match holder {
            Some(leader) if leader == target => COALITION_TARGET_CLIENT,
            None => COALITION_OPEN_CITY_STATE,
            _ => 0,
        };
        (proximity + side) / needed.max(1)
    }

    /// Whether a neighbour can be invited to a joint war against the target:
    /// the engine's `joint_war_available`, read from this side so that no
    /// refused order is issued.
    fn coalition_joint_war_partner(
        &self,
        g: &Game,
        pid: usize,
        partner: usize,
        target: usize,
    ) -> bool {
        let foreign_trade = |who: usize| {
            g.players[who]
                .civics
                .contains(&crate::name!("foreign_trade"))
        };
        g.has_met(pid, partner)
            && g.has_met(partner, target)
            && foreign_trade(pid)
            && foreign_trade(partner)
            && !g.is_at_war(pid, partner)
            && !g.is_at_war(partner, target)
            && !g.are_friends(partner, target)
            && !g.are_allied(partner, target)
            && g.peace_treaty_until(partner, target).is_none()
            && g.peace_treaty_until(pid, target).is_none()
    }

    /// Whether the public board supports expecting this partner to join.
    /// Terminal victory pressure is the Advanced controller's acceptance
    /// escape hatch. Otherwise mirror the Basic controller's exact grievance
    /// and joint-power gate, so version two does not spend a strike turn on a
    /// merely legal but predictably refused invitation.
    fn coalition_credible_partner(
        &self,
        g: &Game,
        pid: usize,
        partner: usize,
        target: usize,
    ) -> bool {
        if self.rival_victory_pressure(g, target).progress >= 78 {
            return true;
        }
        let grievance = g.players[partner]
            .grievances
            .get(&target)
            .copied()
            .unwrap_or(0.0);
        let joint_power = g.military_power(pid) + g.military_power(partner);
        grievance >= 20.0 && joint_power > g.military_power(target) * 1.2 + 20.0
    }

    /// At the strike: invite every eligible neighbour of the target to a
    /// joint war and hold the declaration while an answer is due. `true`
    /// consumes the turn's war-opening decision; `false` lets the desk
    /// declare. Nothing outside the window.
    pub(crate) fn coalition_invites_before_declaring(
        &mut self,
        g: &mut Game,
        pid: usize,
        target: usize,
    ) -> bool {
        if !self.coalition_before_war && !self.coalition_before_war_2 {
            return false;
        }
        let Some(coalition) = self.coalition.as_ref().filter(|c| c.target == target) else {
            return false;
        };
        let turn = g.turn;
        let pending = g.pending_deals.iter().any(|deal| {
            deal.from == pid && deal.joint_war_target == Some(target) && deal.expires >= turn
        });
        if let Some(sent) = coalition.invited {
            if self.coalition_before_war_2 {
                // The invitation call already consumed one strike decision.
                // Keep it held only if this method is revisited in that same
                // turn; on the next turn declare instead of retrying or
                // extending the wait.
                return pending && turn == sent;
            }
            if pending && turn < sent + COALITION_JOINT_WAR_PATIENCE {
                *g.players[pid]
                    .counters
                    .entry("coalition:held_declarations".to_string())
                    .or_insert(0) += 1;
                if self.journal().wants(crate::reasoning::Level::Strategy) {
                    think!(self.journal(), Military, Strategy,
                           "Holding the declaration on {}", g.players[target].civ;
                           "a joint-war invitation is still unanswered; an accepted one \
                            opens the war on two fronts at once");
                }
                return true;
            }
            if turn < sent + COALITION_REASK_TURNS {
                return false;
            }
        }
        let mut partners: Vec<usize> =
            self.coalition_neighbours(g, pid, target)
                .into_iter()
                .filter(|partner| {
                    self.coalition_joint_war_partner(g, pid, *partner, target)
                        && (!self.coalition_before_war_2
                            || self.coalition_credible_partner(g, pid, *partner, target))
                        && !coalition
                            .joint_war_asked
                            .get(partner)
                            .is_some_and(|asked| turn < asked + COALITION_REASK_TURNS)
                        && !g.pending_deals.iter().any(|deal| {
                            deal.expires >= turn && deal.from == pid && deal.to == *partner
                        })
                })
                .collect();
        if self.coalition_before_war_2 {
            partners.sort_by(|left, right| {
                let grievance = |partner: usize| {
                    g.players[partner]
                        .grievances
                        .get(&target)
                        .copied()
                        .unwrap_or(0.0)
                };
                g.military_power(*right)
                    .total_cmp(&g.military_power(*left))
                    .then_with(|| grievance(*right).total_cmp(&grievance(*left)))
                    .then_with(|| left.cmp(right))
            });
            partners.truncate(1);
        }
        let mut invited = 0;
        for partner in partners {
            if g.apply(
                pid,
                &Action::ProposeJointWar {
                    player: partner,
                    target,
                },
            )
            .is_err()
            {
                continue;
            }
            invited += 1;
            if let Some(coalition) = self.coalition.as_mut() {
                coalition.joint_war_asked.insert(partner, turn);
            }
            if self.journal().wants(crate::reasoning::Level::Decision) {
                think!(self.journal(), Diplomacy, Decision,
                       "Inviting {} to a joint war on {}",
                       g.players[partner].civ, g.players[target].civ;
                       "they border the target; if they accept, both of us declare at once");
            }
        }
        if let Some(coalition) = self.coalition.as_mut() {
            coalition.invited = Some(turn);
        }
        if invited == 0 {
            return false;
        }
        *g.players[pid]
            .counters
            .entry("coalition:joint_wars_proposed".to_string())
            .or_insert(0) += invited;
        *g.players[pid]
            .counters
            .entry("coalition:held_declarations".to_string())
            .or_insert(0) += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::opt_in_off_in_both_controllers;
    use super::super::{AdvancedAi, GrandStrategy, StrategicPlan};
    use super::*;
    use crate::game::{Action, Game};

    #[test]
    fn coalition_before_war_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("coalition-before-war", |ai| ai.coalition_before_war);
        opt_in_off_in_both_controllers("coalition-before-war-2", |ai| ai.coalition_before_war_2);

        let mut ai = AdvancedAi::new();
        ai.enable_coalition_before_war();
        assert!(ai.coalition_before_war);
        assert!(!ai.coalition_before_war_2);
        ai.enable_coalition_before_war_2();
        assert!(!ai.coalition_before_war);
        assert!(ai.coalition_before_war_2);
    }

    /// Open land at exactly `distance` from `anchor`, for seating a city.
    fn open_ground_at(game: &Game, anchor: crate::Pos, distance: i32) -> crate::Pos {
        game.map
            .tiles
            .keys()
            .copied()
            .filter(|position| {
                game.wdist(*position, anchor) == distance
                    && game.rules.is_passable(&game.map.tiles[position])
                    && !game.rules.is_water(&game.map.tiles[position])
                    && game.map.tiles[position].owner_city.is_none()
                    && game.city_at(*position).is_none()
            })
            .min()
            .expect("open ground at the distance")
    }

    /// Three majors with a capital each and two city-states. Major 1 is the
    /// target; major 2 is seated as its neighbour (a second city six tiles
    /// from the target's capital); the first city-state four tiles from the
    /// target's capital, the second as far from it as the map allows.
    fn coalition_board() -> (Game, StrategicPlan, usize, usize) {
        let mut game = Game::new_full(3, 24, 16, 7_923, 300, 2, false);
        for pid in 0..3 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .expect("every major starts with a settler");
            game.found_city_for(pid, game.units[&settler].pos, None);
            game.remove_unit(settler);
        }
        for pid in 0..3 {
            for other in pid + 1..3 {
                game.record_contact(pid, other);
            }
        }
        for player in game.players.iter_mut().filter(|p| !p.is_minor) {
            player.civics.insert(crate::name!("civil_service"));
            player.civics.insert(crate::name!("foreign_trade"));
        }
        let target_capital = game.cities[&game.player_city_ids(1)[0]].pos;
        let border = open_ground_at(&game, target_capital, 6);
        game.found_city_for(2, border, None);
        let minors: Vec<usize> = game
            .players
            .iter()
            .filter(|p| p.is_minor && !p.is_barbarian)
            .map(|p| p.id)
            .collect();
        assert_eq!(minors.len(), 2);
        for minor in &minors {
            for cid in game.player_city_ids(*minor) {
                game.cities.remove(&cid);
            }
            for unit in game.player_unit_ids(*minor) {
                game.remove_unit(unit);
            }
        }
        let near_seat = open_ground_at(&game, target_capital, 4);
        game.found_city_for(minors[0], near_seat, None);
        let farthest = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|position| {
                game.rules.is_passable(&game.map.tiles[position])
                    && !game.rules.is_water(&game.map.tiles[position])
                    && game.map.tiles[position].owner_city.is_none()
                    && game.city_at(*position).is_none()
            })
            .max_by_key(|position| (game.wdist(*position, target_capital), *position))
            .expect("land somewhere far");
        assert!(game.wdist(farthest, target_capital) > COALITION_CITY_STATE_RADIUS);
        game.found_city_for(minors[1], farthest, None);
        for pid in 0..3 {
            game.players[pid].met.insert(minors[0]);
            game.players[pid].met.insert(minors[1]);
            game.players[minors[0]].met.insert(pid);
            game.players[minors[1]].met.insert(pid);
        }
        game.turn = 60;
        game.current = 0;
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(game.player_city_ids(1)[0]),
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        (game, plan, minors[0], minors[1])
    }

    fn coalition_ai() -> AdvancedAi {
        let mut ai = AdvancedAi::new();
        ai.enable_coalition_before_war();
        ai
    }

    fn coalition_v2_ai() -> AdvancedAi {
        let mut ai = AdvancedAi::new();
        ai.enable_coalition_before_war_2();
        ai
    }

    /// The window opens on a peaceful Conquest target, stays shut with the
    /// gene off, and closes the turn the war starts.
    #[test]
    fn the_window_opens_on_a_peaceful_target_and_closes_at_war() {
        let (mut game, plan, _, _) = coalition_board();
        let mut stock = AdvancedAi::new();
        assert_eq!(stock.coalition_target(&game, 0, &plan), None);
        stock.coalition_observe(&mut game, 0, &plan);
        assert!(stock.coalition.is_none());

        let mut ai = coalition_ai();
        assert_eq!(ai.coalition_target(&game, 0, &plan), Some(1));
        ai.coalition_observe(&mut game, 0, &plan);
        let opened = ai.coalition.clone().expect("the window is open");
        assert_eq!((opened.target, opened.opened), (1, 60));
        assert_eq!(game.players[0].counters.get("coalition:opened"), Some(&1));
        assert_eq!(ai.coalition_neighbours(&game, 0, 1), vec![2]);

        // A second observation of the same target keeps the window.
        game.turn = 61;
        ai.coalition_observe(&mut game, 0, &plan);
        assert_eq!(ai.coalition.as_ref().map(|c| c.opened), Some(60));

        // A plan that is not a war has no window; the war's start closes it.
        let peaceful = StrategicPlan {
            strategy: GrandStrategy::Science,
            ..plan.clone()
        };
        assert_eq!(ai.coalition_target(&game, 0, &peaceful), None);
        game.at_war.insert((0, 1));
        assert_eq!(ai.coalition_target(&game, 0, &plan), None);
        ai.coalition_observe(&mut game, 0, &plan);
        assert!(ai.coalition.is_none());
    }

    /// One military alliance is proposed to the target's neighbour, with
    /// friendship bundled; never to the target, never twice while the first
    /// is pending, and never with the gene off.
    #[test]
    fn an_alliance_is_proposed_to_the_targets_neighbour_and_never_to_the_target() {
        let (mut game, plan, _, _) = coalition_board();
        let mut stock = AdvancedAi::new();
        stock.coalition_observe(&mut game, 0, &plan);
        stock.coalition_alliance_step(&mut game, 0);
        assert!(game.pending_deals.is_empty());

        let mut ai = coalition_ai();
        ai.coalition_observe(&mut game, 0, &plan);
        ai.coalition_alliance_step(&mut game, 0);
        let proposals: Vec<_> = game
            .pending_deals
            .iter()
            .filter(|deal| deal.from == 0)
            .cloned()
            .collect();
        assert_eq!(proposals.len(), 1);
        let proposal = &proposals[0];
        assert_eq!(proposal.to, 2);
        assert_eq!(proposal.alliance.as_deref(), Some("military"));
        assert!(proposal.friendship);
        assert_eq!(
            game.players[0].counters.get("coalition:alliances_proposed"),
            Some(&1)
        );
        assert_eq!(
            ai.coalition.as_ref().and_then(|c| c.alliance_asked.get(&2)),
            Some(&60)
        );
        // Pending: no second proposal this turn.
        ai.coalition_alliance_step(&mut game, 0);
        assert_eq!(
            game.pending_deals
                .iter()
                .filter(|deal| deal.from == 0)
                .count(),
            1
        );

        // The neighbour accepts: friendship and a military alliance stand,
        // and the desk asks for nothing more of them.
        let id = proposal.id;
        game.current = 2;
        game.apply(2, &Action::AcceptDeal { deal: id })
            .expect("the neighbour accepts");
        game.current = 0;
        assert_eq!(
            game.alliance_with(0, 2)
                .map(|alliance| alliance.kind.as_str()),
            Some("military")
        );
        assert!(game.are_friends(0, 2));
        game.turn = 75;
        ai.coalition_alliance_step(&mut game, 0);
        assert!(game.pending_deals.iter().all(|deal| deal.from != 0));
    }

    /// A refused neighbour is asked again only after the re-ask interval,
    /// and the kind falls to the next free one when `military` is taken.
    #[test]
    fn a_refusal_waits_for_the_reask_interval_and_a_taken_kind_falls_through() {
        let (mut game, plan, _, _) = coalition_board();
        let mut ai = coalition_ai();
        ai.coalition_observe(&mut game, 0, &plan);
        ai.coalition_alliance_step(&mut game, 0);
        let id = game
            .pending_deals
            .iter()
            .find(|deal| deal.from == 0)
            .unwrap()
            .id;
        game.current = 2;
        game.apply(2, &Action::RejectDeal { deal: id })
            .expect("refused");
        game.current = 0;
        assert!(game.pending_deals.iter().all(|deal| deal.from != 0));
        game.turn = 60 + COALITION_REASK_TURNS - 1;
        ai.coalition_alliance_step(&mut game, 0);
        assert!(game.pending_deals.iter().all(|deal| deal.from != 0));
        // Meanwhile our military alliance went to somebody else: the next
        // free kind is asked for.
        game.players[0].alliances.insert(
            1,
            crate::game::AllianceState {
                kind: "military".to_string(),
                points: 0.0,
                level: 1,
                ends: game.turn + 30,
            },
        );
        game.turn = 60 + COALITION_REASK_TURNS;
        ai.coalition_alliance_step(&mut game, 0);
        let proposal = game
            .pending_deals
            .iter()
            .find(|deal| deal.from == 0)
            .unwrap();
        assert_eq!(proposal.to, 2);
        assert_eq!(proposal.alliance.as_deref(), Some("economic"));
    }

    /// The envoy term: a city-state beside the target is worth envoys, most
    /// when the target holds it, nothing beyond the radius, nothing outside
    /// the window, nothing with the gene off.
    #[test]
    fn a_city_state_beside_the_target_is_worth_envoys_most_when_the_target_holds_it() {
        let (mut game, plan, near, far) = coalition_board();
        let mut stock = AdvancedAi::new();
        stock.coalition_observe(&mut game, 0, &plan);
        assert_eq!(stock.coalition_city_state_bonus(&game, 0, near, 2), 0);

        let mut ai = coalition_ai();
        // No window yet: nothing.
        assert_eq!(ai.coalition_city_state_bonus(&game, 0, near, 2), 0);
        ai.coalition_observe(&mut game, 0, &plan);
        let open = ai.coalition_city_state_bonus(&game, 0, near, 2);
        assert_eq!(
            open,
            (i64::from(COALITION_CITY_STATE_RADIUS + 1 - 4) * COALITION_CITY_STATE_PER_TILE
                + COALITION_OPEN_CITY_STATE)
                / 2
        );
        assert_eq!(ai.coalition_city_state_bonus(&game, 0, far, 2), 0);

        // The target takes the suzerainty: its client, our front.
        game.players[1].envoys_free = 3;
        for _ in 0..3 {
            game.current = 1;
            game.apply(1, &Action::SendEnvoy { player: near })
                .expect("the target places an envoy");
        }
        game.current = 0;
        assert_eq!(game.suzerain_of(near), Some(1));
        let client = ai.coalition_city_state_bonus(&game, 0, near, 2);
        assert!(client > open, "{client} > {open}");
        assert_eq!(
            client,
            (i64::from(COALITION_CITY_STATE_RADIUS + 1 - 4) * COALITION_CITY_STATE_PER_TILE
                + COALITION_TARGET_CLIENT)
                / 2
        );
        // Amortised over the envoys still needed.
        assert_eq!(
            ai.coalition_city_state_bonus(&game, 0, near, 4),
            client * 2 / 4
        );

        // A third party's client is proximity only.
        game.players[2].envoys_free = 4;
        for _ in 0..4 {
            game.current = 2;
            game.apply(2, &Action::SendEnvoy { player: near })
                .expect("the neighbour places an envoy");
        }
        game.current = 0;
        assert_eq!(game.suzerain_of(near), Some(2));
        assert_eq!(
            ai.coalition_city_state_bonus(&game, 0, near, 2),
            i64::from(COALITION_CITY_STATE_RADIUS + 1 - 4) * COALITION_CITY_STATE_PER_TILE / 2
        );
    }

    /// Version two refuses speculative setup, then asks a credible partner
    /// only for the military alliance and values only a target-held client.
    /// At the strike it invites once and declares next turn after a refusal.
    #[test]
    fn v2_limits_setup_and_invites_one_credible_partner_once() {
        let (mut game, plan, near, _) = coalition_board();
        let mut ai = coalition_v2_ai();
        ai.coalition_observe(&mut game, 0, &plan);

        // A merely legal neighbour gets no speculative alliance, and an open
        // city-state gets no proximity-only Envoy premium.
        ai.coalition_alliance_step(&mut game, 0);
        assert!(game.pending_deals.is_empty());
        assert_eq!(ai.coalition_city_state_bonus(&game, 0, near, 1), 0);
        assert!(!ai.coalition_credible_partner(&game, 0, 2, 1));

        // Unseating the target from its own nearby client is direct value.
        game.players[1].envoys_free = 3;
        for _ in 0..3 {
            game.current = 1;
            game.apply(1, &Action::SendEnvoy { player: near })
                .expect("the target places an envoy");
        }
        game.current = 0;
        assert_eq!(game.suzerain_of(near), Some(1));
        assert!(ai.coalition_city_state_bonus(&game, 0, near, 1) > 0);

        // A resentful neighbour with enough combined power is credible. V2
        // asks it only for the military alliance, never a fallback kind.
        game.players[2].grievances.insert(1, 40.0);
        let home = game.cities[&game.player_city_ids(2)[0]].pos;
        for _ in 0..6 {
            game.spawn_test_unit("warrior", 2, home);
        }
        assert!(ai.coalition_credible_partner(&game, 0, 2, 1));
        ai.coalition_alliance_step(&mut game, 0);
        let alliance = game
            .pending_deals
            .iter()
            .find(|deal| deal.from == 0 && deal.to == 2)
            .expect("the credible neighbour gets an alliance proposal");
        assert_eq!(alliance.alliance.as_deref(), Some("military"));
        let alliance_id = alliance.id;
        game.current = 2;
        game.apply(2, &Action::RejectDeal { deal: alliance_id })
            .expect("the alliance can be refused");
        game.current = 0;

        assert!(ai.coalition_invites_before_declaring(&mut game, 0, 1));
        let invitations: Vec<_> = game
            .pending_deals
            .iter()
            .filter(|deal| deal.from == 0 && deal.joint_war_target == Some(1))
            .collect();
        assert_eq!(invitations.len(), 1);
        let id = invitations[0].id;

        game.current = 2;
        game.apply(2, &Action::RejectDeal { deal: id })
            .expect("the invitation can be refused");
        game.current = 0;
        game.turn += 1;
        assert!(!ai.coalition_invites_before_declaring(&mut game, 0, 1));
        assert!(game.pending_deals.iter().all(|deal| deal.from != 0));
    }

    /// The strike: every eligible neighbour is invited to a joint war and the
    /// declaration held while the answer is due; refused, the desk declares
    /// alone; nothing with the gene off.
    #[test]
    fn the_strike_invites_the_neighbours_and_holds_for_the_answer() {
        let (mut game, plan, _, _) = coalition_board();
        let mut stock = AdvancedAi::new();
        stock.coalition_observe(&mut game, 0, &plan);
        assert!(!stock.coalition_invites_before_declaring(&mut game, 0, 1));
        assert!(game.pending_deals.is_empty());

        let mut ai = coalition_ai();
        ai.coalition_observe(&mut game, 0, &plan);
        assert!(ai.coalition_invites_before_declaring(&mut game, 0, 1));
        let invitations: Vec<_> = game
            .pending_deals
            .iter()
            .filter(|deal| deal.from == 0 && deal.joint_war_target == Some(1))
            .collect();
        assert_eq!(invitations.len(), 1);
        assert_eq!(invitations[0].to, 2);
        let id = invitations[0].id;
        assert_eq!(
            game.players[0]
                .counters
                .get("coalition:joint_wars_proposed"),
            Some(&1)
        );
        assert_eq!(
            game.players[0].counters.get("coalition:held_declarations"),
            Some(&1)
        );
        // Still unanswered next turn: held once more, no second invitation.
        game.turn = 61;
        assert!(ai.coalition_invites_before_declaring(&mut game, 0, 1));
        assert_eq!(
            game.pending_deals
                .iter()
                .filter(|deal| deal.from == 0)
                .count(),
            1
        );
        assert_eq!(
            game.players[0].counters.get("coalition:held_declarations"),
            Some(&2)
        );
        // Refused: the desk declares alone, and does not ask again yet.
        game.current = 2;
        game.apply(2, &Action::RejectDeal { deal: id })
            .expect("refused");
        game.current = 0;
        assert!(!ai.coalition_invites_before_declaring(&mut game, 0, 1));
        assert!(game.pending_deals.iter().all(|deal| deal.from != 0));
        // Patience runs out on an unanswered invitation too.
        game.turn = 60 + COALITION_REASK_TURNS;
        assert!(ai.coalition_invites_before_declaring(&mut game, 0, 1));
        let again = game
            .pending_deals
            .iter()
            .find(|deal| deal.from == 0)
            .unwrap();
        assert_eq!(again.joint_war_target, Some(1));
        game.turn += COALITION_JOINT_WAR_PATIENCE;
        assert!(!ai.coalition_invites_before_declaring(&mut game, 0, 1));
    }

    /// An accepted invitation starts the war for both of us at once.
    #[test]
    fn an_accepted_invitation_opens_the_war_on_two_fronts() {
        let (mut game, plan, _, _) = coalition_board();
        // A neighbour who resents the target and, with us, outweighs it.
        game.players[2].grievances.insert(1, 40.0);
        let home = game.cities[&game.player_city_ids(2)[0]].pos;
        for _ in 0..3 {
            game.spawn_test_unit("warrior", 2, home);
        }
        let mut ai = coalition_ai();
        ai.coalition_observe(&mut game, 0, &plan);
        assert!(ai.coalition_invites_before_declaring(&mut game, 0, 1));
        let id = game
            .pending_deals
            .iter()
            .find(|deal| deal.from == 0)
            .unwrap()
            .id;
        game.current = 2;
        game.apply(2, &Action::AcceptDeal { deal: id })
            .expect("the neighbour accepts the joint war");
        game.current = 0;
        assert!(game.is_at_war(0, 1));
        assert!(game.is_at_war(2, 1));
        assert!(!game.is_at_war(0, 2));
        // The window closes with the war on.
        ai.coalition_observe(&mut game, 0, &plan);
        assert!(ai.coalition.is_none());
        assert!(!ai.coalition_invites_before_declaring(&mut game, 0, 1));
    }
}
