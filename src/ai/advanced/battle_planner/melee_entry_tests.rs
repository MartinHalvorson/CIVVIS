use super::*;

fn at(col: i32, row: i32) -> Pos {
    crate::hex::offset_to_axial(col, row)
}

fn field() -> Game {
    let mut g = crate::doctrine::build(
        crate::doctrine::position("the_reserve").expect("fixture"),
        3,
    )
    .expect("buildable");
    let units: Vec<_> = g.units.keys().copied().collect();
    for uid in units {
        g.remove_unit(uid);
    }
    for player in g.players.iter_mut() {
        player.civ = "Rome".to_string();
        player.government = None;
        player.policies.clear();
        player.techs.clear();
        player.civics.clear();
    }
    g.map.clear_rivers();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.road = 0;
        tile.owner_city = None;
    }
    g
}

fn closing_enemy(g: &mut Game) -> (u32, u32) {
    let ours = g.spawn_unit("warrior", 0, at(10, 6));
    let enemy = g.spawn_unit("warrior", 1, at(12, 6));
    assert_eq!(g.unit_max_moves(enemy), 2.0);
    assert_eq!(g.wdist(g.units[&enemy].pos, g.units[&ours].pos), 2);
    (ours, enemy)
}

fn walked(g: &Game, enemy: u32) -> Game {
    let mut after = g.speculative_clone();
    after.current = 1;
    after
        .apply(
            1,
            &Action::MoveTo {
                unit: enemy,
                to: at(11, 6),
            },
        )
        .expect("the closing stand is legal");
    assert_eq!(after.units[&enemy].pos, at(11, 6));
    assert_eq!(after.units[&enemy].moves_left, 1.0);
    after
}

#[test]
fn rough_tile_does_not_receive_a_melee_blow_the_executor_cannot_pay() {
    let mut g = field();
    let (ours, enemy) = closing_enemy(&mut g);
    g.map.tiles.get_mut(&at(10, 6)).unwrap().hills = true;
    let mut after = walked(&g, enemy);
    assert_eq!(after.step_cost_for(enemy, at(11, 6), at(10, 6)), 2.0);
    assert!(!after.can_pay_melee_entry(enemy, at(10, 6)));
    assert_eq!(
        after.apply(
            1,
            &Action::Attack {
                unit: enemy,
                target: at(10, 6)
            }
        ),
        Err("not enough movement to attack".to_string())
    );
    assert!(!strike_reach_of(&mut g, 0, enemy).contains(&at(10, 6)));
    assert_eq!(strike_danger(&g, 0, at(10, 6), ours), 0.0);
}

#[test]
fn river_entry_must_fit_after_the_approach() {
    let mut g = field();
    let (ours, enemy) = closing_enemy(&mut g);
    assert!(g.map.set_river_edge(at(11, 6), at(10, 6), true));
    let mut after = walked(&g, enemy);
    assert!(after.step_cost_for(enemy, at(11, 6), at(10, 6)) > 1.0);
    assert!(!after.can_pay_melee_entry(enemy, at(10, 6)));
    assert!(after
        .apply(
            1,
            &Action::Attack {
                unit: enemy,
                target: at(10, 6)
            }
        )
        .is_err());
    assert!(!strike_reach_of(&mut g, 0, enemy).contains(&at(10, 6)));
    assert_eq!(strike_danger(&g, 0, at(10, 6), ours), 0.0);
}

#[test]
fn a_flat_closing_blow_remains_a_real_threat() {
    let mut g = field();
    let (ours, enemy) = closing_enemy(&mut g);
    let mut after = walked(&g, enemy);
    assert!(after.can_pay_melee_entry(enemy, at(10, 6)));
    after
        .apply(
            1,
            &Action::Attack {
                unit: enemy,
                target: at(10, 6),
            },
        )
        .expect("the flat closing blow lands");
    assert!(strike_reach_of(&mut g, 0, enemy).contains(&at(10, 6)));
    assert!(strike_danger(&g, 0, at(10, 6), ours) > 0.0);
}

#[test]
fn a_full_movement_adjacent_blow_keeps_the_expensive_entry_exception() {
    let mut g = field();
    let ours = g.spawn_unit("warrior", 0, at(10, 6));
    let enemy = g.spawn_unit("warrior", 1, at(11, 6));
    let tile = g.map.tiles.get_mut(&at(10, 6)).unwrap();
    tile.hills = true;
    tile.feature = Some(crate::name!("forest"));
    assert!(g.step_cost_for(enemy, at(11, 6), at(10, 6)) > g.unit_max_moves(enemy));
    assert!(g.can_pay_melee_entry(enemy, at(10, 6)));
    assert!(strike_reach_of(&mut g, 0, enemy).contains(&at(10, 6)));
    assert!(strike_danger(&g, 0, at(10, 6), ours) > 0.0);
}

#[test]
fn ranged_reach_does_not_pay_melee_terrain_entry() {
    let mut g = field();
    let ours = g.spawn_unit("warrior", 0, at(10, 6));
    let enemy = g.spawn_unit("archer", 1, at(12, 6));
    g.map.tiles.get_mut(&at(10, 6)).unwrap().hills = true;
    assert!(strike_reach_of(&mut g, 0, enemy).contains(&at(10, 6)));
    assert!(strike_danger(&g, 0, at(10, 6), ours) > 0.0);
}

#[test]
fn a_reach_query_restores_unit_state_spatial_indexes_and_action_log() {
    let mut g = field();
    let (_, enemy) = closing_enemy(&mut g);
    g.arena_flags.insert(0, at(11, 6));
    let unit = g.units.get_mut(&enemy).unwrap();
    unit.moves_left = 0.5;
    unit.moved = true;
    unit.acted = true;
    unit.zoc_stopped = true;
    unit.started_turn_in_zoc = true;
    let before = serde_json::to_value(g.units.values().collect::<Vec<_>>()).unwrap();
    let game_before = serde_json::to_value(&g).unwrap();
    let positions: Vec<_> = g.map.tiles.keys().copied().collect();
    let index: Vec<_> = positions
        .iter()
        .map(|pos| g.unit_ids_at(*pos).to_vec())
        .collect();
    let log = g.log.len();
    let _ = strike_reach_of(&mut g, 0, enemy);
    assert!(
        serde_json::to_value(&g).unwrap() == game_before,
        "a hypothetical stand changed serialized game state"
    );
    assert_eq!(
        serde_json::to_value(g.units.values().collect::<Vec<_>>()).unwrap(),
        before
    );
    assert_eq!(g.log.len(), log);
    for (pos, ids) in positions.iter().zip(index) {
        assert_eq!(g.unit_ids_at(*pos), ids);
    }
}

#[test]
#[ignore = "read-only native fixture supplied by CIVVIS_BREACH_PREFIX"]
fn inspect_native_breach_approach_cost() {
    let path = std::env::var("CIVVIS_BREACH_PREFIX").expect("frozen native prefix");
    let path = std::path::Path::new(&path);
    let snapshot = crate::mirror::snapshot_from_events(path).unwrap();
    let state = crate::mirror::state_from_events(path, Some(190)).unwrap();
    assert_eq!(state.frame, 2);
    let live = crate::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 6);
    let mut g = live.game;
    let ours = live.uid_of[&9568265];
    let enemy = live.foreign_uid_of[&14483458];
    let target = at(30, 17);
    g.apply(
        0,
        &Action::MoveTo {
            unit: ours,
            to: target,
        },
    )
    .unwrap();
    let flood = g.attack_reach(enemy).contains(&target);
    let strike = strike_reach_of(&mut g, 0, enemy).contains(&target);
    let read = strike_danger(&g, 0, target, ours);
    eprintln!("NATIVE_BREACH_APPROACH flood={flood} strike={strike} danger={read}");
}

// Frozen pre-memo oracle from 5a11088. Keep its semantics unchanged: this
// comparison isolates memo lifetime, not the original affordability policy.
fn uncached_strike_reach(probe: &mut Game, pid: usize, uid: u32) -> Vec<Pos> {
    let Some(saved) = probe.units.get(&uid).cloned() else {
        return Vec::new();
    };
    let spec = &probe.rules.units[saved.kind];
    if spec.class != "military" || !(spec.is_melee_capable() || spec.has_ranged_attack()) {
        return Vec::new();
    }
    if spec.domain.as_deref() == Some("air") {
        return probe.attack_reach(uid);
    }
    let max_moves = probe.unit_max_moves(uid);
    if max_moves <= 0.0 {
        return Vec::new();
    }
    let melee = spec.is_melee_capable();
    let ranged = spec.has_ranged_attack();
    let siege = spec.siege;
    let sea = spec.domain.as_deref() == Some("sea");
    if let Some(live) = probe.units.get_mut(&uid) {
        live.moves_left = max_moves;
        live.moved = false;
        live.acted = false;
        live.zoc_stopped = false;
        live.started_turn_in_zoc = false;
    }
    let mut stands: Vec<(Pos, f64)> = vec![(saved.pos, max_moves)];
    stands.extend(
        probe
            .approach_reach(uid)
            .into_iter()
            .map(|(pos, (kept, _path))| (pos, kept)),
    );
    if let Some(live) = probe.units.get_mut(&uid) {
        *live = saved.clone();
    }
    let range = if ranged {
        probe.unit_attack_range(uid).max(1)
    } else {
        0
    };
    let after_move = probe.promotion_effect(&saved, "attack_after_move") > 0.0;
    let host_sight = mirrored_board(probe, pid);
    let mut targets: Vec<Pos> = Vec::new();
    for (from, kept) in stands {
        if kept <= 0.0 {
            continue;
        }
        // A land unit standing on water is embarked there and strikes nothing.
        let embarked = !sea
            && probe
                .map
                .get(from)
                .is_some_and(|tile| probe.rules.is_water(tile));
        if embarked {
            continue;
        }
        if melee {
            // A stand with movement left is not enough: entering the target
            // can still cost more than the approach left in hand. Ask the
            // executor's exact preflight from this stand, including cliffs
            // and its full-movement exception, rather than price a phantom
            // blow across a river or into rough terrain.
            for target in probe.nbrs(from) {
                if probe.map.tiles.contains_key(&target)
                    && probe.unit_can_melee_target_domain(uid, target)
                    && probe.can_pay_melee_entry_from(uid, from, kept, target)
                {
                    targets.push(target);
                }
            }
        }
        if ranged && (!siege || from == saved.pos || after_move) {
            for target in probe.wdisk(from, range) {
                if target != from
                    && probe.map.tiles.contains_key(&target)
                    && (host_sight || probe.unit_has_line_of_sight_from(uid, from, target))
                {
                    targets.push(target);
                }
            }
        }
    }
    targets.sort_unstable();
    targets.dedup();
    targets
}

fn fixed_reach_boards() -> Vec<(&'static str, Game, u32)> {
    let mut boards = Vec::new();
    for label in [
        "flat",
        "rough",
        "river",
        "formation-aura",
        "host-allowance",
        "war-cart",
    ] {
        let mut g = field();
        let (_, mut enemy) = closing_enemy(&mut g);
        match label {
            "rough" => {
                let tile = g.map.tiles.get_mut(&at(10, 6)).unwrap();
                tile.hills = true;
                tile.feature = Some(crate::name!("forest"));
            }
            "river" => {
                assert!(g.map.set_river_edge(at(11, 6), at(10, 6), true));
            }
            "formation-aura" => {
                let peer = g.spawn_unit("builder", 1, at(12, 6));
                g.units.get_mut(&enemy).unwrap().linked_to = Some(peer);
                g.units.get_mut(&peer).unwrap().linked_to = Some(enemy);
                g.spawn_unit("supply_convoy", 1, at(12, 7));
            }
            "host-allowance" => {
                Arc::make_mut(&mut g.host_unit_facts)
                    .entry(enemy)
                    .or_default()
                    .max_moves = Some(4.0);
            }
            "war-cart" => {
                g.remove_unit(enemy);
                enemy = g.spawn_unit("war_cart", 1, at(12, 6));
                let positions: Vec<_> = g.map.tiles.keys().copied().collect();
                for pos in positions {
                    g.map.tiles.get_mut(&pos).unwrap().hills = (pos.0 + pos.1).rem_euclid(3) == 0;
                }
            }
            _ => {}
        }
        boards.push((label, g, enemy));
    }
    let mut g = field();
    g.spawn_unit("warrior", 0, at(10, 6));
    let enemy = g.spawn_unit("warrior", 1, at(11, 6));
    let tile = g.map.tiles.get_mut(&at(10, 6)).unwrap();
    tile.hills = true;
    tile.feature = Some(crate::name!("forest"));
    boards.push(("adjacent-full", g, enemy));
    let mut g = field();
    g.spawn_unit("warrior", 0, at(10, 6));
    let enemy = g.spawn_unit("archer", 1, at(12, 6));
    g.map.tiles.get_mut(&at(10, 6)).unwrap().hills = true;
    boards.push(("ranged", g, enemy));
    let mut g = field();
    let positions: Vec<_> = g.map.tiles.keys().copied().collect();
    for pos in positions {
        if crate::hex::axial_to_offset(pos.0, pos.1).0 >= 11 {
            g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
        }
    }
    let enemy = g.spawn_unit("galley", 1, at(12, 6));
    boards.push(("sea", g, enemy));
    boards
}

fn verify_reach_pair(g: &mut Game, enemy: u32) -> Vec<Pos> {
    let before = serde_json::to_value(&*g).unwrap();
    let positions: Vec<_> = g.map.tiles.keys().copied().collect();
    let occupancy: Vec<_> = positions
        .iter()
        .map(|pos| g.unit_ids_at(*pos).to_vec())
        .collect();
    let reference = uncached_strike_reach(g, 0, enemy);
    assert!(
        serde_json::to_value(&*g).unwrap() == before,
        "oracle mutated the board"
    );
    let actual = strike_reach_of(g, 0, enemy);
    assert_eq!(actual, reference, "memo changed the reachable targets");
    assert!(
        serde_json::to_value(&*g).unwrap() == before,
        "memo query mutated the board"
    );
    for (pos, ids) in positions.iter().zip(occupancy) {
        assert_eq!(g.unit_ids_at(*pos), ids);
    }
    actual
}

#[test]
fn restored_target_memo_matches_uncached_reach_on_fixed_boards() {
    for (label, mut g, enemy) in fixed_reach_boards() {
        assert!(
            !verify_reach_pair(&mut g, enemy).is_empty(),
            "{label} must exercise a reach"
        );
    }
}

#[test]
#[ignore = "manual fixed-board paired timing, requires frozen native prefix and output path"]
fn measure_fixed_board_reach_memo() {
    let phase = std::env::var("CIVVIS_REACH_PHASE").expect("aa or memo phase");
    let compiled_memo = include_str!("../battle_planner.rs")
        .contains("let _restored_target_memo = probe.query_memo();");
    assert_eq!(compiled_memo, phase == "memo", "wrong compiled arm");
    assert!(matches!(phase.as_str(), "aa" | "memo"));
    let prefix = std::env::var("CIVVIS_BREACH_PREFIX").expect("frozen native prefix");
    let prefix = std::path::Path::new(&prefix);
    let snapshot = crate::mirror::snapshot_from_events(prefix).unwrap();
    let state = crate::mirror::state_from_events(prefix, Some(190)).unwrap();
    assert_eq!((state.turn, state.frame), (190, 2));
    let live = crate::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 650, 6);
    let ours = live.uid_of[&9568265];
    let enemy = live.foreign_uid_of[&14483458];
    let mut g = live.game;
    g.apply(
        0,
        &Action::MoveTo {
            unit: ours,
            to: at(30, 17),
        },
    )
    .unwrap();
    let mut boards = fixed_reach_boards();
    boards.push(("actual-native190f2-prefix", g, enemy));
    fn elapsed(g: &mut Game, enemy: u32, query: fn(&mut Game, usize, u32) -> Vec<Pos>) -> u128 {
        let start = std::time::Instant::now();
        for _ in 0..128 {
            std::hint::black_box(query(g, 0, enemy));
        }
        start.elapsed().as_nanos()
    }
    let mut rows = Vec::new();
    for (label, mut g, enemy) in boards {
        let targets = verify_reach_pair(&mut g, enemy);
        let before = serde_json::to_value(&g).unwrap();
        for _ in 0..4 {
            std::hint::black_box(uncached_strike_reach(&mut g, 0, enemy));
            std::hint::black_box(strike_reach_of(&mut g, 0, enemy));
        }
        let mut batches = Vec::new();
        for pair in 0..15 {
            let (reference_ns, actual_ns) = if pair % 2 == 0 {
                let reference = elapsed(&mut g, enemy, uncached_strike_reach);
                (reference, elapsed(&mut g, enemy, strike_reach_of))
            } else {
                let actual = elapsed(&mut g, enemy, strike_reach_of);
                (elapsed(&mut g, enemy, uncached_strike_reach), actual)
            };
            batches.push(
                serde_json::json!({"pair":pair,"reference_ns":reference_ns,"actual_ns":actual_ns}),
            );
        }
        assert!(
            serde_json::to_value(&g).unwrap() == before,
            "timed queries mutated {label}"
        );
        verify_reach_pair(&mut g, enemy);
        rows.push(serde_json::json!({"fixture":label,"enemy":enemy,"targets":targets,"calls_per_batch":128,"batches":batches}));
    }
    let output = std::env::var("CIVVIS_REACH_OUTPUT").expect("external result path");
    std::fs::write(
        output,
        serde_json::to_vec_pretty(&serde_json::json!({
            "phase":phase,"compiled_memo":compiled_memo,"profile":"ci","fixtures":rows
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "read-only host-combat witnesses supplied by external manifest"]
fn inspect_recorded_native_melee_permissions() {
    let manifest = std::env::var("CIVVIS_NATIVE_ENTRY_MANIFEST").unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(manifest).unwrap()).unwrap();
    let mut rows = Vec::new();
    for case in manifest["cases"].as_array().unwrap() {
        let path = std::path::Path::new(case["prefix"].as_str().unwrap());
        let snapshot = crate::mirror::snapshot_from_events(path).unwrap();
        let state = crate::mirror::state_from_events(path, None).unwrap();
        assert_eq!(u64::from(state.turn), case["turn"].as_u64().unwrap());
        assert_eq!(u64::from(state.frame), case["frame"].as_u64().unwrap());
        let live = crate::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 650, 6);
        let uid = live.uid_of[&case["native_unit"].as_i64().unwrap()];
        let g = live.game;
        let from = at(
            case["from"][0].as_i64().unwrap() as i32,
            case["from"][1].as_i64().unwrap() as i32,
        );
        let target = at(
            case["target"][0].as_i64().unwrap() as i32,
            case["target"][1].as_i64().unwrap() as i32,
        );
        assert_eq!(g.units[&uid].pos, from);
        assert_eq!(g.units[&uid].moves_left, case["moves"].as_f64().unwrap());
        assert_eq!(g.unit_max_moves(uid), case["max_moves"].as_f64().unwrap());
        let mut row = case.clone();
        row["model_step_cost"] = serde_json::json!(g.step_cost_for(uid, from, target));
        row["model_can_pay_entry"] = serde_json::json!(g.can_pay_melee_entry(uid, target));
        row["model_domain"] = serde_json::json!(g.rules.units[g.units[&uid].kind].domain);
        rows.push(row);
    }
    let output = std::env::var("CIVVIS_NATIVE_ENTRY_OUTPUT").unwrap();
    std::fs::write(output, serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
}

#[test]
#[ignore = "report-only exact lane failure trace supplied by external output path"]
fn inspect_exact_lane_dispatch_failure() {
    use crate::ai::{run_game, AdvancedAi, Ai};
    use crate::game::GameOptions;
    fn queues(g: &Game, pid: usize) -> serde_json::Value {
        serde_json::to_value(
            g.player_city_ids(pid)
                .into_iter()
                .map(|cid| {
                    let city = &g.cities[&cid];
                    serde_json::json!({"city":cid,"queue":city.queue,
                        "production":city.production,"progress":city.production_progress})
                })
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }
    fn players(g: &Game, gene: bool) -> Vec<AdvancedAi> {
        g.players
            .iter()
            .map(|_| {
                let mut ai = AdvancedAi::targeting(crate::ai::VictoryTarget::Science);
                if gene {
                    ai.enable_lane_delegates_production();
                }
                assert!(!ai.uses_player_observation());
                ai
            })
            .collect()
    }
    let mut arms = Vec::new();
    for gene in [false, true] {
        let mut silent = Game::new_with(GameOptions::new(3, 32, 20, 92_409, 40, 1));
        let mut ais = players(&silent, gene);
        run_game(&mut silent, &mut ais);
        let silent_log = serde_json::to_value(&silent.log).unwrap();
        let mut g = Game::new_with(GameOptions::new(3, 32, 20, 92_409, 40, 1));
        let mut ais = players(&g, gene);
        let journal = crate::reasoning::Journal::recording();
        journal.set_ceiling(crate::reasoning::Level::Strategy);
        for ai in &mut ais {
            ai.attach_journal(journal.handle());
        }
        g.set_fog_memory(false);
        g.set_war_ledger(false);
        let mut cursor = 0;
        let mut steps = Vec::new();
        while g.winner.is_none() && g.turn <= g.max_turns {
            let pid = g.current;
            let turn = g.turn;
            let before_book = ais[pid].base.book_pos;
            let specialization = ais[pid].phase_specialization_active(&g);
            let target = ais[pid].active_victory_target(&g);
            let delegates = ais[pid].lane_delegates_now(target, specialization);
            let war_before = ais[pid].war_plan.is_some();
            let queues_before = queues(&g, pid);
            ais[pid].take_turn(&mut g, pid);
            let delta = journal.since(cursor);
            cursor = delta.cursor;
            if pid < 3 {
                let plans: Vec<_> = delta.thoughts.iter()
                    .filter(|thought| thought.headline.starts_with("Grand strategy:"))
                    .map(|thought| serde_json::json!({"headline":thought.headline,"detail":thought.detail}))
                    .collect();
                steps.push(serde_json::json!({
                    "turn":turn,"pid":pid,"book_before":before_book,
                    "book_after":ais[pid].base.book_pos,"specialization":specialization,
                    "target":target,"delegates":delegates,
                    "war_before":war_before,"war_after":ais[pid].war_plan.is_some(),
                    "recovery_governor":ais[pid].governor_in_recovery,
                    "expansion_dispatch":ais[pid].expansion_dispatch,"plans":plans,
                    "queues_before":queues_before,"queues_after":queues(&g,pid),
                    "strategy_thoughts":delta.thoughts.iter().map(|thought|
                        serde_json::json!({"headline":thought.headline,"detail":thought.detail})
                    ).collect::<Vec<_>>(),
                    "truncated_turns":delta.truncated_turns
                }));
            }
            if g.winner.is_none() && g.current == pid {
                g.apply(pid, &Action::EndTurn).unwrap();
            }
        }
        g.finish_at_turn_limit();
        let log = serde_json::to_value(&g.log).unwrap();
        assert!(
            log == silent_log,
            "journal or diagnostic driver changed the action stream"
        );
        assert!(
            serde_json::to_value(&g).unwrap() == serde_json::to_value(&silent).unwrap(),
            "diagnostic driver changed the complete serialized game"
        );
        arms.push(serde_json::json!({"gene":gene,"steps":steps,"log":log}));
    }
    let output = std::env::var("CIVVIS_LANE_TRACE_OUTPUT").unwrap();
    std::fs::write(output, serde_json::to_vec_pretty(&arms).unwrap()).unwrap();
}
