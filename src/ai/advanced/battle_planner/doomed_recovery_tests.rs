use super::*;

#[test]
#[ignore = "local archived native-board diagnosis"]
fn inspect_native_doomed_bombard() {
    let path = std::env::var("CIVVIS_RECOVERY_SNAPSHOT").expect("snapshot path");
    let path = std::path::Path::new(&path);
    let snapshot = crate::mirror::snapshot_from_events_at(path, Some(160)).unwrap();
    let state = crate::mirror::state_from_events(path, Some(160)).unwrap();
    let mut mirror = crate::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let uid = mirror.uid_of[&7208992];
    let g = &mut mirror.game;
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner_2();
    ai.enable_siege_train();
    ai.enable_safest_stand();
    let mut field = DangerField::with_reach(g, 0, true);
    let unit = &g.units[&uid];
    let here = field.danger(unit.pos, uid);
    eprintln!("unit id={uid} hp={} pos={:?} moves={} linked={:?} linked_valid={} city={:?} encamp={:?} danger={here}", unit.hp, unit.pos, unit.moves_left, unit.linked_to,
        super::super::siege_train::linked_support_carrier(g, uid),
        g.city_at(unit.pos), g.encampment_at(unit.pos));
    let mut tiles = vec![unit.pos];
    tiles.extend(g.reachable(uid));
    for tile in tiles {
        eprintln!("tile={tile:?} danger={} heal={}", field.danger(tile, uid), g.unit_heal_rate_at(uid, tile));
    }
    eprintln!("least={:?}", ai.least_danger_stand(g, uid, &mut field, here, true));
    ai.rotate_wounded(g, 0, &mut field, &BTreeSet::new(), &BTreeSet::new());
    eprintln!("after={:?} claimed={}", g.units[&uid].pos, ai.battle_planner_claims(uid));
}
