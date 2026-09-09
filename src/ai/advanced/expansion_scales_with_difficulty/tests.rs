use super::*;
use crate::setup::GameSpeed;

/// A 250-turn Online board, the shape the opening corpus was measured on.
fn board() -> Game {
    let mut g = Game::new(2, 24, 16, 71, 250, 0);
    g.game_speed = GameSpeed::Online;
    g
}

/// The same board at the named rung.
fn board_at(rung: &str) -> Game {
    let mut g = board();
    g.difficulty = rung.to_string();
    g
}

fn wide() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_expansion_scales_with_difficulty();
    ai
}

#[test]
fn the_gene_is_registered_opt_in_and_ships_off_with_twin_toggles() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "expansion-scales-with-difficulty",
        |ai| ai.expansion_scales_with_difficulty,
    );
    let mut ai = AdvancedAi::new();
    assert!(!ai.expansion_scales_with_difficulty, "an opt-in ships off");
    assert!(!AdvancedAi::legacy().expansion_scales_with_difficulty);
    ai.enable_expansion_scales_with_difficulty();
    assert!(ai.expansion_scales_with_difficulty);
    ai.disable_expansion_scales_with_difficulty();
    assert!(!ai.expansion_scales_with_difficulty);
}

#[test]
fn the_level_is_the_distance_above_prince_and_the_target_is_the_band_plus_it() {
    // `data/difficulties.json` orders Settler 0 through Deity 7, Prince 3.
    let seen: Vec<(usize, usize, usize)> = [
        "settler",
        "chieftain",
        "warlord",
        "prince",
        "king",
        "emperor",
        "immortal",
        "deity",
    ]
    .into_iter()
    .map(|rung| {
        let level = level_above_prince(&board_at(rung));
        (level, city_target(level), cities_by_hundred(level))
    })
    .collect();
    assert_eq!(
        seen,
        vec![
            (0, 5, 7),  // Settler   — below Prince, the measured band exactly
            (0, 5, 7),  // Chieftain
            (0, 5, 7),  // Warlord
            (0, 5, 7),  // Prince    — the unhandicapped reference rung
            (1, 6, 8),  // King      — the rung the 4-6 corpus was measured on
            (2, 7, 9),  // Emperor   — rivals at +16% yields and a free Settler
            (3, 8, 10), // Immortal
            (4, 9, 10), // Deity    — WIDE_CAP holds the second target at ten
        ],
        "{seen:?}"
    );
    assert_eq!(city_target(0), OPENING_BAND_CITIES, "at Prince, the band");
    assert_eq!(city_target(99), WIDE_CAP, "the cap is a hard ceiling");
    assert_eq!(cities_by_hundred(99), WIDE_CAP);
}

#[test]
fn the_deadline_extends_by_ten_speed_scaled_turns_per_rung() {
    for (rung, level) in [("prince", 0), ("king", 1), ("emperor", 2), ("deity", 4)] {
        let g = board_at(rung);
        let band = AdvancedAi::expansion_band_turn(&g);
        assert_eq!(band, 60, "the ladder's band turn");
        let expected = band + g.standard_duration(WIDE_DEADLINE_TURNS_PER_LEVEL * level as u32);
        assert_eq!(
            wide().expansion_deadline_turn(&g),
            expected,
            "{rung} extends by {level} levels of speed-scaled turns"
        );
        assert_eq!(
            AdvancedAi::new().expansion_deadline_turn(&g),
            band,
            "{rung} off is exactly the band turn"
        );
    }
    // Standard speed is the unscaled column, so the constant reads directly:
    // Emperor's two rungs are twenty standard turns past its band turn.
    let mut g = board_at("emperor");
    g.game_speed = GameSpeed::Standard;
    let band = AdvancedAi::expansion_band_turn(&g);
    assert_eq!(wide().expansion_deadline_turn(&g), band + 20);
}

#[test]
fn the_pace_ramps_to_the_rung_target_then_on_to_the_second_target() {
    let mut g = board_at("emperor");
    let ai = wide();
    let band = AdvancedAi::expansion_band_turn(&g);
    let deadline = ai.expansion_deadline_turn(&g);
    let horizon = ai.expansion_cadence_horizon(&g);
    assert!(
        band < deadline && deadline < horizon,
        "two ordered horizons"
    );
    assert_eq!(horizon, 100, "turn 100 of the ladder's 250-turn clock");

    g.turn = 0;
    assert_eq!(ai.expansion_pace_now(&g), 1, "one city at the start");
    g.turn = deadline;
    assert_eq!(ai.expansion_pace_now(&g), 7, "Emperor's opening target");
    g.turn = horizon;
    assert_eq!(ai.expansion_pace_now(&g), 9, "and its second target");
    g.turn = horizon + 40;
    assert_eq!(ai.expansion_pace_now(&g), 9, "flat past the horizon");

    // Monotone the whole way: a pace that dipped would ask the empire to
    // un-found a city.
    let mut previous = 0;
    for turn in 0..=(horizon + 5) {
        g.turn = turn;
        let now = ai.expansion_pace_now(&g);
        assert!(now >= previous, "pace fell at turn {turn}");
        previous = now;
    }
}

#[test]
fn off_every_quantity_is_the_shipped_one_at_every_rung_and_turn() {
    let off = AdvancedAi::new();
    for rung in ["prince", "king", "emperor", "immortal", "deity"] {
        let mut g = board_at(rung);
        for turn in [0u32, 20, 59, 60, 61, 100, 180] {
            g.turn = turn;
            assert_eq!(
                off.expansion_deadline_turn(&g),
                AdvancedAi::expansion_band_turn(&g)
            );
            assert_eq!(
                off.expansion_cadence_horizon(&g),
                AdvancedAi::expansion_band_turn(&g)
            );
            assert_eq!(off.expansion_pace_now(&g), AdvancedAi::expansion_pace(&g));
            assert_eq!(off.expansion_wide_level(&g), None);
            assert_eq!(off.expansion_wide_city_target(&g), None);
            // The schedule's own gene is off too, so the pipeline is silent.
            assert_eq!(off.expansion_pace_shortfall(&g, 1, 0), 0);
            assert_eq!(off.expansion_schedule_pipeline(&g, 9, 1, 0), None);
            assert_eq!(off.expansion_wide_pipeline(&g, 9, 1, 0), None);
        }
    }
}

#[test]
fn the_schedule_gene_alone_keeps_its_exact_shipped_schedule_at_every_rung() {
    let mut schedule = AdvancedAi::new();
    schedule.enable_expansion_schedule();
    let mut g = board_at("deity");
    // The rung must not reach `expansion-schedule`: it is a separate gene with
    // its own measured band, and this gene is off.
    g.turn = 40;
    assert_eq!(schedule.expansion_pace_shortfall(&g, 1, 0), 2);
    assert_eq!(schedule.expansion_schedule_pipeline(&g, 9, 1, 0), Some(3));
    assert_eq!(schedule.expansion_wide_pipeline(&g, 9, 1, 0), None);
    g.turn = 61;
    assert_eq!(
        schedule.expansion_pace_shortfall(&g, 1, 0),
        0,
        "past the band"
    );
}

#[test]
fn the_wide_pipeline_counts_founded_cities_and_stops_at_two_slots() {
    let ai = wide();
    let mut g = board_at("emperor");
    g.turn = 40;
    // Two cities and a walker: the schedule's shortfall counts the walker and
    // closes, while the rung's cadence reads the two cities actually founded
    // and keeps a second slot open.
    let mut schedule = AdvancedAi::new();
    schedule.enable_expansion_schedule();
    assert_eq!(schedule.expansion_schedule_pipeline(&g, 9, 2, 1), None);
    assert_eq!(ai.expansion_wide_pipeline(&g, 9, 2, 1), Some(2));
    assert_eq!(
        ai.expansion_wide_pipeline(&g, 9, 2, 0),
        Some(WIDE_PARALLEL_SETTLERS),
        "never more than the two slots"
    );
    // On pace, at the target, and past the horizon: nothing asked for.
    assert_eq!(ai.expansion_wide_pipeline(&g, 9, 9, 0), None, "at target");
    assert_eq!(
        ai.expansion_wide_pipeline(&g, 3, 2, 0),
        Some(1),
        "the city target stays the hard cap"
    );
    g.turn = 101;
    assert_eq!(
        ai.expansion_wide_pipeline(&g, 9, 1, 0),
        None,
        "past horizon"
    );
}

#[test]
fn the_wide_pipeline_stays_open_past_the_shipped_band() {
    let ai = wide();
    let mut g = board_at("emperor");
    g.turn = 61;
    // The shipped schedule is inert past turn 60; the rung's cadence is not.
    let mut schedule = AdvancedAi::new();
    schedule.enable_expansion_schedule();
    assert_eq!(schedule.expansion_schedule_pipeline(&g, 9, 1, 0), None);
    assert_eq!(ai.expansion_wide_pipeline(&g, 9, 1, 0), Some(2));
}

#[test]
fn the_target_horizon_never_lowers_an_empire_that_is_already_wider() {
    let g = board_at("emperor");
    let ai = wide();
    assert_eq!(ai.expansion_wide_city_target(&g), Some(9));
    // The hook in `assess` takes the maximum, so a land-grab target of twelve
    // is untouched; every land-aware cap below it still runs afterwards.
    assert_eq!(12usize.max(ai.expansion_wide_city_target(&g).unwrap()), 12);
}

#[test]
fn the_science_contract_is_raised_to_the_rung_not_applied_over_it() {
    use super::super::{VictoryTarget, SCIENCE_CITY_TARGET_CAP};
    use crate::game::{GameOptions, VictoryConditions};
    // The board the shipped Science-contract test plans on, at Emperor.
    let mut g = Game::new_with(GameOptions {
        speed: "online".to_string(),
        ..GameOptions::new(2, 74, 46, 91_508, 250, 0)
    });
    g.victory_conditions = VictoryConditions::parse("science,score").unwrap();
    g.current = 0;
    g.difficulty = "emperor".to_string();
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.found_city_for(0, g.units[&settler].pos, None);
    g.remove_unit(settler);
    // Half the clock: the native contract's `min(SCIENCE_CITY_TARGET_CAP)`
    // is active from here on a Science seat.
    g.turn = g.max_turns / 2;

    let mut ai = AdvancedAi::new();
    ai.victory_target = Some(VictoryTarget::Science);
    let off = ai.assess(&g, 0).desired_cities;
    assert!(
        off <= SCIENCE_CITY_TARGET_CAP,
        "off, the shipped contract caps the seat at {SCIENCE_CITY_TARGET_CAP}: {off}"
    );

    ai.enable_expansion_scales_with_difficulty();
    let on = ai.assess(&g, 0).desired_cities;
    let rung = ai.expansion_wide_city_target(&g).unwrap();
    assert_eq!(rung, 9, "Emperor's second target");
    assert_eq!(
        on, rung,
        "on, the contract is raised to the rung's horizon rather than clamping it back to {SCIENCE_CITY_TARGET_CAP}"
    );
}
