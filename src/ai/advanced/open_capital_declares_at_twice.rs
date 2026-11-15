//! `open-capital-declares-at-twice` (opt-in, Domination only): a seat holding
//! [`OPEN_CAPITAL_EDGE`] times its campaign target's steady military, while
//! the target's original capital stands with no walls inside the declaration
//! range and reachable by land, declares on that target as on prey.
//!
//! What it waives, and only for that target:
//!
//! - the Objective Board's two holds in `war_policy_declaration`
//!   (`war-policy-via-board`): "the Defend row at … is short N unit(s)" and
//!   "the Siege row for … asks N strength and M is staged on its ring";
//! - the staged-ring requirement of the declaration step itself (it joins
//!   `prey_is_declared_on_now`, the way `prey-is-declared-on` does);
//! - `declaration-needs-production-parity`: an open capital falls inside the
//!   first turns of the war or not at all, before the target's Production
//!   can reinforce it.
//!
//! What still applies: the Conquest plan, another major war, the treasury,
//! the declaration range, the coalition invitation, the denouncement and its
//! turns (the Formal War), `prey-waits-for-the-march` (the field army's march
//! to the objective), and `declaration-needs-a-capture` (the soldiers in reach
//! must forecast a capture before the war itself is declared). Melee and
//! cavalry take a city without walls with no breaker, so none is asked.
//!
//! **Why (live Immortal, 2026-11-15).** No Domination game of the day
//! declared a conquest war of its own. In the two that reached twice their
//! neighbour's power with its capital still open, the board held the war to
//! retirement:
//!
//! - civvis-20261115T172539Z: Arabia at 0.43-0.49 of our power t106-111,
//!   Cairo without walls 15 tiles from our nearest city; "the Defend row at
//!   Bad-Tibira is short 1 unit(s)" (x12) and "the Siege row for Damascus asks
//!   79 strength and 65 is staged on its ring in 2 bodies" (x7).
//! - civvis-20261115T181018Z: Germany at 0.47-0.50 t100-102 (its friendship
//!   lapsed at t99), Aachen without walls 11 tiles out; "the Defend row at
//!   Larsa is short 1 unit(s)" and "the Siege row for Hong Kong asks 122
//!   strength and -0 is staged".
//!
//! Both seats were out-produced about two to one (Arabia 110 against our
//! 62-71, Germany 229-247 against 112-123), so production parity would have
//! held them after the board. The Deity games of the same day never met the
//! condition: their neighbours' capitals were walled by turn 80.
//!
//! Off, nothing changes.

use super::*;

/// Our military over the target's steady military (`steady_rival_power`) at
/// which an open capital is declared on.
pub(super) const OPEN_CAPITAL_EDGE: f64 = 2.0;

impl AdvancedAi {
    /// `open-capital-declares-at-twice`: the id of `target`'s original
    /// capital when the gene declares on `target` this turn, else `None`.
    /// Always `None` with the gene off or off the Domination lane.
    pub(super) fn open_capital_at_twice(&self, g: &Game, pid: usize, target: usize) -> Option<u32> {
        if !self.open_capital_declares_at_twice
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return None;
        }
        let rival = g.players.get(target)?;
        if target == pid
            || !rival.alive
            || rival.is_minor
            || rival.is_barbarian
            || rival.is_free_city
            || g.is_at_war(pid, target)
            || !self.campaign_target_legal(g, pid, target)
        {
            return None;
        }
        let ours = g.military_power(pid);
        if ours < OPEN_CAPITAL_EDGE * self.steady_rival_power(g, target).max(1.0) {
            return None;
        }
        let capital = g
            .player_city_ids(target)
            .into_iter()
            .filter_map(|cid| g.cities.get(&cid))
            .find(|city| city.is_capital && city.original_owner == target)?;
        (capital.wall_hp <= 0
            && Self::city_within_declaration_range(g, pid, capital.pos)
            && self.rival_reachable_by_land(g, pid, target))
        .then_some(capital.id)
    }

    /// The journal line for a declaration step the gene opens.
    pub(super) fn journal_open_capital_at_twice(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        capital: u32,
    ) {
        if !self.journal().wants(crate::reasoning::Level::Detail) {
            return;
        }
        let Some(city) = g.cities.get(&capital) else {
            return;
        };
        let tiles = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| g.cities.get(&cid))
            .map(|ours| g.wdist(ours.pos, city.pos))
            .min()
            .unwrap_or(i32::MAX);
        think!(self.journal(), Military, Detail,
               "Opening on {} at its open capital", g.players[target].civ;
               "open-capital-declares-at-twice: {} stands without walls {tiles} tiles from our nearest city; \
                {:.0} power against their steady {:.0} (at least {OPEN_CAPITAL_EDGE:.1} times): the board's \
                Defend and Siege rows and production parity are not asked",
               city.name, g.military_power(pid), self.steady_rival_power(g, target);
               city.pos);
    }
}

#[cfg(test)]
mod tests;
