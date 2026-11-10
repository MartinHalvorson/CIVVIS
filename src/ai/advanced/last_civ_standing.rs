//! A world played on to its last civilization leaves every seat one lane: conquest.

use super::*;

impl AdvancedAi {
    /// Once "one more turn" has made the survivor the only possible result,
    /// science, culture, faith and votes can win nothing, so every civilization
    /// plays for conquest for the rest of the game.
    pub(super) fn plays_for_the_last_civ(&mut self, g: &Game, pid: usize) {
        if g.plays_to_the_last_civ()
            && !g.players[pid].is_minor
            && !g.players[pid].is_barbarian
            && self.victory_target != Some(VictoryTarget::Domination)
        {
            self.retarget(VictoryTarget::Domination);
        }
    }
}
