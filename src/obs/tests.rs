use super::*;

/// The three largest arrays of the document are built once and moved in. Named
/// inside `json!` they are serialized a second time, element by element, which
/// was an eighth of the worker's time on a spectated turn.
#[test]
fn the_state_document_moves_its_largest_arrays_in_rather_than_copying_them() {
    let game = Game::new(2, 24, 18, 91_143, 60, 0);
    let observed = observation_spectator(&game, 0);
    assert_eq!(
        observed["map"]["tiles"].as_array().map(Vec::len),
        Some(game.map.tiles.len()),
        "the spectator's document still carries every tile"
    );
    assert_eq!(
        observed["units"].as_array().map(Vec::len),
        Some(game.units.len()),
        "and every unit"
    );
    assert!(observed["cities"].is_array(), "and the city list");

    let source = include_str!("../obs.rs");
    for assignment in [
        "observation[\"map\"][\"tiles\"] = Value::Array(tiles);",
        "observation[\"units\"] = Value::Array(units);",
        "observation[\"cities\"] = Value::Array(cities);",
    ] {
        assert!(source.contains(assignment), "missing {assignment}");
    }
}

/// `Session::state` holds one memo across the whole document, because a frame
/// otherwise derives each city's yields, citizens and amenities again for the
/// city list, the player totals and the victory forecasts: a fifth of a
/// late-game turn. The memo may only ever save work.
#[test]
fn a_memo_held_across_the_document_changes_nothing_in_it() {
    let game = Game::new(3, 32, 20, 77_001, 60, 0);
    let plain = observation_spectator(&game, 0);
    let memo = game.query_memo();
    assert_eq!(observation_spectator(&game, 0), plain);
    drop(memo);
    assert_eq!(
        observation_spectator(&game, 0),
        plain,
        "and it leaves no residue"
    );

    let server = include_str!("../server.rs");
    let state = server
        .split("pub fn state(&self) -> Value {")
        .nth(1)
        .expect("Session::state");
    assert!(
        state
            .trim_start()
            .starts_with("// A frame asks the same questions"),
        "the memo opens before anything is derived"
    );
    assert!(state.contains("let _memo = self.game.query_memo();"));
}

/// Pointing at a unit is a question asked through the same fog the
/// observation was built with, and it has to answer with the same
/// discipline: the omniscient spectator is told everything, a seat is told
/// nothing at all about a unit it cannot see, and what a seat *is* told
/// stops at the edge of the ground it has explored. Without the last of
/// those, the perimeter of a Knight's reach draws the coastline of a
/// continent nobody in the game has sailed to.
#[test]
fn unit_intel_answers_the_spectator_and_keeps_a_seat_inside_its_own_fog() {
    let game = Game::new(2, 24, 18, 91_143, 60, 0);
    let (vis, explored) = visibility(&game, 0);
    let hidden = game
        .units
        .values()
        .find(|unit| unit.owner == 1 && !vis.contains(&unit.pos))
        .expect("a rival unit out of sight on turn one")
        .id;

    let open = unit_intel(&game, None, hidden);
    assert!(
        open["error"].is_null(),
        "the spectator sees the whole board"
    );
    assert_eq!(open["unit"], hidden);
    assert!(
        !open["vision"].as_array().expect("a vision set").is_empty(),
        "a unit standing anywhere can see at least its own tile"
    );

    assert_eq!(
        unit_intel(&game, Some(0), hidden)["error"],
        "that unit is not in view",
        "a seat may not ask about a unit it has not found"
    );

    // Its own unit is fair game, and every tile handed back is ground this
    // seat has actually been to.
    let mine = game
        .units
        .values()
        .find(|unit| unit.owner == 0)
        .expect("a starting unit")
        .id;
    let own = unit_intel(&game, Some(0), mine);
    assert!(own["error"].is_null());
    for field in ["reachable", "vision"] {
        for tile in own[field].as_array().expect("a tile set") {
            let pos = (
                tile[0].as_i64().expect("a column") as i32,
                tile[1].as_i64().expect("a row") as i32,
            );
            assert!(
                explored.contains(&pos),
                "{field} handed back {pos:?}, which this seat has never seen"
            );
        }
    }
}

/// The browser draws a flat chart by unrolling it about the camera, which
/// is right for a world and wrong for an arena: a Tactics battlefield is
/// walled on all four sides, so a unit at the east wall drawn again past
/// the west wall is a unit off the map. `shape` cannot answer it — an
/// arena is flat like every stock script — so the observation carries the
/// wrap itself, read off the built map so a loaded save answers too.
#[test]
fn the_observation_says_whether_east_and_west_are_the_same_edge() {
    let arena = Game::new_with(crate::game::GameOptions {
        map_script: crate::setup::MapScript::Battlefield,
        ..crate::game::GameOptions::new(2, 12, 12, 4_402, 60, 0)
    });
    assert!(arena.is_arena());
    assert_eq!(observation_spectator(&arena, 0)["map"]["shape"], "flat");
    assert_eq!(
        observation_spectator(&arena, 0)["map"]["wrap_x"],
        json!(false),
        "an arena has a wall where a world has a seam"
    );

    // Every other shape does come back on itself: a cylinder through its
    // own east-west seam, a globe through its geometry.
    let world = Game::new_full(2, 20, 14, 4_402, 60, 1, false);
    assert_eq!(
        observation_spectator(&world, 0)["map"]["wrap_x"],
        json!(true)
    );
    let size = crate::setup::MapSize::for_players(2);
    let (width, height) = size.dimensions(crate::setup::MapTopology::Planet);
    let globe = Game::new_with(crate::game::GameOptions {
        map_topology: crate::setup::MapTopology::Planet,
        ..crate::game::GameOptions::new(2, width, height, 4_402, 60, 2)
    });
    assert_eq!(
        observation_spectator(&globe, 0)["map"]["wrap_x"],
        json!(true)
    );
}

#[test]
fn tile_hover_identifies_the_owning_city_and_orders_each_gameplay_layer() {
    let mut game = Game::new_full(2, 20, 14, 19_067, 120, 1, false);
    let capital_position = game
        .player_unit_ids(0)
        .into_iter()
        .find_map(|unit| {
            let unit = &game.units[&unit];
            (unit.kind == "settler").then_some(unit.pos)
        })
        .expect("the player starts with a settler");
    let city_id = game.found_city_for(0, capital_position, None);
    let city = &game.cities[&city_id];
    let tile = &game.map.tiles[&city.pos];
    let tourism = game.tourism_by_tile(0);
    let spectator = revealed_resources(&game, 0, true);
    let seated = revealed_resources(&game, 0, false);
    let live = tile_json(&game, tile, Some(0), &spectator, true, &tourism, None);
    assert_eq!(live["owner_city"], json!(city_id));
    assert_eq!(live["owner_city_name"], json!(city.name));
    assert!(live["tourism"].is_number());

    let remembered = tile_json(&game, tile, Some(0), &seated, false, &tourism, None);
    assert_eq!(remembered["owner_city"], json!(city_id));
    assert!(remembered["tourism"].is_null());
    assert!(
        remembered["owner_city_name"].is_null(),
        "current city names must not leak through a remembered tile"
    );

    game.cities
        .get_mut(&city_id)
        .unwrap()
        .pressure
        .insert("Test Faith".to_string(), 240.0);
    let observed = observation_spectator(&game, 0);
    let observed_city = observed["cities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|candidate| candidate["id"] == city_id)
        .unwrap();
    assert_eq!(observed_city["religious_pressure"]["Test Faith"], 240.0);
    assert!(observed_city["atheist_pressure"].is_number());
    assert!(observed_city["power_demand"].is_number());
    assert!(observed_city["power_supply"].is_number());
    assert!(observed_city["powered"].is_boolean());
    assert!(observed_city["loyalty"].is_number());
    assert!(observed_city["loyalty_per_turn"].is_number());

    // Mixed contract: markup lives in the document, renderer code in its
    // external script - assert the whole page source.
    let index_owned = format!(
        "{}{}",
        include_str!("../../web/index.html"),
        include_str!("../../web/assets/app.js")
    );
    let index: &str = &index_owned;
    assert!(index.contains("id=\"map-controls-dock\""));
    // The compact two-column menu pairs related readings: land and borders,
    // city planning and empire detail, then culture, tile value, and the
    // two city-pressure lenses.
    let toolbar = index
        .split_once("<div id=\"map-lens-strip\"")
        .expect("map lens toolbar")
        .1
        .split_once("</div>")
        .expect("end of map lens toolbar")
        .0;
    let lenses: Vec<&str> = toolbar
        .match_indices("data-map-lens=\"")
        .map(|(at, marker)| {
            let rest = &toolbar[at + marker.len()..];
            &rest[..rest.find('"').expect("lens name is quoted")]
        })
        .collect();
    assert_eq!(
        lenses,
        [
            "continent",
            "political",
            "settler",
            "empire",
            "religion",
            "government",
            "appeal",
            "tourism",
            "loyalty",
            "power",
        ],
        "every manual Civilization VI lens belongs in the toolbar, in reading order"
    );
    for renderer in [
        "function drawReligiousPressureRing(",
        "function drawLoyaltyLensFlat(",
        "function drawLoyaltyLensPlanet(",
        "function drawPowerLensFlat(",
        "function drawPowerLensPlanet(",
        "function drawFlatLensGroupLabels(",
        "function drawPlanetLensGroupLabels(",
        "function drawThematicLensFlat(",
        "function drawThematicLensPlanet(",
        "function drawEmpireDetailsFlat(",
        "function drawEmpireDetailsPlanet(",
    ] {
        assert!(
            index.contains(renderer),
            "the lens toolbar is missing renderer {renderer}"
        );
    }
    // Civilization VI puts the Settler lens on `4` and the whole set on
    // `2` through `0`; this client's roster is that same set, so the keys
    // are that game's too rather than a shorter list of its own.
    assert!(index.contains(
        "{id: \"LensSettler\", key: \"4\", spectator: true, run: () => setMapLens(\"settler\")},"
    ));
    for (id, key, lens) in [
        ("LensContinent", "2", "continent"),
        ("LensAppeal", "3", "appeal"),
        ("LensGovernment", "5", "government"),
        ("LensPolitical", "6", "political"),
        ("LensTourism", "7", "tourism"),
        ("LensLoyalty", "8", "loyalty"),
        ("LensEmpire", "9", "empire"),
        ("LensPower", "0", "power"),
    ] {
        assert!(
                index.contains(&format!(
                    "{{id: \"{id}\", key: \"{key}\", spectator: true, run: () => setMapLens(\"{lens}\")}},"
                )),
                "the {lens} lens must answer Civ 6's own {key}"
            );
    }
    let start = index
        .find("function tileTipLines(t, pos, tileKey)")
        .expect("the tile hover has one ordered builder");
    let end = index[start..]
        .find("\n// tooltip")
        .map(|offset| start + offset)
        .expect("the tile hover builder ends before its event handler");
    let hover = &index[start..end];
    let ordered = [
        "for (const unit of state.units)",
        "lines.push(...tileBuiltTipLines(t, city));",
        "tileOwnershipTipLine(t)",
        "lines.push(tileTerrainTipLine(t));",
        "const resource = tileResourceTipLine(t);",
        "lines.push(...tileBaseYieldLines(t));",
        "const yields = tileTotalYields(t);",
        "const mp = tileMoveCost(t);",
        "const defense = tileDefense(t);",
        "if (t.appeal !== null && t.appeal !== undefined)",
    ];
    let mut previous = 0;
    for marker in ordered {
        let at = hover
            .find(marker)
            .unwrap_or_else(|| panic!("the tile hover is missing {marker}"));
        assert!(
            at >= previous,
            "tile hover layer {marker} is out of the requested order"
        );
        previous = at;
    }
    assert!(index.contains(".tip-primary, .tip-unit"));
    assert!(index.contains("function civPossessive(civ)"));
    assert!(index.contains("function tileTerrainTipLine(t)"));
    assert!(index.contains("function tileBuiltTipLines(t, city)"));
    assert!(index.contains("function tileBaseYieldLines(t)"));
    assert!(index.contains("function tileTotalYields(t)"));
    assert!(index.contains("function tileYieldMarkers(yields)"));
    assert!(hover.contains("${civPossessive(civ)} ${titleCase(unit.type)} - "));
    assert!(hover.contains("Total tile yields: ${yieldWords || \"unavailable\"}"));
    assert!(hover.contains("Movement: ${movement}"));
    assert!(hover.contains("Defense: ${defenseText}"));
    assert!(hover.contains("Appeal: "));
    assert!(!hover.contains("civAdjective("));
    assert!(!hover.contains("Capital: "));
    assert!(!hover.contains("<b>"));
}

#[test]
fn player_cities_name_their_governor_and_spectator_cities_do_not() {
    let mut game = Game::new(2, 18, 12, 70_126, 25, 0);
    let capital_position = game
        .player_unit_ids(0)
        .into_iter()
        .find_map(|unit| {
            let unit = &game.units[&unit];
            (unit.kind == "settler").then_some(unit.pos)
        })
        .expect("the player starts with a settler");
    let city_id = game.found_city_for(0, capital_position, Some("Academia".to_string()));
    game.players[0].civics.insert(crate::name!("early_empire"));
    game.apply(
        0,
        &crate::game::Action::AppointGovernor {
            governor: crate::name!("pingala"),
            city: city_id,
        },
    )
    .expect("the earned title appoints Pingala");

    let active_turn =
        game.turn + game.standard_duration(game.rules.governors["pingala"].establish_turns);
    let observed = observation(&game, 0);
    let city = observed["cities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|city| city["id"] == json!(city_id))
        .unwrap();
    assert_eq!(city["governor_assignment"]["id"], json!("pingala"));
    assert_eq!(city["governor_assignment"]["name"], json!("Pingala"));
    assert_eq!(city["governor_assignment"]["title"], json!("The Educator"));
    assert_eq!(city["governor_assignment"]["status"], json!("establishing"));
    assert_eq!(
        city["governor_assignment"]["turns_remaining"],
        json!(active_turn - game.turn)
    );
    assert_eq!(city["governor_assignment"]["established"], json!(false));

    let watched = observation_player_view(&game, 0);
    let watched_city = watched["cities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|city| city["id"] == json!(city_id))
        .unwrap();
    assert_eq!(
        watched_city["governor_assignment"]["name"],
        json!("Pingala"),
        "a read-only player perspective retains that player's private posting"
    );

    let spectated = observation_spectator(&game, 0);
    let spectator_city = spectated["cities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|city| city["id"] == json!(city_id))
        .unwrap();
    assert!(
        spectator_city.get("governor_assignment").is_none(),
        "omniscient spectator city records stay free of player-only Governor labels"
    );

    game.turn = active_turn;
    let established = observation(&game, 0);
    let established_city = established["cities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|city| city["id"] == json!(city_id))
        .unwrap();
    assert_eq!(
        established_city["governor_assignment"]["status"],
        json!("established")
    );
    assert_eq!(
        established_city["governor_assignment"]["turns_remaining"],
        json!(0)
    );
    assert_eq!(
        established_city["governor_assignment"]["established"],
        json!(true)
    );

    const INDEX: &str = include_str!("../../web/assets/app.js");
    assert!(INDEX.contains("function cityGovernor(city)"));
    assert!(INDEX.contains("if (!state || SPEC || !city) return null;"));
    assert!(INDEX.contains("const posting = `⚑ ${governor.name.toUpperCase()}${waiting}`;"));
    assert!(INDEX
        .contains("`<b>${governor.name} · ${governor.title} · ${governorStatus(governor)}</b>"));
    assert!(INDEX.contains("const mine = posting !== undefined;"));
    assert!(INDEX.contains("posting.city === null ? \"Awaiting a city assignment.<br>\""));
}

/// The player HUD and the victory tracker are drawn from `players`, so an
/// empire nobody has met has to be missing from it rather than merely
/// skipped by the browser. What is left is the seat's identity — the id
/// and civ its cities fly on the map — and nothing anyone could rank it by.
#[test]
fn an_unmet_civilization_reaches_the_wire_as_identity_and_nothing_else() {
    let mut game = Game::new(2, 18, 12, 74_115, 25, 0);
    for pid in 0..2 {
        game.players[pid].met.clear();
    }
    let hidden = &observation(&game, 0)["players"][1];
    assert_eq!(hidden["met"], json!(false));
    assert_eq!(hidden["civ"], json!(game.players[1].civ));
    for withheld in [
        "score",
        "victories",
        "yields",
        "military",
        "cities",
        "population",
        "gold",
        "leader",
        "agenda",
        "government",
        "age",
        "wonder_count",
        "nuclear_devices",
        "thermonuclear_devices",
        "opinion_of_me",
    ] {
        assert!(
            hidden[withheld].is_null(),
            "an unmet civilization must not report {withheld}"
        );
    }
    // The viewer is always on its own ledger, and so is the omniscient
    // spectator's whole table.
    assert_eq!(observation(&game, 0)["players"][0]["met"], json!(true));
    let spectated = observation_spectator(&game, 0);
    assert_eq!(spectated["players"][1]["met"], json!(true));
    assert!(spectated["players"][1]["score"].is_number());

    game.record_contact(0, 1);
    let found = &observation(&game, 0)["players"][1];
    assert_eq!(found["met"], json!(true));
    assert!(
        found["score"].is_number(),
        "the dashboard arrives on contact"
    );
    assert!(!found["victories"].is_null());
    assert_eq!(
        observation(&game, 1)["players"][0]["met"],
        json!(true),
        "and the meeting was mutual"
    );
}

/// The standings' NUK column counts finished devices of both classes, so
/// the two stockpile counters have to reach the wire separately and unmixed
/// with the Manhattan Project that unlocked them. Whether the bomb exists
/// at all is the world's own fact beside `world_era`: nobody can hold a
/// device before somebody researches nuclear fission, and no seat learns
/// from it who that somebody was.
#[test]
fn the_world_reports_when_nuclear_weapons_have_been_unlocked() {
    let mut game = Game::new_full(3, 22, 16, 74_118, 25, 1, false);
    assert_eq!(
        observation_spectator(&game, 0)["nuclear_weapons_unlocked"],
        json!(false),
        "an ancient world has not invented the bomb"
    );
    assert_eq!(
        observation(&game, 1)["nuclear_weapons_unlocked"],
        json!(false)
    );

    game.players[2]
        .techs
        .insert(crate::name!("nuclear_fission"));
    for observed in [observation_spectator(&game, 0), observation(&game, 1)] {
        assert_eq!(
            observed["nuclear_weapons_unlocked"],
            json!(true),
            "the whole world is in the nuclear age once one civilization is"
        );
        assert!(
            observed["players"][2]["techs"].is_null(),
            "and it still says nothing about whose tech tree it came from"
        );
    }

    // History does not un-happen: losing the empire that invented the bomb
    // does not un-invent it.
    game.players[2].alive = false;
    assert_eq!(
        observation_spectator(&game, 0)["nuclear_weapons_unlocked"],
        json!(true)
    );
}

/// Both classes of device are counted, apart, for every civilization the
/// viewer has met — the same public ledger military strength rides on.
#[test]
fn player_standings_report_each_civilizations_nuclear_stockpile() {
    let mut game = Game::new_full(3, 22, 16, 74_117, 25, 1, false);
    game.players[0]
        .science_projects
        .insert("manhattan_project".to_string());
    game.players[0]
        .counters
        .insert("project_effect:nuclear_devices".to_string(), 3);
    game.players[0]
        .counters
        .insert("project_effect:thermonuclear_devices".to_string(), 2);

    let observed = observation_spectator(&game, 0);
    let armed = &observed["players"][0];
    assert_eq!(armed["nuclear_devices"], json!(3));
    assert_eq!(armed["thermonuclear_devices"], json!(2));
    assert!(
        armed["science_projects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|project| project == "manhattan_project"),
        "the project that unlocks the column is named on the same ledger"
    );
    // Everybody else is on the same ledger with an explicit nothing, so a
    // disarmed row reads as a zero rather than as an unavailable figure.
    let disarmed = &observed["players"][1];
    assert_eq!(disarmed["nuclear_devices"], json!(0));
    assert_eq!(disarmed["thermonuclear_devices"], json!(0));

    const APP: &str = include_str!("../../web/assets/app.js");
    assert!(
        APP.contains("[\"nukes\", \"Nuclear stockpile\", worldStandingsInPlay],"),
        "the column stands from turn one wherever there is an empire to arm"
    );
    assert!(APP.contains("nukes:[\"NUK\", \"Nuclear devices held\"],"));
}

/// Ages belong to civilizations rather than to the observing seat. Once a
/// civilization is on the public standings ledger, its current age must be
/// present there so every row can render the AGE column independently.
#[test]
fn player_standings_report_each_civilizations_age() {
    let mut game = Game::new_full(4, 22, 16, 74_116, 25, 1, false);
    let expected = ["heroic", "golden", "normal", "dark"];
    for (player, age) in game
        .players
        .iter_mut()
        .filter(|player| !player.is_minor && !player.is_barbarian)
        .zip(expected)
    {
        player.age = age.to_string();
    }

    let observed = observation_spectator(&game, 0);
    let ages = observed["players"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|player| {
            !player["is_minor"].as_bool().unwrap_or(false)
                && !player["is_barbarian"].as_bool().unwrap_or(false)
        })
        .map(|player| player["age"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ages, expected);

    const INDEX: &str = include_str!("../../web/assets/app.js");
    let age_column = INDEX.find("{key:\"age\", label:\"Age\"").unwrap();
    let closest_column = INDEX
        .find("{key:\"closest_victory\", label:\"Closest victory\"")
        .unwrap();
    let next_column = INDEX[closest_column + 1..].find("{key:\"").unwrap() + closest_column + 1;
    assert_eq!(
        next_column, age_column,
        "the resizable AGE column belongs immediately after CLOSEST"
    );
    assert!(INDEX.contains("age:[\"AGE\", \"Civilization age\"],"));
    assert!(INDEX.contains("diplomacy-identity-field diplomacy-age"));
}

/// The viewer starts a world facing wherever it was found and only puts
/// north at the top once the civilization it is watching can find north, so
/// the observation has to say. A spectator is above the world rather than in
/// it and always can; a civilization has to research it.
#[test]
fn finding_north_is_a_technology_for_a_civilization_and_free_for_a_spectator() {
    let mut game = Game::new(2, 18, 12, 11, 25, 0);
    assert!(
        game.rules.techs.get(NORTH_TECH).is_some(),
        "{NORTH_TECH} must name a technology in the shipped tree",
    );
    game.players[0].techs.remove(&Name::new(NORTH_TECH));

    let own = observation(&game, 0);
    assert_eq!(own["me"]["found_north"], json!(false));
    assert_eq!(own["me"]["north_tech"], json!(NORTH_TECH));
    // Watching one civilization's own view is that civilization's knowledge.
    assert_eq!(
        observation_player_view(&game, 0)["me"]["found_north"],
        json!(false),
    );
    assert_eq!(
        observation_spectator(&game, 0)["me"]["found_north"],
        json!(true),
    );

    game.players[0].techs.insert(Name::new(NORTH_TECH));
    assert_eq!(observation(&game, 0)["me"]["found_north"], json!(true));
    assert_eq!(
        observation_player_view(&game, 0)["me"]["found_north"],
        json!(true),
    );
}

/// The other half of a world's bearing, and all of its extent: a people
/// know their world comes back on itself once their own knowledge has run
/// the whole way round it. A spectator is above the world and is told at
/// once; a civilization has to go.
#[test]
fn going_the_whole_way_round_is_earned_by_a_civilization_and_free_for_a_spectator() {
    let mut game = Game::new(2, 18, 12, 11, 25, 0);

    let own = observation(&game, 0);
    assert_eq!(own["me"]["went_around"], json!(false));
    assert_eq!(
        observation_player_view(&game, 0)["me"]["went_around"],
        json!(false),
    );
    assert_eq!(
        observation_spectator(&game, 0)["me"]["went_around"],
        json!(true),
    );

    game.players[0].went_around = true;
    assert_eq!(observation(&game, 0)["me"]["went_around"], json!(true));
    assert_eq!(
        observation_player_view(&game, 0)["me"]["went_around"],
        json!(true),
    );
}

/// Sailing round the world is a proof that it is round. A people who set
/// out west and came home from the east have settled the question on the
/// water, without anyone having to read it in a book first.
#[test]
fn a_lap_of_the_world_proves_it_round_without_the_technology() {
    let mut game = Game::new(2, 18, 12, 11, 25, 0);
    for tech in GLOBE_TECHS {
        game.players[0].techs.remove(&Name::new(tech));
    }
    game.players[0].great_people.clear();
    game.players[0].went_around = false;
    assert_eq!(observation(&game, 0)["me"]["knows_globe"], json!(false));

    game.players[0].went_around = true;
    assert_eq!(observation(&game, 0)["me"]["knows_globe"], json!(true));
    // Still only the world they have been round: the far end of the system
    // waits for an instrument above the air either way.
    assert_eq!(observation(&game, 0)["me"]["sees_exoplanet"], json!(false));
}

/// The shape of the world is the same kind of fact as which way is north:
/// the viewer has to be told, because it draws a globe a people who have not
/// worked it out yet are not supposed to be looking at. Either proof opens
/// it, the great person opens it early, and the far end of the system waits
/// for an eye above the air.
#[test]
fn a_round_world_is_a_discovery_and_the_far_planet_waits_for_a_satellite() {
    let mut game = Game::new(2, 18, 12, 11, 25, 0);
    for tech in GLOBE_TECHS {
        assert!(
            game.rules.techs.get(tech).is_some(),
            "{tech} must name a technology in the shipped tree",
        );
        game.players[0].techs.remove(&Name::new(tech));
    }
    assert!(
        game.rules.great_people.get(GLOBE_GREAT_PERSON).is_some(),
        "{GLOBE_GREAT_PERSON} must name a great person in the shipped roster",
    );
    game.players[0].great_people.clear();
    game.players[0].science_projects.remove(EXOPLANET_EYE);

    let own = observation(&game, 0);
    assert_eq!(own["me"]["knows_globe"], json!(false));
    assert_eq!(own["me"]["sees_exoplanet"], json!(false));
    assert_eq!(own["me"]["globe_techs"], json!(GLOBE_TECHS));
    assert_eq!(own["me"]["globe_great_person"], json!(GLOBE_GREAT_PERSON));
    assert_eq!(own["me"]["exoplanet_eye"], json!(EXOPLANET_EYE));
    assert_eq!(
        observation_player_view(&game, 0)["me"]["knows_globe"],
        json!(false),
    );
    // A spectator is above the world and has never had to prove anything
    // about it, so the whole system is open from the first turn.
    let watching = observation_spectator(&game, 0);
    assert_eq!(watching["me"]["knows_globe"], json!(true));
    assert_eq!(watching["me"]["sees_exoplanet"], json!(true));

    // Each road on its own is enough.
    for tech in GLOBE_TECHS {
        game.players[0].techs.insert(Name::new(tech));
        assert_eq!(observation(&game, 0)["me"]["knows_globe"], json!(true));
        game.players[0].techs.remove(&Name::new(tech));
    }
    game.players[0]
        .great_people
        .push(GLOBE_GREAT_PERSON.to_string());
    assert_eq!(observation(&game, 0)["me"]["knows_globe"], json!(true));

    // The other star stays out of reach until something has actually been
    // put up there to see it with.
    assert_eq!(observation(&game, 0)["me"]["sees_exoplanet"], json!(false));
    game.players[0]
        .science_projects
        .insert(EXOPLANET_EYE.to_string());
    assert_eq!(observation(&game, 0)["me"]["sees_exoplanet"], json!(true));
}

/// The rung between the round world and the space age.
///
/// The five wandering stars are naked-eye objects — Mercury, Venus, Mars,
/// Jupiter and Saturn have been in every sky anybody ever looked at — so a
/// people who know their world is a ball can place all five. Everything
/// past Saturn arrived with the telescope, and it arrived as one piece:
/// Uranus in 1781, Ceres in 1801, Neptune in 1846 by prediction, and in
/// 1838 the first measured distance to another star. The gate hands over
/// the outer system and a neighbourhood with real distances in it together,
/// because that is when both of them turned up.
#[test]
fn the_outer_system_waits_for_the_instrument_that_found_it() {
    let mut game = Game::new(2, 18, 12, 5_517, 25, 0);

    // A people who cannot place their own world round have no system to put
    // an outer planet in, so the discovery on its own is not enough.
    game.players[0]
        .techs
        .insert(Name::new(OUTER_SYSTEM_TECHS[0]));
    assert_eq!(observation(&game, 0)["me"]["knows_globe"], json!(false));
    assert_eq!(
        observation(&game, 0)["me"]["sees_outer_system"],
        json!(false),
    );

    // With the round world proved, the same discovery opens it.
    game.players[0].went_around = true;
    assert_eq!(
        observation(&game, 0)["me"]["sees_outer_system"],
        json!(true),
    );

    // Newton is the recruit who does it without the discovery, for the same
    // reason Hypatia opens the globe: the reflecting telescope every one of
    // those findings was made with a descendant of is his.
    game.players[0]
        .techs
        .remove(&Name::new(OUTER_SYSTEM_TECHS[0]));
    assert_eq!(
        observation(&game, 0)["me"]["sees_outer_system"],
        json!(false),
    );
    game.players[0]
        .great_people
        .push(OUTER_SYSTEM_GREAT_PERSON.to_string());
    assert_eq!(
        observation(&game, 0)["me"]["sees_outer_system"],
        json!(true),
    );

    // A civilization that skipped straight to putting an eye above the air
    // is not sent back down a rung for it. The ladder only ever climbs.
    let mut leaper = Game::new(2, 18, 12, 5_519, 25, 0);
    leaper.players[0].went_around = true;
    leaper.players[0]
        .science_projects
        .insert(EXOPLANET_EYE.to_string());
    let seen = observation(&leaper, 0);
    assert_eq!(seen["me"]["sees_exoplanet"], json!(true));
    assert_eq!(seen["me"]["sees_outer_system"], json!(true));

    // And a spectator was never the party in the dark about any of it.
    let watching = observation_spectator(&game, 1);
    assert_eq!(watching["me"]["sees_outer_system"], json!(true));
    assert_eq!(
        observation_player_view(&game, 1)["me"]["sees_outer_system"],
        json!(false),
    );

    // The client is told which discovery it was, so it does not have to
    // invent the sentence naming it.
    let own = observation(&game, 0);
    assert_eq!(own["me"]["outer_system_techs"], json!(OUTER_SYSTEM_TECHS));
    assert_eq!(
        own["me"]["outer_system_great_person"],
        json!(OUTER_SYSTEM_GREAT_PERSON),
    );
}

/// A launch is a fact about the world, not about the shape the world is
/// drawn in, so the same craft belongs over a flat board as over the globe.
/// A sheet of paper has no limb for it to pass behind, so what a flat map
/// draws is the ground track: the line directly under the craft, laid a
/// little further west on every pass because the world turned underneath
/// while the craft went round. That westward term is the whole difference
/// between an orbit and a wave scrolling across a chart, so it is the part
/// worth pinning.
#[test]
fn a_launched_satellite_crosses_a_flat_board_as_a_ground_track() {
    let mut game = Game::new(2, 18, 12, 4_412, 25, 0);
    assert!(
        game.rules.projects.contains_key(EXOPLANET_EYE),
        "{EXOPLANET_EYE} must name a shipped project",
    );
    for player in game.players.iter_mut() {
        player.science_projects.clear();
    }
    game.record_contact(0, 1);
    let projects = |observed: &Value, pid: usize| -> Vec<String> {
        observed["players"][pid]["science_projects"]
            .as_array()
            .expect("a met civilization reports what it has finished")
            .iter()
            .map(|project| project.as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    assert!(projects(&observation(&game, 1), 0).is_empty());
    game.players[0]
        .science_projects
        .insert(EXOPLANET_EYE.to_string());
    // The neighbour sees whose satellite it is, which is what colours the
    // track. An unmet civilization reports nothing at all and so has no
    // craft drawn for it; that contract has its own test.
    assert_eq!(
        projects(&observation(&game, 1), 0),
        vec![EXOPLANET_EYE.to_string()],
    );

    const INDEX: &str = include_str!("../../web/assets/app.js");
    assert!(INDEX.contains("satellite:\"launch_earth_satellite\","));
    // One orbit per civilization, in the world's own frame, so the globe
    // and the flat board draw the same launch rather than two of them.
    assert!(INDEX.contains("function skyOrbit(player) {"));
    assert!(INDEX.contains("const SKY_SATELLITE_ALTITUDE_SCALE = 1.7;"));
    assert!(INDEX.contains(
        "function skySatelliteOrbit(player, index, radius, elements = skyOrbit(player)) {"
    ));
    assert!(INDEX.contains("const orbit = skyOrbit(player);"));
    // The ground track itself: the orbit's own latitude and longitude, less
    // the turn the world made under it.
    assert!(INDEX.contains("const FLAT_SAT_DRIFT = .1;"));
    assert!(INDEX.contains("const FLAT_SAT_TAIL_SCALE = .3;"));
    assert!(INDEX.contains("const FLAT_SAT_HORIZON_RADIUS = .024;"));
    assert!(INDEX.contains("function flatSatelliteGround(orbit, theta) {"));
    assert!(
        INDEX.contains("- FLAT_SAT_DRIFT * theta;"),
        "a ground track without the world's own turn under it is a sine wave",
    );
    assert!(INDEX.contains("track(-1.35 * FLAT_SAT_TAIL_SCALE, 0, 1, .2, false);"));
    assert!(INDEX.contains("track(-.38 * FLAT_SAT_TAIL_SCALE, 0, 1.4, .45, false);"));
    assert!(INDEX.contains("const horizon = WW() * FLAT_SAT_HORIZON_RADIUS;"));
    // Overhead is only in the picture once the camera is off the ground,
    // and the board keeps painting while a craft is up there — a strategic
    // map is otherwise perfectly still between turns.
    assert!(INDEX.contains("if (!state || planetMap()) return 0;"));
    assert!(INDEX.contains("return Math.max(0, Math.min(1, (.86 - cam.scale) / .34));"));
    assert!(INDEX.contains("flatSkyShown() > .02 && skyCrews().satellite.length > 0"));
    assert!(INDEX.contains("|| planetSkyAnimating() || flatSkyAnimating();"));
    assert!(INDEX.contains("  drawFlatSatellites(now0);\n  drawFlatLaunches(now0);"));
}

#[test]
fn space_race_launches_use_mission_specific_rockets_on_both_maps() {
    // Mixed contract: markup lives in the document, renderer code in its
    // external script - assert the whole page source.
    let index_owned = format!(
        "{}{}",
        include_str!("../../web/index.html"),
        include_str!("../../web/assets/app.js")
    );
    let index: &str = &index_owned;

    // Apollo 11 flew Saturn V, Mars gets the Starship silhouette, and the
    // exoplanet mission gets the same silhouette at exactly twice the
    // scale — doubled uniformly, with a matching double-size plume, so the
    // monumental ship keeps rocket proportions instead of reading as an
    // egg with fins.
    assert!(index.contains(
        "moon:{project:SKY_CHAIN.moon, duration:2800, body:\"moon\", rocket:\"saturn_v\"}"
    ));
    assert!(index.contains(
        "mars:{project:SKY_CHAIN.mars, duration:3600, body:\"mars\", rocket:\"starship\"}"
    ));
    assert!(index.contains(
            "expedition:{project:SKY_CHAIN.expedition, duration:3000, body:\"exo\", rocket:\"starship_xl\"}"
        ));
    assert!(index.contains("function drawMissionRocket(rocket, player, size, now"));
    assert!(index.contains("length:starshipLength * 2"));
    assert!(index.contains("halfWidth:starshipHalfWidth * 2, plume:2,"));
    assert!(index.contains("const flame = size * (shape.plume || 1);"));

    // Spaceflight visuals default to visible, retain a viewer preference,
    // and treat the perpetual satellite orbit like a rocket animation.
    assert!(index.contains(
            "<input type=\"checkbox\" id=\"rocketanimchk\" checked> Show rocket &amp; satellite animations"
        ));
    assert!(index.contains(
            "let SHOW_ROCKET_ANIMATIONS = localStorage.getItem(\"civvis-show-rocket-animations\") !== \"0\";"
        ));
    assert!(index.contains("function setShowRocketAnimations(on)"));
    assert!(index.contains("if (!SHOW_ROCKET_ANIMATIONS) anim.skyLaunches.length = 0;"));
    assert!(index.contains("rockets.onchange = () => setShowRocketAnimations(rockets.checked);"));
    assert!(index.contains(
            "function drawSkySatellites(crew, camera, radius, centerX, centerY, alpha, now) {\n  if (!SHOW_ROCKET_ANIMATIONS) return;"
        ));
    assert!(index.contains("cx.clip(\"evenodd\");"));
    assert!(index
        .contains("function drawFlatSatellites(now) {\n  if (!SHOW_ROCKET_ANIMATIONS) return;"));

    // State-diff detection is shared, then each projection owns a flight
    // path while both call the same rocket-art renderer.
    assert!(index.contains("function queueSkyLaunches(prev, next"));
    assert!(index.contains("if (!SHOW_ROCKET_ANIMATIONS || !prev || !next"));
    assert!(index.contains("function drawSkyLaunches(crews, placements"));
    assert!(index.contains("function flatLaunchRoute(launch, player)"));
    assert!(index.contains("function drawFlatLaunches(now)"));
    assert!(index.contains("drawMissionRocket(flight.rocket, player, size, now, pixel, fade);"));
    assert!(index.contains(
            "drawFlatSatellites(now0);\n  drawFlatLaunches(now0);\n  drawMissileStrikes(now0);\n  drawNuclearBlasts(now0);"
        ));
    assert!(index.contains(
            "return SHOW_ROCKET_ANIMATIONS && (activeSkyLaunches().length > 0 ||\n    (flatSkyShown() > .02"
        ));
}

/// The simulation is allowed to finish many turns while one mission
/// rocket is still on screen. Its route therefore belongs to the frame in
/// which the project completed, while only its progress along that frozen
/// route belongs to the wall clock. In particular, the Exoplanet
/// Expedition starts accumulating light-years on the following turn; using
/// that live distance as the temporary rocket's endpoint pulls the launch
/// off-screen at fast spectator paces even though its timer is still live.
#[test]
fn space_race_launches_outlive_fast_turn_boundaries() {
    // Mixed contract: markup lives in the document, renderer code in its
    // external script - assert the whole page source.
    let index_owned = format!(
        "{}{}",
        include_str!("../../web/index.html"),
        include_str!("../../web/assets/app.js")
    );
    let index: &str = &index_owned;

    let queue = index
        .split_once("function queueSkyLaunches(")
        .unwrap()
        .1
        .split_once("function activeSkyLaunches(")
        .unwrap()
        .0;
    assert!(queue.contains("endsAt:now + flight.duration"));
    assert!(queue.contains("originPos:Array.isArray(origin) ? [...origin] : null"));
    assert!(queue.contains("crew:launchCrew"));
    assert!(queue.contains("expeditionProgress:Math.max(0, Math.min(1,"));
    // The rocket leaves from the Spaceport that ran the project. Only an
    // empire with no visible Spaceport falls back to its anchor city.
    assert!(
        queue.contains("const origin = skyLaunchOrigin(prev, next, player.id, flight.project) ||")
    );
    assert!(index.contains("function citySpaceportPos(city)"));
    assert!(index.contains("RULES?.districts?.[district]?.replaces !== \"spaceport\""));
    assert!(index.contains("owned(prev).find(city => (city.queue || [])[0]?.project === project)"));

    let lifetime = index
        .split_once("function activeSkyLaunches(")
        .unwrap()
        .1
        .split_once("function skyLaunchInFlight(")
        .unwrap()
        .0;
    assert!(lifetime.contains("if (now >= endsAt)"));
    assert!(
        !lifetime.contains(".turn"),
        "a wall-clock launch lifetime must not expire at a turn boundary"
    );

    assert!(index.contains("place, radius, centerX, centerY, launch.expeditionProgress"));
    assert!(index.contains(
        "const anchor = launch.originPos ? {pos:launch.originPos} : playerAnchor(player.id);"
    ));
    assert!(index.contains("const player = launch.player ||"));
}

/// Keeping the launch alive across turns is not enough if nobody can see it.
/// From a close-up Earth the true route runs thousands to millions of pixels
/// off the frame, so the rocket left the picture almost at once and the next
/// turn looked as though it had cut the launch off. The film is played over
/// the part of the route in view.
#[test]
fn a_launch_film_plays_over_the_part_of_its_route_in_view() {
    let app = include_str!("../../web/assets/app.js");

    let painter = app
        .split_once("function drawSkyLaunches(")
        .unwrap()
        .1
        .split_once("function drawSkyCaption(")
        .unwrap()
        .0;
    assert!(painter.contains("const film = skyGlobeFlight("));
    assert!(painter.contains("const at = pointAt(progress);"));
    assert!(app.contains("const film = skyLaunchFilm(start,"));
    assert!(
        !painter.contains("skyOnStage("),
        "the film is already fitted to the stage; culling it again drops its ending"
    );
    // Neither the ignition nor the arrival may be drawn at a point where the
    // frame merely cut the route.
    assert!(painter.contains("clock < .3 && !film.entering"));
    assert!(painter.contains("clock > .82 && !film.leaving"));

    let film = app
        .split_once("function skyLaunchFilm(")
        .unwrap()
        .1
        .split_once("function skyRocketGeometry(")
        .unwrap()
        .0;
    // A route wholly in the picture keeps its true endpoints however short it
    // is: at Solar System zoom the Moon is three pixels from the Earth.
    assert!(film.contains("(!seen.entering && !seen.leaving)"));
}

/// Every launch says whose it is, and a satellite launch ends as the craft the
/// standing orbit is already drawing. It has nothing to land on, so the two
/// have to meet: same place, same velocity, one fading as the other appears.
#[test]
fn launches_name_their_civ_and_a_satellite_hands_over_to_its_orbit() {
    let app = include_str!("../../web/assets/app.js");

    // One label, painted by every map. The old mission-only text is gone.
    assert!(app.contains("return `${civName(launch.playerId)} ${goal}`.toUpperCase();"));
    assert!(app.contains("drawSkyLaunchLabel(launch, at.x, at.y"));
    assert!(!app.contains("\"EXOPLANET LAUNCH\""));

    assert!(
        app.contains("satellite:{project:SKY_CHAIN.satellite, duration:3200, rocket:\"orbital\"}")
    );

    // The range flown reaches the orbit's own rate exactly at the end, while
    // the height has already levelled off, so the velocity carries over.
    let ascent = app
        .split_once("function skySatelliteClimb(")
        .unwrap()
        .1
        .split_once("function skySatellitePlan(")
        .unwrap()
        .0;
    assert!(ascent.contains("const early = 3 * range - sweep, late = sweep - 2 * range;"));
    assert!(ascent.contains("from + early * p * p + late * p * p * p"));

    // The orbit a launch settles belongs to the world it was flown in.
    assert!(app.contains("...anim.skySettledOrbits.get(player.id),"));
    assert!(app.contains("anim.skySettledOrbits.clear();"));

    // Both maps hold the craft back while its rocket flies, and the rocket
    // fades over the same stretch, so neither is ever missing or doubled.
    assert!(app.contains("deployed:skySatelliteDeployed(player.id, now)"));
    assert!(app.contains("deployed = skySatelliteDeployed(player.id, now);"));
    assert!(app.contains(
        "launch.mission === \"satellite\" ? 1 - skySatelliteDeployed(player.id, now)\n    : 1 - skySmooth((clock - .9) / .1);"
    ));
}

/// A launch is a flight through the world rather than a line drawn on the
/// picture: it leaves the real Spaceport, goes east the way the planet turns,
/// and is hidden by the planet for as long as the planet is in the way. Whoever
/// is watching still gets to see it, from any camera.
#[test]
fn launches_climb_from_their_spaceports_and_come_round_the_planet() {
    let app = include_str!("../../web/assets/app.js");

    // From the pad, and east.
    assert!(
        app.contains("const cell = launch.originPos && PLANET?.cells.get(key(launch.originPos));")
    );
    assert!(app.contains("across > 1e-6 ? [-pad[1] / across, pad[0] / across, 0] : [1, 0, 0]"));
    assert!(app.contains("const arc = skyPitchArc(1.46, 1.02 - order * .13);"));

    // A turning world is planned for, and the route is in its frame.
    assert!(app.contains("camera.longitude - 2 * Math.PI * ms / WORLD_SPIN_PERIOD_MS"));

    // A satellite's orbit is swung round to pass over the pad it went up from.
    assert!(app.contains(
        "Math.atan2(pad[1], pad[0]) -\n      Math.atan2(Math.sin(from) * Math.cos(inclination), Math.cos(from));"
    ));

    // The planet hides what is behind it, cutting a rocket at the limb.
    assert!(
        app.contains("point.z < 0 &&\n    Math.hypot(point.x - limb.x, point.y - limb.y) < limb.r")
    );
    assert!(app.contains("if (limb && at.z < 0) {"));

    // Nobody misses one: a film opens where the rocket comes into sight, a
    // launch nowhere in the picture crosses it, and a chart still shows them.
    assert!(app.contains("const law = skyThrustLaw(from, from > 0 ? SKY_ROLLING_START : 0);"));
    assert!(app.contains("launch.route = {kind:\"crossing\"};"));
    assert!(app.contains("if (camera.chart) drawChartLaunches(camera, radius, centerX, centerY);"));
}

/// What makes a launch look wrong is motion that does not add up: a rocket that
/// hooks back across the planet, stalls in mid-air, jumps, or blinks out. Each
/// of those came from a specific shape of code, and these pin the shape that
/// replaced it. A launch has one curve and one clock, settled once.
#[test]
fn a_launch_flies_one_curve_on_one_clock() {
    let app = include_str!("../../web/assets/app.js");

    let painter = app
        .split_once("function drawLaunchFlight(")
        .unwrap()
        .1
        .split_once("// A body nobody could name is just a light.")
        .unwrap()
        .0;
    // One clock, read through one smooth law. The piecewise schedules that made
    // a far-side rocket sprint, stall and resume are gone.
    assert!(painter.contains("const progress = film.law(clock);"));
    assert!(!app.contains("function skyLaunchProgress("));
    assert!(!app.contains("launch.haste"));
    assert!(!app.contains("launch.emerges"));

    // A rocket leaving the picture is still gathering speed when it goes.
    assert!(app.contains(
        "return from + (1 - from) * (rolling * t + (1 - rolling) * Math.pow(t, SKY_THRUST_POWER));"
    ));

    // One arc, which an orthographic camera can only turn into another arc.
    assert!(app.contains("function skyPitchArc(pitchFrom, pitchTo) {"));
    assert!(app.contains("const pitch = pitchFrom + turn * progress;"));
    assert!(app.contains("pad[axis] * (1 + reach * up) + heading[axis] * reach * along),"));
    assert!(
        !app.contains("x:climb.x + (end.x - climb.x) * bent"),
        "blending a world climb toward a point on the screen is what drew the hooks"
    );

    // Planned on the first frame and only read after that.
    let flight = app
        .split_once("function skyGlobeFlight(")
        .unwrap()
        .1
        .split_once("function drawSkyLaunches(")
        .unwrap()
        .0;
    assert!(flight.contains("if (!launch.route) {"));
    assert_eq!(flight.matches("skyPlanMission(").count(), 1);

    // Never stops in the frame: what is left at the end of its film goes out.
    assert!(painter.contains(": 1 - skySmooth((clock - .9) / .1);"));

    // A crossing goes through the picture, not along the edge of it.
    assert!(app.contains("const through = Math.atan2(-inY, -inX);"));

    // A satellite is a gravity turn that cannot overrun its orbit and brake.
    assert!(app.contains("const steep = Math.log(orbit.ring) /"));
    assert!(app.contains("for (let share = .5; share <= 2 / 3 + 1e-6; share += 1 / 24) {"));

    // A nose pointed at the camera has no steady direction: it turns, it
    // does not flip.
    assert!(painter.contains("launch.nose += Math.max(-most, Math.min(most, turn));"));
}

#[test]
fn the_spectator_feed_trades_per_item_research_for_era_firsts() {
    let mut game = Game::new(2, 18, 12, 7, 25, 0);
    game.note(0, "Science", "researched pottery", None);
    game.note(0, "Culture", "adopted code of laws", None);
    game.note(0, "War", "declared war on somebody", None);

    let categories = |observed: &Value| -> Vec<String> {
        observed["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| event["category"].as_str().unwrap().to_string())
            .collect()
    };

    let spectator = categories(&observation_spectator(&game, 0));
    assert!(!spectator.iter().any(|category| category == "Science"));
    assert!(!spectator.iter().any(|category| category == "Culture"));
    assert!(spectator.iter().any(|category| category == "War"));

    let personal = categories(&observation(&game, 0));
    assert!(personal.iter().any(|category| category == "Science"));
    assert!(personal.iter().any(|category| category == "Culture"));
}

/// Every entry says whose it is.
///
/// The omniscient feed rotates through the seats, so the combined log a
/// spectator reads is fed from all of them and "the civilization being
/// observed" is a different answer on the next frame. Without this the
/// browser's event log can show an entry it cannot attribute, and its civ
/// filter hides an entry that plainly names the civilization in its text.
#[test]
fn an_event_on_the_wire_names_the_civilization_it_belongs_to() {
    let mut game = Game::new(3, 18, 12, 7, 25, 0);
    game.note(1, "War", "declared war on somebody", None);
    for observed in [observation(&game, 1), observation_spectator(&game, 1)] {
        let events = observed["events"].as_array().unwrap();
        let mine = events
            .iter()
            .find(|event| event["category"] == "War")
            .expect("the war note");
        assert_eq!(mine["player"], 1);
    }
}

/// The browser draws movement tails straight off this field, so the wire
/// shape is pinned here: the spectator reads every walked route whole,
/// while a seated view keeps its own routes and sees everyone else's only
/// where its fog currently lifts — an enemy walk that dips out of sight
/// arrives as the visible stretch alone, and a single glimpsed tile is no
/// route at all.
#[test]
fn move_trails_reach_the_spectator_whole_and_a_seat_through_its_fog() {
    let mut game = Game::new(2, 18, 12, 40_223, 25, 0);
    let vis = game.player_visibility(0);
    let home = *vis.iter().next().expect("a starting seat sees something");
    let seen = *game
        .nbrs(home)
        .iter()
        .find(|position| vis.contains(*position))
        .expect("a visible tile has a visible neighbor");
    let mut dark = game
        .map
        .tiles
        .keys()
        .filter(|position| !vis.contains(*position))
        .copied();
    let dark_a = dark.next().expect("fog exists on a fresh map");
    let dark_b = dark.next().expect("fog is wider than one tile");
    game.unit_move_trails.push(UnitMoveTrail {
        unit: 701,
        owner: 1,
        turn: game.turn,
        path: vec![dark_a, dark_b, home, seen],
    });
    game.unit_move_trails.push(UnitMoveTrail {
        unit: 702,
        owner: 1,
        turn: game.turn,
        path: vec![dark_a, home, dark_b],
    });
    game.unit_move_trails.push(UnitMoveTrail {
        unit: 703,
        owner: 0,
        turn: game.turn,
        path: vec![dark_a, dark_b],
    });

    let spectated = observation_spectator(&game, 0);
    let whole = spectated["unit_move_trails"].as_array().unwrap();
    assert_eq!(whole.len(), 3, "the spectator reads every route whole");
    assert_eq!(whole[0]["unit"], 701);
    assert_eq!(whole[0]["owner"], 1);
    assert_eq!(whole[0]["turn"], serde_json::json!(game.turn));
    assert_eq!(
        whole[0]["path"],
        serde_json::json!([
            [dark_a.0, dark_a.1],
            [dark_b.0, dark_b.1],
            [home.0, home.1],
            [seen.0, seen.1]
        ])
    );

    let watched = observation_player_view(&game, 0);
    let fogged = watched["unit_move_trails"].as_array().unwrap();
    assert_eq!(
        fogged.len(),
        2,
        "one clipped enemy stretch, one whole own route; the glimpsed \
             single tile is no route"
    );
    assert_eq!(fogged[0]["unit"], 701);
    assert_eq!(
        fogged[0]["path"],
        serde_json::json!([[home.0, home.1], [seen.0, seen.1]]),
        "an enemy walk survives only where the fog lifts"
    );
    assert_eq!(fogged[1]["unit"], 703);
    assert_eq!(
        fogged[1]["path"],
        serde_json::json!([[dark_a.0, dark_a.1], [dark_b.0, dark_b.1]]),
        "a seat remembers its own walking wherever it went"
    );
}

/// The browser draws its blast, writes its log entry and marks its war card
/// from these three fields and nothing else. A detonation the client cannot
/// see is the same as one that did not happen, so the wire shape is pinned
/// here rather than left to a screenshot.
#[test]
fn a_detonation_reaches_the_wire_whole() {
    let mut game = Game::new_full(2, 24, 16, 91_806, 200, 0, false);
    let mut capitals = Vec::new();
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        capitals.push(game.found_city_for(pid, game.units[&settler].pos, None));
        game.remove_unit(settler);
    }
    let target = game.cities[&capitals[1]].pos;
    let city_name = game.cities[&capitals[1]].name.clone();
    game.at_war.insert((0, 1));
    // Whether two randomly placed capitals sit inside 12 tiles or 15 is a
    // property of the seed, so the device is chosen to reach rather than the
    // fixture bent to suit one.
    let spec = game.rules.wmds["thermonuclear_device"].clone();
    let distance = game.wdist(game.cities[&capitals[0]].pos, target);
    assert!(
        distance <= spec.icbm_strike_range,
        "fixture capitals must sit within ICBM range ({distance})"
    );
    game.players[0]
        .counters
        .insert("project_effect:thermonuclear_devices".to_string(), 1);
    for position in game.wdisk(target, spec.blast_radius) {
        game.players[0].explored.insert(position);
    }
    game.apply(
        0,
        &crate::game::Action::WmdStrike {
            city: capitals[0],
            target,
            thermonuclear: true,
        },
    )
    .expect("the fixture capitals are inside ICBM range");

    let observed = observation_spectator(&game, 0);

    // 1. The strike ledger, which is what the animation reads.
    let strikes = observed["nuclear_strikes"].as_array().unwrap();
    assert_eq!(strikes.len(), 1);
    let strike = &strikes[0];
    assert!(strike["id"].as_u64().is_some(), "a blast needs an identity");
    assert_eq!(strike["attacker"], serde_json::json!(0));
    assert_eq!(strike["turn"], serde_json::json!(game.turn));
    assert_eq!(
        strike["target"],
        serde_json::json!([target.0, target.1]),
        "ground zero cannot be inferred from a halved city"
    );
    assert_eq!(strike["thermonuclear"], serde_json::json!(true));
    assert_eq!(strike["platform"], serde_json::json!("city"));
    assert_eq!(strike["blast_radius"], serde_json::json!(spec.blast_radius));
    assert_eq!(strike["cities"], serde_json::json!([city_name]));
    assert_eq!(strike["victims"], serde_json::json!([1]));
    assert!(strike["fallout_until"].as_u64().unwrap() > game.turn as u64);

    // 2. The notification, flagged so the log pins it.
    let detonation = observed["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["category"] == "Nuclear")
        .expect("the log hears about it");
    assert_eq!(detonation["important"], serde_json::json!(true));
    assert!(detonation["text"]
        .as_str()
        .unwrap()
        .contains("thermonuclear device"));
    assert_eq!(
        detonation["pos"],
        serde_json::json!([target.0, target.1]),
        "the entry points at the crater"
    );

    // 3. The war moment, so the card can badge the war as nuclear.
    let moment = observed["wars"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|war| war["highlights"].as_array().unwrap())
        .find(|moment| moment["kind"] == "nuclear_strike")
        .expect("the war ledger names the detonation");
    assert_eq!(moment["actor"], serde_json::json!(0));
    assert_eq!(moment["subject"], serde_json::json!(1));
    assert_eq!(moment["city"], serde_json::json!(city_name));

    // The victim's own view carries the same detonation, marked the same way.
    let victim = observation(&game, 1);
    let told = victim["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["category"] == "Nuclear")
        .expect("the civilization it landed on is told");
    assert_eq!(told["important"], serde_json::json!(true));
}

#[test]
fn observation_exposes_compact_hud_and_victory_race_metrics() {
    let mut game = Game::new_full(2, 20, 14, 81_004, 120, 1, false);
    let capital_position = game
        .player_unit_ids(0)
        .into_iter()
        .find_map(|unit| {
            let unit = &game.units[&unit];
            (unit.kind == "settler").then_some(unit.pos)
        })
        .unwrap();
    game.found_city_for(0, capital_position, None);
    let city_id = game.player_city_ids(0)[0];
    game.cities.get_mut(&city_id).unwrap().pop = 7;
    let observed = observation_spectator(&game, 0);
    assert_eq!(observed["max_turns"], serde_json::json!(120));
    // The setup panel adopts the running game's handicap, so the
    // observation has to carry it.
    assert_eq!(
        observed["difficulty"],
        serde_json::json!(game.difficulty),
        "the observation reports the difficulty the game is played on"
    );

    let player = observed["players"]
        .as_array()
        .unwrap()
        .iter()
        .find(|player| player["id"] == serde_json::json!(0))
        .unwrap();
    assert_eq!(
        player["cities"],
        serde_json::json!(game.player_city_ids(0).len()),
    );
    assert_eq!(
        player["population"],
        serde_json::json!(7),
        "the player HUD payload reports the empire's total population"
    );
    assert!(player["suzerain_count"].is_number());
    assert!(player["wonder_count"].is_number());

    let free_cities = observed["players"]
        .as_array()
        .unwrap()
        .iter()
        .find(|player| player["is_free_city"] == serde_json::json!(true))
        .unwrap();
    assert_eq!(free_cities["alive"], serde_json::json!(false));

    let source_city = &game.cities[&city_id];
    let city = observed["cities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|city| city["id"] == serde_json::json!(city_id))
        .unwrap();
    assert_eq!(
        city["amenities"],
        serde_json::json!(game.city_amenities(source_city))
    );
    assert_eq!(
        city["amenities_required"],
        serde_json::json!(Game::city_amenities_required(source_city))
    );
    assert_eq!(
        city["amenity_surplus"],
        serde_json::json!(game.city_amenity_surplus(source_city))
    );
    assert_eq!(
        city["happiness"],
        serde_json::json!(game.city_happiness(source_city))
    );
    assert!(city["loyalty_per_turn"].is_number());
    assert!(city["loyalty_state"].is_string());
    assert_eq!(city["free_city"], serde_json::json!(false));

    let victories = player["victories"].as_object().unwrap();
    for victory in [
        "science",
        "culture",
        "religious",
        "diplomatic",
        "domination",
        "score",
    ] {
        assert!(victories[victory]["progress"].is_number(), "{victory}");
    }
}

#[test]
fn observation_marks_every_civilization_that_founded_a_religion() {
    let mut game = Game::new_full(3, 22, 16, 81_005, 120, 0, false);
    for pid in 0..3 {
        let settler_position = game
            .player_unit_ids(pid)
            .into_iter()
            .find_map(|unit| {
                let unit = &game.units[&unit];
                (unit.kind == "settler").then_some(unit.pos)
            })
            .unwrap();
        game.found_city_for(pid, settler_position, None);
    }

    game.players[0].religion = Some("Living Faith".to_string());
    game.players[1].religion = Some("Unseen Faith".to_string());
    let living_city = game.player_city_ids(0)[0];
    let city = game.cities.get_mut(&living_city).unwrap();
    city.atheist_pressure = 0.0;
    city.pressure.insert("Living Faith".to_string(), 100.0);

    let observed = observation_spectator(&game, 0);
    let marker = |pid| {
        observed["players"]
            .as_array()
            .unwrap()
            .iter()
            .find(|player| player["id"] == json!(pid))
            .unwrap()["founded_religion_exists"]
            .clone()
    };
    assert_eq!(marker(0), json!(true), "a followed founded faith is marked");
    assert_eq!(
        marker(1),
        json!(true),
        "a founder is marked though no city the board can see follows its faith"
    );
    assert_eq!(
        marker(2),
        json!(false),
        "a civilization that never founded is not marked"
    );
}

/// The victory ribbon carries the researched technology and civic counts
/// behind the science and culture races. Domination counts every original
/// capital in the world, a civilization's own included, which is how
/// `check_domination` reads the board.
#[test]
fn victory_metrics_carry_researched_trees_and_every_capital() {
    let players = 6;
    let game = Game::new_full(players, 26, 18, 81_005, 120, 1, false);
    let observed = observation_spectator(&game, 0);
    let victories = observed["players"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|player| player["victories"].is_object())
        .map(|player| player["victories"].clone())
        .collect::<Vec<_>>();
    assert_eq!(victories.len(), players);

    for victory in &victories {
        assert_eq!(victory["domination"]["capitals"], serde_json::json!(1));
        assert_eq!(victory["domination"]["target"], serde_json::json!(players));
        assert_eq!(
            victory["science"]["tech_total"],
            serde_json::json!(game.rules.techs.len()),
        );
        assert!(victory["science"]["techs"].is_number());
        assert_eq!(
            victory["culture"]["civic_total"],
            serde_json::json!(game.rules.civics.len()),
        );
        assert!(victory["culture"]["civics"].is_number());
        assert!(victory["culture"]["domestic"].is_number());
        assert!(victory["culture"]["leading_domestic"].is_number());
    }
}

/// The score race ends on the turn limit rather than at a threshold, so
/// its meter is the clock: the leader is exactly as far along as the game
/// is, and every rival is that same distance cut by their share of the
/// leader's score.
#[test]
fn score_meter_is_the_turn_clock_scaled_by_the_share_of_the_leader() {
    let mut game = Game::new_full(4, 26, 18, 81_006, 100, 1, false);
    // Give the meter an explicit leader instead of relying on generated
    // starts to have founded and scored before this observation-only test.
    std::sync::Arc::make_mut(&mut game.observed_score).insert(0, 100);
    game.turn = 25;
    let leading = game
        .players
        .iter()
        .filter(|player| !player.is_minor && !player.is_barbarian)
        .map(|player| game.team_score_rank_key(player.id).0)
        .max()
        .unwrap();
    assert!(leading > 0);

    let observed = observation_spectator(&game, 0);
    let mut saw_leader = false;
    for player in observed["players"].as_array().unwrap() {
        let Some(score) = player["victories"]["score"].as_object() else {
            continue;
        };
        let points = score["points"].as_i64().unwrap();
        let expected = round1(100.0 * 0.25 * points as f64 / leading as f64);
        assert_eq!(score["progress"], serde_json::json!(expected));
        if points == leading {
            saw_leader = true;
            // A quarter of the turns played puts the leader a quarter up.
            assert_eq!(score["progress"], serde_json::json!(25.0));
        }
    }
    assert!(saw_leader);

    // The last playable turn fills the leader's meter: the score victory
    // is awarded once the turn passes the limit.
    game.turn = game.max_turns;
    let observed = observation_spectator(&game, 0);
    let best = observed["players"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|player| player["victories"]["score"]["progress"].as_f64())
        .fold(0.0_f64, f64::max);
    assert_eq!(best, 100.0);
}

/// Nobody in an Ancient world knows what Iron is. The omniscient
/// spectator watches that world rather than a survey of it, so the
/// deposit reaches the wire only once the first civilization has the
/// technology to recognise it — and a seat that has not researched it
/// still sees bare ground.
#[test]
fn a_strategic_deposit_reaches_the_spectator_when_the_first_civ_discovers_it() {
    let mut game = Game::new_full(2, 20, 14, 19_067, 120, 1, false);
    let deposit = game
        .player_unit_ids(0)
        .into_iter()
        .find_map(|unit| {
            let unit = &game.units[&unit];
            (unit.kind == "settler").then_some(unit.pos)
        })
        .expect("the player starts with a settler");
    game.map.tiles.get_mut(&deposit).unwrap().resource = Some(crate::name!("iron"));
    for player in game.players.iter_mut() {
        player.techs.remove(&Name::new("bronze_working"));
    }

    let spectated =
        |game: &Game| observed_tile(&observation_spectator(game, 0), deposit)["resource"].clone();
    assert!(
        spectated(&game).is_null(),
        "no civilization has Bronze Working, so the Iron is on nobody's map"
    );

    let city_state = game
        .players
        .iter()
        .position(|player| player.is_minor)
        .expect("the world has a city-state");
    game.players[city_state]
        .techs
        .insert(crate::name!("bronze_working"));
    assert!(
        spectated(&game).is_null(),
        "a city-state is not one of the civilizations the spectator follows"
    );

    game.players[1].techs.insert(crate::name!("bronze_working"));
    assert_eq!(
        spectated(&game),
        json!("iron"),
        "one civilization's discovery puts the deposit on the world map"
    );

    // The seat itself is unmoved by a rival's research: its own view is
    // still gated on its own technology.
    assert!(
        observed_tile(&observation(&game, 0), deposit)["resource"].is_null(),
        "seat 0 has not researched Bronze Working and must still see bare ground"
    );
    game.players[0].techs.insert(crate::name!("bronze_working"));
    assert_eq!(
        observed_tile(&observation(&game, 0), deposit)["resource"],
        json!("iron")
    );
}

/// A viewer is told the turn the result is dated on, which for the score
/// tiebreak is the turn limit rather than the wrap the count was taken on.
#[test]
fn a_finished_game_reports_the_turn_its_result_is_dated_on() {
    let mut game = Game::new_full(2, 20, 14, 19_068, 250, 0, false);
    assert!(
        observation_spectator(&game, 0)["victory_turn"].is_null(),
        "a live world has no result to date"
    );
    game.turn = 251;
    game.winner = Some(0);
    game.victory_type = Some("score".to_string());
    let finished = observation_spectator(&game, 0);
    assert_eq!(
        finished["turn"],
        json!(251),
        "the engine keeps the wrap it took the count on"
    );
    assert_eq!(
        finished["victory_turn"],
        json!(250),
        "a 250-turn game is won on turn 250"
    );
}

#[test]
fn a_tactics_draw_is_published_as_finished_without_a_winner() {
    let mut game = Game::new_with(crate::game::GameOptions {
        map_script: crate::setup::MapScript::Battlefield,
        tactics: crate::setup::TacticsRules {
            turn_limit: 50,
            ..crate::setup::TacticsRules::default()
        },
        ..crate::game::GameOptions::new(2, 10, 10, 19_071, 50, 0)
    });
    game.turn = 51;
    game.victory_type = Some(crate::game::DRAW_RESULT.to_string());

    let result = observation_spectator(&game, 0);
    assert_eq!(result["finished"], json!(true));
    assert_eq!(result["draw"], json!(true));
    assert!(result["winner"].is_null());
    assert_eq!(result["winners"], json!([]));
    assert_eq!(result["victory_type"], json!(crate::game::DRAW_RESULT));
    assert_eq!(result["victory_label"], json!(crate::game::DRAW_RESULT));
    assert_eq!(result["victory_turn"], json!(50));
}

/// The Mercy Rule's notation is composed once, in the engine, so the
/// finish screen, the play-on plate and anything else reading `/state`
/// denote the same win the same way.
#[test]
fn a_mercy_ending_publishes_its_notation_beside_the_bare_type() {
    let mut game = Game::new_full(2, 20, 14, 19_069, 250, 0, false);
    assert!(
        observation_spectator(&game, 0)["victory_label"].is_null(),
        "a live world has no verdict to write"
    );
    game.winner = Some(0);
    game.victory_type = Some(crate::game::MERCY_VICTORY.to_string());
    game.mercy_lanes = vec!["science".to_string()];
    let finished = observation_spectator(&game, 0);
    assert_eq!(
        finished["victory_type"],
        json!("mercy"),
        "the engine's own answer is unchanged"
    );
    assert_eq!(finished["victory_label"], json!("Mercy Rule - Science"));

    // A world playing on past that result is live again and has no verdict
    // of its own, but the one it borrowed its turns from keeps the
    // notation rather than reverting to the bare word.
    assert!(game.play_on(crate::game::PlayOnMode::UntilNextVictory));
    let extended = observation_spectator(&game, 0);
    assert!(extended["victory_label"].is_null());
    assert_eq!(
        extended["decided"]["victory_type"],
        json!("mercy"),
        "the type a play-on extension suppresses is still the type"
    );
    assert_eq!(
        extended["decided"]["victory_label"],
        json!("Mercy Rule - Science")
    );
}

/// ★★★★★ THE BOARD PUBLISHES THE ENGINE'S OWN TILE YIELDS.
///
/// The client used to derive them from the shipped catalogue — terrain,
/// Hills, feature, resource, improvement — and that sum has no room for the
/// fertility a flood or an eruption leaves on the ground. Volcanic Soil
/// carries no yields of its own in Civilization VI either, so a tile the
/// game pays 3 Food 3 Production was drawn as the bare Grassland underneath
/// it. Publishing `Game::workable_tile_yields` is what makes the two agree
/// by construction.
#[test]
fn a_tiles_published_yields_carry_the_fertility_a_disaster_left() {
    let mut game = Game::new_full(2, 20, 14, 19_067, 120, 1, false);
    let ground = game
        .player_unit_ids(0)
        .into_iter()
        .find_map(|unit| {
            let unit = &game.units[&unit];
            (unit.kind == "settler").then_some(unit.pos)
        })
        .expect("the player starts with a settler");
    {
        let tile = game.map.tiles.get_mut(&ground).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
    }
    let bare = observed_tile(&observation_spectator(&game, 0), ground)["yields"].clone();
    assert_eq!(bare["food"], json!(2.0), "grassland pays 2 Food: {bare}");
    assert_eq!(bare["production"], json!(0.0));

    // One eruption's Food and one storm's Production, exactly as
    // `Tile::disaster_food` and `Tile::disaster_production` hold them.
    {
        let tile = game.map.tiles.get_mut(&ground).unwrap();
        tile.feature = Some(crate::name!("volcanic_soil"));
        tile.disaster_food = 1.0;
        tile.disaster_production = 3.0;
    }
    let fertile = observed_tile(&observation_spectator(&game, 0), ground)["yields"].clone();
    assert_eq!(
        (fertile["food"].clone(), fertile["production"].clone()),
        (json!(3.0), json!(3.0)),
        "the board has to draw what the game pays: {fertile}"
    );
    // And the raw fertility is still readable beside the total, so the
    // client can name where the extra came from.
    let published = observed_tile(&observation_spectator(&game, 0), ground).clone();
    assert_eq!(published["disaster_yields"]["food"], json!(1.0));
    assert_eq!(published["disaster_yields"]["production"], json!(3.0));
}

/// The mirror's host-to-model correction is part of what a tile pays, so it
/// is part of what the board draws. On a live game this is the only place
/// an eruption's fertility is written down at all: the host stores it on the
/// plot and the tile catalogue has no row for it.
#[test]
fn a_tiles_published_yields_carry_the_hosts_own_correction() {
    let mut game = Game::new_full(2, 20, 14, 19_067, 120, 1, false);
    let ground = game
        .player_unit_ids(0)
        .into_iter()
        .find_map(|unit| {
            let unit = &game.units[&unit];
            (unit.kind == "settler").then_some(unit.pos)
        })
        .expect("the player starts with a settler");
    {
        let tile = game.map.tiles.get_mut(&ground).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
    }
    std::sync::Arc::make_mut(&mut game.observed_tile_yield_adjustments).insert(
        ground,
        crate::rules::Yields {
            food: 1.0,
            production: 3.0,
            ..crate::rules::Yields::default()
        },
    );
    let published = observed_tile(&observation_spectator(&game, 0), ground)["yields"].clone();
    assert_eq!(
        (published["food"].clone(), published["production"].clone()),
        (json!(3.0), json!(3.0)),
        "host 3/3 against a modelled 2/0: {published}"
    );
}

/// The host-import fields are `Arc`-shared so a search branch bumps a
/// refcount instead of deep-copying 44 trees, and sharing must be
/// invisible from outside the board. `Game` serializes through `GameSer`,
/// which still holds the plain collections, so a save and the `/state`
/// endpoint carry exactly the JSON they carried before: the ones the save
/// format names come back verbatim, and the refusal sets it deliberately
/// omits still come back empty and are rebuilt from the run's event log.
#[test]
fn arc_shared_host_imports_keep_the_save_format_they_had() {
    let mut game = Game::new_full(2, 20, 14, 19_071, 120, 1, false);
    std::sync::Arc::make_mut(&mut game.observed_score).insert(1, 240);
    std::sync::Arc::make_mut(&mut game.observed_city_max_wall_hp).insert(7, 100);
    std::sync::Arc::make_mut(&mut game.blocked_city_sites).insert((3, 4));

    let encoded = serde_json::to_string(&game).unwrap();
    let raw: Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(
        raw["observed_score"],
        json!({ "1": 240 }),
        "an Arc serializes as the map it holds, not as a wrapper: {}",
        raw["observed_score"]
    );
    assert_eq!(raw["observed_city_max_wall_hp"], json!({ "7": 100 }));
    assert!(
        raw.get("blocked_city_sites").is_none(),
        "host refusals were never in the save format and must stay out of it"
    );

    let restored: Game = serde_json::from_str(&encoded).unwrap();
    assert_eq!(restored.observed_score.get(&1), Some(&240));
    assert_eq!(restored.observed_city_max_wall_hp.get(&7), Some(&100));
    assert!(restored.blocked_city_sites.is_empty());
}

fn observed_tile(observation: &Value, position: Pos) -> &Value {
    observation["map"]["tiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tile| tile["pos"] == json!([position.0, position.1]))
        .expect("tile is in the observation")
}

/// A detonation is a local event. It once bleached the whole frame as well,
/// which in a game watched for hours is a flash grenade every time one is
/// fired. The fireball, rings and fallout are what show a blast.
#[test]
fn a_detonation_glows_on_its_fireball_and_leaves_the_frame_alone() {
    let app = include_str!("../../web/assets/app.js");
    let flash = app
        .split_once("function drawNuclearFlash(now) {")
        .unwrap()
        .1
        .split_once("\n}\n")
        .unwrap()
        .0;
    assert!(flash.contains("cx.fillRect(sx - r, sy - r, r * 2, r * 2);"));
    assert!(
        !flash.contains("cv.width"),
        "a detonation must not paint across the whole canvas"
    );
}
