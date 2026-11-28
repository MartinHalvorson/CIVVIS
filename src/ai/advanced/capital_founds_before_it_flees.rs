//! `capital-founds-before-it-flees`: a seat with no city founds its capital
//! where its Settler stands instead of retreating, detouring or walking far.
//!
//! **The finding** (hotseat check game `crash-p4-fa`, 2026-11-28, seat p1,
//! Greece, agent jack, `--genome live`). The first Settler walked two turns
//! toward a better plot (177.4 against 151.7 where it began) and stood on it
//! on turn 3 with its full movement. A barbarian Scout three tiles off put
//! the tile's risk at 38 against `SETTLER_STEP_RISK_LIMIT` (30), so the
//! formationless retreat in `advanced_settler_step_inner` stepped it off the
//! site and the threat detour set the approach aside. On turns 4 to 8 the
//! capture lessons fled that Scout, seen on turns 3 and 5 and never again,
//! whose remembered envelope grows a tile a turn (`HOSTILE_MEMORY_TURNS`):
//! each turn the Settler stepped a tile or two outside it, eight tiles from
//! where it began, and gave up its site on turn 6. On turn 9 the memory
//! lapsed and it turned back toward a site "? turn(s) away"; on turn 10 that
//! walk uncovered a barbarian camp two tiles off, the camp's Warrior took
//! the Settler on the barbarian turn, and the seat, which had never held a
//! city, was eliminated (`DEFEAT_DEFAULT`). No other major was ever in sight.
//!
//! Every rule that moved it is right for a Settler of an empire that has a
//! city. For a seat with none, a capture ends the game, while a city cannot
//! be taken by stepping onto it: yet the opening branch refuses to found on
//! a tile whose risk passes the limit, and the shelter founding
//! (`settler_founds_for_shelter`) leaves a cityless seat out.
//!
//! **The gene** (opt-in, off). For a Settler of a seat that holds no city,
//! with movement left:
//!
//! 1. *Threat.* Before any retreat, detour or walk: when a known hostile (in
//!    sight or remembered, the reach every flee reads) could take its tile
//!    next turn and nothing guards it there, or the tile's risk passes the
//!    step limit, it founds where it stands, unless the visible attackers in
//!    reach would break the new city (`raiders_spare_the_city`). Where the
//!    engine refuses that tile, it takes the best neighbouring plot it can
//!    still found on this turn: one step and the city, the pair the agent's
//!    one-step founding hand-off sends together.
//! 2. *Deadline.* From [`CAPITAL_DEADLINE_TURNS`] turns after the seat first
//!    stepped a Settler without a city, it founds where it stands.
//! 3. *Walk.* In the opening branch, a walk longer than
//!    [`CAPITAL_WALK_TURNS`] turns (or of unknown length), a detour around a
//!    threat, or no site at all gives way to founding where it stands.
//!
//! A seat with a city, a Settler at sea and every board with the gene off
//! are untouched. A person's soldier counts as the threat it already is
//! under `person-units-threaten-civilians`: parking a Scout beside the first
//! Settler now gets the capital founded, not chased off.
use super::civilian_safety::REACH_SCAN_RADIUS;
use super::*;

/// The longest walk the first Settler still takes to a better plot.
pub(crate) const CAPITAL_WALK_TURNS: u32 = 2;

/// Turns after the seat first stepped a Settler without a city by which it
/// founds where it stands.
pub(crate) const CAPITAL_DEADLINE_TURNS: u32 = 3;

impl AdvancedAi {
    /// Rules 1 and 2, ahead of every retreat: `true` when the Settler founded
    /// (where it stood, or one step away). `false` with the gene off, for a
    /// seat that holds a city, and for a Settler that cannot act.
    pub(super) fn capital_founds_before_it_flees_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> bool {
        if !self.capital_founds_before_it_flees {
            return false;
        }
        if !g.player_city_ids(pid).is_empty() {
            self.capital_founds_clock = None;
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        if unit.owner != pid || unit.kind != "settler" || unit.moves_left <= 0.0 {
            return false;
        }
        let here = unit.pos;
        let first = *self.capital_founds_clock.get_or_insert(g.turn);
        let visible = self.battlefront_visibility(g, pid);
        let reach = self.barbarian_reach(g, pid, here, REACH_SCAN_RADIUS);
        let raiders = if self.civilian_safe_at(g, pid, uid, here, &reach) {
            0
        } else {
            reach.raiders_covering(g, here)
        };
        let risk = self.settlement_tile_risk(g, pid, Some(uid), here, &visible);
        let threatened = raiders > 0 || risk > SETTLER_STEP_RISK_LIMIT;
        let overdue = g.turn >= first + CAPITAL_DEADLINE_TURNS;
        if !threatened && !overdue {
            return false;
        }
        let why = if threatened {
            format!(
                "{raiders} raider(s) could take this tile next turn and its risk is {risk:.0} \
                 (limit {SETTLER_STEP_RISK_LIMIT:.0}); a Settler is taken by a step, a city is not"
            )
        } else {
            format!(
                "it has walked since turn {first}, {CAPITAL_DEADLINE_TURNS} turns without a city"
            )
        };
        if g.can_found_city(uid)
            && (!threatened || self.raiders_spare_the_city(g, pid, uid, here, &visible))
        {
            return self.found_the_capital_where_it_stands(g, pid, uid, &why);
        }
        if !threatened {
            return false;
        }
        // The engine refuses this tile: the best neighbouring plot founded on
        // this same turn, which needs movement left after the step.
        let mut best: Option<(f64, Pos)> = None;
        for step in g.nbrs(here) {
            if !g.can_move(uid, step) {
                continue;
            }
            let mut trial = g.speculative_clone();
            if trial
                .apply(
                    pid,
                    &Action::Move {
                        unit: uid,
                        to: step,
                    },
                )
                .is_err()
                || trial
                    .units
                    .get(&uid)
                    .is_none_or(|moved| moved.pos != step || moved.moves_left <= 0.0)
                || !trial.can_found_city(uid)
                || !self.raiders_spare_the_city(&trial, pid, uid, step, &visible)
            {
                continue;
            }
            let worth = self.settle_value(g, pid, step);
            if best.is_none_or(|(value, _)| worth > value) {
                best = Some((worth, step));
            }
        }
        let Some((_, step)) = best else {
            return false;
        };
        if g.apply(
            pid,
            &Action::Move {
                unit: uid,
                to: step,
            },
        )
        .is_err()
            || g.units
                .get(&uid)
                .is_none_or(|moved| moved.pos != step || moved.moves_left <= 0.0)
        {
            return false;
        }
        let why = format!("{why}; the engine refuses {here:?}, so it steps to {step:?}");
        self.found_the_capital_where_it_stands(g, pid, uid, &why)
    }

    /// Rule 3, in the opening branch once its target is known (`planned`
    /// before the threat detour, `target` after it): a walk longer than
    /// [`CAPITAL_WALK_TURNS`], a detour or no site at all gives way to
    /// founding where the Settler stands. `false` with the gene off and
    /// whenever the walk stands.
    pub(super) fn capital_founds_instead_of_walking(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        planned: Option<Pos>,
        target: Option<Pos>,
    ) -> bool {
        if !self.capital_founds_before_it_flees || !g.player_city_ids(pid).is_empty() {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let here = unit.pos;
        if unit.owner != pid
            || unit.kind != "settler"
            || unit.moves_left <= 0.0
            || target == Some(here)
            || !g.can_found_city(uid)
        {
            return false;
        }
        let why = match (planned, target) {
            (_, None) => "it has no site to walk to".to_string(),
            (Some(planned), Some(target)) if planned != target => format!(
                "the approach to {planned:?} is threatened and the detour leads to {target:?}"
            ),
            (_, Some(target)) => {
                let flood = Self::settle_sooner_walk_flood(g, uid, CAPITAL_WALK_TURNS as i32 + 1);
                match Self::opening_walk_turns(g, uid, here, target, &flood) {
                    Some(turns) if turns <= CAPITAL_WALK_TURNS => return false,
                    Some(turns) => format!("{target:?} is {turns} turns away"),
                    None => format!("{target:?} is more than {CAPITAL_WALK_TURNS} turns away"),
                }
            }
        };
        self.found_the_capital_where_it_stands(g, pid, uid, &why)
    }

    /// The founding itself, journaled after the engine answers (see the
    /// opening branch's note on why a Decision line asserts an applied
    /// action).
    fn found_the_capital_where_it_stands(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        why: &str,
    ) -> bool {
        let here = g.units[&uid].pos;
        let worth = self.settle_value(g, pid, here);
        let yielded = self.hidden_border_yield_before_founding(g, pid, here);
        let fogged = self.fog_yield_before_founding(g, pid, here);
        if g.apply(pid, &Action::FoundCity { unit: uid }).is_err() {
            return false;
        }
        self.note_hidden_border_founding(here, yielded);
        self.note_fog_founding(here, fogged);
        self.settler_targets.remove(&uid);
        self.settler_stalls.remove(&uid);
        self.settler_blocked_turns.remove(&uid);
        self.settler_retreats.remove(&uid);
        self.capital_founds_clock = None;
        think!(self.journal(), Expansion, Decision,
               "Founding the capital at {here:?} before the Settler runs";
               "capital-founds-before-it-flees: the seat holds no city and {why}; the site \
                is worth {worth:.1}"; here);
        true
    }
}

#[cfg(test)]
mod tests;
