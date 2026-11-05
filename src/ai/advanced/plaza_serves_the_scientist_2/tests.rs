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
