//! Native pantheons must not consume one of a religion's four belief slots.

use super::*;

fn founder() -> (Game, Pos) {
    let mut game = Game::new(2, 24, 16, 76_332, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let pos = game.units[&settler].pos;
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    game.players[0].religion = Some("Buddhism".to_string());
    game.players[0].pantheon = Some("divine_spark".to_string());
    game.players[0].religion_beliefs = vec![
        "divine_spark".to_string(),
        "work_ethic".to_string(),
        "pilgrimage".to_string(),
    ];
    (game, pos)
}

fn apostle(game: &mut Game, pos: Pos) -> u32 {
    let unit = game.spawn_test_unit("apostle", 0, pos);
    game.units.get_mut(&unit).unwrap().religion = Some("Buddhism".to_string());
    unit
}

#[test]
fn pantheon_leaves_the_fourth_belief_legal_and_executable() {
    let (mut game, pos) = founder();
    game.players[0]
        .religion_beliefs
        .push("holy_order".to_string());
    let unit = apostle(&mut game, pos);
    let fourth = Action::EvangelizeBelief {
        unit,
        belief: crate::name!("mosque"),
    };
    assert!(game.legal_actions(0).contains(&fourth));
    game.apply(0, &fourth).unwrap();
    assert!(!game.units.contains_key(&unit));
    assert_eq!(game.religion_belief_count(0), 4);
    assert!(game.players[0]
        .religion_beliefs
        .contains(&"divine_spark".to_string()));

    let extra = apostle(&mut game, pos);
    assert!(!game.legal_actions(0).iter().any(|action| {
        matches!(action, Action::EvangelizeBelief { unit, .. } if *unit == extra)
    }));
    assert!(game
        .apply(
            0,
            &Action::EvangelizeBelief {
                unit: extra,
                belief: crate::name!("wat"),
            }
        )
        .is_err());
    assert!(game.units.contains_key(&extra));
}

#[test]
fn pantheon_does_not_award_full_development_at_three_religion_beliefs() {
    let (mut game, pos) = founder();
    let unit = apostle(&mut game, pos);
    game.apply(
        0,
        &Action::EvangelizeBelief {
            unit,
            belief: crate::name!("holy_order"),
        },
    )
    .unwrap();
    let moment = "historic_moment:religion_fully_developed";
    assert!(!game.players[0].counters.contains_key(moment));
    let unit = apostle(&mut game, pos);
    game.apply(
        0,
        &Action::EvangelizeBelief {
            unit,
            belief: crate::name!("mosque"),
        },
    )
    .unwrap();
    assert_eq!(game.players[0].counters.get(moment), Some(&1));
}

#[test]
fn pantheon_does_not_relax_taken_belief_or_category_guards() {
    let (mut game, pos) = founder();
    game.players[0]
        .religion_beliefs
        .push("holy_order".to_string());
    game.players[1].religion_beliefs.push("mosque".to_string());
    let unit = apostle(&mut game, pos);
    for belief in ["mosque", "scripture"] {
        let action = Action::EvangelizeBelief {
            unit,
            belief: Name::new(belief),
        };
        assert!(!game.legal_actions(0).contains(&action));
        assert!(game.apply(0, &action).is_err());
        assert!(game.units.contains_key(&unit));
    }
}
