use super::*;
use civvis::mirror::{LiveMirror, Plot, Snapshot, StateCity, StateSnapshot, TilesChunk};

/// An 8x8 patch of revealed grass on a 20x20 map with one city: the ring
/// beyond it is the unrevealed frontier the mirror assumes is open.
fn board(turn: u32) -> (Snapshot, StateSnapshot) {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 1,
        width: 20,
        height: 20,
        chunk: 1,
        plots: (0..8)
            .flat_map(|x| (0..8).map(move |y| (x, y)))
            .map(|(x, y)| {
                serde_json::from_value::<Plot>(serde_json::json!({
                    "x": x, "y": y, "t": "TERRAIN_GRASS"
                }))
                .expect("a plot with serde defaults deserializes")
            })
            .collect(),
    }]);
    let state = StateSnapshot {
        turn,
        cities: vec![StateCity {
            id: 7,
            name: "Athens".to_string(),
            x: 4,
            y: 4,
            pop: 4,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    (snapshot, state)
}

fn log(name: &str, lines: &[&str]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("civvis-closed-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("events.jsonl");
    std::fs::write(&path, lines.join("\n") + "\n").expect("write events");
    path
}

/// The event as the control mod writes it (`CivvisLedger.refuseWarStarter`)
/// and the relay re-encodes it.
fn refused(turn: u32, verb: &str, x: i32, y: i32) -> String {
    format!(
        r#"{{"ctx": "agent", "kind": "war_refused", "players": [5], "target_owner": 5, "turn": {turn}, "unit": 196608, "verb": "{verb}", "x": {x}, "y": {y}}}"#
    )
}

#[test]
fn a_move_refused_as_a_war_starter_closes_its_fog_plot_for_ten_turns() {
    // (8,5) is the first unrevealed column past the revealed patch.
    let path = log("closes", &[&refused(65, "MOVE_TO", 8, 5)]);
    let mut ledger = Ledger::default();
    ledger.refresh(&path);
    assert_eq!(
        ledger.refused[&(8, 5)].iter().copied().collect::<Vec<_>>(),
        vec![65]
    );

    let pos = civvis::hex::offset_to_axial(8, 5);
    let (snapshot, state) = board(66);
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 2);
    assert_eq!(mirror.game.map.tiles[&pos].terrain.as_str(), "unknown");
    assert!(
        mirror.game.map.tiles[&pos].assumed_traversable,
        "fixture precondition: the frontier plot starts out assumed open"
    );
    ledger.apply(&mut mirror, 66);
    assert!(!mirror.game.map.tiles[&pos].assumed_traversable);
    assert!(!mirror.game.map.tiles[&pos].assumed_navigable);
    assert!(
        !mirror.game.rules.is_passable(&mirror.game.map.tiles[&pos]),
        "the planner's paths route around it"
    );
    assert_eq!(ledger.closed_on(66), 1);

    // Still closed on the ninth turn after the refusal, open again on the tenth.
    assert_eq!(ledger.closed_on(74), 1);
    assert_eq!(ledger.closed_on(75), 0);
    let (snapshot, state) = board(75);
    let mut later = LiveMirror::new(&snapshot, &state, 4, 1, 250, 2);
    ledger.apply(&mut later, 75);
    assert!(
        later.game.map.tiles[&pos].assumed_traversable,
        "a border that may have opened is tried again"
    );
    let _ = std::fs::remove_dir_all(path.parent().expect("scratch dir"));
}

#[test]
fn revealed_ground_keeps_its_own_truth() {
    // (6,5) is revealed grass: its owner and the border rules answer for it.
    let path = log("revealed", &[&refused(65, "MOVE_TO", 6, 5)]);
    let mut ledger = Ledger::default();
    ledger.refresh(&path);
    let pos = civvis::hex::offset_to_axial(6, 5);
    let (snapshot, state) = board(66);
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 2);
    let before = mirror.game.map.tiles[&pos].assumed_traversable;
    ledger.apply(&mut mirror, 66);
    assert_eq!(mirror.game.map.tiles[&pos].terrain.as_str(), "grassland");
    assert_eq!(mirror.game.map.tiles[&pos].assumed_traversable, before);
    assert!(mirror.game.rules.is_passable(&mirror.game.map.tiles[&pos]));
    let _ = std::fs::remove_dir_all(path.parent().expect("scratch dir"));
}

#[test]
fn only_a_plain_move_refused_as_a_war_starter_is_read() {
    let path = log(
        "kinds",
        &[
            // A refused strike blocks the strike, not the ground under it.
            &refused(65, "ATTACK", 8, 5),
            &refused(65, "RANGE_ATTACK", 8, 6),
            // A declaration's refusal names a target and no plot.
            r#"{"kind": "war_refused", "turn": 65, "target": 5, "why": "MOVE_TO never appears here"}"#,
            // Another event that happens to carry the same words.
            r#"{"kind": "move_refused", "turn": 65, "unit": 9, "verb": "MOVE_TO", "x": 8, "y": 7, "note": "war_refused"}"#,
            "not json at all war_refused MOVE_TO",
            &refused(66, "MOVE_TO", 9, 4),
        ],
    );
    let mut ledger = Ledger::default();
    ledger.refresh(&path);
    assert_eq!(
        ledger.refused.keys().copied().collect::<Vec<_>>(),
        vec![(9, 4)],
        "only the plain move's plot is kept"
    );
    let _ = std::fs::remove_dir_all(path.parent().expect("scratch dir"));
}

#[test]
fn the_log_is_read_from_where_it_stopped_and_a_new_run_starts_over() {
    let path = log("grows", &[&refused(65, "MOVE_TO", 8, 5)]);
    let mut ledger = Ledger::default();
    ledger.refresh(&path);
    assert_eq!(ledger.refused.len(), 1);

    // The run appends: a second refusal of the same plot moves its turn on,
    // a half-written last line waits for its newline.
    let mut text = std::fs::read_to_string(&path).expect("read events");
    text.push_str(&refused(70, "MOVE_TO", 8, 5));
    text.push('\n');
    text.push_str(&refused(71, "MOVE_TO", 9, 9)[..40]);
    std::fs::write(&path, &text).expect("append events");
    ledger.refresh(&path);
    assert_eq!(
        ledger.refused[&(8, 5)].iter().copied().collect::<Vec<_>>(),
        vec![65, 70]
    );
    assert_eq!(ledger.refused.len(), 1, "a torn line is not read");
    assert_eq!(
        ledger.closed_on(79),
        1,
        "the later refusal sets the ten turns"
    );
    assert_eq!(ledger.closed_on(80), 0);

    // A replay reading a log that runs past its turn answers as the live
    // game did on that turn: the first refusal counts, the later one does not.
    assert_eq!(ledger.closed_on(64), 0);
    assert_eq!(ledger.closed_on(69), 1);

    // A shorter file is another run.
    std::fs::write(&path, refused(3, "MOVE_TO", 1, 1) + "\n").expect("new run");
    ledger.refresh(&path);
    assert_eq!(
        ledger.refused.keys().copied().collect::<Vec<_>>(),
        vec![(1, 1)]
    );
    let _ = std::fs::remove_dir_all(path.parent().expect("scratch dir"));
}
