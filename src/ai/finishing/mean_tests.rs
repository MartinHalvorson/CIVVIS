//! `ranged-finish-on-the-mean`.
use super::*;
use crate::game::Game;
use std::collections::{BTreeMap, BTreeSet};

/// Grassland; our `kind` at (16, 10), a hostile Warrior two tiles off at
/// (18, 10); at war.
fn field(kind: &str, ours_at: (i32, i32)) -> (Game, u32, u32, BTreeMap<u32, i64>) {
    let mut g = Game::new_full(2, 40, 24, 440000, 500, 0, false);
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
    g.current = 0;
    g.at_war.insert((0, 1));
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    let ours = g.spawn_test_unit(kind, 0, ours_at);
    let target = g.spawn_test_unit("warrior", 1, (18, 10));
    let mapped: BTreeMap<u32, i64> = [(ours, 1)].into_iter().collect();
    (g, ours, target, mapped)
}

/// The first target health the lowest roll leaves alive while the centre
/// roll and the engine's own proof remove it, if any.
fn between_floor_and_mean(g: &Game, target: u32, mapped: &BTreeMap<u32, i64>) -> Option<i32> {
    (1..100).find(|&hp| {
        let mut game = g.clone();
        game.units.get_mut(&target).unwrap().hp = hp;
        let candidates = live_finishing_candidates(&game, 0, target, mapped, &BTreeSet::new());
        let Some(candidate) = candidates.first() else {
            return false;
        };
        let floor = live_finishing_damage_at(&game, 0, target, candidate, 0.8);
        let mean = live_finishing_damage_at(&game, 0, target, candidate, 1.0);
        candidate.kills
            && floor.is_some_and(|damage| damage < hp)
            && mean.is_some_and(|damage| damage >= hp)
    })
}

#[test]
fn the_gene_ships_off_and_is_registered() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "ranged-finish-on-the-mean",
        |ai| ai.ranged_finishes_on_the_mean(),
    );
}

/// An Archer whose lowest roll leaves the Warrior alive but whose centre
/// roll removes it: refused on the roll with the gene off, committed with it.
#[test]
fn a_lone_archer_commits_on_its_centre_roll() {
    let (g, _, target, mapped) = field("archer", (16, 10));
    let hp = between_floor_and_mean(&g, target, &mapped)
        .expect("the fixture must hold a health the floor misses and the centre roll takes");
    let mut base = g.clone();
    base.units.get_mut(&target).unwrap().hp = hp;

    let mut off = base.clone();
    let volley = finish_live_war_units_excluding(
        &mut off,
        0,
        &mapped,
        &BTreeSet::new(),
        &crate::ai::AdvancedAi::new(),
    );
    assert_eq!((volley.targets, volley.roll_rejections), (0, 1));

    let mut ai = crate::ai::AdvancedAi::new();
    ai.enable_ranged_finish_on_the_mean();
    let mut on = base.clone();
    let volley = finish_live_war_units_excluding(&mut on, 0, &mapped, &BTreeSet::new(), &ai);
    assert_eq!((volley.targets, volley.mean_finishes), (1, 1));
    assert_eq!(volley.roll_rejections, 0);
}

/// A melee line keeps the floor: its blow draws return damage.
#[test]
fn a_melee_line_keeps_the_floor() {
    let (g, _, target, mapped) = field("swordsman", (17, 10));
    let Some(hp) = between_floor_and_mean(&g, target, &mapped) else {
        return;
    };
    let mut ai = crate::ai::AdvancedAi::new();
    ai.enable_ranged_finish_on_the_mean();
    let mut on = g.clone();
    on.units.get_mut(&target).unwrap().hp = hp;
    let volley = finish_live_war_units_excluding(&mut on, 0, &mapped, &BTreeSet::new(), &ai);
    assert_eq!(volley.mean_finishes, 0);
    assert_eq!(volley.targets, 0);
}
