//! `spies-steal-the-lead`'s civic term in `civic_value`.

use super::*;
use crate::ai::advanced::VictoryTarget;

fn value(ai: &AdvancedAi, g: &Game, civic: &str, strategy: GrandStrategy) -> f64 {
    ai.civic_value(g, 0, civic, strategy)
}

#[test]
fn a_stealing_science_seat_values_diplomatic_service_sooner() {
    let g = Game::new_full(2, 24, 16, 91_771, 250, 0, false);
    let off = AdvancedAi::targeting(VictoryTarget::Science);
    let mut on = AdvancedAi::targeting(VictoryTarget::Science);
    on.enable_spies_steal_the_lead();
    let science = GrandStrategy::Science;
    assert!(
        value(&on, &g, "diplomatic_service", science)
            > value(&off, &g, "diplomatic_service", science)
    );
    assert!(value(&on, &g, "guilds", science) > value(&off, &g, "guilds", science));
    for civic in ["feudalism", "civil_service", "political_philosophy"] {
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
    on.enable_spies_steal_the_lead();
    for civic in ["diplomatic_service", "guilds"] {
        assert_eq!(
            value(&on, &g, civic, GrandStrategy::Conquest),
            value(&off, &g, civic, GrandStrategy::Conquest),
            "{civic}"
        );
    }
}
