//! `culture-deck-seats-the-great-person-cards`: a seat aiming at a Culture
//! victory wants Literary Tradition (+2 Great Writer points a turn, Drama and
//! Poetry) and Frescoes (+2 Great Artist points a turn and +2 more for every
//! Art Museum, Humanism) in its wildcard slots.
//!
//! **Why (live Deity, 6-player Continents, Online, the 30 Culture-seat games
//! of 2026-11-18/19 that reached turn 150).** Visitors come from Great Works,
//! and Great Works come from Great Writers, Artists and Musicians, which the
//! Deity rivals race us for. The seats that got close (Pericles and Lady Six
//! Sky won, Gorgo reached 532 of 556) earned 3,200-6,300 Writer, Artist and
//! Musician points by t175 and held 20-38 works; the weak ones earned
//! 600-2,200 and held 1-11, and by t150 had taken 0-3 Great Writers while
//! the rivals took 15-24. Neither card was slotted on
//! any turn of any of the 30 games, though the host offered Literary
//! Tradition from Drama and Poetry (t28-46) and Frescoes from Humanism
//! (t73-115): every wildcard went to Urban Planning, Liberalism,
//! Colonization, Serfdom or Rationalism. The two cards were worth a median
//! 276 and 360 points by t175 at the seats' own Art Museum counts, about a
//! fifth of the gap between the strong seats and the weak ones.
//!
//! **Why the deck never took them.** The Culture portfolio in
//! `strategic_policies` names neither card, and the ordinary deck
//! (`policy_deck::empire_reading`) prices yields, strength, influence and
//! upkeep: a Great Person point card reads exactly zero there and loses every
//! wildcard to any card worth a rounding error of Production.
//!
//! **The gene.** While the active victory target is Culture and the lane is
//! not released, both cards join the deck right behind Aesthetics (or at its
//! head), so `desired_set` keeps them once slotted. Frescoes leads once the
//! empire holds an Art Museum (four points against Literary Tradition's two),
//! Literary Tradition before that. They are wildcard-only cards, so they take
//! only a wildcard seat from a card the plan does not want, the oldest first;
//! the timed Settler and Builder cards, the contest and emergency cards and
//! every other wanted card keep theirs. The host offers each card only while
//! its Great Person class is still available, so an exhausted class drops
//! out by itself. Off, nothing changes.
//!
//! Grep the journal for `culture-deck-seats-the-great-person-cards:`.

use super::*;

impl AdvancedAi {
    /// The Great Person cards a Culture seat wants, in the order it wants
    /// them. Empty with the gene off, off the Culture lane or once it is lost.
    pub(super) fn culture_great_person_cards(
        &self,
        g: &Game,
        city_ids: &[u32],
    ) -> Vec<&'static str> {
        if !self.culture_deck_seats_the_great_person_cards
            || self.lane_lost
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
        {
            return Vec::new();
        }
        let museums = Self::art_museums(g, city_ids);
        let cards = if museums > 0 {
            vec!["frescoes", "literary_tradition"]
        } else {
            vec!["literary_tradition", "frescoes"]
        };
        think!(self.journal(), Policies, Detail,
               "Great Person cards for the Culture deck";
               "culture-deck-seats-the-great-person-cards: {} wanted; Literary Tradition is \
                +2 Great Writer points a turn, Frescoes +2 Great Artist points and +2 for each \
                of our {museums} Art Museums",
               cards.iter().map(|card| plain(card)).collect::<Vec<_>>().join(" then "));
        cards
    }

    /// Art Museums (or a civilization's replacement) standing in our cities.
    fn art_museums(g: &Game, city_ids: &[u32]) -> usize {
        city_ids
            .iter()
            .filter_map(|city| g.cities.get(city))
            .map(|city| {
                city.buildings
                    .iter()
                    .filter(|building| {
                        g.building_is_family(*building, crate::name!("art_museum"))
                            && !city.pillaged_buildings.contains(*building)
                    })
                    .count()
            })
            .sum()
    }
}

#[cfg(test)]
mod tests;
