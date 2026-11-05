use super::*;
use crate::game::{Game, GameOptions};
use crate::rules::Rules;
use crate::setup::{MapScript, MapTopology, TacticsRules};

/// The scenario as the lobby and the launcher actually build it.
fn battle(seed: u64) -> Game {
    Game::new_with(GameOptions {
        map_script: MapScript::Trafalgar,
        map_topology: MapTopology::Flat,
        // Everything a player could ask for and a scenario refuses: a
        // city each, production, gold, research, uniques and a flag.
        tactics: TacticsRules {
            cities: 1,
            production: 90,
            gold: 90,
            turns_per_tech: 3,
            unique_units: true,
            flag: true,
            heal: false,
            ..TacticsRules::default()
        },
        civs: vec!["Nubia".to_string(), "Scythia".to_string()],
        randomize_civs: true,
        ..GameOptions::new(2, WIDTH, HEIGHT, seed, 100, 4)
    })
}

/// The chart has to be a rectangle before anything reading it by column
/// means what it says. A short row would shift every cell after it north
/// and west without any of the tables below noticing.
#[test]
fn the_chart_is_rectangular() {
    for (row, line) in CHART.iter().enumerate() {
        assert_eq!(
            line.len() as i32,
            WIDTH,
            "chart row {row} is {} cells wide, not {WIDTH}",
            line.len()
        );
        assert!(
            line.bytes()
                .all(|cell| matches!(cell, b'~' | b':' | b'.' | b'#')),
            "chart row {row} has a cell that is not sea, shoal, shore or inland"
        );
    }
}

/// Sixty ships of the line, which is the count both fleets are known by:
/// twenty-seven against thirty-three. If either table is edited into a
/// different battle, this is where it is caught.
#[test]
fn both_fleets_are_the_size_history_records() {
    assert_eq!(BRITISH.len(), 27, "Nelson had twenty-seven of the line");
    assert_eq!(COMBINED.len(), 33, "the Combined Fleet had thirty-three");
}

/// Every ship rates something a ship of the line rated, and the two fleets
/// carry the rates history gave them. A gun figure typed with a digit
/// missing would otherwise silently move a ship between bands.
#[test]
fn every_ship_rates_what_a_ship_of_the_line_rated() {
    for ship in BRITISH.iter().chain(COMBINED.iter()) {
        assert!(
            (64..=136).contains(&ship.guns),
            "{} rates {} guns, which is not a ship of the line",
            ship.name,
            ship.guns
        );
    }
    let heaviest = COMBINED.iter().max_by_key(|ship| ship.guns).unwrap();
    assert_eq!(
        heaviest.guns, 136,
        "the Santisima Trinidad was the largest ship afloat"
    );
    assert!(heaviest.name.starts_with("Santisima Trinidad"));
    // First rates: three British against four. The Combined Fleet was the
    // heavier as well as the larger, which is the position Nelson chose to
    // attack and the reason it is worth modelling at all.
    let first_rates = |fleet: &[Ship]| fleet.iter().filter(|ship| ship.guns >= 100).count();
    assert_eq!(first_rates(&BRITISH), 3);
    assert_eq!(first_rates(&COMBINED), 4);
}

/// A ship's rate reaches the board as promotions, by one rule applied to
/// both fleets — and every promotion it grants has to be one the Frigate
/// could take and one that does something in *this* battle. Most of the
/// naval tree is anti-land, anti-district, anti-air or healing, and a
/// grant out of those branches would look like modelling while changing
/// nothing.
#[test]
fn a_ships_rate_grants_promotions_that_do_something_here() {
    let rules = Rules::embedded();
    // What a fleet action can actually feel: a bigger broadside against
    // ships, and reach. Nothing else on the board is a land unit, a
    // district or an aircraft, and `unit_heal_rate` returns 0 on every
    // Tactics map, so a healing promotion is inert here by construction.
    let live = ["ranged_vs_naval", "ranged_vs_units", "range"];
    for guns in [64u16, 74, 80, 98, 100, 112, 136] {
        let granted = rate_promotions(guns);
        let held: BTreeSet<&str> = granted.iter().copied().collect();
        assert_eq!(
            held.len(),
            granted.len(),
            "{guns} guns grants a promotion twice"
        );
        for name in granted {
            let spec = rules
                .promotions
                .get(&crate::name::Name::new(name))
                .unwrap_or_else(|| panic!("{name} is not a promotion in the ruleset"));
            // The Frigate's own tree, so these are promotions this ship
            // could be offered rather than borrowed from another class.
            assert_eq!(
                spec.class, "naval_ranged",
                "{name} is a {} promotion, not one a ship of the line can take",
                spec.class
            );
            assert!(
                spec.effects
                    .keys()
                    .any(|effect| live.contains(&effect.as_str())),
                "{name} is granted at {guns} guns but its effects {:?} do nothing in a \
                     fleet action",
                spec.effects.keys().collect::<Vec<_>>()
            );
        }
    }

    // The ladder itself: a heavier ship never carries less, and never
    // loses something a lighter one has.
    assert!(rate_promotions(64).is_empty(), "a 64 is the plain ship");
    let ladder = [64u16, 74, 80, 98, 100, 112, 136];
    for pair in ladder.windows(2) {
        let (lighter, heavier) = (rate_promotions(pair[0]), rate_promotions(pair[1]));
        assert!(
            heavier.len() >= lighter.len() && lighter.iter().all(|name| heavier.contains(name)),
            "{} guns does not carry everything {} guns does",
            pair[1],
            pair[0]
        );
    }
    // The bands are read off the gun figure alone, so the same rate gets
    // the same ship whichever line she was in.
    assert_eq!(rate_promotions(74), rate_promotions(98));
    assert_eq!(rate_promotions(100), rate_promotions(136));
    // And a first rate is a ship of the line and no more. The band that
    // would have separated her was measured and removed — see
    // `rate_promotions` for the runs. Asserted rather than assumed,
    // because the tempting fix is to give the biggest ships the only
    // remaining lever (+1 attack range) and that lever decides the battle.
    assert_eq!(
        rate_promotions(136),
        rate_promotions(74),
        "a three-decker has been given something a 74 has not; re-measure before keeping it"
    );
    assert!(
        !rate_promotions(136).contains(&"coincidence_rangefinding"),
        "outranging the whole board turned this scenario into a rout inside thirty turns"
    );
}

/// Every ship is on open water, on the chart, and alone on her tile. Only
/// one military unit stands on a hex, so a duplicate coordinate would
/// silently drop a ship from the board — and a ship placed on the shore or
/// on the shoals could not be there at all.
#[test]
fn every_ship_is_afloat_and_on_her_own_tile() {
    let mut taken: BTreeSet<(i32, i32)> = BTreeSet::new();
    for Ship {
        at: (col, row),
        name: ship,
        ..
    } in BRITISH.iter().chain(COMBINED.iter())
    {
        assert!(
            (0..WIDTH).contains(col) && (0..HEIGHT).contains(row),
            "{ship} is off the chart at ({col}, {row})"
        );
        assert_eq!(
            cell(*col, *row),
            Some(b'~'),
            "{ship} is not on open water at ({col}, {row})"
        );
        assert!(
            taken.insert((*col, *row)),
            "{ship} shares a tile at ({col}, {row})"
        );
    }
    assert_eq!(
        taken.len(),
        60,
        "sixty ships of the line were in the action"
    );
}

/// The seats are the two commanders-in-chief, in that order. Britain first
/// is the whole of "Britain moves first", so it is asserted rather than
/// left to the order two constants happen to be written in.
#[test]
fn the_seats_are_victory_then_bucentaure() {
    assert_eq!(FLAGSHIPS[0], BRITISH[0].at);
    assert!(BRITISH[0].name.starts_with("Victory"));
    assert_eq!(FLAGSHIPS[1], COMBINED[11].at);
    assert!(COMBINED[11].name.starts_with("Bucentaure"));
}

/// The two British columns start west of the Combined Fleet's line, and
/// clear of it. A scenario whose fleets began interleaved would open with
/// the approach already over — and the approach is the battle.
#[test]
fn the_british_are_to_windward_with_water_still_to_cross() {
    let van_and_line = COMBINED.iter().map(|ship| ship.at.0).min().unwrap();
    let leading_briton = BRITISH.iter().map(|ship| ship.at.0).max().unwrap();
    assert!(
        leading_briton < van_and_line,
        "the leading British ship is at column {leading_briton}, not west of the \
             Combined Fleet's nearest ship at {van_and_line}"
    );
    // Collingwood beat Nelson into action, so his column starts nearer.
    let royal_sovereign = &BRITISH[11];
    assert!(royal_sovereign.name.starts_with("Royal Sovereign"));
    assert!(
        royal_sovereign.at.0 > BRITISH[0].at.0,
        "Royal Sovereign should start ahead of Victory"
    );
}

/// The Combined Fleet's line has to be sailable along its whole length, or
/// the crescent is really two fleets with a wall between them.
#[test]
fn the_enemy_line_stands_in_open_water_from_van_to_rear() {
    let rows: Vec<i32> = COMBINED.iter().map(|ship| ship.at.1).collect();
    let (top, bottom) = (*rows.iter().min().unwrap(), *rows.iter().max().unwrap());
    assert!(
        bottom - top >= 20,
        "the line should span most of the map north to south"
    );
    for row in top..=bottom {
        assert!(
            COMBINED.iter().any(|ship| ship.at.1 == row),
            "the line has a gap at row {row}"
        );
    }
}

/// The cape is under the Combined Fleet's lee, which is the position they
/// could not retreat out of. Shoals exist, and they are to seaward of the
/// headland where a fleet driven east would strike them first.
#[test]
fn cape_trafalgar_and_its_shoals_are_on_the_lee_side() {
    let shoals: Vec<(i32, i32)> = (0..HEIGHT)
        .flat_map(|row| (0..WIDTH).map(move |col| (col, row)))
        .filter(|(col, row)| cell(*col, *row) == Some(b':'))
        .collect();
    assert!(!shoals.is_empty(), "the cape has no shoals");
    for (col, row) in &shoals {
        assert!(
            !is_sea(col + 2, *row) || cell(col + 2, *row) == Some(b':'),
            "the shoal at ({col}, {row}) has open water behind it rather than the cape"
        );
    }
    let rear = COMBINED.last().unwrap().at;
    assert!(
        shoals.iter().any(|(col, _)| *col > rear.0),
        "the shoals should lie to leeward of the Combined Fleet's rear"
    );
}

/// The whole scenario, built the way the lobby builds it: sixty ships of
/// the line on the water, twenty-seven to twenty-three, each one on the
/// tile its table names. This is the test the tables above exist to
/// support — they describe a battle, and this is the battle arriving.
#[test]
fn the_two_fleets_reach_the_board_ship_for_ship() {
    let game = battle(1_805);
    for (pid, fleet) in [(0usize, &BRITISH[..]), (1, &COMBINED[..])] {
        for Ship {
            at: (col, row),
            name: ship,
            ..
        } in fleet
        {
            let pos = hex::offset_to_axial(*col, *row);
            let standing = game.units_at(pos);
            assert_eq!(standing.len(), 1, "no ship stands where {ship} should");
            let unit = &game.units[&standing[0]];
            assert_eq!(unit.owner, pid, "{ship} is flying the wrong flag");
            assert_eq!(
                unit.kind.as_str(),
                SHIP_OF_THE_LINE,
                "{ship} is the wrong ship"
            );
        }
    }
    let afloat = |pid: usize| game.player_unit_ids(pid).len();
    assert_eq!(afloat(0), BRITISH.len());
    assert_eq!(afloat(1), COMBINED.len());
}

/// A ship's rate reaches the water, and it reaches combat.
///
/// The table and the ladder are both checked above; this is the wiring
/// between them and the engine. The last assertion is the one that
/// matters — a promotion the combat layer never reads would pass every
/// other test in this file and change nothing on the board.
/// Nine flag officers, where history put them, rated on one scale.
///
/// The counts are the load-bearing part: three British flags against six
/// in the Combined Fleet is why an admiral's rating is spent on his
/// fleet's flanking rather than on his own hull — see
/// `fleet_flanking_bonus_pct`. If a rating is ever edited, this says out
/// loud that the larger fleet still holds more flags.
#[test]
fn the_nine_flag_officers_are_aboard_the_ships_they_flew_in() {
    let flags = |fleet: &'static [Ship]| -> Vec<&'static Ship> {
        fleet.iter().filter(|ship| ship.stars > 0).collect()
    };
    let (british, combined) = (flags(&BRITISH), flags(&COMBINED));
    assert_eq!(british.len(), 3, "Nelson, Collingwood and Northesk");
    assert_eq!(
        combined.len(),
        6,
        "Villeneuve, Gravina, Alava, Magon, Dumanoir, Cisneros"
    );
    assert!(
        combined.len() > british.len(),
        "the Combined Fleet held more flags, which is the whole reason a \
             per-flagship bonus was rejected"
    );
    for ship in british.iter().chain(combined.iter()) {
        assert!(
            (2..=5).contains(&ship.stars),
            "{} is rated {} stars, off the 2-to-5 scale",
            ship.name,
            ship.stars
        );
    }
    let nelson = commander_in_chief(0);
    let villeneuve = commander_in_chief(1);
    assert!(nelson.name.contains("Nelson") && nelson.stars == 5);
    assert!(villeneuve.name.contains("Villeneuve") && villeneuve.stars == 2);

    // The threshold is what turns more Combined flags into less Combined
    // advantage, so it is asserted as the counts rather than as a rule.
    let worth = |fleet: &'static [Ship]| -> usize {
        fleet
            .iter()
            .filter(|ship| admiral_formation(ship.stars) > 0)
            .count()
    };
    assert_eq!(worth(&BRITISH), 2, "Nelson and Collingwood");
    assert_eq!(worth(&COMBINED), 1, "Gravina alone");
    assert!(
        worth(&BRITISH) > worth(&COMBINED),
        "the side with fewer flags must still come out ahead on the ones that count"
    );
    assert_eq!(
        admiral_formation(3),
        0,
        "three stars is not a fighting tier"
    );
    assert_eq!(admiral_formation(4), 1);
    assert_eq!(admiral_formation(5), 1);
}

/// The admirals reach the board: every flagship sails a tile further, and
/// the ones worth a fighting tier fire ten heavier.
///
/// The broadside is fired for real rather than read off a strength
/// function, for the same reason the rate test does it: this whole feature
/// once ran through `flanking_bonus`, which the ranged attack path never
/// consults, and every table-level assertion passed while the board did
/// not move at all.
#[test]
fn an_admirals_stars_reach_his_flagship() {
    let game = battle(1_805);
    let plain = game.rules.units[SHIP_OF_THE_LINE].moves;
    for (pid, fleet) in [(0usize, &BRITISH[..]), (1, &COMBINED[..])] {
        for ship in fleet {
            let uid = game.units_at(hex::offset_to_axial(ship.at.0, ship.at.1))[0];
            let flagship = ship.stars > 0;
            let moves = game.unit_max_moves(uid);
            let expected = plain
                + if flagship {
                    ADMIRAL_MOVEMENT_BONUS
                } else {
                    0.0
                };
            assert!(
                (moves - expected).abs() < 1e-9,
                "{} (seat {pid}) has {moves} movement, not {expected}",
                ship.name
            );
            assert_eq!(
                game.units[&uid].formation,
                admiral_formation(ship.stars),
                "{} is in the wrong fighting tier",
                ship.name
            );
        }
    }

    // Two ships alike in every way but the flag aboard, in separate copies
    // of the same battle, firing on the same enemy from the same water.
    let one_broadside = |stars: u8| -> i32 {
        let mut game = battle(1_805);
        let (from, at) = (hex::offset_to_axial(4, 2), hex::offset_to_axial(5, 2));
        let firing = game.spawn_unit(SHIP_OF_THE_LINE, 0, from);
        let struck = game.spawn_unit(SHIP_OF_THE_LINE, 1, at);
        let unit = game.units.get_mut(&firing).unwrap();
        unit.promotions.extend(
            rate_promotions(100)
                .iter()
                .map(|name| crate::name::Name::new(name)),
        );
        unit.formation = admiral_formation(stars);
        game.apply(
            0,
            &crate::game::Action::Ranged {
                unit: firing,
                target: at,
            },
        )
        .expect("a ship of the line can fire on an enemy alongside");
        game.units[&struck].hp
    };
    let (nelson, villeneuve) = (one_broadside(5), one_broadside(2));
    assert!(
        nelson < villeneuve,
        "a five-star admiral's ship left {nelson} hit points where a two-star's left \
             {villeneuve}"
    );
    assert_eq!(
        one_broadside(4),
        nelson,
        "four and five stars are the same tier"
    );
    assert_eq!(
        one_broadside(3),
        villeneuve,
        "three stars is not a fighting tier"
    );
}

#[test]
fn a_ship_of_the_line_fights_heavier_than_a_sixty_four() {
    let game = battle(1_805);
    let afloat = |at: (i32, i32)| {
        let standing = game.units_at(hex::offset_to_axial(at.0, at.1));
        &game.units[&standing[0]]
    };
    for ship in BRITISH.iter().chain(COMBINED.iter()) {
        let unit = afloat(ship.at);
        let expected: BTreeSet<String> = rate_promotions(ship.guns)
            .iter()
            .map(|name| (*name).to_string())
            .collect();
        let held: BTreeSet<String> = unit
            .promotions
            .iter()
            .map(|name| name.to_string())
            .collect();
        assert_eq!(held, expected, "{} carries the wrong rate", ship.name);
        assert_eq!(
            unit.level,
            1 + expected.len() as i32,
            "{}'s level",
            ship.name
        );
    }

    // Santisima Trinidad, 136, against San Leandro, 64 — the heaviest and
    // the lightest ship in the same line, so nothing but the rate differs.
    let trinidad = afloat(COMBINED[10].at);
    let leandro = afloat(COMBINED[15].at);
    assert!(COMBINED[10].name.starts_with("Santisima Trinidad"));
    assert!(COMBINED[15].name.starts_with("San Leandro"));
    assert_eq!(trinidad.kind, leandro.kind, "the two differ by rate alone");

    // Reach is deliberately equal: every ship on the board fires two
    // tiles, whatever she rates.
    for at in [trinidad.id, leandro.id, afloat(COMBINED[12].at).id] {
        assert_eq!(
            game.unit_attack_range(at),
            2,
            "reach is not a rate difference here"
        );
    }

    // And the broadside is heavier. Fired for real rather than read off a
    // strength function, because the bonus is applied inside the attack
    // and a promotion the combat layer never consults would satisfy every
    // other assertion in this file while changing nothing on the board.
    //
    // Two identical ships on the same empty water at the top of the chart,
    // in separate copies of the same battle, so the only difference
    // between the two firings is the rate of the ship firing.
    let one_broadside = |rate: u16| -> i32 {
        let mut game = battle(1_805);
        let (from, at) = (hex::offset_to_axial(4, 2), hex::offset_to_axial(5, 2));
        assert!(game.units_at(from).is_empty() && game.units_at(at).is_empty());
        let firing = game.spawn_unit(SHIP_OF_THE_LINE, 0, from);
        let struck = game.spawn_unit(SHIP_OF_THE_LINE, 1, at);
        let promotions: Vec<crate::name::Name> = rate_promotions(rate)
            .iter()
            .map(|name| crate::name::Name::new(name))
            .collect();
        game.units
            .get_mut(&firing)
            .unwrap()
            .promotions
            .extend(promotions);
        game.apply(
            0,
            &crate::game::Action::Ranged {
                unit: firing,
                target: at,
            },
        )
        .expect("a ship of the line can fire on an enemy alongside");
        game.units[&struck].hp
    };
    let (heavy, light) = (one_broadside(74), one_broadside(64));
    assert!(
        heavy < light,
        "a ship of the line's broadside left {heavy} hit points where a 64 left {light}"
    );
}

/// Britain moves first. Seat 0 is Britain, seat 0 opens the game, and
/// nothing about the roster the caller asked for changes either.
#[test]
fn britain_holds_the_first_seat_and_the_first_turn() {
    let game = battle(1_805);
    assert_eq!(game.players[0].civ, "England");
    assert_eq!(game.players[1].civ, "France");
    assert_eq!(game.current, 0, "Britain moves first");
    assert_eq!(game.turn, 1);
}

/// The seed rolls the map on every other script. Here it must move
/// nothing: two launches of a scenario are the same battle or the
/// scenario is not one.
#[test]
fn two_seeds_produce_the_same_battle() {
    let first = battle(1_805);
    let second = battle(20_261_021);
    let fleet = |game: &Game| {
        let mut ships: Vec<(usize, (i32, i32))> = game
            .units
            .values()
            .map(|unit| (unit.owner, hex::axial_to_offset(unit.pos.0, unit.pos.1)))
            .collect();
        ships.sort();
        ships
    };
    assert_eq!(
        fleet(&first),
        fleet(&second),
        "the fleets moved with the seed"
    );
    let ground = |game: &Game| {
        let mut chart: Vec<((i32, i32), String, Option<String>)> = game
            .map
            .tiles
            .iter()
            .map(|(pos, tile)| {
                (
                    hex::axial_to_offset(pos.0, pos.1),
                    tile.terrain.to_string(),
                    tile.feature.map(|feature| feature.to_string()),
                )
            })
            .collect();
        chart.sort();
        chart
    };
    assert_eq!(
        ground(&first),
        ground(&second),
        "the chart moved with the seed"
    );
}

/// The board is the chart: the sea both fleets manoeuvre over is sailable
/// coast, Cape Trafalgar is dry land, and the shoals are reef. A scenario
/// that quietly grew ice or an island would still pass every table test
/// above and be unplayable.
#[test]
fn the_board_is_the_chart_that_was_drawn() {
    let game = battle(1_805);
    let (mut sea, mut shore, mut shoal) = (0, 0, 0);
    for row in 0..HEIGHT {
        for col in 0..WIDTH {
            let tile = &game.map.tiles[&hex::offset_to_axial(col, row)];
            let feature = tile.feature.map(|feature| feature.to_string());
            match cell(col, row) {
                Some(b'~') => {
                    assert_eq!(tile.terrain.as_str(), "coast", "({col}, {row}) is not sea");
                    assert_eq!(feature, None, "({col}, {row}) grew something in open water");
                    sea += 1;
                }
                Some(b':') => {
                    assert_eq!(tile.terrain.as_str(), "coast");
                    assert_eq!(
                        feature.as_deref(),
                        Some("reef"),
                        "({col}, {row}) is not shoal"
                    );
                    shoal += 1;
                }
                _ => {
                    assert!(
                        !game.rules.is_water(tile),
                        "({col}, {row}) should be the Andalusian shore"
                    );
                    shore += 1;
                }
            }
        }
    }
    assert_eq!(sea + shoal + shore, WIDTH * HEIGHT);
    assert!(
        shoal > 0 && shore > 0 && sea > shore * 8,
        "{sea} sea, {shoal} shoal, {shore} shore"
    );
}

/// A scenario overrules the arena economy it was handed. Nothing was
/// produced, bought, researched or defended at Trafalgar, and a city or a
/// flag on this water would be a different battle — so the request the
/// fixture makes for all of them is refused, and the two things that are
/// only about how long you want to play survive it.
#[test]
fn the_scenario_fixes_the_economy_it_is_fought_under() {
    let game = battle(1_805);
    assert_eq!(game.tactics.cities, 0);
    assert_eq!(game.tactics.production, 0);
    assert_eq!(game.tactics.gold, 0);
    assert_eq!(game.tactics.turns_per_tech, 0);
    assert!(!game.tactics.unique_units);
    assert!(!game.tactics.flag);
    assert!(!game.tactics.fog);
    assert!(game.cities.is_empty(), "no city stands on this water");
    assert!(
        game.arena_flags.is_empty(),
        "no flag is planted on this water"
    );
    // Still the caller's to choose, because neither is a claim about 1805.
    assert_eq!(game.tactics.turn_limit, TacticsRules::default().turn_limit);
    assert_eq!(game.tactics.best_of, TacticsRules::default().best_of);
    // A Tactics map seats no city-states whatever the request asked for.
    // The dormant Free Cities seat every world reserves is not one.
    assert_eq!(
        game.players
            .iter()
            .filter(|player| player.is_minor && !player.is_free_city)
            .count(),
        0
    );
}

/// The two sides are at war and know it, and every ship can reach the
/// fighting. A fleet that opened at peace, or one walled off from the
/// enemy by unsailable water, would sit still for a hundred turns and
/// report a draw.
#[test]
fn both_fleets_open_at_war_with_the_water_between_them_sailable() {
    let game = battle(1_805);
    assert!(game.is_at_war(0, 1), "the fleets must open in action");
    let victory = game.units_at(hex::offset_to_axial(FLAGSHIPS[0].0, FLAGSHIPS[0].1))[0];
    let bucentaure = hex::offset_to_axial(FLAGSHIPS[1].0, FLAGSHIPS[1].1);
    assert!(
        game.route_step(victory, bucentaure, 0).is_some(),
        "Victory cannot bear down on Bucentaure"
    );
    // And the far corners of the action are connected too: Africa is off
    // on her own in the north and has to come the length of the line.
    let africa_at = BRITISH.last().unwrap().at;
    let africa = game.units_at(hex::offset_to_axial(africa_at.0, africa_at.1))[0];
    let rear = COMBINED.last().unwrap().at;
    assert!(
        game.route_step(africa, hex::offset_to_axial(rear.0, rear.1), 0)
            .is_some(),
        "Africa cannot reach the enemy rear"
    );
}
