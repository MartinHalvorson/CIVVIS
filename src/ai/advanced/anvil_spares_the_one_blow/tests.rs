use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::doctrine::{build, position};

/// `the_storming`'s city of player 1 (our seat) with every unit removed.
fn bare_town() -> (Game, Pos) {
    let mut g = build(position("the_storming").expect("known"), 3).expect("buildable");
    let seeded: Vec<u32> = (0..2).flat_map(|pid| g.player_unit_ids(pid)).collect();
    for uid in seeded {
        g.remove_unit(uid);
    }
    let cid = *g.cities.keys().next().expect("the position states a city");
    assert_eq!(g.cities[&cid].owner, 1);
    assert!(g.is_at_war(1, 0), "the storming is a war");
    g.current = 1;
    let pos = g.cities[&cid].pos;
    (g, pos)
}

fn land(g: &Game, pos: Pos) -> bool {
    g.map
        .get(pos)
        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        && g.unit_ids_at(pos).is_empty()
}

#[test]
fn anvil_spares_the_one_blow_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.anvil_spares_the_one_blow, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "anvil_spares_the_one_blow"));
    opt_in_off_in_both_controllers("anvil-spares-the-one-blow", |ai| {
        ai.anvil_spares_the_one_blow
    });
}

/// Live 033244Z t151: the force's only soldier, posted to the ring with
/// nobody in the city, keeps the City Center instead.
#[test]
fn the_city_tile_is_never_left_to_nobody() {
    let (mut g, city) = bare_town();
    let ring = g
        .wring(city, 1)
        .into_iter()
        .find(|pos| land(&g, *pos))
        .expect("a land ring tile");
    let warrior = g.spawn_test_unit("warrior", 1, ring);
    let mut orders = BTreeMap::from([(warrior, ring)]);
    let mut field = DangerField::new(&g, 1);
    let spared = spare_the_one_blow(&g, 1, city, &[warrior], &mut orders, &mut field);
    assert_eq!(spared.filled, Some(warrior));
    assert_eq!(orders[&warrior], city);
    // A soldier of ours outside the force already holds it: nothing changes.
    let mut orders = BTreeMap::from([(warrior, ring)]);
    g.spawn_test_unit("warrior", 1, city);
    let mut field = DangerField::new(&g, 1);
    let spared = spare_the_one_blow(&g, 1, city, &[warrior], &mut orders, &mut field);
    assert_eq!(spared, Spared::default());
    assert_eq!(orders[&warrior], ring);
}

/// A front post a Musketman's blow removes a wounded Warrior on is traded
/// for the Warrior's own tile out of its reach; the City Center's holder is
/// untouched. The Musketman stands beside the post (it strikes from its own
/// tile, whatever the ground), and the Warrior's tile is four from it.
#[test]
fn a_post_one_blow_removes_its_soldier_on_is_moved() {
    let (mut g, city) = bare_town();
    let (front, musket_at, safe) = g
        .wring(city, 1)
        .into_iter()
        .filter(|pos| land(&g, *pos))
        .find_map(|front| {
            let musket = g
                .nbrs(front)
                .into_iter()
                .find(|pos| g.wdist(*pos, city) == 2 && land(&g, *pos))?;
            let safe = g
                .wring(city, 2)
                .into_iter()
                .filter(|pos| land(&g, *pos) && g.wdist(*pos, musket) >= 4)
                .min()?;
            Some((front, musket, safe))
        })
        .expect("a front tile, land beside it, and land four beyond");
    let archer = g.spawn_test_unit("archer", 1, city);
    let warrior = g.spawn_test_unit("warrior", 1, safe);
    g.units.get_mut(&warrior).unwrap().hp = 30;
    g.spawn_test_unit("musketman", 0, musket_at);
    let mut field = DangerField::new(&g, 1);
    assert!(
        field.upper_danger(front, warrior) >= 30.0,
        "the front removes the wounded Warrior"
    );
    assert!(
        field.upper_danger(safe, warrior) < 30.0,
        "its own tile is out of reach"
    );
    let mut orders = BTreeMap::from([(archer, city), (warrior, front)]);
    let spared = spare_the_one_blow(&g, 1, city, &[archer, warrior], &mut orders, &mut field);
    assert_eq!(spared.filled, None, "the Archer holds the city");
    assert_eq!(spared.moved, vec![(warrior, front, safe)]);
    assert_eq!(orders[&archer], city);
    assert_eq!(orders[&warrior], safe);
}
