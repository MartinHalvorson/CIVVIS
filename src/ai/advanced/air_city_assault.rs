//! A bounded city assault: preserve sight and a cavalry exit around the volley.
use super::{battle_planner, AdvancedAi, GrandStrategy, StrategicPlan};
use crate::game::{Action, Game};
use crate::Pos;
use std::collections::BTreeSet;

/// A city at or below this many hit points behind fallen walls is a breach
/// any melee body can finish.
///
/// ★★★ A BOMBED CITY NEVER READS ZERO. `do_air_strike` (and every ranged
/// city strike) floors City Center health at one, exactly as the host does,
/// so the breach test used to read `hp <= 0` and could only match a city a
/// melee blow had already emptied. Live King 20260930T211803Z: Edirne sat at
/// `walls 0/400, city 1/200` from turn 188 to 191 with four Bombers overhead
/// and nobody finished it. The capture simulation still decides whether the
/// blow actually takes the city; this only lets the search look.
const AIR_ASSAULT_BREACH_HP: i32 = 1;
/// A breach may be finished by any healthy land melee body, not only the
/// cavalry the volley maneuver keeps for its spotting move.
const AIR_ASSAULT_BREACH_TAKER_HP: i32 = 30;
/// How far a capture body will look for a breach it can finish this turn.
const AIR_ASSAULT_TAKER_REACH: i32 = 6;
/// How near a healthy land melee body must stand for a volley with no
/// cavalry to be worth flying: about two turns' march. Live King
/// 2026-10-03T131343Z bombed Sheffield to one health at turn 179 ("1
/// sorties with no cavalry in reach; walls 0, city 1; captured false")
/// with nobody coming, and the city healed; Coba took the same at turn 193.
const AIR_ASSAULT_FOLLOWUP_REACH: i32 = 8;
/// How many takers, strongest first, the capture search simulates.
const AIR_ASSAULT_CAPTURE_TRIES: usize = 6;
/// Aircraft one volley may commit to the city.
const AIR_ASSAULT_MAX_SORTIES: usize = 8;

/// The chosen maneuver on a disposable planning board. Native adapters must
/// observe the spot and the volley before releasing the dependent phase.
#[derive(Clone, Debug)]
pub struct AirCityAssault {
    pub target: Pos,
    /// The capture or spotting body, if the maneuver had one. A volley the
    /// wing flies at a visible city with no body in reach has none.
    pub cavalry: Option<u32>,
    pub spot: Pos,
    pub moved_to_spot: bool,
    pub aircraft: Vec<u32>,
}

struct Opening {
    cavalry: u32,
    origin: Pos,
    spot: Pos,
    actions: Vec<Action>,
    board: Game,
    spent: f64,
}

fn walk(g: &mut Game, pid: usize, uid: u32, to: Pos) -> Option<Action> {
    let action = Action::MoveTo { unit: uid, to };
    g.apply(pid, &action).ok()?;
    (g.units.get(&uid)?.pos == to).then_some(action)
}

impl AdvancedAi {
    /// Set by a host adapter for each fresh board, before any speculation.
    pub fn observe_air_assault_frame(&mut self, visible: BTreeSet<Pos>, frames_left: u32) {
        self.air_assault_observation = Some((visible, frames_left));
    }

    fn air_assault_target_visible(&self, g: &Game, pid: usize, target: Pos) -> bool {
        self.air_assault_observation.as_ref().map_or_else(
            || g.player_can_see(pid, target),
            |(visible, _)| visible.contains(&target),
        )
    }

    pub fn planned_air_city_assault(&self) -> Option<&AirCityAssault> {
        self.air_city_assault.as_ref()
    }

    /// Run after the immediate-kill prepass, before individual units spend
    /// their whole turns. Only the opt-in air surge owns this maneuver.
    pub(super) fn plan_air_city_assault(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> BTreeSet<u32> {
        self.air_city_assault = None;
        let mut reserved = BTreeSet::new();
        if !self.air_surge_enabled() || plan.strategy != GrandStrategy::Conquest {
            return reserved;
        }
        // A sortie can leave a city defenseless on one host frame, then the
        // fresh strategic plan can select the next capital before the cavalry
        // receives its deferred order. Finish the observed breach first.
        let mut breached: Vec<u32> = g
            .cities
            .values()
            .filter(|city| {
                city.owner != pid
                    && g.is_at_war(pid, city.owner)
                    && city.hp <= AIR_ASSAULT_BREACH_HP
                    && city.wall_hp <= 0
                    && self.air_assault_target_visible(g, pid, city.pos)
            })
            .map(|city| city.id)
            .collect();
        breached.sort_by_key(|cid| {
            let city = &g.cities[cid];
            (Some(*cid) != plan.target_city, !city.is_capital, *cid)
        });
        for cid in breached {
            let target = g.cities[&cid].pos;
            let takers = self.air_assault_takers(g, pid, target);
            if takers.is_empty() {
                crate::think!(self.journal(), Military, Detail,
                    "Breach at {} waits for a body", g.cities[&cid].name;
                    "{} health behind fallen walls; no healthy melee unit with moves stands within {} tiles",
                    g.cities[&cid].hp, AIR_ASSAULT_TAKER_REACH);
                continue;
            }
            let Some((taker, actions)) = self.air_assault_best_capture(g, pid, cid, &takers) else {
                // ★★ THE BREACH WAS GUARDED, NOT FAR. Live King
                // 20260930T211803Z, Edirne at one health turns 188-191: the
                // approach tiles stood in 200-459 danger from the Ottoman
                // army around the city, and the wing spent its sorties
                // holding a one-health city at one. Strike the defenders on
                // the approach instead, then look for the capture again.
                if let Some((sorties, struck, _)) =
                    self.air_assault_clear_approach(g, pid, plan, cid, target)
                {
                    reserved.extend(sorties.iter().copied());
                    let takers = self.air_assault_takers(g, pid, target);
                    if let Some((taker, capture)) = self
                        .air_assault_best_capture(g, pid, cid, &takers)
                        .filter(|(taker, _)| self.capture_holdable(g, pid, cid, *taker))
                    {
                        let origin = g.units[&taker].pos;
                        if capture.iter().all(|action| g.apply(pid, action).is_ok()) {
                            self.resolve_city_dispositions(g, pid, plan.strategy);
                            reserved.insert(taker);
                            crate::think!(self.journal(), Military, Decision,
                                "Bombers clear the approach to {}", g.cities.get(&cid).map_or("the city", |city| city.name.as_str());
                                "{} sorties on {} defenders; {} walks in from {} tiles; city captured {}",
                                sorties.len(), struck,
                                crate::reasoning::plain(g.units.get(&taker).map_or("unit", |unit| unit.kind.as_str())),
                                g.wdist(origin, target),
                                g.cities.get(&cid).is_some_and(|city| city.owner == pid));
                            return reserved;
                        }
                    }
                    crate::think!(self.journal(), Military, Decision,
                        "Bombers clear the approach to {}", g.cities[&cid].name;
                        "{} sorties on {} defenders around a breach nobody can finish yet",
                        sorties.len(), struck);
                }
                // Nobody can finish it this turn. The walls are down, so the
                // city cannot strike the approach: close the nearest body in
                // so the next board's capture is one step, and keep it out of
                // the ordinary loop that would walk it back to an anchor.
                let takers = self.air_assault_takers(g, pid, target);
                if takers.is_empty() {
                    continue;
                }
                let nearest = *takers
                    .iter()
                    .min_by_key(|uid| (g.wdist(g.units[*uid].pos, target), **uid))
                    .expect("takers is not empty");
                let mut board = g.speculative_clone();
                let advanced = self
                    .air_assault_advance(&mut board, pid, nearest, target, true)
                    .filter(|action| g.apply(pid, action).is_ok());
                crate::think!(self.journal(), Military, Detail,
                    "Breach at {} cannot be finished this turn", g.cities[&cid].name;
                    "{} bodies within {} tiles, none with a ring step whose blow takes the city; the nearest {}",
                    takers.len(), AIR_ASSAULT_TAKER_REACH,
                    if advanced.is_some() { "closes in" } else { "has no safe step closer" });
                if advanced.is_some() {
                    reserved.insert(nearest);
                }
                continue;
            };
            // `capture-holds-the-ring`: see `capture_holdable`.
            if !self.capture_holdable(g, pid, cid, taker) {
                crate::think!(self.journal(), Military, Decision,
                    "Holding off the capture of {}", g.cities[&cid].name;
                    "a hostile that can retake it stands within two tiles and too few of ours could end the turn beside it";
                    target);
                continue;
            }
            let origin = g.units[&taker].pos;
            if actions.iter().all(|action| g.apply(pid, action).is_ok()) {
                self.resolve_city_dispositions(g, pid, plan.strategy);
                reserved.insert(taker);
                crate::think!(self.journal(), Military, Decision,
                    "Finishing the breach at {}", g.cities.get(&cid).map_or("the city", |city| city.name.as_str());
                    "{} {} walks in from {} tiles; city captured {}",
                    crate::reasoning::plain(g.units.get(&taker).map_or("unit", |unit| unit.kind.as_str())),
                    taker, g.wdist(origin, target),
                    g.cities.get(&cid).is_some_and(|city| city.owner == pid));
                self.air_city_assault = Some(AirCityAssault {
                    target,
                    cavalry: Some(taker),
                    spot: origin,
                    moved_to_spot: false,
                    aircraft: Vec::new(),
                });
                return reserved;
            }
        }
        let Some(city) = plan.target_city.and_then(|id| g.cities.get(&id)) else {
            return reserved;
        };
        if city.owner == pid || !g.is_at_war(pid, city.owner) {
            return reserved;
        }
        let (cid, target) = (city.id, city.pos);
        let aircraft: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                spec.domain.as_deref() == Some("air")
                    && spec.siege
                    && unit.hp >= 50
                    && g.wdist(unit.pos, target) <= g.unit_attack_range(*uid)
            })
            .take(AIR_ASSAULT_MAX_SORTIES)
            .collect();
        if aircraft.is_empty() {
            return reserved;
        }
        let visible = self.air_assault_target_visible(g, pid, target);
        let ready = aircraft.iter().any(|uid| {
            let unit = &g.units[uid];
            unit.moves_left > 0.0
                && !unit.acted
                && !g.strike_blocked(*uid, target)
                && (!visible || self.air_strike_value(g, pid, *uid, target, plan) > 0.0)
        });
        // After an observed volley, the aircraft may be spent but the cavalry
        // still has its capture/retreat decision. Never scout for spent planes.
        if !ready && !visible {
            return reserved;
        }
        if ready
            && self
                .air_assault_observation
                .as_ref()
                .is_some_and(|(_, left)| *left < if visible { 1 } else { 2 })
        {
            return reserved;
        }
        let Some(mut opening) = self.air_assault_opening(g, pid, target, ready) else {
            // ★★★ NO CAVALRY, NO VOLLEY — AND THE WING WENT ELSEWHERE. Live King
            // 20260930T211803Z turn 184, frame 0: Edirne the objective, four
            // Bombers in range at full health worth 92 a sortie, and no healthy
            // cavalry within six tiles yet, so this returned before a single
            // sortie and the unit loop spent all four on field units. Frame 1's
            // volley was then refused by the host as second strikes. Cavalry
            // is needed to see a hidden city and to take a breached one; the
            // walls of a city the empire can see fall to the wing alone.
            if ready && visible && self.air_surge_2 && Self::air_assault_followup_near(g, pid, target)
            {
                return self.air_assault_volley(g, pid, plan, cid, target, &aircraft, reserved);
            }
            return reserved;
        };
        let mut sorties = Vec::new();
        for uid in aircraft {
            if opening.board.strike_blocked(uid, target) {
                continue;
            }
            let value = self.air_strike_value(&opening.board, pid, uid, target, plan);
            if value <= 0.0 || !value.is_finite() {
                continue;
            }
            let action = Action::AirStrike { unit: uid, target };
            if opening.board.apply(pid, &action).is_ok() {
                opening.actions.push(action);
                sorties.push(uid);
            }
        }
        if ready && sorties.is_empty() {
            return reserved;
        }
        let mut uid = opening.cavalry;
        let mut capture = self.air_assault_capture(&opening.board, pid, uid, cid);
        if capture.is_none() {
            // The spotter is only the nearest cavalry. After the volley, the
            // strongest body in reach may still take what it cannot: a city's
            // melee defence follows its owner's best unit, and a Cuirassier
            // that dies on a one-health Edirne is not the only body there.
            let others: Vec<u32> = self
                .air_assault_takers(&opening.board, pid, target)
                .into_iter()
                .filter(|other| *other != uid)
                .collect();
            if let Some((other, actions)) =
                self.air_assault_best_capture(&opening.board, pid, cid, &others)
            {
                uid = other;
                capture = Some(actions);
            }
        }
        let breached_after_sorties = opening
            .board
            .cities
            .get(&cid)
            .is_some_and(|city| city.owner != pid && city.wall_hp <= 0);
        // The volley leaves the city at one health, and Bombers that did not
        // fly at it (a one-health city is worth no further sortie) strike the
        // defenders on its approach before the capture is tried again.
        if capture.is_none() && breached_after_sorties {
            if let Some((cleared, _, actions)) =
                self.air_assault_clear_approach(&mut opening.board, pid, plan, cid, target)
            {
                opening.actions.extend(actions);
                sorties.extend(cleared);
                let takers = self.air_assault_takers(&opening.board, pid, target);
                if let Some((other, actions)) =
                    self.air_assault_best_capture(&opening.board, pid, cid, &takers)
                {
                    uid = other;
                    capture = Some(actions);
                }
            }
        }
        if let Some(actions) = capture {
            opening.actions.extend(actions);
        } else if let Some(action) = breached_after_sorties
            .then(|| self.air_assault_advance(&mut opening.board, pid, uid, target, false))
            .flatten()
        {
            // The walls are down, so the city cannot strike the approach, and
            // a body that waits out of reach lets the city heal twenty a turn
            // while the wing spends its sorties holding it at one. Close in
            // so the next board's capture is a single step.
            opening.actions.push(action);
        } else if opening.spot != opening.origin {
            // The opening already proved this return affordable before any
            // bomber was spent. Recheck against the board after the strikes.
            let Some(action) = walk(&mut opening.board, pid, uid, opening.origin) else {
                return reserved;
            };
            opening.actions.push(action);
        } else if let Some(action) = self.air_assault_withdraw(&mut opening.board, pid, uid) {
            opening.actions.push(action);
        } else if sorties.is_empty() {
            return reserved;
        }
        // A body the maneuver gave no step keeps its turn for the siege and
        // the battle planner. Reserving it idle is how a capture body sat six
        // tiles from a one-health Edirne for four turns.
        let cavalry_acted = opening.actions.iter().any(|action| {
            matches!(action, Action::MoveTo { unit, .. } | Action::Attack { unit, .. } if *unit == uid)
        });
        let report = AirCityAssault {
            target,
            cavalry: Some(uid),
            spot: opening.spot,
            moved_to_spot: opening.spot != opening.origin
                || opening.actions.iter().any(|action| {
                    matches!(action, Action::MoveTo { unit, to } if *unit == opening.cavalry && *to == opening.spot)
                }),
            aircraft: sorties,
        };
        // All steps were checked together on one branch; replay only those
        // selected steps so unrelated decisions retain the real action log.
        for action in &opening.actions {
            if g.apply(pid, action).is_err() {
                break;
            }
        }
        // Capturing in a prepass opens the city's disposition prompt. Resolve
        // it through the existing policy before the remaining units act;
        // otherwise every later attack is rejected by the pending prompt.
        self.resolve_city_dispositions(g, pid, plan.strategy);
        if cavalry_acted {
            reserved.insert(uid);
        }
        reserved.extend(report.aircraft.iter().copied());
        crate::think!(self.journal(), Military, Decision,
            "Cavalry coordinates a bomber city assault";
            "cavalry {}; spotting move {}; {} sorties; city captured {}",
            uid, report.moved_to_spot, report.aircraft.len(),
            g.cities.get(&cid).is_some_and(|city| city.owner == pid));
        self.air_city_assault = Some(report);
        reserved
    }

    /// `air-surge-2`: the wing strikes the enemy units standing within two
    /// tiles of a breached city, one sortie per aircraft. Returns the
    /// aircraft that flew, the defenders struck and the strike actions, all
    /// already applied to `g`.
    ///
    /// A Bomber's exchange with a healthy defender usually prices below zero
    /// on its own; a volley that opens the capture this turn is worth that
    /// cost. So the wing first tries every defender it can reach regardless
    /// of the exchange and keeps that volley only if a capture follows it;
    /// otherwise only blows worth their own exchange fly.
    fn air_assault_clear_approach(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        cid: u32,
        target: Pos,
    ) -> Option<(Vec<u32>, usize, Vec<Action>)> {
        if !self.air_surge_2 {
            return None;
        }
        let mut costly = g.speculative_clone();
        if let Some(volley) = self.air_assault_approach_volley(&mut costly, pid, plan, cid, target, true)
        {
            let takers = self.air_assault_takers(&costly, pid, target);
            if self.air_assault_best_capture(&costly, pid, cid, &takers).is_some() {
                for action in &volley.2 {
                    if g.apply(pid, action).is_err() {
                        break;
                    }
                }
                return Some(volley);
            }
        }
        self.air_assault_approach_volley(g, pid, plan, cid, target, false)
    }

    fn air_assault_approach_volley(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        cid: u32,
        target: Pos,
        regardless_of_exchange: bool,
    ) -> Option<(Vec<u32>, usize, Vec<Action>)> {
        let owner = g.cities.get(&cid)?.owner;
        let aircraft: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                spec.domain.as_deref() == Some("air")
                    && spec.siege
                    && unit.hp >= 50
                    && unit.moves_left > 0.0
                    && !unit.acted
            })
            .collect();
        if aircraft.is_empty() {
            return None;
        }
        let mut sorties = Vec::new();
        let mut struck = BTreeSet::new();
        let mut actions = Vec::new();
        for uid in aircraft.into_iter().take(AIR_ASSAULT_MAX_SORTIES) {
            // Re-read the ring each sortie: an earlier blow may have killed
            // its defender.
            let defenders: BTreeSet<Pos> = g
                .units
                .values()
                .filter(|unit| {
                    unit.owner != pid
                        && (unit.owner == owner || g.is_at_war(pid, unit.owner))
                        && g.rules.units[unit.kind].class == "military"
                        && g.wdist(unit.pos, target) <= 2
                        && unit.pos != target
                })
                .map(|unit| unit.pos)
                .collect();
            let best = defenders
                .iter()
                .filter(|pos| {
                    g.wdist(g.units[&uid].pos, **pos) <= g.unit_attack_range(uid)
                        && !g.strike_blocked(uid, **pos)
                })
                .map(|pos| (self.air_strike_value(g, pid, uid, *pos, plan), *pos))
                .filter(|(value, _)| value.is_finite() && (regardless_of_exchange || *value > 0.0))
                .max_by(|left, right| left.0.total_cmp(&right.0).then(right.1.cmp(&left.1)));
            let Some((_, pos)) = best else {
                continue;
            };
            let action = Action::AirStrike { unit: uid, target: pos };
            if g.apply(pid, &action).is_ok() {
                sorties.push(uid);
                struck.insert(pos);
                actions.push(action);
            }
        }
        (!sorties.is_empty()).then_some((sorties, struck.len(), actions))
    }

    /// The wing's sorties at a visible city with no spotting cavalry, then the
    /// strongest body in reach if the volley leaves a breach it can finish.
    #[allow(clippy::too_many_arguments)]
    fn air_assault_volley(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        cid: u32,
        target: Pos,
        aircraft: &[u32],
        mut reserved: BTreeSet<u32>,
    ) -> BTreeSet<u32> {
        let mut board = g.speculative_clone();
        let mut actions = Vec::new();
        let mut sorties = Vec::new();
        for uid in aircraft {
            if board.strike_blocked(*uid, target) {
                continue;
            }
            let value = self.air_strike_value(&board, pid, *uid, target, plan);
            if value <= 0.0 || !value.is_finite() {
                continue;
            }
            let action = Action::AirStrike { unit: *uid, target };
            if board.apply(pid, &action).is_ok() {
                actions.push(action);
                sorties.push(*uid);
            }
        }
        if sorties.is_empty() {
            return reserved;
        }
        let takers = self.air_assault_takers(&board, pid, target);
        let taker = self
            .air_assault_best_capture(&board, pid, cid, &takers)
            .map(|(taker, capture)| {
                actions.extend(capture);
                taker
            });
        for action in &actions {
            if g.apply(pid, action).is_err() {
                break;
            }
        }
        self.resolve_city_dispositions(g, pid, plan.strategy);
        reserved.extend(sorties.iter().copied());
        reserved.extend(taker);
        crate::think!(self.journal(), Military, Decision,
            "Bombers strike {} ahead of the capture body", g.cities.get(&cid).map_or("the city", |city| city.name.as_str());
            "{} sorties with no cavalry in reach; walls {}, city {}; captured {}",
            sorties.len(),
            g.cities.get(&cid).map_or(0, |city| city.wall_hp),
            g.cities.get(&cid).map_or(0, |city| city.hp),
            g.cities.get(&cid).is_some_and(|city| city.owner == pid));
        self.air_city_assault = Some(AirCityAssault {
            target,
            cavalry: taker,
            spot: target,
            moved_to_spot: false,
            aircraft: sorties,
        });
        reserved
    }

    fn air_assault_opening(
        &self,
        g: &Game,
        pid: usize,
        target: Pos,
        ready: bool,
    ) -> Option<Opening> {
        let visible = self.air_assault_target_visible(g, pid, target);
        let mut cavalry: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                spec.cavalry
                    && spec.is_melee_capable()
                    && unit.hp >= 60
                    && unit.moves_left > 0.0
                    && !unit.acted
                    && unit.linked_to.is_none()
                    && g.wdist(unit.pos, target) <= AIR_ASSAULT_TAKER_REACH
            })
            .collect();
        cavalry.sort_by_key(|uid| (g.wdist(g.units[uid].pos, target), *uid));
        let mut best: Option<Opening> = None;
        for uid in cavalry.into_iter().take(4) {
            let origin = g.units[&uid].pos;
            let mut spots = if visible {
                vec![origin]
            } else {
                g.reachable(uid)
            };
            spots.retain(|pos| {
                *pos == origin || (ready && (1..=2).contains(&g.wdist(*pos, target)))
            });
            spots.sort_by_key(|pos| (g.wdist(origin, *pos), *pos));
            for spot in spots.into_iter().take(12) {
                let mut board = g.speculative_clone();
                let mut actions = Vec::new();
                if spot != origin {
                    let Some(action) = walk(&mut board, pid, uid, spot) else {
                        continue;
                    };
                    actions.push(action);
                }
                if !board.player_can_see(pid, target) || board.units[&uid].moves_left <= 0.0 {
                    continue;
                }
                if spot != origin {
                    let mut returning = board.speculative_clone();
                    if walk(&mut returning, pid, uid, origin).is_none()
                        || battle_planner::strike_danger(&returning, pid, origin, uid)
                            >= returning.units[&uid].hp as f64 * 0.5
                    {
                        continue;
                    }
                }
                let spent = g.units[&uid].moves_left - board.units[&uid].moves_left;
                if best.as_ref().is_none_or(|old| {
                    spent.total_cmp(&old.spent).is_lt()
                        || (spent == old.spent
                            && (g.wdist(spot, target), uid, spot)
                                < (g.wdist(old.spot, target), old.cavalry, old.spot))
                }) {
                    best = Some(Opening {
                        cavalry: uid,
                        origin,
                        spot,
                        actions,
                        board,
                        spent,
                    });
                }
            }
        }
        if best.is_none() && !visible && ready && self.air_surge_2 {
            best = self.air_assault_standing_spotter(g, pid, target);
        }
        best
    }

    /// `air-surge-2`: with no cavalry to look and come back, any healthy land
    /// soldier near an unseen objective steps to the nearest tile that sees it
    /// and stays there, if the blows it takes there leave it half its health.
    ///
    /// ★★ THE WING NEVER SAW HASTINGS. Domination pair seed 37140004: the
    /// wing was re-aimed at Hastings on turn 164 with three Bombers in range,
    /// the land siege mustered three to four tiles out, beyond sight, for
    /// eighteen turns, and with no cavalry in reach not one sortie flew before
    /// peace closed the war. A volley needs the city in sight, not a horse.
    fn air_assault_standing_spotter(&self, g: &Game, pid: usize, target: Pos) -> Option<Opening> {
        let mut soldiers: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && unit.hp >= 60
                    && unit.moves_left > 0.0
                    && !unit.acted
                    && unit.linked_to.is_none()
                    && !g.is_embarked(unit)
                    && g.wdist(unit.pos, target) <= AIR_ASSAULT_TAKER_REACH
            })
            .collect();
        soldiers.sort_by_key(|uid| (g.wdist(g.units[uid].pos, target), *uid));
        for uid in soldiers.into_iter().take(4) {
            let origin = g.units[&uid].pos;
            // Nearest the city first: those are the tiles that see it.
            let mut spots = g.reachable(uid);
            spots.sort_by_key(|pos| (g.wdist(*pos, target), g.wdist(origin, *pos), *pos));
            for spot in spots.into_iter().take(16) {
                let mut board = g.speculative_clone();
                let Some(action) = walk(&mut board, pid, uid, spot) else {
                    continue;
                };
                if !board.player_can_see(pid, target)
                    || self.air_assault_danger(&board, pid, spot, uid)
                        >= f64::from(board.units[&uid].hp) * 0.5
                {
                    continue;
                }
                let spent = g.units[&uid].moves_left - board.units[&uid].moves_left;
                // It stays where it looked: the opening's origin is its spot,
                // so no return walk is owed.
                return Some(Opening {
                    cavalry: uid,
                    origin: spot,
                    spot,
                    actions: vec![action],
                    board,
                    spent,
                });
            }
        }
        None
    }

    /// Every land melee body that could finish a breach at `target` this
    /// turn, strongest first: a city's melee defence follows its owner's best
    /// unit, so the blow that survives it is the strongest one.
    /// A healthy land melee body within [`AIR_ASSAULT_FOLLOWUP_REACH`] of the
    /// city: someone can walk in on the breach before it heals.
    fn air_assault_followup_near(g: &Game, pid: usize, target: Pos) -> bool {
        g.player_unit_ids(pid).into_iter().any(|uid| {
            let unit = &g.units[&uid];
            let spec = &g.rules.units[unit.kind];
            spec.class == "military"
                && spec.is_melee_capable()
                && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                && unit.hp >= AIR_ASSAULT_BREACH_TAKER_HP
                && g.wdist(unit.pos, target) <= AIR_ASSAULT_FOLLOWUP_REACH
        })
    }

    fn air_assault_takers(&self, g: &Game, pid: usize, target: Pos) -> Vec<u32> {
        let mut takers: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                spec.class == "military"
                    && spec.is_melee_capable()
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && unit.hp >= AIR_ASSAULT_BREACH_TAKER_HP
                    && unit.moves_left > 0.0
                    && !unit.acted
                    && unit.linked_to.is_none()
                    && g.wdist(unit.pos, target) <= AIR_ASSAULT_TAKER_REACH
            })
            .collect();
        takers.sort_by(|left, right| {
            let blow = |uid: &u32| {
                let unit = &g.units[uid];
                g.unit_strength(unit, false) * f64::from(unit.hp.clamp(0, 100)) / 100.0
            };
            blow(right)
                .total_cmp(&blow(left))
                .then_with(|| g.wdist(g.units[left].pos, target).cmp(&g.wdist(g.units[right].pos, target)))
                .then_with(|| left.cmp(right))
        });
        takers
    }

    /// The first taker, strongest first, whose ring step and blow take the
    /// city on this board. Bounded: each try clones the board per ring tile.
    fn air_assault_best_capture(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        takers: &[u32],
    ) -> Option<(u32, Vec<Action>)> {
        takers
            .iter()
            .take(AIR_ASSAULT_CAPTURE_TRIES)
            .find_map(|uid| self.air_assault_capture(g, pid, *uid, cid).map(|actions| (*uid, actions)))
    }

    fn air_assault_capture(&self, g: &Game, pid: usize, uid: u32, cid: u32) -> Option<Vec<Action>> {
        if Self::should_defer_city_capture(g, pid, cid) {
            return None;
        }
        let target = g.cities[&cid].pos;
        let mut ring = g.wdisk(target, 1);
        ring.retain(|pos| *pos != target);
        ring.sort_by_key(|pos| (g.wdist(g.units[&uid].pos, *pos), *pos));
        for stand in ring {
            let mut after = g.speculative_clone();
            let mut actions = Vec::new();
            if stand != after.units[&uid].pos {
                let Some(action) = walk(&mut after, pid, uid, stand) else {
                    continue;
                };
                actions.push(action);
            }
            // Even a city at zero HP is still an enemy city. Ordinary movement
            // cannot enter it in the model, and the host needs an attack move
            // to transfer ownership.
            let finish = Action::Attack { unit: uid, target };
            if after.apply(pid, &finish).is_err()
                || after.cities.get(&cid).is_none_or(|city| city.owner != pid)
                || after.units.get(&uid).is_none_or(|unit| unit.hp < 30)
                || battle_planner::strike_danger(&after, pid, target, uid)
                    >= after.units[&uid].hp as f64 * 0.5
            {
                continue;
            }
            actions.push(finish);
            return Some(actions);
        }
        None
    }

    /// One step closer to `target` that the body survives. Toward a breach
    /// (`breach`, with `air-surge-2`), the step only has to leave the body a
    /// taker for the next board's capture; otherwise it keeps half its health.
    ///
    /// ★★ HALF HEALTH AGAINST EVERY BLOW AT ONCE. The full field charged
    /// every hostile's blow to the one approaching body and asked it to keep
    /// half its health, while a hostile strikes once a turn and a city behind
    /// fallen walls cannot strike at all. With `air-surge-2` each blow is
    /// shared among our land units in its reach (never under the strongest
    /// single blow). Live King 20261001T050754Z: Uppsala sat breached from
    /// turn 103 to 113 reading "the nearest has no safe step closer"; there
    /// the approach was also closed by our own spent Trebuchets on every
    /// distance-two tile, which no danger reading can open.
    fn air_assault_advance(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
        breach: bool,
    ) -> Option<Action> {
        let origin = g.units.get(&uid)?.pos;
        let here = g.wdist(origin, target);
        if here <= 1 {
            return None;
        }
        let mut candidates: Vec<Pos> = g
            .reachable(uid)
            .into_iter()
            .filter(|pos| {
                let distance = g.wdist(*pos, target);
                (1..here).contains(&distance) && g.city_at(*pos).is_none()
            })
            .collect();
        candidates.sort_by_key(|pos| (g.wdist(*pos, target), g.wdist(origin, *pos), *pos));
        for to in candidates.into_iter().take(24) {
            let mut after = g.speculative_clone();
            let Some(action) = walk(&mut after, pid, uid, to) else {
                continue;
            };
            let hp = after.units[&uid].hp as f64;
            let danger = self.air_assault_danger(&after, pid, to, uid);
            let safe = if breach && self.air_surge_2 {
                hp - danger >= f64::from(AIR_ASSAULT_BREACH_TAKER_HP)
            } else {
                danger < hp * 0.5
            };
            if safe {
                *g = after;
                return Some(action);
            }
        }
        None
    }

    /// The blows `uid` takes on `tile` next turn: shared among our exposed
    /// land units with `air-surge-2` or `shared-danger`, the full field
    /// otherwise.
    fn air_assault_danger(&self, g: &Game, pid: usize, tile: Pos, uid: u32) -> f64 {
        if !self.air_surge_2 && !self.shared_danger {
            return battle_planner::strike_danger(g, pid, tile, uid);
        }
        let mut field = battle_planner::DangerField::with_reach(g, pid, true);
        field.share(g);
        field.rotation_danger(tile, uid)
    }

    fn air_assault_withdraw(&self, g: &mut Game, pid: usize, uid: u32) -> Option<Action> {
        let origin = g.units[&uid].pos;
        let danger = battle_planner::strike_danger(g, pid, origin, uid);
        if danger <= 0.0 {
            return None;
        }
        let mut candidates = g.reachable(uid);
        candidates.sort_by_key(|pos| (g.wdist(origin, *pos), *pos));
        for to in candidates.into_iter().take(24) {
            let mut after = g.speculative_clone();
            let Some(action) = walk(&mut after, pid, uid, to) else {
                continue;
            };
            let risk = battle_planner::strike_danger(&after, pid, to, uid);
            if risk < danger && risk < after.units[&uid].hp as f64 * 0.5 {
                *g = after;
                return Some(action);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
