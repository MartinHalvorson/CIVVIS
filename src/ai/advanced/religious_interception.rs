use super::*;

fn visible_home_spreaders(g: &Game, pid: usize, rival: usize, faith: &str) -> Vec<u32> {
    let visible = g.player_visibility(pid);
    g.player_unit_ids(rival)
        .into_iter()
        .filter(|uid| {
            let unit = &g.units[uid];
            g.rules.units[unit.kind].class == "religious"
                && unit.religion.as_deref() == Some(faith)
                && visible.contains(&unit.pos)
                && g.player_city_ids(pid)
                    .iter()
                    .any(|cid| g.wdist(g.cities[cid].pos, unit.pos) <= 6)
        })
        .collect()
}

/// The interception opens a major war to condemn one spreader, so it needs
/// at least the rival's power behind it. Live King 2026-10-03T100536Z
/// declared on Phoenicia at turn 50 with 228 power against their 317. The
/// next turn the plan read "at war and losing ground at home", and the war
/// ended in our own peace offer at turn 80 ("the last window was a rout").
pub(crate) const RELIGIOUS_INTERCEPTION_POWER_FLOOR: f64 = 1.0;

/// `religious-match-point-defence`: the interception beside a running war
/// needs this many times the faith's steady power (`steady_rival_power`),
/// besides no less power than the faith and every current enemy together.
/// The war condemns spreaders at home and asks no siege of the army, so it
/// sits between the interception's own 1.0 floor and the 1.5 edge
/// (`one_war::DECLARATION_EDGE_RATIO`) that counter wars aimed at cities
/// need. Live Emperor G186 at turn 69: 300 power against Indonesia's steady
/// 211 (1.42 times) and 295 with Sweden's 112, a Hindu Apostle beside our
/// archers; replayed with the turns' power memory, the 1.5 edge held it.
pub(crate) const MATCH_POINT_DEFENCE_EDGE: f64 = 1.2;

impl AdvancedAi {
    /// `religious-match-point-defence`: whether `rival`'s founded faith is
    /// at the religious match point read on the religion lane alone
    /// (`one_war::faith_at_match_point`). A rival's strongest lane can mask
    /// it: in live Emperor G186 Indonesia's score lead grew from 16% to 23%
    /// between turns 73 and 80 while its Hinduism held every major but one.
    /// False with the gene off.
    pub(crate) fn match_point_faith(&self, g: &Game, rival: usize) -> bool {
        self.religious_match_point_defence
            && g.players[rival].religion.is_some()
            && self.faith_at_match_point(g, rival)
    }

    /// `religious-match-point-defence`: a match-point faith
    /// (`match_point_faith`) whose spreader stands where the interception
    /// would condemn it. The war's opening is then urgent, as
    /// `faith_counter_spreaders_at_home` makes it for a counter target: only
    /// war lets our soldiers condemn it, and a Formal War waits five turns.
    pub(crate) fn match_point_spreaders_at_home(&self, g: &Game, pid: usize, rival: usize) -> bool {
        if !self.match_point_faith(g, rival) {
            return false;
        }
        let Some(faith) = g.players[rival].religion.as_deref() else {
            return false;
        };
        !visible_home_spreaders(g, pid, rival, faith).is_empty()
    }

    /// `religious-match-point-defence`: whether the interception may open on
    /// `rival` beside the wars already fought with `enemies`. The war to
    /// condemn a spreader at home needs no army at the faith's cities, so it
    /// is no second siege; it still opens only at
    /// [`MATCH_POINT_DEFENCE_EDGE`] times the faith's steady power and with
    /// no less power than the faith and every current enemy together. Live
    /// Emperor G186 at turn 73: 341 power against Indonesia's steady 195, and
    /// 337 with Sweden's 142; the shipped interception refused because the
    /// Sweden war was running, as it did on every turn from 53 to 83.
    pub(crate) fn match_point_defence_has_the_edge(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        enemies: &[usize],
    ) -> bool {
        let ours = g.military_power(pid);
        let edge = MATCH_POINT_DEFENCE_EDGE * self.steady_rival_power(g, rival).max(1.0);
        let together = g.military_power(rival)
            + enemies
                .iter()
                .filter(|enemy| **enemy != rival)
                .map(|enemy| g.military_power(*enemy))
                .sum::<f64>();
        self.religious_match_point_defence && ours >= edge && ours >= together
    }

    /// A defensive condemnation must not occupy the only major-war slot
    /// after its victory threat is gone and a different visible city can be
    /// campaigned against. The host still decides whether to accept white
    /// peace; this only asks when the game's minimum war term has elapsed.
    pub(super) fn religious_interception_handoff_peace(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        plan: &StrategicPlan,
    ) -> bool {
        let Some((intercepted, opened)) = self.religious_interception_war else {
            return false;
        };
        if intercepted != rival
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !g.is_at_war(pid, rival)
            || g.turn.saturating_sub(opened) < g.standard_duration(10).max(1)
            || g.peace_available_at(pid, rival).is_some()
            || g.emergency_war_pair(pid, rival)
            || self.urgent_victory_threat(g, rival)
            || self.religious_interception_holds_war(g, pid, rival)
            || plan.strategy != GrandStrategy::Conquest
            || plan.threatened_city.is_some()
            || plan.target_player == Some(rival)
            || self.one_war_prizes_in_reach(g, pid)
        {
            return false;
        }
        plan.target_city
            .and_then(|cid| g.cities.get(&cid))
            .is_some_and(|city| {
                Some(city.owner) == plan.target_player
                    && city.owner != pid
                    && !g.same_team(pid, city.owner)
                    && g.sees(&g.player_vision_frame(pid), city.pos)
            })
    }

    /// A war opened to stop an approaching religious victory should remain
    /// open while that founder still has visible spreaders near our cities.
    /// Once the local threat or the rival's victory stake recedes, ordinary
    /// peace evaluation resumes.
    pub(super) fn religious_interception_holds_war(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> bool {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !self.deny_leaders
            || !g.is_at_war(pid, rival)
        {
            return false;
        }
        let Some(stakes) = self.religious_veto_engaged(g, pid) else {
            return false;
        };
        if stakes.founder != rival || stakes.our_converted == 0 {
            return false;
        }
        let Some(faith) = g.players[rival].religion.as_deref() else {
            return false;
        };
        !visible_home_spreaders(g, pid, rival, faith).is_empty()
    }

    /// A religious match point can be intercepted at home even before a scout
    /// finds the founder's cities. Promise a legal condemnation, not a distant
    /// siege: replay the declaration and the same-turn interception first.
    pub(super) fn religious_interception_opening(&mut self, g: &mut Game, pid: usize) -> bool {
        let enemies: Vec<usize> = g
            .players
            .iter()
            .filter(|p| !p.is_minor && !p.is_barbarian && p.id != pid && g.is_at_war(pid, p.id))
            .map(|p| p.id)
            .collect();
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !self.deny_leaders
            || !g.victory_conditions.religious
            || !self.war_is_affordable(g, pid)
            || self.threatened_city(g, pid).is_some()
            // `religious-match-point-defence`: a running war no longer
            // refuses the interception outright; each rival must clear
            // `match_point_defence_has_the_edge` below instead.
            || (!enemies.is_empty() && !self.religious_match_point_defence)
        {
            return false;
        }
        for (rival, pressure) in self.ranked_rival_victory_pressures(g, pid, &BTreeMap::new()) {
            // See `match_point_faith`: under the gene the match point is read
            // on the religion lane alone, whatever lane the rival leads.
            let at_match_point = (pressure.strategy == GrandStrategy::Religion
                && self.victory_pressure_is_urgent(g, rival, pressure))
                || self.match_point_faith(g, rival);
            if !at_match_point
                || !self.campaign_target_legal(g, pid, rival)
                || g.military_power(pid)
                    < RELIGIOUS_INTERCEPTION_POWER_FLOOR * g.military_power(rival)
            {
                continue;
            }
            let Some(faith) = g.players[rival].religion.as_deref() else {
                continue;
            };
            let spreaders = visible_home_spreaders(g, pid, rival, faith);
            if spreaders.is_empty() {
                continue;
            }
            if !enemies.is_empty() {
                if g.is_at_war(pid, rival) {
                    continue;
                }
                if !self.match_point_defence_has_the_edge(g, pid, rival, &enemies) {
                    if self.journal().wants(crate::reasoning::Level::Detail) {
                        let together = g.military_power(rival)
                            + enemies.iter().map(|e| g.military_power(*e)).sum::<f64>();
                        think!(self.journal(), Military, Detail,
                            "Holding the religious interception on {}", g.players[rival].civ;
                            "religious-match-point-defence: their faith holds every major but one and its spreader is at home, but beside the running war we have {:.0} power against their steady {:.0} (needs {:.1}x) and {:.0} with every enemy",
                            g.military_power(pid), self.steady_rival_power(g, rival),
                            MATCH_POINT_DEFENCE_EDGE, together);
                    }
                    continue;
                }
            }
            let Some(opening) = self.preferred_war_opening(g, pid, rival) else {
                continue;
            };
            if !matches!(
                opening,
                Action::DeclareWar { .. } | Action::DeclareWarWithCasusBelli { .. }
            ) {
                continue;
            }
            let mut declared = g.speculative_clone();
            if declared.apply(pid, &opening).is_err() {
                continue;
            }
            for uid in g.player_unit_ids(pid) {
                let unit = &g.units[&uid];
                let spec = &g.rules.units[unit.kind];
                if spec.class != "military"
                    || spec.domain.as_deref() == Some("air")
                    || unit.hp as f64 <= self.base.w.withdraw_hp
                    || unit.moves_left <= 0.0
                {
                    continue;
                }
                for target_unit in &spreaders {
                    let position = g.units[target_unit].pos;
                    if g.wdist(unit.pos, position) > 1 {
                        continue;
                    }
                    let mut forecast = declared.speculative_clone();
                    let movement = (unit.pos != position).then_some(Action::Move {
                        unit: uid,
                        to: position,
                    });
                    if movement
                        .as_ref()
                        .is_some_and(|a| forecast.apply(pid, a).is_err())
                    {
                        continue;
                    }
                    let condemn = Action::CondemnHeretic {
                        unit: uid,
                        target_unit: *target_unit,
                    };
                    if forecast.apply(pid, &condemn).is_err() {
                        continue;
                    }
                    if g.apply(pid, &opening).is_err() {
                        return false;
                    }
                    self.religious_interception_war = Some((rival, g.turn));
                    if let Some(action) = movement {
                        if g.apply(pid, &action).is_err() {
                            return true;
                        }
                    }
                    let condemned = g.apply(pid, &condemn).is_ok();
                    if enemies.is_empty() {
                        think!(self.journal(), Military, Strategy,
                            "Opening a religious interception against {}", g.players[rival].civ;
                            "a visible spreader at home supplies an immediate counter to the religious match point; condemnation executed: {condemned}");
                    } else {
                        think!(self.journal(), Military, Strategy,
                            "Opening a religious interception against {} beside the running war", g.players[rival].civ;
                            "religious-match-point-defence: their faith holds every major but one and its spreader is at home; we hold the edge over them and every enemy together; condemnation executed: {condemned}");
                    }
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests;
