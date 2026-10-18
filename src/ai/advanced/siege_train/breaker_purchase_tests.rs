//! `breaker-bought-for-the-siege`: the wall-breaker a held siege waits for.

use std::sync::Arc;

use super::tests::walled_city;
use super::*;

/// `walled_city` (Ancient Walls, 100 points) at war, a city of ours six to
/// nine tiles away with Masonry and a full treasury, and
/// `melee` Swordsmen plus `ranged` Archers two tiles from the walls.
fn held_siege(melee: usize, ranged: usize) -> (Game, AdvancedAi, u32, u32) {
    let (mut g, cid) = walled_city();
    // Off the arena: an arena builds and buys only fighting units.
    g.map_script = crate::setup::MapScript::Pangaea;
    g.at_war.insert((0, 1));
    g.record_contact(0, 1);
    let target = g.cities[&cid].pos;
    let site = g
        .map
        .tiles
        .values()
        .filter(|tile| {
            g.rules.is_passable(tile)
                && !g.rules.is_water(tile)
                && tile.owner_city.is_none()
                && (6..=9).contains(&g.wdist(tile.pos, target))
        })
        .map(|tile| tile.pos)
        .min()
        .expect("room for a city of ours");
    let own = g.found_city_for(0, site, None);
    for tech in [
        "masonry",
        "bronze_working",
        "construction",
        "mining",
        "the_wheel",
    ] {
        g.players[0].techs.insert(crate::name::Name::new(tech));
    }
    g.players[0].gold = 2_000.0;
    let posts: Vec<Pos> = g
        .wring(target, 2)
        .into_iter()
        .filter(|p| {
            g.map
                .get(*p)
                .is_some_and(|t| g.rules.is_passable(t) && !g.rules.is_water(t))
        })
        .collect();
    assert!(posts.len() >= melee + ranged, "room around the walls");
    for (n, pos) in posts.iter().take(melee + ranged).enumerate() {
        let kind = if n < melee { "swordsman" } else { "archer" };
        g.spawn_test_unit(kind, 0, *pos);
    }
    let mut ai = AdvancedAi::new();
    ai.siege_needs_a_breaker = true;
    ai.enable_breaker_bought_for_the_siege();
    ai.siege_breaker_waits.insert(
        target,
        BreakerWait {
            since: g.turn,
            last: g.turn,
            nearest: i32::MAX,
            nearest_turn: g.turn,
        },
    );
    (g, ai, cid, own)
}

fn owned(g: &Game, kind: &str) -> usize {
    g.units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == kind)
        .count()
}

#[test]
fn a_held_siege_behind_ancient_walls_buys_a_ram() {
    let (mut g, mut ai, cid, own) = held_siege(4, 0);
    assert!(g.city_allows_siege_support(cid, "battering_ram"));
    let gold = g.players[0].gold;
    assert!(ai.siege_breaker_purchase(&mut g, 0));
    assert_eq!(owned(&g, "battering_ram"), 1, "one Ram bought");
    assert!(g.players[0].gold < gold);
    let ram = g
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "battering_ram")
        .unwrap();
    assert_eq!(
        ram.pos, g.cities[&own].pos,
        "bought in the city nearest the siege"
    );
    // One per city within the cooldown.
    assert!(!ai.siege_breaker_purchase(&mut g, 0));
    assert_eq!(owned(&g, "battering_ram"), 1);
}

#[test]
fn off_nothing_is_bought() {
    let (mut g, mut ai, _, _) = held_siege(4, 0);
    ai.disable_breaker_bought_for_the_siege();
    let gold = g.players[0].gold;
    assert!(!ai.siege_breaker_purchase(&mut g, 0));
    assert_eq!(owned(&g, "battering_ram"), 0);
    assert_eq!(g.players[0].gold, gold);
}

#[test]
fn walls_a_ram_cannot_open_buy_a_tower() {
    let (mut g, mut ai, cid, _) = held_siege(4, 0);
    // A pool of 200 stands behind Medieval Walls: a Ram cannot work on it, a
    // Tower can.
    Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, 200);
    g.cities.get_mut(&cid).unwrap().wall_hp = 200;
    g.observed_wall_tier_rules = true;
    g.players[0].techs.insert(crate::name!("machinery"));
    assert!(!g.city_allows_siege_support(cid, "battering_ram"));
    assert!(g.city_allows_siege_support(cid, "siege_tower"));
    assert!(ai.siege_breaker_purchase(&mut g, 0));
    assert_eq!(owned(&g, "battering_ram"), 0);
    assert_eq!(owned(&g, "siege_tower"), 1);
}

#[test]
fn shooters_alone_and_no_gun_on_sale_buy_nothing() {
    // No melee to swing beside a Ram or Tower, and no siege gun's tech.
    let (mut g, mut ai, _, _) = held_siege(0, 4);
    assert!(!ai.siege_breaker_purchase(&mut g, 0));
    assert_eq!(owned(&g, "battering_ram") + owned(&g, "siege_tower"), 0);
}

#[test]
fn no_army_at_the_walls_buys_nothing() {
    let (mut g, mut ai, _, _) = held_siege(0, 0);
    assert!(!ai.siege_breaker_purchase(&mut g, 0));
    assert_eq!(owned(&g, "battering_ram"), 0);
}

#[test]
fn a_stale_wait_buys_nothing() {
    let (mut g, mut ai, _, _) = held_siege(4, 0);
    g.turn += 5;
    assert!(!ai.siege_breaker_purchase(&mut g, 0));
    assert_eq!(owned(&g, "battering_ram"), 0);
}
