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
        assert_eq!(ai.score_seat_army_target(8, false, 16), 16, "{target:?}");
    }
}

/// At peace the land target stops at one body a city and never rises; at war,
/// or off, it is the stock target.
#[test]
fn the_peacetime_army_is_one_body_a_city() {
    let on = score_seat(true);
    let off = score_seat(false);
    assert_eq!(on.score_seat_army_target(10, false, 17), 10);
    assert_eq!(on.score_seat_army_target(10, false, 6), 6, "never raised");
    assert_eq!(
        on.score_seat_army_target(10, true, 17),
        17,
        "a war keeps it"
    );
    assert_eq!(off.score_seat_army_target(10, false, 17), 17);
    assert!(on.score_seat_holds_the_army(false));
    assert!(!on.score_seat_holds_the_army(true));
    assert!(!off.score_seat_holds_the_army(false));
}

/// The scorer: a seat holding one Warrior for its one city prices another
/// unit as saturated with the gene; off it still buys one (the foundation
/// window asks two a city, plus a buffer). At war the two agree.
#[test]
fn a_unit_past_one_a_city_is_priced_out_at_peace_only() {
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let (g, home) = one_city_with_a_warrior(40);
    let plan = expansion_plan(g.turn);
    let (on, off) = (score_seat(true), score_seat(false));
    let counts = off.counts(&g, 0);
    let value_on = on.production_value(&g, 0, home, &warrior, &plan, &counts);
    let value_off = off.production_value(&g, 0, home, &warrior, &plan, &counts);
    assert_eq!(value_on, -2_000.0, "on prices the unit out");
    assert!(value_off > -2_000.0, "off buys it: {value_off}");

    let (mut war, home) = one_city_with_a_warrior(40);
    war.players[0].met.insert(1);
    war.players[1].met.insert(0);
    war.at_war.insert((0, 1));
    let counts = off.counts(&war, 0);
    assert_eq!(
        on.production_value(&war, 0, home, &warrior, &plan, &counts),
        off.production_value(&war, 0, home, &warrior, &plan, &counts),
        "a declared war keeps the stock army"
    );
}

/// The wonder race opens under the live race's development guards only.
#[test]
fn the_wonder_race_keeps_the_live_guards() {
    let on = score_seat(true);
    // Ten cities, five buildings, a wonder of the world's era, none racing,
    // fifteen turns: open.
    assert!(on.score_seat_wonder_race_opens(10, 5, 4, 4, 0, 15.0));
    assert!(!score_seat(false).score_seat_wonder_race_opens(10, 5, 4, 4, 0, 15.0));
    assert!(
        !on.score_seat_wonder_race_opens(2, 5, 4, 4, 0, 15.0),
        "two cities"
    );
    assert!(
        !on.score_seat_wonder_race_opens(10, 2, 4, 4, 0, 15.0),
        "two buildings"
    );
    assert!(
        !on.score_seat_wonder_race_opens(10, 5, 1, 4, 0, 15.0),
        "three eras behind"
    );
    assert!(
        on.score_seat_wonder_race_opens(10, 5, 2, 4, 0, 15.0),
        "two eras behind"
    );
    // One lane plus one per six cities: ten cities race two at once.
    assert!(on.score_seat_wonder_race_opens(10, 5, 4, 4, 1, 15.0));
    assert!(
        !on.score_seat_wonder_race_opens(10, 5, 4, 4, 2, 15.0),
        "lanes full"
    );
    assert!(
        !on.score_seat_wonder_race_opens(10, 5, 4, 4, 0, CHEAPEST_WONDER_LANE_MAX_TURNS + 1.0),
        "too slow a race"
    );
}

/// The race's price is the live race's bonus, rising with the game.
#[test]
fn the_wonder_price_is_the_live_race_bonus() {
    let (mut g, _) = one_city_with_a_warrior(1);
    let early = AdvancedAi::score_seat_wonder_price(&g);
    g.turn = 170;
    let late = AdvancedAi::score_seat_wonder_price(&g);
    assert!(early >= LIVE_WONDER_RACE_BONUS, "{early}");
    assert!(late > early, "{late} > {early}");
    assert!(early > 180.0 * 5.0, "far above the stock Score lane");
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

/// The journal observes only: off it records nothing; on, a foundation-window
/// seat whose stock target is two a city records the hold once.
#[test]
fn the_journal_records_the_army_hold_once() {
    let (g, _) = one_city_with_a_warrior(40);
    let plan = expansion_plan(g.turn);
    let mut off = score_seat(false);
    off.journal_score_seat(&g, 0, &plan);
    assert!(off.score_seat_journal.army.is_none());

    let mut on = score_seat(true);
    on.journal_score_seat(&g, 0, &plan);
    let Some((target, stock, held, turn)) = on.score_seat_journal.army else {
        panic!("the hold is journalled");
    };
    assert_eq!((target, held, turn), (1, 1, 40));
    assert!(stock > target, "stock {stock}");
    let before = on.score_seat_journal.army;
    on.journal_score_seat(&g, 0, &plan);
    assert_eq!(
        on.score_seat_journal.army, before,
        "unchanged, not repeated"
    );
}
