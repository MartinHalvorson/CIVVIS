use super::*;
use serde_json::json;

fn controlled_game(seed: u64) -> (Game, Pos) {
    let mut game = Game::new_full(2, 20, 14, seed, 40, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    for player in game.players.iter_mut() {
        player.explored.clear();
        player.remembered_tiles.forget_all();
        player.remembered_cities.clear();
    }
    game.map.clear_rivers();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.owner_city = None;
        tile.hills = false;
        tile.road = 0;
    }
    let center = *game
        .map
        .tiles
        .keys()
        .find(|position| game.wdisk(**position, 4).len() == 61)
        .expect("controlled map has an interior tile");
    game.current = 0;
    (game, center)
}

fn along(game: &Game, origin: Pos, distance: i32) -> Pos {
    hex::canon((origin.0 + distance, origin.1), game.map.width)
}

/// A visibility ray on a cylindrical world already picks the same shortest
/// path as `Game::wdist`; reuse that winning comparison instead of measuring
/// all three wrapped images a second time. This exhausts one small world so a
/// seam tie cannot quietly make the fast adjacent-ray path differ.
#[test]
fn unwrapped_cylinder_ray_distance_matches_world_distance() {
    let (game, _) = controlled_game(63_097);
    assert!(game.map.sphere().is_none());
    assert!(game.map.topology.wraps_east_west());
    let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
    for from in &positions {
        for to in &positions {
            let (_, unwrapped_distance) = game.unwrapped_toward(*from, *to);
            assert_eq!(
                unwrapped_distance,
                game.wdist(*from, *to),
                "the cylindrical seam must choose the same distance from {from:?} to {to:?}"
            );
        }
    }
}

#[test]
fn vision_frames_reuse_static_inputs_and_invalidate_on_sight_changes() {
    let (mut game, center) = controlled_game(63_099);
    let scout = game.spawn_unit("scout", 0, center);

    let first = game.vision_frame(0, &mut game.height_field());
    let first_stamp = game.vision_input_stamp(0);
    let again = game.vision_frame(0, &mut game.height_field());
    assert!(
        Arc::ptr_eq(&first, &again),
        "a static frame should be reused"
    );
    assert_eq!(
        game.vision_frames
            .unit_stamps
            .borrow()
            .as_ref()
            .map(|(epoch, _)| *epoch),
        Some(game.units.vision_epoch()),
        "the cached unit fan-out is keyed to the roster that produced it"
    );

    // A speculative branch inherits the same immutable frame. The stamp is
    // still the authority: moving a sight source in that branch must replace
    // the inherited Arc without disturbing the source world's cache.
    let source_unit_cache = Arc::clone(&game.vision.borrow().entries);
    let inherited_worker_cache = game.speculative_clone().vision.into_inner();
    let stamp = game.world_stamp();
    game.vision
        .borrow_mut()
        .merge_current(inherited_worker_cache, stamp);
    assert!(
        Arc::ptr_eq(&source_unit_cache, &game.vision.borrow().entries),
        "merging a worker with no ray misses must not fork the source table"
    );
    let mut branch = game.speculative_clone();
    assert!(
        Arc::ptr_eq(&source_unit_cache, &branch.vision.borrow().entries),
        "an unchanged branch should share the validated per-unit ray table"
    );
    let inherited = branch.vision_frame(0, &mut branch.height_field());
    assert!(
        Arc::ptr_eq(&first, &inherited),
        "an unchanged branch should inherit the populated frame"
    );
    branch.relocate(scout, along(&branch, center, 1));
    let branch_moved = branch.vision_frame(0, &mut branch.height_field());
    assert!(
        !Arc::ptr_eq(&first, &branch_moved),
        "a changed branch must reject the inherited frame"
    );
    assert!(
        !Arc::ptr_eq(&source_unit_cache, &branch.vision.borrow().entries),
        "the moved source must fork the ray table before writing its replacement"
    );
    assert!(
        Arc::ptr_eq(&source_unit_cache, &game.vision.borrow().entries),
        "a branch cache write must leave the source world's rays untouched"
    );
    let branch_uncached = branch.player_vision(&mut branch.height_field(), 0);
    assert!(
        branch_moved.as_ref() == &branch_uncached,
        "a changed branch must recompute the exact uncached frame"
    );
    assert!(
        Arc::ptr_eq(&first, &game.vision_frame(0, &mut game.height_field())),
        "branch recomputation must not replace the source world's frame"
    );

    // A turn advances remembered-tile timestamps, not the sight ray.
    game.turn += 1;
    let next_turn = game.vision_frame(0, &mut game.height_field());
    assert!(
        Arc::ptr_eq(&first, &next_turn),
        "turn-only changes should keep the compact sight frame"
    );
    assert_eq!(game.vision_input_stamp(0), first_stamp);

    // Combat state is not a sight input, so changing HP does not evict the
    // frame.  This is the high-frequency action that the input stamp must
    // deliberately ignore.
    game.units.get_mut(&scout).unwrap().hp -= 1;
    let damaged_epoch = game.units.vision_epoch();
    let damaged = game.vision_frame(0, &mut game.height_field());
    assert!(
        Arc::ptr_eq(&first, &damaged),
        "non-vision unit state should not rebuild sight"
    );
    assert_eq!(
        game.vision_frames
            .unit_stamps
            .borrow()
            .as_ref()
            .map(|(epoch, _)| *epoch),
        Some(damaged_epoch),
        "a mutable unit access refreshes the fan-out before the next frame lookup"
    );

    // Tile writes that do not participate in a sight ray still advance
    // the map's general mutation epoch.  The geometry fold must filter
    // those writes so an improvement/road update does not evict every
    // seat's frame.
    let road_tile = along(&game, center, 3);
    game.map.tiles.get_mut(&road_tile).unwrap().road = 1;
    let road_changed = game.vision_frame(0, &mut game.height_field());
    assert!(
        Arc::ptr_eq(&first, &road_changed),
        "unrelated tile writes should preserve the sight frame"
    );

    // Moving the source changes the compact signature and must produce a
    // new frame with exactly the same answer as an uncached derivation.
    let moved_to = along(&game, center, 1);
    game.relocate(scout, moved_to);
    let moved = game.vision_frame(0, &mut game.height_field());
    assert!(!Arc::ptr_eq(&first, &moved));
    let mut heights = game.height_field();
    let uncached = game.player_vision(&mut heights, 0);
    assert!(moved.as_ref() == &uncached, "cached and fresh sight differ");

    // Geometry changes invalidate every source frame even when no unit
    // moved; the epoch is the map's single mutation boundary.
    let tile = along(&game, center, 2);
    game.map.tiles.get_mut(&tile).unwrap().hills = true;
    let changed_map = game.vision_frame(0, &mut game.height_field());
    assert!(!Arc::ptr_eq(&moved, &changed_map));
}

/// The final per-seat signature has a faster memo ahead of it, but that memo
/// must preserve the old direct handling for the two inputs that do not share
/// a mutation epoch: raw spies and host-provided mirrored sight.
#[test]
fn vision_input_stamp_memo_rekeys_spies_and_bypasses_host_observations() {
    let (mut game, center) = controlled_game(63_106);
    let city = game.place_city(0, along(&game, center, 2), None);
    game.spawn_unit("scout", 0, center);

    let first_frame = game.player_vision_frame(0);
    let first_stamp = game.vision_input_stamp(0);
    let before_spy = game
        .vision_frames
        .input_stamps
        .borrow()
        .clone()
        .expect("the first frame lookup installs its input signature");
    assert_eq!(before_spy.1[0], Some(first_stamp));
    assert_eq!(game.vision_input_stamp(0), first_stamp);

    game.spies.insert(
        1,
        Spy {
            id: 1,
            owner: 0,
            level: 0,
            promotions: BTreeSet::new(),
            city: Some(city),
            ready_turn: 0,
            mission: None,
            sources_city: None,
            sources_until: 0,
            captured_by: None,
        },
    );
    let spy_stamp = game.vision_input_stamp(0);
    let after_spy = game
        .vision_frames
        .input_stamps
        .borrow()
        .clone()
        .expect("a changed spy input reinstalls the signature");
    assert_ne!(spy_stamp, first_stamp, "an established spy is sight");
    assert_ne!(
        after_spy.0.spies, before_spy.0.spies,
        "the memo key must notice direct writes to the public spy map"
    );
    let spy_frame = game.player_vision_frame(0);
    assert!(!Arc::ptr_eq(&first_frame, &spy_frame));

    let host_only = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| !game.sees(&spy_frame, *position))
        .expect("the controlled board has a tile outside the ordinary view");
    let before_host = game.vision_frames.input_stamps.borrow().clone();
    game.host_observed = Arc::new(BTreeSet::from([host_only]));
    let host_stamp = game.vision_input_stamp(0);
    assert_ne!(host_stamp, spy_stamp, "the host observation is sight");
    assert_eq!(
        *game.vision_frames.input_stamps.borrow(),
        before_host,
        "a populated host observation must take the uncached signature path"
    );
    let host_frame = game.player_vision_frame(0);
    assert!(game.sees(&host_frame, host_only));
    let mut heights = game.height_field();
    let uncached = game.player_vision(&mut heights, 0);
    assert!(host_frame.as_ref() == &uncached);
}

/// The unit half of the sight signature is cached behind the roster's
/// mutation epoch; this is the city half. Every ask used to rehash every
/// owned tile of every city the seat holds — about 8.6 M tile hashes over a
/// game — so the fold now lives on the roster and the ask reads one number.
///
/// What that trades away is the ability to notice a border move by rehashing
/// it, which is exactly what this test is here to keep honest: the roster
/// must drop the fold on *any* mutable access, the refolded signature must
/// still ignore the city state sight does not read, and every change that
/// does move a sight source must reach the frame.
#[test]
fn vision_frames_reuse_city_border_stamps_and_invalidate_on_a_border_change() {
    let (mut game, center) = controlled_game(63_100);
    let city = game.place_city(0, center, None);
    assert!(
        !game.cities[&city].owned_tiles.is_empty(),
        "a founded city owns its centre and ring"
    );

    let first = game.vision_frame(0, &mut game.height_field());
    let first_stamp = game.vision_input_stamp(0);
    assert!(
        game.cities.vision_stamps_cached(),
        "asking for sight installs the folded roster"
    );
    let again = game.vision_frame(0, &mut game.height_field());
    assert!(
        Arc::ptr_eq(&first, &again),
        "an unchanged roster should be reused"
    );

    // A city write that sight never reads still drops the fold — the
    // accessor hands out `&mut City` before the caller has said what it
    // intends to change — but the refolded signature is identical, so the
    // frame itself survives. This is the city analogue of a hitpoint write.
    game.cities.get_mut(&city).unwrap().food += 1.0;
    assert!(
        !game.cities.vision_stamps_cached(),
        "a mutable city handle evicts the fold on the way out"
    );
    let after_write = game.vision_frame(0, &mut game.height_field());
    assert!(
        Arc::ptr_eq(&first, &after_write),
        "non-sight city state should not rebuild sight"
    );
    assert_eq!(game.vision_input_stamp(0), first_stamp);
    assert!(
        game.cities.vision_stamps_cached(),
        "the next ask reinstalls the fold"
    );

    // Growing the border is the change this cache exists to notice.
    let grown = along(&game, center, 3);
    assert!(
        !game.cities[&city].owned_tiles.contains(&grown),
        "the test tile is outside the founded border"
    );
    game.cities.get_mut(&city).unwrap().owned_tiles.push(grown);
    let moved_border = game.vision_frame(0, &mut game.height_field());
    assert!(
        !Arc::ptr_eq(&first, &moved_border),
        "a border change must reject the cached frame"
    );
    assert_ne!(
        game.vision_input_stamp(0),
        first_stamp,
        "a border change must move the signature"
    );
    let mut heights = game.height_field();
    let uncached = game.player_vision(&mut heights, 0);
    assert!(
        moved_border.as_ref() == &uncached,
        "cached and fresh sight differ after a border change"
    );

    // A branch inherits the fold with the roster it was cloned from, and
    // restamping inside the branch leaves the source world alone.
    let mut branch = game.speculative_clone();
    assert!(
        branch.cities.vision_stamps_cached(),
        "a clone inherits the fold for the roster it copied"
    );
    let inherited = branch.vision_frame(0, &mut branch.height_field());
    assert!(
        Arc::ptr_eq(&moved_border, &inherited),
        "an unchanged branch should inherit the populated frame"
    );
    let branch_growth = along(&branch, center, 4);
    branch
        .cities
        .get_mut(&city)
        .unwrap()
        .owned_tiles
        .push(branch_growth);
    let branch_frame = branch.vision_frame(0, &mut branch.height_field());
    assert!(
        !Arc::ptr_eq(&moved_border, &branch_frame),
        "a branch that moves a border must reject the inherited frame"
    );
    let mut branch_heights = branch.height_field();
    let branch_uncached = branch.player_vision(&mut branch_heights, 0);
    assert!(
        branch_frame.as_ref() == &branch_uncached,
        "a changed branch must recompute the exact uncached frame"
    );
    assert!(
        Arc::ptr_eq(
            &moved_border,
            &game.vision_frame(0, &mut game.height_field())
        ),
        "branch recomputation must not replace the source world's frame"
    );

    // Handing the city to a rival moves the same sight source between seats.
    // The fold is keyed by owner, so both signatures have to move.
    let rival_before = game.vision_input_stamp(1);
    let owner_before = game.vision_input_stamp(0);
    game.cities.get_mut(&city).unwrap().owner = 1;
    assert_ne!(
        game.vision_input_stamp(0),
        owner_before,
        "the losing seat must stop seeing the border it no longer owns"
    );
    assert_ne!(
        game.vision_input_stamp(1),
        rival_before,
        "the gaining seat must start seeing it"
    );
    let captured = game.vision_frame(0, &mut game.height_field());
    let mut captured_heights = game.height_field();
    let captured_uncached = game.player_vision(&mut captured_heights, 0);
    assert!(
        captured.as_ref() == &captured_uncached,
        "a captured city must leave the old owner the sight an uncached walk gives"
    );

    // Razing is a structural change rather than a field write, and the
    // roster is the only thing that can report it.
    let razed_before = game.vision_input_stamp(1);
    game.cities.remove(&city);
    assert!(
        !game.cities.vision_stamps_cached(),
        "removing a city evicts the fold"
    );
    assert_ne!(
        game.vision_input_stamp(1),
        razed_before,
        "a razed city must move its owner's signature"
    );
    let razed = game.vision_frame(1, &mut game.height_field());
    let mut razed_heights = game.height_field();
    let razed_uncached = game.player_vision(&mut razed_heights, 1);
    assert!(
        razed.as_ref() == &razed_uncached,
        "a razed city must leave the exact uncached frame behind"
    );
}

/// Whole-roster replacement is the case an epoch counter beside the cache
/// would get wrong: the counter travels with the new cities while the memo
/// stays with the old. The fold is a field of the roster precisely so that
/// `mem::take`, `mem::swap` and plain assignment cannot come apart from it.
#[test]
fn vision_frames_follow_a_wholesale_roster_replacement() {
    let (mut game, center) = controlled_game(63_104);
    let city = game.place_city(0, center, None);
    let populated = game.vision_frame(0, &mut game.height_field());
    let populated_stamp = game.vision_input_stamp(0);
    assert!(game.cities.vision_stamps_cached());

    // A roster taken out and put back unchanged is the same roster, memo and
    // all, so nothing about sight may move.
    let roster = std::mem::take(&mut game.cities);
    assert!(
        !game.cities.vision_stamps_cached(),
        "the emptied field carries no fold"
    );
    assert_ne!(
        game.vision_input_stamp(0),
        populated_stamp,
        "a seat with no cities does not sign like a seat with one"
    );
    game.cities = roster;
    assert_eq!(
        game.vision_input_stamp(0),
        populated_stamp,
        "restoring the roster restores the signature"
    );
    assert!(
        Arc::ptr_eq(&populated, &game.vision_frame(0, &mut game.height_field())),
        "restoring the roster restores the frame"
    );

    // A roster replaced by a *different* one must be signed as that one, even
    // though the field it lands in previously held a fold of its own.
    let mut other = game.clone();
    other
        .cities
        .get_mut(&city)
        .unwrap()
        .owned_tiles
        .push(along(&game, center, 3));
    let other_stamp = other.vision_input_stamp(0);
    assert_ne!(other_stamp, populated_stamp);

    // Give the source roster the same mutation generation through a write
    // sight ignores. The replacement below must still restamp from city
    // content rather than mistaking equal counters for equal rosters.
    game.cities.get_mut(&city).unwrap().food += 1.0;
    assert_eq!(game.cities.generation(), other.cities.generation());
    assert_eq!(game.vision_input_stamp(0), populated_stamp);
    game.cities = other.cities.clone();
    assert_eq!(
        game.vision_input_stamp(0),
        other_stamp,
        "an assigned roster brings its own fold, not the one it displaced"
    );
    let mut heights = game.height_field();
    let uncached = game.player_vision(&mut heights, 0);
    assert!(game.vision_frame(0, &mut game.height_field()).as_ref() == &uncached);
}

/// The suzerain map and shared-vision viewer set are folded once per
/// diplomacy epoch (`Game::diplomacy_epoch`) instead of once per ask -- see
/// `Game::with_suzerain_input_map` and `Game::with_visibility_viewers` -- so
/// an input the epoch fails to notice would leave a stale answer installed
/// silently rather than merely slow. Prove the memoized answer always agrees
/// with a from-scratch derivation across every input the epoch is supposed
/// to track (suzerainty, a raw envoy count that does not flip it, a unit
/// move), and that it correctly ignores a spy establishing -- neither
/// `suzerain_input_map` nor `visibility_viewers` reads a spy, so the
/// diplomacy epoch must hold still even though the overall sight frame moves
/// through the final memo's separate compact spy signature. A unit move gets
/// no such epoch-stability claim: `Game::relocate` reveals ground through
/// `Game::reveal`, which writes the mover's own seat state, so the epoch is
/// allowed to move there too -- only the answer is required to stay exact.
#[test]
fn diplomacy_caches_agree_with_an_uncached_derivation_across_every_input() {
    let (mut game, center) = controlled_game(63_105);
    game.players[1].is_minor = true;
    let minor = 1usize;
    let city = game.found_city_for(minor, center, Some("Sight State".to_string()));

    let assert_caches_agree = |game: &Game, label: &str| {
        let fresh_suzerains = game.suzerain_input_map();
        let memo_suzerains = game.with_suzerain_input_map(|suzerains| suzerains.clone());
        assert_eq!(
            memo_suzerains, fresh_suzerains,
            "{label}: memoized suzerain map must match a fresh derivation"
        );
        let fresh_viewers = game.visibility_viewers(0);
        let memo_viewers = game.with_visibility_viewers(0, |viewers| viewers.clone());
        assert_eq!(
            memo_viewers, fresh_viewers,
            "{label}: memoized viewer set must match a fresh derivation"
        );
        let mut heights = game.height_field();
        let uncached = game.player_vision(&mut heights, 0);
        assert!(
            game.player_vision_frame(0).as_ref() == &uncached,
            "{label}: the full sight frame must match an uncached derivation"
        );
    };

    // Baseline: no envoys placed yet.
    assert_eq!(game.suzerain_of(minor), None);
    assert_caches_agree(&game, "no envoys");

    // Suzerainty change: crossing the three-envoy threshold.
    game.players[0].envoys.push((minor, 3));
    assert_eq!(game.suzerain_of(minor), Some(0));
    assert_caches_agree(&game, "suzerainty gained");

    // An envoy change that does not flip the suzerain still has to move the
    // diplomacy epoch -- a `Players` write happened -- even though the
    // derived map ends up with the same content either way.
    let epoch_before = game.diplomacy_epoch();
    for entry in game.players[0].envoys.iter_mut() {
        if entry.0 == minor {
            entry.1 += 3;
        }
    }
    assert_ne!(
        game.diplomacy_epoch(),
        epoch_before,
        "an envoy write must move the diplomacy epoch even without flipping suzerainty"
    );
    assert_eq!(game.suzerain_of(minor), Some(0));
    assert_caches_agree(&game, "envoy count changed, suzerain unchanged");

    // A unit move reads neither a suzerain nor a viewer set directly, but
    // `Game::relocate` reveals ground through `Game::reveal`, which writes
    // the mover's own `explored`/contact state -- a `Players` write like any
    // other -- so the diplomacy epoch is allowed to move here. What must not
    // move is the *answer*: the memoized map and viewer set still have to
    // match a fresh derivation on the far side of it.
    let scout = game.spawn_unit("scout", 0, along(&game, center, 2));
    assert_caches_agree(&game, "after spawning a unit");
    game.relocate(scout, along(&game, center, 3));
    assert_caches_agree(&game, "after a unit move");

    // A spy is the same story for this cache, but not for the overall
    // stamp: the final input memo has a separate compact live-spy signature,
    // so the full frame must still notice one appearing.
    let epoch_before = game.diplomacy_epoch();
    let overall_before = game.vision_input_stamp(0);
    game.spies.insert(
        1,
        Spy {
            id: 1,
            owner: 0,
            level: 0,
            promotions: BTreeSet::new(),
            city: Some(city),
            ready_turn: 0,
            mission: None,
            sources_city: None,
            sources_until: 0,
            captured_by: None,
        },
    );
    assert_eq!(
        game.diplomacy_epoch(),
        epoch_before,
        "establishing a spy must not move the diplomacy epoch"
    );
    assert_ne!(
        game.vision_input_stamp(0),
        overall_before,
        "a spy is still a live sight source, folded outside this cache"
    );
    assert_caches_agree(&game, "after a spy is established");
}

fn observed_tile(observation: &serde_json::Value, position: Pos) -> &serde_json::Value {
    observation["map"]["tiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tile| tile["pos"] == json!([position.0, position.1]))
        .expect("tile is in the observation")
}

/// Civilization VI does not put a rival on your diplomacy screen because
/// the map generator dealt them in. Somebody has to walk far enough to
/// find them, and until they do, that empire is not on the ledger.
#[test]
fn a_civilization_is_met_only_when_something_of_theirs_is_seen() {
    let (mut game, center) = controlled_game(63_101);
    let scout = game.spawn_unit("scout", 0, center);
    let far = along(&game, center, 9);
    game.spawn_unit("warrior", 1, far);
    game.refresh_all_visibility();
    assert!(
        !game.has_met(0, 1),
        "a warrior nine tiles away has not been found"
    );
    assert!(!game.has_met(1, 0), "and has found nobody either");

    // Walk the scout to within sight of it, the long way round: contact is
    // read off the same visibility the observation is.
    let doorstep = along(&game, center, 7);
    game.units.get_mut(&scout).unwrap().pos = doorstep;
    game.refresh_all_visibility();
    assert!(game.has_met(0, 1), "a warrior two tiles away is in sight");
    assert!(
        game.has_met(1, 0),
        "meeting is mutual — a scout that sees is a scout that was seen"
    );

    // And it stays met once the scout walks home again.
    game.units.get_mut(&scout).unwrap().pos = center;
    game.refresh_all_visibility();
    assert!(
        game.has_met(0, 1),
        "an empire once found is never forgotten"
    );
}

/// Diplomacy needs somebody to conduct it with. Every act on the panel is
/// withheld until contact; only belligerents pulled into an existing war
/// by a team or defensive pact are introduced by that war itself.
#[test]
fn diplomacy_waits_for_contact_before_a_declaration() {
    let (mut game, center) = controlled_game(63_102);
    game.spawn_unit("warrior", 0, center);
    game.spawn_unit("warrior", 1, along(&game, center, 9));
    game.refresh_all_visibility();
    assert!(!game.has_met(0, 1));
    assert!(
        !game
            .legal_actions(0)
            .iter()
            .any(|action| matches!(action, Action::DeclareWar { player: 1 })),
        "there is no embassy to declare war on"
    );

    game.record_contact(0, 1);
    assert!(
        game.legal_actions(0)
            .iter()
            .any(|action| matches!(action, Action::DeclareWar { player: 1 })),
        "once met, the whole panel opens"
    );
    assert_eq!(game.apply(0, &Action::DeclareWar { player: 1 }), Ok(()));
    assert!(game.is_at_war(0, 1));
    assert!(game.has_met(1, 0), "the prewar contact remains mutual");
}

/// `legal_actions` is a discovery aid, not the authority boundary: every
/// controller ultimately submits an `Action` directly to `Game::apply`.
/// Hidden civilizations must therefore be rejected by the handlers too,
/// without leaving a grievance, offer, or war behind.
#[test]
fn direct_bilateral_diplomacy_cannot_bypass_contact() {
    let (mut game, center) = controlled_game(63_103);
    game.spawn_unit("warrior", 0, center);
    game.spawn_unit("warrior", 1, along(&game, center, 9));
    game.refresh_all_visibility();
    assert!(!game.has_met(0, 1));

    let actions = [
        Action::DeclareWar { player: 1 },
        Action::DeclareWarWithCasusBelli {
            player: 1,
            casus_belli: "golden_age_war".to_string(),
        },
        Action::Denounce { player: 1 },
        Action::ProposeDeal {
            player: 1,
            give_gold: 0.0,
            request_gold: 0.0,
            open_borders: false,
            friendship: true,
            peace: false,
            alliance: None,
        },
    ];
    for action in actions {
        let mut attempt = game.clone();
        assert!(
            attempt.apply(0, &action).is_err(),
            "hidden diplomacy was accepted: {action:?}"
        );
        assert!(!attempt.is_at_war(0, 1));
        assert!(attempt.pending_deals.is_empty());
        assert!(attempt.players[0].denounced_until.is_empty());
        assert!(attempt.players[0].grievances.is_empty());
        assert!(attempt.players[1].grievances.is_empty());
    }

    game.record_contact(0, 1);
    assert!(game.apply(0, &Action::Denounce { player: 1 }).is_ok());
}

#[test]
fn stock_unit_sight_ranges_match_civilization_vi() {
    let rules = Rules::embedded();
    let sight_three: BTreeSet<&str> = [
        "settler",
        "spy",
        "caravel",
        "ironclad",
        "destroyer",
        "missile_cruiser",
        "observation_balloon",
        "rocket_artillery",
        "helicopter",
        "giant_death_robot",
        "naturalist",
        // Two unique units inherit the sight of what they replace: the Nau is
        // a Caravel and the Oromo Cavalry a Courser with a Scout's eye. The Nau
        // reached this list late — `data/units.json` gave it 2, so the anchor
        // agreed with the bug rather than with Civ VI, and the fidelity audit
        // could not see either because `UNIT_PORTUGUESE_NAU` was compared
        // against nothing until its alias landed.
        "nau",
        "oromo_cavalry",
        // The Varu and Voi Chien ship three sight in their own unit rows; the
        // generic heavy-cavalry and crossbowman fallbacks each had only two.
        "varu",
        "voi_chien",
    ]
    .into_iter()
    .collect();
    let sight_four: BTreeSet<&str> = ["biplane", "fighter", "bomber"].into_iter().collect();
    let sight_five: BTreeSet<&str> = ["drone", "jet_fighter", "jet_bomber"].into_iter().collect();

    for (unit, spec) in &rules.units {
        let expected = if sight_three.contains(unit.as_str()) {
            3
        } else if sight_four.contains(unit.as_str()) {
            4
        } else if sight_five.contains(unit.as_str()) {
            5
        } else {
            2
        };
        assert_eq!(spec.sight, expected, "incorrect stock sight for {unit}");
    }
}

#[test]
fn kongo_shield_bearer_matches_firaxis_identity_movement_sight_and_defense() {
    let (mut game, origin) = controlled_game(91_011);
    let woods = along(&game, origin, 1);
    let beyond = along(&game, origin, 2);
    game.map.tiles.get_mut(&woods).unwrap().feature = Some(crate::name!("forest"));
    let unit = game.spawn_unit("kongo_shield_bearer", 0, origin);
    let spec = &game.rules.units["kongo_shield_bearer"];

    assert_eq!(
        (spec.cost, spec.maintenance, spec.strength),
        (110.0, 2.0, 38.0)
    );
    assert_eq!(spec.resource_cost, 5.0);
    assert_eq!(spec.replaces.as_deref(), Some("swordsman"));
    assert_eq!(spec.upgrade_to.as_deref(), Some("man_at_arms"));
    assert_eq!(game.unit_step_cost(unit, origin, woods), 1.0);
    assert!(game.unit_visible_tiles(unit).contains(&beyond));
    assert_eq!(game.ranged_defense_bonus(&game.units[&unit], false), 10.0);
}

#[test]
fn oromo_cavalry_matches_firaxis_identity_sight_and_hill_movement() {
    let (mut game, origin) = controlled_game(91_012);
    let hill = along(&game, origin, 1);
    game.map.tiles.get_mut(&hill).unwrap().hills = true;
    game.players[0].civ = "Ethiopia".to_string();
    let unit = game.spawn_unit("oromo_cavalry", 0, origin);
    let spec = &game.rules.units["oromo_cavalry"];

    assert_eq!(
        (spec.cost, spec.maintenance, spec.strength),
        (200.0, 3.0, 48.0)
    );
    assert_eq!((spec.moves, spec.sight), (5.0, 3));
    assert_eq!(spec.resource_cost, 10.0);
    assert_eq!(spec.requires_resource.as_deref(), Some("horses"));
    assert_eq!(spec.replaces.as_deref(), Some("courser"));
    assert_eq!(spec.upgrade_to.as_deref(), Some("cavalry"));
    assert_eq!(game.unit_step_cost(unit, origin, hill), 1.0);
}

#[test]
fn bireme_matches_firaxis_identity_and_replaces_the_galley() {
    let (mut game, origin) = controlled_game(91_013);
    game.players[0].civ = "Phoenicia".to_string();
    let spec = &game.rules.units["bireme"];

    assert_eq!(
        (spec.cost, spec.maintenance, spec.moves, spec.strength),
        (65.0, 1.0, 4.0, 35.0)
    );
    assert_eq!(spec.replaces.as_deref(), Some("galley"));
    assert_eq!(spec.upgrade_to.as_deref(), Some("caravel"));
    assert_eq!(
        game.rules.civs["Phoenicia"].unique_unit.as_deref(),
        Some("bireme")
    );
    assert_eq!(
        game.player_unit_replacement(0, crate::name!("galley")),
        "bireme"
    );

    let unit = game.spawn_unit("bireme", 0, origin);
    assert_eq!(game.unit_max_moves(unit), 4.0);
    assert_eq!(game.unit_unembarked_strength(&game.units[&unit]), 35.0);
}

#[test]
fn terrain_elevation_features_and_promotions_control_live_sight() {
    let (mut game, origin) = controlled_game(91_001);
    let blocker = along(&game, origin, 1);
    let target = along(&game, origin, 2);
    let beyond = along(&game, origin, 3);
    let warrior = game.spawn_unit("warrior", 0, origin);

    assert!(game.unit_visible_tiles(warrior).contains(&target));
    assert!(!game.unit_visible_tiles(warrior).contains(&beyond));

    game.map.tiles.get_mut(&blocker).unwrap().feature = Some(crate::name!("forest"));
    let visible = game.unit_visible_tiles(warrior);
    assert!(
        visible.contains(&blocker),
        "adjacent tiles are always visible"
    );
    assert!(!visible.contains(&target), "flat Woods block a flat viewer");
    let hidden_enemy = game.spawn_unit("warrior", 1, target);
    assert!(
        !crate::obs::observation(&game, 0)["units"]
            .as_array()
            .unwrap()
            .iter()
            .any(|unit| unit["id"] == hidden_enemy),
        "an enemy behind blocking terrain must not leak into the player view"
    );

    game.map.tiles.get_mut(&target).unwrap().hills = true;
    game.map.tiles.get_mut(&target).unwrap().feature = Some(crate::name!("forest"));
    assert!(
        game.unit_visible_tiles(warrior).contains(&target),
        "a wooded Hill rises above intervening flat Woods"
    );
    assert!(crate::obs::observation(&game, 0)["units"]
        .as_array()
        .unwrap()
        .iter()
        .any(|unit| unit["id"] == hidden_enemy));

    game.map.tiles.get_mut(&target).unwrap().hills = false;
    game.map.tiles.get_mut(&target).unwrap().feature = None;
    game.map.tiles.get_mut(&origin).unwrap().hills = true;
    assert!(
        game.unit_visible_tiles(warrior).contains(&target),
        "a Hill gives enough elevation to see over flat Woods"
    );
    game.map.tiles.get_mut(&blocker).unwrap().hills = true;
    assert!(
        !game.unit_visible_tiles(warrior).contains(&target),
        "wooded Hills still block a unit standing on a bare Hill"
    );

    game.units
        .get_mut(&warrior)
        .unwrap()
        .promotions
        .insert(crate::name!("sentry"));
    assert!(
        game.unit_visible_tiles(warrior).contains(&target),
        "Sentry sees through Woods and Rainforest"
    );
    game.map.tiles.get_mut(&blocker).unwrap().terrain = crate::name!("mountain");
    assert!(
        !game.unit_visible_tiles(warrior).contains(&target),
        "Sentry cannot see through Mountains"
    );
    let aircraft = game.spawn_unit("biplane", 0, origin);
    assert!(
        game.unit_visible_tiles(aircraft).contains(&target),
        "aircraft sight ignores terrain obstruction"
    );
    game.units
        .get_mut(&warrior)
        .unwrap()
        .promotions
        .insert(crate::name!("spyglass"));
    game.map.tiles.get_mut(&blocker).unwrap().terrain = crate::name!("plains");
    game.map.tiles.get_mut(&blocker).unwrap().feature = None;
    game.map.tiles.get_mut(&blocker).unwrap().hills = false;
    assert!(game.unit_visible_tiles(warrior).contains(&beyond));
}

/// Sight range is a hard cap. Elevation decides what a unit sees *past*,
/// never how far it sees, so a mountain range outside a Settler's three
/// tiles stays dark — the bug that had a turn-one start revealing tiles
/// four away.
#[test]
fn no_terrain_is_ever_visible_beyond_a_unit_s_sight_range() {
    let (mut game, origin) = controlled_game(91_010);
    let settler = game.spawn_unit("settler", 0, origin);
    assert_eq!(game.unit_sight(settler), 3, "a Settler sees three tiles");

    // Open ground the whole way out, then a mountain range starting exactly
    // one tile past the Settler's range: nothing hides these peaks except
    // the range itself.
    for distance in 4..=6 {
        let position = along(&game, origin, distance);
        game.map.tiles.get_mut(&position).unwrap().terrain = crate::name!("mountain");
    }
    let visible = game.unit_visible_tiles(settler);
    assert!(visible.contains(&along(&game, origin, 3)));
    for distance in 4..=6 {
        let position = along(&game, origin, distance);
        assert!(
            !visible.contains(&position),
            "a mountain {distance} tiles away is outside a three-tile Settler's sight"
        );
    }
    assert_eq!(
        visible.iter().map(|at| game.wdist(origin, *at)).max(),
        Some(3),
        "nothing at all is seen past the printed range"
    );

    // The same cap holds for the shorter ranges and for wooded ground.
    let warrior = game.spawn_unit("warrior", 0, origin);
    assert_eq!(game.unit_sight(warrior), 2);
    for distance in 3..=4 {
        let position = along(&game, origin, distance);
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.hills = true;
        tile.feature = Some(crate::name!("forest"));
    }
    assert_eq!(
        game.unit_visible_tiles(warrior)
            .iter()
            .map(|at| game.wdist(origin, *at))
            .max(),
        Some(2)
    );
}

/// The shipped `SightModifier`/`SightThroughModifier` columns: Mountain 2,
/// Hills 1, flat 0, with Woods and Rainforest adding 1 to whatever they
/// stand on. A blocker hides everything no taller than itself, which is why
/// a Mountain shows over Woods and Hills but not over a wooded Hill.
#[test]
fn civilization_vi_sight_levels_decide_what_rises_above_cover() {
    let (mut game, origin) = controlled_game(91_011);
    let blocker = along(&game, origin, 1);
    let target = along(&game, origin, 2);
    let warrior = game.spawn_unit("warrior", 0, origin);

    let shape = |game: &mut Game, at: Pos, terrain: &str, hills: bool, feature: Option<&str>| {
        let tile = game.map.tiles.get_mut(&at).unwrap();
        tile.terrain = Name::new(terrain);
        tile.hills = hills;
        tile.feature = feature.map(Name::new);
    };

    // Level-1 cover: flat Woods, flat Rainforest, and a bare Hill.
    for (cover, hills, feature) in [
        ("plains", false, Some("forest")),
        ("plains", false, Some("jungle")),
        ("plains", true, None),
    ] {
        shape(&mut game, blocker, cover, hills, feature);

        shape(&mut game, target, "mountain", false, None);
        assert!(
            game.unit_visible_tiles(warrior).contains(&target),
            "a Mountain rises over {cover} cover at level 1"
        );

        shape(&mut game, target, "plains", true, Some("forest"));
        assert!(
            game.unit_visible_tiles(warrior).contains(&target),
            "a wooded Hill rises over {cover} cover at level 1"
        );

        shape(&mut game, target, "plains", false, Some("forest"));
        assert!(
            !game.unit_visible_tiles(warrior).contains(&target),
            "flat Woods are no taller than {cover} cover and stay hidden"
        );

        shape(&mut game, target, "plains", true, None);
        assert!(
            !game.unit_visible_tiles(warrior).contains(&target),
            "a bare Hill is level 1 like {cover} cover and stays hidden too"
        );
    }

    // Level-2 cover: a wooded Hill stands exactly as tall as a Mountain, so
    // neither shows past the other.
    shape(&mut game, blocker, "plains", true, Some("forest"));
    shape(&mut game, target, "mountain", false, None);
    assert!(
        !game.unit_visible_tiles(warrior).contains(&target),
        "a Mountain does not rise over a wooded Hill"
    );
    shape(&mut game, blocker, "mountain", false, None);
    shape(&mut game, target, "plains", true, Some("forest"));
    assert!(
        !game.unit_visible_tiles(warrior).contains(&target),
        "and a wooded Hill does not rise over a Mountain"
    );

    // A viewer's own elevation is terrain alone — standing in Woods is not
    // standing higher, but standing on a Mountain is.
    shape(&mut game, blocker, "plains", false, Some("forest"));
    shape(&mut game, target, "plains", false, None);
    shape(&mut game, origin, "plains", false, Some("forest"));
    assert!(
        !game.unit_visible_tiles(warrior).contains(&target),
        "Woods underfoot are not a vantage point"
    );
    shape(&mut game, origin, "mountain", false, None);
    shape(&mut game, blocker, "plains", true, Some("forest"));
    assert!(
        game.unit_visible_tiles(warrior).contains(&target),
        "a Mountain looks out over level-2 cover"
    );
}

/// Natural Wonders carry their own shipped `SightThroughModifier` rather
/// than one blanket height: Everest and Yosemite are cover above anything
/// else on the map, while Crater Lake, the Pantanal, the Dead Sea and the
/// Great Barrier Reef block nothing.
#[test]
fn natural_wonders_block_sight_only_where_civilization_vi_says_they_do() {
    let (mut game, origin) = controlled_game(91_012);
    let blocker = along(&game, origin, 1);
    let target = along(&game, origin, 2);
    let warrior = game.spawn_unit("warrior", 0, origin);

    for wonder in ["crater_lake", "pantanal", "dead_sea", "great_barrier_reef"] {
        game.map.tiles.get_mut(&blocker).unwrap().feature = Some(Name::new(wonder));
        assert!(
            game.unit_visible_tiles(warrior).contains(&target),
            "{wonder} is flat ground for line of sight"
        );
    }
    for wonder in ["mount_everest", "yosemite", "uluru", "pamukkale"] {
        game.map.tiles.get_mut(&blocker).unwrap().feature = Some(Name::new(wonder));
        assert!(
            !game.unit_visible_tiles(warrior).contains(&target),
            "{wonder} is cover"
        );
    }

    // Everest at level 2 over flat ground is visible past anything a Hill or
    // Woods can offer, and is itself cover a wooded Hill cannot see over.
    game.map.tiles.get_mut(&blocker).unwrap().feature = Some(crate::name!("forest"));
    game.map.tiles.get_mut(&target).unwrap().feature = Some(crate::name!("mount_everest"));
    assert!(game.unit_visible_tiles(warrior).contains(&target));
    game.map.tiles.get_mut(&blocker).unwrap().hills = true;
    assert!(
        !game.unit_visible_tiles(warrior).contains(&target),
        "level-2 Everest does not rise over a level-2 wooded Hill"
    );
}

#[test]
fn owned_borders_and_their_outer_ring_are_always_visible() {
    let (mut game, center) = controlled_game(91_002);
    let city = game.found_city_for(0, center, Some("Border Test".to_string()));
    let owned = game.cities[&city].owned_tiles.clone();
    for position in &owned {
        let tile = game.map.tiles.get_mut(position).unwrap();
        tile.terrain = crate::name!("mountain");
        tile.feature = Some(crate::name!("forest"));
    }

    let visible = game.player_visibility(0);
    for position in &owned {
        assert!(visible.contains(position));
        for neighbor in game.nbrs(*position) {
            assert!(
                visible.contains(&neighbor),
                "the tile immediately outside an empire border is visible"
            );
        }
    }
    let outside = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| {
            owned
                .iter()
                .all(|border| game.wdist(*border, *position) > 1)
        })
        .expect("map has a tile beyond the border ring");
    assert!(!visible.contains(&outside));
}

#[test]
fn suzerain_reveals_exactly_three_tiles_around_the_city_state() {
    let (mut game, center) = controlled_game(91_005);
    game.players[1].is_minor = true;
    let city = game.found_city_for(1, center, Some("Sight State".to_string()));
    game.players[0].envoys.push((1, 3));
    assert_eq!(game.suzerain_of(1), Some(0));

    let visible = game.player_visibility(0);
    for position in game.wdisk(game.cities[&city].pos, 3) {
        assert!(
            visible.contains(&position),
            "the complete three-tile suzerain radius must be visible"
        );
    }
    let outside = along(&game, center, 4);
    assert!(
        !visible.contains(&outside),
        "suzerainty does not copy the city-state's unrelated unit sight"
    );
}

#[test]
fn active_emergency_members_share_live_sight_not_only_map_memory() {
    let (mut game, center) = controlled_game(91_006);
    let unit = game.spawn_unit("warrior", 1, center);
    game.active_emergencies.push(Emergency {
        id: 1,
        kind: "military".to_string(),
        target: 0,
        city: 0,
        original_owner: 0,
        members: BTreeSet::from([0, 1]),
        contributions: BTreeMap::new(),
        started: game.turn,
        ends: game.turn + 30,
    });

    assert!(game.player_visibility(0).contains(&center));
    assert!(crate::obs::observation(&game, 0)["units"]
        .as_array()
        .unwrap()
        .iter()
        .any(|known| known["id"] == unit));

    game.turn += 30;
    assert!(!game.player_visibility(0).contains(&center));
}

#[test]
fn defensible_districts_have_their_stock_visibility_elevation() {
    let (mut game, origin) = controlled_game(91_007);
    let district = along(&game, origin, 1);
    let target = along(&game, origin, 2);
    game.found_city_for(1, district, Some("High Walls".to_string()));
    let warrior = game.spawn_unit("warrior", 0, origin);

    assert!(game.unit_visible_tiles(warrior).contains(&district));
    assert!(
        !game.unit_visible_tiles(warrior).contains(&target),
        "a flat City Center blocks a flat viewer's sight"
    );
    game.map.tiles.get_mut(&origin).unwrap().hills = true;
    assert!(
        game.unit_visible_tiles(warrior).contains(&target),
        "a Hill supplies the matching vantage level"
    );
}

#[test]
fn indirect_fire_still_requires_a_current_friendly_spotter() {
    let (mut game, origin) = controlled_game(91_008);
    let blocker = along(&game, origin, 1);
    let target = along(&game, origin, 3);
    game.map.tiles.get_mut(&blocker).unwrap().terrain = crate::name!("mountain");
    let artillery = game.spawn_unit("rocket_artillery", 0, origin);
    game.spawn_unit("warrior", 1, target);
    game.at_war.insert(pair(0, 1));
    let attack = Action::Ranged {
        unit: artillery,
        target,
    };

    assert!(!game.player_visibility(0).contains(&target));
    assert!(!game.legal_actions(0).contains(&attack));
    assert_eq!(
        game.apply(0, &attack),
        Err("target is not visible".to_string())
    );

    let spotter = game
        .nbrs(target)
        .into_iter()
        .find(|position| *position != along(&game, origin, 2))
        .unwrap();
    game.spawn_unit("warrior", 0, spotter);
    assert!(game.player_visibility(0).contains(&target));
    assert!(game.legal_actions(0).contains(&attack));
}

#[test]
fn shared_exploration_keeps_the_newest_memory_without_copying_units() {
    let (mut game, position) = controlled_game(91_004);
    game.turn = 1;
    game.map.tiles.get_mut(&position).unwrap().improvement = Some(crate::name!("farm"));
    game.reveal(0, position, 0);

    game.turn = 2;
    game.map.tiles.get_mut(&position).unwrap().improvement = Some(crate::name!("mine"));
    game.reveal(1, position, 0);
    let hidden_unit = game.spawn_unit("warrior", 1, position);
    game.remove_unit(hidden_unit);

    game.share_visibility_memories(&[0, 1]);
    let shared = &game.players[0].remembered_tiles[&position];
    assert_eq!(shared.seen_turn, 2);
    assert_eq!(shared.tile.improvement.as_deref(), Some("mine"));
    assert!(
        game.units.is_empty(),
        "map memory never contains unit state"
    );
}

#[test]
fn fog_uses_last_seen_tiles_and_cities_and_never_remembers_units() {
    let (mut game, origin) = controlled_game(91_003);
    let remembered_position = along(&game, origin, 2);
    let city_position = along(&game, origin, -2);
    game.map
        .tiles
        .get_mut(&remembered_position)
        .unwrap()
        .improvement = Some(crate::name!("farm"));
    let scout = game.spawn_unit("warrior", 0, origin);
    let enemy = game.spawn_unit("warrior", 1, remembered_position);
    let city = game.found_city_for(1, city_position, Some("Last Seen".to_string()));
    game.cities.get_mut(&city).unwrap().pop = 4;
    game.refresh_player_visibility(0);

    let visible = crate::obs::observation(&game, 0);
    assert_eq!(
        observed_tile(&visible, remembered_position)["improvement"],
        "farm"
    );
    // Appeal is read off the tile's *current* neighbours, so it is reported
    // for what a player can see and withheld from what they only remember.
    assert!(observed_tile(&visible, remembered_position)["appeal"].is_i64());
    assert!(visible["units"]
        .as_array()
        .unwrap()
        .iter()
        .any(|unit| unit["id"] == enemy));
    assert_eq!(
        visible["cities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|known| known["id"] == city)
            .unwrap()["pop"],
        4
    );

    game.remove_unit(scout);
    game.map
        .tiles
        .get_mut(&remembered_position)
        .unwrap()
        .improvement = Some(crate::name!("mine"));
    game.units.get_mut(&enemy).unwrap().hp = 41;
    game.cities.get_mut(&city).unwrap().pop = 9;
    game.cities.get_mut(&city).unwrap().name = "Changed Under Fog".to_string();

    let held = crate::obs::observation(&game, 0);
    assert!(!game.player_visibility(0).contains(&remembered_position));
    assert!(!held["visible"]
        .as_array()
        .unwrap()
        .contains(&json!([remembered_position.0, remembered_position.1])));
    assert!(held["turn_visible"]
        .as_array()
        .unwrap()
        .contains(&json!([remembered_position.0, remembered_position.1])));
    assert_eq!(
        observed_tile(&held, remembered_position)["improvement"],
        "farm",
        "turn visibility holds the last frame rather than leaking a change under fog"
    );
    let held_enemy = held["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|unit| unit["id"] == enemy)
        .expect("the last-seen enemy remains on its turn-visible tile");
    assert_eq!(
        held_enemy["hp"], 100,
        "a turn contact is a last-seen snapshot, not its changing hidden state"
    );
    let held_city = held["cities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|known| known["id"] == city)
        .unwrap();
    assert_eq!(held_city["name"], "Last Seen");
    assert_eq!(held_city["pop"], 4);

    game.apply(0, &Action::EndTurn).unwrap();
    let fogged = crate::obs::observation(&game, 0);
    assert!(!game.player_visibility(0).contains(&remembered_position));
    assert!(!fogged["turn_visible"]
        .as_array()
        .unwrap()
        .contains(&json!([remembered_position.0, remembered_position.1])));
    assert_eq!(
        observed_tile(&fogged, remembered_position)["improvement"],
        "farm"
    );
    assert!(observed_tile(&fogged, remembered_position)["appeal"].is_null());
    assert!(!fogged["units"]
        .as_array()
        .unwrap()
        .iter()
        .any(|unit| unit["id"] == enemy));
    let remembered_city = fogged["cities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|known| known["id"] == city)
        .unwrap();
    assert_eq!(remembered_city["name"], "Last Seen");
    assert_eq!(remembered_city["pop"], 4);

    let mut restored: Game = serde_json::from_value(serde_json::to_value(&game).unwrap()).unwrap();
    let restored_fog = crate::obs::observation(&restored, 0);
    assert_eq!(
        observed_tile(&restored_fog, remembered_position)["improvement"],
        "farm",
        "serialized memory must not refresh a fogged tile while loading"
    );

    restored.spawn_unit("warrior", 0, origin);
    let revealed = crate::obs::observation(&restored, 0);
    assert_eq!(
        observed_tile(&revealed, remembered_position)["improvement"],
        "mine"
    );
    assert!(revealed["units"]
        .as_array()
        .unwrap()
        .iter()
        .any(|unit| unit["id"] == enemy));
    let revealed_city = revealed["cities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|known| known["id"] == city)
        .unwrap();
    assert_eq!(revealed_city["name"], "Changed Under Fog");
    assert_eq!(
        revealed_city["pop"], restored.cities[&city].pop,
        "seeing the city again reveals its post-turn live population"
    );
}

#[test]
fn fog_learns_a_city_founded_on_previously_explored_ground() {
    let (mut game, origin) = controlled_game(91_011);
    let explored_position = along(&game, origin, 2);
    let unexplored_position = along(&game, origin, -2);

    // This is historical map knowledge, not current sight.  A later settlement
    // at this plot must reach the map just as it does in Civilization VI, while
    // a settlement where the player has never explored must remain hidden.
    game.reveal(0, explored_position, 0);
    assert!(game.players[0].explored.contains(&explored_position));
    assert!(!game.player_visibility(0).contains(&explored_position));

    let known_city = game.found_city_for(1, explored_position, Some("New Horizon".to_string()));
    let hidden_city = game.found_city_for(1, unexplored_position, Some("Still Hidden".to_string()));
    game.refresh_player_visibility(0);

    let observation = crate::obs::observation(&game, 0);
    let cities = observation["cities"].as_array().expect("a city list");
    assert!(
        cities.iter().any(|city| city["id"] == known_city),
        "a city founded on explored ground must appear through fog"
    );
    assert!(
        !cities.iter().any(|city| city["id"] == hidden_city),
        "a city founded on unexplored ground must stay hidden"
    );
}

#[test]
fn moving_sight_updates_the_live_perimeter_but_holds_contacts_until_player_end_turn() {
    let (mut game, origin) = controlled_game(91_010);
    let trailing = along(&game, origin, -2);
    let step = along(&game, origin, 1);
    let scout = game.spawn_unit("warrior", 0, origin);
    let enemy = game.spawn_unit("warrior", 1, trailing);
    game.refresh_all_visibility();
    assert!(game.player_visibility(0).contains(&trailing));

    game.apply(
        0,
        &Action::Move {
            unit: scout,
            to: step,
        },
    )
    .unwrap();
    assert!(
        !game.player_visibility(0).contains(&trailing),
        "the exact current sight perimeter follows the moving unit"
    );
    let held = crate::obs::observation(&game, 0);
    assert!(!held["visible"]
        .as_array()
        .unwrap()
        .contains(&json!([trailing.0, trailing.1])));
    assert!(held["turn_visible"]
        .as_array()
        .unwrap()
        .contains(&json!([trailing.0, trailing.1])));
    assert!(held["units"]
        .as_array()
        .unwrap()
        .iter()
        .any(|unit| unit["id"] == enemy));

    let world_turn = game.turn;
    game.apply(0, &Action::EndTurn).unwrap();
    assert_eq!(
        game.turn, world_turn,
        "turn memory expires at the player's boundary before the world turn wraps"
    );
    let remembered = crate::obs::observation(&game, 0);
    assert!(!remembered["turn_visible"]
        .as_array()
        .unwrap()
        .contains(&json!([trailing.0, trailing.1])));
    assert!(!remembered["units"]
        .as_array()
        .unwrap()
        .iter()
        .any(|unit| unit["id"] == enemy));
}

#[test]
fn first_major_to_meet_a_city_state_receives_its_automatic_envoy() {
    let mut game = Game::new_full(2, 28, 18, 91_031, 120, 1, false);
    let city_state = game
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .expect("the fixture seats one city-state");
    let free_before = game.players[1].envoys_free;

    game.record_contact(1, city_state);
    assert_eq!(game.envoys_at(1, city_state), 1);
    assert_eq!(
        game.players[1].envoys_free, free_before,
        "first discovery places the envoy at the city-state rather than adding a free stock"
    );

    // Discovery remains a world fact after the discoverer is eliminated;
    // a later civilization does not become "first" retroactively.
    game.players[1].alive = false;
    game.record_contact(0, city_state);
    assert_eq!(
        game.envoys_at(0, city_state),
        0,
        "a later visitor must not receive the first-discovery envoy"
    );
    assert_eq!(game.envoys_at(1, city_state), 1);
}

#[test]
fn popping_a_village_counts_in_the_player_counters() {
    let mut game = Game::new_full(2, 28, 18, 91_032, 120, 0, false);
    let unit = game
        .units
        .iter()
        .find(|(_, unit)| unit.owner == 0)
        .map(|(id, _)| *id)
        .expect("player 0 starts with a unit");
    let pos = game.units[&unit].pos;

    game.map.tiles.get_mut(&pos).unwrap().improvement = Some(crate::name!("goody_hut"));
    game.maybe_goody_hut(unit);
    assert_eq!(
        game.players[0].counters.get("goody_huts_claimed"),
        Some(&1),
        "a popped tribal village must be tallied even outside the Ancient era"
    );
    assert!(game.map.get(pos).unwrap().improvement.is_none());

    game.map.tiles.get_mut(&pos).unwrap().improvement = Some(crate::name!("meteor_goody"));
    game.maybe_goody_hut(unit);
    assert_eq!(
        game.players[0].counters.get("meteor_goodies_claimed"),
        Some(&1)
    );
    assert_eq!(game.players[0].counters.get("goody_huts_claimed"), Some(&1));
}

#[test]
fn browser_lights_turn_memory_but_traces_only_exact_current_sight() {
    const INDEX: &str = include_str!("../../web/assets/app.js");
    assert!(INDEX.contains("function drawFlatVisibilityPerimeter(tiles, visible)"));
    assert!(INDEX.contains("function drawPlanetVisibilityPerimeter(cells, visible)"));
    assert!(INDEX.contains("const visible = new Set(state.visible.map(key));"));
    assert!(INDEX
        .contains("const turnVisible = new Set((state.turn_visible || state.visible).map(key));"));
    assert!(
        INDEX.contains("const visSet = new Set((state.turn_visible || state.visible).map(key));")
    );
    assert!(INDEX.contains("drawPlanetVisibilityPerimeter(cells, visible);"));
    assert!(INDEX.contains("drawFlatVisibilityPerimeter(tiles, visible);"));
}

#[test]
fn deferred_ai_visibility_publishes_the_same_seat_boundary_state() {
    let (mut immediate, origin) = controlled_game(91_009);
    let scout = immediate.spawn_unit("scout", 0, origin);
    let first = along(&immediate, origin, 1);
    let second = along(&immediate, origin, 2);
    let enemy = along(&immediate, origin, 4);
    immediate.spawn_unit("warrior", 1, enemy);
    immediate.refresh_all_visibility();
    let mut deferred = immediate.clone();
    let mut parallel = immediate.clone();

    let play = |game: &mut Game| {
        game.apply(
            0,
            &Action::Move {
                unit: scout,
                to: first,
            },
        )
        .unwrap();
        game.apply(
            0,
            &Action::Move {
                unit: scout,
                to: second,
            },
        )
        .unwrap();
        game.apply(0, &Action::EndTurn).unwrap();
    };
    play(&mut immediate);
    deferred.with_deferred_visibility(play);
    let pool = WorkPool::new(4);
    parallel.visibility_batch.depth += 1;
    play(&mut parallel);
    parallel.visibility_batch.depth -= 1;
    assert!(parallel.visibility_batch.refresh_all);
    parallel.visibility_batch.refresh_all = false;
    parallel.visibility_batch.refresh_teams.clear();
    parallel.refresh_all_visibility_parallel(&pool);

    assert_eq!(
        serde_json::to_value(&deferred).unwrap(),
        serde_json::to_value(&immediate).unwrap(),
        "coalescing may change when visibility is derived, never the game or fog memory published at the seat boundary"
    );
    assert_eq!(
        serde_json::to_value(&parallel).unwrap(),
        serde_json::to_value(&immediate).unwrap(),
        "parallel sight computation must publish the same game and fog memory"
    );
    for pid in 0..immediate.players.len() {
        assert_eq!(
            crate::obs::observation(&deferred, pid),
            crate::obs::observation(&immediate, pid),
            "seat {pid} must receive the same observation"
        );
        assert_eq!(
            crate::obs::observation(&parallel, pid),
            crate::obs::observation(&immediate, pid),
            "parallel visibility must publish seat {pid}'s exact observation"
        );
    }
}

#[test]
fn speculative_clones_share_valid_sight_work_and_keep_rules_state() {
    let (mut game, origin) = controlled_game(91_011);
    let scout = game.spawn_unit("scout", 0, origin);
    let _ = game.unit_visible_tiles(scout);
    assert!(
        !game.vision.borrow().entries.seen.is_empty(),
        "the control world should have a populated unit-vision cache"
    );
    let _ = game.wonder_built("pyramids");
    assert!(
        game.vision.borrow().built_wonders.is_some(),
        "the control world should also have its independent production catalog"
    );

    let source_explored = game.players[0].explored.len();
    let mut branch = game.speculative_clone();
    assert!(
        Arc::ptr_eq(
            &game.vision.borrow().entries,
            &branch.vision.borrow().entries
        ),
        "a clone should borrow immutable sight rays until a changed source misses its key"
    );
    assert!(
        !branch.vision.borrow().entries.seen.is_empty(),
        "the branch should retain the source ray table rather than begin cold"
    );
    assert!(
        branch.vision.borrow().built_wonders.is_none(),
        "only immutable sight rays cross into the speculative branch"
    );
    assert!(!branch.track_fog_memory);
    assert!(!branch.track_war_ledger);
    assert!(branch.visibility_suppressed);
    assert_eq!(branch.units.len(), game.units.len());
    assert_eq!(branch.cities.len(), game.cities.len());

    let explored = branch.players[0].explored.len();
    branch.spawn_unit("warrior", 0, along(&branch, origin, 1));
    assert_eq!(
        branch.players[0].explored.len(),
        explored,
        "a disposable branch does not copy or grow fog exploration"
    );
    assert_eq!(
        game.players[0].explored.len(),
        source_explored,
        "the branch's observer-only work must not leak to its source"
    );
}
