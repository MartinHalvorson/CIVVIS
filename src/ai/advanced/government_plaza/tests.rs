use super::*;
use crate::ai::advanced::GrandStrategy;

fn board() -> (Game, u32, StrategicPlan) {
    let mut g = Game::new_full(
        2,
        24,
        16,
        crate::rng::fixture_seed("PLAZARESERVE", 91_770),
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
    let cid = g.player_city_ids(0)[0];
    g.players[0].civics.insert(crate::name!("state_workforce"));
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 5.0;
    let city = g.cities.get_mut(&cid).unwrap();
    city.pop = 7;
    city.queue.clear();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, cid, plan)
}

fn queued_plaza(g: &Game, cid: u32) -> bool {
    matches!(g.cities[&cid].queue.first(), Some(Item::District { district, .. })
        if g.district_family(*district) == "government_plaza")
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

/// The idle capital takes the empire's first Plaza with the gene on, and is
/// left alone with the gene off, while it is due a Settler, while it is
/// threatened, and while a Prophet race it has not started a Holy Site for
/// is open.
#[test]
fn the_idle_capital_queues_the_first_plaza_ahead_of_the_shortfall() {
    let (g, cid, plan) = board();
    let mut ai = AdvancedAi::new();

    let mut off = g.clone();
    ai.reserve_government_plaza(&mut off, 0, &plan);
    assert!(off.cities[&cid].queue.is_empty(), "off by default");

    ai.enable_plaza_in_the_district_list();
    let mut due = g.clone();
    ai.reserve_government_plaza(&mut due, 0, &plan);
    assert!(
        due.cities[&cid].queue.is_empty(),
        "a city due a Settler keeps it"
    );
    // At its city target the empire sends no Settler.
    ai.base.w.city_target = 1.0;

    let mut threatened = g.clone();
    let mut held = plan.clone();
    held.threatened_city = Some(cid);
    ai.reserve_government_plaza(&mut threatened, 0, &held);
    assert!(
        threatened.cities[&cid].queue.is_empty(),
        "a threatened city defends"
    );

    ai.base.enter_prophet_race = true;
    let mut race = g.clone();
    ai.reserve_government_plaza(&mut race, 0, &plan);
    assert!(
        race.cities[&cid].queue.is_empty(),
        "an open Prophet race goes first"
    );
    let mut started = g.clone();
    place_district(&mut started, cid, crate::name!("holy_site"));
    ai.reserve_government_plaza(&mut started, 0, &plan);
    assert!(
        queued_plaza(&started, cid),
        "the race under way: {:?}",
        started.cities[&cid].queue
    );
    ai.base.enter_prophet_race = false;

    let mut g = g;
    ai.reserve_government_plaza(&mut g, 0, &plan);
    assert!(queued_plaza(&g, cid), "queue: {:?}", g.cities[&cid].queue);
}

/// With the Plaza standing, its city builds the Warlord's Throne, then the
/// Grand Master's Chapel; no second Plaza is ever queued.
#[test]
fn the_standing_plaza_builds_its_throne_then_its_chapel() {
    let (mut g, cid, plan) = board();
    let mut ai = AdvancedAi::new();
    ai.enable_plaza_in_the_district_list();
    ai.base.w.city_target = 1.0;
    place_district(&mut g, cid, crate::name!("government_plaza"));

    let mut throne = g.clone();
    ai.reserve_government_plaza(&mut throne, 0, &plan);
    assert!(
        matches!(throne.cities[&cid].queue.first(),
            Some(Item::Building { building }) if building == "warlords_throne"),
        "queue: {:?}",
        throne.cities[&cid].queue
    );

    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("warlords_throne"));
    ai.reserve_government_plaza(&mut g, 0, &plan);
    assert!(
        matches!(g.cities[&cid].queue.first(),
            Some(Item::Building { building }) if building == "grand_masters_chapel"),
        "queue: {:?}",
        g.cities[&cid].queue
    );
}
