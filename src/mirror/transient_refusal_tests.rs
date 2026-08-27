use super::*;

/// Same temp-dir convention as the rest of this file's tests: `tempfile` is
/// not a dependency of this crate.
fn events(name: &str, lines: &[&str]) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("civvis-refusal-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("events.jsonl");
    std::fs::write(&path, lines.join("\n") + "\n").expect("write events");
    path
}

/// ⚠⚠⚠ `blocked_improvement_sites` is extended and NEVER cleared, so
/// anything that reaches it is a permanent verdict on that ground.
///
/// On run `civvis-20260811T103914Z` the builder had `movesRemaining == 0`
/// on 25 of 26 refusals — a condition that clears itself next turn — and
/// every one of those tiles was being blacklisted for the rest of the game.
#[test]
fn a_builder_out_of_moves_does_not_kill_the_tile_forever() {
    let p = events(
        "outofmoves",
        &[r#"{"kind":"improve_refused","turn":5,"x":10,"y":12,"moves":0}"#],
    );
    let refused = refused_sites_of_kind_through(&p, "improve_refused", None);
    assert!(
        refused.is_empty(),
        "a builder that merely ran out of movement must not cost the empire \
             the tile: {refused:?}"
    );
}

/// The set still has to do its job. A refusal with movement left is the
/// engine rejecting the GROUND, which is exactly what it exists to record.
#[test]
fn a_refusal_with_movement_left_still_blocks_the_tile() {
    let p = events(
        "hasmoves",
        &[r#"{"kind":"improve_refused","turn":5,"x":10,"y":12,"moves":2}"#],
    );
    let refused = refused_sites_of_kind_through(&p, "improve_refused", None);
    assert_eq!(refused.len(), 1, "a genuine refusal must still block");
}

/// ⚠⚠ THE SAME FILTER HAS TO COVER `found_refused`, and that must be proven
/// rather than assumed from the fact that one function serves both.
///
/// `found_refused` feeds `blocked_city_sites`, which is also extended and
/// never cleared. Across every live run of 2026-08-11, 9 found refusals: the
/// settler had `movesRemaining == 0` on EIGHT. A condemned city site is the
/// more expensive half of this defect — expansion is this project's measured
/// binding constraint, with 36% of games ending on one city.
#[test]
fn a_settler_out_of_moves_does_not_kill_the_city_site_forever() {
    let p = events(
        "settler",
        &[
            r#"{"kind":"found_refused","turn":9,"x":4,"y":7,"moves":0}"#,
            r#"{"kind":"found_refused","turn":9,"x":5,"y":8,"moves":2}"#,
        ],
    );
    let refused = refused_sites_of_kind_through(&p, "found_refused", None);
    assert_eq!(
        refused.len(),
        1,
        "the spent-move site must survive and the genuine refusal must \
             still block: {refused:?}"
    );
    assert!(refused.contains(&crate::hex::offset_to_axial(5, 8)));
}

/// ⚠ EACH CASE NEEDS ITS OWN FILE. `events` builds a path from the name and
/// the process id, so four tests passing the same name share one events.jsonl
/// and overwrite each other under `cargo test`'s parallelism. Mine did: the
/// stale-improvement case failed in the full run and passed alone, which is
/// the signature of a shared fixture rather than a logic error.
fn improved_snapshot(name: &str, lines: &[&str]) -> Snapshot {
    let p = events(name, lines);
    snapshot_from_events_at(&p, None).expect("snapshot")
}

const SWEEP: &str = r#"{"kind":"tiles","turn":16,"width":4,"height":4,"chunk":1,"plots":[{"x":1,"y":1,"t":"TERRAIN_GRASS","o":0}]}"#;

/// ★ The point: a finished improvement is on the board before the next sweep
/// repeats it. 23 duplicate orders in one run came from this gap.
#[test]
fn a_finished_improvement_reaches_the_board_before_the_next_sweep() {
    let snap = improved_snapshot(
        "improved_reaches",
        &[
            SWEEP,
            r#"{"kind":"improved","turn":18,"x":1,"y":1,"im":"IMPROVEMENT_MINE"}"#,
        ],
    );
    assert_eq!(
        snap.plot((1, 1)).and_then(|p| p.im.clone()),
        Some("IMPROVEMENT_MINE".to_string())
    );
}

/// ⚠ RULE 1: only `im` is touched. `from_chunks` REPLACES a plot, so the
/// cheap version — folding a partial plot in as a one-plot chunk — would
/// strip the tile's terrain and owner. This is why it is a field mutation.
#[test]
fn folding_an_improvement_keeps_the_rest_of_the_plot() {
    let snap = improved_snapshot(
        "improved_keeps",
        &[
            SWEEP,
            r#"{"kind":"improved","turn":18,"x":1,"y":1,"im":"IMPROVEMENT_MINE"}"#,
        ],
    );
    let plot = snap.plot((1, 1)).expect("the plot survives");
    assert_eq!(
        plot.t.as_deref(),
        Some("TERRAIN_GRASS"),
        "terrain must survive"
    );
    assert_eq!(plot.o, 0, "owner must survive");
}

/// ⚠ RULE 2: never invent ground. An improvement on a plot the seat has not
/// revealed would hand the simulator information the seat does not have.
#[test]
fn an_improvement_on_unrevealed_ground_is_ignored() {
    let snap = improved_snapshot(
        "improved_unseen",
        &[
            SWEEP,
            r#"{"kind":"improved","turn":18,"x":3,"y":3,"im":"IMPROVEMENT_MINE"}"#,
        ],
    );
    assert!(snap.plot((3, 3)).is_none(), "unseen ground stays unseen");
}

/// ⚠ RULE 3: an older event cannot override a fresher sweep — which is what
/// keeps a removed improvement from coming back.
#[test]
fn a_stale_improvement_never_overrides_a_newer_sweep() {
    let snap = improved_snapshot(
        "improved_stale",
        &[
            r#"{"kind":"improved","turn":5,"x":1,"y":1,"im":"IMPROVEMENT_MINE"}"#,
            SWEEP,
        ],
    );
    assert_eq!(
        snap.plot((1, 1)).and_then(|p| p.im.clone()),
        None,
        "the turn-16 sweep says bare and it is newer than the turn-5 event"
    );
}

/// ⚠⚠ `build_no_plot` already carries the discriminator and the block ignored
/// it. `Game::blocked_districts` says zero offered plots means the district is
/// impossible ANYWHERE (a Government Plaza that already exists), while above
/// zero is "a placement disagreement in one city that must not stop the
/// empire" — the district IS placeable there, CIVVIS just named a plot the
/// engine would not take.
///
/// Across every live run of 2026-08-11, 47 events: **41 had `offered > 0`**.
#[test]
fn a_wrong_plot_does_not_block_a_placeable_district() {
    let p = events(
        "noplot",
        &[
            r#"{"kind":"build_no_plot","turn":40,"city":7,"district":"DISTRICT_CAMPUS","offered":4}"#,
            r#"{"kind":"build_no_plot","turn":41,"city":7,"district":"DISTRICT_GOVERNMENT","offered":0}"#,
        ],
    );
    let refused = refused_no_plot_through(&p, None, "district", "DISTRICT_");
    let blocked = refused
        .get(&7)
        .expect("the impossible district still blocks");
    assert!(
        !blocked.contains("DISTRICT_CAMPUS"),
        "a Campus with four offered plots is placeable; only the tile was wrong"
    );
    assert!(
        blocked.contains("DISTRICT_GOVERNMENT"),
        "zero offered plots is the engine saying nowhere, and must still block"
    );
}

/// A zero-site wonder response is a world fact, not the city-local cooldown
/// used for a wrong-coordinate response. Keep only explicit modern telemetry:
/// an old event without `offered` cannot prove that the wonder is gone.
#[test]
fn a_zero_site_wonder_becomes_a_permanent_world_fact() {
    let p = events(
        "world_wonder",
        &[
            r#"{"kind":"build_no_plot","turn":40,"city":7,"building":"BUILDING_GREAT_BATH","offered":0}"#,
            r#"{"kind":"build_no_plot","turn":41,"city":8,"building":"BUILDING_PYRAMIDS","offered":2}"#,
            r#"{"kind":"build_no_plot","turn":42,"city":8,"building":"BUILDING_ORACLE"}"#,
            r#"{"kind":"build_no_plot","city":8,"building":"BUILDING_ORACLE","offered":0}"#,
            r#"{"kind":"build_no_plot","turn":43,"city":8,"building":"BUILDING_NOT_MODELED","offered":0}"#,
            r#"{"kind":"build_no_plot","turn":50,"city":8,"building":"BUILDING_HANGING_GARDENS","offered":0}"#,
            r#"{"kind":"state","turn":49}"#,
        ],
    );
    let state = state_from_events(&p, None).expect("state at the current turn");
    assert_eq!(
        state.host_unavailable_wonders,
        BTreeSet::from([
            "BUILDING_GREAT_BATH".to_string(),
            "BUILDING_NOT_MODELED".to_string(),
        ]),
        "only an explicit, timestamped zero-target answer before this board becomes a world fact"
    );
    assert_eq!(
        host_unavailable_wonders_from(
            &state.host_unavailable_wonders,
            &crate::rules::Rules::embedded(),
        ),
        BTreeSet::from([Name::new("great_bath")]),
        "unknown host names stay observable in the state but cannot populate a dead gate"
    );
}

/// ⚠⚠⚠ "Never block it" was the wrong half. #1555 dropped these refusals
/// entirely and the very next full run showed the loop it recreated:
/// `civvis-20260811T202458Z`, 28 `build_no_plot` events in 250 turns, **all
/// 28 the same pair** — one Commercial Hub asked for and refused twenty-eight
/// times because nothing remembered the previous twenty-seven.
///
/// A fresh placement disagreement blocks, which ends the loop.
#[test]
fn a_fresh_placement_disagreement_blocks() {
    let p = events(
        "noplot_fresh",
        &[
            r#"{"kind":"build_no_plot","turn":40,"city":7,"district":"DISTRICT_CAMPUS","offered":4}"#,
        ],
    );
    let refused = refused_no_plot_through(&p, Some(42), "district", "DISTRICT_");
    assert!(
        refused[&7].contains("DISTRICT_CAMPUS"),
        "or it is asked every turn"
    );
}

/// The host already supplied the way out of a wrong-coordinate refusal. Keep
/// only the latest fresh offer: an old positive answer must not override a newer
/// zero-site answer, and neither belongs on a later board after the cooldown.
#[test]
fn fresh_host_district_sites_follow_the_newest_offer() {
    let p = events(
        "host_sites",
        &[
            r#"{"kind":"build_no_plot","turn":40,"city":7,"district":"DISTRICT_CAMPUS","offered":2,"offered_plots":[{"x":10,"y":8}]}"#,
            r#"{"kind":"build_no_plot","turn":41,"city":7,"district":"DISTRICT_CAMPUS","offered":1,"offered_plots":[{"x":10,"y":7}]}"#,
            r#"{"kind":"build_no_plot","turn":42,"city":7,"district":"DISTRICT_THEATER","offered":1,"offered_plots":[{"x":9,"y":8}]}"#,
            r#"{"kind":"build_no_plot","turn":43,"city":7,"district":"DISTRICT_THEATER","offered":0,"offered_plots":[]}"#,
            r#"{"kind":"state","turn":49}"#,
        ],
    );
    let state = state_from_events(&p, Some(49)).expect("state at the current turn");
    let campus = state
        .host_district_sites
        .get(&7)
        .and_then(|by_district| by_district.get("DISTRICT_CAMPUS"))
        .expect("the latest positive Campus offer is fresh");
    assert_eq!(
        campus.iter().copied().collect::<Vec<_>>(),
        vec![crate::hex::offset_to_axial(10, 7)],
        "the newest host location replaces the older coordinate rather than merging it"
    );
    let city_ids: BTreeMap<u32, i64> = [(99, 7)].into_iter().collect();
    let mapped = host_district_sites_from(
        &state.host_district_sites,
        &city_ids,
        &crate::rules::Rules::embedded(),
    );
    assert_eq!(
        mapped
            .get(&99)
            .and_then(|by_district| by_district.get(&crate::name::Name::new("campus"))),
        Some(campus),
        "the CIV6 city/name pair must reach its reconstructed city and district"
    );
    assert!(
        state
            .host_district_sites
            .get(&7)
            .is_none_or(|by_district| !by_district.contains_key("DISTRICT_THEATER")),
        "a newer zero-site answer withdraws the previous positive offer"
    );
    assert!(
        host_district_sites_through(&p, 50).is_empty(),
        "a placement response older than the production cooldown is no longer current"
    );
}

/// Wonders carry their production type under `building`, rather than the
/// district key. Their host candidates must still replace an invalid CIVVIS
/// coordinate and vanish after a newer zero response or the normal TTL.
#[test]
fn fresh_host_wonder_sites_follow_the_newest_offer() {
    let p = events(
        "host_wonder_sites",
        &[
            r#"{"kind":"build_no_plot","turn":40,"city":7,"building":"BUILDING_PYRAMIDS","offered":2,"offered_plots":[{"x":10,"y":8}]}"#,
            r#"{"kind":"build_no_plot","turn":41,"city":7,"building":"BUILDING_PYRAMIDS","offered":1,"offered_plots":[{"x":10,"y":7}]}"#,
            r#"{"kind":"build_no_plot","turn":42,"city":7,"building":"BUILDING_ORACLE","offered":1,"offered_plots":[{"x":9,"y":8}]}"#,
            r#"{"kind":"build_no_plot","turn":43,"city":7,"building":"BUILDING_ORACLE","offered":0,"offered_plots":[]}"#,
            r#"{"kind":"state","turn":49}"#,
        ],
    );
    let state = state_from_events(&p, Some(49)).expect("state at the current turn");
    let pyramids = state
        .host_wonder_sites
        .get(&7)
        .and_then(|by_wonder| by_wonder.get("BUILDING_PYRAMIDS"))
        .expect("the latest positive Pyramids offer is fresh");
    assert_eq!(
        pyramids.iter().copied().collect::<Vec<_>>(),
        vec![crate::hex::offset_to_axial(10, 7)],
        "the newest host location replaces the older coordinate rather than merging it"
    );
    let city_ids: BTreeMap<u32, i64> = [(99, 7)].into_iter().collect();
    let mapped = host_wonder_sites_from(
        &state.host_wonder_sites,
        &city_ids,
        &crate::rules::Rules::embedded(),
    );
    assert_eq!(
        mapped
            .get(&99)
            .and_then(|by_wonder| by_wonder.get(&crate::name::Name::new("pyramids"))),
        Some(pyramids),
        "the CIV6 city/name pair must reach its reconstructed city and wonder"
    );
    assert!(
        state
            .host_wonder_sites
            .get(&7)
            .is_none_or(|by_wonder| !by_wonder.contains_key("BUILDING_ORACLE")),
        "a newer zero-site answer withdraws the previous positive offer"
    );
    assert!(
        host_wonder_sites_through(&p, 50).is_empty(),
        "a placement response older than the production cooldown is no longer current"
    );
}

/// And expires, which is what keeps the district from being foreclosed in a
/// city that may yet make room for it — the reason #1555 existed at all.
#[test]
fn a_stale_placement_disagreement_stops_blocking() {
    let p = events(
        "noplot_stale",
        &[
            r#"{"kind":"build_no_plot","turn":40,"city":7,"district":"DISTRICT_CAMPUS","offered":4}"#,
        ],
    );
    let refused = refused_no_plot_through(
        &p,
        Some(40 + PRODUCTION_REFUSAL_TTL + 1),
        "district",
        "DISTRICT_",
    );
    assert!(
        refused
            .get(&7)
            .is_none_or(|d| !d.contains("DISTRICT_CAMPUS")),
        "a placement disagreement must not condemn the city forever"
    );
}

/// ⚠ Zero offered plots is a different statement — the engine has no target
/// ANYWHERE, a Government Plaza that already exists — and that does not go
/// stale. It must still block long after the TTL.
#[test]
fn no_plot_anywhere_still_blocks_forever() {
    let p = events(
        "noplot_never",
        &[
            r#"{"kind":"build_no_plot","turn":40,"city":7,"district":"DISTRICT_GOVERNMENT","offered":0}"#,
        ],
    );
    let refused = refused_no_plot_through(
        &p,
        Some(40 + PRODUCTION_REFUSAL_TTL * 10),
        "district",
        "DISTRICT_",
    );
    assert!(refused[&7].contains("DISTRICT_GOVERNMENT"));
}

/// An absent `offered` is not a reading — older exports sent none, and those
/// must keep the old behaviour so a replayed run is unchanged.
#[test]
fn a_no_plot_event_without_offered_keeps_the_old_behaviour() {
    let p = events(
        "noplot_old",
        &[r#"{"kind":"build_no_plot","turn":40,"city":7,"district":"DISTRICT_CAMPUS"}"#],
    );
    let refused = refused_no_plot_through(&p, None, "district", "DISTRICT_");
    assert!(refused[&7].contains("DISTRICT_CAMPUS"));
}

/// ⚠ Events written before #1548 carry no `moves`, and an absent reading is
/// not evidence of anything. Replaying an older run must be unchanged.
#[test]
fn a_refusal_that_never_recorded_moves_keeps_the_old_behaviour() {
    let p = events(
        "nomovesfield",
        &[r#"{"kind":"improve_refused","turn":5,"x":10,"y":12}"#],
    );
    let refused = refused_sites_of_kind_through(&p, "improve_refused", None);
    assert_eq!(refused.len(), 1, "no reading is not a transient reading");
}
