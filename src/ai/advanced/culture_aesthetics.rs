//! `culture-keeps-its-aesthetics`: a seat aiming at a Culture victory keeps
//! Aesthetics (+100% Theater Square adjacency) at the head of its policy deck
//! once at least three of its cities hold a Theater Square or Acropolis.
//!
//! **Why (live Deity Pericles, 2026-11-08, 21 games).** The Culture deck list
//! does not name Aesthetics, so the card only reached a slot through the
//! general score. 11 of 21 seats held it at t100 and 5 of 21 at t130; the
//! Science multipliers took the Economic slots instead (Natural Philosophy
//! 20 of 21 at t100, Rationalism 16 of 21 at t130), and the Liberalism repair
//! demotes Aesthetics by name. An Acropolis earns +1 Culture per adjacent
//! district and +1 from the City Center, and Culture is what the seat is
//! short of: Cold War, which opens Rock Bands, landed at a median of t153.
//!
//! **What the gene does.** Aesthetics goes first in the deck and Natural
//! Philosophy, the same-slot Campus adjacency card, leaves the wanted list so
//! that Aesthetics may take its slot. It reads after the Liberalism repair.

use super::*;

/// Cities holding a Theater Square before the card pays for an Economic slot.
const AESTHETICS_MIN_THEATER_CITIES: usize = 3;

impl AdvancedAi {
    pub(super) fn culture_aesthetics_leads(&self, g: &Game, city_ids: &[u32]) -> bool {
        self.culture_keeps_its_aesthetics
            && !self.lane_lost
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
            && city_ids
                .iter()
                .filter(|city| {
                    g.city_has_district_family(&g.cities[*city], crate::name!("theater_square"))
                })
                .count()
                >= AESTHETICS_MIN_THEATER_CITIES
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Action;

    /// A board where our cities hold `theaters` Theater Squares.
    fn board(theaters: usize) -> (Game, Vec<u32>) {
        let mut g = Game::new_full(2, 30, 20, 79_208, 250, 0, false);
        g.current = 0;
        let unit = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit }).unwrap();
        let first = g.player_city_ids(0)[0];
        let pos = g.cities[&first].pos;
        for extra in 1..3u32 {
            let mut city = g.cities[&first].clone();
            city.id = first + 1_000 * extra;
            g.cities.insert(city.id, city);
        }
        let ids: Vec<u32> = vec![first, first + 1_000, first + 2_000];
        for id in ids.iter().take(theaters) {
            g.cities
                .get_mut(id)
                .unwrap()
                .districts
                .insert(crate::name!("theater_square"), pos);
        }
        (g, ids)
    }

    fn seat() -> AdvancedAi {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
        ai.enable_culture_keeps_its_aesthetics();
        ai
    }

    #[test]
    fn three_theater_cities_put_aesthetics_first() {
        let (g, ids) = board(3);
        assert!(seat().culture_aesthetics_leads(&g, &ids));
    }

    #[test]
    fn two_theater_cities_do_not_pay_for_the_slot() {
        let (g, ids) = board(2);
        assert!(!seat().culture_aesthetics_leads(&g, &ids));
    }

    #[test]
    fn a_released_lane_lets_the_card_go() {
        let (g, ids) = board(3);
        let mut ai = seat();
        ai.lane_lost = true;
        assert!(!ai.culture_aesthetics_leads(&g, &ids));
    }

    #[test]
    fn off_is_inert() {
        let (g, ids) = board(3);
        let ai = AdvancedAi::targeting(VictoryTarget::Culture);
        assert!(!ai.culture_aesthetics_leads(&g, &ids));
    }
}
