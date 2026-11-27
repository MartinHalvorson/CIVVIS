use super::*;
use crate::game::install_test_district;

/// A founded Religion seat with a population-3 capital (Holy Site, the queue
/// as given) and a population-1 second city, on open grassland at turn 30.
fn two_cities(head: Option<Item>) -> (Game, u32) {
    let mut g = Game::new_full(2, 60, 24, 936_312, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (6, 8), None);
    let capital = g.player_city_ids(0)[0];
    g.found_city_for(0, (12, 8), None);
    install_test_district(&mut g, capital, "holy_site");
    g.players[0].religion = Some("Buddhism".into());
    for tech in ["pottery", "astrology", "mining", "bronze_working"] {
        g.players[0].techs.insert(Name::new(tech));
    }
    let city = g.cities.get_mut(&capital).unwrap();
    city.pressure.insert("Buddhism".into(), 500.0);
    city.pop = 3;
    city.queue = head.into_iter().collect();
    g.current = 0;
    g.turn = 30;
    (g, capital)
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_religion_seat_first_shrine();
    if gene {
        ai.enable_religion_seat_third_city_first();
    }
    ai
}

fn plan_for(g: &Game, ai: &AdvancedAi) -> StrategicPlan {
    let mut plan = ai.assess(g, 0);
    plan.threatened_city = None;
    plan
}

fn claim(ai: &AdvancedAi, g: &mut Game) {
    let plan = plan_for(g, ai);
    ai.claim_religion_seat_third_city(g, 0, &plan);
}

fn settler() -> Item {
    AdvancedAi::second_city_settler()
}

fn shrine() -> Item {
    Item::Building {
        building: crate::name!("shrine"),
    }
}

fn builder() -> Item {
    Item::Unit {
        unit: crate::name!("builder"),
    }
}

/// The shape of `civvis-t8-20261126T093036Z` t25: the founding turn's first
/// Shrine went ahead of the capital's Settler. With the gene the Settler
/// keeps the head; off, the Shrine still takes it.
#[test]
fn the_first_shrine_leaves_the_third_city_settler_alone() {
    let (mut on, capital) = two_cities(Some(settler()));
    let ai = seat(VictoryTarget::Religion, true);
    let plan = plan_for(&on, &ai);
    ai.claim_religion_seat_first_shrine(&mut on, 0, &plan);
    assert_eq!(on.cities[&capital].queue.first(), Some(&settler()));

    let (mut off, _) = two_cities(Some(settler()));
    let stock = seat(VictoryTarget::Religion, false);
    let plan = plan_for(&off, &stock);
    stock.claim_religion_seat_first_shrine(&mut off, 0, &plan);
    assert_eq!(off.cities[&capital].queue.first(), Some(&shrine()));
}

/// A fresh Shrine, a Builder and an idle queue all give way to the third
/// city's Settler in the city that trains it soonest; the population-1
/// second city is passed over.
#[test]
fn the_third_city_settler_takes_the_queue() {
    for head in [None, Some(shrine()), Some(builder())] {
        let (mut g, capital) = two_cities(head.clone());
        claim(&seat(VictoryTarget::Religion, true), &mut g);
        assert_eq!(
            g.cities[&capital].queue.first(),
            Some(&settler()),
            "head {head:?}"
        );
        let second = g.player_city_ids(0)[1];
        assert_ne!(g.cities[&second].queue.first(), Some(&settler()));
    }
}

/// Off, for another target, past the band turn, with a Settler already out
/// or with a third city, nothing changes.
#[test]
fn nothing_changes_off_or_once_the_seat_is_past_the_window() {
    let check = |ai: &AdvancedAi, g: &mut Game, capital: u32, why: &str| {
        claim(ai, g);
        assert_eq!(g.cities[&capital].queue.first(), Some(&builder()), "{why}");
    };
    let (mut g, capital) = two_cities(Some(builder()));
    check(
        &seat(VictoryTarget::Religion, false),
        &mut g,
        capital,
        "off",
    );
    let (mut g, capital) = two_cities(Some(builder()));
    check(
        &seat(VictoryTarget::Science, true),
        &mut g,
        capital,
        "science",
    );

    let ai = seat(VictoryTarget::Religion, true);
    let (mut late, capital) = two_cities(Some(builder()));
    late.turn = late.standard_duration(EARLY_SETTLER_FLOOR_STANDARD_TURN);
    check(&ai, &mut late, capital, "past the band turn");

    let (mut walking, capital) = two_cities(Some(builder()));
    let home = walking.cities[&capital].pos;
    walking.spawn_unit("settler", 0, home);
    check(&ai, &mut walking, capital, "a Settler out");

    let (mut three, capital) = two_cities(Some(builder()));
    three.found_city_for(0, (18, 8), None);
    check(&ai, &mut three, capital, "three cities");

    let (mut small, capital) = two_cities(Some(builder()));
    small.cities.get_mut(&capital).unwrap().pop = 1;
    check(&ai, &mut small, capital, "below the Settler floor");
}

/// A head finishing this turn keeps its place, as in the second-city gene.
#[test]
fn a_head_finishing_this_turn_keeps_the_queue() {
    let (mut g, capital) = two_cities(Some(builder()));
    let cost = g.item_cost_for_city(0, capital, &builder());
    g.cities
        .get_mut(&capital)
        .unwrap()
        .production_progress
        .insert(Game::item_progress_key(&builder()), cost - 0.5);
    claim(&seat(VictoryTarget::Religion, true), &mut g);
    assert_eq!(g.cities[&capital].queue.first(), Some(&builder()));
}
