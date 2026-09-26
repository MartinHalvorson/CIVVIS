use super::*;

fn corridor() -> (Game, u32, u32, u32, Pos, Pos) {
    let mut g =
        crate::doctrine::build(crate::doctrine::position("the_reserve").unwrap(), 3).unwrap();
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    let enemy_at = crate::hex::offset_to_axial(10, 6);
    let from = crate::hex::offset_to_axial(11, 6);
    let to = crate::hex::offset_to_axial(12, 6);
    for pos in g.map.tiles.keys().copied().collect::<Vec<_>>() {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = if [enemy_at, from, to].contains(&pos) {
            "grassland".into()
        } else {
            "mountain".into()
        };
        tile.hills = false;
        tile.feature = None;
    }
    let enemy = g.spawn_test_unit("warrior", 1, enemy_at);
    let gun = g.spawn_test_unit("bombard", 0, from);
    let tower = g.spawn_test_unit("siege_tower", 0, from);
    g.apply(
        0,
        &Action::LinkUnits {
            unit: gun,
            with: tower,
        },
    )
    .unwrap();
    g.units.get_mut(&gun).unwrap().hp = 45;
    (g, gun, tower, enemy, from, to)
}

#[test]
fn retreat_danger_accounts_for_the_corridor_a_support_pair_vacates() {
    let (g, gun, tower, enemy, from, to) = corridor();
    let mut moved = g.speculative_clone();
    moved.relocate(gun, to);
    moved.relocate(tower, to);
    assert!(strike_reach_of(&mut moved, 0, enemy).contains(&to));
    let mut moved_field = DangerField::with_reach(&moved, 0, true);
    let expected = moved_field.danger(to, gun);
    assert!(
        expected > NO_DANGER,
        "the opened corridor exposes the retreat"
    );
    let mut field = DangerField::with_reach(&g, 0, true);
    assert_eq!(
        field.danger(to, gun),
        expected,
        "the unit must not shield its own retreat by still occupying its old tile"
    );
    assert_eq!(field.probe.units[&gun].pos, from);
    assert_eq!(field.probe.units[&tower].pos, from);
    assert_eq!(g.units[&gun].pos, from);
    assert_eq!(g.units[&tower].pos, from);
}

#[test]
fn an_unlinked_unit_also_vacates_its_old_blocking_tile() {
    let (mut g, gun, tower, _, from, to) = corridor();
    g.apply(0, &Action::UnlinkUnits { unit: gun }).unwrap();
    g.remove_unit(tower);
    let mut field = DangerField::with_reach(&g, 0, true);
    assert!(field.danger(to, gun) > NO_DANGER);
    assert_eq!(field.probe.units[&gun].pos, from);
}

#[test]
fn moving_reach_cache_is_reused_without_reusing_the_wrong_hp_price() {
    let (g, gun, tower, _, from, to) = corridor();
    let mut field = DangerField::with_reach(&g, 0, true);
    let before = field.danger(from, gun);
    let normal = field.danger(to, gun);
    let cached = field.moved_reach.len();
    assert!(cached > 0);
    let wounded: f64 = field
        .contributions_at_hp(to, gun, 10)
        .iter()
        .map(|(_, damage)| damage)
        .sum();
    assert!(wounded > normal);
    assert_eq!(field.moved_reach.len(), cached);
    assert_eq!(field.danger(from, gun), before);
    assert_eq!(field.probe.units[&gun].hp, 45);
    assert_eq!(field.probe.units[&gun].pos, from);
    assert_eq!(field.probe.units[&tower].pos, from);
}

#[test]
fn an_initially_blocked_landing_can_threaten_a_vacated_shore() {
    let (mut g, gun, tower, enemy, from, to) = corridor();
    let start = g.units[&enemy].pos;
    g.map.tiles.get_mut(&start).unwrap().terrain = "coast".into();
    g.players[1].techs.insert("shipbuilding".into());
    std::sync::Arc::make_mut(&mut g.host_unit_facts).insert(
        enemy,
        crate::game::HostUnitFacts {
            max_moves: Some(6.0),
            ..Default::default()
        },
    );
    assert!(g.is_embarked(&g.units[&enemy]));
    assert!(
        strike_reach_of(&mut g, 0, enemy).is_empty(),
        "the only landing is occupied"
    );
    let mut field = DangerField::with_reach(&g, 0, true);
    assert!(
        field.danger(to, gun) > NO_DANGER,
        "a visible enemy with no initial strike must still be priced after the landing opens"
    );
    assert_eq!(field.probe.units[&gun].pos, from);
    assert_eq!(field.probe.units[&tower].pos, from);
}

#[test]
#[ignore = "local archived native-board diagnosis"]
fn inspect_native_doomed_bombard() {
    let path = std::env::var("CIVVIS_RECOVERY_SNAPSHOT").expect("snapshot path");
    let path = std::path::Path::new(&path);
    let snapshot = crate::mirror::snapshot_from_events_at(path, None).unwrap();
    let state = crate::mirror::state_from_events(path, None).unwrap();
    eprintln!("turn={} frame={}", state.turn, state.frame);
    let mut mirror = crate::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let uid = mirror.uid_of[&7208992];
    let enemy = mirror.foreign_uid_of.get(&3997696).copied();
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
        let mut relocated = g.speculative_clone();
        relocated.relocate(uid, tile);
        if let Some(peer) = g.units[&uid].linked_to {
            relocated.relocate(peer, tile);
        }
        let mut relocated_field = DangerField::with_reach(&relocated, 0, true);
        eprintln!(
            "tile={tile:?} danger={} after_vacating={} heal={}",
            field.danger(tile, uid),
            relocated_field.danger(tile, uid),
            g.unit_heal_rate_at(uid, tile)
        );
        if let Some(enemy) = enemy {
            let max_moves = relocated.unit_max_moves(enemy);
            let reach = strike_reach_of(&mut relocated, 0, enemy);
            eprintln!(
                "enemy={enemy} at={:?} moves={max_moves} embarked={} strike={:?}",
                relocated.units[&enemy].pos,
                relocated.is_embarked(&relocated.units[&enemy]),
                reach
            );
            let hostile = relocated.units.get_mut(&enemy).unwrap();
            hostile.moves_left = max_moves;
            hostile.moved = false;
            hostile.acted = false;
            hostile.zoc_stopped = false;
            hostile.started_turn_in_zoc = false;
            eprintln!("approach={:?}", relocated.approach_reach(enemy));
            std::sync::Arc::make_mut(&mut relocated.host_unit_facts)
                .get_mut(&enemy)
                .unwrap()
                .max_moves = Some(6.0);
            let reach = strike_reach_of(&mut relocated, 0, enemy);
            let mut full_field = DangerField::with_reach(&relocated, 0, true);
            eprintln!(
                "hypothetical_max6_reaches_target={} danger={}",
                reach.contains(&tile),
                full_field.danger(tile, uid)
            );
        }
    }
    eprintln!(
        "least={:?}",
        ai.least_danger_stand(g, uid, &mut field, here, true)
    );
    ai.rotate_wounded(g, 0, &mut field, &BTreeSet::new(), &BTreeSet::new());
    eprintln!(
        "after={:?} claimed={}",
        g.units[&uid].pos,
        ai.battle_planner_claims(uid)
    );
}
