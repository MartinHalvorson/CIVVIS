use super::super::science_victory_drive::ScienceDrive;
use super::*;

#[test]
fn domination_joins_the_race_is_a_native_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers("domination-joins-the-race", |ai| {
        ai.domination_joins_the_race
    });
}

fn standing(own_techs: usize, best_rival_techs: usize) -> ScienceStanding {
    ScienceStanding {
        own_science: 300.0,
        best_rival_science: 400.0,
        own_techs,
        best_rival_techs,
    }
}

/// 073004Z's readings: 62 to 67 at t210 stays out, 67 to 70 at t220
/// opens; 060258Z's 65 to 77 never does. Only a Domination seat, and only
/// with the gene.
#[test]
fn parity_opens_the_answer_for_a_domination_seat_only() {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(!ai.domination_answers_at_parity(&standing(67, 70)));
    ai.enable_domination_joins_the_race();
    assert!(!ai.domination_answers_at_parity(&standing(62, 67)));
    assert!(ai.domination_answers_at_parity(&standing(67, 70)));
    assert!(!ai.domination_answers_at_parity(&standing(65, 77)));

    let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
    culture.enable_domination_joins_the_race();
    assert!(!culture.domination_answers_at_parity(&standing(70, 70)));
}

/// The Domination exclusion lifts only for its opted-in answer. Science
/// remains eligible for its own space production.
#[test]
fn the_exclusions_lift_only_while_the_answer_drives() {
    let drive = |answering| ScienceDrive {
        since: 220,
        reviewed: 220,
        standing: standing(67, 70),
        launch_city: None,
        assigned: false,
        answering,
    };
    let mut stock = AdvancedAi::targeting(VictoryTarget::Domination);
    stock.science_drive = Some(drive(true));
    assert!(
        stock.space_race_excluded(),
        "the gene off keeps the exclusion"
    );

    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_domination_joins_the_race();
    assert!(ai.space_race_excluded(), "no drive, no race");
    ai.science_drive = Some(drive(false));
    assert!(ai.space_race_excluded(), "a drive that is not the answer");
    ai.science_drive = Some(drive(true));
    assert!(
        !ai.space_race_excluded(),
        "the answer drives: the race opens"
    );

    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_domination_joins_the_race();
    assert!(!science.space_race_excluded());
}

fn a_stranded_space_scientist() -> (crate::game::Game, u32) {
    use crate::game::{Action, LiveGreatPersonActivationNeed};
    let mut game = crate::game::Game::new_full(1, 20, 14, 41_119, 250, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    for position in game.cities[&city].owned_tiles.clone() {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.wonder = None;
    }
    game.players[0].techs = game.rules.techs.keys().copied().collect();
    game.cities.get_mut(&city).unwrap().queue.clear();
    game.players[0]
        .live_great_person_activation_needs
        .push(LiveGreatPersonActivationNeed {
            kind: "scientist".into(),
            individual: Some("stephanie_kwolek".into()),
            required_district: Some("spaceport".into()),
            ..Default::default()
        });
    (game, city)
}

#[test]
fn culture_does_not_build_a_spaceport_to_activate_an_idle_scientist() {
    use crate::game::Item;
    let (game, city) = a_stranded_space_scientist();
    for target in [VictoryTarget::Culture, VictoryTarget::Science] {
        let mut candidate = game.clone();
        let mut ai = AdvancedAi::targeting(target);
        ai.base.exclude_space_race = ai.space_race_excluded();
        let changed = ai
            .base
            .prioritize_live_great_person_activation(&mut candidate, 0);
        if target == VictoryTarget::Culture {
            assert!(!changed);
            assert!(candidate.cities[&city].queue.is_empty());
            assert!(!ai.science_endgame_lane_committed(&candidate, 0));
        } else {
            assert!(
                changed,
                "the same legal activation path remains useful to Science"
            );
            assert!(matches!(candidate.cities[&city].queue.first(),
                Some(Item::District { district, .. }) if candidate.district_family(*district) == "spaceport"));
        }
    }
}

#[test]
fn culture_still_opens_a_theater_for_an_idle_writer() {
    use crate::game::Item;
    let (mut game, city) = a_stranded_space_scientist();
    game.players[0].civics = game.rules.civics.keys().copied().collect();
    let need = &mut game.players[0].live_great_person_activation_needs[0];
    need.kind = "writer".into();
    need.individual = Some("alexander_pushkin".into());
    need.required_district = Some("theater_square".into());
    need.required_great_work = Some("writing".into());
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.base.exclude_space_race = ai.space_race_excluded();
    assert!(ai
        .base
        .prioritize_live_great_person_activation(&mut game, 0));
    assert!(matches!(game.cities[&city].queue.first(),
        Some(Item::District { district, .. }) if game.district_family(*district) == "theater_square"));
}

#[test]
fn a_science_answer_cannot_reopen_an_explicit_culture_targets_space_race() {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_domination_joins_the_race();
    ai.science_drive = Some(ScienceDrive {
        since: 160,
        reviewed: 160,
        standing: standing(67, 70),
        launch_city: None,
        assigned: false,
        answering: true,
    });
    assert!(ai.space_race_excluded());
    assert!(!AdvancedAi::new().space_race_excluded());
}
