use super::*;
use crate::ai::advanced::battle_planner::STRUCTURE_SOURCE;
use crate::ai::advanced::VictoryTarget;

/// A grassland field: our city at (6, 10), a walled Babylonian city at
/// (20, 10), at war, and our trebuchet at (16, 10) at `hp` health. The city
/// strikes two tiles out, so (18, 10) is in its reach and (17, 10) is not.
fn fixture(hp: i32) -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(2, 40, 24, 377300, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (6, 10), None);
    let cid = g.found_city_for(1, (20, 10), None);
    g.cities.get_mut(&cid).unwrap().wall_hp = 100;
    g.current = 0;
    g.at_war.insert((0, 1));
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    let gun = g.spawn_test_unit("trebuchet", 0, (16, 10));
    g.units.get_mut(&gun).unwrap().hp = hp;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.base.recorded_tactical_step = true;
    (g, ai, gun, cid)
}

/// Off: nothing is read or marked, and the step into the city's strike is
/// taken as before, with or without `guns-stay-out-of-reach`.
#[test]
fn off_the_gene_is_an_exact_no_op() {
    let (mut g, mut ai, gun, _) = fixture(10);
    assert!(ai.wounded_guns(&g, 0).is_empty());
    ai.mark_gun_lethal_tiles(&g, 0);
    assert!(ai.base.gun_lethal_tiles.is_empty());
    assert!(ai.base.path_move(&mut g, 0, gun, (17, 10)));
    assert!(ai.base.path_move(&mut g, 0, gun, (18, 10)));

    let (mut g, mut ai, gun, _) = fixture(10);
    ai.enable_guns_stay_out_of_reach();
    ai.mark_gun_lethal_tiles(&g, 0);
    assert!(
        ai.base.gun_lethal_tiles.is_empty(),
        "{:?}",
        ai.base.gun_lethal_tiles
    );
    ai.guns_step_out_of_reach(&mut g, 0);
    assert_eq!(g.units[&gun].pos, (16, 10));
    assert!(ai.base.path_walk_to(&mut g, 0, gun, (18, 10)));
}

/// On: a wounded gun does not step into a walled city's strike that takes
/// its health; the step short of it is still taken.
#[test]
fn a_wounded_gun_does_not_step_into_a_strike_that_takes_its_health() {
    let (mut g, mut ai, gun, _) = fixture(10);
    ai.enable_wounded_gun_counts_every_blow();
    ai.mark_gun_lethal_tiles(&g, 0);
    let marks = &ai.base.gun_lethal_tiles[&gun];
    assert!(marks.contains_key(&(18, 10)), "{marks:?}");
    assert!(!marks.contains_key(&(17, 10)), "{marks:?}");
    assert!(!marks.contains_key(&(16, 10)), "{marks:?}");
    assert!(
        ai.base.path_move(&mut g, 0, gun, (17, 10)),
        "the safe step is taken"
    );
    assert!(
        !ai.base.path_move(&mut g, 0, gun, (18, 10)),
        "the step into the strike is refused"
    );
    assert_eq!(g.units[&gun].pos, (17, 10));
}

/// A whole gun is not read: fresh guns still enter a city's strike ring
/// together (`guns-enter-together`).
#[test]
fn a_whole_gun_is_left_alone() {
    let (mut g, mut ai, gun, _) = fixture(FULL_HEALTH);
    ai.enable_wounded_gun_counts_every_blow();
    assert!(ai.wounded_guns(&g, 0).is_empty());
    ai.mark_gun_lethal_tiles(&g, 0);
    assert!(ai.base.gun_lethal_tiles.is_empty());
    assert!(ai.base.path_walk_to(&mut g, 0, gun, (18, 10)));
}

/// A wounded gun standing in the strike steps off it; off, it stays.
#[test]
fn a_wounded_gun_inside_the_strike_steps_out() {
    for on in [false, true] {
        let (mut g, mut ai, gun, cid) = fixture(10);
        let city = g.cities[&cid].pos;
        assert!(ai.base.path_walk_to(&mut g, 0, gun, (18, 10)));
        g.units.get_mut(&gun).unwrap().moves_left = 2.0;
        if on {
            ai.enable_wounded_gun_counts_every_blow();
        }
        ai.mark_gun_lethal_tiles(&g, 0);
        ai.guns_step_out_of_reach(&mut g, 0);
        let pos = g.units[&gun].pos;
        if on {
            assert!(g.wdist(pos, city) > 2, "it stands at {pos:?}");
        } else {
            assert_eq!(pos, (18, 10));
        }
    }
}

/// The case the live guns died to: a city strike and a hostile's blow, each
/// short of the gun's health, together meet it. One-blow
/// `guns-stay-out-of-reach` leaves the tile open; this gene marks it.
#[test]
fn two_blows_short_of_the_health_alone_are_counted_together() {
    const TILE: Pos = (18, 10);
    // A board with a visible hostile Crossbowman at `at`, a spotter of ours
    // beside it, and the gun at `hp`.
    let board = |hp: i32, at: Pos| {
        let (mut g, ai, gun, cid) = fixture(hp);
        let city = g.cities[&cid].pos;
        let crossbow = g.spawn_test_unit("crossbowman", 1, at);
        let spot = g
            .wdisk(at, 1)
            .into_iter()
            .find(|pos| *pos != at && *pos != city && *pos != TILE && g.units_at(*pos).is_empty())
            .expect("a free tile beside the hostile");
        g.spawn_test_unit("warrior", 0, spot);
        assert!(g.unit_visible_to(crossbow, 0));
        (g, ai, gun)
    };
    // The structure's and the unit's blows on the gun standing on TILE.
    let blows = |g: &Game, ai: &AdvancedAi, gun: u32| {
        let mut field = DangerField::with_reach(g, 0, ai.strike_reach);
        let (mut structure, mut unit) = (0.0_f64, 0.0_f64);
        for (source, blow) in field.contributions(TILE, gun).iter() {
            match source {
                Some(id) if id & STRUCTURE_SOURCE != 0 => structure += blow,
                Some(_) => unit = unit.max(*blow),
                None => {}
            }
        }
        (structure, unit)
    };
    let (g, _, _) = fixture(50);
    let at = g
        .wdisk(TILE, 2)
        .into_iter()
        .filter(|pos| g.wdist(*pos, TILE) >= 1 && *pos != (20, 10) && *pos != (16, 10))
        .find(|at| {
            let (g, ai, gun) = board(50, *at);
            let (structure, unit) = blows(&g, &ai, gun);
            structure > 0.0 && unit > 0.0
        })
        .expect("a tile from which a Crossbowman's blow reaches the city's strike ring");
    let mut seen = Vec::new();
    let found = (1..FULL_HEALTH).rev().find(|&hp| {
        let (g, mut ai, gun) = board(hp, at);
        let (structure, unit) = blows(&g, &ai, gun);
        seen.push((hp, structure, unit));
        // Each blow alone, at the top of its roll, leaves the gun standing;
        // the two together do not.
        let top = crate::ai::COMBAT_ROLL_MAX;
        if structure * top >= f64::from(hp)
            || unit * top >= f64::from(hp)
            || (structure + unit) * top < f64::from(hp)
        {
            return false;
        }
        ai.enable_guns_stay_out_of_reach();
        ai.mark_gun_lethal_tiles(&g, 0);
        let one_blow = ai
            .base
            .gun_lethal_tiles
            .get(&gun)
            .is_some_and(|marks| marks.contains_key(&TILE));
        let (g, mut ai, gun) = board(hp, at);
        ai.enable_guns_stay_out_of_reach();
        ai.enable_wounded_gun_counts_every_blow();
        ai.mark_gun_lethal_tiles(&g, 0);
        let summed = ai
            .base
            .gun_lethal_tiles
            .get(&gun)
            .is_some_and(|marks| marks.contains_key(&TILE));
        !one_blow && summed
    });
    assert!(
        found.is_some(),
        "some health is safe from each blow and not from both: {seen:?}"
    );
}
