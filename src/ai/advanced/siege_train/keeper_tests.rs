//! `siege-keeps-a-shooter`: see `siege_wall_keepers`.

use super::tests::{at_distance, walled_city};
use super::*;
use crate::ai::advanced::battle_planner::DangerField;

/// `walled_city` on open grassland, at war, healing on, its walls at `walls`
/// and a siege of it in Reduce.
fn breached(walls: i32) -> (Game, u32, AdvancedAi) {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    for tile in g.map.tiles.values_mut() {
        if tile.pos != city {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
    }
    g.at_war.insert((0, 1));
    g.tactics.heal = true;
    g.cities.get_mut(&cid).unwrap().wall_hp = walls;
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.sieges.insert(
        cid,
        Siege {
            stage: SiegeStage::Reduce,
            taker: None,
            entered: 25,
            assessed: 29,
            posts: BTreeMap::new(),
            short_since: None,
        },
    );
    (g, cid, ai)
}

/// Two shooters of `kind` two tiles out, at `hps`.
fn shooters(g: &mut Game, cid: u32, kind: &str, hps: [i32; 2]) -> [u32; 2] {
    let tiles = at_distance(g, cid, 2);
    let first = g.spawn_unit(kind, 0, tiles[0]);
    let second = g.spawn_unit(
        kind,
        0,
        *tiles
            .iter()
            .find(|pos| g.wdist(**pos, tiles[0]) >= 2)
            .unwrap(),
    );
    g.units.get_mut(&first).unwrap().hp = hps[0];
    g.units.get_mut(&second).unwrap().hp = hps[1];
    [first, second]
}

#[test]
fn the_healthiest_wounded_shooter_keeps_firing_on_a_breached_wall() {
    for gene in [false, true] {
        let (mut g, cid, mut ai) = breached(40);
        let [weaker, stronger] = shooters(&mut g, cid, "crossbowman", [45, 48]);
        if gene {
            ai.enable_siege_keeps_a_shooter();
        }
        let start = (g.units[&weaker].pos, g.units[&stronger].pos);
        let mut field = DangerField::with_reach(&g, 0, true);
        ai.rotate_wounded(&mut g, 0, &mut field, &BTreeSet::new(), &BTreeSet::new());
        if gene {
            assert_eq!(ai.siege_wall_keepers, BTreeSet::from([stronger]));
            assert_eq!(g.units[&stronger].pos, start.1, "the keeper holds its post");
            assert!(!ai.battle_planner_recovering.contains(&stronger));
            assert!(
                ai.battle_planner_recovering.contains(&weaker),
                "the other heals"
            );
        } else {
            assert!(ai.siege_wall_keepers.is_empty());
            assert!(ai.battle_planner_recovering.contains(&stronger));
            assert!(ai.battle_planner_recovering.contains(&weaker));
        }
    }
}

#[test]
fn no_keeper_over_a_whole_wall_or_beside_a_healthy_shooter() {
    // Walls above the share: nothing to keep down.
    let (mut g, cid, mut ai) = breached(100);
    shooters(&mut g, cid, "crossbowman", [45, 48]);
    ai.enable_siege_keeps_a_shooter();
    let mut field = DangerField::with_reach(&g, 0, true);
    assert!(ai.siege_wall_keepers(&g, 0, &mut field).is_empty());
    // A healthy shooter in range already fires.
    let (mut g, cid, mut ai) = breached(40);
    shooters(&mut g, cid, "crossbowman", [45, 100]);
    ai.enable_siege_keeps_a_shooter();
    let mut field = DangerField::with_reach(&g, 0, true);
    assert!(ai.siege_wall_keepers(&g, 0, &mut field).is_empty());
}

#[test]
fn a_shooter_the_strike_would_kill_is_not_kept() {
    let (mut g, cid, mut ai) = breached(40);
    let [first, second] = shooters(&mut g, cid, "archer", [12, 11]);
    ai.enable_siege_keeps_a_shooter();
    let mut field = DangerField::with_reach(&g, 0, true);
    let danger = field.rotation_danger(g.units[&first].pos, first);
    assert!(
        12.0 - danger < KEEPER_SURVIVE_MARGIN,
        "fixture: the city's strike ({danger:.0}) would finish it"
    );
    assert!(ai.siege_wall_keepers(&g, 0, &mut field).is_empty());
    let _ = second;
}
