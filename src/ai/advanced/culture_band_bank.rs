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
//! last few turns, and it does not touch the army's Faith. Naturalists wait
//! too: live 20261109T041324Z banked 404 Faith by t135, then bought one for
//! about 390 at t136, three turns before Cold War, and its first band came at
//! t142. A band returned about 14,000 tourism there for 150-200 Faith.
//! Once Cold War is known the ordinary band purchase spends the bank.
//!
//! ## `culture-bank-opens-at-nationalism`
//!
//! **Why (live Deity Kristina, 2026-11-10).** civvis-20261110T140731Z knew
//! Nationalism at t115 but never reached Ideology, so the bank above never
//! opened, and 1,370 Faith went on three Great People: an Artist (370 Faith,
//! t124) that then idled 27 turns, a Writer (450, t132) and a Scientist (550,
//! t144, "worth 8 to the recovery plan"). That is about the first four Online
//! Rock Bands, and a band returned 5-11 visitors in the Pericles games.
//! Nationalism is on the road to Cold War, so it is reached in every game that
//! reaches the bands, typically 30-40 turns before them.
//!
//! **What the gene does.** The same bank opens once Nationalism is known, and
//! from then on Faith buys no Great Person but a Writer, an Artist or a
//! Musician (the slot check in `advanced_great_people` still applies), and
//! none at all while the plan is Recovery. The under-fire release and the
//! army's own Faith are unchanged. Gold buyouts are unchanged.
//!
//! ## `band-bank-bars-people-under-fire`
//!
//! **Why (live Deity Pericles 20261110T232642Z, the strongest culture economy
//! we have had).** The bank stands aside for every pass while a city has
//! been attacked in the last four turns. At t122 Mycenae was under fire; the
//! seat held 2,111 Faith with Cold War one turn away, and at t123 the Great
//! Person pass bought out the Writer race for 1,990 Faith ("worth 35 to the
//! culture plan": its reserve read 100, not 1,600). Cold War landed at t124
//! with 62 Faith, so the first band came at t124 and the fourth at t134
//! instead of four on the unlock turn. 1,990 Faith is the first five Online
//! bands (300 + 350 + 400 + 450 + 500), and a band bought on the unlock turn
//! has the longest career; a Writer's two Great Works return a few Tourism a
//! turn. The release exists so the defence can spend; the Great Person pass
//! buys no defence.
//!
//! **What the gene does.** While a city is under fire, the Great Person pass
//! still leaves the bank (`culture_band_bank_for_people`). Every other pass
//! and the army's Faith keep the release; Cold War spends the bank on bands
//! as before.

use super::*;

/// The first four Rock Bands at Standard speed (600 + 700 + 800 + 900): 1,500
/// at Online, matching the host's quoted purchase costs.
const BAND_BANK_STANDARD: f64 = 3_000.0;

/// Turns after an attack on a city during which the bank stands aside.
const BAND_BANK_UNDER_FIRE_TURNS: u32 = 4;

impl AdvancedAi {
    /// Whether the bank is open, so a Naturalist waits for the bands.
    pub(super) fn culture_band_bank_holds_naturalists(&self, g: &Game, pid: usize) -> bool {
        self.culture_band_bank(g, pid) > 0.0
    }

    /// The Faith the Great Person, worship-building and religious-unit passes
    /// must leave untouched. Zero with the gene off.
    pub(super) fn culture_band_bank(&self, g: &Game, pid: usize) -> f64 {
        self.culture_band_bank_held(g, pid, false)
    }

    /// `band-bank-bars-people-under-fire`: the bank the Great Person pass
    /// leaves untouched. With the gene on, a city under fire does not open it:
    /// that release exists for the defence, and the Great Person pass buys
    /// none. Equal to [`culture_band_bank`](Self::culture_band_bank) off.
    pub(super) fn culture_band_bank_for_people(&self, g: &Game, pid: usize) -> f64 {
        self.culture_band_bank_held(g, pid, self.band_bank_bars_people_under_fire)
    }

    fn culture_band_bank_held(&self, g: &Game, pid: usize, through_fire: bool) -> f64 {
        if !(self.culture_banks_for_the_bands || self.culture_bank_opens_at_nationalism)
            || self.lane_lost
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
        {
            return 0.0;
        }
        let player = &g.players[pid];
        if player.civics.contains(&crate::name!("cold_war")) {
            return 0.0;
        }
        let nearing = (self.culture_banks_for_the_bands
            && (player.civics.contains(&crate::name!("ideology"))
                || matches!(player.civic.as_deref(), Some("ideology" | "cold_war"))))
            || (self.culture_bank_opens_at_nationalism
                && player.civics.contains(&crate::name!("nationalism")));
        let under_fire = g.player_city_ids(pid).iter().any(|cid| {
            let city = &g.cities[cid];
            city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= BAND_BANK_UNDER_FIRE_TURNS
        });
        if !nearing || (under_fire && !through_fire) {
            return 0.0;
        }
        g.game_speed.scale(BAND_BANK_STANDARD)
    }

    /// `culture-bank-opens-at-nationalism`: whether Faith may not buy out the
    /// `kind` race. From Nationalism a live Culture target spends Faith on
    /// Writers, Artists and Musicians only, and on none while `strategy` is
    /// Recovery. `false` with the gene off.
    pub(super) fn culture_bank_bars_faith_great_person(
        &self,
        g: &Game,
        pid: usize,
        strategy: GrandStrategy,
        kind: &str,
    ) -> bool {
        self.culture_bank_opens_at_nationalism
            && !self.lane_lost
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
            && g.players[pid].civics.contains(&crate::name!("nationalism"))
            && (strategy == GrandStrategy::Recovery
                || !matches!(kind, "writer" | "artist" | "musician"))
    }

    /// `culture-bank-holds-the-counterweight`: whether a Culture seat's home
    /// religious defence (`religious_defense`) leaves its Faith for the Rock
    /// Bands instead of buying a counterweight Missionary against `threat`.
    /// From Nationalism on (the band bank's window, then the bands
    /// themselves), unless the faith's founder stands at the religious match
    /// point (`faith_at_match_point`), when the veto is worth the Faith.
    ///
    /// The counterweight buys ignore the band bank: live Deity Online Kristina
    /// 2026-11-10 bought 21 Missionaries (about 85 Faith each) from
    /// Nationalism on in 5 of 8 games (2, 3, 4, 5 and 7, three of the seven
    /// after Cold War), while 171451Z reached Cold War at t143 with 156 Faith
    /// and bought its first 300-Faith band at t161. One of 294 Pericles
    /// and Kristina ladder games from 11-06 to 11-10 ended in a rival's
    /// Religious victory (179 to a rival's Science). False with the gene off
    /// or for another target.
    pub(super) fn culture_bank_holds_counterweight(
        &self,
        g: &Game,
        pid: usize,
        threat: &str,
    ) -> bool {
        if !self.culture_bank_holds_the_counterweight
            || self.lane_lost
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
            || !g.players[pid].civics.contains(&crate::name!("nationalism"))
        {
            return false;
        }
        let at_match_point = g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && player.religion.as_deref() == Some(threat)
                && self.faith_at_match_point(g, player.id)
        });
        if at_match_point {
            return false;
        }
        think!(self.journal(), Faith, Detail,
            "Holding the counterweight Missionary against {}", threat;
            "culture-bank-holds-the-counterweight: Nationalism is known and {} is short of match point; the Faith stays for the Rock Bands",
            threat);
        true
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
    fn naturalists_wait_only_while_the_bank_is_open() {
        let mut g = board();
        assert!(!seat().culture_band_bank_holds_naturalists(&g, 0));
        known(&mut g, "ideology");
        assert!(seat().culture_band_bank_holds_naturalists(&g, 0));
        known(&mut g, "cold_war");
        assert!(!seat().culture_band_bank_holds_naturalists(&g, 0));
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
    fn under_fire_the_bank_stays_shut_to_great_people_only_with_the_gene() {
        let mut g = board();
        known(&mut g, "ideology");
        let city = g.player_city_ids(0)[0];
        g.cities.get_mut(&city).unwrap().last_attacked = g.turn - 1;
        let mut ai = seat();
        assert_eq!(ai.culture_band_bank_for_people(&g, 0), 0.0);
        ai.enable_band_bank_bars_people_under_fire();
        let held = g.game_speed.scale(BAND_BANK_STANDARD);
        assert_eq!(ai.culture_band_bank_for_people(&g, 0), held);
        // The defence's release is unchanged.
        assert_eq!(ai.culture_band_bank(&g, 0), 0.0);
        // Cold War spends the bank on the bands as before.
        known(&mut g, "cold_war");
        assert_eq!(ai.culture_band_bank_for_people(&g, 0), 0.0);
    }

    #[test]
    fn only_a_culture_target_banks() {
        let mut g = board();
        known(&mut g, "ideology");
        let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
        ai.enable_culture_banks_for_the_bands();
        assert_eq!(ai.culture_band_bank(&g, 0), 0.0);
    }

    fn nationalism_seat() -> AdvancedAi {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
        ai.enable_culture_bank_opens_at_nationalism();
        ai
    }

    #[test]
    fn nationalism_opens_the_bank_only_with_the_gene() {
        let mut g = board();
        known(&mut g, "nationalism");
        assert_eq!(seat().culture_band_bank(&g, 0), 0.0);
        assert_eq!(
            nationalism_seat().culture_band_bank(&g, 0),
            g.game_speed.scale(BAND_BANK_STANDARD)
        );
        assert!(nationalism_seat().culture_band_bank_holds_naturalists(&g, 0));
        known(&mut g, "cold_war");
        assert_eq!(nationalism_seat().culture_band_bank(&g, 0), 0.0);
    }

    #[test]
    fn the_nationalism_bank_waits_for_nationalism_and_stands_aside_under_fire() {
        let mut g = board();
        assert_eq!(nationalism_seat().culture_band_bank(&g, 0), 0.0);
        known(&mut g, "nationalism");
        let city = g.player_city_ids(0)[0];
        g.cities.get_mut(&city).unwrap().last_attacked = g.turn - 1;
        assert_eq!(nationalism_seat().culture_band_bank(&g, 0), 0.0);
        let mut science = AdvancedAi::targeting(VictoryTarget::Science);
        science.enable_culture_bank_opens_at_nationalism();
        g.cities.get_mut(&city).unwrap().last_attacked = 0;
        assert_eq!(science.culture_band_bank(&g, 0), 0.0);
    }

    #[test]
    fn from_nationalism_faith_buys_only_the_lane_creators() {
        let mut g = board();
        let ai = nationalism_seat();
        for kind in ["scientist", "engineer", "merchant", "artist"] {
            assert!(!ai.culture_bank_bars_faith_great_person(&g, 0, GrandStrategy::Culture, kind));
        }
        known(&mut g, "nationalism");
        for kind in ["scientist", "engineer", "merchant", "general", "prophet"] {
            assert!(ai.culture_bank_bars_faith_great_person(&g, 0, GrandStrategy::Culture, kind));
        }
        for kind in ["writer", "artist", "musician"] {
            assert!(!ai.culture_bank_bars_faith_great_person(&g, 0, GrandStrategy::Culture, kind));
            assert!(ai.culture_bank_bars_faith_great_person(&g, 0, GrandStrategy::Recovery, kind));
        }
        // Off, a released lane, or another target bars nothing.
        let off = AdvancedAi::targeting(VictoryTarget::Culture);
        assert!(!off.culture_bank_bars_faith_great_person(
            &g,
            0,
            GrandStrategy::Recovery,
            "scientist"
        ));
        let mut released = nationalism_seat();
        released.lane_lost = true;
        assert!(!released.culture_bank_bars_faith_great_person(
            &g,
            0,
            GrandStrategy::Recovery,
            "scientist"
        ));
        let mut science = AdvancedAi::targeting(VictoryTarget::Science);
        science.enable_culture_bank_opens_at_nationalism();
        assert!(!science.culture_bank_bars_faith_great_person(
            &g,
            0,
            GrandStrategy::Science,
            "scientist"
        ));
    }

    #[test]
    fn a_released_lane_spends_freely() {
        let mut g = board();
        known(&mut g, "ideology");
        let mut ai = seat();
        ai.lane_lost = true;
        assert_eq!(ai.culture_band_bank(&g, 0), 0.0);
    }

    fn counterweight_seat() -> AdvancedAi {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
        ai.enable_culture_bank_holds_the_counterweight();
        ai
    }

    #[test]
    fn counterweight_hold_opens_at_nationalism_on_a_culture_seat_only() {
        let mut g = board();
        g.players[1].religion = Some("buddhism".into());
        let ai = counterweight_seat();
        assert!(
            !ai.culture_bank_holds_counterweight(&g, 0, "buddhism"),
            "before Nationalism the shipped defence buys"
        );
        known(&mut g, "nationalism");
        assert_eq!(
            ai.culture_bank_holds_counterweight(&g, 0, "buddhism"),
            !ai.faith_at_match_point(&g, 1)
        );
        assert!(
            !ai.faith_at_match_point(&g, 1),
            "the fixture is short of match point"
        );
        assert!(ai.culture_bank_holds_counterweight(&g, 0, "buddhism"));
        known(&mut g, "cold_war");
        assert!(
            ai.culture_bank_holds_counterweight(&g, 0, "buddhism"),
            "the Rock Bands keep the Faith after Cold War too"
        );

        let off = AdvancedAi::targeting(VictoryTarget::Culture);
        assert!(!off.culture_bank_holds_counterweight(&g, 0, "buddhism"));
        let mut science = AdvancedAi::targeting(VictoryTarget::Science);
        science.enable_culture_bank_holds_the_counterweight();
        assert!(!science.culture_bank_holds_counterweight(&g, 0, "buddhism"));
        let mut lost = counterweight_seat();
        lost.lane_lost = true;
        assert!(!lost.culture_bank_holds_counterweight(&g, 0, "buddhism"));
    }
}
