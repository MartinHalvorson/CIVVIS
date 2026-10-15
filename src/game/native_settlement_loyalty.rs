//! A native settler-lens reading replaces nearby-city pressure in a forecast,
//! while the engine still prices governors, amenities and other local effects.

use super::{City, Game};
use crate::Pos;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NativeSettlementPressure {
    pub player: usize,
    pub turn: u32,
    pub plots: Vec<(Pos, f64)>,
}

impl Game {
    /// Loyalty for a speculative new settlement. An observed warning is the
    /// nearby-city contribution, not the full rate. Actual owned-city readbacks
    /// and normal turn processing continue to use `city_loyalty_per_turn`.
    pub fn settlement_loyalty_per_turn(&self, city: &City) -> f64 {
        let pressure = self
            .observed_settlement_loyalty
            .as_deref()
            .filter(|reading| reading.player == city.owner && reading.turn == self.turn)
            .and_then(|reading| {
                reading
                    .plots
                    .iter()
                    .find(|(position, _)| *position == city.pos)
                    .map(|(_, pressure)| *pressure)
            })
            .filter(|pressure| pressure.is_finite() && (-20.0..=20.0).contains(pressure));
        match pressure {
            Some(pressure) => {
                self.loyalty_change_for_city_with_pressure(city.owner, city.id, Some(pressure))
                    .delta
            }
            None => self.city_loyalty_per_turn(city),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GovernorState;
    use std::collections::BTreeSet;
    use std::sync::Arc;

    fn fixture() -> (Game, u32) {
        let mut game = Game::new_full(2, 30, 20, 918, 250, 0, false);
        game.clear_mirror_cities();
        let pos = game
            .map
            .tiles
            .values()
            .find(|tile| !game.rules.is_water(tile) && game.rules.is_passable(tile))
            .unwrap()
            .pos;
        let city = game.found_city_for(0, pos, None);
        game.observed_settlement_loyalty = Some(Arc::new(NativeSettlementPressure {
            player: 0,
            turn: game.turn,
            plots: vec![(pos, -20.0)],
        }));
        (game, city)
    }

    #[test]
    fn native_pressure_replaces_only_the_population_contribution() {
        let (game, city) = fixture();
        let ordinary = game.city_loyalty_per_turn(&game.cities[&city]);
        assert_eq!(
            game.settlement_loyalty_per_turn(&game.cities[&city]),
            ordinary - 40.0
        );
        assert_eq!(game.city_loyalty_per_turn(&game.cities[&city]), ordinary);
    }

    #[test]
    fn a_governor_still_adds_its_loyalty_before_establishment() {
        let (mut game, city) = fixture();
        let before = game.settlement_loyalty_per_turn(&game.cities[&city]);
        game.players[0].governor_roster.insert(
            "victor".into(),
            GovernorState {
                city: Some(city),
                assigned_turn: game.turn,
                disabled_until: 0,
                promotions: BTreeSet::new(),
            },
        );
        game.sync_governor_cities(0);
        assert_eq!(
            game.settlement_loyalty_per_turn(&game.cities[&city]),
            before + 8.0
        );
    }

    #[test]
    fn full_loyalty_protection_still_overrides_a_native_warning() {
        let (mut game, city) = fixture();
        let pos = game.cities[&city].pos;
        game.cities
            .get_mut(&city)
            .unwrap()
            .wonders
            .insert(crate::name!("statue_of_liberty"), pos);
        assert_eq!(game.settlement_loyalty_per_turn(&game.cities[&city]), 0.0);
    }

    #[test]
    fn stale_other_player_invalid_and_missing_readings_keep_the_legacy_rate() {
        let (mut game, city) = fixture();
        let reading = game.observed_settlement_loyalty.as_deref().unwrap().clone();
        let ordinary = game.city_loyalty_per_turn(&game.cities[&city]);
        for bad in [f64::NAN, f64::INFINITY, -21.0, 21.0] {
            let mut changed = reading.clone();
            changed.plots[0].1 = bad;
            game.observed_settlement_loyalty = Some(Arc::new(changed));
            assert_eq!(
                game.settlement_loyalty_per_turn(&game.cities[&city]),
                ordinary
            );
        }
        for changed in [
            NativeSettlementPressure {
                player: 1,
                ..reading.clone()
            },
            NativeSettlementPressure {
                turn: reading.turn + 1,
                ..reading.clone()
            },
            NativeSettlementPressure {
                plots: vec![],
                ..reading
            },
        ] {
            game.observed_settlement_loyalty = Some(Arc::new(changed));
            assert_eq!(
                game.settlement_loyalty_per_turn(&game.cities[&city]),
                ordinary
            );
        }
        game.observed_settlement_loyalty = None;
        assert_eq!(
            game.settlement_loyalty_per_turn(&game.cities[&city]),
            ordinary
        );
    }

    #[test]
    fn saved_and_speculative_boards_keep_the_reading_but_other_views_do_not() {
        let (game, city) = fixture();
        let clone = game.speculative_clone();
        assert_eq!(
            clone.observed_settlement_loyalty,
            game.observed_settlement_loyalty
        );
        let saved = serde_json::to_string(&game).unwrap();
        let restored: Game = serde_json::from_str(&saved).unwrap();
        assert_eq!(
            restored.observed_settlement_loyalty,
            game.observed_settlement_loyalty
        );
        assert_eq!(
            restored.settlement_loyalty_per_turn(&restored.cities[&city]),
            game.settlement_loyalty_per_turn(&game.cities[&city])
        );
        assert!(game
            .player_decision_view(0)
            .observed_settlement_loyalty
            .is_some());
        assert!(game
            .player_decision_view(1)
            .observed_settlement_loyalty
            .is_none());
    }

    #[test]
    fn owned_city_native_total_is_not_replaced_by_a_settlement_forecast() {
        let (mut game, city) = fixture();
        Arc::make_mut(&mut game.observed_city_loyalty_per_turn).insert(city, -7.5);
        assert_eq!(game.city_loyalty_per_turn(&game.cities[&city]), -7.5);
    }
}
