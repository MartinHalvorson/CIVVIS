//! Focused tests for `city-state-before-the-first-capital`.
//!
//! The seat is built the way the live Domination seat runs the opening: the
//! armed `early-conquest-opening`, `conquest-opening-stays-near`,
//! `early-conquest-stands-down`, `declaration-needs-production-parity`,
//! `domination-ignores-city-states` and `first-capital-before-the-walls`, on
//! Online speed, with or without this gene.

use super::super::city_state_harvest::HarvestTarget;
use super::super::early_conquest::{CONQUEST_ASSEMBLY_SHARE, CONQUEST_MIN_PREPARATION_TURNS};
use super::super::objective_board::ObjectiveKey;
use super::super::test_support::opt_in_off_in_both_controllers;
use super::*;
use crate::game::Game;
use crate::name;
use std::sync::Arc;

fn at(col: i32, row: i32) -> Pos {
    (col, row)
}

/// Seat 0 holds its capital at (6, 12) and a second city at (6, 18). Major 1
/// stands 24 tiles east (past the capital opening's twenty). City-state 2's
/// city is 8 tiles out, city-state 3's 11, both behind Ancient Walls, met and
/// charted. Online speed, turn 30, every starting unit cleared.
fn fixture() -> (Game, u32, u32) {
    fixture_at(at(30, 12), at(14, 12), at(17, 12))
}

/// [`fixture`] with major 1's capital and the two city-states' cities on
/// the tiles named.
fn fixture_at(rival: Pos, near: Pos, far: Pos) -> (Game, u32, u32) {
    let mut game = Game::new_full(4, 60, 30, 5_051_001, 1_000, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    for pos in [at(6, 12), at(6, 18), rival, near, far] {
        assert!(game.map.tiles.contains_key(&pos), "{pos:?} is on the map");
    }
    game.found_city_for(0, at(6, 12), None);
    game.found_city_for(0, at(6, 18), None);
    game.found_city_for(1, rival, None);
    let near = game.found_city_for(2, near, None);
    let far = game.found_city_for(3, far, None);
    for minor in [2, 3] {
        game.players[minor].is_minor = true;
    }
    for cid in [near, far] {
        Arc::make_mut(&mut game.observed_city_max_wall_hp).insert(cid, 100);
    }
    for other in 1..4 {
        game.record_contact(0, other);
    }
    let tiles: Vec<Pos> = game.map.tiles.keys().copied().collect();
    game.players[0].explored.extend(tiles);
    game.at_war.clear();
    game.game_speed = crate::setup::GameSpeed::Online;
    game.turn = 30;
    game.current = 0;
    (game, near, far)
}

/// The live Domination seat's opening genes, with or without this one.
fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_early_conquest_opening();
    ai.enable_conquest_opening_stays_near();
    ai.enable_early_conquest_stands_down();
    ai.enable_declaration_needs_production_parity();
    ai.enable_domination_ignores_city_states();
    ai.enable_first_capital_before_the_walls();
    ai.conquest_opening_needs_the_production = false;
    if gene {
        ai.enable_city_state_before_the_first_capital();
    }
    ai
}

fn unit(game: &mut Game, kind: &str, pos: Pos) -> u32 {
    let uid = game.spawn_test_unit(kind, 0, pos);
    let moves = game.unit_max_moves(uid);
    let unit = game.units.get_mut(&uid).unwrap();
    unit.moves_left = moves;
    unit.moved = false;
    uid
}

/// Five Warriors and an Archer on the ring around `rally`, the city-states'
/// centres at the floor strength an undefended one reads.
fn force_at(game: &mut Game, rally: Pos) -> Vec<u32> {
    let stones: Vec<u32> = game
        .cities
        .values()
        .filter(|city| game.players[city.owner].is_minor)
        .map(|city| city.id)
        .collect();
    for cid in stones {
        Arc::make_mut(&mut game.observed_city_strength).insert(cid, 20.0);
    }
    let ring: Vec<Pos> = game
        .wring(rally, 1)
        .into_iter()
        .filter(|pos| game.city_at(*pos).is_none() && game.unit_ids_at(*pos).is_empty())
        .collect();
    [
        "warrior", "warrior", "warrior", "warrior", "warrior", "archer",
    ]
    .iter()
    .zip(ring)
    .map(|(kind, pos)| unit(game, kind, pos))
    .collect()
}

fn counter(game: &Game, key: &str) -> Option<i64> {
    game.players[0]
        .counters
        .get(&format!("city_state_stone:{key}"))
        .copied()
}

/// The stone named on `near` with its force and a Ram at the rally,
/// declared, on the turn after.
fn declared() -> (Game, AdvancedAi, u32) {
    let (mut game, near, _) = fixture();
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    let rally = ai.conquest_opening.as_ref().expect("named").rally;
    force_at(&mut game, rally);
    unit(&mut game, "battering_ram", rally);
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_declaration(&mut game, 0), "the war opens");
    (game, ai, near)
}

// ----------------------------------------------------------------- registry

#[test]
fn city_state_before_the_first_capital_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("city-state-before-the-first-capital", |ai| {
        ai.city_state_before_the_first_capital
    });
    assert!(!seat(false).city_state_stone_active());
    assert!(seat(true).city_state_stone_active());
    let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
    culture.enable_early_conquest_opening();
    culture.enable_city_state_before_the_first_capital();
    assert!(!culture.city_state_stone_active(), "Domination lane only");
    let mut bare = AdvancedAi::targeting(VictoryTarget::Domination);
    bare.enable_city_state_before_the_first_capital();
    assert!(
        !bare.city_state_stone_active(),
        "rides on early-conquest-opening"
    );
    assert_eq!((STONE_RANGED, STONE_MELEE), (1, 5));
    assert_eq!(
        crate::setup::GameSpeed::Online.scale_turns(STONE_NAMING_DEADLINE),
        50
    );
}

// ------------------------------------------------------------------- target

#[test]
fn inert_off_and_off_the_domination_lane() {
    let (mut game, _, _) = fixture();
    let mut off = seat(false);
    off.maintain_conquest_opening(&mut game, 0);
    assert!(
        off.conquest_opening.is_none(),
        "off: the shipped scan skips minors"
    );
    assert_eq!(counter(&game, "named"), None);

    let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
    culture.enable_early_conquest_opening();
    culture.enable_city_state_before_the_first_capital();
    culture.maintain_conquest_opening(&mut game, 0);
    assert!(culture.conquest_opening.is_none(), "another lane: nothing");
    assert_eq!(culture.city_state_opening_tile(), None);
}

#[test]
fn names_the_nearest_eligible_city_state_with_its_own_force_and_clock() {
    let (mut game, near, _) = fixture();
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    let opening = ai.conquest_opening.clone().expect("the stone is named");
    assert_eq!(
        (opening.target, opening.city),
        (2, near),
        "8 tiles beats 11"
    );
    assert!(ai.city_state_stone_stands());
    assert_eq!(ai.city_state_opening_tile(), Some(at(14, 12)));
    assert_eq!(ai.conquest_quota(), (STONE_RANGED, STONE_MELEE));
    assert_eq!(ai.conquest_force_size(), STONE_RANGED + STONE_MELEE);
    assert_eq!(ai.conquest_first_settler_cities(), STONE_CITIES);
    assert_eq!(
        ai.conquest_preparation(&game),
        game.standard_duration(CONQUEST_MIN_PREPARATION_TURNS),
        "the shipped window, not first-capital's"
    );
    assert_eq!(opening.preparing_since, Some(game.turn));
    assert_eq!(counter(&game, "named"), Some(1));
    assert!(ai.city_state_stone_spares_envoys(2));
    assert!(!ai.city_state_stone_spares_envoys(3));
}

#[test]
fn a_lighter_garrison_breaks_a_distance_tie() {
    // City-state 3 stands 8 tiles out as well, with nobody home; 2 holds a
    // Warrior the seat can see.
    let (mut game, near, other) = fixture_at(at(30, 12), at(14, 12), at(14, 4));
    let home = at(6, 12);
    assert_eq!(game.wdist(home, at(14, 4)), game.wdist(home, at(14, 12)));
    let ai = seat(true);
    let scan = ai.city_state_stone_scan(&game, 0, home);
    assert_eq!(scan.best.map(|best| best.2), Some(near), "a tie goes by id");
    game.spawn_test_unit("warrior", 2, at(14, 12));
    unit(&mut game, "scout", at(13, 12));
    let scan = ai.city_state_stone_scan(&game, 0, home);
    assert_eq!(
        scan.best.map(|best| (best.3, best.2)),
        Some((3, other)),
        "the empty city"
    );
}

#[test]
fn every_ineligible_city_state_is_refused_with_its_reason() {
    let home = at(6, 12);
    let reason = |game: &Game| seat(true).city_state_stone_scan(game, 0, home);

    let (mut game, _, _) = fixture();
    game.players[0].explored.remove(&at(14, 12));
    game.players[0].explored.remove(&at(17, 12));
    let scan = reason(&game);
    assert_eq!(scan.best, None);
    assert_eq!(scan.refused.map(|r| r.2), Some("its city is not charted"));

    // Both 13 tiles out.
    let (game, _, _) = fixture_at(at(30, 12), at(19, 12), at(6, 25));
    let scan = reason(&game);
    assert_eq!(scan.best, None);
    assert_eq!(
        scan.refused.map(|r| r.2),
        Some("too far: past the 12 tiles a stepping stone reaches"),
        "13 tiles"
    );

    let (mut game, near, far) = fixture();
    for cid in [near, far] {
        Arc::make_mut(&mut game.observed_city_max_wall_hp).insert(cid, 200);
    }
    let scan = reason(&game);
    assert_eq!(scan.best, None);
    assert_eq!(scan.refused.map(|r| r.2), Some("walled past Ancient Walls"));

    let (mut game, _, _) = fixture();
    game.players[1].envoys = vec![(2, 3), (3, 3)];
    assert_eq!(game.suzerain_of(2), Some(1));
    let scan = reason(&game);
    assert_eq!(scan.best, None);
    assert_eq!(scan.refused.map(|r| r.2), Some("it has a suzerain"));

    let (mut game, _, _) = fixture();
    game.players[0].envoys = vec![(2, 3), (3, 3)];
    let scan = reason(&game);
    assert_eq!(scan.best, None);
    assert_eq!(scan.refused.map(|r| r.2), Some("we are its suzerain"));

    let (mut game, _, _) = fixture();
    for minor in [2, 3] {
        game.players[0].met.remove(&minor);
        game.players[minor].met.remove(&0);
    }
    let scan = reason(&game);
    assert_eq!((scan.met, scan.best), (0, None), "no city-state met");

    // A Free City is a minor seat, never a city-state to take.
    let (mut game, _, _) = fixture();
    for minor in [2, 3] {
        game.players[minor].is_barbarian = true;
        game.players[minor].is_free_city = true;
    }
    let scan = reason(&game);
    assert_eq!(
        (scan.met, scan.best),
        (0, None),
        "the Free Cities are not a stone"
    );
}

#[test]
fn nothing_is_named_with_one_city_past_turn_50_or_in_a_major_war() {
    let (mut game, _, _) = fixture();
    let second = game
        .player_city_ids(0)
        .into_iter()
        .find(|cid| !game.cities[cid].is_capital)
        .unwrap();
    game.cities.remove(&second);
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(
        ai.conquest_opening.is_none(),
        "the first Settler comes first"
    );

    for (turn, opens) in [(49, true), (50, false)] {
        let (mut game, _, _) = fixture();
        game.turn = turn;
        let mut ai = seat(true);
        ai.conquest_first_turn = Some(1);
        ai.maintain_conquest_opening(&mut game, 0);
        assert_eq!(ai.conquest_opening.is_some(), opens, "turn {turn}");
    }

    let (mut game, _, _) = fixture();
    game.at_war.insert((0, 1));
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(
        ai.conquest_opening.is_none(),
        "never while a major is at war with us"
    );
    assert!(
        !ai.city_state_stone_named,
        "and the game's stone is not spent"
    );
}

// ---------------------------------------------------------------- the force

#[test]
fn reserves_five_melee_a_shooter_and_a_ram_on_top_of_the_city_guards() {
    let (mut game, _, _) = fixture();
    let capital_guard = unit(&mut game, "archer", at(6, 12));
    let second_guard = unit(&mut game, "warrior", at(6, 18));
    let free = unit(&mut game, "warrior", at(9, 12));
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.city_state_stone_stands());
    assert_eq!(
        ai.first_capital_home_guards(&game, 0),
        BTreeSet::from([capital_guard, second_guard])
    );
    let counts = ai.counts(&game, 0);
    assert_eq!(
        ai.first_capital_shortfall(&game, 0, &counts),
        (1, 4),
        "a shooter and five melee bodies on top of the guards; one Warrior stands"
    );
    let force = &ai.conquest_opening.as_ref().unwrap().force;
    assert_eq!(*force, BTreeSet::from([free]), "no guard marches");

    let capital = AdvancedAi::conquest_capital(&game, 0).unwrap();
    let warrior = game.rules.units["warrior"].clone();
    assert!(ai.conquest_reservation(&game, 0, capital, &warrior, &counts, false) > 0.0);
    assert!(ai.conquest_defers_the_settler(&game, 0, capital, &counts, 2, false));

    // The walls ask for a Ram: the research credit chases Masonry, and once
    // it is known the capital reserves the Ram.
    assert!(ai.conquest_research_value(&game, 0, "masonry") > 0.0);
    assert_eq!(
        ai.conquest_breaker_value(&game, 0, capital, "battering_ram"),
        0.0,
        "no Ram before Masonry"
    );
    game.players[0].techs.insert(name!("masonry"));
    assert!(ai.conquest_breaker_value(&game, 0, capital, "battering_ram") > 0.0);

    let off = seat(false);
    assert!(off.first_capital_home_guards(&game, 0).len() == 2);
    assert_eq!(off.conquest_quota(), (2, 4), "first-capital's own force");
}

#[test]
fn from_turn_one_research_chases_the_ram_and_the_unique_melee_line() {
    let (mut game, _, _) = fixture();
    game.turn = 1;
    game.players[0].civ = "Macedon".to_string();
    let on = seat(true);
    let off = seat(false);
    assert!(on.conquest_opening.is_none(), "nothing is named yet");
    assert_eq!(
        AdvancedAi::city_state_stone_research_goals(&game, 0),
        vec![name!("masonry"), name!("iron_working")],
        "the Ram, and the Hypaspist that out-fights a Warrior"
    );
    for tech in ["masonry", "bronze_working", "iron_working"] {
        assert_eq!(
            off.conquest_research_value(&game, 0, tech),
            0.0,
            "off: {tech}"
        );
        assert!(
            on.conquest_research_value(&game, 0, tech) > 0.0,
            "on: {tech}"
        );
    }
    assert_eq!(on.conquest_research_value(&game, 0, "pottery"), 0.0);
    assert_eq!(
        on.conquest_research_value(&game, 0, "mining"),
        2.0 * super::super::early_conquest::CONQUEST_RESEARCH,
        "Mining leads to both"
    );

    // A civilization with no stronger unique melee unit asks for Masonry
    // alone, and nothing once it is known.
    game.players[0].civ = "Nowhere".to_string();
    assert_eq!(
        AdvancedAi::city_state_stone_research_goals(&game, 0),
        vec![name!("masonry")]
    );
    game.players[0].techs.insert(name!("masonry"));
    assert!(AdvancedAi::city_state_stone_research_goals(&game, 0).is_empty());
    assert_eq!(on.conquest_research_value(&game, 0, "masonry"), 0.0);

    // Closed past the naming deadline with no stone named.
    game.players[0].civ = "Macedon".to_string();
    game.turn = 50;
    assert_eq!(on.conquest_research_value(&game, 0, "iron_working"), 0.0);
}

// ----------------------------------------------------------- the declaration

#[test]
fn declares_on_the_city_state_once_assembled_with_a_ram_and_the_bill_covered() {
    let (mut game, near, _) = fixture();
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    let force = force_at(&mut game, rally);
    ai.maintain_conquest_opening(&mut game, 0);
    let opening = ai.conquest_opening.clone().unwrap();
    assert!(opening.assembled.is_some(), "six of six at the rally");
    assert!(ai.conquest_share(&game, &opening) >= CONQUEST_ASSEMBLY_SHARE);
    assert!(
        ai.conquest_preview_takes_the_city(&game, 0, &opening),
        "the minor's bill is priced and covered"
    );
    assert!(!ai.conquest_declaration(&mut game, 0), "no Ram yet");
    assert!(!game.is_at_war(0, 2));

    unit(&mut game, "battering_ram", rally);
    assert!(ai.conquest_declaration(&mut game, 0));
    assert!(game.is_at_war(0, 2), "a surprise war on the city-state");
    assert!(!game.is_at_war(0, 3) && !game.is_at_war(0, 1));
    let opening = ai.conquest_opening.clone().unwrap();
    assert_eq!(opening.declared, Some(game.turn));
    assert_eq!(counter(&game, "declared"), Some(1));

    // The campaign is pinned on the city-state's city ...
    assert_eq!(ai.campaign_target(&game, 0), Some(2));
    assert_eq!(ai.campaign_objective_city(&game, 0, Some(2)), Some(near));

    // ... the next turn's plan aims at it, through the city-state rules the
    // Domination seat arms, and a Recovery read off an unfought major is
    // lifted ...
    game.turn += 1;
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(
        ai.city_state_stone_stands(),
        "the stone stands through its war"
    );
    assert!(ai.city_state_stone.as_ref().unwrap().seen_at_war);
    ai.enable_city_state_war_is_no_front();
    let mut plan = ai.assess(&game, 0);
    ai.apply_city_state_stone_to_strategy(&game, 0, &mut plan);
    assert_eq!(
        (plan.target_player, plan.target_city),
        (Some(2), Some(near))
    );
    let mut recovery = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        threatened_city: None,
        target_player: None,
        target_city: None,
        ..plan.clone()
    };
    ai.apply_city_state_stone_to_strategy(&game, 0, &mut recovery);
    assert_eq!(recovery.strategy, GrandStrategy::Conquest);
    assert_eq!(recovery.target_city, Some(near));

    // ... and the force marches on it: the board raises a Siege row on the
    // city and the strike force serves it.
    ai.enable_objective_board();
    ai.rebuild_force_groups(&game, 0, &plan);
    let siege = ai
        .objective_board()
        .forces
        .iter()
        .find(|row| row.objective_key == ObjectiveKey::Siege(near))
        .expect("a Siege force on the city-state's city");
    assert!(
        siege.units.iter().any(|uid| force.contains(uid)),
        "the strike force marches on it"
    );
}

/// The assembled force holds while its melee bodies would not finish the
/// city: the kill-time preview prices blows, not strength.
#[test]
fn the_force_holds_until_its_melee_can_finish_the_city() {
    let (mut game, near, _) = fixture();
    stand_walls(&mut game, near);
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    force_at(&mut game, rally);
    unit(&mut game, "battering_ram", rally);
    ai.maintain_conquest_opening(&mut game, 0);
    let opening = ai.conquest_opening.clone().unwrap();

    // Five Warriors against the floor centre: 30 a blow, four blows each.
    let (bodies, blows, needed) =
        AdvancedAi::city_state_stone_kill_time(&game, &opening).expect("priced");
    assert_eq!(bodies, 5, "the Archer and the Ram land no blow");
    assert!(
        (needed - 360.0).abs() < 1e-6,
        "200 health, 100 walls, 60 healing: {needed}"
    );
    assert!(blows > needed, "{blows} against {needed}");
    assert_eq!(ai.city_state_stone_holds(&game, 0, &opening), None);

    // A centre of 28 kills a Warrior on its third blow, each worth about 22:
    // five of them fall short, and the force holds.
    Arc::make_mut(&mut game.observed_city_strength).insert(near, 28.0);
    let (_, blows, needed) =
        AdvancedAi::city_state_stone_kill_time(&game, &opening).expect("priced");
    assert!(blows <= needed, "{blows} against {needed}");
    assert_eq!(
        ai.city_state_stone_holds(&game, 0, &opening),
        Some("the melee bodies at the rally would not finish the city before they fall")
    );
    assert!(
        !ai.conquest_declaration(&mut game, 0),
        "no war it cannot finish"
    );
    assert!(!game.is_at_war(0, 2));

    // Bodies away from the rally are not counted.
    Arc::make_mut(&mut game.observed_city_strength).insert(near, 20.0);
    let far_tile = at(3, 14);
    let mover = *opening
        .force
        .iter()
        .find(|uid| AdvancedAi::conquest_melee_body(&game.rules.units[game.units[*uid].kind]))
        .unwrap();
    game.units.get_mut(&mover).unwrap().pos = far_tile;
    let (bodies, _, _) = AdvancedAi::city_state_stone_kill_time(&game, &opening).expect("priced");
    assert_eq!(bodies, 4);
}

#[test]
fn never_declares_while_a_major_is_at_war_with_us() {
    let (mut game, _, _) = fixture();
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    force_at(&mut game, rally);
    unit(&mut game, "battering_ram", rally);
    ai.maintain_conquest_opening(&mut game, 0);
    game.at_war.insert((0, 1));
    assert!(!ai.conquest_declaration(&mut game, 0));
    assert!(!game.is_at_war(0, 2));
    assert!(ai.city_state_stone_stands(), "held, not released");
}

// ------------------------------------------------------------------- after

#[test]
fn a_capture_keeps_the_city_and_leaves_the_capital_opening_free() {
    // A rival capital 18 tiles out that the seat has not charted yet.
    let (mut game, near, _) = fixture_at(at(24, 12), at(14, 12), at(17, 12));
    let capital = game.player_city_ids(1)[0];
    assert!(game.cities[&capital].is_capital);
    game.players[0].explored.remove(&at(24, 12));
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    assert_eq!(ai.conquest_opening.as_ref().unwrap().target, 2);

    // Kept, whatever the disposition otherwise reads.
    assert!(ai.city_state_stone_keeps(at(14, 12)));
    let keep = ai.city_disposition_value(
        &game,
        &game,
        0,
        GrandStrategy::Expansion,
        &Action::KeepCity { city: near },
    );
    let raze = ai.city_disposition_value(
        &game,
        &game,
        0,
        GrandStrategy::Expansion,
        &Action::RazeCity { city: near },
    );
    assert!(keep - raze >= STONE_KEEP_VALUE * 0.9);
    assert!(!seat(false).city_state_stone_keeps(at(14, 12)));

    // Taken.
    {
        let city = game.cities.get_mut(&near).unwrap();
        city.owner = 0;
        city.is_capital = false;
    }
    game.turn += 1;
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_opening.is_none(), "the stone closes");
    assert!(ai.city_state_stone.is_none());
    assert!(!ai.conquest_closed, "the capital opening is not spent");
    assert_eq!(counter(&game, "captured"), Some(1));
    assert_eq!(counter(&game, "captured_turn"), Some(i64::from(game.turn)));

    // The capital is charted: first-capital names it; no second stone.
    game.players[0].explored.insert(at(24, 12));
    game.turn += 1;
    ai.maintain_conquest_opening(&mut game, 0);
    let opening = ai.conquest_opening.as_ref().expect("the capital is named");
    assert_eq!((opening.target, opening.city), (1, capital));
    assert!(!ai.city_state_stone_stands());
    assert_eq!(counter(&game, "named"), Some(1));
}

#[test]
fn a_rival_capital_in_hand_comes_first() {
    let (mut game, _, _) = fixture_at(at(24, 12), at(14, 12), at(17, 12));
    let capital = game.player_city_ids(1)[0];
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    let opening = ai.conquest_opening.as_ref().unwrap();
    assert_eq!((opening.target, opening.city), (1, capital));
    assert!(!ai.city_state_stone_stands());
    assert_eq!(counter(&game, "named"), None);
}

#[test]
fn a_released_stone_never_closes_the_capital_opening_and_is_never_renamed() {
    // The war ends before the city falls.
    let (mut game, mut ai, _) = declared();
    game.turn += 1;
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.city_state_stone_stands());
    game.at_war.clear();
    game.turn += 1;
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_opening.is_none(), "the war has ended");
    assert!(!ai.conquest_closed);
    game.turn += 1;
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(
        ai.conquest_opening.is_none(),
        "city-state 3 is eligible, but a game tries one stone"
    );

    // A suzerain before the war.
    let (mut game, _, _) = fixture();
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    game.players[1].envoys = vec![(2, 3)];
    game.turn += 1;
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_opening.is_none(), "a suzerain took it first");
    assert!(!ai.conquest_closed);
}

#[test]
fn a_war_the_host_refused_releases_the_stone() {
    let (mut game, _, _) = fixture();
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    force_at(&mut game, rally);
    ai.maintain_conquest_opening(&mut game, 0);
    // Live, the model holds the declaration for the frame; the host refused.
    if let Some(opening) = ai.conquest_opening.as_mut() {
        assert!(!opening.force.is_empty());
        opening.declared = Some(game.turn);
    }
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(
        ai.city_state_stone_stands(),
        "the declaring turn's own frames"
    );
    game.turn += 1;
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_opening.is_none(), "the host refused the war");
    assert!(!ai.conquest_closed);
    assert!(!ai.conquest_declaration(&mut game, 0), "never re-declared");
    assert!(!game.is_at_war(0, 2));
}

#[test]
fn a_shifted_city_id_or_minor_seat_is_re_read_from_the_tile() {
    let (mut game, near, _) = fixture();
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    // Live, the next frame's mirror can number the city and the city-state
    // differently.
    if let Some(opening) = ai.conquest_opening.as_mut() {
        opening.city = 424_242;
        opening.target = 3;
    }
    assert!(ai.city_state_stone_spares_envoys(3), "the stale seat");
    game.turn += 1;
    ai.maintain_conquest_opening(&mut game, 0);
    let opening = ai.conquest_opening.as_ref().expect("kept");
    assert_eq!((opening.target, opening.city), (2, near));
    assert!(ai.city_state_stone_spares_envoys(2));
}

#[test]
fn the_harvest_and_the_stone_never_hold_two_names() {
    // A rebel Free City beside our second city is a harvest candidate from
    // turn 40.
    let (mut game, _, rebel) = fixture_at(at(30, 12), at(14, 12), at(9, 18));
    game.cities.get_mut(&rebel).unwrap().pop = 6;
    game.players[3].is_barbarian = true;
    game.players[3].is_free_city = true;
    for _ in 0..4 {
        unit(&mut game, "warrior", at(6, 18));
    }
    game.turn = 45;

    let mut harvest_only = seat(false);
    harvest_only.enable_city_state_harvest();
    harvest_only.maintain_conquest_opening(&mut game.clone(), 0);
    let mut alone = game.clone();
    harvest_only.maintain_city_state_harvest(&mut alone, 0);
    assert!(
        harvest_only.city_state_harvest_target.is_some(),
        "without the stone the harvest names the Free City"
    );

    let mut both = seat(true);
    both.enable_city_state_harvest();
    both.conquest_first_turn = Some(1);
    both.maintain_conquest_opening(&mut game, 0);
    assert!(both.city_state_stone_stands());
    both.maintain_city_state_harvest(&mut game, 0);
    assert!(
        both.city_state_harvest_target.is_none(),
        "one name at a time"
    );

    // And a harvest name holds the stone off.
    let (mut game, near, _) = fixture();
    let mut held = seat(true);
    held.city_state_harvest_target = Some(HarvestTarget {
        player: 2,
        city: near,
        named_turn: game.turn,
        declared_turn: None,
        pos: at(14, 12),
        owner_civ: game.players[2].civ.clone(),
        outclassed_turns: 0,
        outclass_seen_turn: game.turn,
    });
    held.maintain_conquest_opening(&mut game, 0);
    assert!(held.conquest_opening.is_none());
}

#[test]
fn the_opening_is_dropped_the_moment_the_gene_is_switched_off() {
    let (mut game, _, _) = fixture();
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.city_state_stone_stands());
    ai.disable_city_state_before_the_first_capital();
    assert_eq!(ai.city_state_opening_tile(), None);
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_opening.is_none());
    assert!(ai.city_state_stone.is_none());
    assert!(!ai.conquest_closed);
}

// ------------------------------------------------------- the stone's clock

/// Ancient Walls standing on `cid` (the fixture's walls are the observed
/// maximum alone), as a live city-state's are.
fn stand_walls(game: &mut Game, cid: u32) {
    let city = game.cities.get_mut(&cid).unwrap();
    city.buildings.push(name!("walls"));
    city.wall_hp = 100;
}

/// An ordinary early plan for the production and research pickers.
fn expansion_plan(game: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: game.turn,
        rush: false,
    }
}

/// Live civvis-20261111T061116Z: Hong Kong named at turn 15, released at 40
/// "before the force ever assembled" while its bodies were still coming.
#[test]
fn the_clock_holds_a_stone_whose_force_is_still_coming_until_turn_60() {
    let (mut game, near, _) = fixture();
    game.turn = 15;
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    assert_eq!(ai.conquest_opening.as_ref().unwrap().city, near);
    // Two bodies on their way, eight tiles from the rally; no Ram yet.
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    for pos in [at(3, 14), at(3, 13)] {
        unit(&mut game, "warrior", pos);
        assert!(game.wdist(pos, rally) > 5);
    }
    for turn in [40, 49, 50, 59] {
        game.turn = turn;
        ai.maintain_conquest_opening(&mut game, 0);
        assert!(
            ai.city_state_stone_stands(),
            "turn {turn}: the force is still coming"
        );
        assert!(ai.conquest_reservation_open(&game), "turn {turn}");
    }
    game.turn = 60;
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_opening.is_none(), "turn 60: the clock runs out");
    assert!(!ai.conquest_closed, "the capital opening is not spent");

    // A major at war with us lifts the hold: the stone's own naming deadline
    // (turn 50) is the clock.
    let (mut game, _, _) = fixture();
    game.turn = 15;
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    unit(&mut game, "warrior", at(3, 14));
    game.at_war.insert((0, 1));
    game.turn = 49;
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(
        ai.city_state_stone_stands(),
        "turn 49: inside the naming deadline"
    );
    game.turn = 50;
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_opening.is_none(), "turn 50 in a major war");
}

#[test]
fn the_ram_is_claimed_ahead_of_the_bodies_once_masonry_is_known() {
    let (mut game, near, _) = fixture();
    stand_walls(&mut game, near);
    game.players[0].techs.insert(name!("masonry"));
    game.players[0].gold = 50.0;
    game.players[0].gold_per_turn = 5.0;
    // A melee body to swing beside the Ram.
    unit(&mut game, "warrior", at(9, 12));
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.city_state_stone_stands());
    let capital = AdvancedAi::conquest_capital(&game, 0).unwrap();
    let counts = ai.counts(&game, 0);
    let ram = ai.conquest_breaker_value(&game, 0, capital, "battering_ram");
    for body in ["warrior", "archer", "spearman"] {
        let spec = game.rules.units[body].clone();
        assert!(
            ram > ai.conquest_reservation(&game, 0, capital, &spec, &counts, false),
            "the Ram outranks a {body}"
        );
    }
    let plan = expansion_plan(&game);
    let mut claimed = game.clone();
    assert!(ai.conquest_opening_production(&mut claimed, 0, &plan));
    assert_eq!(
        claimed.cities[&capital].queue.first(),
        Some(&crate::game::Item::Unit {
            unit: name!("battering_ram")
        }),
        "the idle capital takes the Ram first"
    );
}

#[test]
fn research_takes_masonry_then_the_unique_melee_line_from_turn_one() {
    // Turn one, nothing known: Mining, the step toward Masonry.
    let (mut game, _, _) = fixture();
    game.turn = 1;
    game.players[0].civ = "Macedon".to_string();
    game.players[0].techs.clear();
    game.players[0].research = None;
    let ai = seat(true);
    assert_eq!(
        ai.city_state_stone_breaker_research_goal(&game, 0),
        Some(name!("masonry"))
    );
    assert_eq!(
        seat(false).city_state_stone_breaker_research_goal(&game, 0),
        None
    );
    let plan = expansion_plan(&game);
    let mut on = game.clone();
    ai.advanced_research(&mut on, 0, &plan);
    assert_eq!(on.players[0].research.as_deref(), Some("mining"));

    // A tech already in progress finishes first.
    let mut busy = game.clone();
    busy.players[0].research = Some("pottery".to_string());
    ai.advanced_research(&mut busy, 0, &plan);
    assert_eq!(busy.players[0].research.as_deref(), Some("pottery"));

    game.turn = 6;
    game.players[0].techs.insert(name!("mining"));
    assert!(ai.conquest_opening.is_none(), "nothing is named yet");
    assert_eq!(
        ai.city_state_stone_research_goal(&game, 0),
        Some(name!("masonry"))
    );
    assert_eq!(seat(false).city_state_stone_research_goal(&game, 0), None);
    let plan = expansion_plan(&game);
    let mut on = game.clone();
    ai.advanced_research(&mut on, 0, &plan);
    assert_eq!(on.players[0].research.as_deref(), Some("masonry"));

    // Masonry known: the step toward Iron Working (the Hypaspist).
    game.players[0].techs.insert(name!("masonry"));
    assert_eq!(
        ai.city_state_stone_research_goal(&game, 0),
        Some(name!("iron_working"))
    );
    let mut on = game.clone();
    ai.advanced_research(&mut on, 0, &plan);
    assert_eq!(on.players[0].research.as_deref(), Some("bronze_working"));
}

#[test]
fn the_force_is_journalled_every_few_turns_before_the_war() {
    let (mut game, _, _) = fixture();
    game.turn = 15;
    let mut ai = seat(true);
    ai.base.journal = crate::reasoning::Journal::recording();
    ai.maintain_conquest_opening(&mut game, 0);
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    force_at(&mut game, rally);
    unit(&mut game, "warrior", at(3, 14));
    game.turn = 16;
    ai.maintain_conquest_opening(&mut game, 0);
    let thoughts = ai.journal().since(0).thoughts;
    let line = thoughts
        .iter()
        .find(|thought| thought.headline.contains("bodies at the rally"))
        .expect("the progress line");
    assert!(
        line.headline.contains("of 6 bodies at the rally for"),
        "{}",
        line.headline
    );
    assert!(
        line.detail.contains("breaker waiting on Masonry"),
        "{}",
        line.detail
    );
    assert!(line.detail.contains("bill "), "{}", line.detail);
    assert!(
        line.detail.contains("the clock holds it"),
        "{}",
        line.detail
    );
}

#[test]
fn the_patience_window_waits_for_a_coming_breaker_while_the_bill_is_covered() {
    let (mut game, _, _) = fixture();
    game.turn = 15;
    let mut ai = seat(true);
    ai.base.journal = crate::reasoning::Journal::recording();
    ai.maintain_conquest_opening(&mut game, 0);
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    force_at(&mut game, rally);
    game.turn = 16;
    ai.maintain_conquest_opening(&mut game, 0);
    let opening = ai.conquest_opening.clone().unwrap();
    assert_eq!(opening.assembled, Some(16), "six of six at the rally");
    assert!(ai.conquest_preview_takes_the_city(&game, 0, &opening));
    // The declaration holds for the Ram and records the covered bill.
    assert!(!ai.conquest_declaration(&mut game, 0));
    assert!(ai.city_state_stone_waits_for_its_breaker(&game, 0, &opening));
    // Past the shipped patience window (13 Online turns), Masonry still to
    // research: the stone stands.
    for turn in [29, 40, 59] {
        game.turn = turn;
        ai.maintain_conquest_opening(&mut game, 0);
        assert!(ai.city_state_stone_stands(), "turn {turn}");
    }
    game.turn = 60;
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_opening.is_none(), "turn 60: the hold is over");
    assert!(!ai.conquest_closed);
    let thoughts = ai.journal().since(0).thoughts;
    assert!(
        thoughts.iter().any(|thought| thought
            .detail
            .contains("with the bill covered, but no breaker reached it")),
        "the release names the missing breaker, not the bill"
    );

    // Off the stone, the shipped window runs: a first-capital seat with no
    // stone is unchanged (this gene's helpers answer `false`).
    assert!(!seat(false).city_state_stone_waits_for_its_breaker(&game, 0, &opening));
}

#[test]
fn a_covered_bill_buys_the_ram_under_breaker_bought_for_the_siege() {
    let (mut game, near, _) = fixture();
    stand_walls(&mut game, near);
    game.players[0].techs.insert(name!("masonry"));
    game.players[0].gold = 2_000.0;
    game.players[0].gold_per_turn = 10.0;
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    force_at(&mut game, rally);
    ai.maintain_conquest_opening(&mut game, 0);
    let rams = |game: &Game| {
        game.units
            .values()
            .filter(|unit| unit.owner == 0 && unit.kind.as_str() == "battering_ram")
            .count()
    };

    // The armed purchase rule off: nothing is bought.
    let mut off = game.clone();
    assert!(!ai.city_state_stone_breaker_purchase(&mut off, 0));
    assert_eq!(rams(&off), 0);

    ai.enable_breaker_bought_for_the_siege();
    assert!(ai.city_state_stone_breaker_purchase(&mut game, 0));
    assert_eq!(rams(&game), 1, "a Ram stands");
    assert!(
        !ai.city_state_stone_breaker_purchase(&mut game, 0),
        "one is enough"
    );
    // And with the Ram standing the declaration goes.
    assert!(ai.conquest_declaration(&mut game, 0));
    assert!(game.is_at_war(0, 2));
}

/// The reserved melee bodies follow the city: five against a floor centre,
/// more against the centre the live city-states show by turn 30, never fewer
/// again, and never past the ceiling.
#[test]
fn the_force_grows_to_what_the_city_asks_for() {
    let (mut game, near, _) = fixture();
    stand_walls(&mut game, near);
    Arc::make_mut(&mut game.observed_city_strength).insert(near, 20.0);
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    ai.maintain_conquest_opening(&mut game, 0);
    assert_eq!(
        ai.conquest_quota(),
        (STONE_RANGED, STONE_MELEE),
        "a floor centre asks for no more than the five"
    );
    let opening = ai.conquest_opening.clone().expect("the stone is named");
    let (bodies, each, needed) =
        AdvancedAi::city_state_stone_bodies_needed(&game, &opening).expect("priced");
    assert!((needed - 360.0).abs() < 1e-6, "{needed}");
    assert!((each - 120.0).abs() < 1e-6, "four blows of 30: {each}");
    assert_eq!(bodies, 5, "four finish it and one is spare");

    // A centre of 26: a Warrior lands three blows of about 24, so six
    // finish the 360 and the seventh is the spare.
    Arc::make_mut(&mut game.observed_city_strength).insert(near, 26.0);
    let (bodies, each, _) =
        AdvancedAi::city_state_stone_bodies_needed(&game, &opening).expect("priced");
    assert!(each > 65.0 && each < 75.0, "{each}");
    assert_eq!(bodies, 7);
    ai.maintain_conquest_opening(&mut game, 0);
    assert_eq!(ai.conquest_quota(), (STONE_RANGED, 7));
    assert_eq!(ai.conquest_force_size(), STONE_RANGED + 7);

    // It does not shrink when the centre reads lower again.
    Arc::make_mut(&mut game.observed_city_strength).insert(near, 20.0);
    ai.maintain_conquest_opening(&mut game, 0);
    assert_eq!(ai.conquest_quota(), (STONE_RANGED, 7));

    // A centre no eight Warriors finish asks for more than the ceiling.
    Arc::make_mut(&mut game.observed_city_strength).insert(near, 40.0);
    let (bodies, _, _) =
        AdvancedAi::city_state_stone_bodies_needed(&game, &opening).expect("priced");
    assert!(bodies > STONE_MELEE_MAX, "{bodies}");
}

/// Gold buys the stone's missing melee bodies: one a turn, the Warrior the
/// capital would train, and nothing once the reserved bodies stand, without
/// the Gold, or with the gene off.
#[test]
fn gold_buys_a_missing_melee_body_for_the_stone() {
    let (mut game, near, _) = fixture();
    stand_walls(&mut game, near);
    game.players[0].gold = 2_000.0;
    game.players[0].gold_per_turn = 10.0;
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.city_state_stone_stands());
    let warriors = |game: &Game| {
        game.units
            .values()
            .filter(|unit| unit.owner == 0 && unit.kind.as_str() == "warrior")
            .count()
    };

    // The gene off: no stone, nothing bought.
    let mut off = game.clone();
    let mut plain = seat(false);
    plain.maintain_conquest_opening(&mut off, 0);
    assert!(!plain.city_state_stone_body_purchase(&mut off, 0));
    assert_eq!(warriors(&off), 0);

    // An empty treasury buys nothing.
    let mut poor = game.clone();
    poor.players[0].gold = 0.0;
    assert!(!ai.clone().city_state_stone_body_purchase(&mut poor, 0));
    assert_eq!(warriors(&poor), 0);

    assert!(ai.city_state_stone_body_purchase(&mut game, 0));
    assert_eq!(warriors(&game), 1, "a Warrior was bought");
    assert!(game.players[0].gold < 2_000.0, "and paid for");

    // The reserved bodies all stand: nothing more is bought.
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    force_at(&mut game, rally);
    ai.maintain_conquest_opening(&mut game, 0);
    let held = warriors(&game);
    assert!(!ai.city_state_stone_body_purchase(&mut game, 0));
    assert_eq!(warriors(&game), held);
}

/// While the stone stands, a city of ours near the rally trains the
/// reserved bodies as the capital does; a far one does not, and off the
/// stone the reservation is the capital's alone.
#[test]
fn a_city_near_the_rally_trains_the_stones_force() {
    let (mut game, near, _) = fixture();
    stand_walls(&mut game, near);
    let far_city = game.found_city_for(0, at(45, 20), None);
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.city_state_stone_stands());
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    let capital = game.city_at(at(6, 12)).expect("our capital");
    let second = game.city_at(at(6, 18)).expect("our second city");
    assert!(game.wdist(at(6, 18), rally) <= STONE_BREAKER_PURCHASE_REACH);
    assert!(game.wdist(at(45, 20), rally) > STONE_BREAKER_PURCHASE_REACH);
    let warrior = game.rules.units.get("warrior").unwrap().clone();
    let counts = ai.counts(&game, 0);
    assert!(ai.conquest_reservation(&game, 0, capital, &warrior, &counts, false) > 0.0);
    assert!(
        ai.conquest_reservation(&game, 0, second, &warrior, &counts, false) > 0.0,
        "the second city trains the force too"
    );
    assert!(ai.city_state_stone_trains_in(&game, second));
    assert!(!ai.city_state_stone_trains_in(&game, far_city));
    assert_eq!(
        ai.conquest_reservation(&game, 0, far_city, &warrior, &counts, false),
        0.0,
        "a city past the reach does not"
    );

    // Off the stone no other city is asked.
    let mut plain = seat(false);
    plain.maintain_conquest_opening(&mut game, 0);
    assert!(!plain.city_state_stone_trains_in(&game, second));
    assert_eq!(
        plain.conquest_reservation(&game, 0, second, &warrior, &counts, false),
        0.0
    );
}
