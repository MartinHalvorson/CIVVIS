use super::*;
use crate::ai::advanced::GrandStrategy;

/// A capital of population 7 with Writing and State Workforce, and two
/// more cities of ours, every queue empty.
fn board() -> (Game, u32, Vec<u32>, StrategicPlan) {
    let mut g = Game::new_full(
        2,
        24,
        16,
        crate::rng::fixture_seed("CAPITALCAMPUS", 91_771),
        250,
        0,
        false,
    );
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = g.player_city_ids(0)[0];
    let center = g.cities[&capital].pos;
    let mut others = Vec::new();
    for (dq, dr) in [(5, 0), (-5, 0), (0, 5), (0, -5), (5, -5), (-5, 5)] {
        if others.len() == 2 {
            break;
        }
        let pos = (center.0 + dq, center.1 + dr);
        let fits = g.map.tiles.get(&pos).is_some_and(|tile| {
            tile.owner_city.is_none() && !g.rules.is_water(tile) && g.rules.is_passable(tile)
        });
        if fits {
            others.push(g.place_city(0, pos, None));
        }
    }
    assert_eq!(others.len(), 2, "the fixture needs two more cities");
    g.players[0].techs.insert(crate::name!("writing"));
    g.players[0].civics.insert(crate::name!("state_workforce"));
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 5.0;
    for cid in g.player_city_ids(0) {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pop = 7;
        city.queue.clear();
    }
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, capital, others, plan)
}

fn leads_with(g: &Game, cid: u32, family: &str) -> bool {
    match g.cities[&cid].queue.first() {
        Some(Item::District { district, .. }) => g.district_family(*district) == family,
        Some(Item::Building { building }) => building == family,
        _ => false,
    }
}

fn place_district(g: &mut Game, cid: u32, district: crate::name::Name) {
    let center = g.cities[&cid].pos;
    let site = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            let tile = &g.map.tiles[position];
            *position != center
                && tile.district.is_none()
                && !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
        })
        .unwrap();
    g.map.tiles.get_mut(&site).unwrap().district = Some(district);
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(district, site);
}

/// Off by default; on, the idle capital queues its first Campus, and once
/// the Campus stands, its Library.
#[test]
fn the_idle_capital_queues_its_campus_then_its_library() {
    let (g, capital, _, plan) = board();
    let mut ai = AdvancedAi::new();
    ai.base.w.city_target = 1.0;

    let mut off = g.clone();
    ai.claim_capital_campus(&mut off, 0, &plan);
    assert!(off.cities[&capital].queue.is_empty(), "off by default");

    ai.enable_capital_campus_before_the_plaza();
    let mut on = g.clone();
    ai.claim_capital_campus(&mut on, 0, &plan);
    assert!(
        leads_with(&on, capital, "campus"),
        "queue: {:?}",
        on.cities[&capital].queue
    );

    let mut standing = g.clone();
    place_district(&mut standing, capital, crate::name!("campus"));
    ai.claim_capital_campus(&mut standing, 0, &plan);
    assert!(
        leads_with(&standing, capital, "library"),
        "queue: {:?}",
        standing.cities[&capital].queue
    );
}

/// The guards the Plaza claim keeps: a capital due a Settler, a threatened
/// capital, an open Prophet race and an empire of fewer than three cities
/// leave the capital alone.
#[test]
fn the_capital_keeps_its_settler_its_defence_and_the_race() {
    let (g, capital, others, plan) = board();
    let mut ai = AdvancedAi::new();
    ai.enable_capital_campus_before_the_plaza();

    // Short of its city target the capital is due a Settler.
    ai.base.w.city_target = 12.0;
    let mut due = g.clone();
    ai.claim_capital_campus(&mut due, 0, &plan);
    assert!(
        due.cities[&capital].queue.is_empty(),
        "a due Settler keeps it"
    );
    ai.base.w.city_target = 1.0;

    let mut threatened = g.clone();
    let mut held = plan.clone();
    held.threatened_city = Some(capital);
    ai.claim_capital_campus(&mut threatened, 0, &held);
    assert!(
        threatened.cities[&capital].queue.is_empty(),
        "a threatened capital defends"
    );

    ai.base.enter_prophet_race = true;
    let mut race = g.clone();
    ai.claim_capital_campus(&mut race, 0, &plan);
    assert!(
        race.cities[&capital].queue.is_empty(),
        "an open Prophet race goes first"
    );
    ai.base.enter_prophet_race = false;

    let mut small = g.clone();
    small.cities.remove(&others[1]);
    ai.claim_capital_campus(&mut small, 0, &plan);
    assert!(
        small.cities[&capital].queue.is_empty(),
        "the opening keeps its capital"
    );
}

/// With both genes on, the capital takes its Campus and the empire's first
/// Plaza goes to another idle city the same turn; with this gene off the
/// Plaza takes the capital, as before.
#[test]
fn the_plaza_goes_to_another_city_while_the_capital_builds_its_campus() {
    let (g, capital, others, plan) = board();
    let mut ai = AdvancedAi::new();
    ai.base.w.city_target = 1.0;
    ai.enable_plaza_in_the_district_list();

    let mut before = g.clone();
    ai.claim_capital_campus(&mut before, 0, &plan);
    ai.reserve_government_plaza(&mut before, 0, &plan);
    assert!(
        leads_with(&before, capital, "government_plaza"),
        "queue: {:?}",
        before.cities[&capital].queue
    );

    ai.enable_capital_campus_before_the_plaza();
    let mut after = g.clone();
    ai.claim_capital_campus(&mut after, 0, &plan);
    ai.reserve_government_plaza(&mut after, 0, &plan);
    assert!(
        leads_with(&after, capital, "campus"),
        "capital: {:?}",
        after.cities[&capital].queue
    );
    assert!(
        others
            .iter()
            .any(|cid| leads_with(&after, *cid, "government_plaza")),
        "others: {:?}",
        others
            .iter()
            .map(|cid| after.cities[cid].queue.first().cloned())
            .collect::<Vec<_>>()
    );
}
