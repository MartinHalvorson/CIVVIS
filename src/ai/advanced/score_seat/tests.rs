use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// A Score seat, with or without the gene.
fn score_seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Score);
    if gene {
        ai.enable_score_seat_plays_for_points();
    }
    ai
}

/// A Score seat with both genes on (`trim`) or with the wonder gene alone.
fn trimming_seat(trim: bool) -> AdvancedAi {
    let mut ai = score_seat(true);
    if trim {
        ai.enable_score_seat_trims_the_idle_army();
    }
    ai
}

/// Sets a player's reported military.
fn set_power(g: &mut Game, pid: usize, power: f64) {
    std::sync::Arc::make_mut(&mut g.observed_military_power).insert(pid, power);
}

/// [`one_city_with_a_warrior`] at Online speed grown to four cities of ours,
/// each with a Warrior on its tile, a city of the met rival eight tiles from
/// our capital, and our military at 300 against the rival's `rival_power`.
/// Returns the game, our cities (capital first) and the rival's near city.
fn four_cities(turn: u32, rival_power: f64) -> (Game, Vec<u32>, u32) {
    let (mut g, home) = one_city_with_a_warrior(turn);
    g.game_speed = crate::setup::GameSpeed::Online;
    let centre = g.cities[&home].pos;
    let mut ours = vec![centre];
    let mut land: Vec<Pos> = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| !g.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .collect();
    land.sort();
    let occupied: Vec<Pos> = g.cities.values().map(|city| city.pos).collect();
    for pos in &land {
        if ours.len() == 4 {
            break;
        }
        let from_home = g.wdist(*pos, centre);
        if (4..=6).contains(&from_home)
            && ours.iter().all(|other| g.wdist(*pos, *other) >= 4)
            && occupied.iter().all(|other| g.wdist(*pos, *other) >= 4)
        {
            ours.push(*pos);
        }
    }
    assert_eq!(ours.len(), 4, "room for four cities");
    let rival_site = *land
        .iter()
        .find(|pos| {
            g.wdist(**pos, centre) == 8
                && ours.iter().all(|other| g.wdist(**pos, *other) >= 4)
                && occupied.iter().all(|other| g.wdist(**pos, *other) >= 4)
        })
        .expect("a rival site eight tiles out");
    for pos in &ours[1..] {
        g.place_city(0, *pos, None);
        g.spawn_unit("warrior", 0, *pos);
    }
    let rival = g.place_city(1, rival_site, None);
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    set_power(&mut g, 0, 300.0);
    set_power(&mut g, 1, rival_power);
    let mut cities = g.player_city_ids(0);
    cities.sort_by_key(|cid| *cid != home);
    (g, cities, rival)
}

fn expansion_plan(turn: u32) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: turn,
        rush: false,
    }
}

/// Two majors with one founded city each, every unit removed, one Warrior
/// standing in our city, turn `turn` of a 170-turn game. Returns the game and
/// our city.
fn one_city_with_a_warrior(turn: u32) -> (Game, u32) {
    let mut g = Game::new_full(2, 40, 26, 170_112, 170, 0, false);
    g.current = 0;
    let settlers: Vec<_> = g
        .units
        .values()
        .filter(|u| u.kind == "settler")
        .map(|u| (u.owner, u.pos))
        .collect();
    for (owner, pos) in settlers {
        g.found_city_for(owner, pos, None);
    }
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    let home = g.player_city_ids(0)[0];
    let pos = g.cities[&home].pos;
    g.spawn_unit("warrior", 0, pos);
    g.players[0].gold = 300.0;
    g.players[0].gold_per_turn = 10.0;
    g.max_turns = 170;
    g.turn = turn;
    (g, home)
}

#[test]
fn score_seat_plays_for_points_is_opt_in() {
    opt_in_off_in_both_controllers("score-seat-plays-for-points", |ai| {
        ai.score_seat_plays_for_points
    });
}

/// The gene acts only on a Score target.
#[test]
fn the_gene_acts_only_on_a_score_target() {
    assert!(score_seat(true).score_seat_on());
    assert!(!score_seat(false).score_seat_on());
    for target in [
        VictoryTarget::Science,
        VictoryTarget::Culture,
        VictoryTarget::Domination,
    ] {
        let mut ai = AdvancedAi::targeting(target);
        ai.enable_score_seat_plays_for_points();
        assert!(!ai.score_seat_on(), "{target:?}");
        ai.enable_score_seat_trims_the_idle_army();
        let (g, _, _) = four_cities(100, 100.0);
        assert!(!ai.score_seat_trims_the_army(&g, 0, false), "{target:?}");
    }
}

#[test]
fn score_seat_trims_the_idle_army_is_opt_in() {
    opt_in_off_in_both_controllers("score-seat-trims-the-idle-army", |ai| {
        ai.score_seat_trims_the_idle_army
    });
}

/// The trimmed target: one body a city with four at least, never above the
/// stock target; untrimmed it is the stock target.
#[test]
fn the_trimmed_target_is_one_a_city_with_four_at_least() {
    let on = trimming_seat(true);
    assert_eq!(on.score_seat_army_target(10, true, 17), 10);
    assert_eq!(on.score_seat_army_target(2, true, 6), 4, "four at least");
    assert_eq!(on.score_seat_army_target(10, true, 6), 6, "never raised");
    assert_eq!(on.score_seat_army_target(10, false, 17), 17);
}

/// ★ The t1 case the live arm lost the capital to (civvis-20261112T211954Z):
/// one city, turn 1 and turn 40, both genes on. The trim does not act, so the
/// target is the stock target and a second soldier is priced exactly as off.
#[test]
fn one_city_keeps_the_stock_army() {
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    for turn in [1, 40] {
        let (g, home) = one_city_with_a_warrior(turn);
        let plan = expansion_plan(g.turn);
        let (on, off) = (trimming_seat(true), score_seat(false));
        assert!(!on.score_seat_trims_the_army(&g, 0, false), "t{turn}");
        assert_eq!(on.score_seat_army_target(1, false, 2), 2, "t{turn}");
        let counts = off.counts(&g, 0);
        let value_on = on.production_value(&g, 0, home, &warrior, &plan, &counts);
        let value_off = off.production_value(&g, 0, home, &warrior, &plan, &counts);
        assert_eq!(value_on, value_off, "t{turn}");
        assert!(value_on > -2_000.0, "t{turn}: the second soldier is bought");
    }
}

/// From turn 80 (Online), four garrisoned cities at 300 against a neighbour's
/// 360 (0.83, above the 0.8 floor): the trim prices a fifth soldier out; the
/// stock asks five (the 1.2 deterrence) and buys it. The wonder gene alone
/// leaves the army as stock.
#[test]
fn the_trim_bites_past_one_a_city_above_the_neighbour_floor() {
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let (g, cities, _) = four_cities(100, 360.0);
    let plan = expansion_plan(g.turn);
    let (on, wonders_only, off) = (trimming_seat(true), trimming_seat(false), score_seat(false));
    assert!(on.score_seat_trims_the_army(&g, 0, false));
    assert!(!wonders_only.score_seat_trims_the_army(&g, 0, false));
    let counts = off.counts(&g, 0);
    let home = cities[0];
    let value_on = on.production_value(&g, 0, home, &warrior, &plan, &counts);
    let value_off = off.production_value(&g, 0, home, &warrior, &plan, &counts);
    assert_eq!(value_on, -2_000.0, "the trim prices the fifth soldier out");
    assert!(value_off > -2_000.0, "stock buys it: {value_off}");
    assert_eq!(
        wonders_only.production_value(&g, 0, home, &warrior, &plan, &counts),
        value_off,
        "the wonder gene alone keeps the stock army"
    );
}

/// ★ The neighbour floor: at 300 against 400 (0.75) the trim does not act and
/// the soldier is priced as stock; before turn 80, or with three cities, or
/// at war, neither.
#[test]
fn the_trim_keeps_the_neighbour_floor_the_clock_and_the_war() {
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let on = trimming_seat(true);
    let off = score_seat(false);
    let (g, cities, _) = four_cities(100, 400.0);
    assert!(!on.score_seat_trims_the_army(&g, 0, false), "below 0.8");
    let plan = expansion_plan(g.turn);
    let counts = off.counts(&g, 0);
    assert_eq!(
        on.production_value(&g, 0, cities[0], &warrior, &plan, &counts),
        off.production_value(&g, 0, cities[0], &warrior, &plan, &counts),
        "outgunned: the stock army"
    );
    let (early, _, _) = four_cities(79, 100.0);
    assert!(
        !on.score_seat_trims_the_army(&early, 0, false),
        "before t80"
    );
    let (late, _, _) = four_cities(80, 100.0);
    assert!(on.score_seat_trims_the_army(&late, 0, false), "from t80");
    let (mut three, cities, _) = four_cities(100, 100.0);
    three.cities.get_mut(&cities[3]).unwrap().owner = 1;
    assert!(
        !on.score_seat_trims_the_army(&three, 0, false),
        "three cities"
    );
    let (g, _, _) = four_cities(100, 100.0);
    assert!(!on.score_seat_trims_the_army(&g, 0, true), "a war");
}

/// A frontier city with no soldier on its tile keeps the stock pricing while
/// the trim acts elsewhere.
#[test]
fn a_frontier_city_keeps_its_garrison() {
    let (mut g, cities, rival) = four_cities(100, 100.0);
    let on = trimming_seat(true);
    let rival_pos = g.cities[&rival].pos;
    let frontier = *cities
        .iter()
        .find(|cid| g.wdist(g.cities[*cid].pos, rival_pos) <= SCORE_TRIM_NEIGHBOUR_TILES)
        .expect("the capital is eight tiles from the rival");
    assert!(on.score_seat_trims_here(&g, 0, frontier, false));
    let pos = g.cities[&frontier].pos;
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        if g.units[&uid].owner == 0 && g.units[&uid].pos == pos {
            g.remove_unit(uid);
        }
    }
    assert!(AdvancedAi::score_seat_frontier_without_garrison(
        &g, 0, frontier
    ));
    assert!(on.score_seat_trims_the_army(&g, 0, false));
    assert!(!on.score_seat_trims_here(&g, 0, frontier, false));
}

/// The last turns: an item finishing after the tally is priced out, a
/// Settler also needs its walk, a Repair never is, and nothing is priced out
/// before the window, off, or off a Score target.
#[test]
fn nothing_that_cannot_finish_is_started_in_the_last_turns() {
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    let repair = Item::Repair {
        repair: crate::name!("district"),
        pos: (0, 0),
    };
    let on = score_seat(true);
    let (g, _) = one_city_with_a_warrior(160);
    // Turn 160 of 170: eleven turns still finish before the tally.
    assert!(!on.score_seat_misses_the_tally(&g, &monument, 11.0));
    assert!(on.score_seat_misses_the_tally(&g, &monument, 11.5));
    assert!(on.score_seat_misses_the_tally(&g, &monument, 12.0));
    assert!(!on.score_seat_misses_the_tally(&g, &settler, 7.0));
    assert!(
        on.score_seat_misses_the_tally(&g, &settler, 8.0),
        "it must walk"
    );
    assert!(!on.score_seat_misses_the_tally(&g, &repair, 40.0));
    assert!(!score_seat(false).score_seat_misses_the_tally(&g, &monument, 40.0));
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_score_seat_plays_for_points();
    assert!(!science.score_seat_misses_the_tally(&g, &monument, 40.0));
    let (early, _) = one_city_with_a_warrior(150);
    assert!(
        !on.score_seat_misses_the_tally(&early, &monument, 40.0),
        "twenty-one turns out the window is shut"
    );
    let (mut endless, _) = one_city_with_a_warrior(160);
    endless.max_turns = 0;
    assert!(!on.score_seat_misses_the_tally(&endless, &monument, 40.0));
}

/// The journal observes only: off, or with the wonder gene alone, or in the
/// t40 one-city case, it records no army line; with the trim acting it
/// records the hold once.
#[test]
fn the_journal_records_the_army_hold_once() {
    let (g, _) = one_city_with_a_warrior(40);
    let plan = expansion_plan(g.turn);
    let mut off = score_seat(false);
    off.journal_score_seat(&g, 0, &plan);
    assert!(off.score_seat_journal.army.is_none());
    let mut early = trimming_seat(true);
    early.journal_score_seat(&g, 0, &plan);
    assert!(early.score_seat_journal.army.is_none(), "one city at t40");

    let (g, _, _) = four_cities(100, 360.0);
    let plan = expansion_plan(g.turn);
    let mut wonders_only = trimming_seat(false);
    wonders_only.journal_score_seat(&g, 0, &plan);
    assert!(wonders_only.score_seat_journal.army.is_none());
    // The live seat carries `peacetime-deterrence` (a repair): the stock asks
    // 1.2 a city against the neighbour's 360, five, where the trim holds four.
    let mut on = trimming_seat(true);
    on.enable_peacetime_deterrence();
    on.journal_score_seat(&g, 0, &plan);
    let Some((target, stock, held, turn)) = on.score_seat_journal.army else {
        panic!("the hold is journalled");
    };
    assert_eq!((target, held, turn), (4, 4, 100));
    assert!(stock > target, "stock {stock}");
    let before = on.score_seat_journal.army;
    on.journal_score_seat(&g, 0, &plan);
    assert_eq!(
        on.score_seat_journal.army, before,
        "unchanged, not repeated"
    );
}
