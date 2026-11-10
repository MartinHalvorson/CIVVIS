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

    /// PROBE: the land army a seat wants in the final war.
    pub(super) fn last_civ_army_target(
        &self,
        g: &Game,
        pid: usize,
        city_count: usize,
        shipped: usize,
    ) -> usize {
        let bodies = crate::lcs_knobs::get("bodies");
        let player = &g.players[pid];
        if bodies <= 0.0 || !g.plays_to_the_last_civ() || player.is_minor || player.is_barbarian {
            return shipped;
        }
        if player.gold_per_turn < 0.0 && player.gold < 20.0 * -player.gold_per_turn {
            return shipped;
        }
        shipped.max((bodies * city_count as f64).ceil() as usize)
    }
}
