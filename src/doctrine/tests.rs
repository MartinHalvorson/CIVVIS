use super::*;
use crate::elo::builtin_ai;

/// A converted position must build the very board the position builds:
/// same tiles, same units, same hit points. The whole curriculum now
/// plays through the owned board, so this is what keeps every recorded
/// arena number comparable with the next one.
#[test]
fn a_converted_position_builds_the_same_board_as_the_position() {
    for spec in POSITIONS {
        let from_position = build(spec, 5).expect("buildable");
        let from_board = build_engagement(&Engagement::from(spec), 5).expect("buildable");
        let units = |g: &Game| -> Vec<(usize, String, Pos, i32)> {
            let mut out: Vec<_> = g
                .units
                .values()
                .map(|unit| (unit.owner, unit.kind.to_string(), unit.pos, unit.hp))
                .collect();
            out.sort();
            out
        };
        assert_eq!(
            units(&from_position),
            units(&from_board),
            "{} seats differ",
            spec.id
        );
        for (pos, tile) in &from_position.map.tiles {
            let other = from_board.map.get(*pos).expect("same board");
            assert_eq!(
                tile.terrain, other.terrain,
                "{} terrain at {pos:?}",
                spec.id
            );
            assert_eq!(tile.hills, other.hills, "{} hills at {pos:?}", spec.id);
            assert_eq!(
                tile.feature, other.feature,
                "{} feature at {pos:?}",
                spec.id
            );
        }
        assert!(!from_board.tactics.heal, "the curriculum does not heal");
    }
}

/// A file of boards reads back as the boards that were written, and a
/// board that names a unit the ruleset lacks is refused by name.
#[test]
fn engagements_round_trip_through_json_and_refuse_a_bad_unit() {
    let boards = Engagement::curriculum();
    let text = Engagement::to_json(&boards);
    let back = Engagement::from_json(&text).expect("the curriculum reads back");
    assert_eq!(back, boards);
    let mut broken = boards[0].clone();
    broken.forces[1][0].kind = "dragoon_of_nowhere".to_string();
    let error = Engagement::from_json(&Engagement::to_json(&[broken])).unwrap_err();
    assert!(error.contains("dragoon_of_nowhere"), "{error}");
}

/// A captured board keeps the geometry of the contact — every pairwise
/// distance between the seated units — along with each unit's hit
/// points, its promotions and the river crossings under it. Run over
/// several seeds so the contact's centre lands on odd rows as well as
/// even ones: the offset board shifts odd rows, and a window that moved
/// by an odd row count would bend every distance in it.
#[test]
fn a_captured_engagement_keeps_the_contacts_geometry_and_state() {
    let mut spec = Engagement::from(position("the_reserve").expect("known"));
    spec.forces[0][0].hp = 37;
    spec.forces[0][0].promotions = vec!["battlecry".to_string()];
    spec.rivers = vec![((9, 6), [true, false, false, false, false, false])];
    let mut odd_centres = 0;
    // Radius 12 covers the whole 24x14 board, so every unit of both
    // lines is inside the window and the distance multisets compare
    // whole; a tighter window would seat only the units near the
    // contact, which is the point of a window but not of this test.
    for seed in 1..=8u64 {
        let mut g = build_engagement(&spec, seed).expect("buildable");
        // Walk the two lines into contact before capturing.
        let mut ais: Vec<Box<dyn Ai>> = (0..2)
            .map(|pid| builtin_ai("advanced", seed + pid as u64))
            .collect();
        let mut board = None;
        for _ in 0..24 {
            if let Some(taken) = capture_engagement(&g, 0, 1, 12, 10, "test") {
                board = Some(taken);
                break;
            }
            let pid = g.current;
            ais[pid].take_turn(&mut g, pid);
            let _ = g.apply(pid, &crate::game::Action::EndTurn);
        }
        let board = board.expect("the reserve comes into contact within a dozen turns");
        let mut source: Vec<(usize, Pos, i32, usize)> = g
            .units
            .values()
            .filter(|unit| unit.owner < 2)
            .map(|unit| (unit.owner, unit.pos, unit.hp, unit.promotions.len()))
            .collect();
        source.sort();
        let seated: usize = board.forces.iter().map(Vec::len).sum();
        assert_eq!(
            seated,
            source.len(),
            "seed {seed}: every unit in the window is seated"
        );
        let mut source_distances: Vec<i32> = Vec::new();
        for i in 0..source.len() {
            for j in (i + 1)..source.len() {
                source_distances.push(g.wdist(source[i].1, source[j].1));
            }
        }
        source_distances.sort();
        let placed: Vec<&Placed> = board.forces.iter().flatten().collect();
        let mut board_distances: Vec<i32> = Vec::new();
        for i in 0..placed.len() {
            for j in (i + 1)..placed.len() {
                board_distances.push(hex::distance(
                    hex::offset_to_axial(placed[i].col, placed[i].row),
                    hex::offset_to_axial(placed[j].col, placed[j].row),
                ));
            }
        }
        board_distances.sort();
        let source_cells: Vec<(usize, (i32, i32))> = source
            .iter()
            .map(|unit| (unit.0, hex::axial_to_offset(unit.1 .0, unit.1 .1)))
            .collect();
        let board_cells: Vec<(i32, i32)> = placed.iter().map(|unit| (unit.col, unit.row)).collect();
        assert_eq!(
                board_distances, source_distances,
                "seed {seed}: the window bent a distance\nsource {source_cells:?}\nboard {board_cells:?}\nwraps {} width {}",
                g.map.wraps_east_west(),
                g.map.width
            );
        let wounded = board.forces[0].iter().find(|unit| unit.hp == 37);
        if source.iter().any(|unit| unit.2 == 37) {
            let wounded = wounded.expect("the wounded warrior is seated as wounded");
            assert_eq!(wounded.promotions, vec!["battlecry".to_string()]);
        }
        if placed.iter().any(|unit| unit.row % 2 == 1) {
            odd_centres += 1;
        }
        // The river under (9,6) is inside every window centred on the
        // contact; it must have crossed with its edges.
        assert!(
            board.rivers.iter().any(|(_, edges)| edges[0]),
            "seed {seed}: the river crossing was lost"
        );
        // And the board replays: it seats every unit it lists, wounded.
        let replay = build_engagement(&board, 3).expect("a captured board builds");
        assert_eq!(replay.units.len(), seated);
        assert!(
            replay.units.values().any(|unit| unit.hp == 37)
                || !source.iter().any(|unit| unit.2 == 37)
        );
    }
    assert!(odd_centres > 0, "no seed exercised the odd-row shift");
}

/// Whether a board heals is the board's own question. The curriculum
/// keeps the arena rule — nothing recovers — and a board that asks for a
/// campaign gets the neutral rate everywhere.
#[test]
fn healing_is_the_boards_question() {
    let spec = Engagement::from(position("the_reserve").expect("known"));
    let frozen = build_engagement(&spec, 4).expect("buildable");
    let uid = frozen.player_unit_ids(0)[0];
    assert_eq!(
        frozen.unit_heal_rate(uid),
        0,
        "the arena rule: permanent damage"
    );
    let campaign = build_engagement(&Engagement { heal: true, ..spec }, 4).expect("buildable");
    let uid = campaign.player_unit_ids(0)[0];
    assert!(campaign.unit_heal_rate(uid) > 0, "a healing board recovers");
    assert!(
        !crate::setup::TacticsRules::default().heal,
        "off in the stock arena"
    );
    let without: crate::setup::TacticsRules =
        serde_json::from_str(r#"{"cities":1,"production":0,"gold":0,"turns_per_tech":5}"#)
            .expect("an old save reads");
    assert!(
        !without.heal,
        "a save without the field played without healing"
    );
}

/// The harvest runs a whole small game and every board it takes is one
/// the arena can read back and seat.
#[test]
fn a_harvest_produces_boards_the_arena_can_play() {
    let small = Harvest {
        players: 2,
        width: 24,
        height: 16,
        turns: 60,
        radius: 5,
        window_turns: 8,
        cooldown: 20,
        majors_only: false,
    };
    let boards = harvest_engagements(17, &small);
    let text = Engagement::to_json(&boards);
    let back = Engagement::from_json(&text).expect("harvested boards read back");
    assert_eq!(back, boards);
    for board in &boards {
        assert!(
            board.forces.iter().all(|force| force.len() >= 2),
            "{}",
            board.id
        );
        assert!(build_engagement(board, 1).is_some(), "{} seats", board.id);
    }
}

/// Every position has to be buildable and has to seat every unit it asks
/// for, or its numbers are measuring a force that is not the one written
/// down. Cheap, and it is the failure that would be easiest to miss.
#[test]
fn every_position_seats_the_force_it_specifies() {
    for spec in POSITIONS {
        for seed in [1u64, 2, 3] {
            let game = build(spec, seed)
                .unwrap_or_else(|| panic!("{} could not be built on seed {seed}", spec.id));
            for role in 0..2 {
                assert_eq!(
                    game.player_unit_ids(role).len(),
                    spec.forces[role].len(),
                    "{} seated the wrong number of units for role {role}",
                    spec.id
                );
            }
        }
    }
}

/// The arena carries its own draw clock, and a position carries a
/// deadline. If the arena's fires first, the ledger is read on a game the
/// engine already stopped and every profile silently shortens. Assert the
/// two clocks cannot disagree.
#[test]
fn the_arenas_draw_clock_cannot_end_a_position_early() {
    for spec in POSITIONS {
        let game = build(spec, 61).expect("buildable");
        assert!(
            game.tactics.turn_limit > spec.turns,
            "{} runs {} turns under an arena clock of {}",
            spec.id,
            spec.turns,
            game.tactics.turn_limit
        );
    }
}

/// Arrival is the measurement behind "march divided, fight united", so it
/// has to be a real spread: zero when a force starts in contact together,
/// and larger for a position that deliberately holds half its army back.
/// It also must not silently drop units that never arrived — an army that
/// left half its strength in the rear would otherwise report the tightest
/// arrival on the board.
#[test]
fn arrival_measures_the_spread_of_a_force_reaching_the_enemy() {
    let close = matched_position(
        position("the_golden_bridge").expect("known"),
        71,
        "advanced",
        "advanced",
        &builtin_ai,
    );
    let staggered = matched_position(
        position("the_reserve").expect("known"),
        71,
        "advanced",
        "advanced",
        &builtin_ai,
    );
    let (Some(pocket), Some(reserve)) = (
        close.a_by_role[0].profile().arrival,
        staggered.a_by_role[1].profile().arrival,
    ) else {
        panic!("both positions must produce an arrival spread");
    };
    assert!(pocket >= 0.0 && reserve >= 0.0);
    assert!(
        reserve > pocket,
        "the far reserve ({reserve}) should arrive less together than a \
             force cornered in a pocket ({pocket})"
    );
}

/// The whole point of the foot split is that it can disagree with the
/// all-units figure — that is what lets a spread caused by cavalry
/// outriding the line be told apart from one caused by the line. Assert
/// the two are actually different measurements on a position that has
/// cavalry, and that the foot figure is drawn from fewer units.
#[test]
fn the_foot_split_is_a_different_measurement_from_the_whole_force() {
    // hammer_and_anvil is the position built around fast units: role 0
    // holds a line and sends two horsemen wide.
    let spec = position("hammer_and_anvil").expect("known");
    let result = matched_position(spec, 81, "advanced", "advanced", &builtin_ai);
    let profile = result.a_by_role[0].profile();
    let (Some(all), Some(foot)) = (profile.arrival, profile.foot_arrival) else {
        panic!("a position with both foot and horse must report both spreads");
    };
    assert!(all >= 0.0 && foot >= 0.0);
    assert!(
        (all - foot).abs() > 1e-9,
        "the foot split reproduced the whole-force figure exactly ({all}), \
             so it is not separating anything"
    );
    // And on a position with no cavalry at all the two must coincide,
    // because then every unit is foot.
    let infantry = position("the_golden_bridge").expect("known");
    let result = matched_position(infantry, 81, "advanced", "advanced", &builtin_ai);
    let profile = result.a_by_role[0].profile();
    assert_eq!(
        profile.arrival, profile.foot_arrival,
        "a force of nothing but foot must report the same spread twice"
    );
}

/// `absent` is the one form of arriving late that no difference in
/// movement points can explain away, so it has to be a real share of a
/// real denominator — every deployed unit, including any that died before
/// they ever reached the enemy.
#[test]
fn absence_is_a_share_of_the_whole_force() {
    for spec in POSITIONS {
        let result = matched_position(spec, 82, "advanced", "advanced", &builtin_ai);
        if result.skipped {
            continue;
        }
        for role in 0..2 {
            let Some(absent) = result.a_by_role[role].profile().absent else {
                panic!("{} role {role} reported no absence share", spec.id);
            };
            assert!(
                (0.0..=1.0).contains(&absent),
                "{} role {role} reported an absence share of {absent}",
                spec.id
            );
        }
    }
}

/// A correlation with a constant series is undefined, not zero. Reporting
/// 0.00 there would be the harness asserting an independence it never
/// measured — which is the same class of error as the sign test's
/// confident null.
#[test]
fn a_correlation_with_no_spread_is_nothing_rather_than_zero() {
    assert!(
        paired::correlation(&[1.0, 2.0], &[1.0, 2.0]).is_none(),
        "too few pairs"
    );
    assert!(
        paired::correlation(&[1.0, 1.0, 1.0], &[1.0, 2.0, 3.0]).is_none(),
        "a constant x has no correlation, not a zero one"
    );
    let (r, n, p) = paired::correlation(&[1.0, 2.0, 3.0, 4.0], &[2.0, 4.0, 6.0, 8.0])
        .expect("a perfect line correlates");
    assert!((r - 1.0).abs() < 1e-12, "r = {r}");
    assert_eq!(n, 4);
    assert_eq!(p, 0.0);
    let (r, _, _) = paired::correlation(&[1.0, 2.0, 3.0, 4.0], &[8.0, 6.0, 4.0, 2.0])
        .expect("a perfect inverse correlates");
    assert!((r + 1.0).abs() < 1e-12, "r = {r}");
}

/// The vanguard is the one column meant to support a causal claim, so its
/// two guarantees have to hold: it is a share of the force deployed, and
/// the instant it is taken at is upstream of essentially every engagement.
/// If that second figure were low the column would be worthless for the
/// job it exists to do.
#[test]
fn the_vanguard_is_recorded_before_the_engagement_decides_anything() {
    let mut clean = 0.0;
    let mut total = 0.0;
    for spec in POSITIONS {
        let result = matched_position(spec, 91, "advanced", "basic", &builtin_ai);
        if result.skipped {
            continue;
        }
        for ledger in [&result.a, &result.b] {
            let profile = ledger.profile();
            let (Some(vanguard), Some(share)) = (profile.vanguard, profile.vanguard_clean) else {
                panic!("{} produced no vanguard", spec.id);
            };
            assert!(
                (0.0..=1.0).contains(&vanguard),
                "{} reported a vanguard of {vanguard}",
                spec.id
            );
            clean += share;
            total += 1.0;
        }
    }
    let overall = clean / total;
    assert!(
        overall > 0.9,
        "only {:.0}% of first-contact instants were clean of a casualty, so \
             the vanguard cannot carry a causal claim",
        overall * 100.0
    );
}

/// The muster has to actually vary, or every seed replays one game and a
/// run of 60 has the resolving power of a run of one. It also has to stay
/// close to the written deployment, or the position stops being the
/// position.
#[test]
fn the_muster_varies_between_seeds_without_leaving_the_position() {
    let spec = &POSITIONS[0];
    let mut layouts = std::collections::BTreeSet::new();
    for seed in 0u64..12 {
        let game = build(spec, seed).expect("buildable");
        let mut layout: Vec<(usize, Pos)> = Vec::new();
        for role in 0..2 {
            for uid in game.player_unit_ids(role) {
                layout.push((role, game.units[&uid].pos));
            }
        }
        layout.sort();
        // Every unit is within the slop the muster is allowed, measured
        // against the nearest cell its own force asked for.
        for (role, pos) in &layout {
            let nearest = spec.forces[*role]
                .iter()
                .map(|deploy| game.wdist(hex::offset_to_axial(deploy.col, deploy.row), *pos))
                .min()
                .unwrap_or(i32::MAX);
            assert!(
                nearest <= 3,
                "{} seated a unit {nearest} tiles from anything it deployed",
                spec.id
            );
        }
        layouts.insert(layout);
    }
    assert!(
        layouts.len() > 1,
        "12 seeds produced one layout: the muster is not varying and every \
             seed is the same game"
    );
}

/// The terrain is the position. A generated map would make these boards
/// samples rather than facts, so assert the painted features survive
/// construction — a defile with no wall is an open field.
#[test]
fn the_board_is_painted_not_generated() {
    for spec in POSITIONS {
        let game = build(spec, 7).expect("buildable");
        for (brush, cells) in spec.terrain {
            for (col, row) in *cells {
                let pos = hex::offset_to_axial(*col, *row);
                let tile = game
                    .map
                    .get(pos)
                    .unwrap_or_else(|| panic!("{} has no tile at {col},{row}", spec.id));
                let painted = match brush {
                    Brush::Mountain => tile.terrain.as_str() == "mountain",
                    Brush::Hills => tile.hills,
                    Brush::Forest => tile.feature.as_deref() == Some("forest"),
                    Brush::Water => game.rules.is_water(tile),
                    Brush::Marsh => tile.feature.as_deref() == Some("marsh"),
                };
                assert!(painted, "{} lost its {brush:?} at {col},{row}", spec.id);
            }
        }
    }
}

/// An arena is at war from the first turn, and a field position founds
/// no city — which is what makes the whole engagement the measurement.
/// A position that *states* a city is the exception, and it is stated,
/// never founded: `FoundCity` stays refused on a battlefield.
#[test]
fn a_position_is_an_arena_at_war_and_founds_no_city() {
    let game = build(&POSITIONS[0], 11).expect("buildable");
    assert!(game.is_arena());
    assert!(game.is_at_war(0, 1));
    assert!(game.cities.is_empty(), "a field position holds no city");
    for spec in POSITIONS.iter().filter(|spec| spec.cities.is_empty()) {
        let game = build(spec, 11).expect("buildable");
        assert!(game.cities.is_empty(), "{} is a field position", spec.id);
    }
    // And the action itself is still refused, so nothing can found one.
    let mut game = build(&POSITIONS[0], 11).expect("buildable");
    let settler = game.spawn_unit("settler", 0, hex::offset_to_axial(6, 6));
    assert!(game
        .apply(0, &crate::game::Action::FoundCity { unit: settler })
        .is_err());
}

/// A position that states a city gets it, with the hit points and the
/// wall it asked for — and the muster does not seat a unit on top of it.
#[test]
fn a_stated_city_stands_where_the_position_puts_it() {
    let spec = position("the_storming").expect("the assault position");
    assert_eq!(spec.cities.len(), 1);
    for seed in [1u64, 2, 3, 4] {
        let game = build(spec, seed).expect("buildable");
        assert_eq!(game.cities.len(), 1, "seed {seed}");
        let held = game.cities.values().next().expect("a city");
        assert_eq!(held.owner, 1, "the garrison holds it");
        assert_eq!(held.pos, hex::offset_to_axial(17, 6));
        assert_eq!(held.hp, 200);
        assert_eq!(
            held.wall_hp, 100,
            "a wall a melee unit cannot capture through"
        );
        assert_eq!(
            game.city_max_wall_hp(held),
            100,
            "and the pool is the stated one"
        );
        assert!(
            game.units_at(held.pos).is_empty(),
            "seed {seed}: the muster does not seat a unit on the city"
        );
        let seated: usize = (0..2).map(|role| game.player_unit_ids(role).len()).sum();
        assert_eq!(seated, spec.forces[0].len() + spec.forces[1].len());
    }
}

/// A city that changes hands is scored to the side that took it, and
/// charged to the side that lost it — the number the live seat has never
/// put above zero in a real war.
#[test]
fn a_city_that_changes_hands_is_counted_both_ways() {
    let spec = position("the_storming").expect("known");
    let mut game = build(spec, 5).expect("buildable");
    let before = super::city_owners(&game);
    let cid = *before.keys().next().expect("a city");
    assert_eq!(before[&cid], 1);
    game.cities.get_mut(&cid).expect("city").owner = 0;
    let after = super::city_owners(&game);
    let mut ledgers = (DoctrineLedger::default(), DoctrineLedger::default());
    super::account_cities(&before, &after, &mut ledgers);
    assert_eq!((ledgers.0.cities_taken, ledgers.0.cities_lost), (1, 0));
    assert_eq!((ledgers.1.cities_taken, ledgers.1.cities_lost), (0, 1));
    // And it survives the merge the report reads through.
    let mut merged = DoctrineLedger::default();
    merged.absorb(&ledgers.0);
    merged.absorb(&ledgers.0);
    assert_eq!(merged.cities_taken, 2);
}

/// A captured board keeps the city the fight was over, on the right
/// role, with the hit points and wall it had.
#[test]
fn a_captured_siege_keeps_its_city() {
    let spec = Engagement::from(position("the_storming").expect("known"));
    let mut g = build_engagement(&spec, 6).expect("buildable");
    let cid = *g.cities.keys().next().expect("a city");
    // The besieger starts six tiles out; walk it up first, because a
    // board is taken at contact. A city heals between turns, so the
    // damage is stamped after the march and before the capture.
    let mut ais: Vec<Box<dyn Ai>> = (0..2)
        .map(|pid| builtin_ai("advanced", 6 + pid as u64))
        .collect();
    let mut in_contact = false;
    for _ in 0..30 {
        if capture_engagement(&g, 0, 1, 12, 10, "probe").is_some() {
            in_contact = true;
            break;
        }
        let pid = g.current;
        ais[pid].take_turn(&mut g, pid);
        let _ = g.apply(pid, &crate::game::Action::EndTurn);
    }
    assert!(in_contact, "the storming comes into contact");
    g.cities.get_mut(&cid).expect("city").hp = 186;
    let board = capture_engagement(&g, 0, 1, 12, 10, "siege").expect("still in contact");
    assert_eq!(board.cities.len(), 1, "the city crossed with the fight");
    let held = &board.cities[0];
    assert_eq!(held.role, 1, "still the defender's");
    assert_eq!(held.hp, 186, "at the hit points the siege had left it");
    assert_eq!(held.wall_hp, 100);
    // And the board replays with it.
    let replay = build_engagement(&board, 2).expect("a captured board builds");
    assert_eq!(replay.cities.len(), 1);
    let back = replay.cities.values().next().expect("a city");
    assert_eq!((back.owner, back.hp, back.wall_hp), (1, 186, 100));
    // A board that names a city off its own edge is refused by name.
    let mut broken = board.clone();
    broken.cities[0].col = 999;
    let error = Engagement::from_json(&Engagement::to_json(&[broken])).unwrap_err();
    assert!(error.contains("off the board"), "{error}");
}

/// A unit killed from a health the controller had a turn's warning of is
/// a different loss from one killed outright, and the share of the first
/// is what says whether a change that preserves units has anything to
/// work with.
#[test]
fn a_loss_the_controller_saw_coming_is_counted_apart() {
    let spec = Engagement::from(position("the_reserve").expect("known"));
    let mut g = build_engagement(&spec, 9).expect("buildable");
    let ours: Vec<u32> = g.player_unit_ids(0);
    let (doomed, healthy) = (ours[0], ours[1]);
    g.units.get_mut(&doomed).expect("unit").hp = SALVAGEABLE_HP;
    g.units.get_mut(&healthy).expect("unit").hp = 100;
    let before = snapshot(&g);
    g.remove_unit(doomed);
    g.remove_unit(healthy);
    let after = snapshot(&g);
    let mut ledgers = (DoctrineLedger::default(), DoctrineLedger::default());
    let mut pending = BTreeMap::new();
    account(&before, &after, &g, &mut ledgers, &mut pending);
    assert_eq!(ledgers.0.losses, 2);
    assert_eq!(
        ledgers.0.losses_when_salvageable, 1,
        "the one at the withdrawal line, not the one at full health"
    );
    let profile = ledgers.0.profile();
    assert!((profile.salvageable.expect("losses were taken") - 0.5).abs() < 1e-9);
    // A ledger with no losses says nothing rather than zero — the same
    // rule every other share in the profile follows.
    assert_eq!(DoctrineLedger::default().profile().salvageable, None);
    // And it survives the merge the report reads through.
    let mut merged = DoctrineLedger::default();
    merged.absorb(&ledgers.0);
    merged.absorb(&ledgers.0);
    assert_eq!((merged.losses, merged.losses_when_salvageable), (4, 2));
}

/// The instrument has to move before it can measure. A position that ends
/// with both forces untouched would report a null for every treatment, and
/// it would be the harness saying it rather than the agents.
#[test]
fn every_position_actually_produces_a_fight() {
    for spec in POSITIONS {
        let result = matched_position(spec, 21, "advanced", "advanced", &builtin_ai);
        assert!(!result.skipped, "{} could not be seated", spec.id);
        let blows = result.a.damage_dealt + result.b.damage_dealt;
        assert!(
            blows > 0.0,
            "{} saw no damage in {} turns of a declared war",
            spec.id,
            result.turns
        );
    }
}

/// Damage is conserved: what one side dealt, the other took, and one
/// side's kills are the other's losses. This is what catches a role swap
/// that credits the wrong ledger.
#[test]
fn the_ledger_is_conserved_between_the_two_sides() {
    for spec in POSITIONS {
        let result = matched_position(spec, 22, "advanced", "advanced", &builtin_ai);
        assert!(!result.skipped);
        assert_eq!(result.a.kills, result.b.losses, "{}", spec.id);
        assert_eq!(result.b.kills, result.a.losses, "{}", spec.id);
        assert!(
            (result.a.damage_dealt - result.b.damage_taken).abs() < 1e-9,
            "{}",
            spec.id
        );
        assert!(
            (result.a.material_destroyed - result.b.material_lost).abs() < 1e-9,
            "{}",
            spec.id
        );
    }
}

/// The control. One agent in both roles plays the two role assignments
/// identically, so the pair must net to exactly zero on every position and
/// every seed. Nothing read out of this harness means anything until this
/// holds — it is what says the reported number is the agent and not the
/// position's own asymmetry.
#[test]
fn a_self_match_nets_to_zero_on_every_position() {
    for spec in POSITIONS {
        for seed in [31u64, 32] {
            let result = matched_position(spec, seed, "advanced", "advanced", &builtin_ai);
            if result.skipped {
                continue;
            }
            assert!(
                result.paired_difference().abs() < 1e-9,
                "{} on seed {seed} gave a self-match a paired difference of {}",
                spec.id,
                result.paired_difference()
            );
        }
    }
}

/// A profile reports `None` rather than a zero when there was nothing to
/// measure from, and its shares stay inside [0, 1]. A harness that invents
/// a zero for an engagement that never happened is worse than one that
/// says nothing.
#[test]
fn the_profile_reports_nothing_rather_than_a_zero() {
    let empty = DoctrineLedger::default().profile();
    assert_eq!(empty, DoctrineProfile::default());
    assert!(empty.concentration.is_none());
    assert!(empty.focus.is_none());

    let result = matched_position(&POSITIONS[0], 41, "advanced", "advanced", &builtin_ai);
    let profile = result.a.profile();
    for value in [
        profile.focus,
        profile.ground,
        profile.screen,
        profile.contact,
    ]
    .into_iter()
    .flatten()
    {
        assert!((0.0..=1.0).contains(&value), "share out of range: {value}");
    }
}

/// Concentration is a local force ratio, so within one engagement one
/// side's figure must be exactly the negative of the other's. Measured
/// against a per-side contact set instead of a shared zone, both armies
/// can report themselves outnumbered at the same contact — which is not a
/// fact about anything, and is invisible until someone tries to read the
/// two rows against each other.
#[test]
fn concentration_is_a_local_force_ratio_and_sums_to_zero() {
    for spec in POSITIONS {
        let result = matched_position(spec, 51, "advanced", "advanced", &builtin_ai);
        if result.skipped {
            continue;
        }
        for role in 0..2 {
            // Within one seating the two roles share a board, so their
            // summed local ratios must cancel turn by turn.
            let (Some(first), Some(second)) = (
                result.a_by_role[role].profile().concentration,
                result.b_by_role[1 - role].profile().concentration,
            ) else {
                continue;
            };
            assert!(
                (first + second).abs() < 1e-9,
                "{} role {role}: {first} against {second} does not cancel",
                spec.id
            );
        }
    }
}

/// The sign test must not report a confident null on overwhelming
/// evidence. The exact form overflows past n≈1023 and `NaN.min(1.0)` in
/// Rust returns 1.0, which is how a 1122-to-317 split once printed
/// `p = 1.0000`. Assert the large-n branch on exactly that split, and
/// assert the two branches agree either side of the switch.
#[test]
fn the_sign_test_does_not_overflow_into_a_confident_null() {
    let p = paired::sign_test(1_122, 317);
    assert!(
        p.is_finite() && p < 1e-6,
        "overwhelming split reported p = {p}"
    );
    assert!((paired::sign_test(0, 0) - 1.0).abs() < 1e-12);
    assert!(
        (paired::sign_test(5, 5) - 1.0).abs() < 1e-9,
        "an even split is p = 1"
    );
    // Either side of the exact/approximate switch, on the same shape.
    let exact = paired::sign_test(600, 400);
    let approximate = paired::sign_test(601, 400);
    assert!(
        (exact - approximate).abs() < 5e-3,
        "the branches disagree: {exact} against {approximate}"
    );
}

/// A paired t on constant differences has no spread, and reporting a
/// finite p there would be the harness inventing precision. One difference
/// is a number, not a result.
#[test]
fn the_paired_t_handles_no_spread_and_too_few_pairs() {
    let (mean, stderr, _, p) = paired::paired_t(&[4.0, 4.0, 4.0]);
    assert!((mean - 4.0).abs() < 1e-12);
    assert_eq!(stderr, 0.0);
    assert_eq!(p, 0.0, "a constant non-zero difference is not a null");
    let (mean, _, _, p) = paired::paired_t(&[7.0]);
    assert!((mean - 7.0).abs() < 1e-12);
    assert_eq!(p, 1.0, "one pair licenses nothing");
    let (_, _, _, p) = paired::paired_t(&[0.0, 0.0, 0.0]);
    assert_eq!(p, 1.0);
}

/// Identifiers are what a command line and a report key on, so they have
/// to be unique and findable.
#[test]
fn every_position_has_a_unique_identifier() {
    let mut seen = std::collections::BTreeSet::new();
    for spec in POSITIONS {
        assert!(seen.insert(spec.id), "duplicate position id {}", spec.id);
        assert_eq!(position(spec.id).map(|found| found.id), Some(spec.id));
    }
    assert!(position("no_such_position").is_none());
}
