//! Forest and jungle fires (`game/fires.rs`): how often they break out, the
//! two burning and four burnt turns, the richer regrowth, the spread into the
//! fire's own feature, and what the flames do to units and improvements.
use super::*;

fn quiet_game() -> Game {
    let mut game = Game::new_full(2, 60, 40, 4243, 500, 0, false);
    // No random disaster of any class, so each test sees only the fire it
    // lit. Fires already burning still advance at intensity zero.
    game.disaster_intensity = 0;
    game
}

/// Drive whole game turns so the world-turn phases (fires included) run.
fn advance(game: &mut Game, turns: u32) {
    let target = game.turn + turns;
    while game.turn < target {
        let before = game.turn;
        while game.turn == before {
            let current = game.current;
            game.apply(current, &Action::EndTurn).unwrap();
        }
    }
}

/// An interior land tile with its whole ring, at least `spacing` steps from
/// every tile already taken, set to the given feature with everything within
/// two steps cleared, so no fire can walk in or out.
fn clearing(game: &mut Game, feature: Name, taken: &mut Vec<Pos>, spacing: i32) -> Pos {
    let position = *game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            !game.rules.is_water(tile)
                && game.map.neighbors(**position).into_iter().count() == 6
                && game
                    .map
                    .neighbors(**position)
                    .into_iter()
                    .all(|next| !game.rules.is_water(&game.map.tiles[&next]))
                && taken
                    .iter()
                    .all(|other| game.wdist(*other, **position) >= spacing)
                && game.city_at(**position).is_none()
        })
        .map(|(position, _)| position)
        .next()
        .expect("the map has open interior land");
    for next in game.wdisk(position, 2) {
        game.map.tiles.get_mut(&next).unwrap().feature = None;
    }
    let tile = game.map.tiles.get_mut(&position).unwrap();
    tile.terrain = crate::name!("grassland");
    tile.feature = Some(feature);
    tile.improvement = None;
    tile.pillaged = false;
    tile.disaster_food = 0.0;
    tile.disaster_production = 0.0;
    taken.push(position);
    position
}

#[test]
fn fires_break_out_at_the_shipped_counts_per_intensity() {
    let mut game = quiet_game();
    for class in ["forest_fire", "jungle_fire"] {
        let counts: Vec<f64> = (0..=4)
            .map(|setting| {
                game.disaster_intensity = setting;
                game.disaster_rate(class)
            })
            .collect();
        // `RandomEvent_Frequencies`: Light 4, Moderate 6, Heavy 10,
        // Hyperreal 15; CIVVIS's 0 is its own "no disasters".
        assert_eq!(counts, vec![0.0, 4.0, 6.0, 10.0, 15.0], "{class}");
    }
    let mut arena = Game::new_with(GameOptions {
        map_script: MapScript::Battlefield,
        ..GameOptions::new(2, 20, 14, 774_411, 60, 0)
    });
    arena.disaster_intensity = 4;
    assert_eq!(
        arena.disaster_rate("forest_fire"),
        0.0,
        "no fires on a battlefield"
    );
}

#[test]
fn a_fire_burns_two_turns_lies_burnt_four_and_regrows_richer() {
    let mut game = quiet_game();
    let position = clearing(&mut game, crate::name!("forest"), &mut Vec::new(), 0);
    game.ignite(position, FireKind::Forest);
    let feature = |game: &Game| game.map.tiles[&position].feature;
    assert_eq!(feature(&game), Some(crate::name!("burning_forest")));
    advance(&mut game, 1);
    assert_eq!(feature(&game), Some(crate::name!("burning_forest")));
    advance(&mut game, 1);
    assert_eq!(feature(&game), Some(crate::name!("burnt_forest")));
    assert_eq!(game.map.tiles[&position].disaster_food, 1.0);
    assert_eq!(game.map.tiles[&position].disaster_production, 0.0);
    advance(&mut game, 3);
    assert_eq!(feature(&game), Some(crate::name!("burnt_forest")));
    advance(&mut game, 1);
    assert_eq!(
        feature(&game),
        Some(crate::name!("forest")),
        "regrown six turns after"
    );
    let tile = &game.map.tiles[&position];
    assert_eq!((tile.disaster_food, tile.disaster_production), (1.0, 1.0));
    assert!(game.fires.is_empty());
}

#[test]
fn a_fire_spreads_into_its_own_feature_about_half_the_time() {
    let mut game = quiet_game();
    let mut taken = Vec::new();
    let mut centres = Vec::new();
    for _ in 0..12 {
        let centre = clearing(&mut game, crate::name!("forest"), &mut taken, 5);
        let ring: Vec<Pos> = game.map.neighbors(centre).into_iter().collect();
        // Half the ring is the fire's own feature, half the other one.
        for (index, next) in ring.iter().enumerate() {
            let tile = game.map.tiles.get_mut(next).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.feature = Some(if index % 2 == 0 {
                crate::name!("forest")
            } else {
                crate::name!("jungle")
            });
        }
        game.ignite(centre, FireKind::Forest);
        centres.push((centre, ring));
    }
    // Turn 1 spreads from the centres alone: a tile lit this turn first
    // spreads next turn.
    advance(&mut game, 1);
    let mut lit = 0;
    for (_, ring) in &centres {
        for (index, next) in ring.iter().enumerate() {
            let feature = game.map.tiles[next].feature;
            if index % 2 == 1 {
                assert_eq!(
                    feature,
                    Some(crate::name!("jungle")),
                    "forest fire never lights jungle"
                );
            } else if feature == Some(crate::name!("burning_forest")) {
                lit += 1;
            }
        }
    }
    // 36 forest neighbours at 50% each.
    assert!((10..=26).contains(&lit), "{lit} of 36 caught");
    // After the centres burn out nothing new catches from them, and every
    // lit tile keeps its own clock.
    advance(&mut game, 8);
    for (centre, ring) in &centres {
        assert_eq!(game.map.tiles[centre].feature, Some(crate::name!("forest")));
        for next in ring {
            assert!(!matches!(
                game.map.tiles[next].feature.as_deref(),
                Some("burning_forest" | "burnt_forest" | "burning_jungle" | "burnt_jungle")
            ));
        }
    }
}

#[test]
fn the_flames_kill_civilians_burn_soldiers_and_pillage_what_was_built() {
    let mut game = quiet_game();
    let position = clearing(&mut game, crate::name!("forest"), &mut Vec::new(), 0);
    game.map.tiles.get_mut(&position).unwrap().improvement = Some(crate::name!("lumber_mill"));
    let builder = game.spawn_unit("builder", 0, position);
    let warrior = game.spawn_unit("warrior", 0, position);
    game.ignite(position, FireKind::Forest);
    assert!(
        game.units.get(&builder).is_none(),
        "a civilian dies in the fire"
    );
    let hp = game.units.get(&warrior).map_or(0, |unit| unit.hp);
    assert!(
        (0..=50).contains(&hp),
        "a warrior loses 50 to 100 HP, has {hp}"
    );
    assert!(game.map.tiles[&position].pillaged);

    // A tribal village is not built, so there is nothing to pillage.
    let mut game = quiet_game();
    let position = clearing(&mut game, crate::name!("jungle"), &mut Vec::new(), 0);
    game.map.tiles.get_mut(&position).unwrap().improvement = Some(crate::name!("goody_hut"));
    game.ignite(position, FireKind::Jungle);
    assert_eq!(
        game.map.tiles[&position].feature,
        Some(crate::name!("burning_jungle"))
    );
    assert!(!game.map.tiles[&position].pillaged);
}

#[test]
fn a_fire_breaks_out_only_on_its_own_feature() {
    let mut game = quiet_game();
    let mut rng = crate::rng::Rng::new(91);
    for _ in 0..5 {
        game.trigger_fire("jungle_fire", &mut rng);
    }
    assert!(!game.fires.is_empty());
    for fire in &game.fires {
        assert_eq!(fire.kind, FireKind::Jungle);
        assert_eq!(
            game.map.tiles[&fire.pos].feature,
            Some(crate::name!("burning_jungle"))
        );
    }
}

#[test]
fn ground_that_changed_under_a_fire_does_not_regrow() {
    let mut game = quiet_game();
    let position = clearing(&mut game, crate::name!("forest"), &mut Vec::new(), 0);
    game.ignite(position, FireKind::Forest);
    advance(&mut game, 3);
    game.map.tiles.get_mut(&position).unwrap().feature = Some(crate::name!("volcanic_soil"));
    advance(&mut game, 4);
    assert_eq!(
        game.map.tiles[&position].feature,
        Some(crate::name!("volcanic_soil"))
    );
    assert!(game.fires.is_empty());
}

#[test]
fn no_city_is_founded_on_burning_or_burnt_ground() {
    let mut game = quiet_game();
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("a major starts with a settler");
    let position = game.units[&settler].pos;
    game.map.tiles.get_mut(&position).unwrap().feature = None;
    assert!(game.can_found_city(settler));
    for feature in [
        "burning_forest",
        "burnt_forest",
        "burning_jungle",
        "burnt_jungle",
    ] {
        game.map.tiles.get_mut(&position).unwrap().feature = Some(Name::new(feature));
        assert!(
            !game.can_found_city(settler),
            "{feature} ships Settlement=false"
        );
    }
}

#[test]
fn a_fire_survives_a_save() {
    let mut game = quiet_game();
    let position = clearing(&mut game, crate::name!("jungle"), &mut Vec::new(), 0);
    game.ignite(position, FireKind::Jungle);
    advance(&mut game, 1);
    let mut restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(restored.fires, game.fires);
    advance(&mut restored, 5);
    assert_eq!(
        restored.map.tiles[&position].feature,
        Some(crate::name!("jungle"))
    );
}
