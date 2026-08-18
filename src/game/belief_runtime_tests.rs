use super::*;

fn game_with_capitals(seed: u64) -> (Game, Vec<u32>) {
    let mut game = Game::new_full(2, 24, 16, seed, 200, 0, false);
    let mut cities = Vec::new();
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let city = game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
        cities.push(city);
    }
    (game, cities)
}

#[test]
fn art_museum_theming_needs_one_era_and_three_artists() {
    let (mut game, cities) = game_with_capitals(91_804);
    let city = cities[0];
    let square = game.cities[&city].owned_tiles[1];
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), square);
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .extend([crate::name!("amphitheater"), crate::name!("art_museum")]);
    for (creator, era) in [("Donatello", 3), ("Rembrandt", 3), ("Monet", 3)] {
        game.grant_great_work(0, "art", era, creator);
    }
    let culture_themed = game.city_yields(city).culture;
    let tourism_themed = game.tourism_per_turn(0);
    assert!(
        game.boost_met(
            0,
            &crate::rules::BoostSpec {
                trigger: "themed_buildings".to_string(),
                count: 1,
                percent: None,
            }
        ),
        "same era, three artists: themed"
    );

    // Swap one artist for a second work by an existing one: the trio
    // keeps its era but loses its third artist, and the bonus with it.
    game.players[0].great_work_pieces[2].creator = "Donatello".to_string();
    let culture_plain = game.city_yields(city).culture;
    let tourism_plain = game.tourism_per_turn(0);
    let band = Game::amenity_yield_mult_for(game.city_amenity_surplus(&game.cities[&city]));
    assert!((culture_themed - culture_plain - 9.0 * band).abs() < 1e-9);
    // The trio adds its own 6 Tourism plus the 15%-of-culture stream on
    // the theming Culture.
    let expected = 6.0 + 0.15 * (culture_themed - culture_plain);
    assert!((tourism_themed - tourism_plain - expected).abs() < 1e-9);
    assert!(!game.boost_met(
        0,
        &crate::rules::BoostSpec {
            trigger: "themed_buildings".to_string(),
            count: 1,
            percent: None,
        }
    ));
}

#[test]
fn the_world_aims_for_three_barbarian_camps_per_major_civilization() {
    // BARBARIAN_CAMP_MAX_PER_MAJOR_CIV is 3, and
    // BARBARIAN_CAMP_FIRST_TURN_PERCENT_OF_TARGET_TO_ADD puts a third of
    // that on the map before the first turn is played. The tournament
    // lobby's eight majors therefore open against a handful of camps and
    // grow toward twenty-four, not toward nine.
    for players in [2_usize, 6, 8] {
        let g = Game::new_full(players, 44, 30, 4_170 + players as u64, 200, 0, true);
        assert_eq!(
            g.barbarian_camp_target(),
            3 * players,
            "{players} majors should aim for three camps each"
        );
        let opening = g.barb_camps.len();
        assert_eq!(
            opening,
            (3 * players * 33 / 100).max(1),
            "{players} majors should open against a third of the target"
        );
        assert!(
            opening < g.barbarian_camp_target(),
            "the opening placement must leave room to grow"
        );
    }
}

#[test]
fn the_rest_of_the_camps_arrive_over_the_opening_turns_not_once_a_decade() {
    // BARBARIAN_CAMP_ODDS_OF_NEW_CAMP_SPAWNING is 2, and it is rolled
    // every turn: the world walks from its opening third toward the full
    // target within the first dozens of turns, so the camps a civilization
    // runs into early are ones that appeared while it was exploring. The
    // engine used to try once every ten turns, which on this eight-player
    // lobby is one camp per decade against a target of 24 — 220 turns of
    // an empty map, i.e. never, in a 500-turn game.
    let mut arrivals: Vec<u32> = Vec::new();
    let mut growth: Vec<usize> = Vec::new();
    for seed in 0..8_u64 {
        let mut game = Game::new_full(8, 44, 30, 4_171 + seed, 500, 0, true);
        let opening = game.barb_camps.len();
        let mut first = None;
        for _ in 0..12 {
            game.turn += 1;
            game.barbarian_phase();
            if first.is_none() && game.barb_camps.len() > opening {
                first = Some(game.turn);
            }
        }
        arrivals.push(first.expect("a new camp inside the opening twelve turns"));
        // Twelve turns of the ten-turn schedule this replaced were one new
        // camp, always on turn ten. Twelve 1-in-2 rolls are six, and what
        // holds the real figure under that is not the cadence but the map:
        // camps stand seven tiles apart and out of everybody's sight, and
        // on a 44x30 world with eight majors and their city-states there
        // is only so much dark ground left that far from everything.
        // Per seed this only asks that the world grew at all: how many
        // camps a *particular* world can seat is a property of that world,
        // not of the cadence. Sweeping map seeds shows the tail — a 44x30
        // board with eight majors and their city-states sometimes has room
        // for one camp in twelve turns and no more — so the cadence is
        // asserted on the mean below, over all eight seeds, where it is
        // actually measurable.
        assert!(
            game.barb_camps.len() > opening,
            "seed {seed} seated no camps at all in twelve turns"
        );
        assert!(
            game.barb_camps.len() <= game.barbarian_camp_target(),
            "the target is a ceiling"
        );
        growth.push(game.barb_camps.len() - opening);
    }
    // A 1-in-2 roll first lands two turns in on average. Anything that
    // schedules camps by the decade instead cannot average under ten.
    let mean_arrival = f64::from(arrivals.iter().sum::<u32>()) / arrivals.len() as f64;
    assert!(
        mean_arrival < 5.0,
        "the first new camp averaged turn {mean_arrival:.1} across {arrivals:?}"
    );
    let mean_growth = growth.iter().sum::<usize>() as f64 / growth.len() as f64;
    assert!(
        mean_growth >= 3.0,
        "twelve turns averaged {mean_growth:.1} new camps: {growth:?}"
    );
}

#[test]
fn the_opening_camps_clear_a_start_that_has_not_founded_its_capital_yet() {
    // At setup every major civilization is a Settler standing on the plot
    // that becomes its capital next turn, and only the city-states have
    // cities. Measuring the four-tile floor from *cities* alone therefore
    // left the one place on the map a camp must never be — a start — with
    // nothing but a Settler's own sight to protect it.
    for seed in 0..10_u64 {
        let game = Game::new_full(8, 44, 30, 4_180 + seed, 500, 0, true);
        let starts: Vec<Pos> = game
            .units
            .values()
            .filter(|unit| unit.kind == "settler" && !game.players[unit.owner].is_barbarian)
            .map(|unit| unit.pos)
            .collect();
        assert!(!starts.is_empty(), "majors open holding Settlers");
        for camp in game.barb_camps.keys() {
            for start in &starts {
                assert!(
                    game.wdist(*camp, *start) >= BARBARIAN_CAMP_MINIMUM_DISTANCE_CITY,
                    "seed {seed}: camp {camp:?} sits {} from the start at {start:?}",
                    game.wdist(*camp, *start)
                );
            }
        }
    }
}

#[test]
fn a_camp_is_never_seated_where_a_civilization_is_already_looking() {
    // The shipped rule, and the one this engine had no notion of: an
    // outpost appears "in any tile that is outside of the visible range of
    // any non-Barbarian unit or city". Distance floors alone let a camp
    // materialise five tiles from a capital in a Warrior's plain sight.
    let mut game = Game::new_full(6, 44, 30, 4_173, 500, 0, true);
    let barbarian = game.barb_pid.expect("a barbarian seat");
    let watchers: Vec<usize> = (0..game.players.len())
        .filter(|pid| *pid != barbarian)
        .collect();
    let mut checked = 0;
    for _ in 0..60 {
        game.turn += 1;
        game.barbarian_phase();
        for camp in game.barb_camps.keys().copied().collect::<Vec<_>>() {
            for pid in &watchers {
                assert!(
                    !game.player_can_see(*pid, camp),
                    "camp {camp:?} is seated in plain sight of player {pid}"
                );
            }
            for city in game.cities.values() {
                assert!(
                    game.wdist(camp, city.pos) >= BARBARIAN_CAMP_MINIMUM_DISTANCE_CITY,
                    "camp {camp:?} sits {} from a city",
                    game.wdist(camp, city.pos)
                );
            }
            for other in game.barb_camps.keys().filter(|other| **other != camp) {
                assert!(
                    game.wdist(camp, *other) >= BARBARIAN_CAMP_MINIMUM_DISTANCE_ANOTHER_CAMP,
                    "camps {camp:?} and {other:?} sit {} apart",
                    game.wdist(camp, *other)
                );
            }
            checked += 1;
        }
    }
    assert!(checked > 0, "no camps were placed to check");
}

#[test]
fn the_top_difficulties_also_spawn_barbarians_twice_as_often() {
    // BarbarianAttackForces carries SpawnRate alongside its force sizes,
    // and it is 2 for every band up to Emperor and 1 from Immortal. The
    // top band does not just field bigger parties -- it assembles them
    // twice as often, on the same band boundary the force scale uses.
    let rules = crate::rules::Rules::embedded();
    for difficulty in ["settler", "chieftain", "warlord", "prince", "king", "emperor"] {
        assert_eq!(rules.difficulties[difficulty].barb_spawn_scale, 1.0, "{difficulty}");
    }
    for difficulty in ["immortal", "deity"] {
        assert_eq!(rules.difficulties[difficulty].barb_spawn_scale, 0.5, "{difficulty}");
    }

    // And it reaches the map: a camp that has just spawned waits half as
    // long to spawn again at Deity as it does at Prince. Read one named
    // camp immediately after its first post-opening spawn so the standing
    // guard and recon units do not make the global cap obscure the timer.
    let rearm = |difficulty: &str| {
        let mut game = Game::new_full(2, 40, 26, 4_172, 200, 0, true);
        game.difficulty = difficulty.to_string();
        let camp = *game.barb_camps.keys().next().unwrap();
        game.turn = game.barb_camps[&camp];
        game.barbarian_phase();
        game.barb_camps[&camp]
    };
    let deity = rearm("deity");
    let prince = rearm("prince");
    assert!(deity < prince, "Deity re-arms sooner than Prince ({deity} vs {prince})");
}

#[test]
fn difficulty_scales_the_standing_barbarian_force() {
    // BarbarianAttackForces bands its forces on difficulty and the bands
    // are what barb_force_scale carries: Settler and Chieftain at 0.5,
    // Warlord through Emperor at 1.0, Immortal and Deity at 1.5. Those
    // boundaries are exactly the shipped MinTargetDifficulty/
    // MaxTargetDifficulty pairs.
    let rules = crate::rules::Rules::embedded();
    for (difficulty, scale) in [
        ("settler", 0.5),
        ("chieftain", 0.5),
        ("warlord", 1.0),
        ("prince", 1.0),
        ("king", 1.0),
        ("emperor", 1.0),
        ("immortal", 1.5),
        ("deity", 1.5),
    ] {
        assert_eq!(rules.difficulties[difficulty].barb_force_scale, scale, "{difficulty}");
    }

    // And the scale reaches the field rather than sitting in the data: the
    // same world run at three difficulties fields three different numbers
    // of barbarians.
    let count = |difficulty: &str| {
        let mut game = Game::new_full(2, 40, 26, 4_171, 200, 0, true);
        game.difficulty = difficulty.to_string();
        // Camps re-arm on a turn counter, so the clock has to move.
        for _ in 0..60 {
            game.turn += 1;
            game.barbarian_phase();
        }
        game.barb_pid
            .map(|bpid| game.player_unit_ids(bpid).len())
            .unwrap_or(0)
    };
    let (low, standard, high) = (count("settler"), count("prince"), count("deity"));
    assert!(low < standard, "Settler fields fewer than Prince: {low} vs {standard}");
    assert!(standard < high, "Deity fields more than Prince: {standard} vs {high}");
}

#[test]
fn barbarian_camps_field_half_the_leaders_technology() {
    let mut game = Game::new_full(2, 24, 16, 91_805, 200, 0, true);
    // Ancient leaders: the camps make do with tech-free units.
    let ancient = game.barbarian_unit_pool();
    assert!(ancient.contains(&crate::name!("warrior")));
    assert!(!ancient.contains(&crate::name!("musketman")));

    // A leader deep in the tree arms the camps with gunpowder, and the
    // pool never fields sea, air, support, or unique units.
    let ranked: Vec<Name> = {
        let mut ranked: Vec<(&Name, &crate::rules::TechSpec)> =
            game.rules.techs.iter().collect();
        ranked.sort_by(|a, b| {
            (a.1.era, a.1.cost as i64, a.0).cmp(&(b.1.era, b.1.cost as i64, b.0))
        });
        ranked.iter().map(|(name, _)| **name).collect()
    };
    for tech in ranked.iter().take(70) {
        game.players[0].techs.insert(*tech);
    }
    let industrial = game.barbarian_unit_pool();
    assert!(
        industrial.contains(&crate::name!("musketman")),
        "half of seventy techs must include gunpowder: {industrial:?}"
    );
    for unit in &industrial {
        let spec = &game.rules.units[unit];
        assert!(spec.unique_to.is_none());
        assert!(!matches!(spec.domain.as_deref(), Some("sea") | Some("air")));
    }
}

#[test]
fn great_work_pieces_track_counters_through_grants_moves_and_deals() {
    let (mut game, _cities) = game_with_capitals(91_803);
    let tally = |game: &Game, pid: usize, kind: &str| {
        let counted = game.players[pid]
            .counters
            .get(&format!("great_work:{kind}"))
            .copied()
            .unwrap_or(0);
        let pieces = game.players[pid]
            .great_work_pieces
            .iter()
            .filter(|piece| piece.kind == kind)
            .count() as i64;
        (counted, pieces)
    };

    // A named writer leaves two signed, era-stamped works.
    let homer = game.rules.great_people["homer"].clone();
    game.named_great_person_effect(0, &homer);
    assert_eq!(tally(&game, 0, "writing"), (2, 2));
    assert!(game.players[0]
        .great_work_pieces
        .iter()
        .all(|piece| piece.creator == "Homer" && piece.era == homer.era));

    // A dig raises an artifact from a past era.
    game.world_era = 3;
    game.grant_great_work(0, "artifact", 1, "antiquity");
    assert_eq!(tally(&game, 0, "artifact"), (1, 1));

    // Deals and thefts move the piece with the counter.
    let items = DealItems {
        great_works: BTreeMap::from([("writing".to_string(), 1)]),
        ..Default::default()
    };
    game.transfer_great_work_items(0, 1, &items);
    assert_eq!(tally(&game, 0, "writing"), (1, 1));
    assert_eq!(tally(&game, 1, "writing"), (1, 1));
    assert_eq!(game.players[1].great_work_pieces[0].creator, "Homer");
}

#[test]
fn wmd_strikes_launch_from_range_consume_devices_and_leave_fallout() {
    let (mut game, cities) = game_with_capitals(91_802);
    let (launch, struck) = (cities[0], cities[1]);
    let target = game.cities[&struck].pos;
    let distance = game.wdist(game.cities[&launch].pos, target);
    let spec = game.rules.wmds["thermonuclear_device"].clone();
    assert!(
        distance <= spec.icbm_strike_range,
        "fixture capitals must sit within ICBM range ({distance})"
    );
    game.players[0].explored.insert(target);
    for position in game.wdisk(target, spec.blast_radius) {
        game.players[0].explored.insert(position);
    }
    game.cities.get_mut(&struck).unwrap().pop = 6;
    game.cities.get_mut(&struck).unwrap().wall_hp = 100;
    let city_name = game.cities[&struck].name.clone();
    let defender = game.spawn_test_unit("warrior", 1, target);

    let strike = Action::WmdStrike {
        city: launch,
        target,
        thermonuclear: true,
    };
    // No device yet, and no war yet: both must refuse the order.
    assert!(game.apply(0, &strike).is_err());
    game.players[0]
        .counters
        .insert("project_effect:thermonuclear_devices".to_string(), 1);
    assert!(
        game.apply(0, &strike).is_err(),
        "nuking a civilization at peace must be refused"
    );
    game.at_war.insert(pair(0, 1));

    // The strike is enumerated once war and a device exist.
    assert!(game.legal_actions(0).contains(&strike));

    let under_the_blast = game.player_unit_ids(1);
    game.apply(0, &strike).unwrap();
    let vaporized = under_the_blast
        .iter()
        .filter(|uid| !game.units.contains_key(uid))
        .count() as u32;
    assert!(vaporized >= 1, "ground zero is lethal");
    assert_eq!(
        game.players[0].counters["project_effect:thermonuclear_devices"],
        0
    );
    assert_eq!(game.players[0].counters["wmd_strikes"], 1);
    let city = &game.cities[&struck];
    assert_eq!(city.pop, 3, "the blast halves the population");
    assert_eq!(city.wall_hp, 0, "ground zero levels the Outer Defenses");
    assert!(
        game.units
            .get(&defender)
            .is_none_or(|unit| unit.hp <= 0 || unit.hp < 100),
        "a full-health defender cannot shrug off ground zero"
    );
    for position in game.wdisk(target, spec.blast_radius) {
        assert!(
            game.map.tiles[&position].fallout_until
                >= game.turn + spec.fallout_duration,
            "every blast tile carries fallout"
        );
    }
    // The stockpile is spent: a second launch must be refused.
    assert!(game.apply(0, &strike).is_err());

    // A device is the loudest thing that happens in a war, and its
    // casualties die down the same path a volcano's do — both have to
    // reach the war's ledger anyway.
    let war = game
        .wars
        .get(&pair(0, 1))
        .expect("a strike is fought inside a war");
    let detonation = war
        .highlights
        .iter()
        .find(|moment| moment.kind == "nuclear_strike")
        .expect("the ledger names the detonation");
    assert_eq!((detonation.actor, detonation.subject), (0, 1));
    assert_eq!(detonation.city.as_deref(), Some(city_name.as_str()));
    assert_eq!(
        war.losses_for(1).units,
        vaporized,
        "every unit the blast killed is counted against its side"
    );

    // The engine's own account of the detonation, which is what a client
    // animates and what the notification log is written from.
    let record = game
        .nuclear_strikes
        .last()
        .expect("a detonation is recorded on the game");
    assert_eq!(record.attacker, 0);
    assert_eq!(record.target, target);
    assert!(record.thermonuclear);
    assert_eq!(record.platform, "city");
    assert_eq!(record.launched_from, game.cities[&launch].pos);
    assert_eq!(record.blast_radius, spec.blast_radius);
    assert_eq!(record.units_destroyed, vaporized);
    assert_eq!(record.cities, vec![city_name.clone()]);
    assert!(record.victims.contains(&1), "the struck civ is a victim");

    // Both sides hear about it, and both entries are pinned: a log running
    // at speed must not scroll a mushroom cloud past unread.
    let launcher_entry = game
        .events_for(0)
        .into_iter()
        .rev()
        .find(|event| event.text.contains("thermonuclear device"))
        .expect("the launcher's log names the detonation");
    assert!(launcher_entry.important);
    assert!(
        launcher_entry.text.contains(&city_name),
        "the entry names what it hit: {}",
        launcher_entry.text
    );
    let victim_entry = game
        .events_for(1)
        .into_iter()
        .rev()
        .find(|event| event.text.contains("thermonuclear device"))
        .expect("the victim's log names the detonation");
    assert!(victim_entry.important);
    assert_eq!(victim_entry.pos, Some(target));
}

/// An SSBN is the reason a device has a range at all rather than a launch
/// site: the boat carries the range to the target.
#[test]
fn a_nuclear_submarine_carries_a_device_past_its_launch_city_reach() {
    let (mut game, cities) = game_with_capitals(91_803);
    let launch = cities[0];
    let home = game.cities[&launch].pos;
    let spec = game.rules.wmds["nuclear_device"].clone();
    game.players[0]
        .counters
        .insert("project_effect:nuclear_devices".to_string(), 2);

    // An empty tile beyond every land platform's reach. Nothing lives there,
    // so this isolates range from every other rule the order checks.
    let distant = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.wdist(*position, home) > spec.icbm_strike_range)
        .expect("a 24x16 map has a tile out of ICBM range");
    game.players[0].explored.insert(distant);
    let strike = Action::WmdStrike {
        city: launch,
        target: distant,
        thermonuclear: false,
    };
    assert_eq!(
        game.apply(0, &strike),
        Err("target out of ICBM range".to_string()),
        "no land platform reaches that far"
    );

    // A boat within range makes the same order legal, and the record says
    // what carried it.
    let boat = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| {
            *position != distant && game.wdist(*position, distant) <= spec.icbm_strike_range
        })
        .expect("some other tile is within range of the target");
    let submarine = game.spawn_test_unit("nuclear_submarine", 0, boat);
    assert!(game.apply(0, &strike).is_ok());
    let record = game.nuclear_strikes.last().expect("the strike is recorded");
    assert_eq!(record.platform, "nuclear_submarine");
    assert_eq!(record.launched_from, boat);
    // The stockpile, not the boat, is what a launch spends.
    assert_eq!(game.players[0].counters["project_effect:nuclear_devices"], 1);
    assert!(
        game.units.contains_key(&submarine),
        "the boat survives its own launch"
    );

    // Sinking it takes the reach away again.
    game.remove_unit(submarine);
    assert_eq!(
        game.apply(0, &strike),
        Err("target out of ICBM range".to_string()),
        "the range left with the boat"
    );
}

/// A reloaded game has to remember the war it is in the middle of. The
/// ledger is a new field on a long-lived save format, so the round trip is
/// the thing that proves the `serde` plumbing on all four sides — struct,
/// save struct, and both conversions — is actually wired up.
#[test]
fn the_strike_ledger_survives_a_save_round_trip() {
    let (mut game, cities) = game_with_capitals(91_807);
    let (launch, struck) = (cities[0], cities[1]);
    let target = game.cities[&struck].pos;
    let spec = game.rules.wmds["thermonuclear_device"].clone();
    let distance = game.wdist(game.cities[&launch].pos, target);
    assert!(
        distance <= spec.icbm_strike_range,
        "fixture capitals must sit within ICBM range ({distance})"
    );
    game.at_war.insert(pair(0, 1));
    game.players[0]
        .counters
        .insert("project_effect:thermonuclear_devices".to_string(), 1);
    for position in game.wdisk(target, spec.blast_radius) {
        game.players[0].explored.insert(position);
    }
    game.apply(
        0,
        &Action::WmdStrike {
            city: launch,
            target,
            thermonuclear: true,
        },
    )
    .unwrap();
    assert_eq!(game.nuclear_strikes.len(), 1);

    let encoded = serde_json::to_value(&game).unwrap();
    let restored: Game = serde_json::from_value(encoded).unwrap();
    assert_eq!(restored.nuclear_strikes, game.nuclear_strikes);

    // And a save written before the ledger existed loads as an empty one
    // rather than refusing to open.
    let mut older = serde_json::to_value(&game).unwrap();
    older
        .as_object_mut()
        .unwrap()
        .remove("nuclear_strikes")
        .expect("the field is written");
    let old_save: Game = serde_json::from_value(older).unwrap();
    assert!(old_save.nuclear_strikes.is_empty());
}

/// An empty plains board with an interior tile to walk from, so a route
/// test controls every step cost and nothing ambushes the walker.
fn flat_walking_game(seed: u64) -> (Game, Pos) {
    let mut game = Game::new_full(2, 20, 14, seed, 40, 0, false);
    let ids: Vec<u32> = game.units.keys().copied().collect();
    for id in ids {
        game.remove_unit(id);
    }
    game.map.clear_rivers();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.owner_city = None;
        tile.hills = false;
        tile.road = 0;
    }
    let center = *game
        .map
        .tiles
        .keys()
        .find(|p| game.wdisk(**p, 2).len() == 19)
        .expect("controlled map has an interior tile");
    game.current = 0;
    (game, center)
}

/// The ledger's whole reason to exist: a spectator frame is one seat's
/// entire turn, so the board diff alone cannot say which tiles carried a
/// unit to where it ended up. The recorded route must be the engine's own
/// executed path, tile by tile.
#[test]
fn a_move_records_its_exact_walked_route() {
    let (mut game, start) = flat_walking_game(52_001);
    let soldier = game.spawn_unit("warrior", 0, start);
    let destination = *game
        .wdisk(start, 2)
        .iter()
        .find(|position| game.wdist(start, **position) == 2)
        .expect("an interior tile has a neighbor two steps out");
    let mut expected = vec![start];
    expected.extend(game.path_to(soldier, destination).expect("plains walk"));
    game.apply(
        0,
        &Action::MoveTo {
            unit: soldier,
            to: destination,
        },
    )
    .unwrap();
    let trail = game.unit_move_trails.last().expect("the walk is recorded");
    assert_eq!(trail.unit, soldier);
    assert_eq!(trail.owner, 0);
    assert_eq!(trail.turn, game.turn);
    assert_eq!(trail.path, expected);
    assert_eq!(*trail.path.last().unwrap(), game.units[&soldier].pos);
    for hop in trail.path.windows(2) {
        assert_eq!(game.wdist(hop[0], hop[1]), 1, "a route only walks");
    }
}

/// One unit's steps within one turn read back as a single route, and the
/// turn boundary starts a fresh one — that boundary is what lets a client
/// age tails out N turns after they were walked.
#[test]
fn steps_chain_within_a_turn_and_break_at_the_boundary() {
    let (mut game, start) = flat_walking_game(52_002);
    let soldier = game.spawn_unit("warrior", 0, start);
    let first = game.nbrs(start)[0];
    let second = *game
        .nbrs(first)
        .iter()
        .find(|position| game.wdist(start, **position) == 2)
        .expect("a neighbor's neighbor lies two steps from home");
    game.apply(
        0,
        &Action::Move {
            unit: soldier,
            to: first,
        },
    )
    .unwrap();
    game.units.get_mut(&soldier).unwrap().moves_left = 2.0;
    game.apply(
        0,
        &Action::Move {
            unit: soldier,
            to: second,
        },
    )
    .unwrap();
    assert_eq!(game.unit_move_trails.len(), 1, "same turn, same route");
    assert_eq!(game.unit_move_trails[0].path, vec![start, first, second]);

    game.turn += 1;
    game.units.get_mut(&soldier).unwrap().moves_left = 2.0;
    game.apply(
        0,
        &Action::Move {
            unit: soldier,
            to: first,
        },
    )
    .unwrap();
    assert_eq!(game.unit_move_trails.len(), 2, "a new turn walks fresh");
    assert_eq!(game.unit_move_trails[1].path, vec![second, first]);
    assert_eq!(game.unit_move_trails[1].turn, game.turn);
}

/// A reloaded game keeps the tails a client was drawing, and a save
/// written before the ledger existed loads as an empty one rather than
/// refusing to open — the same four-sided `serde` proof the strike ledger
/// needed.
#[test]
fn the_trail_ledger_survives_a_save_round_trip() {
    let (mut game, start) = flat_walking_game(52_003);
    let soldier = game.spawn_unit("warrior", 0, start);
    game.apply(
        0,
        &Action::Move {
            unit: soldier,
            to: game.nbrs(start)[0],
        },
    )
    .unwrap();
    assert_eq!(game.unit_move_trails.len(), 1);

    let encoded = serde_json::to_value(&game).unwrap();
    let restored: Game = serde_json::from_value(encoded).unwrap();
    assert_eq!(restored.unit_move_trails, game.unit_move_trails);

    let mut older = serde_json::to_value(&game).unwrap();
    older
        .as_object_mut()
        .unwrap()
        .remove("unit_move_trails")
        .expect("the field is written");
    let old_save: Game = serde_json::from_value(older).unwrap();
    assert!(old_save.unit_move_trails.is_empty());
}

/// The ledger is a tail, not a chronicle: however long a game runs, the
/// oldest walks fall off rather than the save growing forever.
#[test]
fn the_trail_ledger_stays_bounded() {
    let (mut game, start) = flat_walking_game(52_004);
    let step = game.nbrs(start)[0];
    for walker in 0..600u32 {
        game.record_move_step(0, 900_000 + walker, start, step);
    }
    assert_eq!(game.unit_move_trails.len(), 512);
    assert_eq!(
        game.unit_move_trails.first().unwrap().unit,
        900_088,
        "the oldest walks are the ones that fell off"
    );
}

/// Teleport-style relocations are not walks: a route that "walked" an
/// airlift would draw a march across the whole map.
#[test]
fn a_long_jump_never_chains_into_a_walked_route() {
    let (mut game, start) = flat_walking_game(52_005);
    let far = *game
        .wdisk(start, 2)
        .iter()
        .find(|position| game.wdist(start, **position) == 2)
        .expect("an interior tile has a neighbor two steps out");
    game.record_move_step(0, 900_000, start, far);
    assert!(game.unit_move_trails.is_empty());
}

/// Fallout that only cost yields would make a crater the safest ground on
/// the map: nothing contests it, and a wounded army could sit in it and
/// heal.
#[test]
fn fallout_wounds_what_stands_in_it_and_stops_it_healing() {
    let (mut game, cities) = game_with_capitals(91_805);
    let position = game.cities[&cities[0]].owned_tiles[1];
    let soldier = game.spawn_test_unit("warrior", 0, position);
    game.units.get_mut(&soldier).unwrap().hp = 60;

    // Clean ground: the same unit on the same tile heals.
    assert!(
        game.unit_heal_rate(soldier) > 0,
        "a wounded unit heals on its own land"
    );

    game.map.tiles.get_mut(&position).unwrap().fallout_until = game.turn + 5;
    assert!(game.fallout_at(position));
    assert_eq!(
        game.unit_heal_rate(soldier),
        0,
        "nothing recovers in fallout"
    );

    game.irradiate_units(0);
    assert_eq!(
        game.units[&soldier].hp,
        60 - Game::FALLOUT_UNIT_DAMAGE,
        "holding irradiated ground costs health every turn"
    );

    // It kills, and the death is logged where the player will see it.
    game.units.get_mut(&soldier).unwrap().hp = Game::FALLOUT_UNIT_DAMAGE;
    game.irradiate_units(0);
    assert!(!game.units.contains_key(&soldier), "fallout finishes it");
    let obituary = game
        .events_for(0)
        .into_iter()
        .rev()
        .find(|event| event.text.contains("nuclear fallout"))
        .expect("a unit lost to fallout is logged");
    assert!(obituary.important);
    assert_eq!(obituary.pos, Some(position));

    // Once it decays, the ground is ordinary again.
    let survivor = game.spawn_test_unit("warrior", 0, position);
    game.units.get_mut(&survivor).unwrap().hp = 60;
    game.turn = game.map.tiles[&position].fallout_until;
    assert!(!game.fallout_at(position));
    assert!(game.unit_heal_rate(survivor) > 0);
    game.irradiate_units(0);
    assert_eq!(game.units[&survivor].hp, 60, "decayed fallout is harmless");
}


fn place_district(game: &mut Game, city: u32, district: &str) -> Pos {
    let center = game.cities[&city].pos;
    let position = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            *position != center
                && game.map.tiles[position].district.is_none()
                && game.map.tiles[position].wonder.is_none()
        })
        .unwrap();
    let tile = game.map.tiles.get_mut(&position).unwrap();
    tile.district = Some(Name::new(district));
    tile.improvement = None;
    tile.pillaged = false;
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(Name::new(district), position);
    position
}

#[test]
fn every_district_pays_the_route_yields_its_shipped_row_names() {
    // District_TradeRouteYields, all 44 rows. The City Center's own row is
    // the base every route starts from; each further district adds its own.
    // Unique districts carry their own rows in the table and every one of
    // them repeats its parent's numbers, so testing the families covers
    // them once district_family has resolved the replacement.
    let (game, cities) = game_with_capitals(24_907);
    let dest = cities[1];
    let base_domestic = game.route_yields(dest, true);
    let base_international = game.route_yields(dest, false);
    assert_eq!((base_domestic.food, base_domestic.production), (1.0, 1.0));
    assert_eq!(base_international.gold, 3.0);

    // (district, domestic food/production, international by yield)
    let rows: &[(&str, f64, f64, f64, f64, f64, f64)] = &[
        // district        dom food  dom prod  intl gold  sci  faith  culture
        ("campus", 1.0, 0.0, 0.0, 1.0, 0.0, 0.0),
        ("holy_site", 1.0, 0.0, 0.0, 0.0, 1.0, 0.0),
        ("theater_square", 1.0, 0.0, 0.0, 0.0, 0.0, 1.0),
        ("commercial_hub", 0.0, 1.0, 3.0, 0.0, 0.0, 0.0),
        ("harbor", 0.0, 1.0, 3.0, 0.0, 0.0, 0.0),
        ("government_plaza", 1.0, 1.0, 2.0, 0.0, 0.0, 0.0),
        ("diplomatic_quarter", 1.0, 1.0, 0.0, 0.0, 0.0, 1.0),
    ];
    for &(district, food, production, gold, science, faith, culture) in rows {
        let mut probe = game.clone();
        place_district(&mut probe, dest, district);
        let domestic = probe.route_yields(dest, true);
        assert_eq!(domestic.food - base_domestic.food, food, "{district} domestic Food");
        assert_eq!(
            domestic.production - base_domestic.production,
            production,
            "{district} domestic Production"
        );
        let abroad = probe.route_yields(dest, false);
        assert_eq!(abroad.gold - base_international.gold, gold, "{district} Gold");
        assert_eq!(abroad.science, science, "{district} Science");
        assert_eq!(abroad.faith, faith, "{district} Faith");
        assert_eq!(abroad.culture, culture, "{district} Culture");
    }

    // A pillaged destination district still pays its row (Rome's pillaged
    // Holy Site and Campus kept paying Cumae's route, run
    // civvis-20260816T200454Z t81-95): the row is for the district's
    // existence, not its working state.
    {
        let mut probe = game.clone();
        let campus = place_district(&mut probe, dest, "campus");
        let paid = probe.route_yields(dest, true);
        probe.map.tiles.get_mut(&campus).unwrap().pillaged = true;
        assert_eq!(probe.route_yields(dest, true), paid, "a pillaged Campus still pays");
        assert_eq!(probe.route_yields(dest, false).science, 1.0);
    }

    // Encampment, Industrial Zone and Entertainment Complex are the three
    // that pay the SAME yield to both kinds of route rather than splitting
    // a domestic yield from an international one.
    for (district, food, production) in [
        ("encampment", 0.0, 1.0),
        ("industrial_zone", 0.0, 1.0),
        ("entertainment_complex", 1.0, 0.0),
    ] {
        let mut probe = game.clone();
        place_district(&mut probe, dest, district);
        for domestic in [true, false] {
            let base = if domestic {
                &base_domestic
            } else {
                &base_international
            };
            let got = probe.route_yields(dest, domestic);
            assert_eq!(got.food - base.food, food, "{district} Food domestic={domestic}");
            assert_eq!(
                got.production - base.production,
                production,
                "{district} Production domestic={domestic}"
            );
        }
    }

    // A unique district is worth exactly what it replaces.
    for (unique, parent) in [
        ("acropolis", "theater_square"),
        ("cothon", "harbor"),
        ("suguba", "commercial_hub"),
        ("observatory", "campus"),
        ("lavra", "holy_site"),
        ("hansa", "industrial_zone"),
        ("ikanda", "encampment"),
        ("hippodrome", "entertainment_complex"),
    ] {
        let (mut a, mut b) = (game.clone(), game.clone());
        place_district(&mut a, dest, unique);
        place_district(&mut b, dest, parent);
        for domestic in [true, false] {
            assert_eq!(
                a.route_yields(dest, domestic),
                b.route_yields(dest, domestic),
                "{unique} should pay what {parent} pays"
            );
        }
    }
}

fn establish_religion(game: &mut Game, city: u32, beliefs: &[&str]) -> String {
    let religion = "Test Faith".to_string();
    game.players[0].religion = Some(religion.clone());
    game.players[0].religion_beliefs =
        beliefs.iter().map(|belief| belief.to_string()).collect();
    game.cities
        .get_mut(&city)
        .unwrap()
        .pressure
        .insert(religion.clone(), 1_000.0);
    religion
}

#[test]
fn the_pantheon_charges_exactly_what_it_asked_for() {
    // ⚠⚠ #1044 made the CHECK speed-aware and left the SPEND a bare 25.0, so on
    // every speed but Standard the engine demanded one price and took another —
    // Online asks 12.5 and took 25, dropping a founder at 13 faith to -12. That
    // is worse than both being wrong together, because the check licenses a
    // purchase the charge then cannot honour.
    let (mut game, _) = game_with_capitals(91_762);
    game.game_speed = crate::setup::GameSpeed::Online;
    game.players[0].pantheon = None;
    game.players[0].faith = 13.0;
    let belief = game.rules.beliefs.pantheon.keys().next().unwrap().clone();
    assert!(game.do_choose_pantheon(0, &belief).is_ok());
    assert_eq!(
        game.players[0].faith,
        0.5,
        "13 faith minus Online's 12.5 price; a bare 25.0 would leave -12"
    );

    // Standard still pays 25, so the default speed is untouched.
    let (mut standard, _) = game_with_capitals(91_762);
    standard.game_speed = crate::setup::GameSpeed::Standard;
    standard.players[0].pantheon = None;
    standard.players[0].faith = 30.0;
    let belief = standard.rules.beliefs.pantheon.keys().next().unwrap().clone();
    assert!(standard.do_choose_pantheon(0, &belief).is_ok());
    assert_eq!(standard.players[0].faith, 5.0);
}

#[test]
fn every_pantheon_price_reads_the_same_helper() {
    // ⚠⚠⚠ ONE TABLE, ONE FORMATTER. This rule had THREE spellings — the
    // legality gate and the affordability check in this file, and the AI's own
    // decision gate in `ai.rs` — and #1044 fixed one of them. The two that were
    // missed are exactly the ones that decide and charge, so the fix looked
    // merged and changed nothing live: on `civvis-20260803T231038Z` the AI was
    // still waiting for 25 while Civilization VI charged ~12 and its own
    // fallback picked the pantheon by database order.
    //
    // Asserted on the SOURCE, because a behavioural test can pass while a fourth
    // spelling is added somewhere it does not reach.
    for (path, source) in [
        ("game.rs", include_str!("../game.rs")),
        ("ai.rs", include_str!("../ai.rs")),
    ] {
        for (n, line) in source.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if !code.contains("pantheon") && !code.contains("Pantheon") {
                continue;
            }
            // ⚠ An assertion ABOUT the price is not a second spelling OF it —
            // `assert_eq!(pantheon_faith_cost(), 25.0)` is exactly the check
            // that pins Standard, and flagging it would make this scan
            // unsatisfiable. Skip assertions and the constant's own definition.
            if code.contains("assert") || code.contains("PANTHEON_FAITH_STANDARD")
            {
                continue;
            }
            assert!(
                !code.contains("25.0"),
                "{path}:{} spells the pantheon price as a literal; use                      `pantheon_faith_cost()`: {code}",
                n + 1
            );
        }
    }
}

#[test]
fn the_pantheon_price_follows_the_game_speed_like_every_other_cost() {
    // ⚠⚠⚠ This was a bare `25.0` in the legality gate AND in `do_choose_pantheon`,
    // while techs, civics and growth beside it all scaled. It cost CIVVIS the
    // decision outright in live games.
    //
    // Measured on live run `civvis-20260803T191900Z` (Online, Rome/Trajan):
    // Civilization VI raised `ENDTURN_BLOCKING_PANTHEON` on turn 19 at FAITH 11,
    // and faith went 11 -> 0 when it was taken. CIVVIS wanted 25, so
    // `ChoosePantheon` never became legal, `civvis_orders` emitted nothing on a
    // replay of turns 18/19/20, and the mod answered instead by walking
    // `GameInfo.Beliefs()` and taking the first untaken row — the empire's one
    // permanent pantheon chosen by database order, in every live game.
    let (mut game, _) = game_with_capitals(91_762);

    game.game_speed = crate::setup::GameSpeed::Standard;
    assert_eq!(game.pantheon_faith_cost(), 25.0, "Standard must not move");

    game.game_speed = crate::setup::GameSpeed::Online;
    assert_eq!(
        game.pantheon_faith_cost(),
        12.5,
        "Online pays half, and 12.5 is the price the host was charging"
    );

    // The gate and the action must agree, or one of them is decorative.
    game.players[0].pantheon = None;
    game.players[0].faith = 13.0;
    let belief = game.rules.beliefs.pantheon.keys().next().unwrap().clone();
    assert!(
        game.legal_actions(0)
            .iter()
            .any(|action| matches!(action, Action::ChoosePantheon { .. })),
        "13 faith on Online is above the real price and must offer the action"
    );
    assert!(game.do_choose_pantheon(0, &belief).is_ok());

    // ⚠ And the same faith on Standard must still be refused, or this is not a
    // scaling fix, it is a discount applied everywhere.
    let (mut standard, _) = game_with_capitals(91_762);
    standard.game_speed = crate::setup::GameSpeed::Standard;
    standard.players[0].pantheon = None;
    standard.players[0].faith = 13.0;
    assert!(
        standard
            .legal_actions(0)
            .iter()
            .all(|action| !matches!(action, Action::ChoosePantheon { .. })),
        "13 faith is below Standard's 25 and must not offer it"
    );
}

#[test]
fn pantheons_spend_faith_grant_units_and_apply_exact_production_gates() {
    for (seed, belief, unit) in [
        (91_760, "fertility_rites", "builder"),
        (91_761, "religious_settlements", "settler"),
    ] {
        let (mut game, _) = game_with_capitals(seed);
        game.players[0].faith = 25.0;
        let before = game
            .units
            .values()
            .filter(|candidate| candidate.owner == 0 && candidate.kind == unit)
            .count();
        game.do_choose_pantheon(0, belief).unwrap();
        assert_eq!(game.players[0].faith, 0.0);
        assert_eq!(
            game.units
                .values()
                .filter(|candidate| candidate.owner == 0 && candidate.kind == unit)
                .count(),
            before + 1
        );
    }

    let (mut game, cities) = game_with_capitals(91_762);
    game.players[0].pantheon = Some("god_of_the_forge".to_string());
    let multiplier = |game: &Game, unit: &str| {
        game.item_prod_mult(
            0,
            cities[0],
            Some(&Item::Unit {
                unit: Name::new(unit),
            }),
        )
    };
    assert_eq!(multiplier(&game, "warrior"), 1.25);
    assert_eq!(multiplier(&game, "catapult"), 1.25);
    assert_eq!(multiplier(&game, "infantry"), 1.0);

    game.players[0].pantheon = Some("divine_spark".to_string());
    let holy_site = place_district(&mut game, cities[0], "holy_site");
    assert!(!game.cities[&cities[0]]
        .buildings
        .contains(&crate::name!("shrine")));
    assert!(!game.map.tiles[&holy_site].pillaged);
    game.process_great_people(0);
    assert_eq!(game.players[0].gpp.get("prophet"), Some(&2.0));
}

#[test]
fn city_states_cannot_claim_pantheons_that_create_unusable_settlers() {
    let (mut game, _) = game_with_capitals(91_769);
    game.players[0].is_minor = true;
    game.players[0].faith = 25.0;
    let settlers = game
        .units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "settler")
        .count();

    assert!(game
        .do_choose_pantheon(0, "religious_settlements")
        .is_err());
    assert_eq!(game.players[0].faith, 25.0);
    assert!(game.players[0].pantheon.is_none());
    assert_eq!(
        game.units
            .values()
            .filter(|unit| unit.owner == 0 && unit.kind == "settler")
            .count(),
        settlers
    );
    assert!(game
        .legal_actions(0)
        .iter()
        .all(|action| !matches!(action, Action::ChoosePantheon { .. })));
}

#[test]
fn follower_beliefs_execute_building_adjacency_amenity_and_route_effects() {
    let (mut game, cities) = game_with_capitals(91_763);
    establish_religion(&mut game, cities[0], &[]);
    let holy_site = place_district(&mut game, cities[0], "holy_site");
    let mountain = game
        .nbrs(holy_site)
        .into_iter()
        .find(|position| *position != game.cities[&cities[0]].pos)
        .unwrap();
    game.map.tiles.get_mut(&mountain).unwrap().terrain = crate::name!("mountain");
    game.cities
        .get_mut(&cities[0])
        .unwrap()
        .buildings
        .extend([crate::name!("shrine"), crate::name!("temple")]);
    // Isolate the buildings' exact follower-belief yields from the
    // governor's intentional citizen reassignment after fixed food rises.
    game.cities.get_mut(&cities[0]).unwrap().pop = 0;

    // A belief that hands the city Food frees its citizens to leave Food
    // tiles for specialist slots, exactly as Civ VI's citizen manager does,
    // so total city Food is not a clean measure of what the belief granted.
    // Measure the belief's own contribution: city yield minus the yield of
    // whichever tiles the plan settled on.
    let worked_food = |game: &Game| -> f64 {
        game.city_citizen_plan(cities[0])
            .worked_tiles
            .iter()
            .map(|pos| game.player_tile_yields(0, *pos, &game.map.tiles[pos]).food)
            .sum()
    };
    let worked_culture = |game: &Game| -> f64 {
        game.city_citizen_plan(cities[0])
            .worked_tiles
            .iter()
            .map(|pos| game.player_tile_yields(0, *pos, &game.map.tiles[pos]).culture)
            .sum()
    };
    let baseline_yields = game.city_yields(cities[0]);
    let baseline_housing = game.city_housing(&game.cities[&cities[0]]);
    let baseline_worked_food = worked_food(&game);
    let baseline_worked_culture = worked_culture(&game);
    game.players[0].religion_beliefs = vec!["feed_the_world".to_string()];
    assert_eq!(
        game.city_yields(cities[0]).food - worked_food(&game),
        baseline_yields.food - baseline_worked_food + 6.0
    );
    assert_eq!(
        game.city_housing(&game.cities[&cities[0]]),
        baseline_housing + 4.0
    );

    game.players[0].religion_beliefs = vec!["choral_music".to_string()];
    assert!(
        (game.city_yields(cities[0]).culture - worked_culture(&game)
            - (baseline_yields.culture - baseline_worked_culture + 6.0))
            .abs()
            < 1e-9
    );

    game.players[0].religion_beliefs = vec!["work_ethic".to_string()];
    let holy_site_adjacency = game.district_yields(crate::name!("holy_site"), holy_site).faith;
    assert!(holy_site_adjacency >= 1.0);
    assert!(
        (game.city_yields(cities[0]).production
            - (baseline_yields.production + holy_site_adjacency))
            .abs()
            < 1e-9
    );

    let baseline_amenities = game.city_local_amenities(&game.cities[&cities[0]]);
    place_district(&mut game, cities[0], "campus");
    game.players[0].religion_beliefs = vec!["zen_meditation".to_string()];
    assert_eq!(
        game.city_local_amenities(&game.cities[&cities[0]]),
        baseline_amenities + 1
    );

    game.routes.push(TradeRoute {
        origin: cities[0],
        dest: cities[1],
        owner: 0,
        ends: game.turn + 30,
    });
    game.players[0].religion_beliefs.clear();
    let baseline_route_gold = game.city_yields(cities[0]).gold;
    game.players[0].religion_beliefs = vec!["religious_community".to_string()];
    assert!((game.city_yields(cities[0]).gold - baseline_route_gold - 6.0).abs() < 1e-9);
}

/// Civ VI grants a majority religion only above half of a city's Citizens,
/// and atheist/pantheon Pressure competes for those Citizens. Without it a
/// trickle of passive Pressure flipped cities outright, which cascaded
/// into Religious victories long before missionaries had done any work.
#[test]
fn atheist_pressure_gates_the_majority_religion() {
    let (mut game, cities) = game_with_capitals(91_764);
    let religion = establish_religion(&mut game, cities[0], &[]);
    let target = cities[1];
    game.cities.get_mut(&target).unwrap().pop = 4;
    game.cities.get_mut(&target).unwrap().pressure.clear();
    assert_eq!(game.cities[&target].atheist_pressure, 50.0);

    // Passive Pressure that merely clears the old flat threshold now buys
    // exactly half the city, which is not a majority.
    game.cities
        .get_mut(&target)
        .unwrap()
        .pressure
        .insert(religion.clone(), 50.0);
    assert_eq!(game.city_religion(&game.cities[&target]), None);
    assert_eq!(game.religious_followers_in_city(&game.cities[&target], &religion), 2.0);

    // One more point of Pressure carries the majority.
    game.cities
        .get_mut(&target)
        .unwrap()
        .pressure
        .insert(religion.clone(), 51.0);
    assert_eq!(
        game.city_religion(&game.cities[&target]),
        Some(religion.as_str())
    );

    // Growth reinforces whichever side already holds the city: the new
    // Citizen joins the majority faith, and the atheist pool otherwise.
    let converted = game.cities[&target].atheist_pressure;
    game.apply_growth_pressure(target);
    assert_eq!(game.cities[&target].atheist_pressure, converted);
    assert_eq!(game.cities[&target].pressure[&religion], 101.0);

    game.cities.get_mut(&target).unwrap().pressure.clear();
    let atheists = game.cities[&target].atheist_pressure;
    game.apply_growth_pressure(target);
    assert_eq!(game.cities[&target].atheist_pressure, atheists + 50.0);
}

/// A Trade Route carries religion in both directions but not equally:
/// `RELIGION_SPREAD_TRADE_ROUTE_PRESSURE_FOR_DESTINATION` is 1.0 and
/// `..._FOR_ORIGIN` is 0.5, so sending a Trader *to* a city pressures it
/// twice as hard as receiving one from it. CIVVIS paid a flat 0.5 both
/// ways, halving the main non-missionary route to a foreign city.
#[test]
fn a_trade_route_pressures_its_destination_twice_as_hard_as_its_origin() {
    let (mut game, cities) = game_with_capitals(91_764);
    let religion = establish_religion(&mut game, cities[0], &[]);
    let target = cities[1];

    // Passive city-to-city Pressure is whatever the map's spacing gives;
    // measure it once so the route's share can be read as a delta.
    let gain = |game: &mut Game| {
        game.cities.get_mut(&target).unwrap().pressure.clear();
        game.process_pressure(1);
        game.cities[&target]
            .pressure
            .get(&religion)
            .copied()
            .unwrap_or(0.0)
    };
    let passive = gain(&mut game);

    game.routes.push(TradeRoute {
        origin: cities[0],
        dest: target,
        owner: 0,
        ends: game.turn + 30,
    });
    assert_eq!(
        gain(&mut game) - passive,
        1.0,
        "the destination of a route takes the full 1.0 Pressure"
    );

    game.routes.clear();
    game.routes.push(TradeRoute {
        origin: target,
        dest: cities[0],
        owner: 1,
        ends: game.turn + 30,
    });
    assert_eq!(
        gain(&mut game) - passive,
        0.5,
        "the origin of a route takes back only half"
    );
}

#[test]
fn the_army_combat_beliefs_do_not_reach_an_apostle() {
    // JUST_WAR_REQUIREMENTS and DEFENDER_OF_FAITH_REQUIREMENTS are the same
    // TEST_ALL set bar one argument, and both open with NOT CLASS_RELIGIOUS
    // and NOT CLASS_SUPPORT. They are army beliefs: an Apostle carries
    // nothing from either into theological combat, where CIVVIS was paying
    // both.
    let (mut game, cities) = game_with_capitals(91_765);
    let religion = establish_religion(&mut game, cities[0], &[]);
    for city in [cities[0], cities[1]] {
        let city = game.cities.get_mut(&city).unwrap();
        city.pressure = BTreeMap::from([(religion.clone(), 100.0)]);
        city.atheist_pressure = 0.0;
    }
    let foreign = game.cities[&cities[1]].pos;

    let apostle = game.spawn_unit("apostle", 0, foreign);
    game.units.get_mut(&apostle).unwrap().religion = Some(religion.clone());
    let bare = game.theological_strength(&game.units[&apostle]);

    game.players[0].religion_beliefs = vec!["just_war".to_string()];
    assert_eq!(
        game.theological_strength(&game.units[&apostle]),
        bare,
        "Just War is an army belief"
    );
    game.players[0].religion_beliefs = vec!["defender_of_the_faith".to_string()];
    assert_eq!(
        game.theological_strength(&game.units[&apostle]),
        bare,
        "so is Defender of the Faith"
    );

    // The army still gets it, so the exclusion is narrow rather than a
    // blanket removal.
    let warrior = game.spawn_unit("warrior", 0, foreign);
    let plain = game.unit_unembarked_strength(&game.units[&warrior]);
    game.players[0].religion_beliefs = vec!["just_war".to_string()];
    assert_eq!(game.unit_unembarked_strength(&game.units[&warrior]), plain + 10.0);
}

#[test]
fn founder_unity_combat_and_loyalty_beliefs_use_runtime_city_state() {
    let (mut game, cities) = game_with_capitals(91_764);
    let religion = establish_religion(&mut game, cities[0], &[]);
    // Both cities are fully evangelized: no atheists remain to dilute the
    // three-of-four follower split these founder beliefs are paid on.
    for city in [cities[0], cities[1]] {
        let city = game.cities.get_mut(&city).unwrap();
        city.pop = 4;
        city.pressure =
            BTreeMap::from([(religion.clone(), 75.0), ("Other Faith".to_string(), 25.0)]);
        city.atheist_pressure = 0.0;
    }

    game.players[0].religion_beliefs = vec!["tithe".to_string()];
    assert_eq!(game.founder_belief_yields(0).gold, 6.0);
    game.players[0].religion_beliefs = vec!["world_church".to_string()];
    assert_eq!(game.founder_belief_yields(0).culture, 1.0);
    game.players[0].religion_beliefs = vec!["pilgrimage".to_string()];
    assert_eq!(game.founder_belief_yields(0).faith, 4.0);

    // Lay Ministry: +1 Faith per Holy Site and +1 Culture per Theater
    // Square in cities following the religion (BELIEF_YIELD_PER_DISTRICT);
    // Sacred Places: +2 of every yield per following city with a Wonder
    // (BELIEF_YIELD_PER_CITY_WITH_WONDER). Both are founder income.
    let holy_site = game
        .cities[&cities[0]]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&cities[0]].pos)
        .unwrap();
    game.map.tiles.get_mut(&holy_site).unwrap().district = Some(crate::name!("holy_site"));
    game.map.tiles.get_mut(&holy_site).unwrap().improvement = None;
    game.cities
        .get_mut(&cities[0])
        .unwrap()
        .districts
        .insert(crate::name!("holy_site"), holy_site);
    game.players[0].religion_beliefs = vec!["lay_ministry".to_string()];
    assert_eq!(game.founder_belief_yields(0).faith, 1.0);
    assert_eq!(game.founder_belief_yields(0).culture, 0.0);
    let second_center = game.cities[&cities[1]].pos;
    game.cities
        .get_mut(&cities[1])
        .unwrap()
        .wonders
        .insert(crate::name!("stonehenge"), second_center);
    game.players[0].religion_beliefs = vec!["sacred_places".to_string()];
    let sacred = game.founder_belief_yields(0);
    assert_eq!((sacred.faith, sacred.culture, sacred.science, sacred.gold), (2.0, 2.0, 2.0, 2.0));
    // Divine Inspiration is a follower belief: +4 Faith per Wonder in the
    // city that follows, paid in that city's own yields — even a rival's.
    game.players[0].religion_beliefs = vec!["divine_inspiration".to_string()];
    let without = {
        game.players[0].religion_beliefs.clear();
        game.city_yields(cities[1]).faith
    };
    game.players[0].religion_beliefs = vec!["divine_inspiration".to_string()];
    assert_eq!(game.city_yields(cities[1]).faith, without + 4.0);
    // Reliquaries triples a Relic's Faith in a following city.
    game.players[0].religion_beliefs.clear();
    game.grant_great_work(1, "relic", 0, "test");
    let plain_relic = game.city_yields(cities[1]).faith;
    game.players[0].religion_beliefs = vec!["reliquaries".to_string()];
    let housed = game
        .housed_great_works(1)
        .get(&cities[1])
        .and_then(|works| works.get("relic").copied())
        .unwrap_or(0) as f64;
    assert_eq!(game.city_yields(cities[1]).faith, plain_relic + 3.0 * 4.0 * housed);
    assert!(housed >= 1.0, "the relic has to be housed for the belief to show");

    game.players[1].is_minor = true;
    game.players[0].religion_beliefs = vec!["religious_unity".to_string()];
    game.award_religious_unity_envoys();
    game.award_religious_unity_envoys();
    assert_eq!(game.players[0].envoys, vec![(1, 1)]);

    let warrior = game.spawn_unit("warrior", 0, game.cities[&cities[1]].pos);
    game.players[0].religion_beliefs = vec!["just_war".to_string()];
    assert_eq!(game.unit_unembarked_strength(&game.units[&warrior]), 30.0);
    game.relocate(warrior, game.cities[&cities[0]].pos);
    game.players[0].religion_beliefs = vec!["defender_of_the_faith".to_string()];
    assert_eq!(game.unit_unembarked_strength(&game.units[&warrior]), 25.0);

    let target_position = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| game.map.tiles[position].owner_city.is_none())
        .find(|position| {
            game.rules.is_passable(&game.map.tiles[position])
                && !game.rules.is_water(&game.map.tiles[position])
        })
        .unwrap();
    let target = game.found_city_for(0, target_position, Some("Loyalty Test".to_string()));
    game.cities.get_mut(&target).unwrap().loyalty = 50.0;
    let encoded = serde_json::to_string(&game).unwrap();
    let mut matching: Game = serde_json::from_str(&encoded).unwrap();
    let mut rival: Game = serde_json::from_str(&encoded).unwrap();
    matching.cities.get_mut(&target).unwrap().pressure =
        BTreeMap::from([(religion.clone(), 100.0)]);
    rival.cities.get_mut(&target).unwrap().pressure =
        BTreeMap::from([("Other Faith".to_string(), 100.0)]);
    matching.process_loyalty(0);
    rival.process_loyalty(0);
    assert_eq!(
        matching.cities[&target].loyalty - rival.cities[&target].loyalty,
        6.0
    );
}
