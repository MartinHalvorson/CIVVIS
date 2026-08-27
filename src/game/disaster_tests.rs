//! Gathering Storm's random disasters: the intensity setting, the four storm
//! systems, river floods, droughts and volcanic eruptions, and the way a
//! warming world makes all of them more frequent and more severe.
use super::{Drought, Game, Item, Pos, Storm, DEFAULT_DISASTER_INTENSITY};
use crate::name::Name;

fn quiet_game() -> Game {
    // Disasters roll against a per-turn probability derived from `max_turns`,
    // so every test states the turn budget it is reasoning about.
    Game::new_full(2, 30, 18, 4242, 500, 0, false)
}

fn game_with_capitals() -> Game {
    let mut game = quiet_game();
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("each major starts with a settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    game
}

/// Drive whole game turns so the world-turn phases (including disasters) run.
fn advance(game: &mut Game, turns: u32) {
    let target = game.turn + turns;
    while game.turn < target {
        let before = game.turn;
        while game.turn == before {
            let current = game.current;
            game.apply(current, &super::Action::EndTurn).unwrap();
        }
    }
}

fn land_tile(game: &Game) -> Pos {
    *game
        .map
        .tiles
        .iter()
        .find(|(_, tile)| !game.rules.is_water(tile) && tile.feature.is_none())
        .map(|(position, _)| position)
        .expect("the map has open land")
}

fn volcano_and_ring(game: &Game) -> (Pos, Vec<Pos>) {
    game.map
        .tiles
        .keys()
        .find_map(|position| {
            let ring: Vec<Pos> = game.map.neighbors(*position).into_iter().collect();
            (ring.len() == 6).then_some((*position, ring))
        })
        .expect("the map has an interior tile with a complete ring")
}

fn arm_certain_eruption(game: &mut Game, volcano: Pos, ring: &[Pos]) {
    std::sync::Arc::make_mut(&mut game.rules)
        .disasters
        .get_mut("volcanic_eruption")
        .unwrap()
        .fertility_chance = vec![1.0; 3];
    {
        let tile = game.map.tiles.get_mut(&volcano).unwrap();
        tile.terrain = crate::name!("mountain");
        tile.hills = false;
        tile.feature = Some(crate::name!("volcano"));
        tile.owner_city = None;
    }
    for position in ring {
        let tile = game.map.tiles.get_mut(position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.hills = false;
        tile.feature = None;
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
        tile.owner_city = None;
        tile.pillaged = false;
    }
}

#[test]
fn the_default_intensity_is_the_middle_of_the_five_settings() {
    let game = quiet_game();
    assert_eq!(game.disaster_intensity(), DEFAULT_DISASTER_INTENSITY);
    assert_eq!(DEFAULT_DISASTER_INTENSITY, 2);
}

#[test]
fn intensity_zero_leaves_every_volcano_dormant_and_fires_nothing() {
    let mut game = quiet_game();
    game.disaster_intensity = 0;
    let volcano = land_tile(&game);
    game.map.tiles.get_mut(&volcano).unwrap().feature = Some(crate::name!("volcano"));
    assert!(!game.volcano_active(volcano));

    advance(&mut game, 30);
    assert!(game.storms.is_empty(), "no storms form with disasters off");
    assert!(game.droughts.is_empty(), "no droughts form with disasters off");
}

/// A player who actually loses population to a random disaster receives the
/// aid target; other majors, never the target, may score the 200-point project.
#[test]
fn disaster_population_loss_seats_a_targeted_native_aid_request() {
    let mut game = game_with_capitals();
    let target = 0;
    let member = 1;
    let target_city = game.player_city_ids(target)[0];
    let member_city = game.player_city_ids(member)[0];
    let target_pos = game.cities[&target_city].pos;
    let aid = Item::Project {
        project: crate::name!("send_aid"),
    };
    game.cities.get_mut(&target_city).unwrap().pop = 3;

    // The staged mechanism leaves an ordinary board alone.
    game.disaster_population_loss(target_pos, 1);
    assert!(game.competition.is_none());

    game.native_competitions = true;
    game.disaster_population_loss(target_pos, 1);
    let running = game
        .competition
        .as_ref()
        .expect("a disaster casualty seats the shipped Send Aid request");
    assert_eq!(running.kind, "EMERGENCY_SEND_AID");
    assert_eq!(running.target, Some(target));
    assert_eq!(
        running.ends,
        game.turn + game.standard_duration(30),
        "Send Aid's shipped duration is 30 turns"
    );

    assert!(
        game.host_competition(member, "EMERGENCY_SEND_AID")
            .is_some(),
        "another major is a request member"
    );
    assert!(
        game.host_competition(target, "EMERGENCY_SEND_AID")
            .is_none(),
        "the affected civilization receives aid rather than scoring it"
    );
    assert!(game.can_produce(member, member_city, &aid));
    assert!(!game.can_produce(target, target_city, &aid));

    assert!(game.complete_item(member, member_city, &aid));
    let score = game.competition.as_ref().unwrap().scores[&member];
    assert_eq!(
        score, game.rules.projects["send_aid"].competition_score,
        "the request uses the shipped 200-point Send Aid project"
    );
    assert!(
        game.complete_item(target, target_city, &aid),
        "the low-level completion path remains harmless when called directly"
    );
    assert_eq!(
        game.competition.as_ref().unwrap().scores[&member],
        score,
        "the aid target cannot manufacture a score through that direct path"
    );

    let before = game.players[member].dvp;
    game.turn = game.competition.as_ref().unwrap().ends;
    game.close_native_competition();
    assert_eq!(
        game.players[member].dvp - before,
        2,
        "Gathering Storm pays an Aid Request's first-place member two Diplomatic Victory Points"
    );
    assert_eq!(
        game.competition_lockout_until["EMERGENCY_SEND_AID"],
        game.turn + game.standard_duration(30),
        "Send Aid carries its own 30-turn lockout"
    );

    game.cities.get_mut(&target_city).unwrap().pop = 3;
    game.disaster_population_loss(target_pos, 1);
    assert!(
        game.competition.is_none(),
        "a second casualty cannot bypass the request's own lockout"
    );
}

#[test]
fn a_higher_intensity_activates_more_volcanoes_and_fires_more_often() {
    let mut game = quiet_game();
    let volcanoes: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| !game.rules.is_water(tile))
        .map(|(position, _)| *position)
        .take(400)
        .collect();
    for position in &volcanoes {
        game.map.tiles.get_mut(position).unwrap().feature = Some(crate::name!("volcano"));
    }
    let active_at = |game: &mut Game, intensity: u8| {
        game.disaster_intensity = intensity;
        volcanoes
            .iter()
            .filter(|position| game.volcano_active(**position))
            .count()
    };
    // The shipped band runs from 45% of the map's cones to 95% of them.
    let minimal = active_at(&mut game, 1);
    let hyperreal = active_at(&mut game, 4);
    assert!(
        minimal < hyperreal,
        "intensity 1 activated {minimal} cones, intensity 4 only {hyperreal}"
    );
    assert!((0.30..0.60).contains(&(minimal as f64 / volcanoes.len() as f64)));
    assert!((0.85..1.0).contains(&(hyperreal as f64 / volcanoes.len() as f64)));

    game.disaster_intensity = 1;
    let quiet = game.disaster_rate("river_flood");
    game.disaster_intensity = 4;
    assert!(game.disaster_rate("river_flood") > quiet * 4.0);
}

#[test]
fn an_eruption_damages_the_ring_and_leaves_volcanic_soil() {
    let mut game = quiet_game();
    let volcano = land_tile(&game);
    game.map.tiles.get_mut(&volcano).unwrap().feature = Some(crate::name!("volcano"));
    let ring: Vec<Pos> = game
        .wdisk(volcano, 1)
        .into_iter()
        .filter(|position| *position != volcano && !game.rules.is_water(&game.map.tiles[position]))
        .collect();
    assert!(!ring.is_empty(), "the volcano needs land around it");
    for position in &ring {
        let tile = game.map.tiles.get_mut(position).unwrap();
        tile.feature = None;
        tile.improvement = Some(crate::name!("farm"));
        tile.pillaged = false;
    }

    game.resolve_eruption(volcano, 3);

    assert!(
        ring.iter()
            .any(|position| game.map.tiles[position].pillaged),
        "a severity-3 eruption pillages what is built around the cone"
    );
    assert!(
        ring.iter()
            .any(|position| game.map.tiles[position].feature.as_deref() == Some("volcanic_soil")),
        "ash leaves Volcanic Soil behind"
    );
    assert_eq!(
        game.map.tiles[&volcano].feature.as_deref(),
        Some("volcano"),
        "the cone itself is not buried"
    );
}

/// ★★★★ A VOLCANIC NATURAL WONDER IS A VOLCANO, AND THIS ENGINE SAID IT WAS
/// SCENERY.
///
/// Gathering Storm ships `Features_XP2.Volcano` on FOUR features: the generic
/// `FEATURE_VOLCANO` and the three volcanic Natural Wonders — Vesuvius,
/// Kilimanjaro and Eyjafjallajokull. Every volcano test in this engine matched
/// the feature NAME `volcano` instead, so the three wonders were dormant for
/// good: they never entered the activation lottery, `trigger_eruption` never
/// drew one, and not one hex of Volcanic Soil ever came out of one.
#[test]
fn the_shipped_volcano_flag_names_the_three_volcanic_natural_wonders_too() {
    let rules = crate::rules::Rules::embedded();
    let volcanic: Vec<&str> = rules
        .features
        .iter()
        .filter(|(_, spec)| spec.volcano)
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(
        volcanic,
        ["eyjafjallajokull", "kilimanjaro", "vesuvius", "volcano"],
        "the shipped Features_XP2 Volcano roster is these four features"
    );
    for wonder in ["vesuvius", "kilimanjaro", "eyjafjallajokull"] {
        assert!(
            rules.features[wonder].natural_wonder,
            "{wonder} is a Natural Wonder as well as a volcano"
        );
    }
}

#[test]
fn a_volcanic_natural_wonder_erupts_and_leaves_its_own_volcanic_soil() {
    for wonder in ["vesuvius", "kilimanjaro", "eyjafjallajokull"] {
        let mut game = quiet_game();
        let (volcano, ring) = volcano_and_ring(&game);
        arm_certain_eruption(&mut game, volcano, &ring);
        game.map.tiles.get_mut(&volcano).unwrap().feature = Some(Name::new(wonder));

        game.resolve_eruption(volcano, 3);

        assert!(
            ring.iter().all(
                |position| game.map.tiles[position].feature.as_deref() == Some("volcanic_soil")
            ),
            "{wonder} left no Volcanic Soil on the ring it reached"
        );
        assert_eq!(
            game.map.tiles[&volcano].feature.as_deref(),
            Some(wonder),
            "the wonder itself is not buried by its own eruption"
        );
    }
}

/// A second hex of the same wonder — Eyjafjallajokull is a two-tile footprint —
/// is part of the cone, not ground the cone can bury.
#[test]
fn an_eruption_does_not_bury_the_rest_of_its_own_natural_wonder() {
    let mut game = quiet_game();
    let (volcano, ring) = volcano_and_ring(&game);
    arm_certain_eruption(&mut game, volcano, &ring);
    game.map.tiles.get_mut(&volcano).unwrap().feature = Some(crate::name!("eyjafjallajokull"));
    let twin = ring[0];
    game.map.tiles.get_mut(&twin).unwrap().feature = Some(crate::name!("eyjafjallajokull"));

    game.resolve_eruption(volcano, 3);

    assert_eq!(
        game.map.tiles[&twin].feature.as_deref(),
        Some("eyjafjallajokull"),
        "the wonder's other hex kept its own feature"
    );
}

#[test]
fn high_intensity_makes_most_volcanic_natural_wonders_active_cones() {
    let mut game = quiet_game();
    let sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| !game.rules.is_water(tile))
        .map(|(position, _)| *position)
        .take(300)
        .collect();
    for position in &sites {
        game.map.tiles.get_mut(position).unwrap().feature = Some(crate::name!("vesuvius"));
    }
    game.disaster_intensity = 0;
    assert_eq!(
        sites
            .iter()
            .filter(|position| game.volcano_active(**position))
            .count(),
        0,
        "disasters off leaves a volcanic wonder dormant like any other cone"
    );
    game.disaster_intensity = 4;
    let active = sites
        .iter()
        .filter(|position| game.volcano_active(**position))
        .count();
    assert!(
        (0.85..=1.0).contains(&(active as f64 / sites.len() as f64)),
        "the shipped band tops out at 95% of the map's cones, got {active} of {}",
        sites.len()
    );
}

#[test]
fn the_eruption_lottery_draws_a_volcanic_natural_wonder() {
    let mut game = quiet_game();
    game.disaster_intensity = 4;
    // Clear the generator's own cones so the only volcano left on the map is a
    // Natural Wonder. A dormant roster produces no eruption at all, which is
    // exactly what used to happen.
    let generated: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| tile.feature.as_deref() == Some("volcano"))
        .map(|(position, _)| *position)
        .collect();
    for position in generated {
        game.map.tiles.get_mut(&position).unwrap().feature = None;
    }
    let (volcano, ring) = game
        .map
        .tiles
        .keys()
        .copied()
        .collect::<Vec<Pos>>()
        .into_iter()
        .find_map(|position| {
            let ring: Vec<Pos> = game.map.neighbors(position).into_iter().collect();
            if ring.len() != 6 {
                return None;
            }
            game.map.tiles.get_mut(&position).unwrap().feature = Some(crate::name!("vesuvius"));
            if game.volcano_active(position) {
                return Some((position, ring));
            }
            game.map.tiles.get_mut(&position).unwrap().feature = None;
            None
        })
        .expect("some interior tile draws an active cone at Apocalyptic intensity");
    arm_certain_eruption(&mut game, volcano, &ring);
    game.map.tiles.get_mut(&volcano).unwrap().feature = Some(crate::name!("vesuvius"));

    let mut rng = crate::rng::Rng::new(9);
    game.trigger_eruption(3, &mut rng);

    assert!(
        ring.iter()
            .any(|position| game.map.tiles[position].feature.as_deref() == Some("volcanic_soil")),
        "the lottery erupted the wonder and Volcanic Soil came out of it"
    );
}

/// ★★★★★ FERTILITY IS FOOD **AND PRODUCTION**, ROLLED APART.
///
/// `RandomEvent_Yields` rates each yield type separately — a Catastrophic
/// eruption leaves Food on half the plots it reaches and Production on a
/// quarter of them — and this engine rolled once and granted Food.
/// `Tile::disaster_production` was summed into a tile's yields the whole time
/// and nothing ever wrote it, so a plot Civilization VI pays 3 Food 3
/// Production after an eruption was paid 3 Food 2 Production here, and the same
/// gap reached the mirrored board through the tile catalogue.
#[test]
fn an_eruption_leaves_production_fertility_and_not_only_food() {
    let mut game = quiet_game();
    let (volcano, ring) = volcano_and_ring(&game);
    arm_certain_eruption(&mut game, volcano, &ring);
    {
        let spec = std::sync::Arc::make_mut(&mut game.rules)
            .disasters
            .get_mut("volcanic_eruption")
            .unwrap();
        spec.fertility_chance = vec![1.0; 3];
        spec.fertility_production_chance = vec![1.0; 3];
        // Nothing may be pillaged out from under the fertility roll.
        spec.pillage_chance = vec![0.0; 3];
    }

    game.resolve_eruption(volcano, 3);

    let fertile: Vec<&Pos> = ring
        .iter()
        .filter(|position| game.map.tiles[*position].disaster_food > 0.0)
        .collect();
    assert!(
        !fertile.is_empty(),
        "a certain eruption fertilises its ring"
    );
    for position in fertile {
        assert_eq!(
            game.map.tiles[position].disaster_production, 1.0,
            "the shipped table's YIELD_PRODUCTION half has to land too"
        );
    }
}

/// The two halves are independent rolls, so a class the shipped table gives
/// Production and no Food — none ships that way today, but the shape is the
/// point — must still pay its Production.
#[test]
fn production_fertility_is_rolled_apart_from_food_fertility() {
    let mut game = quiet_game();
    let (volcano, ring) = volcano_and_ring(&game);
    arm_certain_eruption(&mut game, volcano, &ring);
    {
        let spec = std::sync::Arc::make_mut(&mut game.rules)
            .disasters
            .get_mut("volcanic_eruption")
            .unwrap();
        spec.fertility_chance = vec![0.0; 3];
        spec.fertility_production_chance = vec![1.0; 3];
        spec.pillage_chance = vec![0.0; 3];
    }

    game.resolve_eruption(volcano, 3);

    let enriched: Vec<&Pos> = ring
        .iter()
        .filter(|position| game.map.tiles[*position].disaster_production > 0.0)
        .collect();
    assert!(
        !enriched.is_empty(),
        "Production fertility does not need a Food roll"
    );
    for position in &enriched {
        assert_eq!(
            game.map.tiles[*position].disaster_food, 0.0,
            "a Food chance of zero must leave no Food"
        );
        assert_eq!(
            game.map.tiles[*position].feature.as_deref(),
            Some("volcanic_soil"),
            "`ReplaceFeature` rides on the yield rows, so ash settles where fertility did"
        );
    }
}

/// The shipped rates, read out of `Expansion2_RandomEvents.xml` rather than
/// guessed: every class the table gives a fertility row has one here, and the
/// two it gives none — Tornado and Drought — stay at zero.
#[test]
fn the_shipped_fertility_table_is_what_the_ruleset_carries() {
    let game = quiet_game();
    for (id, food, production) in [
        ("volcanic_eruption", true, true),
        ("river_flood", true, true),
        ("hurricane", true, true),
        ("blizzard", true, false),
        ("dust_storm", true, true),
    ] {
        let spec = &game.rules.disasters[id];
        assert!(
            spec.fertility_chance.iter().any(|chance| *chance > 0.0) == food,
            "{id} Food fertility"
        );
        assert!(
            spec.fertility_production_chance
                .iter()
                .any(|chance| *chance > 0.0)
                == production,
            "{id} Production fertility"
        );
    }
    for id in ["tornado", "drought"] {
        let spec = &game.rules.disasters[id];
        assert!(
            spec.fertility_chance.iter().all(|chance| *chance == 0.0),
            "{id}"
        );
        assert!(
            spec.fertility_production_chance
                .iter()
                .all(|chance| *chance == 0.0),
            "{id}"
        );
    }
}

/// ★★★★★ AND AN UNOWNED PLOT KEEPS IT TOO.
///
/// `modeled_tile_yields` used to hand a tile nobody owns straight to
/// `Rules::worked_tile_yields`, which reads terrain, feature, resource and
/// improvement and knows nothing about the ground's own history — so the
/// fertility an eruption left, a neighbouring wonder's adjacency, drought and
/// fallout were all invisible until somebody's border reached the plot. That is
/// exactly backwards: unclaimed ground is what a Settler and a Builder are
/// choosing BETWEEN, and Volcanic Soil has no yields of its own, so the
/// fertility is the whole of what the plot is worth.
#[test]
fn unclaimed_ground_still_pays_the_fertility_a_disaster_left_on_it() {
    let mut game = quiet_game();
    let position = land_tile(&game);
    {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.owner_city = None;
    }
    let bare = game.modeled_tile_yields(position);
    assert_eq!((bare.food, bare.production), (2.0, 0.0), "{bare:?}");

    {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.feature = Some(crate::name!("volcanic_soil"));
        tile.disaster_food = 1.0;
        tile.disaster_production = 3.0;
    }
    let fertile = game.modeled_tile_yields(position);
    assert_eq!(
        (fertile.food, fertile.production),
        (3.0, 3.0),
        "unclaimed volcanic soil pays what the ash left: {fertile:?}"
    );

    // And the ground stops paying while it is under fallout, whoever holds it.
    game.map.tiles.get_mut(&position).unwrap().fallout_until = game.turn + 5;
    assert_eq!(
        game.modeled_tile_yields(position),
        crate::rules::Yields::default()
    );
}

#[test]
fn an_eruption_keeps_volcanic_soil_off_mountains_and_every_kind_of_water() {
    let mut game = quiet_game();
    let (volcano, ring) = volcano_and_ring(&game);
    arm_certain_eruption(&mut game, volcano, &ring);
    for (position, terrain) in ring[..4]
        .iter()
        .zip(["mountain", "coast", "lake", "ocean"])
    {
        game.map.tiles.get_mut(position).unwrap().terrain = Name::new(terrain);
    }
    game.map.tiles.get_mut(&ring[5]).unwrap().terrain = crate::name!("snow");
    game.map.tiles.get_mut(&ring[5]).unwrap().hills = true;

    game.resolve_eruption(volcano, 3);

    for position in &ring[..4] {
        assert_eq!(
            game.map.tiles[position].feature, None,
            "{} cannot carry Volcanic Soil",
            game.map.tiles[position].terrain
        );
    }
    for position in &ring[4..] {
        assert_eq!(
            game.map.tiles[position].feature.as_deref(),
            Some("volcanic_soil"),
            "{} land can carry Volcanic Soil",
            game.map.tiles[position].terrain
        );
    }
}

#[test]
fn eruption_soil_obeys_replacement_and_built_site_rules() {
    let mut game = quiet_game();
    let (volcano, ring) = volcano_and_ring(&game);
    arm_certain_eruption(&mut game, volcano, &ring);
    game.map.tiles.get_mut(&ring[0]).unwrap().district = Some(crate::name!("campus"));
    game.map.tiles.get_mut(&ring[1]).unwrap().wonder = Some(crate::name!("pyramids"));
    game.map.tiles.get_mut(&ring[2]).unwrap().feature = Some(crate::name!("forest"));
    game.map.tiles.get_mut(&ring[3]).unwrap().feature = Some(crate::name!("geothermal_fissure"));

    game.resolve_eruption(volcano, 3);

    for position in &ring[..3] {
        assert_eq!(
            game.map.tiles[position].feature.as_deref(),
            Some("volcanic_soil"),
            "soil is compatible with built sites and replaces ordinary ground cover"
        );
    }
    assert_eq!(
        game.map.tiles[&ring[3]].feature.as_deref(),
        Some("geothermal_fissure"),
        "a feature without ValidForReplacement survives the eruption"
    );
}

#[test]
fn the_top_two_intensities_widen_the_eruption_to_two_rings() {
    let mut game = quiet_game();
    let volcano = land_tile(&game);
    game.map.tiles.get_mut(&volcano).unwrap().feature = Some(crate::name!("volcano"));
    let second_ring: Vec<Pos> = game
        .wdisk(volcano, 2)
        .into_iter()
        .filter(|position| {
            game.wdist(*position, volcano) == 2 && !game.rules.is_water(&game.map.tiles[position])
        })
        .collect();
    assert!(!second_ring.is_empty());
    let arm = |game: &mut Game| {
        for position in &second_ring {
            let tile = game.map.tiles.get_mut(position).unwrap();
            tile.feature = None;
            tile.improvement = Some(crate::name!("farm"));
            tile.pillaged = false;
        }
    };

    game.disaster_intensity = 2;
    arm(&mut game);
    game.resolve_eruption(volcano, 3);
    assert!(
        !second_ring
            .iter()
            .any(|position| game.map.tiles[position].pillaged),
        "at Moderate an eruption reaches one ring"
    );

    game.disaster_intensity = 4;
    arm(&mut game);
    game.resolve_eruption(volcano, 3);
    assert!(
        second_ring
            .iter()
            .any(|position| game.map.tiles[position].pillaged),
        "at Hyperreal it reaches two"
    );
}

#[test]
fn a_drought_holds_its_tiles_then_lifts_on_schedule() {
    let mut game = quiet_game();
    let farm = land_tile(&game);
    game.map.tiles.get_mut(&farm).unwrap().improvement = Some(crate::name!("farm"));
    game.resolve_drought(&[farm]);
    game.droughts.push(Drought {
        tiles: vec![farm],
        severity: 1,
        ends: game.turn + 3,
    });
    assert!(game.map.tiles[&farm].drought);

    advance(&mut game, 2);
    assert!(game.map.tiles[&farm].drought, "the drought has not run out");

    advance(&mut game, 2);
    assert!(!game.map.tiles[&farm].drought, "the rain came back");
    assert!(game.droughts.is_empty());
    assert!(
        game.map.tiles[&farm].pillaged,
        "the farm it killed stays killed until it is repaired"
    );
}

#[test]
fn a_storm_drifts_for_three_turns_and_then_dissipates() {
    let mut game = quiet_game();
    let origin = *game
        .map
        .tiles
        .iter()
        .find(|(_, tile)| tile.terrain == "plains")
        .map(|(position, _)| position)
        .expect("the map has plains");
    game.storms.push(Storm {
        kind: "tornado".to_string(),
        pos: origin,
        heading: 0,
        severity: 1,
        ends: game.turn + 3,
    });
    // Anything a storm forms on top of is not what the storm is; clear the
    // rest of the roll so only this system is under test.
    game.disaster_intensity = 0;

    advance(&mut game, 1);
    let moved = game.storms.first().map(|storm| storm.pos);
    assert!(moved.is_some(), "the system is still alive on turn one");
    assert_ne!(moved, Some(origin), "a storm does not sit still");
    assert!(
        game.map
            .tiles
            .values()
            .any(|tile| tile.storm.as_deref() == Some("tornado")),
        "the tiles under it are marked while it passes"
    );

    advance(&mut game, 3);
    assert!(game.storms.is_empty(), "three turns and it is gone");
    assert!(
        game.map
            .tiles
            .values()
            .all(|tile| tile.storm.is_none()),
        "and it leaves no marker behind"
    );
}

#[test]
fn a_warming_world_is_a_stormier_one() {
    let mut game = quiet_game();
    let cold = game.disaster_rate("hurricane");
    game.climate_phase = 6;
    let hot = game.disaster_rate("hurricane");
    assert!(
        hot > cold,
        "climate phase 6 should raise the hurricane rate above {cold}, got {hot}"
    );
}

#[test]
fn disasters_actually_fire_over_a_full_game() {
    // The rates are per-game expectations, so a real run has to produce
    // events; a system that never triggers is the failure this guards.
    let mut game = Game::new_full(2, 30, 18, 31337, 200, 0, false);
    game.disaster_intensity = 4;
    let mut storms = 0usize;
    let mut droughts = 0usize;
    for _ in 0..60 {
        advance(&mut game, 1);
        storms += game.storms.len();
        droughts += game.droughts.len();
    }
    assert!(
        storms > 0 || droughts > 0,
        "sixty turns at Hyperreal produced no disasters at all"
    );
}

#[test]
fn disaster_state_survives_a_save() {
    let mut game = quiet_game();
    let farm = land_tile(&game);
    game.map.tiles.get_mut(&farm).unwrap().disaster_food = 2.0;
    game.disaster_intensity = 3;
    game.storms.push(Storm {
        kind: "blizzard".to_string(),
        pos: farm,
        heading: 2,
        severity: 2,
        ends: game.turn + 3,
    });
    game.droughts.push(Drought {
        tiles: vec![farm],
        severity: 1,
        ends: game.turn + 5,
    });

    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(restored.disaster_intensity, 3);
    assert_eq!(restored.storms, game.storms);
    assert_eq!(restored.droughts, game.droughts);
    assert_eq!(restored.map.tiles[&farm].disaster_food, 2.0);
}

#[test]
fn a_save_written_before_disasters_loads_at_the_default_intensity() {
    let game = quiet_game();
    let mut value: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    let object = value.as_object_mut().unwrap();
    object.remove("disaster_intensity");
    object.remove("storms");
    object.remove("droughts");
    let restored: Game = serde_json::from_value(value).unwrap();
    assert_eq!(restored.disaster_intensity(), DEFAULT_DISASTER_INTENSITY);
    assert!(restored.storms.is_empty());
}

/// The rates in `disasters.json` are per-game expectations, so a full game
/// has to land near them. This is the guard on the whole scheduler: a class
/// that silently stops firing, or one that fires every turn, shows up here.
#[test]
fn a_full_game_lands_near_the_rates_the_ruleset_asks_for() {
    let mut spawned = 0usize;
    const GAMES: u64 = 3;
    for seed in 0..GAMES {
        let mut game = Game::new_full(2, 40, 24, 900 + seed, 500, 0, false);
        game.disaster_intensity = DEFAULT_DISASTER_INTENSITY;
        for _ in 0..500 {
            let turn = game.turn;
            while game.turn == turn {
                let current = game.current;
                if game.apply(current, &super::Action::EndTurn).is_err() {
                    break;
                }
            }
            // A system is counted on the turn it forms, when its expiry is
            // still a full duration away.
            spawned += game
                .storms
                .iter()
                .filter(|storm| storm.ends == game.turn + 3)
                .count();
        }
    }
    // The four storm classes budget 26 systems a game between them at
    // Moderate; a Poisson-ish spread over three games should stay well inside
    // half to double that.
    let per_game = spawned as f64 / GAMES as f64;
    assert!(
        (13.0..=52.0).contains(&per_game),
        "{per_game} storms a game is nowhere near the 26 the ruleset budgets"
    );
}

/// Intensity has to move the whole system, not just the volcano share.
#[test]
fn raising_the_intensity_raises_what_actually_happens() {
    let count = |intensity: u8| {
        let mut game = Game::new_full(2, 40, 24, 4711, 500, 0, false);
        game.disaster_intensity = intensity;
        let mut spawned = 0usize;
        for _ in 0..300 {
            let turn = game.turn;
            while game.turn == turn {
                let current = game.current;
                if game.apply(current, &super::Action::EndTurn).is_err() {
                    break;
                }
            }
            spawned += game
                .storms
                .iter()
                .filter(|storm| storm.ends == game.turn + 3)
                .count();
        }
        spawned
    };
    let light = count(1);
    let hyperreal = count(4);
    assert!(
        hyperreal > light * 2,
        "Hyperreal produced {hyperreal} storms against Light's {light}"
    );
}

#[test]
fn every_disaster_covers_the_tile_count_its_hexes_column_ships() {
    // RandomEvents.Hexes is a tile count, and CIVVIS stores a hex-disk radius,
    // so the two line up exactly wherever Hexes is a ring size: 1, 7 or 19.
    let game = quiet_game();
    let rules = crate::rules::Rules::embedded();
    let centre: Pos = (12, 8);
    let tiles = |radius: i32| game.wdisk(centre, radius).len();
    assert_eq!((tiles(0), tiles(1), tiles(2)), (1, 7, 19));

    // (disaster, severity, shipped Hexes)
    let expected: &[(&str, u8, usize)] = &[
        // HURRICANE_CAT_4 7, CAT_5 19.
        ("hurricane", 1, 7),
        ("hurricane", 2, 19),
        // BLIZZARD_SIGNIFICANT 7, CRIPPLING 19.
        ("blizzard", 1, 7),
        ("blizzard", 2, 19),
        // TORNADO_FAMILY is a single tile.
        ("tornado", 1, 1),
        // DUST_STORM_HABOOB is 7, not the 19 CIVVIS used to cover.
        ("dust_storm", 2, 7),
        // DROUGHT_MAJOR and _EXTREME are both 7.
        ("drought", 1, 7),
        ("drought", 2, 7),
    ];
    for &(disaster, severity, hexes) in expected {
        let spec = &rules.disasters[disaster];
        assert_eq!(
            tiles(spec.radius(severity)),
            hexes,
            "{disaster} severity {severity}"
        );
    }

    // Drought ships two levels, not three -- RANDOM_EVENT_DROUGHT_MAJOR at
    // Severity 0 and _EXTREME at Severity 1 -- and their Durations are 5 and
    // 10, where CIVVIS carried an invented third level running 8/12/16.
    let drought = &rules.disasters["drought"];
    assert_eq!(drought.severities, 2);
    assert_eq!(drought.duration(1), 5);
    assert_eq!(drought.duration(2), 10);

    // Every storm system ships Duration 3.
    for storm in ["hurricane", "blizzard", "tornado", "dust_storm"] {
        for severity in 1..=2 {
            assert_eq!(rules.disasters[storm].duration(severity), 3, "{storm}");
        }
    }
}
