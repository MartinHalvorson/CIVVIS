//! Give a Domination economy its first science/culture governor after the
//! non-founder religious defense has reached Citadel of God, before it can
//! train Apostles that benefit from Patron Saint.
//!
//! ## `domination-seat-appoints-pingala-first`
//!
//! A Domination target spends its first Governor titles on Pingala, seated
//! in its largest city, and on her Researcher promotion, instead of the
//! opening's Magnus.
//!
//! **Why (live Deity Gran Colombia, 4-player Tiny Pangaea, Online,
//! 2026-11-12).** The seat's binding gap is Science: 9-17 at t50 and 30-80
//! at t100 against the best rival's 150-200, 22-29 techs against 42-46, and
//! a city's strength follows its owner's best unit, so a siege a tier behind
//! does not land (civvis-20261112T043248Z stood at 771 power against
//! Russia's 197 at t100 and never left the staging ring of Smolensk, whose
//! strength read 67 against our best 36). In civvis-20261112T072713Z the
//! first three titles came at t28, t36 and t56 and all went to Magnus, in
//! the second city; Pingala was seated at t69 and took Researcher at t71.
//! The opening plan reads Expansion, whose roster starts with Magnus, and
//! the rule that finishes the first incumbent's foundation gives him the
//! next titles too. Researcher is +1 Science per citizen in her city and
//! Librarian +15% Science and Culture there: for the capital's seven to
//! nine citizens at t36-t60 that is 8 to 11 Science a turn while the whole
//! empire made 8 to 13.
//!
//! **The gene.** While the seat's victory target is Domination and Pingala
//! does not yet hold Researcher, a plan that reads Expansion or Conquest
//! orders its titles as Science does: Pingala is the first appointee
//! (`best_governor_city` seats her in the largest city), and Researcher is
//! her first promotion under any plan. Once Researcher is in, the plan's own
//! roster returns, so Magnus is the second Governor on an Expansion plan and
//! Victor on a Conquest plan. A Recovery plan keeps its Victor-first order:
//! a seat under attack still answers the attack first.
//!
//! A seated Pingala then answers a Loyalty emergency only in a city about to
//! flip, as `culture-keeps-pingala-home` does for a Culture seat: the base
//! Loyalty pass otherwise moves the only seated Governor to any city losing
//! Loyalty, and with her goes the Science of the capital.
//!
//! The cost is Magnus arriving two titles later (his Provision spares a
//! citizen per Settler trained in his city), so the gene is judged on cities
//! at t60 as well as on Science at t60 and t100. Off, or for any victory
//! target but Domination and Score, nothing changes.
//!
//! **A Score seat too (live Deity Rome, 6-player Continents, Online, turn
//! limit 170, from 2026-11-12 18:58Z).** The score counts 3 a civic, 5 a
//! city, 2 a district, 1 a building, 1 a citizen and 2 a technology
//! (`Game::score_parts`), and the Gran Colombia seat held 0.40 of the best
//! rival's score at every mark from t25 to t170, so the economy this gene
//! builds is the Score seat's aim as much as the Domination seat's. It reads
//! [`AdvancedAi::conquest_or_score_seat`].

use super::{AdvancedAi, GrandStrategy};
use crate::game::Game;

impl AdvancedAi {
    /// `domination-seat-appoints-pingala-first`: the gene is on and the seat
    /// plays the Domination lane or aims at Score
    /// ([`AdvancedAi::conquest_or_score_seat`]).
    pub(super) fn domination_seat_titles_pingala_first(&self, g: &Game) -> bool {
        self.domination_seat_appoints_pingala_first && self.conquest_or_score_seat(g)
    }

    /// `domination-seat-appoints-pingala-first`: whether this frame's titles
    /// follow the Science roster instead of `strategy`'s. True on an
    /// Expansion or Conquest plan while Pingala does not yet hold Researcher.
    pub(super) fn domination_titles_follow_science(
        &self,
        g: &Game,
        pid: usize,
        strategy: GrandStrategy,
    ) -> bool {
        matches!(strategy, GrandStrategy::Expansion | GrandStrategy::Conquest)
            && self.domination_seat_titles_pingala_first(g)
            && !g.players[pid]
                .governor_roster
                .get("pingala")
                .is_some_and(|state| state.promotions.contains("researcher"))
    }

    /// `domination-seat-appoints-pingala-first`: Researcher completes
    /// Pingala's foundation, so the next title seats the plan's own first
    /// Governor instead of her second promotion. Under
    /// `domination-seat-raises-its-culture` Connoisseur completes it instead
    /// (outside a Recovery `strategy`); see `advanced/domination_culture.rs`.
    pub(super) fn domination_pingala_founded(
        &self,
        g: &Game,
        pid: usize,
        governor: &str,
        strategy: GrandStrategy,
    ) -> bool {
        governor == "pingala"
            && self.domination_seat_titles_pingala_first(g)
            && g.players[pid]
                .governor_roster
                .get("pingala")
                .is_some_and(|state| state.promotions.contains("researcher"))
            && !self.domination_pingala_owes_connoisseur(g, pid, strategy)
    }

    pub(super) fn domination_needs_economic_governor(&self, g: &Game, pid: usize) -> bool {
        // A Score seat too: see [`AdvancedAi::conquest_or_score_seat`].
        self.conquest_or_score_seat(g)
            && g.players[pid]
                .governor_roster
                .get("moksha")
                .is_some_and(|state| state.promotions.contains("citadel_of_god"))
            && !g.player_city_ids(pid).into_iter().any(|cid| {
                let city = &g.cities[&cid];
                city.buildings.iter().any(|building| {
                    g.building_is_family(*building, crate::name!("temple"))
                        && !city.pillaged_buildings.contains(building)
                })
            })
            && !g.players[pid]
                .governor_roster
                .get("pingala")
                .is_some_and(|state| {
                    state.promotions.contains("researcher")
                        && state.promotions.contains("connoisseur")
                })
    }
}

#[cfg(test)]
mod tests;
