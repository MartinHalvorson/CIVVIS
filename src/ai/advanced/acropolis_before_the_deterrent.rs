//! `acropolis-before-the-deterrent`: at peace, a Culture target's first
//! Theater Square that can finish quickly goes ahead of a `near-rival-deterrence`
//! unit in the same queue.
//!
//! **Why (live Deity Pericles, 2026-11-08, 29 games).** Within 40 turns of
//! Drama and Poetry there were 1,153 city-turns with a legal, unbuilt
//! Acropolis. A military unit held the queue in 457 of them, 319 at peace. The
//! Theater claim yields to any land unit the deterrence claim still wants, and
//! that claim trains while a neighbour out-guns us, which at Deity is most of
//! the window from t30 to t80. An Acropolis costs 27 production at Online,
//! about two turns in a capital, and pays Culture, Great Person points and an
//! Envoy for the rest of the game. In 235547Z Athens made 13-14 production with
//! a legal site from t40 to t60 and built Hoplites, Archers and a Horseman.
//!
//! **What the gene does.** Inside the Theater claim, a deterrent at the head
//! of the queue no longer holds it when the Theater would finish within
//! [`QUICK_THEATER_STANDARD_TURNS`]. The unit keeps its progress and returns
//! behind the district. Every other hold, and every wartime rule, is unchanged.

use super::*;

/// The longest build, in Standard turns (four at Online), that may go ahead
/// of a deterrent.
const QUICK_THEATER_STANDARD_TURNS: f64 = 8.0;

impl AdvancedAi {
    /// Whether a first Theater Square of `turns` build time passes a deterrent
    /// at the head of its city's queue. False with the gene off.
    pub(super) fn quick_theater_passes_the_deterrent(&self, g: &Game, turns: f64) -> bool {
        self.acropolis_before_the_deterrent
            && turns <= g.game_speed.scale(QUICK_THEATER_STANDARD_TURNS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Game {
        Game::new_full(2, 30, 20, 79_208, 250, 0, false)
    }

    #[test]
    fn off_never_passes() {
        let ai = AdvancedAi::targeting(VictoryTarget::Culture);
        assert!(!ai.quick_theater_passes_the_deterrent(&game(), 1.0));
    }

    #[test]
    fn a_quick_theater_passes_and_a_slow_one_does_not() {
        let g = game();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
        ai.enable_acropolis_before_the_deterrent();
        let limit = g.game_speed.scale(QUICK_THEATER_STANDARD_TURNS);
        assert!(ai.quick_theater_passes_the_deterrent(&g, limit));
        assert!(!ai.quick_theater_passes_the_deterrent(&g, limit + 0.5));
    }
}
