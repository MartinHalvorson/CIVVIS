//! `culture-banks-for-the-bands`: from Ideology until Cold War, a seat aiming
//! at a Culture victory holds enough Faith back to buy its first four Rock
//! Bands on the turn they unlock.
//!
//! **Why (live Deity Pericles, 2026-11-08, 27 games).** Rock Bands are the
//! largest source of visitors, they open at Cold War, and they compound with
//! every concert a band survives. In the 25 turns before Cold War the seat
//! earned a median of 1,204 Faith and spent 1,094 of it, on Missionaries,
//! Walls, worship buildings and Great People, and reached the unlock with 241
//! banked. Native Pericles 20261109T011348Z-cont2 quoted its first four
//! Online bands at 300, 350, 400 and 450 Faith: 1,500 in total. The former
//! 900-Faith bank covered two of those bands, rather than four.
//!
//! **What the gene does.** While a live Culture target has researched Ideology
//! or is researching Ideology or Cold War, and Cold War is not yet known, the
//! Great Person, worship-building and religious-unit passes keep this bank on
//! top of their own reserves. It stands aside if a city was attacked in the
//! last few turns, and it does not touch Naturalists or the army's Faith.
//! Once Cold War is known the ordinary band purchase spends it.

use super::*;

/// The first four Rock Bands at Standard speed (600 + 700 + 800 + 900): 1,500
/// at Online, matching the host's quoted purchase costs.
const BAND_BANK_STANDARD: f64 = 3_000.0;

/// Turns after an attack on a city during which the bank stands aside.
const BAND_BANK_UNDER_FIRE_TURNS: u32 = 4;

impl AdvancedAi {
    /// The Faith the Great Person, worship-building and religious-unit passes
    /// must leave untouched. Zero with the gene off.
    pub(super) fn culture_band_bank(&self, g: &Game, pid: usize) -> f64 {
        if !self.culture_banks_for_the_bands
            || self.lane_lost
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
        {
            return 0.0;
        }
        let player = &g.players[pid];
        if player.civics.contains(&crate::name!("cold_war")) {
            return 0.0;
        }
        let nearing = player.civics.contains(&crate::name!("ideology"))
            || matches!(player.civic.as_deref(), Some("ideology" | "cold_war"));
        let under_fire = g.player_city_ids(pid).iter().any(|cid| {
            let city = &g.cities[cid];
            city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= BAND_BANK_UNDER_FIRE_TURNS
        });
        if !nearing || under_fire {
            return 0.0;
        }
        g.game_speed.scale(BAND_BANK_STANDARD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Action;
    use crate::setup::GameSpeed;

    fn board() -> Game {
        let mut g = Game::new_full(2, 30, 20, 79_208, 250, 0, false);
        g.current = 0;
        let unit = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit }).unwrap();
        g.turn = 120;
        g.players[0].civic = None;
        g
    }

    fn seat() -> AdvancedAi {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
        ai.enable_culture_banks_for_the_bands();
        ai
    }

    fn known(g: &mut Game, civic: &str) {
        g.players[0].civics.insert(Name::new(civic));
    }

    #[test]
    fn off_holds_nothing() {
        let mut g = board();
        known(&mut g, "ideology");
        let ai = AdvancedAi::targeting(VictoryTarget::Culture);
        assert_eq!(ai.culture_band_bank(&g, 0), 0.0);
    }

    #[test]
    fn no_bank_before_the_ideology_window() {
        let g = board();
        assert_eq!(seat().culture_band_bank(&g, 0), 0.0);
    }

    #[test]
    fn researching_ideology_opens_the_bank() {
        let mut g = board();
        g.players[0].civic = Some("ideology".into());
        assert_eq!(
            seat().culture_band_bank(&g, 0),
            g.game_speed.scale(BAND_BANK_STANDARD)
        );
    }

    #[test]
    fn knowing_ideology_holds_the_bank() {
        let mut g = board();
        known(&mut g, "ideology");
        assert!(seat().culture_band_bank(&g, 0) > 0.0);
    }

    #[test]
    fn the_bank_covers_four_actual_band_purchases_at_each_speed() {
        for (speed, expected) in [
            (GameSpeed::Online, [300.0, 350.0, 400.0, 450.0]),
            (GameSpeed::Standard, [600.0, 700.0, 800.0, 900.0]),
        ] {
            let mut g = board();
            g.game_speed = speed;
            g.players[0].civ = "Greece".into();
            known(&mut g, "ideology");
            let bank = seat().culture_band_bank(&g, 0);

            // Price by executing purchases, including the per-copy increase,
            // on a throwaway board where the unit has just unlocked.
            let mut unlocked = g.clone();
            known(&mut unlocked, "cold_war");
            unlocked.players[0].faith = 10_000.0;
            let city = unlocked.player_city_ids(0)[0];
            let mut spent = 0.0;
            for price in expected {
                let before = unlocked.players[0].faith;
                unlocked
                    .apply(
                        0,
                        &Action::Buy {
                            city,
                            unit: crate::name!("rock_band"),
                            formation: 0,
                            currency: "faith".into(),
                        },
                    )
                    .unwrap();
                assert_eq!(before - unlocked.players[0].faith, price);
                spent += price;
                // Each next purchase needs its civilian tile free; its price
                // still counts every earlier purchase after the unit leaves.
                let band = unlocked
                    .player_unit_ids(0)
                    .into_iter()
                    .find(|uid| unlocked.units[uid].kind == "rock_band")
                    .unwrap();
                unlocked.remove_unit(band);
            }
            assert_eq!(bank, spent);
        }
    }

    #[test]
    fn cold_war_releases_the_bank() {
        let mut g = board();
        known(&mut g, "ideology");
        known(&mut g, "cold_war");
        assert_eq!(seat().culture_band_bank(&g, 0), 0.0);
    }

    #[test]
    fn a_city_under_attack_releases_the_bank() {
        let mut g = board();
        known(&mut g, "ideology");
        let city = g.player_city_ids(0)[0];
        g.cities.get_mut(&city).unwrap().last_attacked = g.turn - 1;
        assert_eq!(seat().culture_band_bank(&g, 0), 0.0);
        g.cities.get_mut(&city).unwrap().last_attacked = g.turn - 5;
        assert!(seat().culture_band_bank(&g, 0) > 0.0);
    }

    #[test]
    fn only_a_culture_target_banks() {
        let mut g = board();
        known(&mut g, "ideology");
        let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
        ai.enable_culture_banks_for_the_bands();
        assert_eq!(ai.culture_band_bank(&g, 0), 0.0);
    }

    #[test]
    fn a_released_lane_spends_freely() {
        let mut g = board();
        known(&mut g, "ideology");
        let mut ai = seat();
        ai.lane_lost = true;
        assert_eq!(ai.culture_band_bank(&g, 0), 0.0);
    }
}
