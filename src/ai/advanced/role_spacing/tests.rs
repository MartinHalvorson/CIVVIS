use super::super::{AdvancedAi, ForceDomain, ForceGroup, ForcePosture};
use crate::game::Game;
use crate::name;

fn clear_approach() -> (Game, u32, ForceGroup) {
    let mut g = Game::new_full(2, 36, 22, 379140, 1_000, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    for player in g.players.iter_mut() {
        player.explored.extend(g.map.tiles.keys().copied());
    }
    g.at_war.clear();
    g.turn = 60;
    g.current = 0;
    let objective = (15, 8);
    let uid = g.spawn_test_unit("archer", 0, (9, 12));
    let group = ForceGroup {
        id: 1,
        domain: ForceDomain::Land,
        units: vec![uid],
        anchor: g.units[&uid].pos,
        objective,
        focus_target: None,
        posture: ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 1.0,
    };
    assert_eq!(g.wdist(g.units[&uid].pos, objective), 6);
    (g, uid, group)
}

#[test]
fn an_ordinary_advancing_archer_crosses_the_clear_six_to_five_approach() {
    let (mut g, uid, group) = clear_approach();
    let ai = AdvancedAi::new();
    let next = g.route_step(uid, group.objective, 2).unwrap();
    assert_eq!(ai.base.projected_counter_damage(&g, uid, next, &[]), 0.0);
    assert!(ai.coordinated_tactical_step(&mut g, 0, uid, &group, &[1], false));
    assert!(
        g.wdist(g.units[&uid].pos, group.objective) < 6,
        "an Advance order must actually close through this clear corridor"
    );
}
