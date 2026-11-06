//! `spy-civics-come-first`: a Science aim researches the civics that add a
//! Spy slot (Nationalism, Ideology, Cold War) sooner, with their direct
//! prerequisites at half the weight.
//!
//! At Deity the one win of the Korea science lane (live 122513Z, 2026-11-05)
//! came from the Spies: 14 stolen Eurekas closed a nine-tech gap, and 17
//! Disrupts held Mali at two space projects and Scotland and England at three
//! while we launched. It fielded three Spies on the map from turn 140 to 180
//! with four slots; the median Deity game that day fielded one with two.
//! Spy slots come from the Intelligence Agency and from civics: one each at
//! Diplomatic Service, Nationalism, Ideology and Cold War. The win reached
//! its third slot at turn 119 and its fourth at 152; 125414Z at 158 and 174,
//! 105753Z at 131 and 173. Diplomatic Service is already brought forward by
//! `spies-steal-the-lead` (with Guilds); this gene does not touch either.
//!
//! On, under a Science aim, [`SPY_CIVIC_VALUE`] is added to Nationalism,
//! Ideology and Cold War, and half of it to The Enlightenment, Mass Media and
//! Mobilization (their prerequisites), in `civic_value`. `Kind::OptIn`; off,
//! the civic order is unchanged.

use super::*;

/// The weight a Science aim adds to a civic that opens a Spy slot (half to
/// its direct prerequisites): `spies-steal-the-lead`'s for Diplomatic
/// Service.
pub(crate) const SPY_CIVIC_VALUE: f64 = super::research::STEAL_CIVIC_VALUE;

impl AdvancedAi {
    /// The term `civic_value` adds for `civic` under the gene: zero off, off a
    /// Science aim, and for every other civic.
    pub(super) fn spy_civic_value(&self, strategy: GrandStrategy, civic: &str) -> f64 {
        if !self.spy_civics_come_first
            || !(strategy == GrandStrategy::Science
                || self.victory_target == Some(VictoryTarget::Science))
        {
            return 0.0;
        }
        match civic {
            "nationalism" | "ideology" | "cold_war" => SPY_CIVIC_VALUE,
            "the_enlightenment" | "mass_media" | "mobilization" => SPY_CIVIC_VALUE / 2.0,
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::opt_in_off_in_both_controllers;
    use super::super::{AdvancedAi, GrandStrategy, VictoryTarget};
    use crate::game::Game;

    #[test]
    fn spy_civics_come_first_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("spy-civics-come-first", |ai| ai.spy_civics_come_first);
    }

    fn value(ai: &AdvancedAi, g: &Game, civic: &str, strategy: GrandStrategy) -> f64 {
        ai.civic_value(g, 0, civic, strategy)
    }

    #[test]
    fn a_science_seat_values_the_spy_civics_sooner() {
        let g = Game::new_full(2, 24, 16, 91_771, 250, 0, false);
        let off = AdvancedAi::targeting(VictoryTarget::Science);
        let mut on = AdvancedAi::targeting(VictoryTarget::Science);
        on.enable_spy_civics_come_first();
        let science = GrandStrategy::Science;
        for civic in [
            "nationalism",
            "ideology",
            "cold_war",
            "the_enlightenment",
            "mass_media",
            "mobilization",
        ] {
            assert!(
                value(&on, &g, civic, science) > value(&off, &g, civic, science),
                "{civic}"
            );
        }
        assert_eq!(
            on.spy_civic_value(science, "nationalism"),
            2.0 * on.spy_civic_value(science, "the_enlightenment")
        );
        for civic in ["diplomatic_service", "guilds", "feudalism", "civil_service"] {
            assert_eq!(
                value(&on, &g, civic, science),
                value(&off, &g, civic, science),
                "{civic} is untouched"
            );
        }
    }

    #[test]
    fn off_a_science_aim_the_civics_are_untouched() {
        let g = Game::new_full(2, 24, 16, 91_771, 250, 0, false);
        let off = AdvancedAi::targeting(VictoryTarget::Domination);
        let mut on = AdvancedAi::targeting(VictoryTarget::Domination);
        on.enable_spy_civics_come_first();
        for civic in ["nationalism", "ideology", "cold_war"] {
            assert_eq!(
                value(&on, &g, civic, GrandStrategy::Conquest),
                value(&off, &g, civic, GrandStrategy::Conquest),
                "{civic}"
            );
        }
    }
}
