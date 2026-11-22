use super::*;

/// Two majors in the Classical era, the first with a Dedication to choose.
fn classical_choice() -> Game {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    game.world_era = 1;
    game.players[0].dedication_choices = 1;
    game
}

fn religion_seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.victory_target = Some(VictoryTarget::Religion);
    if gene {
        ai.enable_religious_seat_dedicates_to_the_evangelists();
    }
    ai
}

/// A Religion seat with a founded religion takes Exodus of the Evangelists
/// when the Classical age offers it; with the gene off it takes nothing here
/// (the ordinary chooser is left to run).
#[test]
fn a_religion_seat_dedicates_to_the_evangelists() {
    let mut game = classical_choice();
    game.players[0].religion = Some("religion_custom_1".to_string());
    assert!(game
        .available_dedications(0)
        .iter()
        .any(|name| *name == EVANGELISTS));

    let mut ai = religion_seat(true);
    assert!(ai.dedicate_to_the_evangelists(&mut game, 0));
    assert!(game.players[0].dedications.contains(EVANGELISTS));
    assert!(
        game.available_dedications(0).is_empty(),
        "the one choice is spent"
    );

    let mut off = classical_choice();
    off.players[0].religion = Some("religion_custom_1".to_string());
    assert!(!religion_seat(false).dedicate_to_the_evangelists(&mut off, 0));
    assert!(!off.players[0].dedications.contains(EVANGELISTS));
}

/// Another lane's seat, or an age past the Renaissance (Exodus is not
/// offered), leaves the choice to the ordinary chooser.
#[test]
fn other_seats_and_later_ages_are_left_alone() {
    let mut game = classical_choice();
    game.players[0].religion = Some("religion_custom_1".to_string());
    let mut science = AdvancedAi::new();
    science.victory_target = Some(VictoryTarget::Science);
    science.enable_religious_seat_dedicates_to_the_evangelists();
    assert!(!science.dedicate_to_the_evangelists(&mut game, 0));
    assert!(!game.players[0].dedications.contains(EVANGELISTS));

    let mut late = classical_choice();
    late.players[0].religion = Some("religion_custom_1".to_string());
    late.world_era = 5;
    assert!(!late
        .available_dedications(0)
        .iter()
        .any(|name| *name == EVANGELISTS));
    assert!(!religion_seat(true).dedicate_to_the_evangelists(&mut late, 0));
}
