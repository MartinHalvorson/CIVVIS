use super::*;

fn reference_count(game: &Game, ours: u32, position: Pos, threats: &[TheologicalThreat]) -> usize {
    threats
        .iter()
        .filter(|threat| {
            threat
                .strike_from
                .iter()
                .any(|from| *from != position && game.wdist(*from, position) <= 1)
                && game
                    .theological_exchange_preview_with_defender_at(threat.rival, ours, position)
                    .is_some_and(|(dealt, received, defender_hp)| {
                        dealt >= f64::from(defender_hp) || dealt > received
                    })
        })
        .count()
}

fn overlapping_threats() -> (Game, u32, Vec<TheologicalThreat>) {
    let (mut game, mine, rival, _) = chased().expect("fixture has an Apostle's threat boundary");
    let positions: Vec<Pos> = game
        .nbrs(game.units[&rival].pos)
        .into_iter()
        .filter(|position| {
            game.map
                .get(*position)
                .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
                && game.units_at(*position).is_empty()
        })
        .take(3)
        .collect();
    assert!(!positions.is_empty(), "fixture can place another rival");
    for position in positions {
        let additional = game.spawn_unit("apostle", 1, position);
        game.units.get_mut(&additional).unwrap().religion = Some("orthodoxy".into());
    }
    let threats = armed().theological_threats(&game, 0, mine);
    assert!(threats.len() >= 2, "fixture reads overlapping rival corps");
    (game, mine, threats)
}

#[test]
fn safety_matches_the_original_count_on_every_tile_and_health_level() {
    let (mut game, mine, threats) = overlapping_threats();
    let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
    for health in [100, 50, 1] {
        game.units.get_mut(&mine).unwrap().hp = health;
        let mut safe = 0;
        let mut overlapping = 0;
        for position in positions.iter().copied() {
            let expected = reference_count(&game, mine, position, &threats);
            assert_eq!(
                AdvancedAi::theological_threats_reaching(&game, mine, position, &threats),
                expected,
                "full count at {position:?}, health {health}"
            );
            assert_eq!(
                AdvancedAi::theological_tile_is_safe(&game, mine, position, &threats),
                expected == 0,
                "safety at {position:?}, health {health}"
            );
            safe += usize::from(expected == 0);
            overlapping += usize::from(expected >= 2);
        }
        assert!(safe > 0, "fixture includes safe tiles");
        assert!(
            overlapping > 0,
            "fixture includes overlapping winning rivals"
        );
    }
}

#[test]
fn an_empty_threat_list_is_safe_everywhere() {
    let (game, mine, _, _) = chased().expect("fixture has an Apostle's threat boundary");
    for position in game.map.tiles.keys().copied() {
        assert!(AdvancedAi::theological_tile_is_safe(
            &game,
            mine,
            position,
            &[]
        ));
        assert_eq!(reference_count(&game, mine, position, &[]), 0);
    }
}
