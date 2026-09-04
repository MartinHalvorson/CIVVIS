use super::*;

#[test]
fn recent_raw_attack_reach_snapshots_reuse_and_stay_bounded() {
    let game = Game::new_full(2, 24, 16, 731_155, 200, 0, false);
    let unit = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| {
            let spec = &game.rules.units[game.units[uid].kind];
            spec.class == "military" && (spec.is_melee_capable() || spec.has_ranged_attack())
        })
        .expect("a new game starts each major with a military unit");
    let keys: Vec<(u32, u64)> = (0..=ATTACK_REACH_SNAPSHOT_CAPACITY)
        .map(|index| (game.turn, index as u64 + 1))
        .collect();

    let first = game.cached_attack_reach_from_flood(keys[0], unit);
    for key in keys.iter().take(ATTACK_REACH_SNAPSHOT_CAPACITY).skip(1) {
        let _ = game.cached_attack_reach_from_flood(*key, unit);
    }
    assert_eq!(
        game.attack_reach_cache_computations(),
        ATTACK_REACH_SNAPSHOT_CAPACITY as u64,
        "one raw flood per distinct snapshot before any reuse"
    );

    let reused = game.cached_attack_reach_from_flood(keys[0], unit);
    assert!(
        Arc::ptr_eq(&first, &reused),
        "returning to a recently visited exact board retains its raw reach"
    );
    assert_eq!(
        game.attack_reach_cache_computations(),
        ATTACK_REACH_SNAPSHOT_CAPACITY as u64,
        "the recent-board hit must not rebuild the flood"
    );

    let _ = game.cached_attack_reach_from_flood(keys[ATTACK_REACH_SNAPSHOT_CAPACITY], unit);
    {
        let cache = game.attack_reach_cache.lock().unwrap();
        assert_eq!(
            cache.snapshots.len(),
            ATTACK_REACH_SNAPSHOT_CAPACITY,
            "a long speculative search cannot grow the raw cache beyond its fixed bound"
        );
    }
    let _ = game.cached_attack_reach_from_flood(keys[1], unit);
    assert_eq!(
        game.attack_reach_cache_computations(),
        ATTACK_REACH_SNAPSHOT_CAPACITY as u64 + 2,
        "the least-recently-used snapshot is evicted while the reused parent remains resident"
    );
    let still_recent = game.cached_attack_reach_from_flood(keys[0], unit);
    assert!(
        Arc::ptr_eq(&first, &still_recent),
        "the parent snapshot was promoted, rather than evicted with the old least-recent entry"
    );
}
