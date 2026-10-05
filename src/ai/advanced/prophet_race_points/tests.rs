use super::*;
use crate::ai::advanced::GrandStrategy;

fn board() -> (Game, u32, StrategicPlan) {
    let mut g = Game::new_full(
        2,
        24,
        16,
        crate::rng::fixture_seed("RACESHRINE", 47_056),
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
    g.players[0].techs.insert(crate::name!("astrology"));
    g.players[0].gold = 175.0;
    g.players[0].gold_per_turn = 5.0;
    let city = g.cities.get_mut(&cid).unwrap();
    city.pop = 5;
    city.queue.clear();
    let center = city.pos;
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
    g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("holy_site"));
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("holy_site"), site);
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

fn racer() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_enter_the_prophet_race_2();
    ai
}

fn shrine_first(g: &Game, cid: u32) -> bool {
    matches!(g.cities[&cid].queue.first(),
        Some(Item::Building { building }) if building == "shrine")
}

/// With a Prophet open, the Holy Site city puts its Shrine in front of a
/// Scout and banks the Scout's progress; the gene off, a founded religion, a
/// closed race, a threatened city and a standing Shrine all leave the queue.
#[test]
fn the_race_shrine_goes_in_front_of_the_holy_site_city_queue() {
    let (mut g, cid, plan) = board();
    let scout = Item::Unit {
        unit: crate::name!("scout"),
    };
    g.apply(0, &Action::Produce { city: cid, item: scout.clone() }).unwrap();
    g.cities.get_mut(&cid).unwrap().production = 4.0;
    let mut ai = racer();

    let mut off = g.clone();
    ai.reserve_race_shrine(&mut off, 0, &plan);
    assert_eq!(off.cities[&cid].queue.first(), Some(&scout), "off by default");

    ai.enable_prophet_race_earns_its_points();

    let mut threatened = g.clone();
    let mut held = plan.clone();
    held.threatened_city = Some(cid);
    ai.reserve_race_shrine(&mut threatened, 0, &held);
    assert_eq!(threatened.cities[&cid].queue.first(), Some(&scout), "a threatened city defends");

    let mut founded = g.clone();
    founded.players[0].religion = Some("RELIGION_BUDDHISM".to_string());
    ai.reserve_race_shrine(&mut founded, 0, &plan);
    assert_eq!(founded.cities[&cid].queue.first(), Some(&scout), "the race is won");

    // The rival's religion and the Prophet it holds take both of a duel
    // map's slots.
    let mut closed = g.clone();
    closed.players[1].religion = Some("RELIGION_HINDUISM".to_string());
    closed.players[1].prophet_pending = true;
    assert_eq!(closed.max_religions(), 2);
    ai.reserve_race_shrine(&mut closed, 0, &plan);
    assert_eq!(closed.cities[&cid].queue.first(), Some(&scout), "the race is closed");

    let mut skipped = g.clone();
    ai.base.skip_prophet_race = true;
    ai.reserve_race_shrine(&mut skipped, 0, &plan);
    assert_eq!(skipped.cities[&cid].queue.first(), Some(&scout), "the race is called off");
    ai.base.skip_prophet_race = false;

    let mut standing = g.clone();
    standing
        .cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("shrine"));
    ai.reserve_race_shrine(&mut standing, 0, &plan);
    assert_eq!(standing.cities[&cid].queue.first(), Some(&scout), "the Shrine stands");

    ai.reserve_race_shrine(&mut g, 0, &plan);
    assert!(shrine_first(&g, cid), "queue: {:?}", g.cities[&cid].queue);
    assert_eq!(g.item_invested_production(cid, &scout), 4.0, "the Scout keeps its progress");
    // Idempotent on the next frame.
    ai.reserve_race_shrine(&mut g, 0, &plan);
    assert!(shrine_first(&g, cid));
}

/// An item that finishes this turn, a district and a wonder keep the queue;
/// an idle Holy Site city takes the Shrine.
#[test]
fn the_race_shrine_waits_for_a_finishing_item_and_takes_an_idle_queue() {
    let (g, cid, plan) = board();
    let mut ai = racer();
    ai.enable_prophet_race_earns_its_points();

    let scout = Item::Unit {
        unit: crate::name!("scout"),
    };
    let mut finishing = g.clone();
    finishing
        .apply(0, &Action::Produce { city: cid, item: scout.clone() })
        .unwrap();
    let cost = finishing.item_cost_for_city(0, cid, &scout);
    finishing.cities.get_mut(&cid).unwrap().production = cost - 0.5;
    assert!(finishing.city_yields(cid).production >= 0.5);
    ai.reserve_race_shrine(&mut finishing, 0, &plan);
    assert_eq!(finishing.cities[&cid].queue.first(), Some(&scout), "the Scout finishes first");

    let mut idle = g.clone();
    ai.reserve_race_shrine(&mut idle, 0, &plan);
    assert!(shrine_first(&idle, cid), "queue: {:?}", idle.cities[&cid].queue);
}

/// The governor keeps the race's Shrine only while the race still wants it.
#[test]
fn the_governor_keeps_the_race_shrine_while_the_race_is_open() {
    let (mut g, cid, _) = board();
    let shrine = Item::Building {
        building: crate::name!("shrine"),
    };
    let mut ai = racer();
    assert!(!ai.race_shrine_committed(&g, 0, cid, &shrine), "off by default");
    ai.enable_prophet_race_earns_its_points();
    assert!(ai.race_shrine_committed(&g, 0, cid, &shrine));
    let library = Item::Building {
        building: crate::name!("library"),
    };
    assert!(!ai.race_shrine_committed(&g, 0, cid, &library));
    g.players[0].prophet_pending = true;
    assert!(!ai.race_shrine_committed(&g, 0, cid, &shrine), "the Prophet is in hand");
}

fn oligarchy(g: &mut Game) {
    g.players[0].government = Some("oligarchy".to_string());
    g.players[0].civics.extend([
        crate::name!("code_of_laws"),
        crate::name!("craftsmanship"),
        crate::name!("early_empire"),
        crate::name!("foreign_trade"),
        crate::name!("mysticism"),
        crate::name!("political_philosophy"),
    ]);
    g.players[0].policies.clear();
    g.players[0].policies.extend([
        crate::name!("agoge"),
        crate::name!("discipline"),
        crate::name!("colonization"),
        crate::name!("urban_planning"),
    ]);
}

/// Under Oligarchy with Mysticism, Revelation takes the wildcard seat while
/// the race is open, Holy Site or not, and is never asked for once the seat
/// holds a religion.
#[test]
fn revelation_takes_the_wildcard_while_a_prophet_is_open() {
    let (mut g, _, _) = board();
    oligarchy(&mut g);
    assert!(
        g.available_policies(0).contains(&crate::name!("revelation")),
        "Mysticism offers Revelation"
    );
    let mut ai = racer();

    let mut off = g.clone();
    ai.strategic_policies(&mut off, 0, GrandStrategy::Conquest);
    assert!(!off.players[0].policies.contains(&crate::name!("revelation")), "off by default");

    ai.enable_prophet_race_earns_its_points();
    let mut founded = g.clone();
    founded.players[0].religion = Some("RELIGION_BUDDHISM".to_string());
    assert!(!ai.race_wants_revelation(&founded, 0));

    let mut on = g.clone();
    ai.strategic_policies(&mut on, 0, GrandStrategy::Conquest);
    let held = &on.players[0].policies;
    assert!(held.contains(&crate::name!("revelation")), "deck: {held:?}");
    assert_eq!(held.len(), 4, "deck: {held:?}");
}
