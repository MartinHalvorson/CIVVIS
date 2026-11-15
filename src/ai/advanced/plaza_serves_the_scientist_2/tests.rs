use super::*;
use crate::ai::advanced::{StrategicPlan, VictoryTarget};

/// One city of ours with a standing Government Plaza and `head` at the head of
/// its queue.
fn plaza_city(head: Item) -> (Game, u32) {
    let mut g = Game::new_full(1, 24, 16, 91_770, 120, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = g.player_city_ids(0)[0];
    g.players[0].civics.insert(crate::name!("state_workforce"));
    let site = g.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            let tile = &g.map.tiles[position];
            *position != g.cities[&city].pos
                && tile.district.is_none()
                && !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
        })
        .unwrap();
    g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("government_plaza"));
    g.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("government_plaza"), site);
    g.cities.get_mut(&city).unwrap().queue = vec![head];
    (g, city)
}

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

fn head(g: &Game, city: u32) -> Option<String> {
    match g.cities[&city].queue.first() {
        Some(Item::Building { building }) => Some(building.to_string()),
        Some(Item::Unit { unit }) => Some(unit.to_string()),
        _ => None,
    }
}

fn scientist(target: VictoryTarget) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_plaza_serves_the_scientist_2();
    ai
}

#[test]
fn the_science_plaza_takes_its_tier_building_ahead_of_routine_work() {
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    let (mut g, city) = plaza_city(builder.clone());

    // Off: nothing changes.
    AdvancedAi::targeting(VictoryTarget::Science).claim_queue_for_the_plaza_tier(
        &mut g,
        0,
        &plan(),
    );
    assert_eq!(head(&g, city).as_deref(), Some("builder"));

    // On, on a Science aim: the Ancestral Hall goes ahead of the Builder.
    let ai = scientist(VictoryTarget::Science);
    assert!(ai.plaza_serves_science_on());
    ai.claim_queue_for_the_plaza_tier(&mut g, 0, &plan());
    assert_eq!(head(&g, city).as_deref(), Some("ancestral_hall"));
    assert_eq!(g.players[0].counters.get("plaza_tier_claims"), Some(&1));

    // Already queued: no second claim.
    ai.claim_queue_for_the_plaza_tier(&mut g, 0, &plan());
    assert_eq!(g.players[0].counters.get("plaza_tier_claims"), Some(&1));
}

#[test]
fn the_science_plaza_leaves_settlers_threats_and_other_aims_alone() {
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    let (mut g, city) = plaza_city(settler);
    scientist(VictoryTarget::Science).claim_queue_for_the_plaza_tier(&mut g, 0, &plan());
    assert_eq!(
        head(&g, city).as_deref(),
        Some("settler"),
        "a Settler is never displaced"
    );

    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    let (mut g, city) = plaza_city(builder.clone());
    let mut threatened = plan();
    threatened.threatened_city = Some(city);
    scientist(VictoryTarget::Science).claim_queue_for_the_plaza_tier(&mut g, 0, &threatened);
    assert_eq!(
        head(&g, city).as_deref(),
        Some("builder"),
        "the threatened city keeps its queue"
    );

    let (mut g, city) = plaza_city(builder);
    let culture = scientist(VictoryTarget::Culture);
    assert!(!culture.plaza_serves_science_on());
    culture.claim_queue_for_the_plaza_tier(&mut g, 0, &plan());
    assert_eq!(
        head(&g, city).as_deref(),
        Some("builder"),
        "off a Science aim"
    );
}

/// Live `civvis-20261113T164001Z`: Antium claimed its first Campus at t34 and
/// the Hall and the Agency went back over it every turn until t58.
#[test]
fn the_tier_building_waits_for_its_citys_first_campus() {
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    let (mut g, city) = plaza_city(builder);
    let site = g.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            let tile = &g.map.tiles[position];
            *position != g.cities[&city].pos
                && tile.district.is_none()
                && !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
        })
        .unwrap();
    let campus = Item::District {
        district: crate::name!("campus"),
        pos: site,
    };
    g.cities.get_mut(&city).unwrap().queue = vec![campus];
    let ai = scientist(VictoryTarget::Science);
    ai.claim_queue_for_the_plaza_tier(&mut g, 0, &plan());
    assert_eq!(
        head_district(&g, city).as_deref(),
        Some("campus"),
        "the first Campus keeps the head"
    );
    assert_eq!(g.players[0].counters.get("plaza_tier_claims"), None);

    // Once a Campus stands, a district at the head is routine work again.
    g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("campus"));
    g.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), site);
    ai.claim_queue_for_the_plaza_tier(&mut g, 0, &plan());
    assert_eq!(head(&g, city).as_deref(), Some("ancestral_hall"));
    assert_eq!(g.players[0].counters.get("plaza_tier_claims"), Some(&1));
}

/// One capital of ours with Political Philosophy's state workforce, no Plaza,
/// `head` at the head of its queue, and a Campus when `campus`; population 7,
/// so a Campus leaves it a free district slot.
fn capital_without_a_plaza(head: Item, campus: bool) -> (Game, u32) {
    let mut g = Game::new_full(1, 24, 16, 91_770, 120, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = g.player_city_ids(0)[0];
    g.players[0].civics.insert(crate::name!("state_workforce"));
    if campus {
        let site = g.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| {
                let tile = &g.map.tiles[position];
                *position != g.cities[&city].pos
                    && tile.district.is_none()
                    && !g.rules.is_water(tile)
                    && g.rules.is_passable(tile)
            })
            .unwrap();
        g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("campus"));
        g.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(crate::name!("campus"), site);
    }
    let capital = g.cities.get_mut(&city).unwrap();
    capital.pop = 7;
    capital.queue = vec![head];
    (g, city)
}

fn head_district(g: &Game, city: u32) -> Option<String> {
    match g.cities[&city].queue.first() {
        Some(Item::District { district, .. }) => Some(g.district_family(*district).to_string()),
        Some(Item::Unit { unit }) => Some(unit.to_string()),
        _ => None,
    }
}

#[test]
fn version_three_starts_the_plaza_once_the_capital_campus_stands() {
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    let mut v3 = AdvancedAi::targeting(VictoryTarget::Science);
    v3.enable_plaza_serves_the_scientist_3();
    assert!(
        v3.plaza_serves_science_on(),
        "v3 carries the Science tier list"
    );

    let (mut g, city) = capital_without_a_plaza(builder.clone(), true);
    AdvancedAi::targeting(VictoryTarget::Science).claim_queue_for_the_plaza_district(
        &mut g,
        0,
        &plan(),
    );
    assert_eq!(
        head_district(&g, city).as_deref(),
        Some("builder"),
        "off: nothing"
    );
    v3.claim_queue_for_the_plaza_district(&mut g, 0, &plan());
    assert_eq!(
        head_district(&g, city).as_deref(),
        Some("government_plaza"),
        "the Plaza goes ahead of the Builder"
    );
    assert_eq!(g.players[0].counters.get("plaza_district_claims"), Some(&1));
    v3.claim_queue_for_the_plaza_district(&mut g, 0, &plan());
    assert_eq!(
        g.players[0].counters.get("plaza_district_claims"),
        Some(&1),
        "a Plaza at a queue's head is not claimed twice"
    );

    let (mut g, city) = capital_without_a_plaza(builder.clone(), false);
    v3.claim_queue_for_the_plaza_district(&mut g, 0, &plan());
    assert_eq!(
        head_district(&g, city).as_deref(),
        Some("builder"),
        "the capital's Campus comes first"
    );

    let (mut g, city) = capital_without_a_plaza(builder, true);
    let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
    culture.enable_plaza_serves_the_scientist_3();
    culture.claim_queue_for_the_plaza_district(&mut g, 0, &plan());
    assert_eq!(
        head_district(&g, city).as_deref(),
        Some("builder"),
        "off a Science aim"
    );
}
