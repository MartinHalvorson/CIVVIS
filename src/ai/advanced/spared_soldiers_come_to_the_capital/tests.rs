use super::*;
use crate::ai::advanced::anvil_spares_the_one_blow::{spare_the_one_blow, Spared};
use crate::ai::advanced::battle_planner::DangerField;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::doctrine::{build, position};

/// `the_storming`'s city of player 1 (our seat) with every unit removed.
fn bare_town() -> (Game, u32, Pos) {
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
    (g, cid, pos)
}

fn land(g: &Game, pos: Pos) -> bool {
    g.map
        .get(pos)
        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        && g.unit_ids_at(pos).is_empty()
}

#[test]
fn spared_soldiers_come_to_the_capital_ships_off_and_is_registered() {
    opt_in_off_in_both_controllers("spared-soldiers-come-to-the-capital", |ai| {
        ai.spared_soldiers_come_to_the_capital
    });
}

/// The limit reads only our own original capital, and only with the gene on.
#[test]
fn the_stand_off_limit_is_our_original_capitals_alone() {
    let (mut g, cid, _) = bare_town();
    {
        let city = g.cities.get_mut(&cid).unwrap();
        city.is_capital = true;
        city.original_owner = 1;
    }
    let mut ai = AdvancedAi::new();
    assert_eq!(
        ai.capital_stand_off(&g, 1, cid),
        None,
        "off, the rewrite is as it was"
    );
    ai.enable_spared_soldiers_come_to_the_capital();
    assert_eq!(ai.capital_stand_off(&g, 1, cid), Some(CAPITAL_STAND_OFF));
    assert_eq!(
        ai.capital_stand_off(&g, 0, cid),
        None,
        "not the reader's city"
    );
    g.cities.get_mut(&cid).unwrap().original_owner = 0;
    assert_eq!(
        ai.capital_stand_off(&g, 1, cid),
        None,
        "a capital we took is not ours"
    );
    {
        let city = g.cities.get_mut(&cid).unwrap();
        city.original_owner = 1;
        city.is_capital = false;
    }
    assert_eq!(
        ai.capital_stand_off(&g, 1, cid),
        None,
        "another city of ours"
    );
}

/// Live civvis-20261115T222126Z t112-t116: a soldier posted to the capital's
/// ring where one Cuirassier blow removes it was handed back its own tile six
/// out. With the limit it takes ground within two of the capital instead,
/// and the journal's record names it.
#[test]
fn a_soldier_standing_off_comes_in_to_the_capital() {
    let (mut g, _, city) = bare_town();
    let (front, musket_at, far) = g
        .wring(city, 1)
        .into_iter()
        .filter(|pos| land(&g, *pos))
        .find_map(|front| {
            let musket = g
                .nbrs(front)
                .into_iter()
                .find(|pos| g.wdist(*pos, city) == 2 && land(&g, *pos))?;
            let far = g
                .wdisk(city, 6)
                .into_iter()
                .filter(|pos| {
                    land(&g, *pos)
                        && g.wdist(*pos, city) > CAPITAL_STAND_OFF + 1
                        && g.wdist(*pos, musket) >= 5
                })
                .min()?;
            Some((front, musket, far))
        })
        .expect("a front tile, land beside it, and land well beyond both");
    let archer = g.spawn_test_unit("archer", 1, city);
    let warrior = g.spawn_test_unit("warrior", 1, far);
    g.units.get_mut(&warrior).unwrap().hp = 30;
    g.spawn_test_unit("musketman", 0, musket_at);
    let mut field = DangerField::new(&g, 1);
    assert!(
        field.upper_danger(front, warrior) >= 30.0,
        "the front removes the wounded Warrior"
    );
    assert!(
        field.upper_danger(far, warrior) < 30.0,
        "its own tile is out of reach"
    );
    let posts = BTreeMap::from([(archer, city), (warrior, front)]);
    let members = [archer, warrior];

    // Without the limit the soldier keeps its own tile, far out.
    let mut orders = posts.clone();
    let spared = spare_the_one_blow(&g, 1, city, &members, &mut orders, &mut field, None);
    assert_eq!(spared.moved, vec![(warrior, front, far)]);
    assert!(spared.called_in.is_empty());
    assert_eq!(orders[&warrior], far);

    // With it the soldier is ordered within two of the capital.
    let mut orders = posts.clone();
    let spared = spare_the_one_blow(
        &g,
        1,
        city,
        &members,
        &mut orders,
        &mut field,
        Some(CAPITAL_STAND_OFF),
    );
    let to = orders[&warrior];
    assert!(
        g.wdist(to, city) <= CAPITAL_STAND_OFF,
        "{to:?} lies within two of the capital"
    );
    assert_eq!(spared.called_in, vec![(warrior, far, to)]);
    assert_eq!(orders[&archer], city, "the centre's holder is untouched");
}

/// A soldier already within two of the capital is offered the same tiles
/// either way, so the limit changes nothing for it.
#[test]
fn a_soldier_within_two_is_left_as_it_was() {
    let (mut g, _, city) = bare_town();
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
    let posts = BTreeMap::from([(archer, city), (warrior, front)]);
    let mut off = posts.clone();
    let without = spare_the_one_blow(&g, 1, city, &[archer, warrior], &mut off, &mut field, None);
    let mut on = posts.clone();
    let with = spare_the_one_blow(
        &g,
        1,
        city,
        &[archer, warrior],
        &mut on,
        &mut field,
        Some(CAPITAL_STAND_OFF),
    );
    assert_eq!(off, on);
    assert_eq!(without, with);
    assert_eq!(
        with,
        Spared {
            filled: None,
            moved: vec![(warrior, front, safe)],
            called_in: vec![]
        }
    );
}
