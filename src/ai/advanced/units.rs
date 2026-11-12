//! The unit turn: how `AdvancedAi` moves each of its units.
//!
//! `advanced_units` walks the empire's units each turn and applies their
//! intents (`apply_unit_intents`, `advance_unit_serial`). Most land and naval
//! units go through `advanced_military_step_with_decline`, the per-unit
//! step: it takes the kills on offer first (`prioritize_immediate_kills`,
//! `immediate_kill_value`, `friendly_volley_extension`, `air_strike_value`),
//! keeps recon out of even fights, and garrisons what it occupies. Religious
//! units go through `advanced_religious_step` (missionaries,
//! `condemn_step`), Traders through `advanced_trader_step`, and explorers
//! through `explorer_turn`/`naval_explorer`.
//!
//! Moved verbatim out of the one `impl AdvancedAi` block in `advanced.rs`
//! (2026-11-05). Methods the rest of the controller calls are `pub(super)`;
//! the code is otherwise unchanged.

use super::last_orders::OrderStep;
use super::*;

impl AdvancedAi {
    pub(super) fn guard_is_bound_to_any_settler(&self, guard: u32) -> bool {
        self.settler_guards
            .values()
            .chain(self.settler_sea_guards.values())
            .any(|bound| *bound == guard)
    }

    pub(super) fn advanced_trader_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        strategy: GrandStrategy,
    ) -> bool {
        let current = g.units[&uid].pos;
        let origin = g.city_at(current).filter(|cid| g.cities[cid].owner == pid);
        // The native simulation lets a Trader walk to its origin, while the
        // live bridge may expose that walk as several host turns. A nearby
        // camp/raider makes the exposed path a bad trade: return to a friendly
        // city before searching for a route. Starting from the city itself is
        // still allowed because route creation is immediate there.
        // The danger result only affects an exposed Trader. A Trader already
        // in its origin starts a route below regardless, so do not scan every
        // city's camps and the world's units to compute an unused answer.
        if origin.is_none()
            && self.base.barbarian_tactics_enabled()
            && (self.base.barbarian_trade_risk_for_controller(g, pid)
                || g.player_city_ids(pid).into_iter().any(|city| {
                    self.base
                        .barbarian_threat_pressure_for_controller(g, pid, city)
                        != 0
                }))
        {
            let target = g
                .cities
                .values()
                .filter(|city| city.owner == pid)
                .min_by_key(|city| (g.wdist(current, city.pos), city.id))
                .map(|city| city.pos);
            if let Some(target) = target {
                return self.base.step_toward(g, pid, uid, target);
            }
        }
        if let Some(origin) = origin {
            // `culture-routes-reach-every-rival`: a rival without a route of
            // ours first. `None` while the gene is off. See
            // `advanced/culture_routes.rs`.
            if let Some((_, city)) = self
                .culture_route_claim(g, pid, origin, strategy)
                .or_else(|| self.best_trade_route_destination(g, pid, origin, strategy))
            {
                return g
                    .apply(pid, &Action::TradeRoute { unit: uid, city })
                    .is_ok();
            }
        }
        let Some((_, origin)) = self.best_trade_route_origin(g, pid, current, strategy) else {
            return false;
        };
        self.base.step_toward(g, pid, uid, g.cities[&origin].pos)
    }

    /// Start a route for the non-walking Trader representation used by the
    /// Civilization VI live bridge.
    ///
    /// Firaxis exports an idle Trader with zero normal movement, while CIVVIS's
    /// native simulation gives it walking movement until it reaches an origin.
    /// Keep that adaptation out of [`Self::advance_unit_serial`]: the latter is
    /// the tournament agent's historical path.  The bridge calls this only for
    /// an externally-normalized, stationary Trader after its ordinary turn.
    fn start_zero_movement_trader_route(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        strategy: GrandStrategy,
    ) -> bool {
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        if unit.owner != pid || unit.kind != "trader" || unit.moves_left > 0.0 {
            return false;
        }
        let current = unit.pos;
        let Some(origin) = g.city_at(current).filter(|cid| g.cities[cid].owner == pid) else {
            return false;
        };
        let Some((_, city)) = self
            .culture_route_claim(g, pid, origin, strategy)
            .or_else(|| self.best_trade_route_destination(g, pid, origin, strategy))
        else {
            return false;
        };
        g.apply(pid, &Action::TradeRoute { unit: uid, city })
            .is_ok()
    }

    pub(super) fn advanced_missionary_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        offensive: bool,
    ) -> bool {
        // A non-founder can still buy a Missionary of a purchase city's
        // adopted majority faith to repel a competing conversion. That unit
        // has no player-owned religion to read, so its own faith is the
        // authority for both its defensive target and its Spread action.
        // `religious_units_heal_first`: a charge spent at 40 hit points buys
        // 40% of a charge's pressure. Standing in an own Holy Site's ring is
        // the only recovery a Missionary has, and taking no action is the only
        // way to collect it.
        // `missionary_evades_raiders`: a spreader inside a visible raider's
        // reach leaves it before anything else, the heal included — a
        // condemned unit heals nothing.
        if let Some(acted) = self.religious_unit_evades_raiders(g, pid, uid) {
            return acted;
        }
        // `spreader-steps-out-of-the-apostle`: out of a rival Apostle's reach
        // before the heal, too — a unit killed in a fight heals nothing.
        if let Some(acted) = self.religious_unit_evades_apostles(g, pid, uid) {
            return acted;
        }
        // `corps-comes-home-alive`: a wounded unit walks to a Holy Site of
        // ours and heals there. See `advanced/apostle_army.rs`.
        if let Some(acted) = self.wounded_preacher_goes_home(g, pid, uid) {
            return acted;
        }
        // `last-charge-joins-the-army`: the kept charge hunts or scouts and
        // never reaches the spread below.
        if let Some(acted) = self
            .army_apostle_step(g, pid, uid)
            .or_else(|| self.scout_missionary_step(g, pid, uid))
        {
            return acted;
        }
        if self.religious_unit_holds_to_heal(g, pid, uid) {
            return false;
        }
        let Some(religion) = g.units[&uid]
            .religion
            .clone()
            .or_else(|| g.players[pid].religion.clone())
        else {
            return false;
        };
        // See `founder_holds_a_foreign_spreader` (`founder-spreads-only-its-faith`).
        if self.founder_holds_a_foreign_spreader(g, pid, &religion) {
            think!(self.journal(), Faith, Decision,
                "Holding a {} spreader", religion;
                "founder-spreads-only-its-faith: we founded our own faith, and a charge of another one spent in our cities converts them away from it");
            return false;
        }
        if !self.adopted_faith_spread_allowed(g, pid, &religion) {
            think!(self.journal(), Faith, Decision,
                "Holding an adopted {} spreader", religion;
                "this faith is now our conversion threat or holds every other major; preserve its charges until it is a safe counterweight again");
            return false;
        }
        // See `holy_city_guard_step` (`founder-holds-the-holy-city`): the
        // guarded Holy City takes the spreader before any other target.
        if let Some(acted) = self.holy_city_guard_step(g, pid, uid, &religion) {
            return acted;
        }
        // `religion-seat-charts-the-rivals`: the corps' scout charts an
        // under-charted rival while the rest work. False off.
        if self.religion_seat_scout_charts(g, pid, uid, &religion, offensive) {
            return true;
        }
        let current = g.units[&uid].pos;
        // `religious_veto_defence`: our cities the threat faith holds or is
        // closing on outrank the rest, cheapest flip first.
        let veto = self.religious_veto_engaged(g, pid);
        let recovery = self.counterfaith_recruitment_targets(g, pid, &religion);
        // `counterweight-flips-the-small-towns`: against the faith holding
        // our cities, the smallest town first. `None` off.
        let small_town_threat = self.small_town_threat(g, pid, &religion);
        // See `advanced/religion_seat_counts_the_pressure.rs`. Empty off.
        let pressure_plan = self.religion_seat_pressure_plan(g, pid, uid, &religion);
        let mut targets: Vec<(bool, i32, std::cmp::Reverse<u32>, Pos)> = g
            .cities
            .values()
            .filter(|city| {
                Self::city_needs_religious_support(g, pid, city, &religion)
                    || (offensive
                        && city.owner != pid
                        && !g.is_at_war(pid, city.owner)
                        && self.religion_campaign_needs_city(g, pid, city, &religion)
                        && g.city_religion(city) != Some(religion.as_str()))
            })
            .map(|city| {
                let own_pressure = city.pressure.get(&religion).copied().unwrap_or(0.0);
                let rival_pressure = city
                    .pressure
                    .iter()
                    .filter(|(belief, _)| belief.as_str() != religion)
                    .map(|(_, pressure)| *pressure)
                    .fold(0.0_f64, f64::max);
                let swing = (rival_pressure - own_pressure).clamp(0.0, 500.0) as i32;
                let foreign = (city.owner != pid) as i32;
                let defensive_conversion = (city.owner == pid) as i32 * 170;
                let held = veto
                    .as_ref()
                    .is_some_and(|veto| g.city_religion(city) == Some(veto.religion.as_str()));
                let score = defensive_conversion
                    + foreign * 90
                    + city.pop * 12
                    + city.is_capital as i32 * 18
                    + swing / 10
                    // See `advanced/religion_seat_flips_the_cheapest.rs`.
                    + self.religion_seat_flip_adjustment(g, pid, city, &religion)
                    + pressure_plan.get(&city.id).copied().unwrap_or(0)
                    + Self::religious_veto_target_bonus(pid, city, veto.as_ref(), held)
                    + Self::small_town_target_adjustment(
                        g,
                        pid,
                        city,
                        &religion,
                        small_town_threat.as_deref(),
                    )
                    - g.wdist(current, city.pos) * 4;
                (
                    recovery.contains(&city.id),
                    score,
                    std::cmp::Reverse(city.id),
                    city.pos,
                )
            })
            .collect();
        targets.sort_by(|left, right| right.cmp(left));
        // See `founder_home_targets` (`founder-converts-its-own-cities`): a
        // founder's spreader converts our own cities before any foreign one.
        self.founder_home_targets(g, pid, &religion, &mut targets, |(_, _, _, pos)| *pos);
        if let Some(threat) = small_town_threat.as_deref() {
            if let Some(city) = targets
                .first()
                .and_then(|(_, _, _, pos)| g.city_at(*pos))
                .map(|cid| &g.cities[&cid])
                .filter(|city| city.owner == pid)
            {
                think!(self.journal(), Faith, Detail,
                    "{} spreader heads for {} (pop {})", religion, city.name, city.pop;
                    "counterweight-flips-the-small-towns: {} holds our cities, every city counts the same toward its majority, and the smallest town flips cheapest", threat);
            }
        }
        // `missionary_last_charge_explores*`: the third charge is the unit's
        // life, and the fog is worth more than a third pass at the same city.
        // Version two is an intentionally separate long-range expedition, so
        // the registry can screen it beside the original local policy.
        let sites: Vec<Pos> = targets.iter().map(|(_, _, _, target)| *target).collect();
        if let Some(acted) = self.last_charge_missionary_expedition(g, pid, uid, &religion, &sites)
        {
            return acted;
        }
        if let Some(acted) = self.last_charge_missionary_explores(g, pid, uid, &religion, &sites) {
            return acted;
        }
        for (_, _, _, target) in targets {
            if g.wdist(current, target) <= 1 {
                return g.apply(pid, &Action::Spread { unit: uid }).is_ok();
            }
            if self.religious_step_toward_range(g, pid, uid, target, 1) {
                return true;
            }
        }
        // `religion-seat-charts-the-rivals`: nothing in sight to convert, so
        // walk into an under-charted rival's ground. False off.
        self.religion_seat_spreader_charts(g, pid, uid, &religion, offensive)
    }

    pub(super) fn advanced_religious_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        offensive: bool,
    ) -> bool {
        // `missionary_evades_raiders` covers the whole corps: an Apostle or a
        // Guru inside a raider's reach leaves it before it does anything else.
        if let Some(acted) = self.religious_unit_evades_raiders(g, pid, uid) {
            return acted;
        }
        // `religion-seat-presses-the-campaign`: a winnable fight comes before
        // stepping away. See `advanced/religion_seat_fields_apostles.rs`.
        if let Some(acted) = self.religion_seat_apostle_fights_first(g, pid, uid) {
            return acted;
        }
        // `spreader-steps-out-of-the-apostle`: the whole corps, likewise, out
        // of a rival Apostle's reach first.
        if let Some(acted) = self.religious_unit_evades_apostles(g, pid, uid) {
            return acted;
        }
        // `corps-comes-home-alive`: see `advanced/apostle_army.rs`.
        if let Some(acted) = self.wounded_preacher_goes_home(g, pid, uid) {
            return acted;
        }
        let unit = g.units[&uid].clone();
        let religion = unit
            .religion
            .clone()
            .or_else(|| g.players[pid].religion.clone());
        let legal = g.legal_actions_within(pid, ActionFamilies::UNITS);

        // A lost core city is more urgent than another enhancer or worship
        // belief. Preserve this Apostle until it reaches the Holy City, then
        // launch the inquisition before the rival can close out the match.
        let needs_inquisition = unit.kind == "apostle"
            && unit.religion == g.players[pid].religion
            && g.players[pid]
                .counters
                .get("inquisition")
                .copied()
                .unwrap_or(0)
                == 0
            && (self.defensive_inquisition_ready(g, pid)
                || religion.as_ref().is_some_and(|faith| {
                    g.player_city_ids(pid)
                        .iter()
                        .any(|city| g.city_religion(&g.cities[city]) != Some(faith.as_str()))
                }));
        if needs_inquisition {
            if let Some(action) = legal
                .iter()
                .find(|action| matches!(action, Action::LaunchInquisition { unit } if *unit == uid))
                .cloned()
            {
                return g.apply(pid, &action).is_ok();
            }
            if let Some(target) = g.players[pid]
                .holy_city
                .and_then(|city| g.cities.get(&city).map(|city| city.pos))
            {
                return self.religious_step_toward_range(g, pid, uid, target, 0);
            }
        }

        // `apostle-evangelizes-unspent`: the host refuses an Apostle that has
        // spent a charge. See `advanced/apostle_evangelizes_unspent.rs`.
        if unit.kind == "apostle"
            && g.religion_belief_count(pid) < 4
            && self.apostle_may_evangelize(g, uid)
        {
            let objective = if g.has_ability(pid, "taxis") {
                GrandStrategy::Conquest
            } else {
                self.decision_objective(GrandStrategy::Religion)
            };
            let evangelize = legal
                .iter()
                .filter_map(|action| match action {
                    Action::EvangelizeBelief { unit, belief } if *unit == uid => {
                        let lane = match (objective, belief.as_str()) {
                            (GrandStrategy::Science, "wat")
                            | (GrandStrategy::Culture, "cathedral")
                            | (GrandStrategy::Diplomacy, "pagoda")
                            | (GrandStrategy::Conquest, "just_war")
                            | (GrandStrategy::Expansion, "religious_colonization")
                            | (GrandStrategy::Religion, "holy_order") => 300,
                            (GrandStrategy::Conquest, "meeting_house")
                            | (GrandStrategy::Expansion, "gurdwara")
                            | (GrandStrategy::Religion, "mosque") => 240,
                            (_, "holy_order" | "mosque" | "wat" | "pagoda") => 180,
                            _ => 100,
                        };
                        // `enhancer_for_the_corps` answers first when the corps
                        // has work; `None` leaves the lane table untouched.
                        let score = self
                            .corps_enhancer_score(g, pid, belief.as_str())
                            .unwrap_or(lane);
                        Some((score, std::cmp::Reverse(*belief), action.clone()))
                    }
                    _ => None,
                })
                .max_by_key(|(score, belief, _)| (*score, *belief));
            if let Some((_, _, action)) = evangelize {
                return g.apply(pid, &action).is_ok();
            }
        }

        if unit.kind == "guru" {
            if let Some(action) = legal
                .iter()
                .find(|action| matches!(action, Action::HealReligious { unit } if *unit == uid))
                .cloned()
            {
                return g.apply(pid, &action).is_ok();
            }
        }
        let mut inquisitor_holding = false;
        if unit.kind == "inquisitor" {
            // `religious_veto_defence`: the charge goes where the heresy is.
            match self.inquisitor_veto_step(g, pid, uid, &legal) {
                Some(true) => return true,
                // Holding a faithful source still permits the existing
                // adjacent combat choice. It spends no cleansing charge
                // and must not send the defender off its post afterward.
                Some(false) => inquisitor_holding = true,
                None => {
                    if let Some(action) = legal
                        .iter()
                        .find(|action| matches!(action, Action::RemoveHeresy { unit } if *unit == uid))
                        .cloned()
                    {
                        return g.apply(pid, &action).is_ok();
                    }
                }
            }
        }

        let theological = legal
            .iter()
            .filter_map(|action| match action {
                Action::TheologicalAttack { unit, target } if *unit == uid => {
                    // `theology-fights-only-to-win`: a fight the mean roll loses
                    // is not taken (see `theology_fight_is_winnable`).
                    if self.theology_fights_only_to_win
                        && !self.theology_fight_is_winnable(g, uid, *target)
                    {
                        return None;
                    }
                    let defender_hp = g
                        .unit_ids_at(*target)
                        .iter()
                        .filter(|other| {
                            let other = &g.units[other];
                            g.rules.units[other.kind].class == "religious"
                                && other.religion != religion
                        })
                        .map(|other| g.units[other].hp)
                        .min()
                        .unwrap_or(100);
                    Some((100 - defender_hp, *target, action.clone()))
                }
                _ => None,
            })
            .max_by_key(|(score, target, _)| (*score, std::cmp::Reverse(*target)));
        if let Some((score, _, action)) = theological {
            if unit.hp >= 55 || score >= 45 {
                return g.apply(pid, &action).is_ok();
            }
        }

        if inquisitor_holding {
            return false;
        }

        // `last-charge-joins-the-army`: an Apostle on its kept charge, or a
        // Debater with prey in sight, hunts before anything is spread.
        if let Some(acted) = self.army_apostle_step(g, pid, uid) {
            return acted;
        }

        // The same hold, after the fight rather than before it: a unit already
        // in a theological exchange resolves it, and only the spread and the
        // hunt below wait for the wound to close.
        if self.religious_unit_holds_to_heal(g, pid, uid) {
            return false;
        }

        if g.rules.units[unit.kind].religious_spread > 0.0 && unit.charges > 0 {
            return self.advanced_missionary_step(g, pid, uid, offensive);
        }

        // `guru-walks-to-the-wounded`: a Guru cannot start a fight, so it goes
        // to its patients instead of at the nearest rival.
        if let Some(target) = self.guru_walk_target(g, pid, uid) {
            return self.religious_step_toward_range(g, pid, uid, target, 1);
        }

        let target = g
            .units
            .values()
            .filter(|other| {
                other.owner != pid
                    && g.rules.units[other.kind].class == "religious"
                    && other.religion != religion
            })
            .min_by_key(|other| (g.wdist(unit.pos, other.pos), other.id))
            .map(|other| other.pos)
            .or_else(|| {
                g.players[pid]
                    .holy_city
                    .and_then(|cid| g.cities.get(&cid).map(|city| city.pos))
            });
        target.is_some_and(|target| self.religious_step_toward_range(g, pid, uid, target, 0))
    }

    /// Reserve one reachable land combat unit for each ungarrisoned occupied
    /// city, weakest-loyalty first. The assignment is recomputed from the
    /// current position before every unit acts, so a completed garrison is
    /// immediately removed from the demand set and cannot attract a second
    /// unit. This is the strategic counterpart to Gathering Storm's -5
    /// occupation Loyalty penalty.
    pub(super) fn occupation_garrison_target(&self, g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        let mut cities: Vec<_> = g
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .filter(|city| {
                city.occupied_from
                    .is_some_and(|former| g.players.get(former).is_some_and(|p| p.alive))
            })
            .filter(|city| {
                !g.unit_ids_at(city.pos).iter().any(|unit| {
                    g.units[unit].owner == pid
                        && g.rules.units[g.units[unit].kind].class == "military"
                })
            })
            .collect();
        cities.sort_by(|left, right| {
            self.conversion_garrison_priority(g, left)
                .total_cmp(&self.conversion_garrison_priority(g, right))
                .then_with(|| left.id.cmp(&right.id))
        });
        if cities.is_empty() {
            return None;
        }
        let mut available: BTreeSet<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|unit| {
                let spec = &g.rules.units[g.units[unit].kind];
                spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && g.units[unit].linked_to.is_none()
            })
            .collect();
        // The nearest, then weakest, reachable unit takes each city. Rank the
        // candidates first and route only until one answers: a route search
        // per unit per city was most of this function's cost, and the unit
        // ranked first can almost always reach. Strength does not depend on
        // the city, so it is read once per unit.
        let strength: BTreeMap<u32, i32> = available
            .iter()
            .map(|unit| (*unit, g.unit_strength(&g.units[unit], true) as i32))
            .collect();
        for city in cities {
            let mut ranked: Vec<(i32, i32, u32)> = available
                .iter()
                .map(|unit| (g.wdist(g.units[unit].pos, city.pos), strength[unit], *unit))
                .collect();
            ranked.sort_unstable();
            let selected = ranked.into_iter().map(|(_, _, unit)| unit).find(|unit| {
                g.units[unit].pos == city.pos || g.route_step(*unit, city.pos, 0).is_some()
            });
            if let Some(selected) = selected {
                available.remove(&selected);
                if selected == uid {
                    return Some(city.pos);
                }
            }
        }
        None
    }

    /// The combat actions this controller can use to remove a unit now.  City
    /// and Encampment strikes have their own always-fire command phase; this
    /// is the unit-order seam where an exchange threshold or reply estimate
    /// used to let a kill sit on the board.
    fn unit_strike_actor_and_target(action: &Action) -> Option<(u32, Pos)> {
        match action {
            Action::Attack { unit, target }
            | Action::Ranged { unit, target }
            | Action::AirStrike { unit, target }
            | Action::PriorityTarget { unit, target } => Some((*unit, *target)),
            _ => None,
        }
    }

    /// Return the positive immediate exchange value and number of hostile
    /// units removed by `action`.  The cloned forward model is intentional:
    /// combat rolls are seeded, so an advertised finishing blow is only a
    /// finish when the exact action actually removes the defender.
    ///
    /// A finish must pay for itself and leave a surviving attacker outside
    /// lethal reply damage. Removing one hostile does not remove its allies.
    pub(super) fn immediate_kill_value(
        &self,
        g: &Game,
        pid: usize,
        action: &Action,
        plan: &StrategicPlan,
    ) -> Option<(f64, usize)> {
        let (uid, target) = Self::unit_strike_actor_and_target(action)?;
        let threatened: Vec<u32> = g
            .unit_ids_at(target)
            .iter()
            .filter(|other| {
                let other = &g.units[other];
                other.owner != pid && g.is_at_war(pid, other.owner)
            })
            .copied()
            .collect();
        if threatened.is_empty() {
            return None;
        }
        let mut after = g.speculative_clone();
        if after.apply(pid, action).is_err() {
            return None;
        }
        let eliminated = threatened
            .iter()
            .filter(|other| !after.units.contains_key(other))
            .count();
        if eliminated == 0 || !self.finisher_survives_reach(&after, pid, uid) {
            return None;
        }
        let value = match action {
            Action::Attack { .. } | Action::Ranged { .. } | Action::PriorityTarget { .. } => {
                let exact = Self::tactical_attack_result_owned(
                    g.speculative_clone(),
                    pid,
                    uid,
                    action,
                    plan,
                );
                // `chase-every-boost`: the kill premium, on a kill the attacker
                // survives. See `advanced/chase_every_boost.rs`.
                if exact.attacker_survives && exact.value.is_finite() {
                    exact.value + self.chase_kill_premium(g, pid, uid, target)
                } else {
                    exact.value
                }
            }
            Action::AirStrike { .. } => self.air_strike_value(g, pid, uid, target, plan),
            _ => return None,
        };
        (value.is_finite() && value > 0.0).then_some((value, eliminated))
    }

    /// Price the post-strike tile and remaining enemies, with next-turn reach.
    /// The killing blow can move a melee attacker out of cover and reduce its
    /// HP; pricing either the original tile or original HP misses that trap.
    pub(super) fn finisher_survives_reach(&self, after: &Game, pid: usize, uid: u32) -> bool {
        let Some(unit) = after.units.get(&uid) else {
            return false;
        };
        let envelopes = self.base.enemy_attack_envelopes(after, pid);
        let incoming = BasicAi::incoming_damage(after, pid, uid, unit.pos, &envelopes);
        incoming.total * crate::ai::COMBAT_ROLL_MAX < f64::from(unit.hp)
    }

    /// Spend one immediate, positive kill per unit before either tactical
    /// planner can decline it.  This deliberately enumerates engine-legal
    /// actions rather than reproducing visibility, terrain, embarkation, and
    /// formation gates locally.  Barbarian seats satisfy `is_at_war`, so this
    /// stays a one-hex opportunistic finish instead of widening the normal
    /// campaign's bounded barbarian target list.
    pub(super) fn prioritize_immediate_kills(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        reserved: &BTreeSet<u32>,
    ) -> usize {
        let actions = g.legal_actions_within(pid, ActionFamilies::UNITS);
        let mut spent = reserved.clone();
        let mut kills = 0usize;
        loop {
            let best = actions
                .iter()
                .enumerate()
                .filter_map(|(order, action)| {
                    let (uid, target) = Self::unit_strike_actor_and_target(action)?;
                    // `centre-keeps-its-garrison`: a threatened city's
                    // garrison makes no melee blow, which would carry it out
                    // of the City Center on a kill. `false` with the gene off.
                    let keeper_melee = matches!(action, Action::Attack { .. })
                        && self.keeps_the_centre(g, pid, uid);
                    (!spent.contains(&uid) && !self.builder_guard_reserved(uid) && !keeper_melee)
                        .then(|| self.immediate_kill_value(g, pid, action, plan))
                        .flatten()
                        .map(|(value, eliminated)| (value, eliminated, target, uid, order, action))
                })
                .max_by(|left, right| {
                    left.0
                        .total_cmp(&right.0)
                        .then_with(|| left.1.cmp(&right.1))
                        .then_with(|| right.2.cmp(&left.2))
                        .then_with(|| right.3.cmp(&left.3))
                        .then_with(|| right.4.cmp(&left.4))
                });
            let Some((_, eliminated, _, uid, _, action)) = best else {
                break;
            };
            // Every candidate above landed on an exact clone.  A prior kill
            // can still invalidate a later choice from this turn's static
            // action list, so mark its unit spent and continue rather than
            // letting one stale line abort other independent finishes.
            spent.insert(uid);
            if g.apply(pid, action).is_ok() {
                kills += eliminated;
                self.force_groups_dirty = true;
            }
        }
        kills
    }

    /// The tactical scan starts from an enemy tile, not `Game::legal_actions`,
    /// so it must retain the engine's melee preflight before it clones and
    /// scores a proposed strike.  In particular, entering a forest across a
    /// river can cost more movement than a unit has left, and a plot shared by
    /// an enemy and a peaceful foreign unit is not a legal plot-addressed
    /// strike because Civilization VI chooses the defender from that plot.
    pub(super) fn tactical_melee_candidate_is_legal(
        g: &Game,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> bool {
        // Keep this call on the authoritative engine predicate.  Repeating
        // only the movement/domain half here let a peaceful bystander through
        // and paid for a speculative clone that `Game::apply` immediately
        // refused.
        g.melee_order_is_legal(pid, uid, target)
    }

    /// Find a direct group kill that the normal per-unit evaluator cannot
    /// see as one line.  The first unit still needs to choose a sound exact
    /// attack; this only credits it when surviving teammates in the same
    /// engaged force can legally remove that *same* defender immediately —
    /// one finisher when one blow is enough, and behind [`Self::volley_chain`]
    /// a pair of finishers when the defender needs three blows in all.  A
    /// single finisher is always preferred: the chain is searched only when
    /// no lone teammate can complete the kill, so the deeper case never
    /// spends a third unit a second could have saved.
    ///
    /// The bounded extension deliberately excludes movement, cities, and
    /// quiet follow-ups.  It fixes the harmful ordering assumption that the
    /// enemy replies between friendly shots without reopening the removed
    /// portfolio search or its performance cost.
    pub(super) fn friendly_volley_extension(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        action: &Action,
        group: &ForceGroup,
        plan: &StrategicPlan,
    ) -> Option<(f64, f64)> {
        if !self.coordinated_finish
            || group.posture != ForcePosture::Engage
            || group.units.len() < 2
        {
            return None;
        }
        let target = match action {
            Action::Attack { unit, target } | Action::Ranged { unit, target } if *unit == uid => {
                *target
            }
            _ => return None,
        };
        let victim = g.unit_ids_at(target).iter().find(|other| {
            let defender = &g.units[other];
            defender.owner != pid
                && g.is_at_war(pid, defender.owner)
                && g.rules.units[defender.kind].class == "military"
        })?;
        let mut after_first = g.speculative_clone();
        if after_first.apply(pid, action).is_err()
            || !after_first.units.contains_key(&uid)
            || !after_first.units.contains_key(victim)
        {
            return None;
        }

        let mut best: Option<(f64, u32, usize, Game)> = None;
        for (order, followup) in Self::forcing_attacks_to(&after_first, pid, target, None)
            .into_iter()
            .filter_map(|followup| match &followup {
                Action::Attack { unit, .. } | Action::Ranged { unit, .. }
                    if *unit != uid && group.units.contains(unit) =>
                {
                    Some(followup)
                }
                _ => None,
            })
            .take(TACTICAL_VOLLEY_FINISHER_LIMIT)
            .enumerate()
        {
            let finisher = match &followup {
                Action::Attack { unit, .. } | Action::Ranged { unit, .. } => *unit,
                _ => unreachable!("friendly volley only retains direct ground attacks"),
            };
            let mut after_second = after_first.speculative_clone();
            if after_second.apply(pid, &followup).is_err()
                || after_second.units.contains_key(victim)
            {
                continue;
            }
            let finish_score = Self::tactical_attack_value_owned(
                after_first.speculative_clone(),
                pid,
                finisher,
                &followup,
                plan,
            ) - self.base.attack_threshold(&after_first, finisher, target);
            if !finish_score.is_finite() || finish_score <= 0.0 {
                continue;
            }
            let better = best.as_ref().is_none_or(|(old, old_uid, old_order, _)| {
                finish_score.total_cmp(old).is_gt()
                    || (finish_score.total_cmp(old).is_eq()
                        && (finisher, order) < (*old_uid, *old_order))
            });
            if better {
                best = Some((finish_score, finisher, order, after_second));
            }
        }
        if let Some((_, finisher, _, after_second)) = best {
            let reply = Self::forcing_reply_penalty_from_position(
                self.work_pool.as_ref(),
                &after_second,
                pid,
                &[uid, finisher],
            );
            return Some((
                self.base.w.kill_bonus * TACTICAL_VOLLEY_KILL_BONUS_SCALE,
                reply,
            ));
        }
        if !self.volley_chain {
            return None;
        }

        // No lone teammate can finish the wounded defender — search a bounded
        // pair of finishers instead, so an enemy two sound blows from death is
        // still a group target.  The same enumeration as above supplies the
        // middle blow; each candidate middle blow that leaves the defender
        // standing is extended by the teammates who can then remove it.  Both
        // blows must clear their own exact-exchange bar, so the chain changes
        // which target the force converges on, not how cheaply it will trade.
        let followups: Vec<Action> = Self::forcing_attacks_to(&after_first, pid, target, None)
            .into_iter()
            .filter(|followup| match followup {
                Action::Attack { unit, .. } | Action::Ranged { unit, .. } => {
                    *unit != uid && group.units.contains(unit)
                }
                _ => false,
            })
            .take(TACTICAL_VOLLEY_FINISHER_LIMIT)
            .collect();
        // A healthy defender pays this branch nothing: unless the two
        // heaviest remaining blows can plausibly cover the survivor's health
        // at the engine's mean damage — with headroom for the roll spread and
        // for modifiers this arithmetic does not see — there is no three-blow
        // kill to find and the clone enumeration below never starts.
        {
            let survivor = &after_first.units[victim];
            let defence = crate::game::effective_strength(
                after_first.unit_strength(survivor, true),
                survivor.hp,
            );
            let mut mean_blows: Vec<f64> = followups
                .iter()
                .filter_map(|followup| {
                    let (shooter, ranged) = match followup {
                        Action::Attack { unit, .. } => (*unit, false),
                        Action::Ranged { unit, .. } => (*unit, true),
                        _ => return None,
                    };
                    let attacker = after_first.units.get(&shooter)?;
                    let attack = if ranged {
                        after_first.unit_ranged_attack_strength(attacker)
                    } else {
                        after_first.unit_strength(attacker, false)
                    };
                    let attack = crate::game::effective_strength(attack, attacker.hp);
                    Some((30.0 * ((attack - defence) / 25.0).exp()).clamp(1.0, 100.0))
                })
                .collect();
            mean_blows.sort_by(|a, b| b.total_cmp(a));
            let heaviest_pair: f64 = mean_blows.iter().take(2).sum();
            if heaviest_pair * 1.5 < survivor.hp as f64 {
                return None;
            }
        }
        // Score, then (middle, middle order, finisher, finisher order) as the
        // deterministic tie-break, then the finished position the reply is
        // priced against.
        type ChainCandidate = (f64, (u32, usize, u32, usize), Game);
        let mut best_chain: Option<ChainCandidate> = None;
        for (order, followup) in followups.into_iter().enumerate() {
            let middle = match &followup {
                Action::Attack { unit, .. } | Action::Ranged { unit, .. } => *unit,
                _ => unreachable!("friendly volley only retains direct ground attacks"),
            };
            let mut after_second = after_first.speculative_clone();
            if after_second.apply(pid, &followup).is_err()
                || !after_second.units.contains_key(victim)
            {
                continue;
            }
            let middle_score = Self::tactical_attack_value_owned(
                after_first.speculative_clone(),
                pid,
                middle,
                &followup,
                plan,
            ) - self.base.attack_threshold(&after_first, middle, target);
            if !middle_score.is_finite() || middle_score <= 0.0 {
                continue;
            }
            for (last_order, last) in Self::forcing_attacks_to(&after_second, pid, target, None)
                .into_iter()
                .filter_map(|last| match &last {
                    Action::Attack { unit, .. } | Action::Ranged { unit, .. }
                        if *unit != uid && *unit != middle && group.units.contains(unit) =>
                    {
                        Some(last)
                    }
                    _ => None,
                })
                .take(TACTICAL_VOLLEY_SECOND_FINISHER_LIMIT)
                .enumerate()
            {
                let finisher = match &last {
                    Action::Attack { unit, .. } | Action::Ranged { unit, .. } => *unit,
                    _ => unreachable!("friendly volley only retains direct ground attacks"),
                };
                let mut after_third = after_second.speculative_clone();
                if after_third.apply(pid, &last).is_err() || after_third.units.contains_key(victim)
                {
                    continue;
                }
                let finish_score = Self::tactical_attack_value_owned(
                    after_second.speculative_clone(),
                    pid,
                    finisher,
                    &last,
                    plan,
                ) - self.base.attack_threshold(&after_second, finisher, target);
                if !finish_score.is_finite() || finish_score <= 0.0 {
                    continue;
                }
                let score = middle_score + finish_score;
                let key = (middle, order, finisher, last_order);
                let better = best_chain.as_ref().is_none_or(|(old, old_key, _)| {
                    score.total_cmp(old).is_gt() || (score.total_cmp(old).is_eq() && key < *old_key)
                });
                if better {
                    best_chain = Some((score, key, after_third));
                }
            }
        }
        let (_, (middle, _, finisher, _), after_third) = best_chain?;
        let reply = Self::forcing_reply_penalty_from_position(
            self.work_pool.as_ref(),
            &after_third,
            pid,
            &[uid, middle, finisher],
        );
        Some((
            self.base.w.kill_bonus * TACTICAL_VOLLEY_KILL_BONUS_SCALE,
            reply,
        ))
    }

    /// Evaluate an air strike by making it on a cloned position. This captures
    /// the seeded combat roll, interception damage, wall-vs-city damage, and
    /// kills in one bounded result score instead of ordering targets by their
    /// pre-combat HP alone.
    pub(super) fn air_strike_value(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        target: Pos,
        plan: &StrategicPlan,
    ) -> f64 {
        let attacker = &g.units[&uid];
        let attacker_spec = &g.rules.units[attacker.kind];
        let target_city = g
            .city_at(target)
            .filter(|city| g.cities[city].owner != pid && g.is_at_war(pid, g.cities[city].owner));
        let target_encampment = target_city
            .is_none()
            .then(|| g.defending_district_at(target))
            .flatten();
        let target_unit = (target_city.is_none() && target_encampment.is_none())
            .then(|| {
                g.unit_ids_at(target).iter().find(|other| {
                    let defender = &g.units[other];
                    defender.owner != pid
                        && g.is_at_war(pid, defender.owner)
                        && g.rules.units[defender.kind].class == "military"
                })
            })
            .flatten();
        let action = Action::AirStrike { unit: uid, target };
        let mut after = g.speculative_clone();
        if after.apply(pid, &action).is_err() {
            return f64::NEG_INFINITY;
        }

        let attacker_loss = match after.units.get(&uid) {
            Some(survivor) => {
                (attacker.hp - survivor.hp).max(0) as f64 * (1.4 + attacker_spec.cost / 700.0)
            }
            None => 260.0 + attacker_spec.cost * 0.7,
        };
        let mut value = -attacker_loss;
        if let Some(unit) = target_unit {
            let defender = &g.units[unit];
            let spec = &g.rules.units[defender.kind];
            let role_value = if spec.siege { 70.0 } else { 0.0 }
                + if spec.is_melee_capable() { 30.0 } else { 0.0 }
                + if spec.domain.as_deref() == Some("air") {
                    85.0
                } else {
                    0.0
                };
            value += match after.units.get(unit) {
                None => {
                    190.0 + spec.cost * 0.45 + g.unit_strength(defender, true) * 1.8 + role_value
                }
                Some(survivor) => {
                    (defender.hp - survivor.hp).max(0) as f64
                        * (1.0 + g.unit_strength(defender, true) / 100.0)
                        + role_value * 0.25
                }
            };
        } else if let Some(city) = target_city {
            let before = &g.cities[&city];
            let after_city = &after.cities[&city];
            let wall_damage = (before.wall_hp - after_city.wall_hp).max(0) as f64;
            let city_damage = (before.hp - after_city.hp).max(0) as f64;
            let progress = wall_damage * 1.35 + city_damage;
            value += progress;
            if progress > 0.0 && plan.target_city == Some(city) {
                value += 45.0;
            }
            if before.is_capital && progress > 0.0 && plan.strategy == GrandStrategy::Conquest {
                value += 25.0;
            }
            // The live Amsterdam assault bombed its original capital to a
            // sliver, then pillaged airstrips for several turns while city
            // healing erased the opening. When a capture body is already on
            // the ring, keep reducing that capital instead of taking the
            // one-time infrastructure payout.
            if progress > 0.0
                && before.is_capital
                && self.active_victory_target(g) == Some(VictoryTarget::Domination)
                && (before.wall_hp <= 100 || before.hp <= 120)
                && g.units.values().any(|unit| {
                    let spec = &g.rules.units[unit.kind];
                    unit.owner == pid
                        && unit.hp >= 50
                        && spec.class == "military"
                        && spec.is_melee_capable()
                        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                        && !g.is_embarked(unit)
                        && g.wdist(unit.pos, before.pos) <= 3
                })
            {
                value += 200.0;
            }
        } else if let Some(city) = target_encampment {
            let before = g.defending_district_state(city, target).unwrap();
            let after_district = after.defending_district_state(city, target).unwrap();
            value += (before.wall_hp - after_district.wall_hp).max(0) as f64 * 1.35
                + (before.hp - after_district.hp).max(0) as f64;
        }
        value
    }

    /// Condemning a foreign Missionary or Apostle destroys it and reduces
    /// its faith's nearby pressure. Preserve a safe competing faith when a
    /// different foreign religion is taking our empire. Previously this only fired when an enemy religious unit
    /// happened to already share our tile, which almost never happens, so
    /// the counter was effectively dead. Now a military unit will step onto
    /// an adjacent one and condemn it.
    pub(super) fn condemn_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let condemnable = |game: &Game, at: Pos| -> Option<u32> {
            game.units_at(at).into_iter().find(|target| {
                let target = &game.units[target];
                target.owner != pid
                    && (game.is_at_war(pid, target.owner)
                        || self.world_religion_condemns(game, target))
                    && game.rules.units[target.kind].class == "religious"
                    && !self.preserve_competing_faith(game, pid, target)
            })
        };
        let here = g.units[&uid].pos;
        if let Some(target_unit) = condemnable(g, here) {
            if g.apply(
                pid,
                &Action::CondemnHeretic {
                    unit: uid,
                    target_unit,
                },
            )
            .is_ok()
            {
                return true;
            }
        }
        // Only chase intruders around our own territory: a lone unit running
        // down missionaries across the map abandons the campaign.
        let near_home = g
            .cities
            .values()
            .any(|city| city.owner == pid && g.wdist(here, city.pos) <= 6);
        if !near_home {
            return false;
        }
        let mut targets: Vec<Pos> = g
            .nbrs(here)
            .into_iter()
            .filter(|n| condemnable(g, *n).is_some() && g.can_move(uid, *n))
            .collect();
        targets.sort();
        let Some(to) = targets.first().copied() else {
            return false;
        };
        if g.apply(pid, &Action::Move { unit: uid, to }).is_err() {
            return false;
        }
        if let Some(target_unit) = condemnable(g, to) {
            let _ = g.apply(
                pid,
                &Action::CondemnHeretic {
                    unit: uid,
                    target_unit,
                },
            );
        }
        true
    }

    /// `early-conquest-opening`: the opening's eyes do not brawl. A Scout
    /// trading blows with a barbarian Scout is an even fight it wins nothing
    /// from: on live King civvis-20261001T050754Z the first Scout swung five
    /// times from turn 13, died on turn 23 at 19 health, and the
    /// replacements spent their turns healing, so Stockholm, eleven tiles
    /// north, went unseen until turn 75 and the opening, which needs a target
    /// before turn 40, never named one. A recon unit still takes a melee kill
    /// it survives, and its ranged shots are untouched.
    pub(super) fn recon_declines_the_swing(
        &self,
        spec: &crate::rules::UnitSpec,
        action: &Action,
        eliminates_enemy_unit: bool,
        attacker_survives: bool,
    ) -> bool {
        self.early_conquest_opening
            && spec.promotion_class == "recon"
            && matches!(action, Action::Attack { .. })
            && !(eliminates_enemy_unit && attacker_survives)
    }

    pub(super) fn advanced_military_step_with_decline(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
        decline_settlers: bool,
    ) -> bool {
        if self.builder_guard_reserved(uid) {
            self.ordering(OrderStep::Escort);
            return self.base.fortify_or_stop(g, pid, uid);
        }
        // `battle-planner`: a unit the plan has already struck with, moved
        // or rotated out this turn stays where the plan put it. Always
        // false with the gene off. See `advanced/battle_planner.rs`.
        if self.battle_planner_claims(uid) {
            self.ordering(OrderStep::BattlePlanner);
            return self.base.fortify_or_stop(g, pid, uid);
        }
        // `defend-force-holds-the-city`: a posted soldier strikes from its
        // post and holds it. `None` with the gene off or without a post. See
        // `advanced/defend_force_holds_the_city.rs`.
        if let Some(acted) = self.defend_hold_step(g, pid, uid) {
            self.ordering(OrderStep::DefendHold);
            return acted;
        }
        // `centre-keeps-its-garrison`: a threatened city's garrison keeps
        // its City Center, and an empty one takes a soldier in reach. `None`
        // with the gene off. See `advanced/centre_keeps_its_garrison.rs`.
        if let Some(acted) = self.centre_garrison_step(g, pid, uid) {
            self.ordering(OrderStep::CentreGarrison);
            return acted;
        }
        // `amphibious-siege-train`: a member of the crossing in hand takes
        // the train's orders before anything else can land it at home. `None`
        // with the gene off. See `advanced/amphibious_siege_train.rs`.
        if let Some(acted) = self.amphibious_member_step(g, pid, uid) {
            self.ordering(OrderStep::Breaker);
            return acted;
        }
        // `breaker-walks-under-a-shield`: the early conquest opening's breaker
        // rides with a melee body of its force, and at war the force holds
        // its swings at the walls until the breaker stands beside them.
        // `None` with the gene off. See `advanced/breaker_walks_under_a_shield.rs`.
        if let Some(acted) = self.shielded_breaker_step(g, pid, uid, plan) {
            self.ordering(OrderStep::Breaker);
            return acted;
        }
        // `domination-target-gets-its-breaker`: before the war a Ram or Tower
        // for the lane's target walks to the army before its walls. `None`
        // with the gene off. See `advanced/domination_target_gets_its_breaker.rs`.
        if let Some(acted) = self.domination_breaker_step(g, pid, uid, plan) {
            self.ordering(OrderStep::Breaker);
            return acted;
        }
        let unit = g.units[&uid].clone();
        let rules = std::sync::Arc::clone(&g.rules);
        let spec = &rules.units[unit.kind];
        let special_improver = unit.charges > 0 && !spec.builds.is_empty();
        let doctrine = BasicAi::unit_doctrine(g, uid);
        if self.base.unit_objective_memory && spec.class == "military" {
            if let Some(city) = plan.target_city {
                self.base.remember_capture_objective(g, pid, uid, city);
            }
        }
        // A barbarian-held settler is not an unwanted one under the
        // `barbarian-settler-capture` gene: it is the capture below.
        let barb_rescue: Option<usize> = if self.base.barbarian_settler_capture {
            g.barb_pid
        } else {
            None
        };
        let unwanted_settler_adjacent = decline_settlers
            && g.nbrs(unit.pos).into_iter().any(|position| {
                g.unit_ids_at(position).iter().any(|other| {
                    let other = &g.units[other];
                    other.owner != pid
                        && g.is_at_war(pid, other.owner)
                        && other.kind == "settler"
                        && barb_rescue != Some(other.owner)
                })
            });
        if !unwanted_settler_adjacent
            && self.victory_planning
            && spec.class == "military"
            && self.condemn_step(g, pid, uid)
        {
            self.ordering(OrderStep::Condemn);
            return true;
        }
        let holding_threatened_city = plan.threatened_city.is_some_and(|cid| {
            g.cities
                .get(&cid)
                .is_some_and(|city| g.wdist(unit.pos, city.pos) <= 3)
        });
        let protect_threatened_rescue =
            self.base.barbarian_settler_capture && holding_threatened_city;
        // A bound guard's first obligation is to rejoin its Settler.  The
        // default live shadow used to give this priority only while the pair
        // was already stacked; after a host frame separated them, recovery or
        // a military objective could spend the guard's turn and the bounded
        // Settler wait eventually resumed the march unprotected.  The
        // `civilian_out_of_reach` opt-in has always made that obligation
        // unconditional.  `settler_guard_holds` is the shipped live repair,
        // so its formationless shadow must do the same without changing the
        // deployed genome.
        if spec.class == "military"
            && self.guard_is_bound_to_any_settler(uid)
            && (self.civilian_out_of_reach
                || (self.settler_guard_holds_on() && self.formationless_settler_escort()))
        {
            if let Some(acted) = self.stacked_guard_step(g, pid, uid) {
                self.ordering(OrderStep::Escort);
                return acted;
            }
        }
        // `swap-rotation`: a wounded unit holding a front trades places with
        // the fresh unit behind it, so the line does not open when it
        // leaves. Ahead of recovery deliberately — recovery is what walks it
        // away. `None` with the gene off. See `advanced/swap_rotation.rs`.
        if !unwanted_settler_adjacent && !holding_threatened_city {
            if let Some(acted) = self.swap_rotation_step(g, pid, uid) {
                self.ordering(OrderStep::SwapRotation);
                return acted;
            }
        }
        // `wounded-out-of-reach`: ahead of the recovery, because the recovery
        // reads the mean blow from visible units and this reads the top of
        // the roll from visible and remembered ones. `None` with the gene off.
        // See `advanced/wounded_out_of_reach.rs`.
        if !unwanted_settler_adjacent && !holding_threatened_city {
            if let Some(acted) = self.wounded_out_of_reach_step(g, pid, uid) {
                self.ordering(OrderStep::WoundedOutOfReach);
                return acted;
            }
        }
        // A healthy member of an active Domination siege keeps its post: the
        // train priced the assault as a whole, the battle planner already
        // leaves such a unit on its post (`active_siege_member`), and the
        // siege step below would otherwise never see it. The envelope
        // evacuation in `healing_step`/`retreat_step` charges every enemy
        // blow to the unit at once and remembers the danger for several
        // turns. Live King civvis-20261004T033533Z (game 46): full-health
        // Archers reached Mari's range-2 posts and were walked back to 3-4
        // tiles every turn of Reduce (`move_to_evacuation_tile`); 4 shots in
        // 9 turns, walls 100 -> 88. Only the wounded come out.
        // A siege still in Stage too: its Stage step holds a gun back on the
        // gun's own risk limit. Frame-0 replay of game 46 turn 140, after the
        // rotation exemption freed the Babylon force: two 100-hp catapults
        // were still held by this evacuation at 7-8 tiles and never reached
        // the siege step.
        let siege_post_holds = spec.class == "military"
            && unit.hp >= battle_planner::ROTATE_HP
            && (self.active_siege_member(g, pid, uid) || self.staging_siege_member(g, pid, uid));
        if !unwanted_settler_adjacent && !holding_threatened_city && !siege_post_holds {
            if let Some(acted) = self.base.healing_step(g, pid, uid) {
                self.ordering(OrderStep::Healing);
                return acted;
            }
        }
        if self.base.unit_objective_memory && !siege_post_holds {
            if let Some(acted) = self.base.retreat_step(g, pid, uid) {
                self.ordering(OrderStep::Retreat);
                return acted;
            }
        }
        // A free scout's job is distant discovery. Recovery and an existing
        // civilian guard obligation had their chance; routine military work
        // must not consume every turn before the exploration fallback.
        if !unwanted_settler_adjacent && !holding_threatened_city {
            if let Some(acted) = self.distance_scout_step(g, pid, uid) {
                self.ordering(OrderStep::Explorer);
                return acted;
            }
        }
        if let Some(acted) = self.timed_war_objective_step(g, pid, uid) {
            self.ordering(OrderStep::WarObjective);
            self.force_groups_dirty |= acted;
            return acted;
        }
        if !unwanted_settler_adjacent && !holding_threatened_city {
            if let Some(acted) = self.domination_upgrade_return_step(g, pid, uid, plan) {
                self.ordering(OrderStep::UpgradeReturn);
                return acted;
            }
        }
        // Refresh assignments before an incidental civilian pickup can pull a
        // siege member off its city's ring. The finisher was already protected
        // here; the shooters and screens need the same first decision.
        if (self.victory_planning || self.objective_board) && self.force_groups_dirty {
            self.rebuild_force_groups(g, pid, plan);
            self.force_groups_dirty = false;
        }
        let assigned_siege = (self.siege_train || self.siege_positive_damage_budget)
            && self.force_groups.iter().any(|group| {
                group.domain == ForceDomain::Land
                    && group.units.contains(&uid)
                    && g.city_at(group.objective).is_some_and(|cid| {
                        g.cities[&cid].owner != pid && g.is_at_war(pid, g.cities[&cid].owner)
                    })
            });
        if self.unit_is_reserved(uid) || assigned_siege {
            if let Some(acted) = self.siege_doctrine_step(g, pid, uid, plan) {
                self.ordering(OrderStep::SiegeDoctrine);
                return acted;
            }
        }
        if self
            .base
            .capture_adjacent_civilian(g, pid, uid, decline_settlers)
        {
            self.ordering(OrderStep::CivilianCapture);
            return true;
        }
        // The gene's pursuit half: a capturable barbarian-held civilian a few
        // tiles out is walked onto rather than watched. #2075 enabled the
        // flag and never called this from here, so on the live seat the
        // pursuit existed only in `BasicAi::military_step`, which a major
        // does not run. A unit already assigned to a threatened city is the
        // exception: the live t83–84 run peeled Warrior 57 from Antium's
        // defense to chase a Barbarian Builder into the siege stack, then
        // lost the Warrior. Immediate adjacent captures were handled above;
        // this guard only refuses the multi-tile detour.
        // `invest-keeps-its-cavalry`: nor does a member an investing siege
        // counts on (see `siege_train::siege_needs_the_unit`); live Emperor
        // game 219's Knight 49 marched off after a civilian the turn its
        // train closed on the Maori capital.
        if self.base.barbarian_settler_capture
            && !holding_threatened_city
            && !(self.invest_keeps_its_cavalry && self.siege_needs_the_unit(g, pid, uid))
            && self
                .base
                .pursue_capturable_civilian(g, pid, uid, decline_settlers)
        {
            self.ordering(OrderStep::CivilianCapture);
            return true;
        }
        // `science-threat-denial`: the pad raid. It runs ahead of the
        // opportunistic raid because it is the objective a war was declared
        // for. See `advanced/science_threat_denial.rs`.
        if let Some(acted) = self.science_denial_raid_step(g, pid, uid, plan) {
            self.ordering(OrderStep::RaidPrize);
            return acted;
        }
        // See `opportunistic_war`: during a raid, pillage under our feet or
        // walk to the nearest prize. The adjacent capture above already took
        // anything one step away.
        if let Some(acted) = self.raid_prize_step(g, pid, uid, plan, decline_settlers) {
            self.ordering(OrderStep::RaidPrize);
            return acted;
        }
        if matches!(doctrine, UnitDoctrine::AirDefense | UnitDoctrine::AirStrike) {
            self.ordering(OrderStep::Air);
            let Some(action) = self.advanced_air_action(g, pid, uid, plan) else {
                return false;
            };
            let changes_force_picture = matches!(
                &action,
                Action::AirStrike { .. } | Action::PriorityTarget { .. }
            );
            let acted = g.apply(pid, &action).is_ok();
            self.force_groups_dirty |= acted && changes_force_picture;
            return acted;
        }
        if let Some(acted) = self.settler_escort_step(g, pid, uid, plan) {
            self.ordering(OrderStep::Escort);
            return acted;
        }
        if let Some(action) = self.base.doctrine_action(g, pid, uid) {
            self.ordering(OrderStep::DoctrineAction);
            let changes_force_picture = matches!(
                &action,
                Action::Attack { .. }
                    | Action::Ranged { .. }
                    | Action::AirStrike { .. }
                    | Action::PriorityTarget { .. }
            );
            let acted = g.apply(pid, &action).is_ok();
            self.force_groups_dirty |= acted && changes_force_picture;
            return acted;
        }
        if !unwanted_settler_adjacent {
            if let Some(city) = self.occupation_garrison_target(g, pid, uid) {
                self.ordering(OrderStep::Garrison);
                if unit.pos != city {
                    return self.base.step_toward(g, pid, uid, city);
                }
                return self.base.fortify_or_stop(g, pid, uid);
            }
        }
        // A named major-war city needs a body on its City Center, not merely
        // a force group holding the surrounding ring. The live siege response
        // assigns the nearest eligible unit before this step falls through to
        // the ring's ordinary tactical posture.
        if let Some(acted) = self.threatened_city_garrison_step(g, pid, uid, plan.threatened_city) {
            self.ordering(OrderStep::Garrison);
            return acted;
        }
        let mut enemies: Vec<usize> = g
            .players
            .iter()
            .filter(|p| p.id != pid && p.alive && !p.is_barbarian && g.is_at_war(pid, p.id))
            .map(|p| p.id)
            .collect();
        // ★★★★★ A RAIDER PILLAGING INSIDE THE EMPIRE IS A WAR AT HOME.
        //
        // The filter above deliberately keeps the barbarian seat out of the
        // campaign machinery — a camp is not a war objective. But this same
        // list feeds the attack scan, `barbarian_response_objective`,
        // `nearest_enemy` and the tactical step, so excluding barbarians here
        // left every one of those layers blind to raiders: with no major war
        // on, `enemies` was empty and each soldier took the peacetime path
        // while barbarians pillaged home districts unanswered (observed on
        // live run `civvis-20260807T172510Z`, turns 40+, six idle military
        // units). Barbarian response must remain active during peace.
        //
        // Admit the barbarian seat exactly when it has a presence within
        // `HOME_THREAT_RADIUS` of one of our cities. `nearest_enemy`'s own
        // near-home and exchange-score gates keep the chase bounded.
        // No `alive` test on the seat: a barbarian player holds no cities, so
        // on several rosters it reads `alive = false` while its raiders are
        // very much on the board. The presence check is the liveness test.
        //
        if let Some(barb) = g.barb_pid {
            let camp_reading = self.base.barbarian_presence_at_home_for_controller(
                g,
                pid,
                crate::ai::HOME_CAMP_RADIUS,
            );
            let xp_farm_reading = self.base.has_harmless_naval_xp_shot(g, pid, uid);
            if self.base.barbarian_tactics_enabled()
                && !enemies.contains(&barb)
                && (camp_reading
                    || xp_farm_reading
                    || self.wonder_clearance.iter().any(|(settler, r)| {
                        r.units.contains(&uid)
                            && self.wonder_clearance_site(g, pid, *settler).is_some()
                    }))
            {
                enemies.push(barb);
            }
        }
        // `first-capital-strikes-the-open-window`: a body of the first
        // capital's force, or its breaker, walks on to the rally through a
        // home faith war, which is no front. `false` with the gene off.
        if self.first_capital_marches_through(g, pid, uid, &enemies) {
            if let Some(acted) = self.campaign_staging_step(g, pid, uid, plan) {
                self.ordering(OrderStep::Staging);
                return acted;
            }
        }
        // A home raider supplies a bounded defensive assignment, not a second
        // major-war front. Remember this exact shape so responders keep their
        // first claim below while every other pre-war campaign unit can retain
        // its staging order.
        let barbarian_only = g
            .barb_pid
            .filter(|barb| enemies.len() == 1 && enemies[0] == *barb);
        if enemies.is_empty() {
            // Camps are captured by entering their tile rather than attacking,
            // so an adjacent empty one must be claimed before this field unit
            // is sent to a longer village, escort, or pre-war staging order.
            // The helper asks current player vision and engine movement first.
            if !unwanted_settler_adjacent
                && self.base.clear_adjacent_empty_barbarian_camp(g, pid, uid)
            {
                self.ordering(OrderStep::Camp);
                return true;
            }
            // A newly charted village is an expiring reward, so resolve it
            // before this otherwise idle unit is assigned to a pre-war staging
            // ring or an incidental camp errand. The shared selector gives
            // Scouts the long chase and admits only a nearby military fallback;
            // Settlers and Builders stay on their own movement paths.
            if self.base.village_seeking && self.base.village_collection_step(g, pid, uid) {
                self.ordering(OrderStep::Explorer);
                return true;
            }
            if spec.domain.as_deref() == Some("sea") {
                if let Some(settler) = unit
                    .linked_to
                    .filter(|peer| g.units.get(peer).is_some_and(|peer| peer.kind == "settler"))
                {
                    self.ordering(OrderStep::Escort);
                    if let Some(target) = self.settler_targets.get(&settler).copied() {
                        let approach = BasicAi::naval_approach(g, uid, target).unwrap_or(target);
                        if approach != unit.pos && self.base.step_toward(g, pid, uid, approach) {
                            return true;
                        }
                    }
                    return self.base.fortify_or_stop(g, pid, uid);
                }
                // ★★★★ THE SEA SCOUT SAILED ONLY IN WARTIME. The explorer
                // roster below was consulted on the tactical path alone, and
                // this branch — no major at war, no raider at home — handed
                // every hull to `BasicAi::military_step`, whose own enemy list
                // always holds the barbarian seat (`is_at_war` says so for
                // every major), so it took the ship to `peacetime_step` with
                // `at_war = true`, where a non-Recon unit never explores. On
                // the live seat that is most of the game: the sea's eye
                // stood down between wars. Same roster, same turn, here too.
                if self.is_naval_explorer(g, pid, uid) {
                    if let Some(acted) = self.explorer_turn(g, pid, uid) {
                        self.ordering(OrderStep::Explorer);
                        return acted;
                    }
                }
            }
            if let Some(acted) = self.war_return_home_step(g, pid, uid) {
                self.ordering(OrderStep::Staging);
                return acted;
            }
            if let Some(acted) = self.campaign_staging_step(g, pid, uid, plan) {
                self.ordering(OrderStep::Staging);
                return acted;
            }
            if special_improver {
                if let Some(acted) = self.advanced_special_improver_step(g, pid, uid, plan.strategy)
                {
                    self.ordering(OrderStep::Improver);
                    return acted;
                }
            }
            // The camp errand. This branch IS peacetime — the enemy list is
            // empty — and it is where every unclaimed military unit exits to
            // the Basic fallback, whose wander produces the incidental
            // clears the barbarian-ledger baseline counted. A couple of
            // claimed hunters convert nearby camps into gold, era score,
            // and boost progress instead. Deliberately NOT the barbarian-response
            // path (that machinery prices a camp as a threat of strength
            // 0.0 and lost its native slot for hovering beside raiders):
            // the errand prices a camp as income, behind the same exchange
            // gate as any other attack, so an outmatched guard keeps its
            // tile. A defended camp is handed to the tactical mover from
            // march distance, not attack radius — the plain walker prices
            // threat-adjacent tiles and hovered two tiles from the guard
            // for twelve turns in this pin's first trace, and no attack
            // scan runs in this branch to take the adjacent trade.
            if self.base.camp_bounty {
                if let Some(camp) = self.base.camp_bounty_target(g, pid, uid) {
                    self.ordering(OrderStep::Camp);
                    let defended = g
                        .unit_ids_at(camp)
                        .iter()
                        .any(|oid| Some(g.units[oid].owner) == g.barb_pid);
                    if !defended {
                        // Walking onto the empty camp is the clear itself.
                        if self.base.step_toward(g, pid, uid, camp) {
                            return true;
                        }
                    } else {
                        // The errand closes and attacks itself. The tactical
                        // mover cannot finish this job: its threat penalty
                        // (~8 beside an ancient guard) always outweighs its
                        // closing progress (~3.5), which is survivable
                        // against a raider that eventually walks into reach
                        // and a deadlock against a fortified guard that
                        // never moves — the errand's first trace hovered at
                        // distance two indefinitely. The exchange gate in
                        // `camp_bounty_target` already priced this fight, so
                        // march by engine pathing and take it.
                        let at = g.units[&uid].pos;
                        let d = g.wdist(at, camp);
                        if spec.has_ranged_attack() && d <= g.unit_attack_range(uid) {
                            return g
                                .apply(
                                    pid,
                                    &Action::Ranged {
                                        unit: uid,
                                        target: camp,
                                    },
                                )
                                .is_ok();
                        }
                        if spec.is_melee_capable() && d == 1 {
                            return g
                                .apply(
                                    pid,
                                    &Action::Attack {
                                        unit: uid,
                                        target: camp,
                                    },
                                )
                                .is_ok();
                        }
                        let stop = if spec.has_ranged_attack() {
                            g.unit_attack_range(uid).max(1)
                        } else {
                            1
                        };
                        if let Some(next) = g
                            .route_step(uid, camp, stop)
                            .filter(|next| g.can_move(uid, *next))
                        {
                            return self.base.path_move(g, pid, uid, next);
                        }
                        return self.base.fortify_or_stop(g, pid, uid);
                    }
                }
            }
            if self.neighbor_contact_step(g, pid, uid) {
                self.ordering(OrderStep::Explorer);
                return true;
            }
            // `pass-picket`: a unit nothing above wanted takes its recon
            // order — the post toward a neighbour once nothing is left to
            // explore. See `advanced/recon_disruption.rs`.
            if let Some(acted) = self.recon_disruption_step(g, pid, uid) {
                self.ordering(OrderStep::Picket);
                return acted;
            }
            // `chokepoint-garrison`: the gate on the approach to one of our
            // cities, held by a body nothing above wanted. Deliberately the
            // same tail and the same reason as the picket above; see the
            // header of `advanced/chokepoints.rs` for why it is not a
            // wartime stand.
            if let Some(acted) = self.chokepoint_garrison_step(g, pid, uid) {
                self.ordering(OrderStep::Garrison);
                return acted;
            }
            // The advanced rescue gene is also present in the BasicAi fallback
            // below. Once this plan has marked a nearby city as threatened,
            // do not let that fallback reopen the same civilian detour after
            // the advanced branches deliberately refused it. There is no
            // visible combat target left in this branch, so holding the
            // defender is the safe answer until the next frame refreshes the
            // threat picture.
            if protect_threatened_rescue {
                self.ordering(OrderStep::Garrison);
                return self.base.fortify_or_stop(g, pid, uid);
            }
            self.ordering(OrderStep::Peacetime);
            return self.base.military_step(g, pid, uid);
        }
        // Combat can change occupancy, local power and the best focus target.
        // Movement cannot change the opposing force, so keep the existing
        // orders until an attack actually dirties them. The former
        // unconditional rebuild before every step made a turn with `n` units
        // repeatedly regroup and rescore all `n` after each unit moved.
        if (self.victory_planning || self.objective_board) && self.force_groups_dirty {
            self.rebuild_force_groups(g, pid, plan);
            self.force_groups_dirty = false;
        }
        // A live rush executes its own siege. See `rush_siege_step`: the
        // general force-group heuristics assemble the stack correctly and then
        // will not put it on the city's ring, and four attempts to make them
        // do so each measured worse.
        if let Some(acted) = self.rush_siege_step(g, pid, uid, plan) {
            self.ordering(OrderStep::RushSiege);
            self.force_groups_dirty = true;
            return acted;
        }
        // `siege-train` / `anvil`: a force whose objective is a city — an
        // enemy city to take, a city of ours to hold — plays the doctrine for
        // it in place of the per-unit scan and the group mover below. `None`
        // with both genes off. See `advanced/siege_train.rs`.
        if let Some(acted) = self.siege_doctrine_step(g, pid, uid, plan) {
            self.ordering(OrderStep::SiegeDoctrine);
            return acted;
        }
        let group = self
            .force_groups
            .iter()
            .find(|group| group.units.contains(&uid))
            .cloned();

        let radius = if spec.has_ranged_attack() {
            g.unit_attack_range(uid).max(1)
        } else {
            1
        };
        let mut candidates = Vec::new();
        // Hoisted out of the tile loop below: `visibility_viewers` walks the
        // alliance graph, and neither frame can move while this loop applies
        // nothing. Built lazily, because most units reach no enemy tile at
        // all. `player_vision_frame` hands back the engine's own `Arc`, so
        // even that one build is a refcount bump rather than a `TileBits`
        // clone.
        let mut vision_frames = None;
        for pos in g.wdisk(unit.pos, radius) {
            if spec.class != "military" {
                break;
            }
            if pos == unit.pos || !self.base.is_enemy_tile(g, pos, &enemies) {
                continue;
            }
            let unusable_settler = g
                .unit_ids_at(pos)
                .iter()
                .any(|oid| g.units[oid].kind == "settler" && decline_settlers);
            if unusable_settler && g.city_at(pos).is_none() {
                continue;
            }
            let distance = g.wdist(unit.pos, pos);
            let mut actions = Vec::with_capacity(2);
            // ★★★★★ EVERY CANDIDATE BELOW COSTS TWO SPECULATIVE CLONES AND A
            // NESTED REPLY SEARCH, so a candidate the engine will refuse is an
            // expensive derivation computed and then discarded -- the exact
            // shape `docs/SIMULATOR_PERFORMANCE.md` names as the thing worth
            // removing. This loop used to propose on enemy-tile and range
            // alone: a census of one 150-turn six-player game counted 340
            // refused Ranged orders (159 "target is not visible", 112 "line of
            // sight blocked") and 163 refused Attack orders (124 "unit cannot
            // attack into that domain"), all of them scored in full first.
            //
            // ⚠ These are the engine's own predicates, not re-derivations of
            // them. `ranged_order_is_legal` is deliberately passed the
            // hoisted frames: rebuilding them per tile made the first version
            // of this guard slower. Its line-of-sight check also knows about
            // the firing unit's `see_through_woods`, so re-deriving any part
            // of the legality rule here would be the same bug in a new place.
            //
            // ⚠ This is a CORRECTNESS change and nothing else. Measured on
            // `tools/speed_ab.py`, 8 paired games at the deployment shape, it
            // is **-0.18%: inside the +/-0.2% noise floor**, so it buys no
            // CPU. It buys no strength either -- the arm returned 24 neutral
            // splits on 24 maps against stock. A refused candidate never wins
            // the argmax anyway, because scoring it applies it to a clone that
            // refuses it too. What it buys is that the candidate set is
            // truthful, which matters to anything that trusts it.
            //
            // ⚠⚠ The first version of this gate was a **+6.43% pessimization**
            // because it called `combat_target_visible`, which recomputes the
            // full vision and viewer derivation per call. The frames are
            // hoisted below for that reason. Do not inline them back.
            let frames = vision_frames
                .get_or_insert_with(|| (g.player_vision_frame(pid), g.visibility_viewers(pid)));
            if g.ranged_order_is_legal(pid, uid, pos, &frames.0, &frames.1) {
                actions.push(Action::Ranged {
                    unit: uid,
                    target: pos,
                });
            }
            if unit.kind == "spec_ops"
                && distance <= g.unit_attack_range(uid)
                && g.priority_support_target_at(pid, pos).is_some()
            {
                actions.push(Action::PriorityTarget {
                    unit: uid,
                    target: pos,
                });
            }
            if Self::tactical_melee_candidate_is_legal(g, pid, uid, pos) {
                actions.push(Action::Attack {
                    unit: uid,
                    target: pos,
                });
            }
            for action in actions {
                candidates.push((pos, action));
            }
        }
        // Each score consumes two private worlds: one for the exact attack
        // result and one for the enemy reply search. Those are the same two
        // clones the serial evaluator made, moved to persistent workers only
        // after cloning so the authoritative `Game` never crosses threads.
        let evaluations = match &self.work_pool {
            Some(pool) if candidates.len() > 1 => {
                let inputs = candidates
                    .iter()
                    .map(|(_, action)| (g.speculative_clone(), action.clone()))
                    .collect();
                let plan = plan.clone();
                let nested_pool = Arc::clone(pool);
                pool.map_owned(inputs, move |(mut board, action)| {
                    let (attack, applied) =
                        Self::tactical_attack_result_in(&mut board, pid, uid, &action, &plan);
                    (
                        attack.value,
                        attack.eliminates_enemy_unit,
                        attack.attacker_survives,
                        Self::forcing_reply_penalty_applied(
                            Some(Arc::clone(&nested_pool)),
                            &mut board,
                            applied,
                            pid,
                            uid,
                            &action,
                        ),
                    )
                })
            }
            _ => candidates
                .iter()
                .map(|(_, action)| {
                    let mut board = g.speculative_clone();
                    let (attack, applied) =
                        Self::tactical_attack_result_in(&mut board, pid, uid, action, plan);
                    (
                        attack.value,
                        attack.eliminates_enemy_unit,
                        attack.attacker_survives,
                        Self::forcing_reply_penalty_applied(
                            self.work_pool.clone(),
                            &mut board,
                            applied,
                            pid,
                            uid,
                            action,
                        ),
                    )
                })
                .collect(),
        };

        let mut scored = Vec::with_capacity(candidates.len());
        for (
            order,
            (
                (pos, action),
                (attack_value, eliminates_enemy_unit, attacker_survives, reply_penalty),
            ),
        ) in candidates.into_iter().zip(evaluations).enumerate()
        {
            // The immediate-kill pass and the ordinary tactical ladder must
            // agree: declining a poisoned finish above is useless if this
            // second path immediately takes it through its kill override.
            if self.victory_planning && eliminates_enemy_unit {
                let mut after = g.speculative_clone();
                if after.apply(pid, &action).is_err()
                    || !self.finisher_survives_reach(&after, pid, uid)
                {
                    continue;
                }
            }
            if self.recon_declines_the_swing(
                spec,
                &action,
                eliminates_enemy_unit,
                attacker_survives,
            ) {
                continue;
            }
            let threshold = self.base.attack_threshold(g, uid, pos);
            let ranged = matches!(&action, Action::Ranged { .. });
            let mut score = attack_value - threshold;
            // `siege-at-the-walls-dies-first`: a blow on a gun at our walls,
            // or on the taker beside an open city, carries the premium, and a
            // shot at one or a kill of one ranks with the lethal kills. Zero
            // with the gene off. See `advanced/siege_at_the_walls_dies_first.rs`.
            let wall_premium = if attack_value.is_finite() {
                self.wall_threat_blow_premium(g, pid, &action)
            } else {
                0.0
            };
            score += wall_premium;
            let wall_tier = wall_premium > 0.0 && (ranged || eliminates_enemy_unit);
            // `chase-every-boost`: a kill this unit survives that fires an open
            // Eureka or Inspiration (`kill_with:` its own kind, a barbarian for
            // Bronze Working, a camp) is worth the research it earns. Zero with
            // the gene off. See `advanced/chase_every_boost.rs`.
            if eliminates_enemy_unit && attacker_survives {
                score += self.chase_kill_premium(g, pid, uid, pos);
            }
            score += self.base.harmless_naval_xp_bonus(g, pid, uid, pos, ranged);
            if plan
                .target_city
                .is_some_and(|cid| g.cities.get(&cid).is_some_and(|c| c.pos == pos))
            {
                score += 28.0;
                // `ring-fires-on-the-city`: see `siege_train::ring_is_threatened`.
                if ranged && self.ring_fires_on_the_city && !self.ring_is_threatened(g, pid, uid) {
                    score += RING_CITY_SHOT_BONUS;
                }
            }
            if g.unit_ids_at(pos).iter().any(|oid| g.units[oid].hp <= 35) {
                score += 16.0;
            }
            if group.as_ref().and_then(|orders| orders.focus_target) == Some(pos) {
                score += self.base.w.focus_fire * 10.0;
            }
            // `fire-plan`: the planned target gets the focus bias too; the
            // exact decision below is unchanged. `None` with the gene off.
            if self.fire_plan_target(uid) == Some(pos) {
                score += self.base.w.focus_fire * 10.0;
            }
            let caution = group
                .as_ref()
                .map(|orders| {
                    self.base.w.local_superiority * (1.0 - orders.local_strength_ratio).max(0.0)
                })
                .unwrap_or(0.0);
            score -= caution;
            let reply = self.base.w.trade_caution * reply_penalty;
            score -= reply;
            scored.push(TacticalAttackCandidate {
                score,
                target: pos,
                action,
                // `advanced_v1` is a frozen evaluator anchor.  This is a
                // production policy, so leave that controller's historical
                // candidate ordering untouched.
                lethal_kill: self.victory_planning
                    && ((eliminates_enemy_unit && attack_value > 0.0) || wall_tier),
                parts: [attack_value, threshold, caution, reply, 0.0],
                order,
            });
        }

        // The ordinary evaluator prices one action at a time, so it treats an
        // enemy reply as if it arrives between a ranged setup shot and the
        // teammate's finishing blow.  Revisit only a few strongest candidates
        // with the bounded friendly-volley extension, then replace that reply
        // price with the exact reply after both friendly actions.
        if self.coordinated_finish
            && group
                .as_ref()
                .is_some_and(|orders| orders.posture == ForcePosture::Engage)
        {
            let focus = group.as_ref().and_then(|orders| orders.focus_target);
            let mut volley_indices: Vec<usize> = (0..scored.len())
                .filter(|index| {
                    g.unit_ids_at(scored[*index].target).iter().any(|other| {
                        let defender = &g.units[other];
                        defender.owner != pid
                            && g.is_at_war(pid, defender.owner)
                            && g.rules.units[defender.kind].class == "military"
                    })
                })
                .collect();
            volley_indices.sort_by(|left, right| {
                let left = &scored[*left];
                let right = &scored[*right];
                (focus == Some(right.target))
                    .cmp(&(focus == Some(left.target)))
                    .then_with(|| right.score.total_cmp(&left.score))
                    .then_with(|| left.target.cmp(&right.target))
                    .then_with(|| left.order.cmp(&right.order))
            });
            let mut examined_targets = BTreeSet::new();
            for index in volley_indices {
                if examined_targets.len() >= TACTICAL_VOLLEY_CANDIDATE_LIMIT
                    || !examined_targets.insert(scored[index].target)
                {
                    continue;
                }
                let Some(orders) = group.as_ref() else {
                    break;
                };
                let Some((bonus, followup_reply)) = self.friendly_volley_extension(
                    g,
                    pid,
                    uid,
                    &scored[index].action,
                    orders,
                    plan,
                ) else {
                    continue;
                };
                let reply = self.base.w.trade_caution * followup_reply;
                let prior_reply = scored[index].parts[3];
                scored[index].score += bonus + prior_reply - reply;
                scored[index].parts[3] = reply;
                scored[index].parts[4] = bonus;
            }
        }
        let best = scored.into_iter().max_by(|left, right| {
            left.lethal_kill.cmp(&right.lethal_kill).then_with(|| {
                left.score
                    .total_cmp(&right.score)
                    .then_with(|| right.target.cmp(&left.target))
                    .then_with(|| right.order.cmp(&left.order))
            })
        });
        if let Some(TacticalAttackCandidate {
            score,
            target: at,
            action,
            lethal_kill,
            parts,
            ..
        }) = best
        {
            let required_margin = if unit.hp < 55 { 12.0 } else { 0.0 };
            if lethal_kill || score > required_margin {
                if self.journal().wants(crate::reasoning::Level::Detail) {
                    let verb = match &action {
                        Action::Ranged { .. } => "shells",
                        Action::PriorityTarget { .. } => "targets",
                        _ => "attacks",
                    };
                    let defender = g
                        .city_at(at)
                        .and_then(|cid| g.cities.get(&cid))
                        .map(|city| city.name.clone())
                        .or_else(|| {
                            g.unit_ids_at(at)
                                .first()
                                .map(|oid| plain(&g.units[oid].kind))
                        })
                        .unwrap_or_else(|| format!("{at:?}"));
                    let orders = group
                        .as_ref()
                        .map(|orders| {
                            format!(
                                "{} group at {:.2} local strength",
                                orders.posture.as_str(),
                                orders.local_strength_ratio
                            )
                        })
                        .unwrap_or_else(|| "unattached".to_string());
                    think!(self.journal(), Military, Detail,
                           "{} {verb} {defender}", plain(&unit.kind);
                           "worth {score:.0} over a margin of {required_margin:.0}, \
                            on {} health, {orders}{}", unit.hp,
                           if lethal_kill { "; finishing it now" } else { "" }; at);
                }
                if g.apply(pid, &action).is_ok() {
                    self.ordering(OrderStep::TacticalAttack);
                    self.force_groups_dirty = true;
                    return true;
                }
            } else if self.journal().wants(crate::reasoning::Level::Detail) {
                // ★★★★★ SAY WHY THE ARMY DID NOT SWING.
                //
                // The line above records every attack that happens and there
                // is none for an attack that does not, so a unit standing in
                // range declining a shot is indistinguishable in `why.log`
                // from one that had no target at all. Measured on live run
                // `civvis-20260803T005930Z`: of 1787 military unit-turns in
                // 188 turns of war, 186 were in range of a target and **117
                // did not attack — 77 of them with movement left and a median
                // 100 health**, 37 of those Field Cannons. Not one of those 77
                // decisions left a trace.
                //
                // The terms are named because the score is a sum and
                // knowing it was negative says nothing about which part sank
                // it. `worth` is the exact forward-model attack value,
                // `asking` the doctrine threshold from
                // `BasicAi::attack_threshold`, `caution` the local-superiority
                // deduction, and `reply` the enemy's best answer priced by
                // `trade_caution`; `volley` is the bounded cooperative setup
                // credit when another force member can finish the defender.
                let [attack_value, threshold, caution, reply, volley] = parts;
                let defender = g
                    .city_at(at)
                    .and_then(|cid| g.cities.get(&cid))
                    .map(|city| city.name.clone())
                    .or_else(|| {
                        g.unit_ids_at(at)
                            .first()
                            .map(|oid| plain(&g.units[oid].kind))
                    })
                    .unwrap_or_else(|| format!("{at:?}"));
                think!(self.journal(), Military, Detail,
                       "{} declines {defender}", plain(&unit.kind);
                       "worth {attack_value:.0} against asking {threshold:.0}, \
                        caution {caution:.0}, reply {reply:.0}, volley {volley:.0} — {score:.0} \
                        under a margin of {required_margin:.0}, on {} health",
                       unit.hp; at);
            }
        }

        // `campaign_pillage`: the blow above came first, and a unit that
        // struck has no movement left. This spends only movement the march
        // below would not use. See `advanced/city_campaign.rs`.
        if let Some(acted) = self.campaign_pillage_step(g, pid, uid, plan, group.as_ref()) {
            self.ordering(OrderStep::Pillage);
            return acted;
        }

        if let Some(acted) = self.wonder_clearance_step(g, pid, uid) {
            self.ordering(OrderStep::Escort);
            self.force_groups_dirty |= acted;
            return acted;
        }

        let linked_settler = (spec.domain.as_deref() == Some("sea"))
            .then_some(unit.linked_to)
            .flatten()
            .filter(|peer| g.units.get(peer).is_some_and(|peer| peer.kind == "settler"));
        let hostile_water_unit = g
            .units
            .values()
            .any(|enemy| enemies.contains(&enemy.owner) && BasicAi::waterborne(g, enemy.id));
        if !hostile_water_unit {
            if let Some(settler) = linked_settler {
                self.ordering(OrderStep::Escort);
                if let Some(target) = self.settler_targets.get(&settler).copied() {
                    let approach = BasicAi::naval_approach(g, uid, target).unwrap_or(target);
                    if approach != unit.pos && self.base.step_toward(g, pid, uid, approach) {
                        return true;
                    }
                }
                return self.base.fortify_or_stop(g, pid, uid);
            }
        }

        if unwanted_settler_adjacent {
            self.ordering(OrderStep::Hold);
            return self.base.fortify_or_stop(g, pid, uid);
        }

        // Up to two free ships are the sea's scouts while water remains dark.
        // Combat, escort, and recovery already had their chance above, so this
        // only spends hulls that have no more immediate job. See
        // `naval_explorer`.
        let naval_explorer = self.is_naval_explorer(g, pid, uid);
        if (doctrine == UnitDoctrine::Recon && self.base.should_explore(g, pid, uid, true))
            || naval_explorer
        {
            if let Some(acted) = self.explorer_turn(g, pid, uid) {
                self.ordering(OrderStep::Explorer);
                return acted;
            }
        }

        // A charged military unique still gets every immediate combat, escort,
        // civilian-capture, and recovery decision above. If none claimed it,
        // use its charge as a strategic job before sending it on an otherwise
        // distant campaign march.
        if special_improver {
            if let Some(acted) = self.advanced_special_improver_step(g, pid, uid, plan.strategy) {
                self.ordering(OrderStep::Improver);
                return acted;
            }
        }

        // Holding a city against a nearby barbarian outranks the campaign march.
        // A production seat (which routes every military unit through here)
        // otherwise watched raiders pillage its home districts while its army
        // staged on a border it was not even at war across. The dedicated
        // response budgets its responders, and a responder it claims marches:
        // the
        // wartime mover below holds a unit outside the enemy's
        // move-and-attack reach, right for a front, wrong for a raider, which
        // it hovers two tiles from forever while the districts burn. Close
        // instead; once the threat is inside this unit's own attack radius
        // the scan above takes the trade on the next pass, and even trades
        // against barbarians are worth taking.
        let strategic_barbarian_response = self.base.barbarian_presence_at_home_for_controller(
            g,
            pid,
            crate::ai::HOME_CAMP_RADIUS,
        );
        if strategic_barbarian_response {
            if let Some(barb) = g.barb_pid.filter(|barb| enemies.contains(barb)) {
                self.ordering(OrderStep::BarbarianResponse);
                let barb_only = [barb];
                let garrisoned = self.base.barbarian_garrison_step(g, pid, uid, &barb_only);
                if garrisoned {
                    return true;
                }
                // `tactical_step` keeps a melee unit at attack range from its
                // target, but an empty camp has no defender to attack. Once an
                // actual city garrison has had first claim, enter this free camp
                // rather than ending the turn beside it.
                if self.base.clear_adjacent_empty_barbarian_camp(g, pid, uid) {
                    return true;
                }
                let threat = self
                    .base
                    .barbarian_response_objective(g, pid, uid, &barb_only);
                if let Some(threat) = threat {
                    if g.wdist(unit.pos, threat) > radius
                        && self.base.step_toward(g, pid, uid, threat)
                    {
                        return true;
                    }
                    return self
                        .base
                        .tactical_step(g, pid, uid, threat, &barb_only, radius);
                }
            }
        }
        // A camp outside the local barbarian-response radius may still be a
        // one-step clear beside this unit. Immediate combat, civilian capture,
        // recovery, escorts, and any local defense above keep their priority.
        if self.base.clear_adjacent_empty_barbarian_camp(g, pid, uid) {
            self.ordering(OrderStep::Camp);
            return true;
        }
        // The defender's claim above is deliberately bounded to the nearest
        // half-army. Let the unclaimed remainder keep assembling on its
        // campaign ring rather than falling through to the barbarian tactical
        // path, which treats the foreign objective as a one-tile approach and
        // pulls correctly staged units out of position. Major wars continue to
        // use their existing group and reinforcement path unchanged.
        if barbarian_only.is_some() {
            if let Some(acted) = self.campaign_staging_step(g, pid, uid, plan) {
                self.ordering(OrderStep::Staging);
                return acted;
            }
        }
        let defend_target = plan.threatened_city.and_then(|cid| {
            let city = g.cities.get(&cid)?;
            g.units
                .values()
                .filter(|u| {
                    enemies.contains(&u.owner)
                        && (!protect_threatened_rescue || g.rules.units[u.kind].class == "military")
                        && g.wdist(city.pos, u.pos) <= 7
                })
                .min_by_key(|u| (g.wdist(unit.pos, u.pos), u.id))
                .map(|u| u.pos)
        });
        let campaign = if spec.domain.as_deref() == Some("sea") {
            defend_target
                .filter(|pos| g.map.get(*pos).is_some_and(|tile| g.rules.is_water(tile)))
                .or_else(|| self.base.nearest_enemy_for_unit(g, pid, uid, &enemies))
        } else {
            defend_target
                .or_else(|| {
                    plan.target_city
                        .and_then(|cid| g.cities.get(&cid).map(|c| c.pos))
                })
                .or_else(|| self.base.capture_objective_target(g, pid, uid))
                .or_else(|| {
                    // A threatened-city defender may still join a real fight,
                    // but must not treat a nearby Barbarian Builder as a
                    // combat objective after the dedicated rescue detour was
                    // refused. The live t83–84 run selected exactly that
                    // civilian through this fallback and lost Warrior 57.
                    if protect_threatened_rescue {
                        self.base
                            .nearest_enemy(g, pid, uid, &enemies)
                            .filter(|position| {
                                g.city_at(*position).is_some()
                                    || g.unit_ids_at(*position).iter().any(|other| {
                                        let enemy = &g.units[other];
                                        enemies.contains(&enemy.owner)
                                            && g.rules.units[enemy.kind].class == "military"
                                    })
                            })
                    } else {
                        self.base.nearest_enemy(g, pid, uid, &enemies)
                    }
                })
        };
        // Rear reinforcements march before taking group orders: Hold is
        // correct for a front force that is locally outnumbered, and wrong
        // for the standing-still trickle at home that will never be locally
        // superior anywhere. Homeland claims all ran above, and the step
        // stands down empire-wide while any city is threatened.
        if let Some(acted) =
            self.wartime_reinforcement_step(g, pid, uid, plan, group.as_ref(), &enemies)
        {
            self.ordering(OrderStep::Reinforcement);
            self.force_groups_dirty = true;
            return acted;
        }
        if let Some(orders) = &group {
            self.ordering(OrderStep::Coordinated);
            return self.coordinated_tactical_step(g, pid, uid, orders, &enemies, decline_settlers);
        }
        self.ordering(if campaign.is_some() {
            OrderStep::Tactical
        } else {
            OrderStep::Peacetime
        });
        match campaign {
            Some(target) => self
                .base
                .tactical_step(g, pid, uid, target, &enemies, radius),
            // Nothing this unit is willing to fight: explore or garrison
            // rather than shadowing a raider it will never strike.
            None => self.base.peacetime_step(g, pid, uid, !enemies.is_empty()),
        }
    }

    pub(super) fn unit_uses_general_planner(&self, g: &Game, uid: u32) -> bool {
        let unit = &g.units[&uid];
        !matches!(
            unit.kind.as_str(),
            "settler"
                | "builder"
                | "military_engineer"
                | "naturalist"
                | "archaeologist"
                | "trader"
                | "missionary"
                | "rock_band"
        ) && (unit.charges <= 0 || g.rules.units[unit.kind].builds.is_empty())
            && !(self.victory_planning && g.rules.units[unit.kind].class == "religious")
    }

    pub(super) fn advance_unit_serial(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
        religious_offensive: bool,
        decline_settlers: bool,
    ) -> bool {
        let mut took_a_turn = false;
        for _ in 0..8 {
            if !g.units.contains_key(&uid) || g.units[&uid].moves_left <= 0.0 {
                break;
            }
            let kind = g.units[&uid].kind;
            let class = g.rules.units[kind].class.clone();
            let acted = match kind.as_str() {
                "settler" => self.advanced_settler_step(g, pid, uid),
                "builder" => self.advanced_builder_step(g, pid, uid, plan.strategy),
                "military_engineer" => self
                    .forward_airfield_engineer_step(g, pid, uid, plan)
                    .unwrap_or_else(|| self.base.military_engineer_step(g, pid, uid)),
                "naturalist" => self.base.naturalist_step(g, pid, uid),
                // `culture-digs-every-site`: see `advanced/culture_digs_every_site.rs`.
                "archaeologist" if self.dig_every_site_active(g) => {
                    self.dig_every_site_step(g, pid, uid)
                }
                "archaeologist" => self.base.archaeologist_step(g, pid, uid),
                "trader" => self.advanced_trader_step(g, pid, uid, plan.strategy),
                "missionary" if self.victory_planning => {
                    self.advanced_missionary_step(g, pid, uid, religious_offensive)
                }
                "missionary" => self.base.missionary_step(g, pid, uid),
                "rock_band" => self.base.rock_band_step(g, pid, uid),
                _ if self.victory_planning && class == "religious" => {
                    self.advanced_religious_step(g, pid, uid, religious_offensive)
                }
                _ => self.credited_military_step(g, pid, uid, plan, decline_settlers),
            };
            // A civilian's step can move its escort. See `advanced/last_orders.rs`.
            self.credit_orders(g, OrderStep::Escort);
            if !acted {
                break;
            }
            took_a_turn = true;
        }
        took_a_turn
    }

    pub(super) fn apply_unit_intents(
        &mut self,
        g: &mut Game,
        pid: usize,
        ids: &[u32],
        intents: Vec<UnitIntent>,
        plan: &StrategicPlan,
        flags: UnitTurnFlags,
    ) {
        let mut snapshot_invalid = false;
        for (&uid, intent) in ids.iter().zip(intents) {
            if snapshot_invalid || !g.units.contains_key(&uid) {
                self.base.clear_prepared_patrol_posts();
                let took_a_turn = g.units.contains_key(&uid)
                    && self.advance_unit_serial(
                        g,
                        pid,
                        uid,
                        plan,
                        flags.religious_offensive,
                        flags.decline_settlers,
                    );
                if !took_a_turn {
                    self.stand_down(g, pid, uid);
                }
                continue;
            }

            let mut took_a_turn = false;
            let mut valid = true;
            for action in &intent.actions {
                let invalidates_followers = match action {
                    Action::Move { to, .. } => {
                        g.barb_camps.contains_key(to)
                            || g.map.get(*to).is_some_and(|tile| {
                                matches!(
                                    tile.improvement.as_deref(),
                                    Some("goody_hut" | "meteor_goody" | "barbarian_camp")
                                )
                            })
                            || g.unit_ids_at(*to)
                                .iter()
                                .any(|other| g.units[other].owner != pid)
                    }
                    // Ordinary empty-tile movement, rebasing, patrol, and
                    // fortification only change this unit. Every other action
                    // available to the general planner can damage, remove,
                    // convert, or alter the shared map and therefore ends the
                    // immutable-position batch after it is committed.
                    Action::AirRebase { .. }
                    | Action::AirPatrol { .. }
                    | Action::Fortify { .. } => false,
                    _ => true,
                };
                if g.apply(pid, action).is_err() {
                    valid = false;
                    break;
                }
                took_a_turn = true;
                if matches!(
                    action,
                    Action::Attack { .. }
                        | Action::Ranged { .. }
                        | Action::AirStrike { .. }
                        | Action::PriorityTarget { .. }
                ) {
                    self.force_groups_dirty = true;
                }
                snapshot_invalid |= invalidates_followers;
            }
            self.credit_orders(g, intent.step);
            if valid {
                self.base.merge_unit_plan_state(uid, intent.base_state);
                took_a_turn |= intent.took_a_turn;
            } else if g.units.contains_key(&uid) {
                // Earlier moves can occupy a planned route. Keep anything
                // already committed, then finish this unit from live state.
                snapshot_invalid = true;
                took_a_turn |= self.advance_unit_serial(
                    g,
                    pid,
                    uid,
                    plan,
                    flags.religious_offensive,
                    flags.decline_settlers,
                );
            }
            if !took_a_turn {
                self.stand_down(g, pid, uid);
            }
        }
        self.base.clear_prepared_patrol_posts();
    }

    /// Whether this hull is on the sea-scout roster this turn. See
    /// `naval_explorer`.
    fn is_naval_explorer(&self, g: &Game, pid: usize, uid: u32) -> bool {
        (self.base.naval_recon_on() || self.base.island_exploration_active(g, pid))
            && g.rules.units[g.units[&uid].kind].domain.as_deref() == Some("sea")
            && self.naval_explorer(g, pid).contains(&uid)
    }

    /// An explorer's turn: slip out of a visible hostile's reach first, take
    /// an empty camp beside it, then sail or walk for the fog. `Some(acted)`
    /// when one of those settled the turn; `None` lets the caller's later
    /// branches have the unit. Shared by the wartime tactical path and the
    /// peacetime path so the two cannot drift apart again.
    pub(super) fn explorer_turn(&mut self, g: &mut Game, pid: usize, uid: u32) -> Option<bool> {
        // An empty camp beside the explorer is a completed reward, not fog
        // to scout past.
        if self.base.clear_adjacent_empty_barbarian_camp(g, pid, uid) {
            return Some(true);
        }
        if self.base.explore_step(g, pid, uid) {
            return Some(true);
        }
        None
    }

    /// The bounded set of ships that explore: the first two movable, unlinked
    /// sea military units with an open route while water remains unseen. A
    /// lake-bound hull cannot hold the job over a real scout. The production
    /// arm still buys only the first ship; this merely lets a spare hull open
    /// a distinct frontier instead of standing down. See `BasicAi::naval_recon`.
    pub(super) fn naval_explorer(&self, g: &Game, pid: usize) -> Vec<u32> {
        if !(self.base.naval_recon_on() || self.base.island_exploration_active(g, pid))
            || !BasicAi::unseen_water_remains(g, pid)
        {
            return Vec::new();
        }
        // `player_unit_ids` follows the roster's BTreeMap order.  The old
        // collect/sort/truncate therefore returned exactly the first two ids
        // that pass this predicate; once both are known, asking every later
        // ship to flood its waterway cannot change the roster.
        let ids = g.player_unit_ids(pid);
        debug_assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
        let mut ships = Vec::with_capacity(NAVAL_RECON_EXPLORER_MAX);
        for uid in ids {
            let unit = &g.units[&uid];
            let spec = &g.rules.units[unit.kind];
            if spec.class == "military"
                && spec.domain.as_deref() == Some("sea")
                && unit.moves_left > 0.0
                && unit.linked_to.is_none()
                && BasicAi::naval_recon_ship_can_chart(g, pid, uid)
            {
                ships.push(uid);
                if ships.len() == NAVAL_RECON_EXPLORER_MAX {
                    break;
                }
            }
        }
        ships
    }

    pub(super) fn advanced_units(&mut self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        // Observes only: see `advanced/last_orders.rs`.
        self.open_unit_orders(g, pid);
        self.builder_support.clear();
        self.base.begin_movement_turn(g, pid);
        self.refresh_wonder_clearance(g, pid);
        // In a native game a Trader has walking movement and the ordinary unit
        // loop below handles it. Firaxis exports an idle Trader with zero
        // walking movement but still permits TradeRoute from the city it
        // occupies. Handle exactly that bridge-only representation before the
        // ordinary loop, while it is still this player's turn.
        if self.live_trader_route_adapter {
            let stationary_traders = g
                .player_unit_ids(pid)
                .into_iter()
                .filter(|uid| {
                    let unit = &g.units[uid];
                    unit.kind == "trader" && unit.moves_left <= 0.0
                })
                .collect::<Vec<_>>();
            for uid in stationary_traders {
                let _ = self.start_zero_movement_trader_route(g, pid, uid, plan.strategy);
            }
        }
        // `objective-board` runs for every major seat, `victory_planning` or
        // not. See `advanced/objective_board.rs`.
        if self.victory_planning || self.objective_board {
            self.rebuild_force_groups(g, pid, plan);
        } else {
            self.force_groups.clear();
        }
        self.force_groups_dirty = false;
        let religious_offensive = self.religious_offensive_posture(g, pid, plan.strategy);
        // Settlement feasibility is an empire/map question, not a unit
        // question. It used to rescan the known world and recompute empire
        // counts up to eight times for every military unit in the same turn.
        let decline_settlers =
            self.counts(g, pid).settlers > 0 || !self.base.has_practical_settle_site(g, pid);
        // The ordinary order gives Settlers priority. While the first city is
        // still unfounded, this opt-in deliberately gives one nearby Warrior a
        // full turn first, then makes the Settler choose from its fresh sight.
        let opening_recon_warrior = self.opening_recon_warrior(g, pid);
        if let Some(warrior) = opening_recon_warrior {
            let before = g.units[&warrior].pos;
            let explored_before = g.players[pid].explored.len();
            let took_a_turn = self.advance_unit_serial(
                g,
                pid,
                warrior,
                plan,
                religious_offensive,
                decline_settlers,
            );
            if !took_a_turn {
                self.stand_down(g, pid, warrior);
            }
            self.refresh_opening_settler_targets_after_recon(
                g,
                pid,
                warrior,
                before,
                explored_before,
            );
        }
        // ★★★★ SETTLERS DECIDE BEFORE THE MILITARY PRE-PASS. See
        // `settler_stack_discipline`: the immediate-kill pass below can kill
        // raiders on the board, and a settler that reads the board afterwards
        // steps beside a raider the host may not have touched (t60 of
        // civvis-20260816T200454Z, `planned_risk 0.0` beside a live slinger).
        // Apart from `opening-warrior-recon-2` above, Settlers lead the unit
        // order; this only moves them ahead of the one thing that used to run
        // first.
        let mut settled_first: Vec<u32> = Vec::new();
        if self.live_formationless_settler_shadow {
            let mut settlers: Vec<u32> = g
                .player_unit_ids(pid)
                .into_iter()
                .filter(|uid| g.units[uid].kind == "settler")
                .collect();
            settlers.sort_unstable();
            for uid in settlers {
                let took_a_turn = self.advance_unit_serial(
                    g,
                    pid,
                    uid,
                    plan,
                    religious_offensive,
                    decline_settlers,
                );
                if !took_a_turn {
                    self.stand_down(g, pid, uid);
                }
                settled_first.push(uid);
            }
        }
        // Reserve civilian support before any military pre-pass can spend it.
        self.plan_builder_support(g, pid);
        if !self.builder_support.is_empty() {
            self.rebuild_force_groups(g, pid, plan);
        }
        let evacuated_aircraft = self.evacuate_air_bases(g, pid, plan);
        self.stage_culture_air_interception(g, pid, plan);
        let held_aircraft = self.hold_staged_culture_aircraft(g, pid, plan);
        self.execute_culture_air_interception(g, pid, plan);
        self.credit_orders(g, OrderStep::Air);
        // Hide waiting planes from the early coordinated assignments. Restore
        // their real readiness immediately afterwards: holding is no action,
        // not an invented engine operation or a permanent readiness change.
        let held_readiness: Vec<_> = held_aircraft
            .iter()
            .filter_map(|uid| {
                let unit = g.units.get_mut(uid)?;
                let saved = (*uid, unit.moves_left, unit.attacks_left);
                unit.moves_left = 0.0;
                unit.attacks_left = 0;
                Some(saved)
            })
            .collect();
        // A direct kill is a local opportunity, not a new campaign objective.
        // Resolve those exact, positive exchanges before the remaining unit
        // loop runs, then rebuild once if the board changed so the force
        // picture does not retain a dead defender in its old group.
        //
        // `battle-planner` takes this seam over: the turn's kills are planned
        // jointly and verified on one clone, the wounded are rotated out,
        // and the planned units are marked so the ladder below leaves them
        // alone. The immediate-kill pass and `fire-plan` are its unplanned
        // predecessors and do not run beside it. Either version of the
        // family; version two adds the positions plan inside. See
        // `advanced/battle_planner.rs`.
        let mut withdrawn = BTreeSet::new();
        // `war-kills-the-bands-2`: the band hunt claims its bodies before the
        // battle is planned. Nothing with the version off. See
        // `advanced/band_hunt.rs`.
        let early_band_hunters = if self.war_kills_the_bands_2 {
            self.plan_band_hunt(g, pid, plan, &BTreeSet::new())
        } else {
            BTreeSet::new()
        };
        self.credit_orders(g, OrderStep::Hunts);
        if self.battle_planner_on() {
            if self.plan_battle(g, pid, plan) {
                self.rebuild_force_groups(g, pid, plan);
                self.force_groups_dirty = false;
            }
            self.credit_orders(g, OrderStep::Positions);
            self.fire_plan_orders = fire_plan::FirePlan::default();
        } else {
            withdrawn = self.withdraw_before_kill_prepass(g, pid, plan);
            self.credit_orders(g, OrderStep::Withdraw);
            let killed = self.victory_planning
                && self.prioritize_immediate_kills(g, pid, plan, &withdrawn) > 0;
            self.credit_orders(g, OrderStep::ImmediateKills);
            if killed || !withdrawn.is_empty() {
                self.rebuild_force_groups(g, pid, plan);
                self.force_groups_dirty = false;
            }
            // `fire-plan`: this turn's kills, drawn once from the board the
            // unit loop is about to play, so the shooters that finish them go
            // first. Empty with the gene off. See `advanced/fire_plan.rs`.
            self.plan_fire(g, pid);
        }
        // `guns-stay-out-of-reach`: the tiles each siege gun must not end on,
        // read once the turn's kills are settled, and a gun already standing
        // on one steps off it. Nothing with the gene off. See
        // `advanced/guns_stay_out_of_reach.rs`.
        self.mark_gun_lethal_tiles(g, pid);
        self.guns_step_out_of_reach(g, pid);
        self.credit_orders(g, OrderStep::GunsOutOfReach);
        let mut air_assault_units = self.plan_air_city_assault(g, pid, plan);
        self.credit_orders(g, OrderStep::Air);
        for (uid, moves, attacks) in held_readiness {
            if let Some(unit) = g.units.get_mut(&uid) {
                unit.moves_left = moves;
                unit.attacks_left = attacks;
            }
        }
        air_assault_units.extend(held_aircraft);
        // `falling-city-calls-the-takers`: a falling city is taken, or its
        // nearest fit bodies close in. Nothing with the gene off. See
        // `advanced/falling_city_takers.rs`.
        let falling_takers = self.plan_falling_city_takers(g, pid, plan, &air_assault_units);
        self.credit_orders(g, OrderStep::FallingCity);
        air_assault_units.extend(falling_takers);
        // `air-surge-2` raiders: spare surge cavalry pillage behind the wing.
        // Nothing is read outside a Domination surge at war. See
        // `advanced/air_surge/raids.rs`.
        // `invest-keeps-its-cavalry`: see `assess_sieges_before_the_raids`.
        self.assess_sieges_before_the_raids(g, pid, plan);
        let raiders = self.plan_air_surge_raids(g, pid, plan, &air_assault_units);
        self.credit_orders(g, OrderStep::Raids);
        air_assault_units.extend(raiders);
        // A faithless Domination seat condemns the spreaders converting it.
        // See `advanced/air_surge/heretic_hunt.rs`.
        let hunters = self.plan_heretic_hunt(g, pid, &air_assault_units);
        self.credit_orders(g, OrderStep::Hunts);
        air_assault_units.extend(hunters);
        // `war-kills-the-bands`: an enemy Rock Band in reach is run down.
        // Nothing with the gene off. See `advanced/band_hunt.rs`.
        // Version two hunted before the battle (`early_band_hunters`).
        let band_hunters = if self.war_kills_the_bands_2 {
            early_band_hunters
        } else {
            self.plan_band_hunt(g, pid, plan, &air_assault_units)
        };
        self.credit_orders(g, OrderStep::Hunts);
        air_assault_units.extend(band_hunters);
        // `war-raids-the-pads`: a decisive space racer's standing pads are
        // pillaged from the air or on foot. Nothing with the gene off. See
        // `advanced/launcher_war.rs`.
        let pad_raiders = self.plan_pad_raids(g, pid, plan, &air_assault_units);
        self.credit_orders(g, OrderStep::Raids);
        air_assault_units.extend(pad_raiders);
        // `pass-picket`: this turn's recon orders, drawn once from the
        // start-of-turn board so units planned in parallel agree on them.
        // Nothing is read with the gene off. See
        // `advanced/recon_disruption.rs`.
        self.recon_disruption_plan(g, pid);
        // `chokepoint-garrison`: this turn's gates and their garrisons, drawn
        // once from the same board and for the same reason. Nothing is read
        // with the gene off. See `advanced/chokepoints.rs`.
        self.chokepoint_gate_plan(g, pid);
        // `defend-force-holds-the-city`: every threatened city's posts, drawn
        // after every other claim of the turn. Nothing with the gene off. See
        // `advanced/defend_force_holds_the_city.rs`.
        if self.defend_force_holds_the_city {
            let mut claimed = withdrawn.clone();
            claimed.extend(air_assault_units.iter().copied());
            claimed.extend(opening_recon_warrior);
            self.hold_the_cities(g, pid, &claimed);
            self.credit_orders(g, OrderStep::DefendHold);
        }
        let mut ids = g.player_unit_ids(pid);
        ids.retain(|uid| {
            !settled_first.contains(uid)
                && Some(*uid) != opening_recon_warrior
                && !withdrawn.contains(uid)
                && !air_assault_units.contains(uid)
                && !evacuated_aircraft.contains(uid)
        });
        // `breaker-walks-under-a-shield`: the breaker's shield moves first, so
        // the bodies that hold for it read where it stands. `None` with the
        // gene off, which keeps the shipped order.
        let shield_first = self.shield_moves_first(g, pid);
        ids.sort_by_key(|uid| {
            let u = &g.units[uid];
            let spec = &g.rules.units[u.kind];
            let order = match u.kind.as_str() {
                "settler" => 0,
                "builder" => 1,
                "naturalist" => 1,
                "archaeologist" => 1,
                "trader" => 2,
                "missionary" => 3,
                "rock_band" => 3,
                _ if spec.has_ranged_attack() && !spec.siege => 4,
                _ if spec.siege => 5,
                _ => 6,
            };
            // Zero for every unit with `fire-plan` off: the shipped key.
            (
                self.fire_plan_rank(*uid),
                Some(*uid) != shield_first,
                order,
                *uid,
            )
        });
        let pool = self
            .work_pool
            .clone()
            .filter(|_| !self.journal().is_recording());
        let mut cursor = 0;
        while cursor < ids.len() {
            let uid = ids[cursor];
            if pool.is_none() || !self.unit_uses_general_planner(g, uid) {
                let took_a_turn = self.advance_unit_serial(
                    g,
                    pid,
                    uid,
                    plan,
                    religious_offensive,
                    decline_settlers,
                );
                if !took_a_turn {
                    self.stand_down(g, pid, uid);
                }
                cursor += 1;
                continue;
            }

            let end = (cursor + 1..ids.len())
                .find(|index| !self.unit_uses_general_planner(g, ids[*index]))
                .unwrap_or(ids.len());
            let batch = ids[cursor..end].to_vec();
            if batch.len() == 1 {
                let took_a_turn = self.advance_unit_serial(
                    g,
                    pid,
                    uid,
                    plan,
                    religious_offensive,
                    decline_settlers,
                );
                if !took_a_turn {
                    self.stand_down(g, pid, uid);
                }
                cursor = end;
                continue;
            }

            let pool = pool.as_ref().expect("parallel unit batch has a pool");
            let batch_ids = Arc::new(batch.clone());
            self.base.prepare_patrol_posts(g, pid, &batch);
            let plan_snapshot = plan.clone();
            let active = pool.threads().min(batch.len());
            let states = (0..active)
                .map(|_| (g.speculative_clone(), self.clone()))
                .collect::<Vec<_>>();
            let intents = pool.map_stateful(batch.len(), states, move |(game, ai), indices| {
                indices
                    .map(|index| {
                        let mut branch = game.speculative_clone();
                        let mut planner = ai.clone();
                        let intent = planner.plan_general_unit_turn(
                            &mut branch,
                            pid,
                            batch_ids[index],
                            &plan_snapshot,
                            decline_settlers,
                        );
                        (index, intent)
                    })
                    .collect()
            });
            self.apply_unit_intents(
                g,
                pid,
                &batch,
                intents,
                plan,
                UnitTurnFlags {
                    religious_offensive,
                    decline_settlers,
                },
            );
            cursor = end;
        }
        // `breaker-walks-under-a-shield`: where the breaker ends our turn,
        // and this turn's strikes beside it. Nothing with the gene off.
        self.breaker_shield_note(g, pid);
        self.settler_targets
            .retain(|uid, _| g.units.contains_key(uid));
        self.settler_guards
            .retain(|settler, guard| g.units.contains_key(settler) && g.units.contains_key(guard));
        self.settler_sea_guards
            .retain(|settler, guard| g.units.contains_key(settler) && g.units.contains_key(guard));
        self.settler_escort_journeys.retain(|settler, journey| {
            g.units.contains_key(settler)
                && self.settler_targets.get(settler) == Some(&journey.target)
        });
        self.guard_wait.retain(|uid, _| g.units.contains_key(uid));
        self.settler_stalls
            .retain(|uid, _| g.units.contains_key(uid));
        self.settler_blocked_turns
            .retain(|uid, _| g.units.contains_key(uid));
        self.settler_avoid
            .retain(|uid, _| g.units.contains_key(uid));
        self.settler_closest
            .retain(|uid, _| g.units.contains_key(uid));
        self.settler_walk_started
            .retain(|uid, _| g.units.contains_key(uid));
        self.settler_walk_clock
            .retain(|uid, _| g.units.contains_key(uid));
        self.settler_idle_streak
            .retain(|uid, _| g.units.contains_key(uid));
        self.settler_stranded_at
            .retain(|uid, _| g.units.contains_key(uid));
        self.settler_relaxed_targets
            .retain(|uid, _| g.units.contains_key(uid));
        self.stalled_settler_floor
            .retain(|uid, _| g.units.contains_key(uid));
        self.builder_targets
            .retain(|uid, _| g.units.contains_key(uid));
        self.builder_avoid
            .retain(|uid, _| g.units.contains_key(uid));
        // A host-refused step first asks for a detour. Only sites with no
        // remaining route are deferred, and only for the refusal's lifetime.
        self.retire_frozen_settler_targets(g);
    }

    /// Retain reachable city sites across replans; a remembered refusal must
    /// not retire every new target for thirty turns without attempting it.
    pub(super) fn retire_frozen_settler_targets(&mut self, g: &Game) {
        if !self.live_move_refusal_break {
            return;
        }
        let frozen: Vec<u32> = self
            .settler_targets
            .keys()
            .copied()
            .filter(|uid| {
                self.base.move_refusal_blocked(g, *uid)
                    && self
                        .settler_refusal_waypoint(g, *uid, self.settler_targets[uid])
                        .is_none()
            })
            .collect();
        for uid in frozen {
            let Some(target) = self.settler_targets.remove(&uid) else {
                continue;
            };
            self.settler_relaxed_targets.remove(&uid);
            self.settler_dead_sites
                .entry(uid)
                .or_default()
                .insert(target, self.base.move_refusal_blocks[&uid].1);
            think!(self.journal(), Expansion, Detail,
                   "Settler retires a destination the host will not walk it toward";
                   "no route avoids the step refused on consecutive turns without the unit \
                    moving, so {target:?} is set aside and a fresh site is chosen";
                   target);
        }
    }
}
