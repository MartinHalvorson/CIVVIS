//! `launched-racer-yields-the-capital`: a rival that has launched its
//! Exoplanet Expedition and holds the city whose capture completes Domination.
//!
//! Once the Expedition is launched, "the only way to prevent a player from
//! achieving a Science Victory is to win another kind of Victory first, or
//! conquer that player" (Expansion2 Civilopedia). Its pads no longer matter:
//! `launcher-war-ignores-the-edge` stands down at the launch by design. When
//! that rival also holds the last original capital we need, taking the city
//! wins the game first, so:
//!
//! * `one_war_second_front_named` names the racer beside the running war at
//!   once, ahead of the front's refusal, its prey and `front_capital_to_finish`
//!   (whose only exemption is a culture finish);
//! * the city is the plan's objective at peace too (`domination_finish_front`
//!   reads only a rival we already fight, so at peace the science counter's
//!   Spaceport city won the objective);
//! * the declaration on it skips the major-war gate, `war_is_affordable` and
//!   the staging hold, and opens like the launcher war: a casus belli or a
//!   surprise war, no denouncement, coalition or production parity.
//!
//! Live Emperor civvis-20261014T155202Z: Babylon eliminated, every Dutch
//! original capital ours, India's Patna the last one (400 walls, defence
//! 116). India launched its Expedition at turn 215. Patna became the finishing
//! capital at 217 with five Giant Death Robots 8-26 tiles out and four Jet
//! Bombers one rebase from range, at 3.0 to 4.2 times India's power. The
//! campaign aimed at Jabalpur, a Spaceport city; the declaration returned on
//! `war_is_affordable` on 7 of turns 217-224 (100 + 25 Gold a city of reserve,
//! 650-800, against 141-294 Gold at negative income) and held "the army has
//! not finished staging" on the eighth. India won on Science at 225.

use super::{AdvancedAi, VictoryTarget, WAR_SOLVENCY_TURNS};
use crate::game::{Action, ActionFamilies, Game};
use crate::think;

/// The project a launched Expedition leaves in `Player::science_projects`.
const EXOPLANET_EXPEDITION: &str = "exoplanet_expedition";

impl AdvancedAi {
    /// The launched racer holding the city whose capture completes
    /// Domination, with that city: a met, living major whose Exoplanet
    /// Expedition is launched. `None` with the gene off, under a forced
    /// target, off the Domination lane, or with no Science Victory to lose.
    pub(crate) fn launched_racer_finish(&self, g: &Game, pid: usize) -> Option<(usize, u32)> {
        if !self.launched_racer_yields_the_capital
            || self.forced_target_player.is_some()
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !g.victory_conditions.science
        {
            return None;
        }
        g.cities
            .values()
            .filter(|city| {
                city.owner != pid
                    && g.players.get(city.owner).is_some_and(|owner| {
                        owner.alive
                            && !owner.is_minor
                            && !owner.is_barbarian
                            && !owner.is_free_city
                            && owner.science_projects.contains(EXOPLANET_EXPEDITION)
                    })
                    && g.has_met(pid, city.owner)
                    && Self::capture_completes_domination(g, pid, city.id)
            })
            .map(|city| (city.owner, city.id))
            .min()
    }

    /// The war on the launched racer, with its journal line: the surprise
    /// war, never a denouncement's five turns.
    pub(crate) fn launched_racer_opening(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        my_power: f64,
    ) -> Option<Action> {
        let surprise = g
            .legal_actions_within(pid, ActionFamilies::DIPLOMACY)
            .into_iter()
            .find(|action| matches!(action, Action::DeclareWar { player } if *player == target))?;
        if self.journal().wants(crate::reasoning::Level::Strategy) {
            let capital = self
                .launched_racer_finish(g, pid)
                .and_then(|(_, city)| g.cities.get(&city))
                .map(|city| city.name.clone())
                .unwrap_or_default();
            think!(self.journal(), Military, Strategy,
                   "Opening the war on {} for {}", g.players[target].civ, capital;
                   "launched-racer-yields-the-capital: its Exoplanet Expedition is launched and {} is the last capital Domination needs; {my_power:.0} power against their {:.0}",
                   capital, g.military_power(target));
        }
        Some(surprise)
    }

    /// Journal only, for every seat (no gene): a declaration that returns on
    /// `war_is_affordable` says so. The hold used to leave no line at all.
    pub(super) fn journal_treasury_holds_the_war(&self, g: &Game, pid: usize, target: usize) {
        if !self.journal().wants(crate::reasoning::Level::Detail) {
            return;
        }
        let reserve = 100.0 + 25.0 * g.player_city_ids(pid).len() as f64;
        let gold = g.players[pid].gold;
        let per_turn = g.players[pid].gold_per_turn;
        let reading = if per_turn < -0.5 && gold < reserve {
            format!("the treasury reserve: {gold:.0} gold of {reserve:.0} needed, at {per_turn:+.1} a turn")
        } else {
            format!(
                "the treasury runs dry: {gold:.0} gold at {per_turn:+.1} a turn lasts under {WAR_SOLVENCY_TURNS:.0} turns"
            )
        };
        think!(self.journal(), Military, Detail,
               "Holding the war on {}", g.players[target].civ;
               "{reading}");
    }
}

#[cfg(test)]
mod tests;
