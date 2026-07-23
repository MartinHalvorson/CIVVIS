//! Rise & Fall Ages: the Normal-Age half of every Dedication, the era each
//! Dedication can be chosen in, and the Dark/Normal/Golden/Heroic ladder.
use super::{Action, Game};

fn two_player_game() -> Game {
    Game::new_full(2, 30, 18, 515, 300, 0, false)
}

#[test]
fn every_dedication_carries_both_halves_and_an_era_span() {
    let rules = crate::rules::Rules::embedded();
    assert_eq!(
        rules.dedications.len(),
        12,
        "Rise & Fall ships twelve Dedications"
    );
    for (name, spec) in rules.dedications.iter() {
        assert!(!spec.normal.is_empty(), "{name} has no Normal-Age text");
        assert!(!spec.golden.is_empty(), "{name} has no Golden-Age text");
        assert!(
            !spec.triggers.is_empty(),
            "{name} pays no Era Score in a Normal Age"
        );
        assert!(
            spec.eras.0 >= 1 && spec.eras.1 < crate::rules::ERA_NAMES.len(),
            "{name} spans {:?}, which is not a run of real eras",
            spec.eras
        );
        assert!(spec.eras.0 <= spec.eras.1, "{name} spans backwards");
    }
}

#[test]
fn dedications_are_offered_only_in_their_own_eras() {
    let mut game = two_player_game();
    game.players[0].dedication_choices = 1;

    // Classical: the early four are on offer and the late ones are not.
    game.world_era = 1;
    let classical = game.available_dedications(0);
    assert!(classical.contains(&"monumentality".to_string()));
    assert!(classical.contains(&"exodus_of_the_evangelists".to_string()));
    assert!(!classical.contains(&"automaton_warfare".to_string()));
    assert!(!classical.contains(&"wish_you_were_here".to_string()));

    // Information: the late ones are, and the Classical-only ones are gone.
    game.world_era = 7;
    let information = game.available_dedications(0);
    assert!(information.contains(&"automaton_warfare".to_string()));
    assert!(information.contains(&"wish_you_were_here".to_string()));
    assert!(!information.contains(&"exodus_of_the_evangelists".to_string()));
    assert!(!information.contains(&"monumentality".to_string()));
}

#[test]
fn the_two_gathering_storm_dedications_exist_and_can_be_chosen() {
    let mut game = two_player_game();
    game.world_era = 7;
    game.players[0].dedication_choices = 2;
    for dedication in ["wish_you_were_here", "bodyguard_of_lies"] {
        game.apply(
            0,
            &Action::ChooseDedication {
                dedication: dedication.to_string(),
            },
        )
        .unwrap_or_else(|error| panic!("{dedication} should be choosable: {error}"));
        assert!(game.players[0].dedications.contains(dedication));
    }
}

#[test]
fn a_normal_age_dedication_still_pays_era_score() {
    let mut game = two_player_game();
    game.players[0].age = "normal".to_string();
    game.players[0]
        .dedications
        .insert("free_inquiry".to_string());
    let before = game.players[0].era_score;

    game.dedication_trigger(0, "eureka", 1);

    assert_eq!(
        game.players[0].era_score,
        before + 1,
        "Free Inquiry pays +1 Era Score per Eureka in a Normal Age"
    );
}

#[test]
fn a_dark_age_dedication_pays_the_same_score_but_not_the_golden_bonus() {
    let mut game = two_player_game();
    game.players[0].age = "dark".to_string();
    game.players[0]
        .dedications
        .insert("monumentality".to_string());
    let before = game.players[0].era_score;

    game.dedication_trigger(0, "specialty_district", 1);

    assert_eq!(
        game.players[0].era_score,
        before + 1,
        "a Dark Age Dedication is how a civilization climbs out of it"
    );
    assert!(
        !game.dedication_active(0, "monumentality"),
        "but the Golden-Age half stays off"
    );

    game.players[0].age = "golden".to_string();
    assert!(game.dedication_active(0, "monumentality"));
}

#[test]
fn a_dedication_pays_only_for_its_own_trigger() {
    let mut game = two_player_game();
    game.players[0].age = "normal".to_string();
    game.players[0].dedications.insert("to_arms".to_string());
    let before = game.players[0].era_score;

    game.dedication_trigger(0, "eureka", 3);
    assert_eq!(game.players[0].era_score, before, "To Arms! is not a Eureka");

    game.dedication_trigger(0, "army_kill", 2);
    assert_eq!(
        game.players[0].era_score,
        before + 4,
        "two Army kills at +2 Era Score each"
    );
}

#[test]
fn a_heroic_age_still_grants_three_dedications() {
    let mut game = two_player_game();
    game.players[0].age = "dark".to_string();
    game.players[0].era_score = game.players[0].golden_age_threshold;
    game.players[1].era_score = 0;
    game.players[0].techs.insert("horseback_riding".to_string());
    game.process_eras();
    assert_eq!(game.players[0].age, "heroic");
    assert_eq!(game.players[0].dedication_choices, 3);
    assert_eq!(
        game.players[1].dedication_choices, 1,
        "and every other age grants exactly one"
    );
}

#[test]
fn an_age_transition_clears_last_age_dedications() {
    let mut game = two_player_game();
    game.players[0]
        .dedications
        .insert("monumentality".to_string());
    game.players[0].techs.insert("horseback_riding".to_string());
    game.process_eras();
    assert!(
        game.players[0].dedications.is_empty(),
        "a Dedication lasts one age"
    );
}
