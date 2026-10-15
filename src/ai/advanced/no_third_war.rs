//! `no-third-war`: with two majors already at war with us, the seat opens no
//! war of its own on a third, whichever opener asks (the campaign, its second
//! front, the appointed and air-surge attacks, the conquest opening, the
//! raid, the culture embargo, the religious interception, the space denial).
//!
//! On the 25 Emperor gate passers of 2026-10-14 07:25Z to 10-15 09:40Z (the
//! census by -34's war-count fork; 1,808 turns from 120 to 200, 22 captures)
//! the 137 turns at war with three majors took no city, against 13.7 captures
//! per 1,000 turns at one war and 17.0 at two. Controlled for our power over
//! the strongest enemy at war (1.5 times or more): 16.0 at one war, 28.5 at
//! two, 0 at three (41 turns).
//!
//! A game-ending clock is exempt: a rival the counters read urgent
//! (`urgent_victory_threat`: a space race past its Moon, a religious or
//! diplomatic match point, a culture race about to finish), and the launched
//! racer holding the capital that completes Domination
//! (`launched_racer_finish`). The launcher and launched-racer openings are
//! not gated at all, and the faith war already holds itself at two wars
//! (`FAITH_WAR_MAX_ENEMIES`). A war the AI declares on us is not ours to
//! stop. The campaign also stops naming such a third rival its second front
//! (`one_war_second_front_unstaged`), so the plan does not aim an army at a
//! war it will not open.
//!
//! Off: unchanged.

use super::*;

/// `no-third-war`: no war of ours opens on another major once this many are
/// at war with us.
pub(crate) const THIRD_WAR_ENEMIES: usize = 2;

/// `no-third-war`: standard turns between two lines about one rival, unless
/// the reading changes (held to exempt or back).
const THIRD_WAR_NOTE_TURNS: u32 = 5;

impl AdvancedAi {
    /// `no-third-war`: whether `target` is a major at peace with us while
    /// two or more others are at war with us, and no game-ending clock
    /// exempts it. Pure; see [`AdvancedAi::third_war_held`] for the journal.
    pub(crate) fn third_war_would_open(&self, g: &Game, pid: usize, target: usize) -> bool {
        self.no_third_war
            && g.players.get(target).is_some_and(|rival| {
                rival.id != pid && rival.alive && !rival.is_minor && !rival.is_barbarian
            })
            && !g.is_at_war(pid, target)
            && self
                .one_war_enemies(g, pid)
                .into_iter()
                .filter(|enemy| *enemy != target)
                .count()
                >= THIRD_WAR_ENEMIES
    }

    /// `no-third-war`: whether a game-ending clock exempts a third war on
    /// `target`, and which.
    pub(crate) fn third_war_exemption(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
    ) -> Option<&'static str> {
        if self
            .launched_racer_finish(g, pid)
            .is_some_and(|(racer, _)| racer == target)
        {
            Some("its Exoplanet Expedition is launched and it holds the last capital Domination needs")
        } else if self.urgent_victory_threat(g, target) {
            Some("its victory clock is urgent")
        } else {
            None
        }
    }

    /// `no-third-war`: whether the war a caller of ours would open on
    /// `target` is held, journalling the hold (or the clock that exempts
    /// it) once per [`THIRD_WAR_NOTE_TURNS`] standard turns. `false` with the
    /// gene off.
    pub(crate) fn third_war_held(&mut self, g: &Game, pid: usize, target: usize) -> bool {
        if !self.third_war_would_open(g, pid, target) {
            return false;
        }
        let exemption = self.third_war_exemption(g, pid, target);
        let held = exemption.is_none();
        let every = g.standard_duration(THIRD_WAR_NOTE_TURNS);
        let due = self
            .no_third_war_noted
            .get(&target)
            .is_none_or(|(was_held, turn)| {
                *was_held != held || g.turn.saturating_sub(*turn) >= every
            });
        if due {
            self.no_third_war_noted.insert(target, (held, g.turn));
        }
        if due && self.journal().wants(crate::reasoning::Level::Detail) {
            let enemies = self
                .one_war_enemies(g, pid)
                .into_iter()
                .filter(|enemy| *enemy != target)
                .map(|enemy| g.players[enemy].civ.to_string())
                .collect::<Vec<_>>();
            let at_war = match enemies.as_slice() {
                [a, b] => format!("{a} and {b}"),
                many => many.join(", "),
            };
            match exemption {
                None => {
                    think!(self.journal(), Military, Detail,
                           "Holding off a third war on {}", g.players[target].civ;
                           "no-third-war: already at war with {at_war}; three-war turns converted 0 of 137");
                }
                Some(clock) => {
                    think!(self.journal(), Military, Detail,
                           "Opening a third war on {} for its clock", g.players[target].civ;
                           "no-third-war exempts a game-ending clock: {clock}; already at war with {at_war}");
                }
            }
        }
        held
    }
}

#[cfg(test)]
mod tests;
