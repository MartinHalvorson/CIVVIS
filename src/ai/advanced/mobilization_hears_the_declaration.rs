//! `mobilization-hears-the-declaration`: `surprise-war-mobilization` also
//! opens its window on a surprise declaration the host announced to us, not
//! only on a record in `Game::wars`.
//!
//! ## Why (live Deity, 6 players, Small Continents, Online; 11-18/19)
//!
//! `surprise-war-mobilization` is armed in every live game and never fires
//! there. Its window (`AdvancedAi::surprise_defense_window`) reads
//! `Game::wars` for a record whose casus belli is `surprise_war`, and the
//! live mirror writes only `Game::at_war` (`mirror.rs`, the rival and minor
//! war bonds), so the war ledger is empty on every live board. Over the 129
//! live games of 11-18/19, 138 wars were declared on us; 34 of the 103 that
//! brought an enemy soldier within two tiles of the original capital took it.
//!
//! The control mod already logs each declaration it closes: a
//! `diplomacy_choice` line with `statement` `DECLARE_SURPRISE_WAR`,
//! `from_player` the declarer and `to_player` us. 131 of those 138 wars carry
//! a `DECLARE_*_WAR` line (122 of them `DECLARE_SURPRISE_WAR`), always after
//! the last state row of the turn before the war shows on the board.
//!
//! ## What it does
//!
//! The live bridge reads those lines from the run's `events.jsonl`
//! (`DeclarationLedger` in `civvis_orders`), only while this gene is on, and
//! hands the AI each surprise declarer's seat with the turn the war began
//! (the turn after the state row the line follows). With
//! `surprise-war-mobilization` also on, a declaration heard this way opens
//! the same bounded window a `surprise_war` record would, under the same
//! guards: the declarer is a living major, we are at war with it now, and the
//! window is six Standard-speed turns from the declaration. A record in
//! `Game::wars` still wins when there is one.
//!
//! Nothing is written to `Game::wars`, so its other readers
//! (`unchosen-war-keeps-the-lane`, `edge-unit-takes-the-soft-city`, the
//! attack-envelope fingerprint) see exactly what they saw before. Off, the
//! bridge reads nothing and the window is unchanged: byte-identical.

use super::surprise_defense::{SurpriseDefenseWindow, SURPRISE_MOBILIZATION_TURNS};
use super::*;

impl AdvancedAi {
    /// Whether `mobilization-hears-the-declaration` is on.
    pub fn mobilization_hears_the_declaration_enabled(&self) -> bool {
        self.mobilization_hears_the_declaration
    }

    /// The live bridge: `(declarer seat, turn the war began)` of each
    /// surprise declaration on us the run's log holds. Replaces what was
    /// there.
    pub fn observe_surprise_declarations(&mut self, declarations: Vec<(usize, u32)>) {
        self.observed_surprise_declarations = declarations;
    }

    /// The newest heard surprise declaration whose window is open this turn.
    /// `None` with the gene off.
    pub(crate) fn heard_surprise_defense_window(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<SurpriseDefenseWindow> {
        if !self.mobilization_hears_the_declaration {
            return None;
        }
        let duration = g.standard_duration(SURPRISE_MOBILIZATION_TURNS).max(1);
        self.observed_surprise_declarations
            .iter()
            .filter(|(attacker, declared)| {
                *attacker != pid
                    && g.players.get(*attacker).is_some_and(|player| {
                        player.alive && !player.is_minor && !player.is_barbarian
                    })
                    && g.is_at_war(pid, *attacker)
                    && *declared <= g.turn
                    && g.turn < declared.saturating_add(duration)
            })
            .max_by_key(|(attacker, declared)| (*declared, std::cmp::Reverse(*attacker)))
            .map(|(attacker, declared)| SurpriseDefenseWindow {
                attacker: *attacker,
                declared: *declared,
                ends: declared.saturating_add(duration),
            })
    }
}

#[cfg(test)]
mod tests;
