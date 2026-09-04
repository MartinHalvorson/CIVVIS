/// ⚠ AN ENEMY UNIT CIVVIS CANNOT SEE IS WORSE THAN A COSMETIC GAP.
///
/// Civilization VI names uniques by CIVILIZATION. Stripping that qualifier
/// from `UNIT_EGYPTIAN_CHARIOT_ARCHER` gives `chariot_archer`, but
/// `data/units.json` calls it **maryannu_chariot_archer**, so neither
/// spelling matched and the unit vanished from the board. Live on
/// `civvis-20260804T233745Z`:
///
///     UNITDATA ⚠ UNIT_EGYPTIAN_CHARIOT_ARCHER@(39, 24) count Civ6=1 CIVVIS=0
#[test]
fn a_unique_unit_resolves_through_its_noun() {
    let rules = crate::rules::Rules::embedded();
    assert_eq!(
        resolved_civvis_unit_name(&rules, "UNIT_EGYPTIAN_CHARIOT_ARCHER").as_deref(),
        Some("maryannu_chariot_archer"),
        "the observed live failure must resolve"
    );
    // The ordinary paths must keep working exactly as before.
    assert_eq!(
        resolved_civvis_unit_name(&rules, "UNIT_WARRIOR").as_deref(),
        Some("warrior")
    );
    assert_eq!(
        resolved_civvis_unit_name(&rules, "UNIT_ROMAN_LEGION").as_deref(),
        Some("legion"),
        "the civ-qualifier fallback already handled this and must not regress"
    );
    assert_eq!(
        resolved_civvis_unit_name(&rules, "UNIT_BARBARIAN_HORSEMAN").as_deref(),
        Some("barbarian_horseman"),
        "the host-only barbarian horseman must keep its slower, weaker profile"
    );
    assert_eq!(
        resolved_civvis_unit_name(&rules, "UNIT_BARBARIAN_HORSE_ARCHER").as_deref(),
        Some("barbarian_horse_archer"),
        "the host-only barbarian horse archer must not become a Saka archer"
    );
    // A Great Person is a MODELLING gap, not a naming one — there is no
    // entry to find and inventing one would be worse than reporting none.
    assert_eq!(
        resolved_civvis_unit_name(&rules, "UNIT_GREAT_SCIENTIST").as_deref(),
        None
    );
    // And a name that matches nothing must stay unresolved.
    assert_eq!(
        resolved_civvis_unit_name(&rules, "UNIT_NOT_A_REAL_UNIT").as_deref(),
        None
    );
}

use super::*;

fn plot(x: i32, y: i32, t: &str) -> Plot {
    Plot {
        x,
        y,
        im: None,
        t: Some(t.to_string()),
        f: None,
        r: None,
        o: -1,
        w: false,
        i: false,
        fw: None,
        rv: 0,
        ri: false,
        ct: None,
        cl: -1,
        p: false,
        d: None,
        dc: None,
        wo: None,
        rt: None,
        rp: false,
        yl: None,
        ap: None,
        np: false,
        vis: false,
    }
}

#[test]
fn the_embedded_vocabulary_is_present_and_complete() {
    // ⚠ The assertion that matters. Every cwd-relative asset read in this
    // project has eventually resolved to None somewhere real — the champion
    // genome, the league roster, and the value net, which has never once
    // loaded. An embedded table cannot do that, and this proves it is not
    // merely embedded but populated.
    let vocab = Vocabulary::embedded();
    assert_eq!(vocab.terrain_count(), 17, "all Civ 6 terrains");
    assert_eq!(vocab.feature_count(), 50, "all Civ 6 features");
    assert_eq!(vocab.resource_count(), 54, "all Civ 6 resources");
}

#[test]
fn persias_pairidaeza_crosses_as_a_real_improvement() {
    let mut site = plot(3, 4, "TERRAIN_GRASS");
    site.im = Some("IMPROVEMENT_PAIRIDAEZA".to_string());
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 1,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![site],
    }]);
    let game = rebuild_game(&snapshot, 2, 1);
    assert_eq!(
        game.map
            .get(crate::hex::offset_to_axial(3, 4))
            .unwrap()
            .improvement,
        Some(crate::name!("pairidaeza"))
    );
}

#[test]
fn armaghs_monastery_crosses_as_a_real_improvement() {
    let mut site = plot(3, 4, "TERRAIN_GRASS");
    site.im = Some("IMPROVEMENT_MONASTERY".to_string());
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 1,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![site],
    }]);
    let game = rebuild_game(&snapshot, 2, 1);
    assert_eq!(
        game.map
            .get(crate::hex::offset_to_axial(3, 4))
            .unwrap()
            .improvement,
        Some(crate::name!("monastery"))
    );
}

#[test]
fn historical_snapshot_does_not_read_tiles_from_a_future_turn() {
    let dir = std::env::temp_dir().join(format!(
        "civvis-mirror-time-boundary-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("events.jsonl");
    std::fs::write(
        &path,
        [
            r#"{"kind":"tiles","turn":1,"width":8,"height":8,"chunk":1,"plots":[{"x":1,"y":1,"t":"TERRAIN_GRASS"}]}"#,
            r#"{"kind":"tiles","turn":10,"width":8,"height":8,"chunk":1,"plots":[{"x":7,"y":7,"t":"TERRAIN_DESERT"}]}"#,
        ]
        .join("\n"),
    )
    .unwrap();

    let early = snapshot_from_events_at(&path, Some(1)).unwrap();
    assert_eq!(early.revealed_count(), 1);
    assert!(early.plot((1, 1)).is_some());
    assert!(early.plot((7, 7)).is_none(), "turn 1 must not see turn 10");
    let latest = snapshot_from_events(&path).unwrap();
    assert_eq!(latest.revealed_count(), 2);
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(dir);
}

/// ★ The mod publishes `state` then `tiles`, so the turn's own sweep and
/// same-frame deltas sit BELOW the state line they belong to. They are this
/// board; only a later frame's delta (next test) or a later turn is not.
#[test]
fn snapshot_reads_the_selected_turns_tiles_written_below_its_state() {
    let dir = std::env::temp_dir().join(format!(
        "civvis-mirror-tiles-below-state-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("events.jsonl");
    std::fs::write(
        &path,
        [
            r#"{"kind":"state","turn":1,"frame":0}"#,
            r#"{"kind":"tiles","turn":1,"width":8,"height":8,"chunk":1,"plots":[{"x":1,"y":1,"t":"TERRAIN_GRASS","f":"FEATURE_FOREST"}]}"#,
            r#"{"kind":"tiles","turn":1,"width":8,"height":8,"chunk":1,"delta":true,"plots":[{"x":2,"y":2,"t":"TERRAIN_PLAINS"}]}"#,
            r#"{"kind":"tiles","turn":1,"width":8,"height":8,"chunk":1,"delta":true,"frame":1,"plots":[{"x":1,"y":1,"t":"TERRAIN_GRASS","f":null}]}"#,
            r#"{"kind":"tiles","turn":2,"width":8,"height":8,"chunk":1,"plots":[{"x":7,"y":7,"t":"TERRAIN_DESERT"}]}"#,
        ]
        .join("\n"),
    )
    .unwrap();
    let turn_one = snapshot_from_events_at(&path, Some(1)).unwrap();
    assert_eq!(
        turn_one.revealed_count(),
        2,
        "the turn's sweep and frame-0 delta are its board"
    );
    assert_eq!(
        turn_one.plot((1, 1)).and_then(|plot| plot.f.as_deref()),
        Some("FEATURE_FOREST"),
        "the frame-1 delta below a frame-0 state is a later board"
    );
    assert!(
        turn_one.plot((7, 7)).is_none(),
        "turn 1 must not see turn 2"
    );
    let latest = snapshot_from_events(&path).unwrap();
    assert_eq!(
        latest.revealed_count(),
        2,
        "the latest state is still turn 1 frame 0"
    );
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(dir);
}

#[test]
fn snapshot_stops_at_the_selected_state_before_a_later_mid_turn_delta() {
    let dir = std::env::temp_dir().join(format!(
        "civvis-mirror-state-boundary-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("events.jsonl");
    std::fs::write(
        &path,
        [
            r#"{"kind":"tiles","turn":1,"width":8,"height":8,"chunk":1,"plots":[{"x":1,"y":1,"t":"TERRAIN_GRASS","f":"FEATURE_FOREST"}]}"#,
            r#"{"kind":"state","turn":5,"frame":0}"#,
            r#"{"kind":"tiles","turn":5,"width":8,"height":8,"chunk":1,"delta":true,"frame":1,"plots":[{"x":1,"y":1,"t":"TERRAIN_GRASS","f":null}]}"#,
        ]
        .join("\n"),
    )
    .unwrap();

    let snapshot = snapshot_from_events_at(&path, Some(5)).unwrap();
    assert_eq!(
        snapshot.plot((1, 1)).and_then(|plot| plot.f.as_deref()),
        Some("FEATURE_FOREST"),
        "the selected state must not be paired with a later frame delta",
    );
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(dir);
}

/// ★★★★ A TILES DELTA IS NEW GROUND, NOT A NEW SWEEP. The mod sends what
/// a unit revealed since the last board went out, every turn and frame,
/// stamped `delta`. It must merge onto the map like any chunk — that is
/// the whole point — and must NOT move the snapshot's sweep turn, or the
/// `improved` fold would discard every improvement finished between the
/// real sweep and the delta (rule 3 of `apply_finished_improvements`).
#[test]
fn a_tiles_delta_merges_new_ground_without_standing_for_a_sweep() {
    let dir =
        std::env::temp_dir().join(format!("civvis-mirror-tiles-delta-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("events.jsonl");
    std::fs::write(
        &path,
        [
            r#"{"kind":"tiles","turn":1,"width":8,"height":8,"chunk":1,"plots":[{"x":1,"y":1,"t":"TERRAIN_GRASS"}]}"#,
            r#"{"kind":"improved","turn":3,"x":1,"y":1,"im":"IMPROVEMENT_FARM"}"#,
            r#"{"kind":"tiles","turn":5,"width":8,"height":8,"chunk":1,"delta":true,"frame":1,"plots":[{"x":2,"y":1,"t":"TERRAIN_PLAINS"}]}"#,
        ]
        .join("\n"),
    )
    .unwrap();

    let snapshot = snapshot_from_events(&path).unwrap();
    assert_eq!(
        snapshot.revealed_count(),
        2,
        "the delta's plot is on the map"
    );
    assert_eq!(
        snapshot.plot((2, 1)).and_then(|plot| plot.t.as_deref()),
        Some("TERRAIN_PLAINS")
    );
    assert_eq!(snapshot.turn, 1, "the sweep turn is the last FULL sweep's");
    assert_eq!(
        snapshot.plot((1, 1)).and_then(|plot| plot.im.as_deref()),
        Some("IMPROVEMENT_FARM"),
        "an improvement finished after the sweep survives a later delta"
    );

    // Stream order decides a plot, whichever kind of chunk carried it:
    // a later sweep overrides an earlier delta's owner, and a later
    // delta overrides the sweep's.
    std::fs::write(
        &path,
        [
            r#"{"kind":"tiles","turn":5,"width":8,"height":8,"chunk":1,"delta":true,"plots":[{"x":2,"y":1,"t":"TERRAIN_PLAINS","o":3}]}"#,
            r#"{"kind":"tiles","turn":25,"width":8,"height":8,"chunk":1,"plots":[{"x":2,"y":1,"t":"TERRAIN_PLAINS","o":-1}]}"#,
            r#"{"kind":"tiles","turn":26,"width":8,"height":8,"chunk":1,"delta":true,"plots":[{"x":2,"y":1,"t":"TERRAIN_PLAINS","o":4}]}"#,
        ]
        .join("\n"),
    )
    .unwrap();
    let at_sweep = snapshot_from_events_at(&path, Some(25)).unwrap();
    assert_eq!(at_sweep.plot((2, 1)).map(|plot| plot.o), Some(-1));
    assert_eq!(at_sweep.turn, 25);
    let latest = snapshot_from_events(&path).unwrap();
    assert_eq!(latest.plot((2, 1)).map(|plot| plot.o), Some(4));
    assert_eq!(latest.turn, 25, "the delta at turn 26 is not a sweep");
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(dir);
}

#[test]
fn recent_host_production_refusals_are_city_scoped_typed_and_expire() {
    let dir =
        std::env::temp_dir().join(format!("civvis-production-refusal-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("events.jsonl");
    std::fs::write(
        &path,
        concat!(
            r#"{"kind":"civvis_build_unplayable","turn":81,"city":12,"item":"BUILDING_UNIVERSITY"}"#,
            "\n",
            r#"{"kind":"civvis_build_unplayable","turn":89,"city":12,"item":"BUILDING_LIBRARY","reasons":["LOC_BUILDING_REQUIRES_DISTRICT"]}"#,
            "\n",
            r#"{"kind":"civvis_build_unplayable","turn":90,"city":14,"item":"PROJECT_ENHANCE_DISTRICT_THEATER"}"#,
            "\n",
            r#"{"kind":"civvis_build_unplayable","turn":91,"city":12,"item":"DISTRICT_CAMPUS"}"#,
            "\n",
            r#"{"kind":"purchase_refused","turn":80,"city":12,"item":"UNIT_BUILDER"}"#,
            "\n",
            r#"{"kind":"purchase_refused","turn":89,"city":12,"item":"UNIT_SETTLER","balance":768,"cost":220}"#,
            "\n",
            r#"{"kind":"purchase_refused","turn":90,"city":14,"item":"BUILDING_LIBRARY"}"#,
            "\n",
            r#"{"kind":"purchase_refused","turn":90,"city":14,"item":"DISTRICT_CAMPUS"}"#,
            "\n",
        ),
    )
    .expect("write events");

    let refused = refused_production(&path, 90);
    assert_eq!(
        refused.get(&12),
        Some(&std::collections::BTreeSet::from([
            "BUILDING_LIBRARY".to_string()
        ])),
        "the stale University, future Campus, and unsupported district event are absent"
    );
    assert_eq!(
        refused.get(&14),
        Some(&std::collections::BTreeSet::from([
            "PROJECT_ENHANCE_DISTRICT_THEATER".to_string()
        ]))
    );

    let rules = crate::rules::Rules::embedded();
    let city_ids = BTreeMap::from([(41, 12), (42, 14)]);
    let blocked = blocked_production_from(&refused, &city_ids, &rules);
    assert_eq!(
        blocked.get(&41),
        Some(&std::collections::BTreeSet::from([
            "building:library".to_string()
        ]))
    );
    assert_eq!(
        blocked.get(&42),
        Some(&std::collections::BTreeSet::from([
            "project:theater_square_festival".to_string()
        ])),
        "Firaxis's district-project name must translate through the same alias as orders"
    );
    let purchase_refusals = refused_purchases(&path, 90);
    assert_eq!(
        purchase_refusals.get(&12),
        Some(&std::collections::BTreeSet::from([
            "UNIT_SETTLER".to_string()
        ])),
        "an old purchase refusal expires while the current Settler refusal remains"
    );
    let blocked_purchases = blocked_production_from(&purchase_refusals, &city_ids, &rules);
    assert_eq!(
        blocked_purchases.get(&41),
        Some(&std::collections::BTreeSet::from([
            "unit:settler".to_string()
        ]))
    );
    assert_eq!(
        blocked_purchases.get(&42),
        Some(&std::collections::BTreeSet::from([
            "building:library".to_string(),
            "district:campus".to_string(),
        ])),
        "district purchase refusals do not need a production-placement plot"
    );

    let mut game = crate::game::Game::new(1, 20, 14, 73_001, 120, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &crate::game::Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];
    let warrior = crate::game::Item::Unit {
        unit: crate::name!("warrior"),
    };
    assert!(game.can_produce(0, city, &warrior));
    let _ = game.producible_items(0, city);
    game.replace_blocked_production(BTreeMap::from([(
        city,
        std::collections::BTreeSet::from(["unit:warrior".to_string()]),
    )]));
    assert!(
        !game.can_produce(0, city, &warrior),
        "the cooldown must reach the legal-production chokepoint"
    );
    assert!(
        !game.producible_items(0, city).contains(&warrior),
        "and invalidate a production menu cached before the host refusal arrived"
    );

    let settler_item = crate::game::Item::Unit {
        unit: crate::name!("settler"),
    };
    Arc::make_mut(&mut game.blocked_production).clear();
    game.cities.get_mut(&city).unwrap().pop = 4;
    game.players[0].gold = 10_000.0;
    assert!(game.can_produce(0, city, &settler_item));
    assert!(game
        .legal_actions_within(0, crate::game::ActionFamilies::PURCHASES)
        .iter()
        .any(
            |action| matches!(action, crate::game::Action::Buy { city: bought_at, unit, .. }
            if *bought_at == city && unit == "settler")
        ));
    game.replace_blocked_purchases(BTreeMap::from([(
        city,
        std::collections::BTreeSet::from(["unit:settler".to_string()]),
    )]));
    assert!(
        !game
            .legal_actions_within(0, crate::game::ActionFamilies::PURCHASES)
            .iter()
            .any(
                |action| matches!(action, crate::game::Action::Buy { city: bought_at, unit, .. }
                if *bought_at == city && unit == "settler")
            ),
        "the rejected host purchase must leave the purchase menu"
    );
    assert!(
        !game.legal_purchase_actions(0).iter().any(
            |action| matches!(action, crate::game::Action::Buy { city: bought_at, unit, .. }
            if *bought_at == city && unit == "settler")
        ),
        "the city-parallel purchase projection must enforce the same cooldown"
    );
    assert!(
        game.can_produce(0, city, &settler_item),
        "a purchase refusal must not suppress the production fallback"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A civilization-unique unit resolves; a Great Person does not, and must not be
/// forced to by stripping a prefix that is not a civilization.
///
/// ⚠ Both halves matter. Run `civvis-20260731T114437Z` dropped 175 units: 162
/// `UNIT_AZTEC_EAGLE_WARRIOR` (a real translation failure, since CIVVIS has
/// `eagle_warrior`) and 13 `UNIT_GREAT_GENERAL` (not a failure — CIVVIS models
/// Great People in `great_people.json`, not as units). Stripping the first token
/// unconditionally fixes the first and mis-reads the second as the civilization
/// "great".
#[test]
fn a_civ_qualifier_is_stripped_and_great_is_not() {
    assert_eq!(
        civvis_unit_name_unqualified("UNIT_AZTEC_EAGLE_WARRIOR").as_deref(),
        Some("eagle_warrior"),
        "a civ-unique unit is the bare unit; this is the 162"
    );
    assert_eq!(
        resolved_civvis_unit_name(&crate::rules::Rules::embedded(), "UNIT_MONGOLIAN_KESHIG")
            .as_deref(),
        Some("keshig"),
        "a visible Keshig is military intelligence and must reach the board"
    );
    assert_eq!(
        resolved_civvis_unit_name(&crate::rules::Rules::embedded(), "UNIT_POLISH_HUSSAR")
            .as_deref(),
        Some("winged_hussar")
    );
    assert_eq!(
        resolved_civvis_unit_name(
            &crate::rules::Rules::embedded(),
            "UNIT_ETHIOPIAN_OROMO_CAVALRY"
        )
        .as_deref(),
        Some("oromo_cavalry"),
        "the rival unit observed on fixed22 must reach the mirror board"
    );
    assert_eq!(
        resolved_civvis_unit_name(&crate::rules::Rules::embedded(), "UNIT_SCOTTISH_HIGHLANDER")
            .as_deref(),
        Some("ranger"),
        "Firaxis declares the Highlander as Scotland's Ranger replacement"
    );
    assert_eq!(
        resolved_civvis_unit_name(&crate::rules::Rules::embedded(), "UNIT_KOREAN_HWACHA")
            .as_deref(),
        Some("field_cannon"),
        "Firaxis declares the Hwacha as Korea's Field Cannon replacement"
    );
    assert_eq!(
        resolved_civvis_unit_name(&crate::rules::Rules::embedded(), "UNIT_GEORGIAN_KHEVSURETI")
            .as_deref(),
        Some("man_at_arms"),
        "Georgia's unmodelled Khevsureti must remain visible as its replacement role"
    );
    assert_eq!(
        resolved_civvis_unit_name(&crate::rules::Rules::embedded(), "UNIT_NUBIAN_PITATI")
            .as_deref(),
        Some("pitati_archer"),
        "the host's shortened Pitati type must round-trip to CIVVIS"
    );
    assert_eq!(
        resolved_civvis_unit_name(&crate::rules::Rules::embedded(), "UNIT_LAHORE_NIHANG")
            .as_deref(),
        Some("nihang"),
        "Nihang is a special suzerain unit, not a civilization-unique row"
    );
    assert_eq!(
        civvis_unit_name_unqualified("UNIT_GREAT_GENERAL"),
        None,
        "`great` is not a civilization, so there is no qualifier to remove"
    );
    assert_eq!(
        civvis_unit_name_unqualified("UNIT_SETTLER"),
        None,
        "a single-token name has no qualifier at all"
    );
}

/// The most frequent live approximations must use their shipped unique rows.
///
/// The local run corpus recorded 239 Cossack, 227 Samurai and 82 Hypaspist
/// observations as ordinary units.  Those substitutions changed combat strength
/// (and, for several rows, movement, sight or upgrade timing) in the threat board.
/// Keep both translation entry points covered: production queues use
/// `civvis_node_name`, while unit observations use `resolved_civvis_unit_name`.
#[test]
fn frequent_unique_units_keep_their_civ6_static_profiles() {
    let rules = crate::rules::Rules::embedded();
    let cases = [
        ("UNIT_GAUL_GAESATAE", "gaesatae", 20.0, 2.0, 2),
        ("UNIT_JAPANESE_SAMURAI", "samurai", 48.0, 2.0, 2),
        ("UNIT_MACEDONIAN_HYPASPIST", "hypaspist", 38.0, 2.0, 2),
        ("UNIT_INDIAN_VARU", "varu", 40.0, 2.0, 3),
        (
            "UNIT_MALI_MANDEKALU_CAVALRY",
            "mandekalu_cavalry",
            55.0,
            4.0,
            2,
        ),
        ("UNIT_RUSSIAN_COSSACK", "cossack", 67.0, 5.0, 2),
        ("UNIT_AMERICAN_ROUGH_RIDER", "rough_rider", 67.0, 5.0, 2),
        ("UNIT_VIETNAMESE_VOI_CHIEN", "voi_chien", 35.0, 3.0, 3),
    ];
    for (host, name, strength, moves, sight) in cases {
        assert_eq!(
            civvis_node_name(&rules.units, host, "UNIT_").as_deref(),
            Some(name),
            "production queue translation for {host}"
        );
        assert_eq!(
            resolved_civvis_unit_name(&rules, host).as_deref(),
            Some(name),
            "unit observation translation for {host}"
        );
        let spec = &rules.units[name];
        assert_eq!(spec.strength, strength, "combat for {name}");
        assert_eq!(spec.moves, moves, "movement for {name}");
        assert_eq!(spec.sight, sight, "sight for {name}");
    }
}

/// A stripped prefix is only a civilization qualifier when the destination
/// rules row says it is a unique unit. Otherwise a missing exact row must not
/// silently turn a modern host unit into an older ordinary unit.
#[test]
fn an_unmodelled_qualified_stock_unit_is_not_relabelled_as_its_tail() {
    for (host, exact, tail) in [
        ("UNIT_JET_FIGHTER", "jet_fighter", "fighter"),
        ("UNIT_JET_BOMBER", "jet_bomber", "bomber"),
        ("UNIT_NUCLEAR_SUBMARINE", "nuclear_submarine", "submarine"),
        ("UNIT_LINE_INFANTRY", "line_infantry", "infantry"),
        ("UNIT_ROCKET_ARTILLERY", "rocket_artillery", "artillery"),
        (
            "UNIT_MECHANIZED_INFANTRY",
            "mechanized_infantry",
            "infantry",
        ),
    ] {
        let mut rules = crate::rules::Rules::embedded();
        assert!(rules.units.remove(exact).is_some());
        assert!(rules.units.contains_key(tail));
        assert_eq!(
            resolved_civvis_unit_name(&rules, host),
            None,
            "a missing exact row must be reported for {host}, not mapped to {tail}"
        );
    }

    assert_eq!(
        resolved_civvis_unit_name(&crate::rules::Rules::embedded(), "UNIT_ROMAN_LEGION").as_deref(),
        Some("legion"),
        "the same fallback remains valid for a ruleset-declared unique"
    );
    assert_eq!(
        resolved_civvis_unit_name(
            &crate::rules::Rules::embedded(),
            "UNIT_EGYPTIAN_CHARIOT_ARCHER"
        )
        .as_deref(),
        Some("maryannu_chariot_archer"),
        "the noun suffix fallback remains valid for an epithet unique"
    );
}

/// Every Great Person is recognised as one, whatever profession it is.
#[test]
fn great_people_are_named_as_a_modelling_gap_not_a_translation_failure() {
    for civ6 in [
        "UNIT_GREAT_GENERAL",
        "UNIT_GREAT_PROPHET",
        "UNIT_GREAT_MERCHANT",
        "UNIT_GREAT_ADMIRAL",
        "UNIT_GREAT_ENGINEER",
    ] {
        assert!(is_great_person(civ6), "{civ6} is a Great Person");
    }
    for civ6 in ["UNIT_SETTLER", "UNIT_AZTEC_EAGLE_WARRIOR", "UNIT_WARRIOR"] {
        assert!(!is_great_person(civ6), "{civ6} is an ordinary unit");
    }
}

/// Revealed ground is the truth, in both directions.
///
/// Unseen ground has its own explicit terrain state. Whether a bounded frontier may
/// be probed is recorded separately and must never change that terrain into a guess.
#[test]
fn revealed_ground_is_the_truth_in_both_directions() {
    let chunks = vec![TilesChunk {
        turn: 4,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(6, 5, "TERRAIN_OCEAN")],
    }];
    let snapshot = Snapshot::from_chunks(&chunks);
    let game = rebuild_game(&snapshot, 4, 7);

    let seen = game.map.get(crate::hex::offset_to_axial(5, 5)).unwrap();
    assert_eq!(
        seen.terrain.as_str(),
        "grassland",
        "revealed grass is grass"
    );
    assert!(!game.rules.is_water(seen), "and it is not water");

    let seen_water = game.map.get(crate::hex::offset_to_axial(6, 5)).unwrap();
    assert!(
        game.rules.is_water(seen_water),
        "revealed ocean really is water — no frontier change may turn the sea into \
             land where the seat has actually looked"
    );

    let unseen = game.map.get(crate::hex::offset_to_axial(15, 15)).unwrap();
    assert_eq!(
        unseen.terrain.as_str(),
        "unknown",
        "unrevealed terrain must not secretly be generated land or ocean"
    );
    assert!(
        !unseen.assumed_traversable,
        "a bare reconstruction has no planning prior"
    );
}

/// ★★★★ Two camps seven tiles from Rome for a whole game, 121 attacks on
/// their raiders and none on the camps, eight of fourteen Settlers captured
/// (civvis-20260816T155856Z): the tile carried `barbarian_camp` and
/// `game.barb_camps` — what the home guard, the settle risk and
/// `defensibility` read — stayed empty. The host's camps now reach the
/// register on every apply, and a camp the host cleared leaves it.
#[test]
fn the_hosts_barbarian_camps_reach_the_boards_camp_register() {
    let mut camp = plot(12, 10, "TERRAIN_GRASS");
    camp.im = Some("IMPROVEMENT_BARBARIAN_CAMP".to_string());
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(10, 10, "TERRAIN_GRASS"), camp],
    }]);
    let game = rebuild_game(&snapshot, 4, 7);
    let camp_pos = crate::hex::offset_to_axial(12, 10);
    assert_eq!(
        game.map.tiles[&camp_pos].improvement.as_deref(),
        Some("barbarian_camp"),
        "the improvement is modelled"
    );
    assert!(
        game.barb_camps.contains_key(&camp_pos),
        "and the camp is in the register the home guard reads: {:?}",
        game.barb_camps
    );
    assert_eq!(game.barb_camps.len(), 1);

    // Cleared by the host: the next apply forgets it.
    let cleared = Snapshot::from_chunks(&[TilesChunk {
        turn: 41,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(10, 10, "TERRAIN_GRASS"), plot(12, 10, "TERRAIN_GRASS")],
    }]);
    let mut game = game;
    apply_terrain(&mut game, &cleared);
    assert!(
        game.barb_camps.is_empty(),
        "a camp the host cleared leaves the register: {:?}",
        game.barb_camps
    );
}

#[test]
fn frontier_access_never_turns_unknown_into_mock_land_or_water() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 4,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(10, 10, "TERRAIN_GRASS")],
    }]);
    let mut game = rebuild_game(&snapshot, 4, 7);
    grow_frontier(&mut game, &snapshot, 2);

    let center = crate::hex::offset_to_axial(10, 10);
    let frontier = crate::hex::neighbors(center)
        .into_iter()
        .find(|pos| game.map.tiles.contains_key(pos))
        .expect("the revealed center has an in-bounds neighbor");
    let far = crate::hex::offset_to_axial(0, 0);
    for (label, pos) in [("frontier", frontier), ("far", far)] {
        let tile = &game.map.tiles[&pos];
        assert_eq!(
            tile.terrain.as_str(),
            "unknown",
            "{label} undisclosed ground keeps its actual knowledge state"
        );
        assert!(game.rules.is_unknown(tile));
        assert_eq!(
            game.rules.tile_yields(tile),
            crate::rules::Yields::default(),
            "unknown ground cannot leak generated yields"
        );
        assert_eq!(
            serde_json::to_value(tile).unwrap()["terrain"],
            "unknown",
            "the serialized board exposes the unknown underneath"
        );
    }
    assert!(game.map.tiles[&frontier].assumed_traversable);
    assert!(game.rules.is_passable(&game.map.tiles[&frontier]));
    assert!(!game.map.tiles[&far].assumed_traversable);
    assert!(!game.rules.is_passable(&game.map.tiles[&far]));

    apply_terrain(&mut game, &snapshot);
    assert_eq!(game.map.tiles[&frontier].terrain.as_str(), "unknown");
    assert!(
        game.map.tiles[&frontier].assumed_traversable,
        "an authoritative terrain refresh must not erase the separately owned prior"
    );

    let warrior = game.spawn_test_unit("warrior", 0, center);
    let galley = game.spawn_test_unit("galley", 0, center);
    assert!(
        game.unit_can_traverse(warrior, frontier),
        "land explorers may probe the terrain-neutral frontier"
    );
    assert!(
        game.unit_can_traverse(galley, frontier),
        "naval explorers may probe it without calling it water underneath"
    );

    let saved = serde_json::to_string(&game).expect("the mirror game saves");
    let loaded: crate::game::Game = serde_json::from_str(&saved).expect("the mirror game reloads");
    assert!(loaded.rules.is_unknown(&loaded.map.tiles[&frontier]));
    assert!(loaded.map.tiles[&frontier].assumed_traversable);

    grow_frontier(&mut game, &snapshot, 0);
    assert_eq!(game.map.tiles[&frontier].terrain.as_str(), "unknown");
    assert!(!game.map.tiles[&frontier].assumed_traversable);
    assert!(!game.unit_can_traverse(warrior, frontier));
    assert!(!game.unit_can_traverse(galley, frontier));
}

/// ★★★★★ A coast revealed to the horizon walled the fleet in. The land
/// prior is grown from revealed land and stops at every revealed tile, so
/// the fog beyond a city's three rings of charted water was reached from
/// nothing: no ship could plan toward it, and the naval recon arm read the
/// sea as finished. Live run `civvis-20260818T225716Z`: t169, Ostia coastal
/// since t44, Cartography in hand, no hull ever built, 559 of 3404 plots
/// seen. The sea now grows its own prior from revealed water; ships read
/// it, the land army does not, and the arm sees water left to chart.
#[test]
fn the_fog_beyond_charted_water_is_the_seas_frontier() {
    let center = crate::hex::offset_to_axial(10, 10);
    let mut plots = vec![plot(10, 10, "TERRAIN_GRASS")];
    for y in 0..20 {
        for x in 0..20 {
            let d = crate::hex::distance(crate::hex::offset_to_axial(x, y), center);
            if (1..=3).contains(&d) {
                plots.push(plot(x, y, "TERRAIN_COAST"));
            }
        }
    }
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 44,
        width: 20,
        height: 20,
        chunk: 1,
        plots,
    }]);
    let mut game = rebuild_game(&snapshot, 4, 7);
    game.players[0].techs.insert(crate::name!("sailing"));
    apply_explored(&mut game, &snapshot);
    let city = game.place_city(0, center, None);
    assert!(crate::ai::BasicAi::empire_is_coastal(&game, 0));

    // Before the sea prior existed this is where every live game stood: no
    // frontier at the water's edge, so nothing anywhere for a ship to seek.
    assert!(
        !crate::ai::BasicAi::unseen_water_remains(&game, 0),
        "a bare reconstruction has no sea frontier"
    );

    grow_frontier(&mut game, &snapshot, 2);
    let mut fog_by_ring: std::collections::BTreeMap<i32, Vec<crate::Pos>> = Default::default();
    for (pos, tile) in &game.map.tiles {
        if game.rules.is_unknown(tile) {
            fog_by_ring
                .entry(crate::hex::distance(*pos, center))
                .or_default()
                .push(*pos);
        }
    }
    // Two rings beyond the charted water carry the sea prior; the land
    // prior reaches nothing, because every neighbour of the island is
    // revealed water and the growth never crosses revealed ground.
    for ring in [4, 5] {
        for pos in &fog_by_ring[&ring] {
            let tile = &game.map.tiles[pos];
            assert!(
                tile.assumed_navigable,
                "ring {ring} {pos:?} is the sea's frontier"
            );
            assert!(
                !tile.assumed_traversable,
                "ring {ring} {pos:?} is not land's"
            );
            assert!(!game.rules.is_passable(tile), "the land prior is untouched");
        }
    }
    for pos in &fog_by_ring[&6] {
        assert!(
            !game.map.tiles[pos].assumed_navigable,
            "depth 2 stops at ring 5"
        );
    }
    let edge = fog_by_ring[&4][0];
    let shore = game
        .nbrs(edge)
        .into_iter()
        .find(|pos| game.rules.is_water(&game.map.tiles[pos]))
        .expect("ring 4 touches the charted coast");
    let galley = game.spawn_test_unit("galley", 0, shore);
    let warrior = game.spawn_test_unit("warrior", 0, shore);
    assert!(
        game.unit_can_traverse(galley, edge),
        "a ship may plan toward the fog beyond charted water"
    );
    assert!(
        !game.unit_can_traverse(warrior, edge),
        "the land army may not — `come_ashore` keeps it dry, and fog with no \
             domain must not smuggle it back to sea"
    );

    // And the arm that buys the empire's naval eye sees water left to chart.
    assert!(crate::ai::BasicAi::unseen_water_remains(&game, 0));
    assert!(
        crate::ai::BasicAi::naval_recon_ship_can_chart(&game, 0, galley),
        "the galley on the shore can chart from where it stands"
    );
    game.remove_unit(galley);
    let mut ai = crate::ai::BasicAi::default();
    ai.enable_naval_recon();
    assert!(
        ai.naval_recon_is_the_missing_arm(&game, 0),
        "with no hull afloat and fog past the coast, the sea scout is the missing arm"
    );
    assert!(
        ai.best_naval_recon(&game, 0, city).is_some(),
        "the coastal city can lay the hull down"
    );

    // A refresh of the authoritative terrain keeps the separately owned
    // prior; a save round-trips it; depth 0 clears it.
    apply_terrain(&mut game, &snapshot);
    assert!(game.map.tiles[&edge].assumed_navigable);
    let saved = serde_json::to_string(&game).expect("the mirror game saves");
    let loaded: crate::game::Game = serde_json::from_str(&saved).expect("the mirror game reloads");
    assert!(loaded.map.tiles[&edge].assumed_navigable);
    grow_frontier(&mut game, &snapshot, 0);
    assert!(!game.map.tiles[&edge].assumed_navigable);
    assert!(!crate::ai::BasicAi::unseen_water_remains(&game, 0));
    assert!(!ai.naval_recon_is_the_missing_arm(&game, 0));
}

#[test]
fn a_revealed_but_untranslatable_terrain_is_still_unknown_underneath() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 4,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_FROM_A_MOD")],
    }]);
    let game = rebuild_game(&snapshot, 4, 7);
    let tile = &game.map.tiles[&crate::hex::offset_to_axial(5, 5)];
    assert_eq!(tile.terrain.as_str(), "unknown");
    assert!(!tile.assumed_traversable);
    assert!(game.rules.is_unknown(tile));
}

/// ★★★★★ The seat must know which ground it has seen, or every adjacent tile
/// looks like a frontier and the explorer shuffles in place.
#[test]
fn the_seat_knows_which_ground_it_has_seen() {
    let chunks = vec![TilesChunk {
        turn: 4,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
    }];
    let snapshot = Snapshot::from_chunks(&chunks);
    let game = rebuild_game(&snapshot, 4, 7);
    let explored = &game.players[0].explored;

    // ⚠ This assertion is the one that corrected me. Before the fix this read 35,
    // not 0: `Game::new` generates a CIVVIS map and reveals a start on it, so the
    // set was populated with plots around a capital the real seat has never been
    // near. `apply_explored` must REPLACE that, not extend it.
    assert_eq!(
        explored.len(),
        2,
        "exactly the two plots the mod exported — the generated map's invented \
             start reveal must be gone, not merged with"
    );
    assert!(explored.contains(&crate::hex::offset_to_axial(5, 5)));
    assert!(explored.contains(&crate::hex::offset_to_axial(5, 6)));
    assert!(
        !explored.contains(&crate::hex::offset_to_axial(15, 15)),
        "ground the seat has never seen must not read as explored"
    );

    // Traversability remains a separate frontier policy; merely being unknown is
    // not enough to make a tile reachable.
    assert!(
        game.map.tiles.keys().any(|pos| !explored.contains(pos)),
        "the seat must not believe it has seen the whole world"
    );
}

/// ★★★★★ Explored ground the seat cannot currently see must still be ON THE BOARD.
///
/// This is the operator's report — *"civvis sometimes only shows current
/// visibility… area has been explored that isn't in civvis map"* — reduced to its
/// mechanism. `obs.rs` walks `explored` and, for a tile that is not currently
/// visible, looks it up in `remembered_tiles` inside a `filter_map`; a tile with no
/// memory is therefore **dropped from the board**, not dimmed. Nothing in this
/// bridge wrote `remembered_tiles`, so before the fix the seated observation of a
/// charted continent contained **zero** tiles.
///
/// ⚠ Asserted through `observation_player_view`, not against `remembered_tiles`
/// directly: the mirror window attaches with `POST /view {"player": 0}` and that is
/// the surface that was empty. A test that only counted the memory map would pass on
/// a memory the viewer never consults.
#[test]
fn ground_the_seat_has_charted_survives_the_fog_closing_over_it() {
    let plots: Vec<Plot> = (0..6).map(|x| plot(5 + x, 5, "TERRAIN_GRASS")).collect();
    let revealed = plots.len();
    let chunks = vec![TilesChunk {
        turn: 40,
        width: 20,
        height: 20,
        chunk: 1,
        plots,
    }];
    let snapshot = Snapshot::from_chunks(&chunks);
    let game = rebuild_game(&snapshot, 4, 7);

    let seat = &game.players[0];
    assert_eq!(
        seat.explored.len(),
        revealed,
        "the export is the explored set"
    );
    assert_eq!(
        seat.remembered_tiles.len(),
        revealed,
        "and memory must cover it exactly — never more (invented ground from the \
             generated map) and never less (a hole the viewer drops)"
    );

    // ⚠ The test is only meaningful if some charted ground is genuinely under fog.
    // Asserted rather than assumed: `Game::new` reveals a generated start, so which
    // plots the seat can see is a property of the map roll, not of this fixture.
    let visible = game.player_visibility(0);
    let fogged: Vec<crate::Pos> = seat
        .explored
        .iter()
        .filter(|pos| !visible.contains(pos))
        .copied()
        .collect();
    assert!(
        !fogged.is_empty(),
        "no charted plot is under fog, so this fixture cannot exercise the defect"
    );

    let view = crate::obs::observation_player_view(&game, 0);
    let tiles = view["map"]["tiles"].as_array().expect("a board of tiles");
    let on_board: std::collections::BTreeSet<crate::Pos> = tiles
        .iter()
        .filter_map(|tile| {
            let pos = tile["pos"].as_array()?;
            Some((pos[0].as_i64()? as i32, pos[1].as_i64()? as i32))
        })
        .collect();
    assert_eq!(
        tiles.len(),
        revealed,
        "every charted plot must still be on the board once the fog closes over it \
             — before the fix only the currently-visible ones survived, which is the \
             whole defect"
    );
    for pos in &fogged {
        assert!(
            on_board.contains(pos),
            "remembered ground {pos:?} was dropped from the board entirely"
        );
    }

    // And it must arrive as REMEMBERED, not as currently seen. A mirror that
    // reported stale ground as live would be the opposite error, and just as wrong.
    let live: std::collections::BTreeSet<crate::Pos> = view["visible"]
        .as_array()
        .expect("a visible set")
        .iter()
        .filter_map(|pos| {
            let pos = pos.as_array()?;
            Some((pos[0].as_i64()? as i32, pos[1].as_i64()? as i32))
        })
        .collect();
    for pos in &fogged {
        assert!(
            !live.contains(pos),
            "fogged ground {pos:?} must not be reported as currently visible"
        );
    }
}

/// ★★★★★ The board's rivers must be Civilization VI's, and ONLY Civilization VI's.
///
/// The generated map `Game::new` builds has its own river network, and nothing used
/// to remove it — so "does the board have rivers" answered yes while every one of
/// them was invented. Both halves are asserted here: the exported segment lands on
/// the right edge of the right tile, **and** no other tile carries a river at all.
/// The second assertion is the one that fails without `clear_rivers`.
#[test]
fn the_rivers_on_the_board_are_the_ones_civ6_exported() {
    let mut wet = plot(5, 6, "TERRAIN_GRASS");
    // W and NE, so the mapping is pinned on two different edges rather than one
    // that might be right by luck.
    wet.rv = 1 | 4;
    let chunks = vec![TilesChunk {
        turn: 8,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![
            wet,
            plot(4, 6, "TERRAIN_GRASS"),
            plot(5, 5, "TERRAIN_GRASS"),
            plot(6, 6, "TERRAIN_GRASS"),
        ],
    }];
    let snapshot = Snapshot::from_chunks(&chunks);
    let game = rebuild_game(&snapshot, 4, 7);

    // ⚠⚠ THE ASSERTION THAT MATTERS IS ABOUT THE NEIGHBOUR, NOT THIS PLOT.
    //
    // `IsWOfRiver` means the plot lies WEST OF the river, so the river is on its
    // EAST edge; `IsNEOfRiver` means it lies NORTH-EAST of the river, so the river
    // is on its SOUTH-WEST edge. The first version of this read the flags as
    // "river on the west/north-east edge" and put every segment on the opposite
    // side of the hex.
    //
    // It passed anyway, because `set_river_edge` marks both tiles sharing a
    // segment: the plot that reported the flag came out riverside under either
    // reading, and only the neighbour differed. So this now names the neighbour
    // explicitly, and asserts the OPPOSITE edges carry nothing.
    let pos = crate::hex::offset_to_axial(5, 6);
    let edge = |d: usize| (pos.0 + crate::hex::DIRS[d].0, pos.1 + crate::hex::DIRS[d].1);
    let (east, south_west) = (edge(0), edge(2));
    let (west, north_east) = (edge(3), edge(5));
    assert!(
        game.map.has_river_edge(pos, east),
        "W of the river means the river is on this plot's EAST edge"
    );
    assert!(
        game.map.has_river_edge(pos, south_west),
        "NE of the river means the river is on this plot's SOUTH-WEST edge"
    );
    assert!(
        !game.map.has_river_edge(pos, west),
        "and NOT on the western edge — that is the reading this test exists to \
             rule out, and it is invisible to any check of this plot alone"
    );
    assert!(
        !game.map.has_river_edge(pos, north_east),
        "nor on the north-eastern edge"
    );
    // Written from both sides, so the two tiles cannot disagree about one segment.
    assert!(
        game.map.has_river_edge(east, pos),
        "and the neighbour must carry the same segment"
    );

    assert!(
        game.map.get(pos).is_some_and(|tile| tile.has_river()),
        "the plot itself reads as riverside"
    );

    // ⚠ The assertion that fails without `clear_rivers`. Before this fix the
    // generated world's network survived here: 33 invented river tiles on a live
    // run, only 36.4% of them on ground Civilization VI even calls fresh water.
    let riverside: Vec<crate::Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| tile.has_river())
        .map(|(pos, _)| *pos)
        .collect();
    assert_eq!(
        riverside.len(),
        3,
        "exactly the exporting plot and the two neighbours across its segments \
             carry a river — every other river on this board was invented by the map \
             generator, and found at {riverside:?}"
    );
}

#[test]
fn a_known_river_edge_survives_when_its_firaxis_holder_is_hidden() {
    let mut wet = plot(5, 6, "TERRAIN_GRASS");
    wet.rv = 8;
    wet.ri = true;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 8,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![wet],
    }]);
    let game = rebuild_game(&snapshot, 4, 7);
    let pos = crate::hex::offset_to_axial(5, 6);
    let west = (pos.0 + crate::hex::DIRS[3].0, pos.1 + crate::hex::DIRS[3].1);
    assert!(game.map.has_river_edge(pos, west));
    assert!(game.map.tiles[&pos].has_river());
}

/// ★★★★ A card the host has retired must stop being offered.
///
/// `POLICY_ILKUM` was chosen and refused **105 times** on live run
/// `civvis-20260801T012454Z` — Civilization VI answered `IsPolicyObsolete` every
/// time and said so in the refusal reason, and nothing read it.
///
/// ⚠ Asserted through `available_policies`, not against `blocked_policies`. That
/// is the single chokepoint the AI, the observation and `legal_actions` all pass
/// through, and a test that only checked the field would pass on a set nothing
/// consults — which is exactly how a populated-but-inert value survives here.
#[test]
fn a_card_the_host_retired_stops_being_offered() {
    let mut game = crate::game::Game::new(4, 20, 20, 7, 500, 0);
    // A fresh seat has no civics, so nothing is unlocked yet. Craftsmanship is
    // what the ruleset's own policy test uses to put cards in hand.
    game.players[0].civics.insert(Name::new("craftsmanship"));
    let offered = game.available_policies(0);
    let victim = offered
        .first()
        .cloned()
        .expect("craftsmanship must put at least one card on offer");
    assert!(
        game.available_policies(0).contains(&victim),
        "precondition: the card is on offer before the host retires it"
    );

    Arc::make_mut(&mut game.blocked_policies).insert(victim);
    assert!(
        !game.available_policies(0).contains(&victim),
        "a card the host ruleset retired must not be offered again — this is the \
             105 ILKUM refusals"
    );
    assert_eq!(
        game.available_policies(0).len(),
        offered.len() - 1,
        "and only that card is withdrawn; blocking one must not empty the hand"
    );
}

/// The retired cards are already in the stream — no new mod event was needed.
#[test]
fn the_hosts_retired_cards_are_read_from_the_refusals_it_already_writes() {
    let dir = std::env::temp_dir().join(format!("civvis-policy-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("events.jsonl");
    // Shaped exactly like the live stream: reasons keyed in `refusals`, repeated
    // across turns, mixed with reasons that are not policies at all.
    std::fs::write(
        &path,
        concat!(
            r#"{"kind":"orders","turn":40,"refusals":{"obsolete_POLICY_ILKUM":1,"MOVE_TO":4}}"#,
            "\n",
            r#"{"kind":"orders","turn":41,"refusals":{"obsolete_POLICY_ILKUM":1,"no_params":2}}"#,
            "\n",
            r#"{"kind":"orders","turn":42,"refusals":{"obsolete_POLICY_NOT_A_REAL_CARD":1}}"#,
            "\n",
        ),
    )
    .expect("write events");

    let names = refused_policies(&path);
    assert!(
        names.contains("POLICY_ILKUM"),
        "the reason the agent already writes is the whole source"
    );
    assert_eq!(
        names.len(),
        2,
        "each distinct card once, however many turns it spans"
    );

    let rules = crate::rules::Rules::embedded();
    let blocked = blocked_policies_from(&names, &rules);
    assert!(
        blocked.contains(&Name::new("ilkum")),
        "and it translates through the shipped policy table"
    );
    assert_eq!(
        blocked.len(),
        1,
        "a card CIVVIS does not model is DROPPED, not inserted under a name that \
             matches nothing — a blocked set full of unmatched names looks populated \
             and filters nothing"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A pantheon a rival holds is already in the stream as `taken_BELIEF_<X>`
/// — the mod's `pantheon` handler writes it when `IsInSomePantheon` says so
/// — and until now nothing read it: the mirror seats no rival pantheons, so
/// the same first choice was re-derived from the same board next turn and,
/// after two sightings, the mod's blocker fallback chose the first untaken
/// belief in database order. See `Game::blocked_pantheons` and
/// `AdvancedAi::expansion_pantheon`.
#[test]
fn the_hosts_taken_pantheons_are_read_from_the_refusals_it_already_writes() {
    let dir = std::env::temp_dir().join(format!("civvis-pantheon-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("events.jsonl");
    std::fs::write(
        &path,
        concat!(
            r#"{"kind":"orders","turn":18,"refusals":{"taken_BELIEF_RELIGIOUS_SETTLEMENTS":1,"MOVE_TO":4}}"#,
            "
",
            r#"{"kind":"orders","turn":19,"refusals":{"taken_BELIEF_RELIGIOUS_SETTLEMENTS":1,"taken_NOT_A_BELIEF":1}}"#,
            "
",
            r#"{"kind":"orders","turn":20,"refusals":{"taken_BELIEF_NOT_A_REAL_PANTHEON":1,"pantheon_already_founded":1}}"#,
            "
",
            r#"{"kind":"orders","turn":40,"refusals":{"taken_BELIEF_FERTILITY_RITES":1}}"#,
            "
",
        ),
    )
    .expect("write events");

    let names = refused_pantheons(&path);
    assert!(names.contains("BELIEF_RELIGIOUS_SETTLEMENTS"));
    assert!(names.contains("BELIEF_FERTILITY_RITES"));
    assert_eq!(
        names.len(),
        3,
        "each distinct belief once, however many turns it spans; a `taken_` reason              that is not a belief is not one: {names:?}"
    );
    // Bounded by turn, the way every per-turn state read asks for it.
    assert!(
        !refused_pantheons_through(&path, Some(30)).contains("BELIEF_FERTILITY_RITES"),
        "a refusal on turn 40 is not known on turn 30"
    );

    let rules = crate::rules::Rules::embedded();
    let blocked = blocked_pantheons_from(&names, &rules);
    assert!(blocked.contains(&Name::new("religious_settlements")));
    assert!(blocked.contains(&Name::new("fertility_rites")));
    assert_eq!(
        blocked.len(),
        2,
        "a belief CIVVIS does not model is DROPPED, not inserted under a name that              matches nothing"
    );

    // And the board refuses what the host refused, so the chooser moves on.
    let mut game = crate::game::Game::new_full(2, 30, 18, 6_101, 200, 0, false);
    game.current = 0;
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &crate::game::Action::FoundCity { unit: settler })
        .unwrap();
    game.players[0].faith = 200.0;
    game.blocked_pantheons = Arc::new(blocked);
    assert!(game
        .apply(
            0,
            &crate::game::Action::ChoosePantheon {
                belief: Name::new("religious_settlements"),
            },
        )
        .is_err());
    assert!(game
        .apply(
            0,
            &crate::game::Action::ChoosePantheon {
                belief: Name::new("divine_spark"),
            },
        )
        .is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

/// ★★★★ The wonder half of `build_no_plot` was being DROPPED ON THE FLOOR.
///
/// The mod emits a refused district under the event's `district` key and a
/// refused WONDER under `building`. The parser read only `district`, so every
/// wonder refusal fell straight through it and nothing ever reached the planner.
/// Measured over 20 live runs: **370 wonder refusals against 55 district ones**,
/// from 29 distinct (run, city, wonder) combinations — a mean of 12.8 re-asks
/// each, and 53 consecutive turns at worst of one city ordering one wonder
/// Civilization VI had no ground for.
///
/// ⚠ Two-sided on purpose: the district side must keep working, and neither key
/// may leak into the other's set.
#[test]
fn a_refused_wonder_is_read_from_the_building_key_not_the_district_key() {
    let dir = std::env::temp_dir().join(format!("civvis-noplot-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("events.jsonl");
    // Shaped exactly like the live stream, including the repeat that made this
    // worth fixing and a bare-hash export that names nothing.
    std::fs::write(
        &path,
        concat!(
            r#"{"kind":"build_no_plot","turn":40,"city":65536,"building":"BUILDING_HANGING_GARDENS","x":8,"y":9}"#,
            "\n",
            r#"{"kind":"build_no_plot","turn":41,"city":65536,"building":"BUILDING_HANGING_GARDENS","x":8,"y":9}"#,
            "\n",
            r#"{"kind":"build_no_plot","turn":42,"city":196610,"district":"DISTRICT_THEATER","x":3,"y":4}"#,
            "\n",
            r#"{"kind":"build_no_plot","turn":43,"city":65536,"building":"-1743686858","x":8,"y":9}"#,
            "\n",
        ),
    )
    .expect("write events");

    let wonders = refused_wonders_through(&path, None);
    assert_eq!(
        wonders.get(&65536).map(|set| set.len()),
        Some(1),
        "each distinct wonder once, however many turns it spans"
    );
    assert!(wonders[&65536].contains("BUILDING_HANGING_GARDENS"));
    assert!(
        !wonders.contains_key(&196610),
        "a refused DISTRICT must not appear in the wonder set"
    );

    let districts = refused_districts_through(&path, None);
    assert!(
        districts[&196610].contains("DISTRICT_THEATER"),
        "the district side must keep working"
    );
    assert!(
        !districts.contains_key(&65536),
        "a refused WONDER must not appear in the district set"
    );

    // And it translates through the shipped wonder table, dropping the bare hash
    // rather than inserting a name that matches nothing.
    let rules = crate::rules::Rules::embedded();
    let city_ids: std::collections::BTreeMap<u32, i64> = [(7u32, 65536i64)].into_iter().collect();
    let blocked = blocked_wonders_from(&wonders, &city_ids, &rules);
    assert_eq!(
        blocked.get(&7).map(|set| set.len()),
        Some(1),
        "a wonder CIVVIS does not model is DROPPED, not inserted under a name \
             that matches nothing"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// ★★★★ A refused DISTRICT must cool down like every other production refusal.
///
/// `blocked_production_from` has always had a `DISTRICT_` fallback and
/// `production_block_key` has always emitted `district:{name}`, but
/// `refused_production` accepted only `UNIT_`/`BUILDING_`/`PROJECT_`, so no
/// district name ever reached either and that branch was dead code.
///
/// The prefix list predicted the cooldown exactly. Over 20 live runs, gaps
/// between successive refusals of the same (run, city, item): every accepted
/// prefix had **zero** gaps of one turn, and `DISTRICT_` had **13 of them and
/// none of eight or more** — `DISTRICT_HOLY_SITE` re-proposed in one city on
/// turns 45 through 58, every consecutive turn, against a TTL of eight.
///
/// ⚠ Asserts the whole chain, not the prefix list: a filter that admits the name
/// but a translator that drops it would leave the block empty and still pass a
/// test written against the parser alone.
#[test]
fn a_refused_district_cools_down_like_every_other_production_refusal() {
    let dir = std::env::temp_dir().join(format!("civvis-prodref-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("events.jsonl");
    std::fs::write(
        &path,
        concat!(
            r#"{"kind":"civvis_build_unplayable","turn":45,"city":65536,"item":"DISTRICT_HOLY_SITE","reasons":[]}"#,
            "\n",
            r#"{"kind":"civvis_build_unplayable","turn":46,"city":65536,"item":"UNIT_SPY","reasons":[]}"#,
            "\n",
            r#"{"kind":"civvis_build_unplayable","turn":30,"city":65536,"item":"DISTRICT_CAMPUS","reasons":[]}"#,
            "\n",
        ),
    )
    .expect("write events");

    // Turn 50: the t45 and t46 refusals are inside the eight-turn window, the
    // t30 one is not.
    let refused = refused_production(&path, 50);
    let names = refused.get(&65536).expect("the city has recent refusals");
    assert!(
        names.contains("DISTRICT_HOLY_SITE"),
        "a district refusal must be carried like any other"
    );
    assert!(
        names.contains("UNIT_SPY"),
        "and the kinds that already worked must keep working"
    );
    assert!(
        !names.contains("DISTRICT_CAMPUS"),
        "the TTL still applies — an old refusal is not a permanent ban"
    );

    // ⚠ The half that was dead code: the name has to survive translation into a
    // key `Game::can_produce` actually checks.
    let rules = crate::rules::Rules::embedded();
    let city_ids: std::collections::BTreeMap<u32, i64> = [(7u32, 65536i64)].into_iter().collect();
    let blocked = blocked_production_from(&refused, &city_ids, &rules);
    let keys = blocked.get(&7).expect("translated block for the city");
    assert!(
        keys.contains("district:holy_site"),
        "translated to the same key production_block_key emits, got {keys:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trade_route_refusals_are_merged_through_the_requested_host_turn() {
    let dir = std::env::temp_dir().join(format!("civvis-route-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("events.jsonl");
    // Each pairing is refused twice: this test is about the TURN LIMIT,
    // and a single refusal no longer condemns anything (see
    // `TRADE_ROUTE_REFUSALS_BEFORE_BLOCK`).
    std::fs::write(
        &path,
        concat!(
            r#"{"kind":"state","turn":41}"#,
            "\n",
            r#"{"kind":"trade_route_refused","turn":39,"unit":9,"from_x":6,"from_y":6,"x":9,"y":9}"#,
            "\n",
            r#"{"kind":"trade_route_refused","turn":40,"unit":9,"from_x":6,"from_y":6,"x":9,"y":9}"#,
            "\n",
            r#"{"kind":"trade_route_refused","turn":42,"unit":9,"from_x":6,"from_y":6,"x":10,"y":10}"#,
            "\n",
            r#"{"kind":"trade_route_refused","turn":43,"unit":9,"from_x":6,"from_y":6,"x":10,"y":10}"#,
            "\n",
        ),
    )
    .expect("write events");

    let state = state_from_events(&path, Some(41)).expect("turn 41 state");
    assert_eq!(
        state.refused_trade_routes,
        std::collections::BTreeSet::from([(
            crate::hex::offset_to_axial(6, 6),
            crate::hex::offset_to_axial(9, 9),
        )]),
        "future refusals must not leak into an earlier reconstructed frame"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Three Traders parked in Rome, and the ledger that put them there.
///
/// Live run `civvis-20260822T020434Z` ended with a trade capacity of 20,
/// only 16 routes running, and four idle Traders. Its refusal ledger holds
/// 23 distinct pairings, **every one refused exactly once**, and 8 of the
/// 15 condemned destinations are our OWN cities. `blocked_trade_routes` is
/// never cleared, so each of those single readings retired a pairing for
/// the rest of the game and the parked Traders were never offered another.
#[test]
fn one_trade_route_refusal_is_a_report_and_two_are_a_verdict() {
    let dir = std::env::temp_dir().join(format!("civvis-route2-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("events.jsonl");
    let refusal = |turn: u32, x: i32, y: i32| {
        format!(
            r#"{{"kind":"trade_route_refused","turn":{turn},"unit":9,"from_x":6,"from_y":6,"x":{x},"y":{y}}}"#
        )
    };
    std::fs::write(
        &path,
        [
            // Two state anchors: a frame can only be reconstructed at a
            // turn the run actually exported one for.
            r#"{"kind":"state","turn":45}"#.to_string(),
            r#"{"kind":"state","turn":60}"#.to_string(),
            // Refused once, exactly like all 23 pairings in the live run.
            refusal(40, 9, 9),
            // Refused twice: the host has said it twice and means it.
            refusal(41, 12, 12),
            refusal(50, 12, 12),
        ]
        .join("\n")
            + "\n",
    )
    .expect("write events");

    let state = state_from_events(&path, Some(60)).expect("turn 60 state");
    assert_eq!(
        state.refused_trade_routes,
        std::collections::BTreeSet::from([(
            crate::hex::offset_to_axial(6, 6),
            crate::hex::offset_to_axial(12, 12),
        )]),
        "only the corroborated pairing is retired; retiring is forever"
    );

    // And the corroboration must fall inside the reconstructed frame: a
    // second refusal from the future cannot condemn a pairing early.
    let earlier = state_from_events(&path, Some(45)).expect("turn 45 state");
    assert!(
        earlier.refused_trade_routes.is_empty(),
        "the second reading is at turn 50 and this frame is turn 45"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// ★★★★ Landmass identity comes from the export, and invented cliffs come off.
///
/// Same defect as the rivers above, two fields over. On the live board 200 of 776
/// tiles carried a continent and 576 carried none — the generated world's regions
/// showing through on a map where every land plot really has one.
#[test]
fn the_landmass_is_civ6s_and_the_generated_cliffs_are_gone() {
    let mut home = plot(5, 5, "TERRAIN_GRASS");
    home.ct = Some("CONTINENT_AFRICA".to_string());
    home.cl = 2;
    let mut away = plot(9, 9, "TERRAIN_GRASS");
    away.ct = Some("CONTINENT_ASIA".to_string());
    let mut beside = plot(5, 6, "TERRAIN_GRASS");
    beside.ct = Some("CONTINENT_AFRICA".to_string());
    // Water: Civilization VI gives it no continent, so neither may CIVVIS.
    let sea = plot(6, 5, "TERRAIN_OCEAN");

    let chunks = vec![TilesChunk {
        turn: 12,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![home, away, beside, sea],
    }];
    let snapshot = Snapshot::from_chunks(&chunks);
    let game = rebuild_game(&snapshot, 4, 7);

    let at = |x, y| game.map.get(crate::hex::offset_to_axial(x, y)).unwrap();
    assert_eq!(
        at(5, 5).continent,
        at(5, 6).continent,
        "two plots Civilization VI puts on one continent must agree"
    );
    assert_ne!(
        at(5, 5).continent,
        at(9, 9).continent,
        "and two it separates must not — 'another continent' is a rule"
    );
    assert_eq!(
        at(6, 5).continent,
        None,
        "water carries no continent, and must LOSE the generated one rather than \
             keep it"
    );
    assert_eq!(at(5, 5).coastal_lowland, 2, "the flood band crosses");

    // ⚠ The assertion that fails without the clear: 66 invented cliffs on the live
    // board, each able to block embarkation at a shore the real game lets a unit
    // leave from.
    let cliffs = game
        .map
        .tiles
        .values()
        .filter(|tile| tile.cliff_edges.iter().any(|edge| *edge))
        .count();
    assert_eq!(
        cliffs, 0,
        "Civilization VI exposes no cliff accessor, so a cliff on this board can \
             only have been invented by the map generator"
    );
}

#[test]
fn civ6_encodes_hills_in_the_terrain_and_civvis_does_not() {
    let vocab = Vocabulary::embedded();
    assert_eq!(
        vocab.terrain("TERRAIN_GRASS"),
        Resolved::Known((Name::new("grassland"), false))
    );
    assert_eq!(
        vocab.terrain("TERRAIN_GRASS_HILLS"),
        Resolved::Known((Name::new("grassland"), true))
    );
    // A mountain is its own CIVVIS terrain rather than an elevated one.
    assert_eq!(
        vocab.terrain("TERRAIN_GRASS_MOUNTAIN"),
        Resolved::Known((Name::new("mountain"), false))
    );
}

#[test]
fn wonders_whose_two_names_disagree_still_resolve() {
    // These are the pairings that made the first coverage report read 74%:
    // Civ 6 names a wonder by type id, CIVVIS by its common name.
    let vocab = Vocabulary::embedded();
    for (civ6, civvis) in [
        ("FEATURE_DEVILSTOWER", "mato_tipila"),
        ("FEATURE_WHITEDESERT", "sahara_el_beyda"),
        ("FEATURE_CLIFFS_DOVER", "cliffs_of_dover"),
        ("FEATURE_IKKIL", "ik_kil"),
        ("FEATURE_BARRIER_REEF", "great_barrier_reef"),
    ] {
        assert_eq!(
            vocab.feature(civ6),
            Resolved::Known(Name::new(civvis)),
            "{civ6} must resolve"
        );
    }
}

#[test]
fn a_deliberate_exclusion_is_not_the_same_answer_as_a_failure() {
    let vocab = Vocabulary::embedded();
    match vocab.resource("RESOURCE_LEY_LINE") {
        Resolved::Excluded(why) => assert!(
            why.contains("Secret Societies"),
            "the exclusion must carry its reason, got {why:?}"
        ),
        other => panic!("ley line should be excluded, got {other:?}"),
    }
    // And something genuinely absent must be Unknown, never a default.
    assert_eq!(
        vocab.terrain("TERRAIN_INVENTED_BY_NOBODY"),
        Resolved::Unknown("TERRAIN_INVENTED_BY_NOBODY".to_string())
    );
}

#[test]
fn unrevealed_ground_stays_a_hole() {
    // ⚠ The information constraint, made executable. The mod sends only
    // revealed plots; anything absent must read as unknown rather than as
    // whatever a map generator would have put there.
    let chunks = vec![TilesChunk {
        turn: 40,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![plot(1, 1, "TERRAIN_GRASS"), plot(1, 2, "TERRAIN_PLAINS")],
    }];
    let snapshot = Snapshot::from_chunks(&chunks);
    assert!(snapshot.is_revealed((1, 1)));
    assert!(!snapshot.is_revealed((5, 5)), "never exported, so unknown");
    assert!(snapshot.plot((5, 5)).is_none());
    assert_eq!(snapshot.revealed_count(), 2);
    // 2 of 100: a ranking computed from this is a very different claim from
    // one computed at 90%, and the caller can see which it has.
    assert!((snapshot.revealed_fraction() - 0.02).abs() < 1e-9);
}

#[test]
fn chunks_reassemble_and_a_re_export_refreshes_rather_than_duplicates() {
    let chunks = vec![
        TilesChunk {
            turn: 40,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![plot(0, 0, "TERRAIN_GRASS")],
        },
        TilesChunk {
            turn: 40,
            width: 8,
            height: 8,
            chunk: 2,
            plots: vec![plot(1, 0, "TERRAIN_DESERT"), plot(0, 0, "TERRAIN_TUNDRA")],
        },
    ];
    let snapshot = Snapshot::from_chunks(&chunks);
    assert_eq!(snapshot.revealed_count(), 2, "one entry per plot");
    assert_eq!(
        snapshot.plot((0, 0)).unwrap().t.as_deref(),
        Some("TERRAIN_TUNDRA"),
        "the later chunk wins"
    );
    assert_eq!(snapshot.turn, 40);
}

#[test]
fn untranslatable_ground_is_reported_not_swallowed() {
    let chunks = vec![TilesChunk {
        turn: 1,
        width: 4,
        height: 4,
        chunk: 1,
        plots: vec![
            plot(0, 0, "TERRAIN_GRASS"),
            plot(1, 0, "TERRAIN_FROM_A_MOD"),
        ],
    }];
    let snapshot = Snapshot::from_chunks(&chunks);
    assert_eq!(
        snapshot.untranslatable(Vocabulary::embedded()),
        vec!["TERRAIN_FROM_A_MOD".to_string()],
        "a type the vocabulary cannot place must surface"
    );
}

#[test]
fn a_revealed_land_plot_becomes_land_and_can_hold_a_city() {
    // ⚠ Written because `rebuild_with_empire` refused to place the capital of a
    // real run on (56,28), a plot the export clearly recorded as TERRAIN_GRASS,
    // not water. Either the tile is not being written or the placement check is
    // wrong, and a test says which.
    let chunks = vec![TilesChunk {
        turn: 10,
        width: 60,
        height: 38,
        chunk: 1,
        plots: vec![Plot {
            x: 56,
            y: 28,
            im: None,
            t: Some("TERRAIN_GRASS".to_string()),
            f: None,
            r: None,
            o: 0,
            w: false,
            i: false,
            fw: Some(true),
            rv: 0,
            ri: false,
            ct: None,
            cl: -1,
            p: false,
            d: None,
            dc: None,
            wo: None,
            rt: None,
            rp: false,
            yl: None,
            ap: None,
            np: false,
            vis: false,
        }],
    }];
    let snapshot = Snapshot::from_chunks(&chunks);
    assert!(
        snapshot.is_revealed((56, 28)),
        "the plot must read as revealed"
    );

    let game = rebuild_game(&snapshot, 4, 1);

    let axial = crate::hex::offset_to_axial(56, 28);
    let tile = game
        .map
        .get(axial)
        .expect("the mirrored map must contain a plot the export described");
    assert_eq!(
        tile.terrain.as_str(),
        "grassland",
        "revealed grass must land as grassland, not the generated terrain"
    );
    assert!(
        !game.rules.is_water(tile),
        "a revealed grass plot must not read as water"
    );

    let (with_empire, placed) = rebuild_with_empire(&snapshot, &[(56, 28)], 4, 1);
    assert_eq!(placed, 1, "the capital must be placeable on its own plot");
    assert!(
        with_empire.cities.values().any(|c| c.pos == axial),
        "and the city must actually be there, at the AXIAL position"
    );
}

#[test]
fn the_real_export_shape_deserializes() {
    // Field-for-field what CivvisControlAgent.lua emits, so a rename on
    // either side fails here rather than in a live game.
    let raw = r#"{
            "turn": 25, "width": 44, "height": 26, "chunk": 1,
            "plots": [
                {"x":3,"y":4,"t":"TERRAIN_GRASS_HILLS","f":"FEATURE_FOREST",
                 "r":"RESOURCE_DEER","o":0,"w":false,"i":false,"fw":true},
                {"x":4,"y":4,"t":"TERRAIN_COAST","o":-1,"w":true,"i":false,"fw":false}
            ]
        }"#;
    let chunk: TilesChunk = serde_json::from_str(raw).expect("export shape parses");
    assert_eq!(chunk.plots.len(), 2);
    let hill = &chunk.plots[0];
    assert_eq!(hill.o, 0);
    assert_eq!(hill.fw, Some(true), "fresh water carries through");
    let vocab = Vocabulary::embedded();
    assert_eq!(
        vocab.terrain(hill.t.as_deref().unwrap()),
        Resolved::Known((Name::new("grassland"), true))
    );
    // A plot with no feature or resource omits them rather than sending 0,
    // which would otherwise be read as a real type.
    assert!(chunk.plots[1].f.is_none() && chunk.plots[1].r.is_none());
}

#[test]
fn new_export_fields_are_reported_instead_of_silently_discarded() {
    let raw = r#"{
            "kind":"state", "ctx":"Gameplay", "run":"contract", "turn":7,
            "t":1788296253, "utc":"2026-09-01T20:57:33.415Z",
            "cities":[{
                "id":1, "x":2, "y":3, "pantheon_active":"BELIEF_CITY_PATRON_GODDESS",
                "producing_hash":123, "future_city_fact":9,
                "districts":[{"type":"DISTRICT_CAMPUS","x":2,"y":4,"pillaged":false}],
                "wonders":[{"type":"BUILDING_PYRAMIDS","x":1,"y":3}]
            }],
            "units":[{"id":4,"kind":"UNIT_WARRIOR","x":2,"y":3,"combat":20,
                      "ranged":0,"player":0,"formation_count":2,"xp":19,"level":2,
                      "promotions":["PROMOTION_BATTLECRY"],"build_charges":0,
                      "spread_charges":0}],
            "future_empire_fact":true
        }"#;
    let state = state_from_json(raw).expect("the state remains usable");
    assert_eq!(state.cities[0].producing_hash, Some(123));
    assert_eq!(state.units[0].combat, 20.0);
    assert_eq!(state.units[0].formation_count, 2);
    assert_eq!(state.units[0].xp, Some(19));
    assert_eq!(state.units[0].level, Some(2));
    assert_eq!(
        state.units[0].promotions.as_deref(),
        Some(["PROMOTION_BATTLECRY".to_string()].as_slice())
    );
    assert_eq!(
        state.schema_gaps,
        vec![
            "schema:city.future_city_fact".to_string(),
            "schema:state.future_empire_fact".to_string(),
        ],
        "recognized metadata and diagnostic fields stay quiet; every new fact is named"
    );

    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 7,
        width: 6,
        height: 6,
        chunk: 1,
        plots: vec![plot(2, 3, "TERRAIN_GRASS")],
    }]);
    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    assert!(recon
        .unmapped
        .contains(&"schema:city.future_city_fact".to_string()));
    assert!(recon
        .unmapped
        .contains(&"schema:state.future_empire_fact".to_string()));
}

#[test]
fn a_civ6_seat_rebuilds_with_its_setup_rules_and_ui_settings() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 8,
        width: 4,
        height: 4,
        chunk: 1,
        plots: vec![plot(1, 1, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 8,
        ..StateSnapshot::default()
    };
    state.seat.speed = "GAMESPEED_ONLINE".to_string();
    state.seat.difficulty = "DIFFICULTY_SETTLER".to_string();
    state.seat.map = "Continents.lua".to_string();

    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);

    assert_eq!(
        civvis_game_speed("GAMESPEED_ONLINE"),
        Some(GameSpeed::Online)
    );
    assert_eq!(
        civvis_difficulty("DIFFICULTY_SETTLER"),
        Some("settler".to_string())
    );
    assert_eq!(
        civvis_map_script("Continents.lua"),
        Some(MapScript::Continents)
    );
    assert_eq!(recon.game.game_speed, GameSpeed::Online);
    assert_eq!(recon.game.speed, "online");
    assert_eq!(recon.game.difficulty, "settler");
    assert_eq!(recon.game.map_script, MapScript::Continents);
}

#[test]
fn rival_identity_follows_the_compacted_mirror_seat() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 20,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS")],
    }]);
    let state = StateSnapshot {
        turn: 20,
        rivals: vec![StateRival {
            player: 3,
            civ: "CIVILIZATION_SCYTHIA".to_string(),
            leader: "LEADER_TOMYRIS".to_string(),
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };

    let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
    assert_eq!(
        recon.game.players[1].civ, "Scythia",
        "the first exported rival owns compacted CIVVIS seat 1"
    );
    assert_eq!(
        recon.game.observed_leader_types.get(&1).map(String::as_str),
        Some("LEADER_TOMYRIS")
    );
    let observed = crate::obs::observation_spectator(&recon.game, 0);
    assert_eq!(
        observed["players"][1]["leader"],
        serde_json::json!("Tomyris")
    );
    assert_eq!(
        observed["players"][1]["leader_type"],
        serde_json::json!("LEADER_TOMYRIS")
    );
    assert_ne!(
        recon.game.players[3].civ, "Scythia",
        "Firaxis player id 3 is translation metadata, not the CIVVIS entity owner"
    );
}

#[test]
fn host_routes_land_on_the_engines_ladder() {
    assert_eq!(route_level(None, false), 0);
    assert_eq!(route_level(Some(""), false), 0);
    assert_eq!(route_level(Some("ROUTE_ANCIENT_ROAD"), false), 1);
    assert_eq!(route_level(Some("ROUTE_MEDIEVAL_ROAD"), false), 2);
    assert_eq!(route_level(Some("ROUTE_INDUSTRIAL_ROAD"), false), 3);
    assert_eq!(route_level(Some("ROUTE_MODERN_ROAD"), false), 4);
    assert_eq!(route_level(Some("ROUTE_RAILROAD"), false), 5);
    // A route the ladder does not name is still a road; a pillaged one pays nothing.
    assert_eq!(route_level(Some("ROUTE_SOMETHING_NEW"), false), 1);
    assert_eq!(route_level(Some("ROUTE_MEDIEVAL_ROAD"), true), 0);
}

/// Roads were never exported and the board wrote `road = 0` everywhere;
/// a plot that names its route now carries it, and an older export
/// without `rt` still reads roadless.
#[test]
fn exported_routes_reach_the_board() {
    let mut roaded = plot(3, 3, "TERRAIN_GRASS");
    roaded.rt = Some("ROUTE_MEDIEVAL_ROAD".to_string());
    let mut pillaged = plot(4, 3, "TERRAIN_GRASS");
    pillaged.rt = Some("ROUTE_ANCIENT_ROAD".to_string());
    pillaged.rp = true;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 8,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![roaded, pillaged, plot(5, 3, "TERRAIN_GRASS")],
    }]);
    let state = StateSnapshot {
        turn: 8,
        ..StateSnapshot::default()
    };
    let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let at = |x, y| mirror.game.map.tiles[&crate::hex::offset_to_axial(x, y)].road;
    assert_eq!(at(3, 3), 2, "a medieval road on the engine's ladder");
    assert_eq!(at(4, 3), 0, "a pillaged road pays no movement");
    assert_eq!(at(5, 3), 0, "no route, no road");
}

/// The export's `moves` is trusted only when the seat says the mod reads
/// it at the start of the turn and keeps the host from spending it first;
/// otherwise every unit keeps its full allowance exactly as before.
#[test]
fn exported_movement_is_trusted_only_with_the_seat_capability() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 8,
        width: 8,
        height: 8,
        chunk: 1,
        plots: (0..8)
            .flat_map(|x| (0..8).map(move |y| plot(x, y, "TERRAIN_GRASS")))
            .collect(),
    }]);
    let units = || {
        vec![
            StateUnit {
                id: 11,
                kind: "UNIT_WARRIOR".to_string(),
                x: 3,
                y: 3,
                moves: 0.0,
                ..StateUnit::default()
            },
            StateUnit {
                id: 12,
                kind: "UNIT_WARRIOR".to_string(),
                x: 4,
                y: 3,
                moves: 2.0,
                ..StateUnit::default()
            },
            StateUnit {
                id: 13,
                kind: "UNIT_WARRIOR".to_string(),
                x: 5,
                y: 3,
                moves: -1.0,
                ..StateUnit::default()
            },
        ]
    };
    let plain = StateSnapshot {
        turn: 8,
        units: units(),
        ..StateSnapshot::default()
    };
    let mirror = LiveMirror::new(&snapshot, &plain, 4, 1, 250, 0);
    for civ6 in [11, 12, 13] {
        let uid = mirror.uid_of[&civ6];
        assert_eq!(
            mirror.game.units[&uid].moves_left,
            mirror.game.unit_max_moves(uid),
            "without the capability unit {civ6} keeps its full allowance"
        );
    }
    assert_eq!(mirror.units_short_of_movement(), 0);

    let trusted = StateSnapshot {
        turn: 8,
        units: units(),
        seat: Seat {
            moves_at_turn_start: true,
            ..Seat::default()
        },
        ..StateSnapshot::default()
    };
    let mut mirror = LiveMirror::new(&snapshot, &trusted, 4, 1, 250, 0);
    let spent = mirror.uid_of[&11];
    let fresh = mirror.uid_of[&12];
    let unreported = mirror.uid_of[&13];
    assert_eq!(
        mirror.game.units[&spent].moves_left, 0.0,
        "the host already walked it"
    );
    assert_eq!(
        mirror.game.units[&fresh].moves_left,
        mirror.game.unit_max_moves(fresh)
    );
    assert_eq!(
        mirror.game.units[&unreported].moves_left,
        mirror.game.unit_max_moves(unreported),
        "a negative export is 'not reported', not zero"
    );
    assert_eq!(mirror.units_short_of_movement(), 1);

    // The persistent path (`sync`) reads the same truth each turn.
    let mut next = trusted;
    next.turn = 9;
    next.units[0].moves = 2.0;
    next.units[1].moves = 1.0;
    mirror.sync(&snapshot, &next, 0);
    assert_eq!(
        mirror.game.units[&spent].moves_left,
        mirror.game.unit_max_moves(spent)
    );
    assert_eq!(mirror.game.units[&fresh].moves_left, 1.0);
}

/// ★★★ The host's own upgrade verdict crosses per unit (docs/FIDELITY.md,
/// "The one-to-one map", item 9): a named successor and bill price the
/// lane, a named block silences it whatever the board's rules would say,
/// and an export without the keys leaves the board's own rule in charge.
/// On both paths.
#[test]
fn the_hosts_upgrade_verdict_prices_the_lane_and_a_named_block_silences_it() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 8,
        height: 8,
        chunk: 1,
        plots: (0..8)
            .flat_map(|x| (0..8).map(move |y| plot(x, y, "TERRAIN_GRASS")))
            .collect(),
    }]);
    let warrior = |to: Option<&str>, cost: Option<f64>, blocked: Option<&str>| StateUnit {
        id: 31,
        kind: "UNIT_WARRIOR".to_string(),
        x: 3,
        y: 3,
        moves: 2.0,
        upgrade_to: to.map(str::to_string),
        upgrade_cost: cost,
        upgrade_blocked_reason: blocked.map(str::to_string),
        ..StateUnit::default()
    };
    let state = |turn: u32, unit: StateUnit| StateSnapshot {
        turn,
        gold: 300,
        units: vec![unit],
        ..StateSnapshot::default()
    };
    let upgrade = |mirror: &LiveMirror| {
        let uid = mirror.uid_of[&31];
        let offer = mirror
            .game
            .unit_gold_upgrade_detail(0, uid)
            .map(|(to, gold, _)| (to.to_string(), gold));
        let lane = mirror.game.legal_unit_upgrade_actions(0).iter().any(
            |action| matches!(action, crate::game::Action::UpgradeUnit { unit } if *unit == uid),
        );
        (offer, lane)
    };

    // No key: the board's own rule, which refuses a unit on unowned ground.
    let mut bare = LiveMirror::new(
        &snapshot,
        &state(40, warrior(None, None, None)),
        4,
        1,
        250,
        0,
    );
    assert_eq!(upgrade(&bare), (Err("neutral territory"), false));

    // The host names the successor and the bill: the lane carries both.
    let priced = |turn| state(turn, warrior(Some("UNIT_SWORDSMAN"), Some(120.0), None));
    let fresh = LiveMirror::new(&snapshot, &priced(40), 4, 1, 250, 0);
    assert_eq!(
        upgrade(&fresh),
        (Ok(("swordsman".to_string(), 120.0)), true)
    );

    // A named block is final, on the board's own refusal vocabulary.
    let blocked = |turn| {
        state(
            turn,
            warrior(
                Some("UNIT_SWORDSMAN"),
                Some(120.0),
                Some("LOC_UNITCOMMAND_UPGRADE_NOT_ENOUGH_GOLD"),
            ),
        )
    };
    let held = LiveMirror::new(&snapshot, &blocked(40), 4, 1, 250, 0);
    assert_eq!(upgrade(&held), (Err("not enough gold"), false));

    // The persistent path reads the same keys off each later export, and
    // an export that drops them hands the decision back to the board.
    bare.sync(&snapshot, &priced(41), 0);
    assert_eq!(upgrade(&bare), (Ok(("swordsman".to_string(), 120.0)), true));
    bare.sync(&snapshot, &blocked(42), 0);
    assert_eq!(upgrade(&bare), (Err("not enough gold"), false));
    bare.sync(&snapshot, &state(43, warrior(None, None, None)), 0);
    assert_eq!(upgrade(&bare), (Err("neutral territory"), false));
}

/// ★★★ A Spy on a host operation is busy on the board too, and the host's
/// own menu bounds what the board may hand an idle one (item 9). Until
/// this crossed every seat 0 Spy was re-seated idle on every sync and
/// `SPY_GAIN_SOURCES` was refused 195 of 862 times on one run.
#[test]
fn a_spy_on_a_host_operation_is_not_retasked_and_the_hosts_menu_bounds_the_boards() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 120,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![
            plot(3, 3, "TERRAIN_GRASS"),
            plot(4, 4, "TERRAIN_GRASS"),
            plot(5, 5, "TERRAIN_GRASS"),
        ],
    }]);
    let spy = |id: i64, x: i32, y: i32| StateUnit {
        id,
        kind: "UNIT_SPY".to_string(),
        x,
        y,
        level: Some(1),
        // A current mod exports the upkeep for every unit, so the entry
        // exists and an absent operation is a real "idle".
        maintenance: Some(4.0),
        ..StateUnit::default()
    };
    let state = |turn: u32, units: Vec<StateUnit>| {
        let mut state = StateSnapshot {
            turn,
            spy_capacity: Some(3),
            units,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Roma".to_string(),
            x: 4,
            y: 4,
            pop: 4,
            ..StateCity::default()
        });
        state.rivals.push(StateRival {
            player: 1,
            cities: vec![StateCity {
                id: 9,
                name: "Aduatuca".to_string(),
                x: 5,
                y: 5,
                pop: 6,
                ..StateCity::default()
            }],
            ..StateRival::default()
        });
        state
    };
    let missions = |game: &crate::game::Game, spy: u32| -> Vec<String> {
        game.legal_spy_actions(0, spy)
            .into_iter()
            .filter_map(|action| match action {
                crate::game::Action::SpyMission { mission, .. } => Some(mission),
                _ => None,
            })
            .collect()
    };

    // Idle abroad with no menu: the board's own offers for a rival city.
    let mut mirror = LiveMirror::new(&snapshot, &state(120, vec![spy(78, 5, 5)]), 2, 1, 250, 0);
    let uid = mirror.uid_of[&78];
    assert!(mirror.game.spies[&uid].mission.is_none());
    let offered = missions(&mirror.game, uid);
    assert!(
        offered.iter().any(|m| m == "gain_sources")
            && offered.iter().any(|m| m == "listening_post"),
        "the board's own menu: {offered:?}"
    );

    // The host's menu names one operation: only that one is offered.
    let mut bounded = spy(78, 5, 5);
    bounded.spy_missions_available = Some(vec!["UNITOPERATION_SPY_LISTENING_POST".to_string()]);
    mirror.sync(&snapshot, &state(121, vec![bounded]), 0);
    let uid = mirror.uid_of[&78];
    assert_eq!(
        missions(&mirror.game, uid),
        vec!["listening_post".to_string()]
    );

    // On an operation: the mission is seated and nothing is offered, on
    // both paths.
    let mut busy = spy(78, 5, 5);
    busy.spy_operation = Some("UNITOPERATION_SPY_GAIN_SOURCES".to_string());
    busy.spy_operation_end_turn = Some(128);
    mirror.sync(&snapshot, &state(122, vec![busy.clone()]), 0);
    let uid = mirror.uid_of[&78];
    let mission = mirror.game.spies[&uid]
        .mission
        .clone()
        .expect("the host's operation seats as the mission");
    assert_eq!((mission.kind.as_str(), mission.ends), ("gain_sources", 128));
    assert_eq!(
        mirror.game.cities[&mission.city].owner, 1,
        "aimed at the rival city it stands in"
    );
    assert!(
        mirror.game.legal_spy_actions(0, uid).is_empty(),
        "a busy Spy is offered nothing"
    );
    let rebuilt = rebuild_from_state(&snapshot, &state(122, vec![busy]), 2, 1, 250, 0);
    let uid = rebuilt
        .unit_ids
        .iter()
        .find(|(_, host)| **host == 78)
        .map(|(uid, _)| *uid)
        .expect("the spy is mirrored");
    assert_eq!(
        rebuilt.game.spies[&uid]
            .mission
            .as_ref()
            .map(|m| m.kind.as_str()),
        Some("gain_sources")
    );
    assert!(rebuilt.game.legal_spy_actions(0, uid).is_empty());

    // And the operation ending hands the Spy back to the board's menu.
    mirror.sync(&snapshot, &state(129, vec![spy(78, 5, 5)]), 0);
    let uid = mirror.uid_of[&78];
    assert!(mirror.game.spies[&uid].mission.is_none());
    assert!(!missions(&mirror.game, uid).is_empty());
}

/// ★★★ The treasury's bill by source and the unit's own upkeep replace the
/// board's sums when they cross, and the host's `GetMaxMoves` is the
/// allowance the board plans with (item 9). Absent, the board bills and
/// moves itself exactly as before.
#[test]
fn the_hosts_bill_by_source_and_the_units_own_upkeep_replace_the_boards_sums() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 12,
        width: 8,
        height: 8,
        chunk: 1,
        plots: (0..8)
            .flat_map(|x| (0..8).map(move |y| plot(x, y, "TERRAIN_GRASS")))
            .collect(),
    }]);
    let archer = |maintenance: Option<f64>, max_moves: Option<f64>| StateUnit {
        id: 41,
        kind: "UNIT_ARCHER".to_string(),
        x: 3,
        y: 3,
        moves: 2.0,
        maintenance,
        max_moves,
        ..StateUnit::default()
    };
    let state = |turn: u32, unit: StateUnit, totals: Option<(f64, f64, f64)>| StateSnapshot {
        turn,
        units: vec![unit],
        unit_maintenance_total: totals.map(|bill| bill.0),
        building_maintenance_total: totals.map(|bill| bill.1),
        district_maintenance_total: totals.map(|bill| bill.2),
        ..StateSnapshot::default()
    };

    let bare = LiveMirror::new(
        &snapshot,
        &state(12, archer(None, None), None),
        4,
        1,
        250,
        0,
    );
    let uid = bare.uid_of[&41];
    let ruleset = bare.game.rules.units["archer"].maintenance;
    assert!(ruleset > 0.0, "an Archer costs upkeep in the ruleset");
    assert_eq!(bare.game.unit_gold_maintenance(0), ruleset);
    assert_eq!(bare.game.infrastructure_gold_maintenance(0), 0.0);
    assert_eq!(bare.game.unit_max_moves(uid), 2.0);
    assert_eq!(bare.game.units[&uid].moves_left, 2.0);

    // The unit's own upkeep and allowance, in place of the ruleset's.
    let own = LiveMirror::new(
        &snapshot,
        &state(12, archer(Some(3.0), Some(3.0)), None),
        4,
        1,
        250,
        0,
    );
    let uid = own.uid_of[&41];
    assert_eq!(own.game.unit_gold_maintenance(0), 3.0);
    assert_eq!(own.game.unit_max_moves(uid), 3.0);
    assert_eq!(
        own.game.units[&uid].moves_left, 3.0,
        "the allowance the board plans with"
    );

    // The treasury's totals outrank the per-unit sum, on both paths.
    let mut carried = bare;
    carried.sync(
        &snapshot,
        &state(13, archer(Some(3.0), None), Some((12.0, 5.0, 2.0))),
        0,
    );
    assert_eq!(carried.game.unit_gold_maintenance(0), 12.0);
    assert_eq!(carried.game.infrastructure_gold_maintenance(0), 7.0);
    let rebuilt = rebuild_from_state(
        &snapshot,
        &state(13, archer(None, None), Some((12.0, 5.0, 2.0))),
        4,
        1,
        250,
        0,
    );
    assert_eq!(rebuilt.game.unit_gold_maintenance(0), 12.0);
    assert_eq!(rebuilt.game.infrastructure_gold_maintenance(0), 7.0);
    // And a later export without them hands the bill back to the board.
    carried.sync(&snapshot, &state(14, archer(None, None), None), 0);
    assert_eq!(carried.game.unit_gold_maintenance(0), ruleset);
    let uid = carried.uid_of[&41];
    assert_eq!(carried.game.unit_max_moves(uid), 2.0);
}

/// On a mid-turn combat frame the host says how many strikes a unit has
/// left; a unit that already struck must not be planned to strike again.
/// Trusted under the same seat capability as movement, on both paths.
#[test]
fn attacks_remaining_reach_the_board_with_the_seat_capability() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 8,
        width: 8,
        height: 8,
        chunk: 1,
        plots: (0..8)
            .flat_map(|x| (0..8).map(move |y| plot(x, y, "TERRAIN_GRASS")))
            .collect(),
    }]);
    let units = |attacks: Option<i32>| {
        vec![StateUnit {
            id: 21,
            kind: "UNIT_ARCHER".to_string(),
            x: 3,
            y: 3,
            moves: 1.0,
            attacks_remaining: attacks,
            ..StateUnit::default()
        }]
    };
    let plain = StateSnapshot {
        turn: 8,
        frame: 1,
        units: units(Some(0)),
        ..StateSnapshot::default()
    };
    let mirror = LiveMirror::new(&snapshot, &plain, 4, 1, 250, 0);
    let uid = mirror.uid_of[&21];
    assert_eq!(
        mirror.game.units[&uid].attacks_left, 1,
        "no capability: the fresh-turn allowance"
    );

    let trusted = StateSnapshot {
        turn: 8,
        frame: 1,
        units: units(Some(0)),
        seat: Seat {
            moves_at_turn_start: true,
            ..Seat::default()
        },
        ..StateSnapshot::default()
    };
    let mut mirror = LiveMirror::new(&snapshot, &trusted, 4, 1, 250, 0);
    let uid = mirror.uid_of[&21];
    assert_eq!(
        mirror.game.units[&uid].attacks_left, 0,
        "the host says it already struck"
    );
    let mut next = trusted;
    next.turn = 9;
    next.frame = 0;
    next.units = units(Some(1));
    mirror.sync(&snapshot, &next, 0);
    assert_eq!(mirror.game.units[&uid].attacks_left, 1);
    next.units = units(None);
    mirror.sync(&snapshot, &next, 0);
    assert_eq!(
        mirror.game.units[&uid].attacks_left, 1,
        "an older export means the allowance"
    );
}

#[test]
fn firaxis_babylon_pack_suffix_is_not_a_second_civilization() {
    assert_eq!(civvis_civ_name("CIVILIZATION_BABYLON_STK"), Some("Babylon"));
    assert_eq!(civvis_civ_name("CIVILIZATION_OTTOMAN"), Some("Ottomans"));
}

#[test]
fn active_research_and_civic_progress_follow_the_live_export() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 8,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 8,
        research: Some("TECH_MINING".to_string()),
        research_progress: 7.5,
        civic: Some("CIVIC_CODE_OF_LAWS".to_string()),
        civic_progress: 3.0,
        ..StateSnapshot::default()
    };

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    assert_eq!(mirror.game.players[0].research.as_deref(), Some("mining"));
    assert_eq!(mirror.game.players[0].research_progress, 7.5);
    assert_eq!(
        mirror.game.players[0].civic.as_deref(),
        Some("code_of_laws")
    );
    assert_eq!(mirror.game.players[0].civic_progress, 3.0);

    state.turn = 9;
    state.research = Some("TECH_ANIMAL_HUSBANDRY".to_string());
    state.research_progress = 11.0;
    state.civic = Some("CIVIC_FOREIGN_TRADE".to_string());
    state.civic_progress = 5.0;
    mirror.sync(&snapshot, &state, 0);

    assert_eq!(
        mirror.game.players[0].research.as_deref(),
        Some("animal_husbandry")
    );
    assert_eq!(mirror.game.players[0].research_progress, 11.0);
    assert_eq!(
        mirror.game.players[0].civic.as_deref(),
        Some("foreign_trade")
    );
    assert_eq!(mirror.game.players[0].civic_progress, 5.0);
}

#[test]
fn public_rival_military_score_survives_rebuild_and_sync() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 40,
        rivals: vec![StateRival {
            player: 3,
            military: 670.0,
            score: 926,
            at_war: true,
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    assert_eq!(
        mirror
            .game
            .units
            .values()
            .filter(|unit| unit.owner == 1)
            .count(),
        0,
        "the rival army is under fog, so no tactical units may be invented"
    );
    assert_eq!(
        mirror.game.military_power(1),
        670.0,
        "the aggregate score is public information and must still drive strategy"
    );
    assert_eq!(mirror.game.score(1), 926);

    let saved = serde_json::to_string(&mirror.game).expect("save mirrored game");
    let loaded: crate::game::Game = serde_json::from_str(&saved).expect("load mirrored game");
    assert_eq!(loaded.military_power(1), 670.0);
    assert_eq!(loaded.score(1), 926);

    state.turn = 41;
    state.rivals[0].military = 342.0;
    state.rivals[0].score = 542;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(
        mirror.game.military_power(1),
        342.0,
        "persistent sync must refresh the score rather than freezing the rebuild value"
    );
    assert_eq!(mirror.game.score(1), 542);
}

/// ★★★★ THE OTHER CIVILIZATIONS' ECONOMIES ARE THE HOST'S FIGURES, NOT A GUESS.
///
/// The standings' rival Science and Culture were CIVVIS's own derivation from
/// whichever rival cities happened to be visible — usually none. The host reads
/// them for every player (as its World Rankings screen does), and now so does
/// the mirror: per-turn Science/Culture/Faith as the seat's own kind of
/// delta, treasury and banked Faith directly, refreshed by every sync.
#[test]
fn rival_economy_reaches_the_rival_seat_and_survives_sync() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 40,
        rivals: vec![StateRival {
            player: 3,
            military: 100.0,
            score: 200,
            science: 41.5,
            culture: 23.25,
            gold: 512.0,
            gold_per_turn: -3.0,
            faith: 88.0,
            faith_per_turn: f64::NAN,
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let seat_yields = |game: &crate::game::Game| {
        let mut total = crate::rules::Yields::default();
        for cid in game.player_city_ids(1) {
            total.add(game.city_yields(cid));
        }
        if let Some(adjustment) = game.observed_yield_adjustments.get(&1) {
            total.add(*adjustment);
        }
        total
    };
    let yields = seat_yields(&mirror.game);
    assert!((yields.science - 41.5).abs() < 1e-9, "{yields:?}");
    assert!((yields.culture - 23.25).abs() < 1e-9);
    assert_eq!(mirror.game.players[1].gold, 512.0);
    assert_eq!(mirror.game.players[1].gold_per_turn, -3.0);
    assert_eq!(mirror.game.players[1].faith, 88.0);

    state.turn = 41;
    state.rivals[0].science = 44.0;
    state.rivals[0].gold = 530.0;
    mirror.sync(&snapshot, &state, 0);
    assert!((seat_yields(&mirror.game).science - 44.0).abs() < 1e-9);
    assert_eq!(mirror.game.players[1].gold, 530.0);

    // An older export (NaN) or a refused read (-1) leaves the model's own
    // derivation alone rather than zeroing the seat. The struct literal's
    // derived Default is zero for a scalar, so make the absent Faith rate
    // explicit too.
    state.turn = 42;
    state.rivals[0].science = -1.0;
    state.rivals[0].culture = f64::NAN;
    state.rivals[0].faith_per_turn = f64::NAN;
    state.rivals[0].gold = -1.0;
    mirror.sync(&snapshot, &state, 0);
    assert!(mirror.game.observed_yield_adjustments.get(&1).is_none());
    assert_eq!(mirror.game.players[1].gold, 530.0);
}

/// The player HUD must use the host's public empire totals for every
/// civilization, even when fog deliberately leaves the rival with no
/// reconstructed city or unit records.
#[test]
fn public_empire_hud_totals_reach_every_civilization_and_refresh() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 40,
        science: Some(12.0),
        culture: Some(9.0),
        faith_per_turn: Some(7.0),
        gold: 75,
        gold_per_turn: Some(-4.0),
        faith: 44,
        score: 120,
        military: 80.0,
        government: Some("GOVERNMENT_MONARCHY".to_string()),
        dark_age: Some(false),
        golden_age: Some(true),
        heroic_golden_age: Some(false),
        public_stats: StatePublicEmpireStats {
            city_count: Some(4),
            population: Some(31),
            food: Some(48.0),
            production: Some(29.0),
            wonder_count: Some(2),
            suzerain_count: Some(1),
            nuclear_devices: Some(3),
            thermonuclear_devices: Some(2),
        },
        rivals: vec![StateRival {
            player: 3,
            military: 670.0,
            score: 926,
            techs: 53.0,
            civics: 44.0,
            science: 41.5,
            culture: 23.0,
            tourism: 61.0,
            gold: 512.0,
            gold_per_turn: -3.0,
            faith: 88.0,
            faith_per_turn: 19.0,
            government: Some("GOVERNMENT_FASCISM".to_string()),
            dark_age: Some(false),
            golden_age: Some(false),
            heroic_golden_age: Some(true),
            public_stats: StatePublicEmpireStats {
                city_count: Some(7),
                population: Some(49),
                food: Some(76.0),
                production: Some(43.0),
                wonder_count: Some(5),
                suzerain_count: Some(2),
                nuclear_devices: Some(4),
                thermonuclear_devices: Some(1),
            },
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    assert!(
        mirror.game.player_city_ids(1).is_empty(),
        "the aggregate must not fabricate fogged rival cities"
    );

    let observed = crate::obs::observation_spectator(&mirror.game, 0);
    let mine = &observed["players"][0];
    let rival = &observed["players"][1];
    assert_eq!(mine["cities"], serde_json::json!(4));
    assert_eq!(mine["population"], serde_json::json!(31));
    assert_eq!(mine["yields"]["food"], serde_json::json!(48.0));
    assert_eq!(mine["yields"]["production"], serde_json::json!(29.0));
    assert_eq!(mine["yields"]["science"], serde_json::json!(12.0));
    assert_eq!(mine["yields"]["culture"], serde_json::json!(9.0));
    assert_eq!(mine["yields"]["faith"], serde_json::json!(7.0));
    assert_eq!(mine["gold"], serde_json::json!(75.0));
    assert_eq!(mine["gold_per_turn"], serde_json::json!(-4.0));
    assert_eq!(mine["faith"], serde_json::json!(44.0));
    assert_eq!(mine["military"], serde_json::json!(80));
    assert_eq!(mine["score"], serde_json::json!(120));
    assert_eq!(mine["government"], serde_json::json!("monarchy"));
    assert_eq!(mine["age"], serde_json::json!("golden"));
    assert_eq!(mine["wonder_count"], serde_json::json!(2));
    assert_eq!(mine["suzerain_count"], serde_json::json!(1));
    assert_eq!(mine["nuclear_devices"], serde_json::json!(3));
    assert_eq!(mine["thermonuclear_devices"], serde_json::json!(2));

    assert_eq!(rival["cities"], serde_json::json!(7));
    assert_eq!(rival["population"], serde_json::json!(49));
    assert_eq!(rival["yields"]["food"], serde_json::json!(76.0));
    assert_eq!(rival["yields"]["production"], serde_json::json!(43.0));
    assert_eq!(rival["yields"]["science"], serde_json::json!(41.5));
    assert_eq!(rival["yields"]["culture"], serde_json::json!(23.0));
    assert_eq!(rival["yields"]["faith"], serde_json::json!(19.0));
    assert_eq!(rival["gold"], serde_json::json!(512.0));
    assert_eq!(rival["gold_per_turn"], serde_json::json!(-3.0));
    assert_eq!(rival["faith"], serde_json::json!(88.0));
    assert_eq!(rival["military"], serde_json::json!(670));
    assert_eq!(rival["government"], serde_json::json!("fascism"));
    assert_eq!(rival["age"], serde_json::json!("heroic"));
    assert_eq!(rival["nuclear_devices"], serde_json::json!(4));
    assert_eq!(rival["thermonuclear_devices"], serde_json::json!(1));
    assert_eq!(rival["wonder_count"], serde_json::json!(5));
    assert_eq!(rival["suzerain_count"], serde_json::json!(2));
    assert_eq!(rival["score"], serde_json::json!(926));
    assert_eq!(rival["tourism_per_turn"], serde_json::json!(61.0));
    assert_eq!(
        rival["victories"]["science"]["techs"],
        serde_json::json!(53)
    );
    assert_eq!(
        rival["victories"]["culture"]["civics"],
        serde_json::json!(44)
    );

    let saved = serde_json::to_string(&mirror.game).expect("save mirrored game");
    let loaded: crate::game::Game = serde_json::from_str(&saved).expect("load mirrored game");
    assert_eq!(
        crate::obs::observation_spectator(&loaded, 0)["players"][1]["cities"],
        serde_json::json!(7),
        "the public totals survive a saved spectator frame"
    );
    assert_eq!(
        crate::obs::observation_spectator(&loaded, 0)["players"][1]["government"],
        serde_json::json!("fascism"),
        "a fogged rival's public government survives a saved spectator frame"
    );
    assert_eq!(
        crate::obs::observation_spectator(&loaded, 0)["players"][1]["age"],
        serde_json::json!("heroic"),
        "a fogged rival's public age survives a saved spectator frame"
    );

    state.turn = 41;
    state.public_stats.city_count = Some(5);
    state.public_stats.nuclear_devices = Some(0);
    state.rivals[0].public_stats.population = Some(55);
    state.rivals[0].public_stats.food = Some(80.0);
    state.rivals[0].public_stats.nuclear_devices = Some(0);
    state.rivals[0].faith_per_turn = 23.0;
    state.rivals[0].techs = 54.0;
    state.rivals[0].government = Some("GOVERNMENT_DEMOCRACY".to_string());
    state.rivals[0].heroic_golden_age = Some(false);
    state.rivals[0].golden_age = Some(false);
    state.rivals[0].dark_age = Some(true);
    mirror.sync(&snapshot, &state, 0);
    let refreshed = crate::obs::observation_spectator(&mirror.game, 0);
    assert_eq!(refreshed["players"][0]["cities"], serde_json::json!(5));
    assert_eq!(
        refreshed["players"][0]["nuclear_devices"],
        serde_json::json!(0)
    );
    assert_eq!(refreshed["players"][1]["population"], serde_json::json!(55));
    assert_eq!(
        refreshed["players"][1]["yields"]["food"],
        serde_json::json!(80.0)
    );
    assert_eq!(
        refreshed["players"][1]["yields"]["faith"],
        serde_json::json!(23.0)
    );
    assert_eq!(
        refreshed["players"][1]["nuclear_devices"],
        serde_json::json!(0)
    );
    assert_eq!(
        refreshed["players"][1]["government"],
        serde_json::json!("democracy")
    );
    assert_eq!(refreshed["players"][1]["age"], serde_json::json!("dark"));
    assert_eq!(
        refreshed["players"][1]["victories"]["science"]["techs"],
        serde_json::json!(54)
    );

    // All three explicit false flags mean Normal, while a missing field is
    // an older control mod and must not erase the last host observation.
    state.turn = 42;
    state.rivals[0].heroic_golden_age = Some(false);
    state.rivals[0].golden_age = Some(false);
    state.rivals[0].dark_age = Some(false);
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(
        crate::obs::observation_spectator(&mirror.game, 0)["players"][1]["age"],
        serde_json::json!("normal")
    );
    state.turn = 43;
    state.rivals[0].government = None;
    state.rivals[0].heroic_golden_age = None;
    state.rivals[0].golden_age = None;
    state.rivals[0].dark_age = None;
    mirror.sync(&snapshot, &state, 0);
    let old_export = crate::obs::observation_spectator(&mirror.game, 0);
    assert_eq!(
        old_export["players"][1]["government"],
        serde_json::json!("democracy")
    );
    assert_eq!(old_export["players"][1]["age"], serde_json::json!("normal"));
}

#[test]
fn public_empire_hud_fields_are_recognized_on_the_live_wire() {
    let state = state_from_json(
        r#"{
                "kind":"state", "turn":40,
                "public_stats":{"city_count":4,"population":31,"food":48.0,
                  "production":29.0,"wonder_count":2,"suzerain_count":1,
                  "nuclear_devices":3,"thermonuclear_devices":2},
                "rivals":[{"player":3,"government":"GOVERNMENT_FASCISM",
                  "dark_age":false,"golden_age":false,"heroic_golden_age":true,
                  "public_stats":{"city_count":7,"population":49,
                  "food":76.0,"production":43.0,"wonder_count":5,"suzerain_count":2,
                  "nuclear_devices":4,"thermonuclear_devices":1}}]
            }"#,
    )
    .expect("the live public standings wire parses");
    assert_eq!(state.public_stats.city_count, Some(4));
    assert_eq!(state.public_stats.thermonuclear_devices, Some(2));
    assert_eq!(state.rivals[0].public_stats.population, Some(49));
    assert_eq!(state.rivals[0].public_stats.wonder_count, Some(5));
    assert_eq!(
        state.rivals[0].government.as_deref(),
        Some("GOVERNMENT_FASCISM")
    );
    assert_eq!(state.rivals[0].dark_age, Some(false));
    assert_eq!(state.rivals[0].golden_age, Some(false));
    assert_eq!(state.rivals[0].heroic_golden_age, Some(true));
    assert!(
        state.schema_gaps.is_empty(),
        "recognized public standings must not become unmapped diagnostics: {:?}",
        state.schema_gaps
    );
}

#[test]
fn live_diplomatic_totals_reach_rebuild_and_sync_without_legacy_erasure() {
    // This is the wire shape currently produced by CivvisControlAgent.lua.
    // Civilization VI already knows all three values, but before this bridge
    // the reconstructed board silently treated each of them as zero.
    let raw = r#"{
            "kind":"state", "ctx":"Gameplay", "run":"contract", "turn":40,
            "dvp":3, "favor":92.5,
            "used_governments":["GOVERNMENT_CHIEFDOM", "GOVERNMENT_OLIGARCHY"],
            "rivals":[{"player":3, "dvp":18}]
        }"#;
    let mut state = state_from_json(raw).expect("the live diplomatic wire parses");
    assert_eq!(state.dvp, Some(3));
    assert_eq!(state.favor, Some(92.5));
    assert_eq!(state.rivals[0].dvp, Some(18));
    assert_eq!(
        state.used_governments,
        vec!["GOVERNMENT_CHIEFDOM", "GOVERNMENT_OLIGARCHY"],
        "government history is a recognized state field, not an unmapped diagnostic"
    );
    assert!(
        state.schema_gaps.is_empty(),
        "the three diplomatic facts and used_governments must be schema-recognized: {:?}",
        state.schema_gaps
    );

    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS")],
    }]);
    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    assert_eq!(rebuilt.game.players[0].dvp, 3);
    assert_eq!(rebuilt.game.players[0].diplomatic_favor, 92.5);
    assert_eq!(rebuilt.game.players[1].dvp, 18);

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    state.turn = 41;
    state.dvp = Some(4);
    state.favor = Some(11.0);
    state.rivals[0].dvp = Some(19);
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.players[0].dvp, 4);
    assert_eq!(mirror.game.players[0].diplomatic_favor, 11.0);
    assert_eq!(mirror.game.players[1].dvp, 19);

    // An already-loaded older control mod omits a new field. Omission means
    // unknown, not an authoritative zero that should erase live knowledge.
    state.turn = 42;
    state.dvp = None;
    state.favor = None;
    state.rivals[0].dvp = None;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.players[0].dvp, 4);
    assert_eq!(mirror.game.players[0].diplomatic_favor, 11.0);
    assert_eq!(mirror.game.players[1].dvp, 19);
}

#[test]
fn met_gated_rivals_keep_their_host_seats_when_another_player_is_missing() {
    let mut state = StateSnapshot {
        turn: 100,
        seat: Seat {
            local_player: 0,
            players: 6,
            civ: "CIVILIZATION_ROME".to_string(),
            ..Seat::default()
        },
        // Host player 3 is deliberately absent, as it is from the early
        // met-gated exports in the recorded diplomatic loss. Player 4 must
        // still land on model seat 4, not inherit seat 3 by array position.
        rivals: vec![
            StateRival {
                player: 1,
                civ: "CIVILIZATION_GREECE".to_string(),
                score: 101,
                military: 11.0,
                ..StateRival::default()
            },
            StateRival {
                player: 2,
                civ: "CIVILIZATION_INDIA".to_string(),
                score: 202,
                military: 22.0,
                ..StateRival::default()
            },
            StateRival {
                player: 4,
                civ: "CIVILIZATION_FRANCE".to_string(),
                score: 404,
                military: 44.0,
                cities: vec![StateCity {
                    id: 400,
                    name: "Paris".to_string(),
                    x: 7,
                    y: 7,
                    pop: 8,
                    capital: true,
                    ..StateCity::default()
                }],
                ..StateRival::default()
            },
        ],
        ..StateSnapshot::default()
    };
    let map = host_major_seat_map(&state, 6);
    assert_eq!(map.get(&0), Some(&0));
    assert_eq!(map.get(&1), Some(&1));
    assert_eq!(map.get(&2), Some(&2));
    assert_eq!(map.get(&3), Some(&3));
    assert_eq!(map.get(&4), Some(&4));
    assert_eq!(
        host_rival_for_seat(&state, 4, 6).map(|rival| rival.player),
        Some(4)
    );

    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 100,
        width: 12,
        height: 12,
        chunk: 1,
        plots: vec![plot(7, 7, "TERRAIN_GRASS")],
    }]);
    let rebuilt = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
    assert_eq!(rebuilt.game.players[1].civ, "Greece");
    assert_eq!(rebuilt.game.players[2].civ, "India");
    assert_eq!(rebuilt.game.players[4].civ, "France");
    assert_ne!(rebuilt.game.players[3].civ, "France");
    assert_eq!(rebuilt.game.players[4].dvp, 0);
    assert_eq!(rebuilt.game.player_city_ids(4).len(), 1);
    assert_eq!(
        rebuilt.game.cities[&rebuilt.game.player_city_ids(4)[0]].name,
        "Paris"
    );
    assert_eq!(rebuilt.game.observed_score.get(&4), Some(&404));
    assert_eq!(rebuilt.game.observed_score.get(&3), None);

    let mut mirror = LiveMirror::new(&snapshot, &state, 6, 1, 250, 0);
    assert_eq!(mirror.game.player_city_ids(4).len(), 1);
    assert_eq!(
        mirror.game.cities[&mirror.game.player_city_ids(4)[0]].owner,
        4
    );
    assert_eq!(mirror.game.observed_military_power.get(&4), Some(&44.0));
    state.rivals[2].score = 405;
    state.turn = 101;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.observed_score.get(&4), Some(&405));
    assert_eq!(mirror.game.observed_score.get(&3), None);
}

/// 🔴🔴🔴 The congress standing seats the majors this seat never met.
///
/// Replays `civvis-20260818T103630Z`, which lost a diplomatic victory at
/// turn 222 while leading on score by 213. Six majors, and the seat had met
/// exactly one of them: the per-turn rival export therefore topped out at
/// LAUTARO's 14 points (70% of the 20 needed), comfortably under the denial
/// alarm, while the congress table the seat votes from showed player 4
/// holding 22. `urgent_victory_threat` never fired once in 222 turns.
#[test]
fn congress_standing_seats_the_majors_this_seat_never_met() {
    let raw = r#"{
            "kind":"state", "ctx":"Gameplay", "run":"contract", "turn":221,
            "dvp":2, "favor":847.0,
            "congress_dvp":{"turn":221, "points":[
                {"player":0, "points":2}, {"player":1, "points":10},
                {"player":3, "points":14}, {"player":4, "points":22},
                {"player":5, "points":16}]},
            "rivals":[{"player":3, "dvp":14}]
        }"#;
    let mut state = state_from_json(raw).expect("the congress standing wire parses");
    assert!(
        state.schema_gaps.is_empty(),
        "congress_dvp must be schema-recognized: {:?}",
        state.schema_gaps
    );
    // The seat arrives as its own event rather than inside `state`.
    state.seat.local_player = 0;
    state.seat.players = 6;
    let congress = state.congress_dvp.as_ref().expect("the table parses");
    assert_eq!(congress.turn, Some(221));
    assert_eq!(congress.points.len(), 5);

    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 221,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS")],
    }]);
    let rebuilt = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
    // Seat 0 is ours and host player 3 is the met rival, so its stable model
    // seat is 3. The congress table fills the other host ids in place,
    // including the majors the seat has never met.
    assert_eq!(rebuilt.game.players[0].dvp, 2);
    assert_eq!(rebuilt.game.players[1].dvp, 10);
    assert_eq!(rebuilt.game.players[2].dvp, 0);
    assert_eq!(rebuilt.game.players[3].dvp, 14);
    assert_eq!(rebuilt.game.players[4].dvp, 22);
    assert_eq!(rebuilt.game.players[5].dvp, 16);
    assert_eq!(
        rebuilt
            .game
            .players
            .iter()
            .map(|player| player.dvp)
            .max()
            .unwrap_or(0),
        22,
        "the empire actually about to win must be visible somewhere on the board"
    );

    // The point of the plumbing: the denial alarm can now see the empire
    // that is one resolution from winning. Diplomatic progress is
    // `dvp * 5`, so 22 points reads as a finished race and 14 reads 70 --
    // under every bar in `urgent_victory_threat`, which is why the shipped
    // seat sat on 847 unspent Favor while the game ended.
    let planner = crate::ai::AdvancedAi::default();
    assert_eq!(planner.rival_pressure(&rebuilt.game, 4).1, 100);
    assert!(
        planner.denial_is_urgent(&rebuilt.game, 4),
        "a rival holding 22 of the 20 points needed is a terminal clock"
    );
    let blind = rebuild_from_state(
        &snapshot,
        &StateSnapshot {
            congress_dvp: None,
            ..state.clone()
        },
        6,
        1,
        250,
        0,
    );
    assert!(
        !planner.denial_is_urgent(&blind.game, 4),
        "and without the congress table it is exactly the silence that lost the game"
    );

    let mut mirror = LiveMirror::new(&snapshot, &state, 6, 1, 250, 0);
    assert_eq!(mirror.game.players[4].dvp, 22);
    // The met rival's live export stays authoritative even when it falls,
    // because `WC_RES_DIPLOVICTORY` option B takes two points away.
    state.turn = 222;
    state.rivals[0].dvp = Some(12);
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(
        mirror.game.players[3].dvp, 12,
        "a met rival's per-turn read outranks a congress table refreshed once a session"
    );
    assert_eq!(mirror.game.players[4].dvp, 22);

    // An older control mod omits the table entirely; that must not erase
    // what a persistent mirror already seated.
    state.turn = 223;
    state.congress_dvp = None;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.players[4].dvp, 22);
}

/// A met rival whose `dvp` the mod could not read still gets the congress
/// number rather than a silent zero.
#[test]
fn congress_standing_backfills_a_met_rival_with_no_live_reading() {
    let raw = r#"{
            "kind":"state", "ctx":"Gameplay", "run":"contract", "turn":180,
            "congress_dvp":{"turn":180, "points":[
                {"player":0, "points":5}, {"player":2, "points":17}]},
            "rivals":[{"player":2}]
        }"#;
    let mut state = state_from_json(raw).expect("the congress standing wire parses");
    state.seat.local_player = 0;
    state.seat.players = 4;
    assert_eq!(state.rivals[0].dvp, None);
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 180,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS")],
    }]);
    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    assert_eq!(rebuilt.game.players[2].dvp, 17);
}

/// Rival victory progress crosses the bridge. Five of the twelve runs the
/// seat was leading on 2026-08-16/17 ended at t229-245 on a rival's
/// culture, technology or diplomatic victory: rival space programs and
/// tourist counts never crossed, so the victory tracker read zero for
/// every rival on exactly the lanes that end games early.
#[test]
fn rival_victory_progress_reaches_rebuild_and_sync() {
    let raw = r#"{
            "kind":"state", "ctx":"Gameplay", "run":"contract", "turn":180,
            "rivals":[{"player":3,
                "science_projects":["PROJECT_LAUNCH_EARTH_SATELLITE",
                                     "PROJECT_LAUNCH_MOON_LANDING"],
                "foreign_tourists":41, "domestic_tourists":66}]
        }"#;
    let mut state = state_from_json(raw).expect("the rival progress wire parses");
    assert_eq!(state.rivals[0].foreign_tourists, 41.0);
    assert_eq!(state.rivals[0].domestic_tourists, 66.0);
    assert!(
        state.schema_gaps.is_empty(),
        "rival victory progress must be schema-recognized: {:?}",
        state.schema_gaps
    );

    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 180,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS")],
    }]);
    let expected = BTreeSet::from([
        "launch_earth_satellite".to_string(),
        "launch_moon_landing".to_string(),
    ]);
    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    assert_eq!(rebuilt.game.players[1].science_projects, expected);
    let observed = rebuilt
        .game
        .observed_public_empire_stats
        .get(&1)
        .expect("a rival with progress has observed stats");
    assert_eq!(observed.foreign_tourists, Some(41));
    assert_eq!(observed.domestic_tourists, Some(66));

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    assert_eq!(mirror.game.players[1].science_projects, expected);
    state.turn = 181;
    state.rivals[0].science_projects = Some(vec![
        "PROJECT_LAUNCH_EARTH_SATELLITE".to_string(),
        "PROJECT_LAUNCH_MOON_LANDING".to_string(),
        "PROJECT_LAUNCH_MARS_BASE".to_string(),
    ]);
    state.rivals[0].foreign_tourists = 44.0;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        mirror.game.players[1]
            .science_projects
            .contains("launch_mars_colony"),
        "a Gathering Storm Mars base must translate exactly as the local seat's does"
    );
    let observed = mirror.game.observed_public_empire_stats.get(&1).unwrap();
    assert_eq!(observed.foreign_tourists, Some(44));

    // An already-loaded older control mod omits the fields, and a refused
    // host read sends -1. The observed table is honest per snapshot —
    // unknown reads None — while the player's completed-milestone record
    // is history and must survive the silence.
    state.turn = 182;
    state.rivals[0].science_projects = None;
    state.rivals[0].foreign_tourists = f64::NAN;
    state.rivals[0].domestic_tourists = -1.0;
    mirror.sync(&snapshot, &state, 0);
    assert!(mirror.game.players[1]
        .science_projects
        .contains("launch_moon_landing"));
    let observed = mirror.game.observed_public_empire_stats.get(&1).unwrap();
    assert_eq!(observed.foreign_tourists, None);
    assert_eq!(observed.domestic_tourists, None);
}

/// The tourist counters the mirror records must outrank the engine's own
/// reconstruction — a live board has no culture history to derive them
/// from, so without the preference every rival's culture-victory progress
/// reads zero (the lane that stole four led runs on 2026-08-16/17).
#[test]
fn observed_tourist_counters_outrank_the_reconstruction() {
    let mut game = crate::game::Game::new(2, 8, 8, 42, 250, 0);
    let engine_foreign = game.foreign_tourists(1);
    let engine_domestic = game.domestic_tourists(1);
    {
        let observed = Arc::make_mut(&mut game.observed_public_empire_stats)
            .entry(1)
            .or_default();
        observed.foreign_tourists = Some(41);
        observed.domestic_tourists = Some(66);
    }
    assert_eq!(game.foreign_tourists(1), 41);
    assert_eq!(game.domestic_tourists(1), 66);
    // An entry with no counters falls back to the engine's arithmetic.
    {
        let observed = Arc::make_mut(&mut game.observed_public_empire_stats)
            .entry(1)
            .or_default();
        observed.foreign_tourists = None;
        observed.domestic_tourists = None;
    }
    assert_eq!(game.foreign_tourists(1), engine_foreign);
    assert_eq!(game.domestic_tourists(1), engine_domestic);
}

/// The seat's own two counters ride the state event and land on the
/// observed table's local entry, exactly as each rival's do.
#[test]
fn own_tourist_counters_reach_the_observed_table() {
    let raw = r#"{"kind":"state", "turn":120,
                      "foreign_tourists":9, "domestic_tourists":31}"#;
    let state = state_from_json(raw).expect("the own-counter wire parses");
    assert_eq!(state.foreign_tourists, 9.0);
    assert_eq!(state.domestic_tourists, 31.0);
    assert!(
        state.schema_gaps.is_empty(),
        "own tourist counters must be schema-recognized: {:?}",
        state.schema_gaps
    );
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 120,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS")],
    }]);
    let rebuilt = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let observed = rebuilt
        .game
        .observed_public_empire_stats
        .get(&0)
        .expect("the local seat has an observed entry");
    assert_eq!(observed.foreign_tourists, Some(9));
    assert_eq!(observed.domestic_tourists, Some(31));
    assert_eq!(rebuilt.game.foreign_tourists(0), 9);
    assert_eq!(rebuilt.game.domestic_tourists(0), 31);
}

/// ★★★ `Game::spies` was empty for the whole of a live game, so the AI's
/// entire espionage layer — twelve missions, per-lane promotion
/// priorities, a +90 weight on the denial target — iterated an empty map
/// and could not choose anything. And the blanket production block is why
/// the seat never held a Spy to seat: over twelve completed live games it
/// finished holding the Diplomatic Service civic in 12 of 12 and fielded
/// zero Spies.
#[test]
fn live_spies_are_seated_and_the_block_follows_capacity() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 120,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS"), plot(4, 4, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 120,
        spy_capacity: Some(2),
        ..StateSnapshot::default()
    };
    // A city, or `player_city_ids` is empty and the block is vacuous in
    // both directions — which is how the first draft of this test passed
    // its "unblocked" assertion while proving nothing.
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 4,
        y: 4,
        pop: 4,
        ..StateCity::default()
    });
    state.units.push(StateUnit {
        id: 77,
        kind: "UNIT_SPY".to_string(),
        x: 3,
        y: 3,
        ..StateUnit::default()
    });
    let rebuilt = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let seated: Vec<_> = rebuilt
        .game
        .spies
        .values()
        .filter(|spy| spy.owner == 0)
        .collect();
    assert_eq!(
        seated.len(),
        1,
        "the live Spy reaches the AI's own structure"
    );
    assert_eq!(
        rebuilt.unit_ids.get(&seated[0].id),
        Some(&77),
        "the spy id is its unit id, so an order translates straight back"
    );

    // One of two: there is room, so the production block lifts.
    let spy_item = crate::game::Item::Unit {
        unit: crate::name!("spy"),
    };
    let key = crate::game::Game::production_block_key(&spy_item);
    let blocked_somewhere = rebuilt
        .game
        .blocked_production
        .values()
        .any(|keys| keys.contains(&key));
    assert!(
        !blocked_somewhere,
        "under capacity the empire must be allowed to train the Spy it can field"
    );

    // At capacity it is blocked again — the refusals the blanket block was
    // written for are exactly ordering past the limit.
    let mut full = state.clone();
    full.spy_capacity = Some(1);
    let at_cap = rebuild_from_state(&snapshot, &full, 2, 1, 250, 0);
    assert!(
        at_cap
            .game
            .blocked_production
            .values()
            .any(|keys| keys.contains(&key)),
        "at capacity the order is unplayable and must stay blocked"
    );

    // An older mod cannot report capacity: keep the old unconditional
    // block rather than loosening a bridge that cannot measure itself.
    let mut silent = state.clone();
    silent.spy_capacity = None;
    let unknown = rebuild_from_state(&snapshot, &silent, 2, 1, 250, 0);
    assert!(
        unknown
            .game
            .blocked_production
            .values()
            .any(|keys| keys.contains(&key)),
        "unknown capacity must fail closed"
    );
}

/// A visible city banner is public; an agent assigned there is not.
#[test]
fn a_rival_spy_on_a_visible_city_never_crosses_the_live_mirror() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 120,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS"), plot(4, 4, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 120,
        spy_capacity: Some(2),
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 3,
        y: 3,
        pop: 4,
        ..StateCity::default()
    });
    state.units.push(StateUnit {
        id: 77,
        kind: "UNIT_SPY".to_string(),
        x: 3,
        y: 3,
        ..StateUnit::default()
    });
    state.rivals.push(StateRival {
        player: 3,
        civ: "CIVILIZATION_BRAZIL".to_string(),
        cities: vec![StateCity {
            id: 9,
            name: "Curitiba".to_string(),
            x: 4,
            y: 4,
            pop: 6,
            ..StateCity::default()
        }],
        units: vec![StateUnit {
            id: 88,
            kind: "UNIT_SPY".to_string(),
            x: 4,
            y: 4,
            ..StateUnit::default()
        }],
        ..StateRival::default()
    });
    let assert_private = |game: &crate::game::Game| {
        assert!(
            game.spies.values().any(|spy| spy.owner == 0),
            "the local Spy must still reach the espionage layer"
        );
        assert!(
            !game
                .units
                .values()
                .any(|unit| unit.owner != 0 && unit.kind == "spy"),
            "seeing Curitiba must not reveal Brazil's Spy"
        );
        let view = crate::obs::observation_player_view(game, 0);
        assert!(
            !view["units"]
                .as_array()
                .expect("units array")
                .iter()
                .any(|unit| unit["type"] == "spy" && unit["owner"] != 0),
            "the player view must not retain the secret either"
        );
    };

    let rebuilt = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    assert_private(&rebuilt.game);

    let mut mirror = LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
    assert_private(&mirror.game);
    state.turn += 1;
    mirror.sync(&snapshot, &state, 0);
    assert_private(&mirror.game);
}

/// ★★★★ A FRESH LIVE SPY OWES NO PROMOTION, so the mission layer is
/// reachable at all. Civilization VI grants a Spy its first promotion at
/// level 2; the native rule owes one per level. Seating the host's level
/// unshifted made every fresh live Spy permanently "promotable", and
/// `legal_spy_actions` returns promotions as the ONLY legal actions while
/// one is owed — so no live Spy ever received a travel or mission order
/// (run civvis-20260818T095712Z: the same impossible promotion sent for
/// 73 consecutive turns). And a Spy that finished its travel must seat in
/// the RIVAL city it stands in — matching own cities only imported it
/// with no city, which generates no missions either.
#[test]
fn a_fresh_live_spy_owes_no_promotion_and_a_travelled_one_seats_abroad() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 120,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![
            plot(3, 3, "TERRAIN_GRASS"),
            plot(4, 4, "TERRAIN_GRASS"),
            plot(5, 5, "TERRAIN_GRASS"),
        ],
    }]);
    let mut state = StateSnapshot {
        turn: 120,
        spy_capacity: Some(3),
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 4,
        y: 4,
        pop: 4,
        ..StateCity::default()
    });
    state.rivals.push(StateRival {
        player: 1,
        cities: vec![StateCity {
            id: 9,
            name: "Aduatuca".to_string(),
            x: 5,
            y: 5,
            pop: 6,
            ..StateCity::default()
        }],
        ..StateRival::default()
    });
    // A fresh Spy at home, a travelled one standing in the rival city, and
    // a genuinely levelled one whose earned pick must survive the shift.
    for (id, x, y, level) in [(77, 4, 4, 1), (78, 5, 5, 1), (79, 4, 4, 2)] {
        state.units.push(StateUnit {
            id,
            kind: "UNIT_SPY".to_string(),
            x,
            y,
            level: Some(level),
            ..StateUnit::default()
        });
    }
    let rebuilt = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let uid = |civ6: i64| {
        *rebuilt
            .unit_ids
            .iter()
            .find(|(_, mapped)| **mapped == civ6)
            .map(|(uid, _)| uid)
            .expect("the spy is mirrored")
    };

    let fresh = &rebuilt.game.spies[&uid(77)];
    assert_eq!(fresh.level, 0, "host level 1 is zero promotions owed");
    assert!(
        rebuilt
            .game
            .legal_spy_actions(0, fresh.id)
            .iter()
            .all(|action| !matches!(action, crate::game::Action::PromoteSpy { .. })),
        "a fresh Spy must not be gated behind a promotion the host refuses"
    );

    let travelled = &rebuilt.game.spies[&uid(78)];
    let seat = travelled.city.expect("the travelled Spy seats in a city");
    assert_ne!(
        rebuilt.game.cities[&seat].owner, 0,
        "the city it stands in is the rival's, which is what missions aim from"
    );

    let levelled = &rebuilt.game.spies[&uid(79)];
    assert_eq!(levelled.level, 1, "host level 2 owes exactly one pick");
    assert!(
        rebuilt
            .game
            .legal_spy_actions(0, levelled.id)
            .iter()
            .any(|action| matches!(action, crate::game::Action::PromoteSpy { .. })),
        "the promotion a mission actually earned is still offered"
    );
}

/// The host's victory checkboxes have crossed the wire in the seat event
/// all along and were dropped: a live board always played the all-six
/// default, so `victory_strategy_enabled` could authorise a lane the
/// lobby had switched off.
#[test]
fn the_seat_victory_checkboxes_reach_the_mirrored_game() {
    let seat: Seat = serde_json::from_str(
        r#"{"local_player":0,
                "victories":{"conquest":false,"score":true,"technology":true,
                             "culture":false,"religious":null,"diplomatic":true}}"#,
    )
    .expect("the seat victory wire parses");
    let victories = seat.victories.expect("checkboxes present");
    assert_eq!(victories.conquest, Some(false));
    assert_eq!(victories.religious, None, "a refused read stays unknown");

    let mut game = crate::game::Game::new(2, 8, 8, 42, 250, 0);
    game.victory_conditions = crate::game::VictoryConditions::default();
    let seat = Seat {
        victories: Some(victories),
        ..Seat::default()
    };
    apply_seat_victories(&mut game, &seat);
    assert!(!game.victory_conditions.domination, "conquest off crosses");
    assert!(!game.victory_conditions.culture);
    assert!(
        game.victory_conditions.science,
        "technology maps to science"
    );
    assert!(game.victory_conditions.score);
    assert!(game.victory_conditions.diplomatic);
    assert!(
        game.victory_conditions.religious,
        "an unknown checkbox keeps the default rather than switching a lane off"
    );

    // An older mod sends no `victories` at all: the default stands whole.
    let mut untouched = crate::game::Game::new(2, 8, 8, 42, 250, 0);
    apply_seat_victories(&mut untouched, &Seat::default());
    assert!(untouched.victory_conditions.domination);
}

#[test]
fn initializing_host_power_cannot_erase_a_visible_starting_warrior() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 1,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS")],
    }]);
    let state = StateSnapshot {
        turn: 1,
        military: 0.0,
        units: vec![StateUnit {
            id: 1,
            kind: "UNIT_WARRIOR".to_string(),
            x: 3,
            y: 3,
            hp: 100.0,
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };
    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    assert!(
        recon.game.military_power(0) >= 20.0,
        "the public aggregate initializes at zero on turn 1, but the visible unit is real"
    );
}

#[test]
fn supported_unique_improvements_and_city_religion_are_not_dropped() {
    let mut improved = plot(4, 4, "TERRAIN_PLAINS");
    improved.im = Some("IMPROVEMENT_KURGAN".to_string());
    let mut resort = plot(6, 4, "TERRAIN_GRASS");
    resort.im = Some("IMPROVEMENT_BEACH_RESORT".to_string());
    let mut mountain_road = plot(7, 4, "TERRAIN_PLAINS_MOUNTAIN");
    mountain_road.im = Some("IMPROVEMENT_MOUNTAIN_ROAD".to_string());
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 50,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![
            improved,
            plot(5, 4, "TERRAIN_PLAINS"),
            resort,
            mountain_road,
        ],
    }]);
    let state = StateSnapshot {
        turn: 50,
        cities: vec![StateCity {
            id: 10,
            name: "Faith City".to_string(),
            x: 5,
            y: 4,
            pop: 4,
            religion: Some("RELIGION_ORTHODOXY".to_string()),
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };

    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let kurgan = crate::hex::offset_to_axial(4, 4);
    assert_eq!(
        recon.game.map.tiles[&kurgan].improvement.as_deref(),
        Some("kurgan")
    );
    let resort = crate::hex::offset_to_axial(6, 4);
    assert_eq!(
        recon.game.map.tiles[&resort].improvement.as_deref(),
        Some("seaside_resort")
    );
    let mountain_road = crate::hex::offset_to_axial(7, 4);
    assert_eq!(
        recon.game.map.tiles[&mountain_road].improvement.as_deref(),
        Some("qhapaq_nan")
    );
    let city = recon
        .game
        .cities
        .values()
        .find(|city| city.owner == 0)
        .unwrap();
    assert_eq!(recon.game.city_religion(city), Some("Orthodoxy"));
}

#[test]
fn gathering_storm_defender_of_faith_alias_reaches_the_model() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS")],
    }]);
    let state = StateSnapshot {
        turn: 30,
        founded_religion: Some("RELIGION_CATHOLICISM".to_string()),
        religion_beliefs: vec!["BELIEF_DEFENDER_OF_FAITH".to_string()],
        ..StateSnapshot::default()
    };
    let rebuilt = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    assert_eq!(
        rebuilt.game.players[0].religion_beliefs,
        vec!["defender_of_the_faith".to_string()]
    );
    assert!(
        !rebuilt
            .unmapped
            .iter()
            .any(|issue| issue == "BELIEF_DEFENDER_OF_FAITH:belief"),
        "the installed XML spelling must not be reported as an unmapped belief: {:?}",
        rebuilt.unmapped
    );
}

/// Each founded religion's beliefs land on its founder's seat, and a city
/// following that religion reads exactly those follower beliefs. Rome
/// followed a Catholicism it did not found and read 23 Faith in the
/// model against the host's 35 for the last twenty turns of run
/// civvis-20260816T123936Z: three Wonders under Divine Inspiration, a
/// belief the union `taken_religion_beliefs` could not place.
#[test]
fn each_religions_beliefs_sit_on_its_founders_seat() {
    let mut center = plot(5, 4, "TERRAIN_PLAINS");
    center.o = 0;
    let mut wonder = plot(6, 4, "TERRAIN_GRASS");
    wonder.o = 0;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 120,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![center, wonder, plot(2, 2, "TERRAIN_PLAINS")],
    }]);
    let state = StateSnapshot {
        turn: 120,
        founded_religions: vec![
            "RELIGION_CATHOLICISM".to_string(),
            "RELIGION_ISLAM".to_string(),
            "RELIGION_JUDAISM".to_string(),
        ],
        taken_religion_beliefs: vec![
            "BELIEF_DIVINE_INSPIRATION".to_string(),
            "BELIEF_FEED_THE_WORLD".to_string(),
            "BELIEF_TITHE".to_string(),
            "BELIEF_WORK_ETHIC".to_string(),
        ],
        religions: vec![
            StateReligion {
                religion: "RELIGION_CATHOLICISM".to_string(),
                founder: 4,
                beliefs: vec![
                    "BELIEF_DIVINE_INSPIRATION".to_string(),
                    "BELIEF_TITHE".to_string(),
                ],
            },
            StateReligion {
                religion: "RELIGION_ISLAM".to_string(),
                founder: 2,
                beliefs: vec!["BELIEF_FEED_THE_WORLD".to_string()],
            },
            // A founder this seat has never met: still counted, still
            // carrying its own beliefs, on a seat nobody else took.
            StateReligion {
                religion: "RELIGION_JUDAISM".to_string(),
                founder: 9,
                beliefs: vec!["BELIEF_WORK_ETHIC".to_string()],
            },
        ],
        rivals: vec![
            StateRival {
                player: 2,
                civ: "CIVILIZATION_ARABIA".to_string(),
                ..StateRival::default()
            },
            StateRival {
                player: 4,
                civ: "CIVILIZATION_SPAIN".to_string(),
                ..StateRival::default()
            },
        ],
        cities: vec![StateCity {
            id: 10,
            name: "Rome".to_string(),
            x: 5,
            y: 4,
            pop: 6,
            loyalty: 100.0,
            religion: Some("RELIGION_CATHOLICISM".to_string()),
            wonders: vec![StateWonder {
                kind: "BUILDING_STONEHENGE".to_string(),
                x: 6,
                y: 4,
            }],
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    let game = &recon.game;
    // Rivals hold seats in host order: Arabia (host 2) is seat 1, Spain
    // (host 4) is seat 2; Judaism's unmet founder takes seat 3.
    assert_eq!(game.players[2].religion.as_deref(), Some("Catholicism"));
    assert_eq!(
        game.players[2].religion_beliefs,
        vec!["divine_inspiration".to_string(), "tithe".to_string()]
    );
    assert_eq!(game.players[1].religion.as_deref(), Some("Islam"));
    assert_eq!(
        game.players[1].religion_beliefs,
        vec!["feed_the_world".to_string()]
    );
    assert_eq!(game.players[3].religion.as_deref(), Some("Judaism"));
    assert_eq!(
        game.players[3].religion_beliefs,
        vec!["work_ethic".to_string()]
    );
    assert_eq!(game.religions_founded(), 3);
    assert!(game.players[0].religion.is_none());
    assert!(game.players[0].religion_beliefs.is_empty());
    // Rome follows Catholicism, so its Wonder pays Divine Inspiration's
    // four Faith in the model itself, not in a correction.
    let rome = game.player_city_ids(0)[0];
    let city = &game.cities[&rome];
    assert_eq!(game.city_religion(city), Some("Catholicism"));
    assert!(city.wonders.contains_key(&crate::name!("stonehenge")));
    let mut without = recon.game.clone();
    without.players[2].religion_beliefs.clear();
    assert_eq!(
        game.city_yields_model(rome).faith,
        without.city_yields_model(rome).faith + 4.0,
        "Divine Inspiration reaches a following city's own Faith"
    );
}

/// The host's Faith per turn is a correction on the empire figure, like
/// science and culture; the Faith paid for unused Great Person points is
/// part of the model's figure and so of what the correction is measured
/// against — and a class absent from the host's cost map is what makes
/// its points unused.
#[test]
fn host_faith_per_turn_and_unused_great_person_classes_reach_the_board() {
    let mut center = plot(5, 4, "TERRAIN_PLAINS");
    center.o = 0;
    let mut campus = plot(6, 4, "TERRAIN_GRASS");
    campus.o = 0;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 220,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![center, campus],
    }]);
    let mut points = BTreeMap::new();
    points.insert("GREAT_PERSON_CLASS_SCIENTIST".to_string(), 700.0);
    points.insert("GREAT_PERSON_CLASS_MERCHANT".to_string(), 40.0);
    let mut costs = BTreeMap::new();
    costs.insert("GREAT_PERSON_CLASS_MERCHANT".to_string(), 660.0);
    let state = StateSnapshot {
        turn: 220,
        faith_per_turn: Some(61.5),
        faith_sources: Some("+35 from Cities\n+26.5 from Other".to_string()),
        great_person_points: Some(points),
        great_person_costs: Some(costs),
        cities: vec![StateCity {
            id: 10,
            name: "Rome".to_string(),
            x: 5,
            y: 4,
            pop: 8,
            loyalty: 100.0,
            districts: vec![StateDistrict {
                kind: "DISTRICT_CAMPUS".to_string(),
                x: 6,
                y: 4,
                complete: true,
                ..StateDistrict::default()
            }],
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let game = &recon.game;
    assert_eq!(
        game.players[0].live_great_person_exhausted,
        Some(["scientist".to_string()].into_iter().collect()),
        "points without a cost: the class has nobody left on the host's timeline"
    );
    assert!(!game.great_person_class_earnable(0, "scientist"));
    assert!(game.great_person_class_earnable(0, "merchant"));
    let rate = game.great_person_points_per_turn(0);
    let scientist = rate.get("scientist").copied().unwrap_or(0.0);
    assert!(
        scientist > 0.0,
        "the Campus pays Scientist points: {rate:?}"
    );
    assert_eq!(game.unused_great_person_faith(0), scientist);
    // Cities plus the empire's extras plus the correction equal the host.
    let mut yields = crate::rules::Yields::default();
    for cid in game.player_city_ids(0) {
        yields.add(game.city_yields(cid));
    }
    yields.add(game.player_yield_extras(0));
    yields.add(game.observed_yield_adjustments[&0]);
    assert!(
        (yields.faith - 61.5).abs() < 1e-9,
        "board faith {} vs host 61.5",
        yields.faith
    );
    assert_eq!(
        state.faith_sources.as_deref(),
        Some("+35 from Cities\n+26.5 from Other")
    );

    // An older export without the cost map leaves the engine's own roster
    // in charge, and without a host figure leaves the model's Faith alone.
    let older = StateSnapshot {
        great_person_costs: None,
        faith_per_turn: None,
        ..state.clone()
    };
    let recon = rebuild_from_state(&snapshot, &older, 2, 1, 250, 0);
    assert_eq!(recon.game.players[0].live_great_person_exhausted, None);
    assert_eq!(
        recon
            .game
            .observed_yield_adjustments
            .get(&0)
            .map(|adjustment| adjustment.faith),
        None
    );

    // The mod's own list wins over the cost-map inference, and an empty
    // list is the real answer "everyone is still available" — even when
    // the cost map is `nil`, which alone could not tell that from an old export.
    let explicit = StateSnapshot {
        great_person_exhausted: Some(vec!["GREAT_PERSON_CLASS_WRITER".to_string()]),
        great_person_costs: None,
        ..state.clone()
    };
    let recon = rebuild_from_state(&snapshot, &explicit, 2, 1, 250, 0);
    assert_eq!(
        recon.game.players[0].live_great_person_exhausted,
        Some(["writer".to_string()].into_iter().collect())
    );
    assert!(recon.game.great_person_class_earnable(0, "scientist"));
    let nobody = StateSnapshot {
        great_person_exhausted: Some(Vec::new()),
        great_person_costs: None,
        ..state.clone()
    };
    let recon = rebuild_from_state(&snapshot, &nobody, 2, 1, 250, 0);
    assert_eq!(
        recon.game.players[0].live_great_person_exhausted,
        Some(BTreeSet::new())
    );
}

#[test]
fn host_economy_loyalty_and_city_defense_survive_the_mirror_save() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 50,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![plot(5, 4, "TERRAIN_PLAINS")],
    }]);
    let state = StateSnapshot {
        turn: 50,
        science: Some(6.75),
        culture: Some(6.03125),
        trade_capacity: Some(3),
        cities: vec![StateCity {
            id: 10,
            name: "Istanbul".to_string(),
            x: 5,
            y: 4,
            pop: 9,
            loyalty: 100.0,
            loyalty_per_turn: 10.2656,
            defense: 40.0,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };

    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let city = recon.game.player_city_ids(0)[0];
    assert_eq!(
        recon.game.city_loyalty_per_turn(&recon.game.cities[&city]),
        10.2656
    );
    assert_eq!(recon.game.city_strength(city), 40.0);
    assert_eq!(recon.game.trade_capacity(0), 3);
    let mut yields = crate::rules::Yields::default();
    for cid in recon.game.player_city_ids(0) {
        yields.add(recon.game.city_yields(cid));
    }
    yields.add(recon.game.observed_yield_adjustments[&0]);
    assert!((yields.science - 6.75).abs() < 1e-9);
    assert!((yields.culture - 6.03125).abs() < 1e-9);

    let saved = serde_json::to_string(&recon.game).expect("save mirrored game");
    let loaded: crate::game::Game = serde_json::from_str(&saved).expect("load mirrored game");
    assert_eq!(loaded.city_loyalty_per_turn(&loaded.cities[&city]), 10.2656);
    assert_eq!(loaded.city_strength(city), 40.0);
    assert_eq!(loaded.trade_capacity(0), 3);
    assert_eq!(
        loaded.observed_yield_adjustments[&0],
        recon.game.observed_yield_adjustments[&0]
    );
}

#[test]
fn zero_host_science_and_culture_override_live_city_yields() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 50,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![plot(5, 4, "TERRAIN_PLAINS")],
    }]);
    let city_yields = crate::rules::Yields {
        science: 6.75,
        culture: 6.03125,
        ..crate::rules::Yields::default()
    };
    let mut state = StateSnapshot {
        turn: 50,
        science: Some(city_yields.science),
        culture: Some(city_yields.culture),
        cities: vec![StateCity {
            id: 10,
            name: "Istanbul".to_string(),
            x: 5,
            y: 4,
            pop: 9,
            yields: Some(city_yields),
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let mut mirror = LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
    let initial = crate::obs::observation_spectator(&mirror.game, 0);
    assert_eq!(
        initial["players"][0]["yields"]["science"],
        serde_json::json!(6.8)
    );
    assert_eq!(
        initial["players"][0]["yields"]["culture"],
        serde_json::json!(6.0)
    );

    // Zero is a valid early-game host result. Retain the positive city reading
    // so this proves the top-bar zero installs its needed negative correction,
    // rather than merely matching a model that happened to yield zero already.
    state.turn = 51;
    state.science = Some(0.0);
    state.culture = Some(0.0);
    mirror.sync(&snapshot, &state, 0);
    let zeroed = crate::obs::observation_spectator(&mirror.game, 0);
    assert_eq!(
        zeroed["players"][0]["yields"]["science"],
        serde_json::json!(0.0)
    );
    assert_eq!(
        zeroed["players"][0]["yields"]["culture"],
        serde_json::json!(0.0)
    );
}

#[test]
fn exact_city_economy_and_great_work_survive_reconstruction() {
    let mut center = plot(5, 4, "TERRAIN_PLAINS");
    center.o = 0;
    let mut worked = plot(6, 4, "TERRAIN_GRASS");
    worked.o = 0;
    let mut theater = plot(5, 5, "TERRAIN_PLAINS");
    theater.o = 0;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 60,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![center, worked, theater],
    }]);
    let host_yields = crate::rules::Yields {
        food: 8.25,
        production: 7.5,
        gold: 6.75,
        science: 5.5,
        culture: 9.25,
        faith: 2.0,
    };
    let state = StateSnapshot {
        turn: 60,
        science: Some(host_yields.science),
        culture: Some(host_yields.culture),
        cities: vec![StateCity {
            id: 10,
            name: "Wroclaw".to_string(),
            x: 5,
            y: 4,
            pop: 2,
            buildings: vec!["BUILDING_AMPHITHEATER".to_string()],
            districts: vec![StateDistrict {
                kind: "DISTRICT_THEATER".to_string(),
                x: 5,
                y: 5,
                complete: true,
                ..StateDistrict::default()
            }],
            worked: Some(vec![StateWorkedPlot {
                x: 6,
                y: 4,
                yields: None,
            }]),
            specialists: Some(vec!["DISTRICT_THEATER".to_string()]),
            great_works: Some(vec![StateGreatWork {
                kind: "GREATWORK_QU_YUAN_1".to_string(),
                object: "GREATWORKOBJECT_WRITING".to_string(),
                era: Some("ERA_CLASSICAL".to_string()),
                creator: "LOC_GREAT_PERSON_INDIVIDUAL_QU_YUAN_NAME".to_string(),
                building: "BUILDING_AMPHITHEATER".to_string(),
                slot: 0,
            }]),
            yields: Some(host_yields),
            producing: Some("UNIT_WARRIOR".to_string()),
            production_progress: 12.5,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };

    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let cid = recon.game.player_city_ids(0)[0];
    let plan = recon.game.city_citizen_plan(cid);
    assert_eq!(
        plan.worked_tiles,
        vec![crate::hex::offset_to_axial(6, 4)],
        "the host assignment, not a freshly optimized replacement, is current state"
    );
    assert_eq!(plan.specialists, vec!["theater_square"]);
    assert_eq!(recon.game.players[0].counters["great_work:writing"], 1);
    assert_eq!(recon.game.players[0].great_work_pieces.len(), 1);
    // And the host's own housing is the model's: the work sits where the
    // export says, not where the model's best-slot heuristic would put it.
    assert_eq!(
        recon
            .game
            .observed_great_work_housing
            .as_ref()
            .and_then(|h| h.get(&cid))
            .and_then(|k| k.get("writing")),
        Some(&1)
    );
    assert_eq!(
        recon
            .game
            .housed_great_works(0)
            .get(&cid)
            .and_then(|k| k.get("writing")),
        Some(&1)
    );
    assert_eq!(recon.game.cities[&cid].production, 12.5);
    assert_eq!(recon.game.city_yields(cid), host_yields);

    let saved = serde_json::to_string(&recon.game).expect("save exact city mirror");
    let loaded: crate::game::Game = serde_json::from_str(&saved).expect("load exact city mirror");
    assert_eq!(
        loaded.city_citizen_plan(cid).worked_tiles,
        plan.worked_tiles
    );
    assert_eq!(loaded.city_yields(cid), host_yields);
    assert_eq!(loaded.players[0].counters["great_work:writing"], 1);
}

/// ★★★★★ A DISTRICT PLOT IN THE HOST'S WORKED LIST IS A SPECIALIST, NOT A TILE.
///
/// `Citizens:IsPlotWorked` answers true for a Campus a citizen staffs, and the
/// export names that citizen in `specialists`. Importing the plot as a worked
/// tile as well paid the specialist twice — its slot yield AND the terrain
/// under the district. Measured on live run civvis-20260816T011314Z: Cumae
/// with two Campus specialists and one Industrial Zone specialist read +2
/// Food, +4 Production over the host for twenty turns.
#[test]
fn a_worked_district_plot_is_the_specialist_not_a_second_tile() {
    let mut center = plot(5, 4, "TERRAIN_PLAINS");
    center.o = 0;
    let mut worked = plot(6, 4, "TERRAIN_GRASS");
    worked.o = 0;
    let mut campus = plot(5, 5, "TERRAIN_GRASS_HILLS");
    campus.o = 0;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 60,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![center, worked, campus],
    }]);
    let state = StateSnapshot {
        turn: 60,
        cities: vec![StateCity {
            id: 10,
            name: "Cumae".to_string(),
            x: 5,
            y: 4,
            pop: 2,
            // `StateCity::default()` is loyalty 0 — the revolt band, which
            // multiplies every yield by zero. A real export always carries
            // loyalty; a fixture must say so or its city yields nothing.
            loyalty: 100.0,
            districts: vec![StateDistrict {
                kind: "DISTRICT_CAMPUS".to_string(),
                x: 5,
                y: 5,
                complete: true,
                ..StateDistrict::default()
            }],
            // Firaxis lists the centre, the farmed tile AND the Campus plot.
            worked: Some(vec![
                StateWorkedPlot {
                    x: 5,
                    y: 4,
                    yields: None,
                },
                StateWorkedPlot {
                    x: 6,
                    y: 4,
                    yields: None,
                },
                StateWorkedPlot {
                    x: 5,
                    y: 5,
                    yields: None,
                },
            ]),
            specialists: Some(vec!["DISTRICT_CAMPUS".to_string()]),
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let cid = recon.game.player_city_ids(0)[0];
    let plan = recon.game.city_citizen_plan(cid);
    assert_eq!(
        plan.worked_tiles,
        vec![crate::hex::offset_to_axial(6, 4)],
        "the Campus plot is the specialist's seat, not a tile job"
    );
    assert_eq!(plan.specialists, vec!["campus"]);
    let ledger = recon.game.city_yield_ledger(cid);
    assert_eq!(ledger.tiles.len(), 1);
    assert_eq!(ledger.specialists.len(), 1);
}

/// ★★★★★ THE HOST'S PER-PLOT YIELDS CROSS AS TILE-LEVEL CORRECTIONS.
///
/// Some of what a tile pays only the host can know — the fertility an
/// eruption left (Rome on run civvis-20260816T003229Z read +12 Food over the
/// model on volcanic soil for forty turns). With `worked[].yields` and
/// `center_yields` in the export, the mirror pays each plot what the host
/// pays it, the city correction carries only what is left, and the modelled
/// tile stays readable beside the correction.
#[test]
fn host_plot_yields_become_tile_corrections_and_the_model_stays_readable() {
    let mut center = plot(5, 4, "TERRAIN_PLAINS");
    center.o = 0;
    let mut worked = plot(6, 4, "TERRAIN_GRASS");
    worked.o = 0;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 60,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![center, worked],
    }]);
    // Grassland pays 2 Food in the ruleset; the host says this one pays 4
    // Food and 1 Production (fertile ground the tile catalogue cannot see).
    let host_plot = crate::rules::Yields {
        food: 4.0,
        production: 1.0,
        ..crate::rules::Yields::default()
    };
    // Plains centre floors to 2 Food / 1 Production; the host says 3 / 2.
    let host_center = crate::rules::Yields {
        food: 3.0,
        production: 2.0,
        ..crate::rules::Yields::default()
    };
    // Centre 3/2 plus the tile 4/1: the food and production are entirely
    // the two plots; the rest is the city's own (Palace, citizen).
    let host_city = crate::rules::Yields {
        food: 7.0,
        production: 3.0,
        gold: 1.0,
        science: 0.5,
        culture: 1.3,
        faith: 0.0,
    };
    let state = StateSnapshot {
        turn: 60,
        cities: vec![StateCity {
            id: 10,
            name: "Ravenna".to_string(),
            x: 5,
            y: 4,
            pop: 1,
            loyalty: 100.0,
            worked: Some(vec![
                StateWorkedPlot {
                    x: 5,
                    y: 4,
                    yields: Some(host_center),
                },
                StateWorkedPlot {
                    x: 6,
                    y: 4,
                    yields: Some(host_plot),
                },
            ]),
            center_yields: Some(host_center),
            yields: Some(host_city),
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let cid = recon.game.player_city_ids(0)[0];
    let worked_pos = crate::hex::offset_to_axial(6, 4);
    let center_pos = crate::hex::offset_to_axial(5, 4);
    let tile_fix = recon.game.observed_tile_yield_adjustments[&worked_pos];
    assert!(
        (tile_fix.food - 2.0).abs() < 1e-9,
        "host 4 against modelled 2: {tile_fix:?}"
    );
    assert!((tile_fix.production - 1.0).abs() < 1e-9);
    // The ledger reads the model, not the corrected board.
    // The ledger reads the model, not the corrected board.
    let ledger = recon.game.city_yield_ledger(cid);
    let center_fix = recon.game.observed_tile_yield_adjustments[&center_pos];
    assert!(
        (center_fix.food - 2.0).abs() < 1e-9,
        "host 3 against the raw plains 1: {center_fix:?}"
    );
    assert!((center_fix.production - 1.0).abs() < 1e-9);
    assert!(
        (ledger.center.food - 2.0).abs() < 1e-9,
        "the ledger shows the floored model centre"
    );
    assert!((ledger.tiles[0].1.food - 2.0).abs() < 1e-9);
    assert_eq!(ledger.tile_adjustments.len(), 2);
    // And the board still agrees with the host to the last yield.
    assert_eq!(recon.game.city_yields(cid), host_city);
    // The tile-level part is out of the city-level correction: nothing but
    // the two plots pays Food here, so the city's own Food correction is
    // exactly zero (Production still carries the Palace's own term).
    let city_fix = recon.game.observed_city_yield_adjustments[&cid];
    assert!(
        (city_fix.food - 0.0).abs() < 1e-9,
        "food is fully explained by the tiles: {city_fix:?}"
    );

    let saved = serde_json::to_string(&recon.game).expect("save");
    let loaded: crate::game::Game = serde_json::from_str(&saved).expect("load");
    assert_eq!(loaded.city_yields(cid), host_city);
    assert_eq!(loaded.observed_tile_yield_adjustments.len(), 2);
}

/// ★★★★★ AND ON EVERY OTHER PLOT THE SWEEP READ, NOT ONLY THE WORKED ONES.
///
/// The correction above covers the six or eight plots a city is working this
/// turn. Everything else — the ground a Builder, a Settler and the citizen
/// governor are choosing BETWEEN — was paid CIVVIS's own catalogue sum, and
/// that sum has no row for the fertility an eruption leaves: Volcanic Soil
/// carries no `Feature_YieldChanges` at all, so the mirror read bare
/// Grassland where the live game showed 3 Food 3 Production.
#[test]
fn a_swept_plots_host_yields_correct_it_even_when_nobody_works_it() {
    let mut center = plot(5, 4, "TERRAIN_PLAINS");
    center.o = 0;
    let mut worked = plot(6, 4, "TERRAIN_GRASS");
    worked.o = 0;
    // The volcano's leavings: Grassland under Volcanic Soil, which the
    // ruleset prices at 2 Food and the host pays 3 Food 3 Production.
    let mut fertile = plot(6, 5, "TERRAIN_GRASS");
    fertile.o = 0;
    fertile.f = Some("FEATURE_VOLCANIC_SOIL".to_string());
    fertile.yl = Some(vec![3.0, 3.0, 0.0, 0.0, 0.0, 0.0]);
    // A plot the sweep read and nobody owns is corrected too: it is exactly
    // the ground a Settler is choosing between.
    let mut wild = plot(7, 5, "TERRAIN_PLAINS");
    wild.yl = Some(vec![1.0, 4.0, 0.0, 0.0, 0.0, 0.0]);
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 60,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![center, worked, fertile, wild],
    }]);
    let host_center = crate::rules::Yields {
        food: 3.0,
        production: 2.0,
        ..crate::rules::Yields::default()
    };
    let state = StateSnapshot {
        turn: 60,
        cities: vec![StateCity {
            id: 10,
            name: "Herculaneum".to_string(),
            x: 5,
            y: 4,
            pop: 1,
            loyalty: 100.0,
            worked: Some(vec![StateWorkedPlot {
                x: 5,
                y: 4,
                yields: Some(host_center),
            }]),
            center_yields: Some(host_center),
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let fertile_pos = crate::hex::offset_to_axial(6, 5);
    let fix = recon.game.observed_tile_yield_adjustments[&fertile_pos];
    // Grassland pays 2 Food in the catalogue and Volcanic Soil pays nothing;
    // the host says 3 Food 3 Production.
    assert!((fix.food - 1.0).abs() < 1e-9, "{fix:?}");
    assert!((fix.production - 3.0).abs() < 1e-9, "{fix:?}");
    let paid = recon.game.workable_tile_yields(fertile_pos);
    assert!(
        (paid.food - 3.0).abs() < 1e-9,
        "the board pays what the host pays: {paid:?}"
    );
    assert!((paid.production - 3.0).abs() < 1e-9, "{paid:?}");

    let wild_pos = crate::hex::offset_to_axial(7, 5);
    let wild_paid = recon.game.workable_tile_yields(wild_pos);
    assert!((wild_paid.production - 4.0).abs() < 1e-9, "{wild_paid:?}");

    // A counterfactual improvement still moves the tile by its modelled
    // amount, because the correction is a delta and not an override.
    let mut planned = recon.game.clone();
    planned.map.tiles.get_mut(&fertile_pos).unwrap().improvement = Some(crate::name!("farm"));
    let farmed = planned.workable_tile_yields(fertile_pos);
    assert!(
        farmed.food > paid.food,
        "a modelled Farm must still pay on top of the host's reading: {farmed:?}"
    );
}

/// The state export's reading is this turn's; the sweep's may be several
/// turns old. Where both speak, the fresher one is the one that pays.
#[test]
fn a_worked_plots_state_reading_outranks_the_sweeps_older_one() {
    let mut center = plot(5, 4, "TERRAIN_PLAINS");
    center.o = 0;
    let mut worked = plot(6, 4, "TERRAIN_GRASS");
    worked.o = 0;
    worked.yl = Some(vec![2.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 60,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![center, worked],
    }]);
    let host_center = crate::rules::Yields {
        food: 3.0,
        production: 2.0,
        ..crate::rules::Yields::default()
    };
    let host_worked = crate::rules::Yields {
        food: 5.0,
        ..crate::rules::Yields::default()
    };
    let state = StateSnapshot {
        turn: 60,
        cities: vec![StateCity {
            id: 10,
            name: "Pompeii".to_string(),
            x: 5,
            y: 4,
            pop: 1,
            loyalty: 100.0,
            worked: Some(vec![
                StateWorkedPlot {
                    x: 5,
                    y: 4,
                    yields: Some(host_center),
                },
                StateWorkedPlot {
                    x: 6,
                    y: 4,
                    yields: Some(host_worked),
                },
            ]),
            center_yields: Some(host_center),
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let worked_pos = crate::hex::offset_to_axial(6, 4);
    let paid = recon.game.workable_tile_yields(worked_pos);
    assert!(
        (paid.food - 5.0).abs() < 1e-9,
        "the state export's 5 Food outranks the sweep's 2: {paid:?}"
    );
}

/// `Snapshot::revealed` accumulates and never forgets, so a plot the newest
/// sweep missed still sits in it looking authoritative. Its yields are a
/// claim about a turn that has passed, and paying a tile on one would freeze
/// it at what it was worth then.
#[test]
fn a_plot_older_than_the_newest_sweep_corrects_nothing() {
    let mut center = plot(5, 4, "TERRAIN_PLAINS");
    center.o = 0;
    let mut stale = plot(6, 5, "TERRAIN_GRASS");
    stale.o = 0;
    stale.yl = Some(vec![9.0, 9.0, 0.0, 0.0, 0.0, 0.0]);
    let snapshot = Snapshot::from_chunks(&[
        TilesChunk {
            turn: 20,
            width: 10,
            height: 10,
            chunk: 1,
            plots: vec![stale],
        },
        TilesChunk {
            turn: 60,
            width: 10,
            height: 10,
            chunk: 1,
            plots: vec![center],
        },
    ]);
    assert!(
        !snapshot.is_current((6, 5)),
        "the turn-20 record is not current"
    );
    assert!(snapshot.is_current((5, 4)));
    let state = StateSnapshot {
        turn: 60,
        cities: vec![StateCity {
            id: 10,
            name: "Stabiae".to_string(),
            x: 5,
            y: 4,
            pop: 1,
            loyalty: 100.0,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let stale_pos = crate::hex::offset_to_axial(6, 5);
    assert!(
        !recon
            .game
            .observed_tile_yield_adjustments
            .contains_key(&stale_pos),
        "a record the newest sweep did not write must correct nothing"
    );
}

/// A mirrored board carries no simulated weather: the generated world's
/// disaster bookkeeping must not survive onto ground the host described,
/// or a modelled eruption would be counted twice — once as the model's own
/// fertility and once inside the host correction taken against it.
#[test]
fn mirrored_ground_keeps_no_generated_disaster_state() {
    let mut plot_a = plot(5, 4, "TERRAIN_PLAINS");
    plot_a.o = 0;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 60,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![plot_a],
    }]);
    let mut game = rebuild_game(&snapshot, 2, 1);
    let pos = crate::hex::offset_to_axial(5, 4);
    {
        let tile = game.map.tiles.get_mut(&pos).unwrap();
        tile.disaster_food = 2.0;
        tile.disaster_production = 1.0;
        tile.disaster_faith = 3.0;
        tile.drought = true;
        tile.flooded = true;
        tile.submerged = true;
        tile.storm = Some("hurricane".to_string());
        tile.fallout_until = 99;
    }
    apply_terrain(&mut game, &snapshot);
    let tile = &game.map.tiles[&pos];
    assert_eq!(tile.disaster_food, 0.0);
    assert_eq!(tile.disaster_production, 0.0);
    assert_eq!(tile.disaster_faith, 0.0);
    assert!(!tile.drought && !tile.flooded && !tile.submerged);
    assert!(tile.storm.is_none());
    assert_eq!(tile.fallout_until, 0);
}

/// An `improved` event folds a finished improvement onto a plot the sweep
/// read earlier. The yield tuple that came with that sweep was measured
/// before the improvement existed, so keeping it would make the correction
/// cancel the very Farm the event reports.
#[test]
fn folding_a_finished_improvement_drops_the_plots_older_yield_reading() {
    let mut fertile = plot(6, 5, "TERRAIN_GRASS");
    fertile.o = 0;
    fertile.yl = Some(vec![3.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    let mut snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 60,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![fertile],
    }]);
    assert!(snapshot.set_improvement((6, 5), "IMPROVEMENT_FARM"));
    assert!(
        snapshot.plot((6, 5)).and_then(Plot::host_yields).is_none(),
        "the reading predates the improvement and cannot stand beside it"
    );
}

/// ★★★★★ A PILLAGED BUILDING PAYS NOTHING, AND THE EXPORT NOW SAYS WHICH.
///
/// `HasBuilding` stays true for a pillaged Library. Without the pillage list
/// the mirror paid Antium +6 Science on a raided Campus for twenty turns
/// (run civvis-20260816T011314Z t147-t170: host 5.9, model 11.2).
#[test]
fn pillaged_buildings_cross_the_bridge_and_stop_paying() {
    let mut center = plot(5, 4, "TERRAIN_PLAINS");
    center.o = 0;
    let mut campus = plot(5, 5, "TERRAIN_GRASS_HILLS");
    campus.o = 0;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 60,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![center, campus],
    }]);
    let city = |pillaged: Option<Vec<String>>| StateCity {
        id: 10,
        name: "Antium".to_string(),
        x: 5,
        y: 4,
        pop: 1,
        loyalty: 100.0,
        buildings: vec!["BUILDING_LIBRARY".to_string()],
        pillaged_buildings: pillaged,
        // Pin the citizen so the only difference between the two boards is
        // the Library itself: left to its own governor, the intact city
        // seats its citizen in the Library's specialist slot (+2 more).
        worked: Some(vec![StateWorkedPlot {
            x: 5,
            y: 4,
            yields: None,
        }]),
        specialists: Some(vec![]),
        districts: vec![StateDistrict {
            kind: "DISTRICT_CAMPUS".to_string(),
            x: 5,
            y: 5,
            complete: true,
            ..StateDistrict::default()
        }],
        ..StateCity::default()
    };
    let intact = StateSnapshot {
        turn: 60,
        cities: vec![city(Some(vec![]))],
        ..StateSnapshot::default()
    };
    let raided = StateSnapshot {
        turn: 60,
        cities: vec![city(Some(vec!["BUILDING_LIBRARY".to_string()]))],
        ..StateSnapshot::default()
    };
    let intact = rebuild_from_state(&snapshot, &intact, 2, 1, 250, 0);
    let raided = rebuild_from_state(&snapshot, &raided, 2, 1, 250, 0);
    let intact_cid = intact.game.player_city_ids(0)[0];
    let raided_cid = raided.game.player_city_ids(0)[0];
    assert!(intact.game.cities[&intact_cid]
        .pillaged_buildings
        .is_empty());
    assert!(raided.game.cities[&raided_cid]
        .pillaged_buildings
        .contains(&crate::name::Name::new("library")));
    let intact_science = intact.game.city_yields_model(intact_cid).science;
    let raided_science = raided.game.city_yields_model(raided_cid).science;
    assert!(
        (intact_science - raided_science - 2.0).abs() < 1e-9,
        "the Library's 2 Science must stop while it is pillaged: {intact_science} vs {raided_science}"
    );
    // An older export says nothing about pillage and must not clear anything.
    let unknown = StateSnapshot {
        turn: 60,
        cities: vec![city(None)],
        ..StateSnapshot::default()
    };
    let unknown = rebuild_from_state(&snapshot, &unknown, 2, 1, 250, 0);
    let unknown_cid = unknown.game.player_city_ids(0)[0];
    assert!(unknown.game.cities[&unknown_cid]
        .pillaged_buildings
        .is_empty());
}

/// The host's Housing ceiling reaches the board as a delta, the Amenity map's
/// twin: the number beside population is the host's, and a counterfactual
/// Granary still moves it by its modelled amount.
#[test]
fn host_housing_reaches_the_board_as_a_delta() {
    let mut center = plot(5, 4, "TERRAIN_PLAINS");
    center.o = 0;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 60,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![center],
    }]);
    let state = StateSnapshot {
        turn: 60,
        cities: vec![StateCity {
            id: 10,
            name: "Ostia".to_string(),
            x: 5,
            y: 4,
            pop: 3,
            loyalty: 100.0,
            housing: Some(9.0),
            amenities: 1.0,
            amenities_needed: 2.0,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let cid = recon.game.player_city_ids(0)[0];
    let city = &recon.game.cities[&cid];
    assert!((recon.game.city_housing(city) - 9.0).abs() < 1e-9);
    assert_eq!(
        recon.game.city_amenities(city),
        1,
        "the count reads the host's, not the model's"
    );
    assert_eq!(recon.game.city_amenity_surplus(city), -1);
    let saved = serde_json::to_string(&recon.game).expect("save");
    let loaded: crate::game::Game = serde_json::from_str(&saved).expect("load");
    assert!((loaded.city_housing(&loaded.cities[&cid]) - 9.0).abs() < 1e-9);
}

/// ★★★★★ THE PALACE SITS WHERE THE HOST'S CAPITAL IS.
///
/// `place_city` flags the first city seated for a player as its capital, so
/// after the founding city fell the mirror paid the Palace in whichever city
/// the export listed first (Antium) while the host had moved it (Aquileia):
/// 5 Gold, 2 Production, 2 Science, 1 Culture wrong in two cities for the
/// rest of run civvis-20260816T040537Z.
#[test]
fn the_palace_follows_the_hosts_capital_flag() {
    let mut first = plot(5, 4, "TERRAIN_PLAINS");
    first.o = 0;
    let mut second = plot(8, 4, "TERRAIN_PLAINS");
    second.o = 0;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 90,
        width: 12,
        height: 10,
        chunk: 1,
        plots: vec![first, second],
    }]);
    let city = |id: i64, name: &str, x: i32, capital: bool| StateCity {
        id,
        name: name.to_string(),
        x,
        y: 4,
        pop: 3,
        loyalty: 100.0,
        capital,
        ..StateCity::default()
    };
    // Listed first, but NOT the capital: the host moved the Palace to the
    // second city after the founding city was lost.
    let state = StateSnapshot {
        turn: 90,
        cities: vec![city(2, "Antium", 5, false), city(3, "Aquileia", 8, true)],
        ..StateSnapshot::default()
    };
    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let antium = recon
        .game
        .city_at(crate::hex::offset_to_axial(5, 4))
        .unwrap();
    let aquileia = recon
        .game
        .city_at(crate::hex::offset_to_axial(8, 4))
        .unwrap();
    assert!(!recon.game.cities[&antium].is_capital);
    assert!(recon.game.cities[&aquileia].is_capital);
    assert!(!recon.game.city_has_palace(&recon.game.cities[&antium]));
    assert!(recon.game.city_has_palace(&recon.game.cities[&aquileia]));
}

/// The tiles export's pillage bit reaches the tile, and only where an
/// improvement stands.
#[test]
fn a_pillaged_improvement_crosses_as_pillaged_and_pays_nothing() {
    let mut center = plot(5, 4, "TERRAIN_PLAINS");
    center.o = 0;
    let mut pasture = plot(6, 4, "TERRAIN_PLAINS");
    pasture.o = 0;
    pasture.r = Some("RESOURCE_HORSES".to_string());
    pasture.im = Some("IMPROVEMENT_PASTURE".to_string());
    pasture.p = true;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 60,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![center, pasture],
    }]);
    let state = StateSnapshot {
        turn: 60,
        cities: vec![StateCity {
            id: 10,
            name: "Aquileia".to_string(),
            x: 5,
            y: 4,
            pop: 1,
            loyalty: 100.0,
            worked: Some(vec![StateWorkedPlot {
                x: 6,
                y: 4,
                yields: None,
            }]),
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let pos = crate::hex::offset_to_axial(6, 4);
    let tile = recon
        .game
        .map
        .get(pos)
        .expect("the pasture plot is on the board");
    assert_eq!(tile.improvement.as_deref(), Some("pasture"));
    assert!(tile.pillaged, "the host's pillage bit must reach the tile");
    // Pillaged, the pasture's Production stops: plains + horses only.
    let paid = recon.game.modeled_tile_yields(pos);
    let mut unpillaged = tile.clone();
    unpillaged.pillaged = false;
    let full = recon.game.rules.tile_yields(&unpillaged);
    assert!(
        paid.production + 1.0 - full.production < 1e-9
            && full.production - paid.production >= 1.0 - 1e-9,
        "pillaged {paid:?} vs standing {full:?}"
    );
}

/// ★★★★ THE AGE AND ITS DEDICATIONS CROSS THE BRIDGE.
///
/// The three age flags were exported and read by nothing, so every mirrored
/// board sat in a Normal Age and no Dedication ever paid. Heartbeat of Steam
/// ("+10 from Campus" under Production in the host's own ledger, run
/// civvis-20260816T132247Z) was the largest gap of that game's Golden Age.
#[test]
fn the_age_and_its_dedications_reach_the_seat() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 180,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 180,
        golden_age: Some(true),
        dark_age: Some(false),
        heroic_golden_age: Some(false),
        dedications: Some(vec![
            "COMMEMORATION_INDUSTRIAL".to_string(),
            "COMMEMORATION_ECONOMIC".to_string(),
            "COMMEMORATION_NOT_A_THING".to_string(),
        ]),
        dedication_choices: Some(1),
        ..StateSnapshot::default()
    };
    let mut mirror = LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
    assert_eq!(mirror.game.players[0].age, "golden");
    assert_eq!(mirror.game.players[0].dedication_choices, 1);
    assert!(mirror.game.players[0]
        .dedications
        .contains("heartbeat_of_steam"));
    assert!(mirror.game.players[0]
        .dedications
        .contains("reform_the_coinage"));
    assert_eq!(
        mirror.game.players[0].dedications.len(),
        2,
        "unknown types are dropped"
    );

    // The age turns over: the sync follows the flags, heroic outranking golden.
    state.turn = 181;
    state.heroic_golden_age = Some(true);
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.players[0].age, "heroic");
    state.turn = 182;
    state.heroic_golden_age = Some(false);
    state.golden_age = Some(false);
    state.dark_age = Some(true);
    state.dedications = Some(vec![]);
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.players[0].age, "dark");
    assert!(mirror.game.players[0].dedications.is_empty());
    // An older export says nothing and changes nothing.
    state.turn = 183;
    state.dark_age = None;
    state.golden_age = None;
    state.heroic_golden_age = None;
    state.dedications = None;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.players[0].age, "dark");
}

/// ★★★★★ A CORRECTION IS MEASURED AFTER EVERYTHING IT CORRECTS FOR IS ON THE BOARD.
///
/// The rival's per-turn correction was derived before the loop that writes
/// a rival city's Population (planted at one) — measured against a size-one
/// city, paid on the size-eleven one: Nubia read 174 Science against the
/// host's 141 on run civvis-20260816T175306Z. And the seat's own Dedications
/// were applied after its correction: Ravenna read 14.5 Science against 9.5.
/// Both boards must read the host's figure exactly, rebuild and sync alike.
#[test]
fn corrections_are_measured_after_population_and_dedications_are_on_the_board() {
    let side = 16;
    let plots: Vec<Plot> = (0..side)
        .flat_map(|x| {
            (0..side).map(move |y| Plot {
                x,
                y,
                im: None,
                t: Some("TERRAIN_GRASS".to_string()),
                f: None,
                r: None,
                o: if (x, y) == (3, 3) {
                    0
                } else if (x, y) == (11, 11) {
                    3
                } else {
                    -1
                },
                w: false,
                i: false,
                fw: None,
                rv: 0,
                ri: false,
                ct: None,
                cl: -1,
                p: false,
                d: None,
                dc: None,
                wo: None,
                rt: None,
                rp: false,
                yl: None,
                ap: None,
                np: false,
                vis: false,
            })
        })
        .collect();
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 90,
        width: side,
        height: side,
        chunk: 1,
        plots,
    }]);
    let mut state = StateSnapshot {
        turn: 90,
        science: Some(30.0),
        culture: Some(12.0),
        golden_age: Some(true),
        dark_age: Some(false),
        heroic_golden_age: Some(false),
        dedications: Some(vec!["COMMEMORATION_SCIENTIFIC".to_string()]),
        cities: vec![StateCity {
            id: 1,
            name: "Rome".to_string(),
            x: 3,
            y: 3,
            pop: 6,
            loyalty: 100.0,
            capital: true,
            districts: vec![StateDistrict {
                kind: "DISTRICT_COMMERCIAL_HUB".to_string(),
                x: 4,
                y: 3,
                complete: true,
                ..StateDistrict::default()
            }],
            yields: Some(crate::rules::Yields {
                food: 20.0,
                production: 9.0,
                gold: 8.0,
                science: 9.5,
                culture: 6.0,
                faith: 0.0,
            }),
            ..StateCity::default()
        }],
        rivals: vec![StateRival {
            player: 3,
            civ: "CIVILIZATION_NUBIA".to_string(),
            science: 41.0,
            culture: 22.0,
            cities: vec![StateCity {
                id: 3,
                name: "Meroe".to_string(),
                x: 11,
                y: 11,
                pop: 11,
                loyalty: 100.0,
                capital: true,
                ..StateCity::default()
            }],
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    let seat_yields = |game: &crate::game::Game, seat: usize| {
        let mut total = crate::rules::Yields::default();
        for cid in game.player_city_ids(seat) {
            total.add(game.city_yields(cid));
        }
        if let Some(adjustment) = game.observed_yield_adjustments.get(&seat) {
            total.add(*adjustment);
        }
        total.add(game.player_yield_extras(seat));
        total
    };
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let rome = mirror.game.player_city_ids(0)[0];
    assert!(
        (mirror.game.city_yields(rome).science - 9.5).abs() < 1e-9,
        "the city reads the host after its Dedication is on the seat: {:?}",
        mirror.game.city_yields(rome)
    );
    assert!((seat_yields(&mirror.game, 0).science - 30.0).abs() < 1e-9);
    let meroe = mirror.game.player_city_ids(1)[0];
    assert_eq!(mirror.game.cities[&meroe].pop, 11);
    assert!(
        (seat_yields(&mirror.game, 1).science - 41.0).abs() < 1e-9,
        "the rival seat reads the host after its city's Population is on the board: {:?}",
        seat_yields(&mirror.game, 1)
    );

    // And after a sync that grows the rival and moves our Dedication.
    state.turn = 91;
    state.rivals[0].cities[0].pop = 14;
    state.rivals[0].science = 47.0;
    state.dedications = Some(vec!["COMMEMORATION_INDUSTRIAL".to_string()]);
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.cities[&meroe].pop, 14);
    assert!((seat_yields(&mirror.game, 1).science - 47.0).abs() < 1e-9);
    assert!((mirror.game.city_yields(rome).science - 9.5).abs() < 1e-9);
}

#[test]
fn a_rivals_route_into_our_city_is_seated_and_the_hosts_trade_policy_pays_it_before_the_correction()
{
    let side = 16;
    let plots: Vec<Plot> = (0..side)
        .flat_map(|x| {
            (0..side).map(move |y| Plot {
                x,
                y,
                im: None,
                t: Some("TERRAIN_GRASS".to_string()),
                f: None,
                r: None,
                o: if (x, y) == (3, 3) {
                    0
                } else if (x, y) == (11, 11) {
                    3
                } else {
                    -1
                },
                w: false,
                i: false,
                fw: None,
                rv: 0,
                ri: false,
                ct: None,
                cl: -1,
                p: false,
                d: None,
                dc: None,
                wo: None,
                rt: None,
                rp: false,
                yl: None,
                ap: None,
                np: false,
                vis: false,
            })
        })
        .collect();
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 90,
        width: side,
        height: side,
        chunk: 1,
        plots,
    }]);
    let host_gold = 12.0;
    let mut state = StateSnapshot {
        turn: 90,
        science: Some(30.0),
        culture: Some(12.0),
        resolutions: Some(vec![
            StateResolution {
                kind: "WC_RES_TRADE_TREATY".to_string(),
                option: 1,
                target: "0".to_string(),
            },
            StateResolution {
                kind: "WC_RES_BORDER_CONTROL".to_string(),
                option: 1,
                target: "1".to_string(),
            },
            StateResolution {
                kind: "WC_RES_LUXURY".to_string(),
                option: 2,
                target: "RESOURCE_SILK".to_string(),
            },
            StateResolution {
                kind: "WC_RES_ARMS_CONTROL".to_string(),
                option: 1,
                target: "".to_string(),
            },
            StateResolution {
                kind: "WC_RES_DIPLOVICTORY".to_string(),
                option: 2,
                target: "3".to_string(),
            },
        ]),
        congress_turns_left: Some(11),
        cities: vec![StateCity {
            id: 1,
            name: "Cumae".to_string(),
            x: 3,
            y: 3,
            pop: 6,
            loyalty: 100.0,
            capital: true,
            yields: Some(crate::rules::Yields {
                food: 20.0,
                production: 9.0,
                gold: host_gold,
                science: 9.5,
                culture: 6.0,
                faith: 0.0,
            }),
            incoming_routes: Some(StateIncomingRoutes {
                foreign: 1,
                domestic: 0,
                origins: vec![StateRouteOrigin {
                    x: 11,
                    y: 11,
                    player: 3,
                }],
            }),
            ..StateCity::default()
        }],
        rivals: vec![StateRival {
            player: 3,
            civ: "CIVILIZATION_MAORI".to_string(),
            cities: vec![StateCity {
                id: 3,
                name: "Auckland".to_string(),
                x: 11,
                y: 11,
                pop: 8,
                loyalty: 100.0,
                capital: true,
                ..StateCity::default()
            }],
            ..StateRival::default()
        }],
        congress_dvp: Some(StateCongressDvp {
            turn: Some(90),
            points: vec![
                StateCongressDvpEntry {
                    player: 0,
                    points: 0,
                },
                StateCongressDvpEntry {
                    player: 1,
                    points: 1,
                },
                StateCongressDvpEntry {
                    player: 3,
                    points: 0,
                },
            ],
        }),
        ..StateSnapshot::default()
    };
    state.seat.local_player = 0;
    state.seat.players = 4;
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let cumae = mirror.game.player_city_ids(0)[0];
    let auckland = mirror.game.player_city_ids(3)[0];
    // The route is on the board, owned by the rival's SEAT, from its city.
    let seated: Vec<_> = mirror
        .game
        .routes
        .iter()
        .filter(|route| route.dest == cumae)
        .collect();
    assert_eq!(
        seated.len(),
        1,
        "one incoming route: {:?}",
        mirror.game.routes
    );
    assert_eq!(seated[0].origin, auckland);
    assert_eq!(seated[0].owner, 3);
    assert_eq!(
        mirror.game.observed_incoming_route_deltas.get(&cumae),
        Some(&(0, 0)),
        "a fully seated route needs no incoming-count correction"
    );
    // The host's Congress is the model's Congress: Trade Policy A on our
    // seat, Luxury Policy B on silk, and the resolution the model has no
    // rule for is reported rather than guessed. Diplomatic Victory is the
    // exception: its standing is imported through `congress_dvp`, so the
    // resolution row is a known no-op for active model effects.
    assert!(mirror.game.congress_effect_active("trade_policy", "A", "0"));
    assert!(mirror
        .game
        .congress_effect_active("border_control_treaty", "A", "1"));
    assert!(mirror
        .game
        .congress_effect_active("luxury_policy", "B", "silk"));
    assert_eq!(mirror.game.active_congress_effects.len(), 3);
    assert_eq!(mirror.game.active_congress_effects[0].expires, 90 + 11 + 1);
    assert!(
        mirror
            .unmapped
            .iter()
            .any(|issue| issue == "congress:WC_RES_ARMS_CONTROL:1:"),
        "unmapped: {:?}",
        mirror.unmapped
    );
    assert!(
        !mirror
            .unmapped
            .iter()
            .any(|issue| issue.starts_with("congress:WC_RES_DIPLOVICTORY:")),
        "a resolution represented by congress_dvp is not a bridge gap: {:?}",
        mirror.unmapped
    );
    // The model pays the +4 itself, so the correction it derives is the
    // host's number minus a model that already includes it — the city reads
    // the host either way, and the model's own view carries the treaty.
    assert!((mirror.game.city_yields(cumae).gold - host_gold).abs() < 1e-9);
    let model = mirror.game.city_yields_model(cumae).gold;
    mirror.game.active_congress_effects.clear();
    let without_treaty = mirror.game.city_yields_model(cumae).gold;
    assert!(
        (model - without_treaty - 4.0).abs() < 1e-9,
        "Trade Policy A pays the destination +4 per incoming foreign route: {} vs {}",
        model,
        without_treaty
    );
    mirror.game.routes.clear();
    let without_route = mirror.game.city_yields_model(cumae).gold;
    assert!(without_route <= without_treaty);

    // The next sync re-seats the route and re-reads the Congress; when the
    // host drops both, the board follows and the correction stays honest.
    state.turn = 91;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(
        mirror
            .game
            .routes
            .iter()
            .filter(|route| route.dest == cumae)
            .count(),
        1
    );
    // An incoming route may remain visible at the destination while its
    // origin city is outside the seat's visibility. Keep the authoritative
    // count even though there is no safe city entity to invent for it.
    state.cities[0].incoming_routes = Some(StateIncomingRoutes {
        foreign: 1,
        domestic: 0,
        origins: vec![StateRouteOrigin {
            x: 19,
            y: 19,
            player: 42,
        }],
    });
    state.turn = 92;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(
        mirror
            .game
            .routes
            .iter()
            .filter(|route| route.dest == cumae)
            .count(),
        0,
        "an unseen origin is not guessed into a city"
    );
    assert_eq!(
        mirror.game.observed_incoming_route_deltas.get(&cumae),
        Some(&(1, 0))
    );
    let with_unseen_route = mirror.game.city_yields_model(cumae).gold;
    std::sync::Arc::make_mut(&mut mirror.game.observed_incoming_route_deltas).clear();
    let without_unseen_route = mirror.game.city_yields_model(cumae).gold;
    assert!(
        (with_unseen_route - without_unseen_route - 4.0).abs() < 1e-9,
        "Trade Policy A must count a foreign route whose origin is fogged: {} vs {}",
        with_unseen_route,
        without_unseen_route
    );
    // Restore the observation before the ordinary no-route sync below.
    state.cities[0].incoming_routes = Some(StateIncomingRoutes::default());
    assert!(mirror.game.congress_effect_active("trade_policy", "A", "0"));
    state.resolutions = Some(vec![]);
    state.turn = 93;
    mirror.sync(&snapshot, &state, 0);
    assert!(mirror.game.active_congress_effects.is_empty());
    assert_eq!(
        mirror
            .game
            .routes
            .iter()
            .filter(|route| route.dest == cumae)
            .count(),
        0
    );
    assert!((mirror.game.city_yields(cumae).gold - host_gold).abs() < 1e-9);
    // An older export (no `resolutions`) leaves the model's own Congress alone.
    state.turn = 93;
    state.resolutions = None;
    mirror
        .game
        .active_congress_effects
        .push(crate::game::CongressEffect {
            resolution: "patronage".to_string(),
            outcome: "A".to_string(),
            target: "scientist".to_string(),
            expires: 200,
        });
    mirror.sync(&snapshot, &state, 0);
    assert!(mirror
        .game
        .congress_effect_active("patronage", "A", "scientist"));
}

#[test]
fn host_resolutions_translate_into_the_models_congress_vocabulary() {
    let rules = crate::rules::Rules::shipped();
    let seats: std::collections::BTreeMap<usize, usize> = [(0, 0), (5, 2)].into_iter().collect();
    let map = |kind: &str, option: i64, target: &str| {
        civvis_congress_effect(
            &rules,
            &StateResolution {
                kind: kind.to_string(),
                option,
                target: target.to_string(),
            },
            &seats,
            120,
        )
        .map(|effect| (effect.resolution, effect.outcome, effect.target))
    };
    assert_eq!(
        map("WC_RES_TRADE_TREATY", 1, "5"),
        Some(("trade_policy".into(), "A".into(), "2".into()))
    );
    assert_eq!(
        map("WC_RES_TRADE_TREATY", 2, "0"),
        Some(("trade_policy".into(), "B".into(), "0".into()))
    );
    assert_eq!(
        map("WC_RES_TRADE_TREATY", 1, "9"),
        None,
        "an unseated player is not guessed"
    );
    assert_eq!(
        map("WC_RES_MERCENARY_COMPANIES", 1, "YIELD_PRODUCTION"),
        Some((
            "mercenary_companies".into(),
            "A".into(),
            "production".into()
        ))
    );
    assert_eq!(
        map("WC_RES_LUXURY", 1, "RESOURCE_WHALES"),
        Some(("luxury_policy".into(), "A".into(), "whales".into()))
    );
    assert_eq!(
        map("WC_RES_LUXURY", 2, "LOC_RESOURCE_TEA_NAME"),
        Some(("luxury_policy".into(), "B".into(), "tea".into()))
    );
    assert_eq!(
        map("WC_RES_URBAN_DEVELOPMENT", 2, "DISTRICT_CAMPUS"),
        Some((
            "urban_development_treaty".into(),
            "B".into(),
            "campus".into()
        ))
    );
    assert_eq!(
        map(
            "WC_RES_URBAN_DEVELOPMENT",
            1,
            "LOC_DISTRICT_GOVERNMENT_NAME"
        ),
        Some((
            "urban_development_treaty".into(),
            "A".into(),
            "government_plaza".into()
        ))
    );
    assert_eq!(
        map("WC_RES_URBAN_DEVELOPMENT", 1, "DISTRICT_CITY_CENTER"),
        Some((
            "urban_development_treaty".into(),
            "A".into(),
            "city_center".into()
        ))
    );
    assert_eq!(
        map("WC_RES_PATRONAGE", 1, "GREAT_PERSON_CLASS_SCIENTIST"),
        Some(("patronage".into(), "A".into(), "scientist".into()))
    );
    assert_eq!(
        map("WC_RES_MILITARY_ADVISORY", 2, "PROMOTION_CLASS_MELEE"),
        Some(("military_advisory".into(), "B".into(), "melee".into()))
    );
    assert_eq!(
        map(
            "WC_RES_MILITARY_ADVISORY",
            1,
            "LOC_PROMOTION_CLASS_MELEE_NAME"
        ),
        Some(("military_advisory".into(), "A".into(), "melee".into()))
    );
    assert_eq!(
        map("WC_RES_ESPIONAGE_PACT", 1, "UNITOPERATION_SPY_SIPHON_FUNDS"),
        Some(("espionage_pact".into(), "A".into(), "siphon_funds".into()))
    );
    assert_eq!(
        map(
            "WC_RES_ESPIONAGE_PACT",
            1,
            "LOC_UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR_DESCRIPTION"
        ),
        Some((
            "espionage_pact".into(),
            "A".into(),
            "neutralize_governor".into()
        ))
    );
    assert_eq!(
        map("WC_RES_HERITAGE_ORG", 1, "GREATWORKOBJECT_WRITING"),
        Some(("heritage_organization".into(), "A".into(), "writing".into()))
    );
    assert_eq!(
        map(
            "WC_RES_HERITAGE_ORG",
            1,
            "LOC_GREAT_WORK_OBJECT_WRITING_NAME"
        ),
        Some(("heritage_organization".into(), "A".into(), "writing".into())),
        "the host's localized Great Work key uses underscores in OBJECT"
    );
    assert_eq!(
        map("WC_RES_DEFORESTATION_TREATY", 1, "FEATURE_FOREST"),
        Some(("deforestation_treaty".into(), "A".into(), "forest".into()))
    );
    assert_eq!(
        map("WC_RES_DEFORESTATION_TREATY", 2, "FEATURE_JUNGLE"),
        Some(("deforestation_treaty".into(), "B".into(), "jungle".into()))
    );
    assert_eq!(
        map("WC_RES_HERITAGE_ORG", 2, "GREATWORKOBJECT_SCULPTURE"),
        Some(("heritage_organization".into(), "B".into(), "art".into()))
    );
    assert_eq!(
        map("WC_RES_MILITARY_ADVISORY", 1, "PROMOTION_CLASS_APOSTLE"),
        Some((
            "military_advisory".into(),
            "A".into(),
            "religious_apostle".into()
        ))
    );
    assert_eq!(
        map(
            "WC_RES_GLOBAL_ENERGY_TREATY",
            1,
            "BUILDING_FOSSIL_FUEL_POWER_PLANT"
        ),
        Some((
            "global_energy_treaty".into(),
            "A".into(),
            "oil_power_plant".into()
        ))
    );
    assert_eq!(
        map(
            "WC_RES_GLOBAL_ENERGY_TREATY",
            2,
            "LOC_BUILDING_POWER_PLANT_EXPANSION2_NAME"
        ),
        Some((
            "global_energy_treaty".into(),
            "B".into(),
            "nuclear_power_plant".into()
        )),
        "Expansion2 reports the nuclear plant through its localized display key"
    );
    assert_eq!(
        map("WC_RES_WORLD_IDEOLOGY", 1, "GOVERNMENT_DEMOCRACY"),
        Some(("world_ideology".into(), "A".into(), "democracy".into()))
    );
    assert_eq!(
        map("WC_RES_PUBLIC_WORKS", 1, "PROJECT_MANHATTAN_PROJECT"),
        Some((
            "public_works_program".into(),
            "A".into(),
            "manhattan_project".into()
        ))
    );
    assert_eq!(
        map("WC_RES_TRADE_TREATY", 0, "0"),
        None,
        "an option the mod could not read is not guessed"
    );
    assert_eq!(
        map("WC_RES_SOVEREIGNTY", 1, "MINOR_CIV_TRADE"),
        Some(("sovereignty".into(), "A".into(), "trade".into()))
    );
    assert_eq!(
        map(
            "WC_RES_SOVEREIGNTY",
            2,
            "LOC_MINOR_CIV_BONUS_SCIENTIFIC_NAME"
        ),
        Some(("sovereignty".into(), "B".into(), "scientific".into()))
    );
    assert_eq!(
        map("WC_RES_DIPLOVICTORY", 2, "4"),
        None,
        "Diplomatic Victory is represented by the separate congress standing export"
    );
}

#[test]
fn observed_worker_swap_overrides_the_nearest_city_guess() {
    let mut first_center = plot(2, 2, "TERRAIN_PLAINS");
    first_center.o = 0;
    let mut second_center = plot(6, 2, "TERRAIN_PLAINS");
    second_center.o = 0;
    let mut swapped = plot(3, 2, "TERRAIN_GRASS");
    swapped.o = 0;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 70,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![first_center, second_center, swapped],
    }]);
    let state = StateSnapshot {
        turn: 70,
        cities: vec![
            StateCity {
                id: 1,
                name: "Rome".to_string(),
                x: 2,
                y: 2,
                pop: 2,
                worked: Some(vec![]),
                ..StateCity::default()
            },
            StateCity {
                id: 2,
                name: "Lugdunum".to_string(),
                x: 6,
                y: 2,
                pop: 2,
                worked: Some(vec![StateWorkedPlot {
                    x: 3,
                    y: 2,
                    yields: None,
                }]),
                ..StateCity::default()
            },
        ],
        ..StateSnapshot::default()
    };

    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let first = recon
        .game
        .city_at(crate::hex::offset_to_axial(2, 2))
        .unwrap();
    let second = recon
        .game
        .city_at(crate::hex::offset_to_axial(6, 2))
        .unwrap();
    let worked = crate::hex::offset_to_axial(3, 2);

    assert_eq!(
        recon.game.city_citizen_plan(second).worked_tiles,
        vec![worked]
    );
    assert_eq!(recon.game.map.tiles[&worked].owner_city, Some(second));
    assert!(!recon.game.cities[&first].owned_tiles.contains(&worked));
    assert!(recon.game.cities[&second].owned_tiles.contains(&worked));
}

#[test]
fn firaxis_city_center_is_implicit_and_palace_yields_are_counted_once() {
    let mut center = plot(5, 4, "TERRAIN_PLAINS");
    center.o = 0;
    let mut worked = plot(6, 4, "TERRAIN_GRASS");
    worked.o = 0;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 3,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![center, worked],
    }]);
    let state = StateSnapshot {
        turn: 3,
        seat: Seat {
            civ: "CIVILIZATION_CHINA".to_string(),
            leader: "LEADER_QIN_SHI_HUANG".to_string(),
            ..Seat::default()
        },
        cities: vec![StateCity {
            id: 10,
            name: "Xi'an".to_string(),
            x: 5,
            y: 4,
            pop: 1,
            capital: true,
            buildings: vec!["BUILDING_PALACE".to_string()],
            worked: Some(vec![
                StateWorkedPlot {
                    x: 5,
                    y: 4,
                    yields: None,
                },
                StateWorkedPlot {
                    x: 6,
                    y: 4,
                    yields: None,
                },
            ]),
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };

    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let cid = recon.game.player_city_ids(0)[0];
    assert_eq!(
        recon.game.city_citizen_plan(cid).worked_tiles,
        vec![crate::hex::offset_to_axial(6, 4)],
        "Firaxis's explicit city centre is not a second citizen assignment"
    );
    assert!(
        !recon.unmapped.contains(&"Xi'an:worked_plot".to_string()),
        "the host's normal GetWorkedPlots shape must be accepted"
    );
    assert!(
        !recon.game.cities[&cid]
            .buildings
            .contains(&crate::name!("palace")),
        "the intrinsic Palace must not also enter the ordinary building list"
    );
    let mut without_explicit_palace = state;
    without_explicit_palace.cities[0].buildings.clear();
    let control = rebuild_from_state(&snapshot, &without_explicit_palace, 2, 1, 250, 0);
    let control_city = control.game.player_city_ids(0)[0];
    assert_eq!(
        recon.game.city_yields_model(cid),
        control.game.city_yields_model(control_city),
        "Firaxis's explicit Palace row must not add a second copy of its yields"
    );
}

#[test]
fn met_city_state_is_an_actor_instead_of_anonymous_blocked_land() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(6, 6, "TERRAIN_PLAINS"), plot(7, 6, "TERRAIN_PLAINS")],
    }]);
    let state = StateSnapshot {
        turn: 30,
        minors: vec![StateMinor {
            player: 6,
            civ: "CIVILIZATION_KABUL".to_string(),
            score: 91,
            military: 74.0,
            suzerain: 0,
            envoys: 3,
            cities: vec![StateCity {
                id: 70,
                name: "Kabul".to_string(),
                x: 6,
                y: 6,
                pop: 4,
                defense: 28.0,
                ..StateCity::default()
            }],
            units: vec![StateUnit {
                id: 71,
                kind: "UNIT_WARRIOR".to_string(),
                x: 7,
                y: 6,
                hp: 100.0,
                ..StateUnit::default()
            }],
            ..StateMinor::default()
        }],
        ..StateSnapshot::default()
    };

    let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
    let minor = recon
        .game
        .players
        .iter()
        .find(|player| player.is_minor && player.civ == "Kabul")
        .expect("Kabul minor seat");
    assert!(recon.game.has_met(0, minor.id));
    assert_eq!(recon.game.score(minor.id), 91);
    assert_eq!(recon.game.military_power(minor.id), 74.0);
    assert_eq!(recon.game.envoys_at(0, minor.id), 3);
    assert_eq!(recon.game.suzerain_of(minor.id), Some(0));
    assert!(recon
        .game
        .cities
        .values()
        .any(|city| { city.owner == minor.id && city.name == "Kabul" }));
    assert!(recon.game.units.values().any(|unit| unit.owner == minor.id));
}

/// The host's climate crossed for the first time on 2026-08-26: `cl` had
/// been exported per plot with no phase to read it against, so the Flood
/// Barrier price, the clean-power premium and the flooding of the bands
/// were all priced at phase 0 on every live turn. The flooded bands are
/// the shipped `CoastalLowlands` rows (1 m at rise 2, 2 m at 3, 3 m at 5).
#[test]
fn the_hosts_climate_level_is_the_boards_phase_and_floods_the_bands_it_names() {
    let mut one = plot(6, 7, "TERRAIN_PLAINS");
    one.cl = 1;
    let mut two = plot(7, 7, "TERRAIN_PLAINS");
    two.cl = 2;
    let mut three = plot(8, 7, "TERRAIN_PLAINS");
    three.cl = 3;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(6, 6, "TERRAIN_PLAINS"), one, two, three],
    }]);
    let mut state = StateSnapshot {
        turn: 30,
        climate: Some(StateClimate {
            level: 3,
            co2_ours: Some(1_200.0),
            co2_total: Some(9_000.0),
            storm_pct: Some(12.5),
            sea_level_turns: Some(17),
            ..StateClimate::default()
        }),
        ..StateSnapshot::default()
    };
    let flooded = |game: &crate::game::Game, x: i32, y: i32| {
        game.map
            .get(crate::hex::offset_to_axial(x, y))
            .expect("the plot is on the board")
            .flooded
    };
    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    assert_eq!(recon.game.climate_phase, 3);
    assert_eq!(recon.game.players[0].co2_emissions, 1_200.0);
    assert_eq!(
        recon.game.global_co2_emissions(),
        9_000.0,
        "the world's CO2 is the host's total, not ours alone"
    );
    assert_eq!(
        recon
            .game
            .observed_climate
            .as_ref()
            .and_then(|climate| climate.sea_level_turns),
        Some(17)
    );
    assert!(flooded(&recon.game, 6, 7), "the 1 m band floods at level 2");
    assert!(flooded(&recon.game, 7, 7), "the 2 m band floods at level 3");
    assert!(
        !flooded(&recon.game, 8, 7),
        "the 3 m band waits for level 5"
    );

    // The same on the live path; a later export without the key leaves it.
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    assert_eq!(mirror.game.climate_phase, 3);
    state.climate = None;
    state.turn = 31;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.climate_phase, 3);
    assert!(flooded(&mirror.game, 7, 7));

    // And an export that never carried it is the pre-industrial board it was.
    let bare = rebuild_from_state(
        &snapshot,
        &StateSnapshot {
            turn: 30,
            ..StateSnapshot::default()
        },
        4,
        1,
        250,
        0,
    );
    assert_eq!(bare.game.climate_phase, 0);
    assert!(!flooded(&bare.game, 6, 7));
    assert!(bare.game.observed_climate.is_none());
}

/// City facts and climate must be in place before host-to-model yield
/// calibration. A fresh rebuild initially gives the first planted city the
/// Palace, while the host can name a later city as capital; the same rebuild
/// can flood a lowland city centre when it applies the host climate phase.
#[test]
fn city_yield_calibration_follows_capital_and_climate_state() {
    let mut first = plot(5, 4, "TERRAIN_PLAINS");
    first.o = 0;
    let mut capital = plot(8, 4, "TERRAIN_PLAINS");
    capital.o = 0;
    capital.cl = 1;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![first, capital],
    }]);
    let host_yields = crate::rules::Yields {
        food: 7.0,
        production: 6.0,
        gold: 4.0,
        science: 3.0,
        culture: 2.0,
        faith: 1.0,
    };
    let state = StateSnapshot {
        turn: 30,
        climate: Some(StateClimate {
            level: 2,
            ..StateClimate::default()
        }),
        cities: vec![
            StateCity {
                id: 10,
                name: "First City".to_string(),
                x: 5,
                y: 4,
                pop: 1,
                loyalty: 100.0,
                capital: false,
                yields: Some(crate::rules::Yields::default()),
                ..StateCity::default()
            },
            StateCity {
                id: 11,
                name: "Moved Palace".to_string(),
                x: 8,
                y: 4,
                pop: 1,
                loyalty: 100.0,
                capital: true,
                yields: Some(host_yields),
                ..StateCity::default()
            },
        ],
        ..StateSnapshot::default()
    };

    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let city = recon
        .game
        .cities
        .values()
        .find(|city| city.name == "Moved Palace")
        .expect("the moved capital is mirrored");
    assert!(city.is_capital);
    assert!(
        recon.game.map.get(city.pos).unwrap().flooded,
        "the host climate phase is applied before calibration"
    );
    assert_eq!(recon.game.city_yields(city.id), host_yields);
}

/// The board rolled its own quest for every pair from a hash; the host's
/// actual request never crossed. Now it seats on the pair where
/// `city_state_quest` and the `quest-*` genes read it.
#[test]
fn a_host_quest_seats_the_city_states_request_on_the_pair() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(6, 6, "TERRAIN_PLAINS"), plot(12, 6, "TERRAIN_PLAINS")],
    }]);
    let minor =
        |player: usize, civ: &str, x: i32, y: i32, quests: Option<Vec<StateQuest>>| StateMinor {
            player,
            civ: civ.to_string(),
            cities: vec![StateCity {
                id: 70,
                name: civ.to_string(),
                x,
                y,
                pop: 3,
                ..StateCity::default()
            }],
            quests,
            ..StateMinor::default()
        };
    let mut state = StateSnapshot {
        turn: 30,
        minors: vec![
            minor(
                6,
                "CIVILIZATION_KABUL",
                6,
                6,
                Some(vec![StateQuest {
                    kind: "QUEST_TRAIN_UNIT_TYPE".to_string(),
                    target: Some("UNIT_SWORDSMAN".to_string()),
                    name: None,
                }]),
            ),
            minor(
                7,
                "CIVILIZATION_ZANZIBAR",
                12,
                6,
                Some(vec![StateQuest {
                    kind: "QUEST_SEND_TRADE_ROUTE".to_string(),
                    target: None,
                    name: None,
                }]),
            ),
        ],
        ..StateSnapshot::default()
    };
    let seat = |game: &crate::game::Game, civ: &str| {
        game.players
            .iter()
            .find(|player| player.is_minor && player.civ == civ)
            .map(|player| player.id)
            .unwrap_or_else(|| panic!("{civ} has a seat"))
    };
    let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
    let kabul = seat(&recon.game, "Kabul");
    let zanzibar = seat(&recon.game, "Zanzibar");
    let quest = recon.game.city_state_quest(0, kabul).expect("Kabul asks");
    assert_eq!(quest.kind, "train_unit_type");
    assert_eq!(
        quest.target, "swordsman",
        "the host's UNIT_ name is the board's"
    );
    assert_eq!(quest.era, recon.game.world_era);
    assert_eq!(
        recon
            .game
            .city_state_quest(0, zanzibar)
            .map(|quest| quest.kind.as_str()),
        Some("send_trade_route")
    );

    let mut mirror = LiveMirror::new(&snapshot, &state, 6, 1, 250, 0);
    let kabul = seat(&mirror.game, "Kabul");
    let zanzibar = seat(&mirror.game, "Zanzibar");
    assert_eq!(
        mirror
            .game
            .city_state_quest(0, kabul)
            .map(|quest| quest.target.as_str()),
        Some("swordsman")
    );
    state.minors[0].quests = Some(Vec::new());
    state.minors[1].quests = None;
    state.turn = 31;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        mirror.game.city_state_quest(0, kabul).is_none(),
        "an empty list is a city-state asking nothing"
    );
    assert_eq!(
        mirror
            .game
            .city_state_quest(0, zanzibar)
            .map(|quest| quest.kind.as_str()),
        Some("send_trade_route"),
        "a missing key leaves the request that stood"
    );
}

/// `envoys` and `most_envoys` said ours and the leader's; a rival's
/// delegation was seeded as the minimum that elects the Suzerain the host
/// names, so the board could never tell one envoy from five, nor see that
/// it stood one short of a suzerainty.
#[test]
fn rival_envoy_counts_cross_and_a_missing_list_keeps_the_minimum_winning_seed() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(6, 6, "TERRAIN_PLAINS")],
    }]);
    let state =
        |envoys: i64, suzerain: i32, most: i64, counts: Option<Vec<(i64, i64)>>| StateSnapshot {
            turn: 30,
            rivals: vec![StateRival {
                player: 3,
                civ: "CIVILIZATION_SCYTHIA".to_string(),
                leader: "LEADER_TOMYRIS".to_string(),
                ..StateRival::default()
            }],
            minors: vec![StateMinor {
                player: 6,
                civ: "CIVILIZATION_KABUL".to_string(),
                suzerain,
                envoys,
                most_envoys: most,
                envoys_by_player: counts.map(|counts| {
                    counts
                        .into_iter()
                        .map(|(player, envoys)| StateEnvoyCount { player, envoys })
                        .collect()
                }),
                cities: vec![StateCity {
                    id: 70,
                    name: "Kabul".to_string(),
                    x: 6,
                    y: 6,
                    pop: 4,
                    ..StateCity::default()
                }],
                ..StateMinor::default()
            }],
            ..StateSnapshot::default()
        };
    let seat = |game: &crate::game::Game| {
        game.players
            .iter()
            .find(|player| player.is_minor && player.civ == "Kabul")
            .map(|player| player.id)
            .expect("Kabul has a seat")
    };
    // Scythia holds five and we hold two: the board reads five, not the
    // minimum that elects her.
    let recon = rebuild_from_state(
        &snapshot,
        &state(2, 3, 5, Some(vec![(0, 2), (3, 5)])),
        6,
        1,
        250,
        0,
    );
    let kabul = seat(&recon.game);
    assert_eq!(
        recon.game.envoys_at(0, kabul),
        2,
        "our count is the seed beside this one"
    );
    assert_eq!(recon.game.envoys_at(1, kabul), 5);
    assert_eq!(recon.game.suzerain_of(kabul), Some(1));
    // Without the list the seed is what it was: the minimum winning delegation.
    let recon = rebuild_from_state(&snapshot, &state(2, 3, 0, None), 6, 1, 250, 0);
    assert_eq!(recon.game.envoys_at(1, seat(&recon.game)), 3);
    // No Suzerain while we hold four and Scythia one: the host's verdict
    // stands, seeded as a tie on the rival it counts highest, and a listed
    // one-envoy rival is not cleared to zero first.
    let recon = rebuild_from_state(
        &snapshot,
        &state(4, -1, 4, Some(vec![(0, 4), (3, 1)])),
        6,
        1,
        250,
        0,
    );
    let kabul = seat(&recon.game);
    assert_eq!(recon.game.suzerain_of(kabul), None);
    assert_eq!(recon.game.envoys_at(1, kabul), 4);
    // A tie the host reports crosses as the tie it is.
    let recon = rebuild_from_state(
        &snapshot,
        &state(3, -1, 3, Some(vec![(0, 3), (3, 3)])),
        6,
        1,
        250,
        0,
    );
    let kabul = seat(&recon.game);
    assert_eq!(recon.game.envoys_at(1, kabul), 3);
    assert_eq!(recon.game.suzerain_of(kabul), None);
    // The live path re-reads it: a lapsed delegation clears.
    let mut mirror = LiveMirror::new(
        &snapshot,
        &state(2, 3, 5, Some(vec![(0, 2), (3, 5)])),
        6,
        1,
        250,
        0,
    );
    let kabul = seat(&mirror.game);
    assert_eq!(mirror.game.envoys_at(1, kabul), 5);
    let mut later = state(2, -1, 2, Some(vec![(0, 2), (3, 0)]));
    later.turn = 31;
    mirror.sync(&snapshot, &later, 0);
    assert_eq!(mirror.game.envoys_at(1, kabul), 0);
    assert_eq!(mirror.game.suzerain_of(kabul), None);
}

/// The board derived appeal from the six neighbours it could see; the host
/// counts every modifier it has. A National Park is a plot flag on the
/// host and an improvement here.
#[test]
fn a_plots_host_appeal_and_national_park_stand_on_the_board() {
    let mut counted = plot(6, 6, "TERRAIN_PLAINS");
    counted.ap = Some(4);
    let mut park = plot(7, 6, "TERRAIN_PLAINS");
    park.np = true;
    park.ap = Some(-2);
    let uncounted = plot(12, 12, "TERRAIN_PLAINS");
    let chunk = |turn: u32, plots: Vec<Plot>| TilesChunk {
        turn,
        width: 20,
        height: 20,
        chunk: 1,
        plots,
    };
    let snapshot = Snapshot::from_chunks(&[chunk(30, vec![counted, park, uncounted.clone()])]);
    let state = StateSnapshot {
        turn: 30,
        ..StateSnapshot::default()
    };
    let at = |x: i32, y: i32| crate::hex::offset_to_axial(x, y);
    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    assert_eq!(
        recon.game.tile_appeal(at(6, 6)),
        4,
        "the host's count stands in for the derivation"
    );
    assert_eq!(recon.game.tile_appeal(at(7, 6)), -2);
    assert_eq!(
        recon
            .game
            .map
            .get(at(7, 6))
            .and_then(|tile| tile.improvement.as_deref()),
        Some("national_park")
    );
    let bare = rebuild_from_state(
        &Snapshot::from_chunks(&[chunk(
            30,
            vec![
                plot(6, 6, "TERRAIN_PLAINS"),
                plot(7, 6, "TERRAIN_PLAINS"),
                uncounted.clone(),
            ],
        )]),
        &state,
        4,
        1,
        250,
        0,
    );
    assert_eq!(
        recon.game.tile_appeal(at(12, 12)),
        bare.game.tile_appeal(at(12, 12)),
        "a plot without a reading keeps the board's own derivation"
    );
    assert!(bare.game.observed_appeal.is_empty());

    // The live path re-reads a later sweep, and a reading that lapsed is gone.
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    assert_eq!(mirror.game.tile_appeal(at(6, 6)), 4);
    let mut recount = plot(6, 6, "TERRAIN_PLAINS");
    recount.ap = Some(1);
    let later = Snapshot::from_chunks(&[chunk(
        31,
        vec![recount, plot(7, 6, "TERRAIN_PLAINS"), uncounted],
    )]);
    mirror.sync(
        &later,
        &StateSnapshot {
            turn: 31,
            ..StateSnapshot::default()
        },
        0,
    );
    assert_eq!(mirror.game.tile_appeal(at(6, 6)), 1);
    assert!(!mirror.game.observed_appeal.contains_key(&at(7, 6)));
    let saved: crate::game::Game =
        serde_json::from_str(&serde_json::to_string(&mirror.game).unwrap()).unwrap();
    assert_eq!(saved.observed_appeal.get(&at(6, 6)), Some(&1));
}

/// `IsRevealed` gated the record, so fog and sight were one state to the
/// board and the tactical layer re-derived sight on a reconstructed map.
#[test]
fn a_plot_the_host_shows_is_in_the_mirrored_seats_sight() {
    let mut shown = plot(15, 15, "TERRAIN_PLAINS");
    shown.vis = true;
    let fogged = plot(16, 15, "TERRAIN_PLAINS");
    let chunk = |turn: u32, plots: Vec<Plot>| TilesChunk {
        turn,
        width: 20,
        height: 20,
        chunk: 1,
        plots,
    };
    let snapshot = Snapshot::from_chunks(&[chunk(
        30,
        vec![plot(3, 3, "TERRAIN_PLAINS"), shown, fogged.clone()],
    )]);
    let state = StateSnapshot {
        turn: 30,
        ..StateSnapshot::default()
    };
    let at = |x: i32, y: i32| crate::hex::offset_to_axial(x, y);
    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    assert!(recon.game.host_observed.contains(&at(15, 15)));
    assert!(
        recon.game.player_can_see(0, at(15, 15)),
        "the host's sight reaches the seat's vision frame"
    );
    assert!(
        !recon.game.player_can_see(0, at(16, 15)),
        "revealed once is not in sight"
    );

    // The live path: a delta that drops the flag drops the sight.
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    assert!(mirror.game.player_can_see(0, at(15, 15)));
    let later = Snapshot::from_chunks(&[chunk(
        31,
        vec![
            plot(3, 3, "TERRAIN_PLAINS"),
            plot(15, 15, "TERRAIN_PLAINS"),
            fogged,
        ],
    )]);
    mirror.sync(
        &later,
        &StateSnapshot {
            turn: 31,
            ..StateSnapshot::default()
        },
        0,
    );
    assert!(!mirror.game.player_can_see(0, at(15, 15)));
    assert!(!mirror.game.host_observed.contains(&at(15, 15)));
}

/// The host prices every route a Trader could start while a slot is open;
/// the pair lands where the trader-destination chooser reads it and
/// lapses with the slot.
#[test]
fn a_route_options_host_yields_reach_the_board_keyed_by_the_pair_and_lapse_with_the_slot() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 20,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_PLAINS"), plot(9, 9, "TERRAIN_PLAINS")],
    }]);
    let host = crate::rules::Yields {
        food: 1.0,
        production: 0.0,
        gold: 7.0,
        science: 2.0,
        culture: 0.0,
        faith: 0.0,
    };
    let mut state = StateSnapshot {
        turn: 20,
        cities: vec![StateCity {
            id: 8,
            name: "Antium".to_string(),
            x: 5,
            y: 5,
            pop: 3,
            ..StateCity::default()
        }],
        // Firaxis allocates city ids per player: the same id as Antium.
        minors: vec![StateMinor {
            player: 6,
            civ: "CIVILIZATION_ZANZIBAR".to_string(),
            cities: vec![StateCity {
                id: 8,
                name: "Zanzibar".to_string(),
                x: 9,
                y: 9,
                pop: 3,
                ..StateCity::default()
            }],
            ..StateMinor::default()
        }],
        route_options: Some(vec![StateRouteOption {
            origin: 8,
            origin_x: 5,
            origin_y: 5,
            dest: 8,
            dest_player: 6,
            dest_x: 9,
            dest_y: 9,
            yields: Some(host),
        }]),
        ..StateSnapshot::default()
    };
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let origin = mirror
        .game
        .city_at(crate::hex::offset_to_axial(5, 5))
        .expect("Antium");
    let destination = mirror
        .game
        .city_at(crate::hex::offset_to_axial(9, 9))
        .expect("Zanzibar");
    assert_eq!(
        mirror
            .game
            .observed_route_options
            .get(&(origin, destination)),
        Some(&host),
        "coordinates resolve the pair despite the colliding city ids"
    );
    let saved: crate::game::Game =
        serde_json::from_str(&serde_json::to_string(&mirror.game).unwrap()).unwrap();
    assert_eq!(
        saved.observed_route_options.get(&(origin, destination)),
        Some(&host)
    );
    state.route_options = None;
    state.turn = 21;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        mirror.game.observed_route_options.is_empty(),
        "no open slot, no projection"
    );
}

/// ⚠ `suzerain: -1` is the export's NO-suzerain sentinel, and skipping the
/// seeding is not enough to mirror it: our own factual envoys are already
/// on the board and no rival delegation is, so three unopposed envoys
/// elect seat 0 by walkover. Measured live on `civvis-20260808T003040Z`:
/// `taruga suzerain Civ6=-1 CIVVIS=0`.
#[test]
fn no_suzerain_sentinel_does_not_elect_seat_zero_by_walkover() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(6, 6, "TERRAIN_PLAINS")],
    }]);
    let state = StateSnapshot {
        turn: 30,
        minors: vec![StateMinor {
            player: 6,
            civ: "CIVILIZATION_TARUGA".to_string(),
            suzerain: -1,
            envoys: 3,
            cities: vec![StateCity {
                id: 70,
                name: "Taruga".to_string(),
                x: 6,
                y: 6,
                pop: 4,
                ..StateCity::default()
            }],
            ..StateMinor::default()
        }],
        ..StateSnapshot::default()
    };

    let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
    let minor = recon
        .game
        .players
        .iter()
        .find(|player| player.is_minor && player.civ == "Taruga")
        .expect("Taruga minor seat");
    // Our delegation is the export's fact and must survive untouched…
    assert_eq!(recon.game.envoys_at(0, minor.id), 3);
    // …while the host's "none" answer must be the board's answer too.
    assert_eq!(
        recon.game.suzerain_of(minor.id),
        None,
        "Civ 6 reported no suzerain (-1); the mirror must not read as ours"
    );
}

/// ★★★★★ The envoys the seat is holding reach the board, so `SendEnvoy` is
/// enumerated against a met city-state — the one input the deployed
/// `advanced_envoys` pass never had on a live board. Measured on the twelve
/// Settler games of 2026-08-15/16: 40–70 unspent at the end, 0 suzerainties
/// in 11 of 12. The host's `-1` ("could not answer") and an absent field
/// must leave the board's count alone rather than zero it.
#[test]
fn unspent_envoys_reach_the_board_and_send_envoy_is_enumerated() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 60,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(6, 6, "TERRAIN_PLAINS"), plot(12, 12, "TERRAIN_PLAINS")],
    }]);
    let minor = StateMinor {
        player: 9,
        civ: "CIVILIZATION_GENEVA".to_string(),
        suzerain: -1,
        envoys: 1,
        cities: vec![StateCity {
            id: 90,
            name: "Geneva".to_string(),
            x: 6,
            y: 6,
            pop: 4,
            ..StateCity::default()
        }],
        ..StateMinor::default()
    };
    let state = StateSnapshot {
        turn: 60,
        envoys_free: Some(4),
        minors: vec![minor.clone()],
        ..StateSnapshot::default()
    };

    let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
    let geneva = recon
        .game
        .players
        .iter()
        .find(|player| player.is_minor && player.civ == "Geneva")
        .expect("Geneva minor seat");
    assert_eq!(
        recon.game.players[0].envoys_free, 4,
        "the held count is the host's fact"
    );
    assert!(
        recon
            .game
            .legal_actions(0)
            .iter()
            .any(|action| matches!(action, crate::game::Action::SendEnvoy { player } if *player == geneva.id)),
        "a held envoy and a met city-state must enumerate SendEnvoy"
    );
    // Sending one on the planning board spends one and lands on Geneva.
    let mut planned = recon.game.clone();
    planned
        .apply(0, &crate::game::Action::SendEnvoy { player: geneva.id })
        .expect("the envoy is legal");
    assert_eq!(planned.players[0].envoys_free, 3);
    assert_eq!(planned.envoys_at(0, geneva.id), 2);

    // The host that did not answer, in both shapes.
    let silent = StateSnapshot {
        turn: 60,
        envoys_free: None,
        minors: vec![minor.clone()],
        ..StateSnapshot::default()
    };
    assert_eq!(
        rebuild_from_state(&snapshot, &silent, 6, 1, 250, 0)
            .game
            .players[0]
            .envoys_free,
        0
    );
    let failed = StateSnapshot {
        turn: 60,
        envoys_free: Some(-1),
        minors: vec![minor],
        ..StateSnapshot::default()
    };
    assert_eq!(
        rebuild_from_state(&snapshot, &failed, 6, 1, 250, 0)
            .game
            .players[0]
            .envoys_free,
        0
    );
}

#[test]
fn renamed_city_state_uses_exported_capital_instead_of_legacy_type_id() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(6, 6, "TERRAIN_PLAINS")],
    }]);
    let state = StateSnapshot {
        turn: 30,
        minors: vec![StateMinor {
            player: 8,
            civ: "CIVILIZATION_JAKARTA".to_string(),
            cities: vec![StateCity {
                id: 65_536,
                name: "Bandar Brunei".to_string(),
                x: 6,
                y: 6,
                pop: 2,
                capital: true,
                ..StateCity::default()
            }],
            ..StateMinor::default()
        }],
        ..StateSnapshot::default()
    };

    let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
    assert!(recon
        .game
        .players
        .iter()
        .any(|player| player.is_minor && player.civ == "Bandar Brunei"));
    assert!(!recon
        .unmapped
        .iter()
        .any(|name| name == "CIVILIZATION_JAKARTA"));
}

#[test]
fn dormant_free_cities_does_not_turn_kabul_into_a_turn_one_enemy() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 1,
        width: 12,
        height: 12,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_PLAINS")],
    }]);
    let state = StateSnapshot {
        turn: 1,
        minors: vec![StateMinor {
            player: 62,
            civ: "CIVILIZATION_FREE_CITIES".to_string(),
            at_war: true,
            ..StateMinor::default()
        }],
        ..StateSnapshot::default()
    };

    let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
    assert_eq!(
        recon
            .game
            .players
            .iter()
            .filter(|player| player.is_minor && !player.is_barbarian)
            .count(),
        0,
        "an empty Firaxis Free Cities placeholder must consume no city-state seat"
    );
    let free = recon
        .game
        .players
        .iter()
        .find(|player| player.is_free_city)
        .unwrap();
    assert!(!free.alive);
    assert!(!recon.game.at_war.contains(&(0, free.id)));
}

#[test]
fn a_present_free_city_uses_the_dedicated_free_cities_seat() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 80,
        width: 12,
        height: 12,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_PLAINS")],
    }]);
    let state = StateSnapshot {
        turn: 80,
        minors: vec![StateMinor {
            player: 62,
            civ: "CIVILIZATION_FREE_CITIES".to_string(),
            score: 20,
            military: 35.0,
            at_war: true,
            cities: vec![StateCity {
                id: 70,
                name: "Free City".to_string(),
                x: 5,
                y: 5,
                pop: 4,
                ..StateCity::default()
            }],
            ..StateMinor::default()
        }],
        ..StateSnapshot::default()
    };

    let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
    let free = recon
        .game
        .players
        .iter()
        .find(|player| player.is_free_city)
        .unwrap();
    assert!(free.alive);
    assert!(recon.game.is_at_war(0, free.id));
    assert_eq!(recon.game.score(free.id), 20);
    assert_eq!(recon.game.military_power(free.id), 35.0);
    assert!(recon.game.cities.values().any(|city| city.owner == free.id));
}

#[test]
fn a_city_state_met_later_uses_a_seat_reserved_by_the_lobby() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 1,
        width: 24,
        height: 24,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_PLAINS"), plot(6, 5, "TERRAIN_PLAINS")],
    }]);
    let mut state = StateSnapshot {
        turn: 1,
        seat: Seat {
            city_states: 2,
            ..Seat::default()
        },
        ..StateSnapshot::default()
    };
    let mut mirror = LiveMirror::new(&snapshot, &state, 6, 1, 250, 0);
    assert_eq!(
        mirror
            .game
            .players
            .iter()
            .filter(|player| player.is_minor && !player.is_barbarian)
            .count(),
        2
    );

    state.turn = 2;
    state.minors.push(StateMinor {
        player: 6,
        civ: "CIVILIZATION_KABUL".to_string(),
        cities: vec![StateCity {
            id: 70,
            name: "Kabul".to_string(),
            x: 6,
            y: 5,
            pop: 2,
            ..StateCity::default()
        }],
        ..StateMinor::default()
    });
    mirror.sync(&snapshot, &state, 0);
    let kabul = mirror
        .game
        .players
        .iter()
        .find(|player| player.civ == "Kabul")
        .expect("the newly met city-state uses a reserved seat");
    assert!(mirror
        .game
        .cities
        .values()
        .any(|city| city.owner == kabul.id));
}

#[test]
fn current_city_production_follows_every_live_state() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 8,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 8,
        cities: vec![StateCity {
            id: 1,
            name: "Delhi".to_string(),
            x: 5,
            y: 5,
            pop: 2,
            producing: Some("UNIT_SCOUT".to_string()),
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let city = mirror.cid_of[&1];
    assert!(matches!(
        mirror.game.cities[&city].queue.first(),
        Some(crate::game::Item::Unit { unit }) if unit == "scout"
    ));

    state.turn = 9;
    state.cities[0].producing = Some("UNIT_SETTLER".to_string());
    mirror.sync(&snapshot, &state, 0);
    assert!(matches!(
        mirror.game.cities[&city].queue.first(),
        Some(crate::game::Item::Unit { unit }) if unit == "settler"
    ));

    state.turn = 10;
    state.cities[0].producing = None;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        mirror.game.cities[&city].queue.is_empty(),
        "the completed item must not remain as a phantom queue entry"
    );
}

/// A city Civilization VI reports as building a WONDER is busy, on both the
/// first reconstruction and every later sync — the mirror must not seed it
/// idle and let the planner replace the wonder the next turn (Hagia Sophia,
/// Rome, run civvis-20260815T202611Z t124→t125).
#[test]
fn a_wonder_under_construction_keeps_the_city_queue_busy() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(6, 5, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 40,
        cities: vec![StateCity {
            id: 1,
            name: "Rome".to_string(),
            x: 5,
            y: 5,
            pop: 6,
            producing: Some("BUILDING_HAGIA_SOPHIA".to_string()),
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let city = mirror.cid_of[&1];
    assert!(
        matches!(
            mirror.game.cities[&city].queue.first(),
            Some(crate::game::Item::Wonder { wonder, .. }) if wonder == "hagia_sophia"
        ),
        "fresh reconstruction: {:?}",
        mirror.game.cities[&city].queue
    );

    state.turn = 41;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        matches!(
            mirror.game.cities[&city].queue.first(),
            Some(crate::game::Item::Wonder { wonder, .. }) if wonder == "hagia_sophia"
        ),
        "later sync: {:?}",
        mirror.game.cities[&city].queue
    );
}

#[test]
fn live_mirror_permanently_blocks_host_granted_spy_production() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 8,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(6, 5, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 8,
        civics: vec!["CIVIC_DIPLOMATIC_SERVICE".to_string()],
        cities: vec![StateCity {
            id: 1,
            name: "Delhi".to_string(),
            x: 5,
            y: 5,
            pop: 2,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let spy = crate::game::Item::Unit {
        unit: crate::name!("spy"),
    };
    let city = mirror.cid_of[&1];
    assert!(
        !mirror.game.can_produce(0, city, &spy),
        // ⚠ This state reports no `spy_capacity`, which is now what the
        // block keys on: an export that cannot say how many Spies the
        // empire may field fails CLOSED, exactly as before. A build that
        // DOES report capacity is allowed to train one while under it —
        // see `live_spies_are_seated_and_the_block_follows_capacity`.
        "an unknown Spy capacity keeps the unconditional block"
    );
    assert_eq!(
        mirror.game.blocked_production[&city],
        std::collections::BTreeSet::from(["unit:spy".to_string()]),
        "the live-only block must not suppress unrelated production"
    );

    // `sync` replaces temporary host-refusal cooldowns. Its permanent host-rule
    // block must survive that replacement and cover a city first seen this turn.
    state.turn = 9;
    state.cities.push(StateCity {
        id: 2,
        name: "Agra".to_string(),
        x: 6,
        y: 5,
        pop: 2,
        ..StateCity::default()
    });
    mirror.sync(&snapshot, &state, 0);
    for host_city in [1, 2] {
        let city = mirror.cid_of[&host_city];
        assert!(
            mirror.game.blocked_production[&city].contains("unit:spy"),
            "city {host_city} must retain the permanent host rule"
        );
        assert!(
            !mirror.game.can_produce(0, city, &spy),
            "city {host_city} must not offer an untrainable Spy after sync"
        );
    }
}

/// ★★★★★ Building aliases cross; a truly unknown building stays observable.
/// ★★★★★ A building CIVVIS does not model must not take the decider down.
///
/// `BUILDING_CASTLE` **panicked the whole decider** on live run
/// `civvis-20260801T012454Z` at turn 238:
///
/// ```text
/// panicked at src/specmap.rs: no ruleset entry named "castle"
///   Game::building_district_is_active -> Game::spawn_unit
///     -> mirror::rebuild_from_state -> LiveMirror::new
/// ```
///
/// The city's buildings were lowercased rather than translated, so the Firaxis
/// internal name `castle` entered the list and `rules.buildings[..]` panicked.
/// Castle is not unmodelled: it is CIVVIS's `medieval_walls`. Dropping it prevents
/// the crash but also removes a real building and gives the city the wrong state.
///
/// ⚠ The assertion is that the rebuild SURVIVES and SAYS SO. A silent drop would
/// also stop the panic and would be the wrong fix — the name has to be counted.
#[test]
fn building_aliases_cross_and_unknown_buildings_are_reported_not_fatal() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 8,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 8,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "London".to_string(),
        x: 5,
        y: 5,
        pop: 6,
        buildings: vec![
            "BUILDING_MONUMENT".to_string(),
            // This exact building name is also a unique prefix of
            // UNIVERSITY_OF_SANKORE in the wonder table.
            "BUILDING_UNIVERSITY".to_string(),
            "BUILDING_CASTLE".to_string(),
            "BUILDING_STAR_FORT".to_string(),
            // Deliberately absent from both rule sets.
            "BUILDING_CIVVIS_MIRROR_SENTINEL".to_string(),
        ],
        ..StateCity::default()
    });

    // Before the fix this line panicked rather than returning.
    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);

    let city = recon
        .game
        .cities
        .values()
        .find(|c| c.owner == 0)
        .expect("the seat's city must be on the board");
    assert!(
        city.buildings.contains(&Name::new("monument")),
        "a building CIVVIS does model still crosses"
    );
    assert!(city.buildings.contains(&Name::new("university")));
    assert!(
        city.buildings.contains(&Name::new("medieval_walls")),
        "Firaxis's BUILDING_CASTLE is CIVVIS's medieval walls"
    );
    assert!(
        city.buildings.contains(&Name::new("renaissance_walls")),
        "Firaxis's BUILDING_STAR_FORT is CIVVIS's Renaissance walls"
    );
    assert!(
        recon
            .unmapped
            .iter()
            .any(|entry| entry.contains("BUILDING_CIVVIS_MIRROR_SENTINEL")),
        "and it must be COUNTED, not silently dropped: {:?}",
        recon.unmapped
    );
    assert!(
        !recon
            .unmapped
            .iter()
            .any(|entry| entry.contains("BUILDING_CASTLE")
                || entry.contains("BUILDING_STAR_FORT")
                || entry.contains("BUILDING_UNIVERSITY")),
        "known buildings and aliases must not be reported as fidelity gaps: {:?}",
        recon.unmapped
    );

    // Incremental state sync has its own city update path and must make the
    // same cross-table decision.
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    state.turn += 1;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        !mirror
            .unmapped
            .iter()
            .any(|entry| entry.contains("BUILDING_UNIVERSITY")),
        "sync must not reclassify an ordinary University as a wonder: {:?}",
        mirror.unmapped
    );
}

/// ⚠ A mirrored city's buildings are the EXPORT's statement, and `place_city`
/// disagrees for a founding-bonus civilization: Rome's Trajan's Column pushes
/// a free monument on every placement, while Civilization VI grants it at
/// founding only. Run `civvis-20260807T172510Z` (#1366): two cities Rome
/// CAPTURED, whose export building lists were empty, mirrored with
/// `extra=['monument']` — ghost culture in exactly the captured cities the
/// recovery planner was re-valuing. Founded cities masked the seed because
/// their real monument is exported and the translation deduplicates.
#[test]
fn a_captured_city_does_not_inherit_the_seats_founding_bonus_monument() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 160,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(9, 5, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 160,
        ..StateSnapshot::default()
    };
    state.seat.civ = "CIVILIZATION_ROME".to_string();
    state.cities.push(StateCity {
        id: 1,
        name: "Rome".to_string(),
        x: 5,
        y: 5,
        pop: 9,
        capital: true,
        buildings: vec![
            "BUILDING_MONUMENT".to_string(),
            "BUILDING_GRANARY".to_string(),
        ],
        ..StateCity::default()
    });
    // Captured this game: Civ 6 reports its building list as empty.
    state.cities.push(StateCity {
        id: 2,
        name: "Karkar".to_string(),
        x: 9,
        y: 5,
        pop: 4,
        ..StateCity::default()
    });

    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let karkar = recon
        .game
        .cities
        .values()
        .find(|city| city.name == "Karkar")
        .expect("the captured city must be on the board");
    assert!(
        karkar.buildings.is_empty(),
        "the export lists no buildings; the mirror must not model a monument: {:?}",
        karkar.buildings
    );
    let rome = recon
        .game
        .cities
        .values()
        .find(|city| city.name == "Rome")
        .expect("the capital must be on the board");
    assert_eq!(
        rome.buildings
            .iter()
            .filter(|building| **building == Name::new("monument"))
            .count(),
        1,
        "the founded capital's real, exported monument still crosses exactly once"
    );
}

#[test]
fn a_completed_wonder_keeps_its_type_and_plot() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS"), plot(4, 3, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 40,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Memphis".to_string(),
        x: 3,
        y: 3,
        pop: 7,
        // Firaxis reports wonders through HasBuilding as well as the exact
        // plot record. It must not be classified as an unknown building.
        buildings: vec!["BUILDING_PYRAMIDS".to_string()],
        wonders: vec![StateWonder {
            kind: "BUILDING_PYRAMIDS".to_string(),
            x: 4,
            y: 3,
        }],
        ..StateCity::default()
    });
    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    let city = recon
        .game
        .cities
        .values()
        .find(|city| city.owner == 0)
        .unwrap();
    let city_id = city.id;
    assert_eq!(
        city.wonders.get(&Name::new("pyramids")),
        Some(&crate::hex::offset_to_axial(4, 3))
    );
    let wonder_pos = crate::hex::offset_to_axial(4, 3);
    assert_eq!(
        recon.game.map.tiles[&wonder_pos].wonder.as_deref(),
        Some("pyramids"),
        "the tile representation must agree with the city's wonder map"
    );
    assert!(
        recon.game.valid_improvements(0, wonder_pos).is_empty(),
        "a Builder must not target a completed wonder"
    );
    let mut bare = recon.game.clone();
    let tile = bare.map.tiles.get_mut(&wonder_pos).unwrap();
    tile.wonder = None;
    tile.owner_city = Some(city_id);
    assert!(
        !bare.valid_improvements(0, wonder_pos).is_empty(),
        "the fixture must otherwise be improvable, or the rejection proves nothing"
    );
    assert!(
        !recon
            .unmapped
            .iter()
            .any(|entry| entry.contains("PYRAMIDS")),
        "a modeled wonder is neither an unknown building nor missing its plot: {:?}",
        recon.unmapped
    );
}

/// ★★★★ A district the host will not place must stop being chosen — IN THAT CITY.
///
/// `DISTRICT_GOVERNMENT` was refused **24** times by turn 115 on live run
/// `civvis-20260801T024428Z`, and `build_no_plot` fired **39** times, every one the
/// same district. A Government Plaza is one per civilization, so once it exists
/// Civilization VI offers no plot anywhere, and CIVVIS re-chose it from the same
/// board turn after turn. Each discard leaves the city with nothing queued and the
/// hand-written ladder picks instead.
///
/// ⚠⚠ The second assertion is the one that matters. The host refuses a district
/// for two opposite reasons — impossible anywhere, or no room in THIS city — and a
/// global block would stop CIVVIS building Campuses across the empire the first
/// time one city ran out of space. That would trade a small waste for a large one.
#[test]
fn a_district_the_host_will_not_place_is_blocked_in_that_city_only() {
    let mut game = crate::game::Game::new(4, 20, 20, 7, 500, 0);
    let mut ours: Vec<u32> = game
        .cities
        .values()
        .filter(|c| c.owner == 0)
        .map(|c| c.id)
        .collect();
    while ours.len() < 2 {
        // A one-city fixture cannot show the scoping, which is the whole point.
        let seed = ours.len() as i32;
        let pos = (seed * 5 + 6, seed * 5 + 6);
        if !game.map.tiles.contains_key(&pos) {
            break;
        }
        game.place_city(0, pos, None);
        ours = game
            .cities
            .values()
            .filter(|c| c.owner == 0)
            .map(|c| c.id)
            .collect();
    }
    assert!(
        ours.len() >= 2,
        "need two cities to prove the block is scoped"
    );
    let (blocked_city, other_city) = (ours[0], ours[1]);
    // A fresh city has one population and no research, so it can site nothing at
    // all — the fixture, not the change, is what would fail. Unlock everything and
    // grow both cities so the question under test is the block and only the block.
    let techs: Vec<Name> = game
        .rules
        .techs
        .keys()
        .map(|t| Name::new(t.as_str()))
        .collect();
    for tech in techs {
        game.players[0].techs.insert(tech);
    }
    let civics: Vec<Name> = game
        .rules
        .civics
        .keys()
        .map(|c| Name::new(c.as_str()))
        .collect();
    for civic in civics {
        game.players[0].civics.insert(civic);
    }
    for cid in [blocked_city, other_city] {
        if let Some(city) = game.cities.get_mut(&cid) {
            city.pop = 12;
        }
    }

    // ⚠ DISCOVERED, not hardcoded. Which districts a fresh city can site depends on
    // population and tech, so naming one made the precondition fail on an
    // unremarkable fixture rather than on anything to do with this change.
    let district = game
        .rules
        .districts
        .keys()
        .map(|name| crate::name::Name::new(name.as_str()))
        .find(|name| {
            !game.district_sites(blocked_city, name).is_empty()
                && !game.district_sites(other_city, name).is_empty()
        })
        .expect("some district must be sitable in both cities for this to prove anything");

    Arc::make_mut(&mut game.blocked_districts)
        .entry(blocked_city)
        .or_default()
        .insert(district);

    assert!(
        game.district_sites(blocked_city, district).is_empty(),
        "the city the host refused must stop offering it"
    );
    assert!(
        !game.district_sites(other_city, district).is_empty(),
        "and every OTHER city must be untouched — a global block would cost far \
             more than the waste it prevents"
    );
}

/// A zero-target answer is stronger than a city-local site disagreement. The
/// host cannot see a location for this world unique anywhere, so every city must
/// stop valuing it — including through the prerequisite-reach query.
#[test]
fn a_world_unique_the_host_cannot_place_is_blocked_in_every_city() {
    let mut game = crate::game::Game::new(4, 20, 20, 71, 500, 0);
    let mut ours: Vec<u32> = game
        .cities
        .values()
        .filter(|city| city.owner == 0)
        .map(|city| city.id)
        .collect();
    while ours.len() < 2 {
        let seed = ours.len() as i32;
        let pos = (seed * 5 + 6, seed * 5 + 6);
        if !game.map.tiles.contains_key(&pos) {
            break;
        }
        game.place_city(0, pos, None);
        ours = game
            .cities
            .values()
            .filter(|city| city.owner == 0)
            .map(|city| city.id)
            .collect();
    }
    assert!(ours.len() >= 2, "need two cities to prove the world scope");
    let (first_city, second_city) = (ours[0], ours[1]);
    game.players[0].techs = game.rules.techs.keys().copied().collect();
    game.players[0].civics = game.rules.civics.keys().copied().collect();
    for city in [first_city, second_city] {
        game.cities.get_mut(&city).unwrap().pop = 12;
    }
    let wonder = game
        .rules
        .wonders
        .keys()
        .copied()
        .find(|wonder| {
            !game.wonder_sites(first_city, wonder).is_empty()
                && !game.wonder_sites(second_city, wonder).is_empty()
        })
        .expect("some wonder must be sitable in both cities for this to prove anything");

    Arc::make_mut(&mut game.host_unavailable_wonders).insert(wonder);

    for city in [first_city, second_city] {
        assert!(
            game.wonder_sites(city, wonder.as_str()).is_empty(),
            "the host's zero-target response must block {wonder:?} in city {city}"
        );
    }
}

/// A positive host answer must beat the temporary block emitted beside it.
/// Otherwise the bridge learns the legal coordinates and still leaves the
/// district unavailable for all eight cooldown turns.
#[test]
fn a_host_approved_district_site_reopens_the_same_city() {
    let mut game = crate::game::Game::new(4, 20, 20, 71, 500, 0);
    assert!(
        game.map.tiles.contains_key(&(6, 6)),
        "fixture city site exists"
    );
    let city = game.place_city(0, (6, 6), None);
    game.players[0].techs = game.rules.techs.keys().copied().collect();
    game.players[0].civics = game.rules.civics.keys().copied().collect();
    game.cities.get_mut(&city).unwrap().pop = 12;
    let mut candidate = None;
    for district in game.rules.districts.keys().copied() {
        for site in game.district_sites(city, district) {
            let item = crate::game::Item::District {
                district,
                pos: site,
            };
            if game.can_produce(0, city, &item) {
                candidate = Some((district, site));
                break;
            }
        }
        if candidate.is_some() {
            break;
        }
    }
    let (district, site) = candidate.expect("an unlocked grown city needs a buildable district");

    Arc::make_mut(&mut game.blocked_districts)
        .entry(city)
        .or_default()
        .insert(district);
    assert!(
        game.district_sites(city, district).is_empty(),
        "precondition: the paired refusal blocks the normal model"
    );
    Arc::make_mut(&mut game.host_district_sites)
        .entry(city)
        .or_default()
        .entry(district)
        .or_default()
        .insert(site);

    assert_eq!(
        game.district_sites(city, district),
        vec![site],
        "the host-approved tile must be the sole fresh candidate"
    );
    assert!(
        game.can_produce(
            0,
            city,
            &crate::game::Item::District {
                district,
                pos: site,
            }
        ),
        "the approved coordinate has to reach the production gate, not merely a field"
    );
}

/// A positive wonder placement response is the escape hatch from its paired
/// temporary refusal, just as it is for districts. This uses Pyramids because
/// its flat desert rule is easy to make explicit in a tiny mirrored board.
#[test]
fn a_host_approved_wonder_site_reopens_the_same_city() {
    let mut game = crate::game::Game::new(4, 20, 20, 71, 500, 0);
    assert!(
        game.map.tiles.contains_key(&(6, 6)),
        "fixture city site exists"
    );
    let city = game.place_city(0, (6, 6), None);
    let site = (7, 6);
    assert!(
        game.map.tiles.contains_key(&site),
        "fixture wonder site exists"
    );
    game.players[0].techs = game.rules.techs.keys().copied().collect();
    game.players[0].civics = game.rules.civics.keys().copied().collect();
    {
        let tile = game.map.tiles.get_mut(&site).unwrap();
        tile.terrain = crate::name!("desert");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
        tile.owner_city = Some(city);
    }
    let owned_tiles = &mut game.cities.get_mut(&city).unwrap().owned_tiles;
    if !owned_tiles.contains(&site) {
        owned_tiles.push(site);
    }
    let wonder = crate::name!("pyramids");
    let item = crate::game::Item::Wonder { wonder, pos: site };
    assert!(
        game.can_produce(0, city, &item),
        "precondition: the configured Pyramids site is buildable"
    );

    Arc::make_mut(&mut game.blocked_wonders)
        .entry(city)
        .or_default()
        .insert(wonder);
    assert!(
        game.wonder_sites(city, &wonder).is_empty(),
        "precondition: the paired refusal blocks the normal model"
    );
    Arc::make_mut(&mut game.host_wonder_sites)
        .entry(city)
        .or_default()
        .entry(wonder)
        .or_default()
        .insert(site);

    assert_eq!(
        game.wonder_sites(city, &wonder),
        vec![site],
        "the host-approved tile must be the sole fresh candidate"
    );
    assert!(
        game.can_produce(0, city, &item),
        "the approved coordinate has to reach the production gate, not merely a field"
    );
}

/// ★★★★★ A MIRRORED CAPITAL MUST NOT BE PAID FOR ITS PALACE TWICE.
///
/// CIVVIS models the palace positionally — `city_has_palace` derives it from
/// capital status, and four separate sites add its yields, housing, amenity and
/// great-work slots off that predicate. Nothing native ever pushes "palace" into
/// a `buildings` list. Civilization VI exports `BUILDING_PALACE`, the translation
/// put it in the list, and every one of those four sites then paid twice.
///
/// Measured on run `civvis-20260802T014139Z`, turn 3 — one city, pop 1, palace
/// only. Civ 6 reported **2.5** science and the reconstruction reported **5.0**:
/// palace 2 twice, plus 0.5 for the citizen. With the seat re-dealt to Rome (a
/// civ carrying no invented per-city yield) the same replay reads
/// `science 2.5/2.5 +0%` afterwards, against `2.5/5.0 +98%` before.
///
/// ⚠ **THIS TEST PINS THE MECHANISM, NOT THE NUMBER, AND THAT IS A COMPROMISE
/// WORTH KNOWING ABOUT.** The number is pinned by the replay above, on real
/// exported data, which is the stronger evidence of the two.
///
/// It cannot be pinned here because a game built by `rebuild_from_state` in a
/// unit test yields **nothing at all**: `city_yields` on this fixture's capital
/// reads science 0, production 0 and *food 0* — through a hard `.max(2.0)` floor
/// on the city-centre tile, so the value is impossible and the body plainly never
/// runs. `city_yields_weighted`, which is documented as never being on the cached
/// path, reads 0 as well, so it is not the query memo.
///
/// RESOLVED (yield-fidelity work, 2026-08-16): the body runs; the LAST line
/// zeroes it. `StateCity::default()` is `loyalty: 0.0` — the serde default
/// `unknown_strength` (-1) applies only when deserializing — the mirror copies
/// any non-negative loyalty onto the city, and `loyalty_yield_mult(0.0)` is the
/// revolt band's **0**. A fixture that wants numbers says `loyalty: 100.0`
/// (see `host_plot_yields_become_tile_corrections_and_the_model_stays_readable`);
/// a real export always carries loyalty, so live boards were never affected.
///
/// ⚠⚠ That silently weakens the sibling test below: it asserts only that the
/// drift string carries **Civ 6's** number and a `%`, never CIVVIS's own, so it
/// passes just as happily on a reconstruction yielding zero. Both halves of
/// that comparison are assertable once the fixture carries loyalty.
#[test]
fn a_mirrored_capital_is_not_paid_for_its_palace_twice() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 3,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 3,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Lisbon".to_string(),
        x: 5,
        y: 5,
        pop: 1,
        // Both halves matter: Civ 6 marks the seat's capital AND exports the
        // palace inside it, and it is the pair that used to pay twice.
        capital: true,
        buildings: vec!["BUILDING_PALACE".to_string()],
        ..StateCity::default()
    });
    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    let city = recon
        .game
        .cities
        .values()
        .find(|city| city.owner == 0)
        .expect("the exported city must be on the board");

    assert!(
        !city.buildings.iter().any(|b| b.as_str() == "palace"),
        "the palace is positional in CIVVIS; listing it is what pays it twice"
    );
    assert!(
        !city.buildings.iter().any(|b| b.as_str() == "palace"),
        "the palace is positional in CIVVIS; listing it is what paid it twice"
    );
    assert!(
        recon.game.city_has_palace(city),
        "and it must still be paid ONCE — city_has_palace is the payer, and it \
             is true for exactly the city Civ 6 exported the palace in"
    );
}

/// ★★★★★ AND IT MUST NAME THE PART THAT IS NOT A DEFECT.
///
/// CIVVIS's civilization abilities are not Civilization VI's — `data/civs.json`
/// gives Arabia "House of Wisdom: +1 science and +1 faith in every city" where
/// the real ability grants no flat per-city yield. A mirrored seat therefore
/// runs hot by exactly `effect x cities`, and on run civvis-20260802T064240Z
/// that was the ENTIRE residual: science +18% median, culture -0%.
///
/// Without attribution that 18% gets re-investigated every time somebody reads
/// it. With it, a reader separates the known offset from a new defect at a
/// glance.
///
/// ⚠ Asserted in BOTH directions. A civ with no flat effect must not grow the
/// clause at all — a line that always fires says nothing.
#[test]
fn the_drift_attributes_the_civ_ability_it_knows_about() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 8,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 8,
        science: Some(5.0),
        culture: Some(3.0),
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Mecca".to_string(),
        x: 5,
        y: 5,
        pop: 4,
        capital: true,
        ..StateCity::default()
    });
    let mut recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);

    recon.game.players[0].civ = "Arabia".to_string();
    let arabia = economy_drift(&recon.game, &state).expect("yields present");
    assert!(
        arabia.contains("of which civ ability Arabia"),
        "the known offset must be named: {arabia}"
    );
    assert!(
        arabia.contains("science +1.0"),
        "and quantified at its real size — city_science 1 over one city: {arabia}"
    );

    // ⚠ Rome carries no flat per-city science or culture, so the clause must be
    // absent entirely rather than reading "+0.0".
    recon.game.players[0].civ = "Rome".to_string();
    let rome = economy_drift(&recon.game, &state).expect("yields present");
    assert!(
        !rome.contains("of which civ ability"),
        "a civ with no flat effect must not grow the clause: {rome}"
    );
}

/// ★★★★ The reconstruction's economic error must be a NUMBER, not a shrug.
///
/// Measured live on `civvis-20260801T024428Z`: `economy civ6/civvis science
/// 5.8/9.2 +59% culture 7.1/9.4 +33%`. Research valuations are spent in these
/// units, so a rate half again too fast makes an unaffordable plan look
/// affordable — and until this line existed nothing said so.
#[test]
fn the_economic_drift_is_reported_and_an_old_export_reads_as_unknown() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 8,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 8,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Washington".to_string(),
        x: 5,
        y: 5,
        pop: 4,
        ..StateCity::default()
    });
    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);

    // ⚠ An export with no yields must read as UNKNOWN, never as agreement. An
    // older mod that reports nothing would otherwise look like a perfect match,
    // which is the failure mode this bridge specialises in.
    assert!(
        economy_drift(&recon.game, &state).is_none(),
        "no yields exported means no claim about drift"
    );

    state.science = Some(5.8);
    state.culture = Some(7.1);
    let drift = economy_drift(&recon.game, &state).expect("yields present");
    assert!(
        drift.contains("science 5.8/"),
        "the game's own number leads, so the comparison is readable: {drift}"
    );
    assert!(
        drift.contains('%'),
        "and the gap is expressed as a percentage: {drift}"
    );

    // ⚠⚠ PRODUCTION was exported by #845 and never deserialized, so it could not
    // appear here at all. It is the yield that decides what every city builds,
    // and since #867 CIVVIS chooses that for every city every turn. The exact
    // empire total is authoritative; the per-city `production` field is the
    // build queue's whole-number accessor and must not be used as a yield total.
    assert!(
        !drift.contains("production"),
        "a city reporting no production figure must stay silent, not claim a \
             100% drift: {drift}"
    );
    state.cities[0].production = 99.0;
    state.public_stats.production = Some(12.0);
    let drift = economy_drift(&recon.game, &state).expect("yields present");
    assert!(
        drift.contains("production 12.0/"),
        "the exact empire production leads, as science and culture do: {drift}"
    );
    assert!(
        !drift.contains("production 99.0/"),
        "the queue production accessor must not masquerade as the city's yield: {drift}"
    );

    // Older exports may have exact city yields without the public aggregate.
    // That fallback is valid only when every city is represented.
    state.public_stats.production = None;
    state.cities[0].yields = Some(crate::rules::Yields {
        production: 12.0,
        ..Default::default()
    });
    let drift = economy_drift(&recon.game, &state).expect("yields present");
    assert!(
        drift.contains("production 12.0/"),
        "exact per-city yields are a safe fallback when the aggregate is absent: {drift}"
    );
}

/// ★★★★★ A barbarian that appears AFTER the mirror is built must reach the board.
///
/// `LiveMirror::sync` had **no reference to `state.hostiles` or `barb_pid` at
/// all**, so barbarians were whatever the construction rebuild found and nothing
/// after. At turn 1 that is normally none — so the decider played entire games
/// against an empty barbarian seat while the export named them every turn.
///
/// Measured on live run `civvis-20260801T040700Z`: Montréal founded turn 26, gone
/// by turn 42, loyalty 100 throughout and at war with nobody it had met — so
/// neither revolt nor a rival took it, and `hostiles` was non-empty in the export.
/// ⚠⚠ `gold_per_turn` gates the whole bankruptcy response and the bridge
/// never wrote it, so `economic_recovery` was unreachable in every real
/// game. A treasury pinned at zero is the case that matters: Civilization VI
/// clamps the balance there and disbands units to pay, so differencing alone
/// reports a healthy zero exactly when the empire is broke.
#[test]
fn an_empty_treasury_reports_insolvency_rather_than_a_flat_balance() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 1,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 1,
        gold: 120,
        ..StateSnapshot::default()
    };
    let mut mirror = LiveMirror::new(&snapshot, &state, 1, 1, 500, 0);

    // The first sample cannot be differenced against anything.
    state.turn = 2;
    state.gold = 108;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(
        mirror.game.players[0].gold_per_turn, -12.0,
        "a falling treasury is negative net income"
    );

    state.turn = 3;
    state.gold = 130;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.players[0].gold_per_turn, 22.0);

    // The defect: broke, and staying broke. The delta is zero and the old
    // reading would have called that solvent.
    state.turn = 4;
    state.gold = 0;
    mirror.sync(&snapshot, &state, 0);
    assert!(mirror.game.players[0].gold_per_turn < -0.5);
    state.turn = 5;
    state.gold = 0;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        mirror.game.players[0].gold_per_turn < -0.5,
        "a treasury pinned at zero is insolvency, not thrift — this is the \
             reading that makes economic_recovery reachable at all"
    );

    // A gap of unknown length is not a rate. Leave the last reading alone
    // rather than inventing one across a resync.
    state.turn = 40;
    state.gold = 400;
    mirror.sync(&snapshot, &state, 0);
    assert!(mirror.game.players[0].gold_per_turn < -0.5);
}

/// A seat that cannot see barbarians cannot garrison against them.
#[test]
fn a_barbarian_that_appears_after_construction_reaches_the_board() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 4,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![
            plot(5, 5, "TERRAIN_GRASS"),
            plot(5, 6, "TERRAIN_GRASS"),
            plot(6, 6, "TERRAIN_GRASS"),
        ],
    }]);
    let mut state = StateSnapshot {
        turn: 4,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Ottawa".to_string(),
        x: 5,
        y: 5,
        pop: 3,
        ..StateCity::default()
    });

    // Turn 4: no barbarian in sight, which is the ordinary opening.
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let barb = mirror
        .game
        .barb_pid
        .expect("a mirrored roster has a barbarian seat");
    assert_eq!(
        mirror
            .game
            .units
            .values()
            .filter(|u| u.owner == barb)
            .count(),
        0,
        "precondition: the board starts with no barbarians"
    );

    // Turn 8: one walks into view. Before the fix nothing here looked at it.
    state.turn = 8;
    state.hostiles.push(StateUnit {
        kind: "UNIT_WARRIOR".to_string(),
        x: 5,
        y: 6,
        hp: 35.0,
        max_moves: Some(3.0),
        fortified: true,
        fortify_turns: 1,
        ..StateUnit::default()
    });
    mirror.sync(&snapshot, &state, 0);

    assert_eq!(
        mirror
            .game
            .units
            .values()
            .filter(|u| u.owner == barb)
            .count(),
        1,
        "a barbarian the export named must be on the board — this is the whole \
             defect, and before the fix it stayed invisible for the rest of the game"
    );
    let hostile = mirror
        .game
        .units
        .values()
        .find(|unit| unit.owner == barb)
        .unwrap();
    assert_eq!(
        hostile.hp, 35,
        "a visible hostile's damage is useful combat state"
    );
    assert_eq!(
        mirror.game.unit_max_moves(hostile.id),
        3.0,
        "a visible hostile's fresh-turn movement comes from the host export"
    );
    assert!(hostile.fortified);
    assert_eq!(hostile.fortify_turns, 1);

    // And it must leave again when it dies or moves out of sight, or the board
    // accumulates ghosts that never attack anything.
    state.hostiles.clear();
    state.turn = 12;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(
        mirror
            .game
            .units
            .values()
            .filter(|u| u.owner == barb)
            .count(),
        0,
        "and one the export no longer names must go, or the threat list only grows"
    );
}

/// ⚠⚠ A GAP LIST THAT REPORTS A FIELD THE MIRROR DOES READ IS A BROKEN
/// INSTRUMENT, and this project navigates by `unmapped`.
///
/// `state_schema_gaps` keeps its own names beside `StateCity`/`StateUnit` and
/// nothing kept the two in step. #877 added `production`, `production_cost` and
/// `production_turns` to the struct and the decider went on printing
/// `unmapped: schema:city.production,...` every turn while reading them
/// perfectly well. `class` had been doing it for longer.
///
/// A superset is fine — serde aliases mean `kind` also answers to `type`, and
/// only the export side needs both. What must never happen again is a struct
/// field with no entry.
#[test]
fn the_schema_allowlists_cover_every_declared_field() {
    for (struct_name, allowed) in [
        ("StateCity", CITY_KEYS),
        ("StateUnit", UNIT_KEYS),
        ("StatePublicEmpireStats", PUBLIC_STATS_KEYS),
    ] {
        let declared = declared_fields(struct_name);
        assert!(
            !declared.is_empty(),
            "{struct_name} parsed to no fields — the extractor broke, not the list"
        );
        let missing: Vec<&String> = declared
            .iter()
            .filter(|field| !allowed.contains(&field.as_str()))
            .collect();
        assert!(
            missing.is_empty(),
            "{struct_name} declares {missing:?}, which state_schema_gaps would \
                 report as unmapped even though the mirror reads them"
        );
    }
}

/// ★★★★★ `DISTRICT_GOVERNMENT` is CIVVIS's `government_plaza`, and missing that
/// cost two separate bugs.
///
/// Prefix-stripping gives `government`, which is in no table, so `civvis_node_name`
/// returned None and both callers did the honest thing with a wrong answer:
/// `civvis_production_item` read a city building one as IDLE (60 repeat orders in
/// one measured run), and #729's blocked-districts reader dropped the name, so the
/// block never engaged for the one district it was built for —
/// `no_params_DISTRICT_GOVERNMENT` still read **9** after it shipped.
#[test]
fn a_civ6_name_that_truncates_a_civvis_one_resolves_only_when_unambiguous() {
    let rules = crate::rules::Rules::embedded();
    assert_eq!(
        civvis_node_name(&rules.districts, "DISTRICT_GOVERNMENT", "DISTRICT_").as_deref(),
        Some("government_plaza"),
        "the truncated Civilization VI name must reach CIVVIS's fuller one"
    );
    // The ordinary case must not regress: an exact name still wins outright.
    assert_eq!(
        civvis_node_name(&rules.districts, "DISTRICT_CAMPUS", "DISTRICT_").as_deref(),
        Some("campus")
    );
    // ⚠ And a stem that is not a whole word must NOT match. `dam` is a real
    // district; without the boundary check it would swallow anything starting
    // "dam...".
    assert!(
        civvis_node_name(&rules.districts, "DISTRICT_DAM", "DISTRICT_").as_deref() == Some("dam"),
        "an exact short name resolves to itself, not to a longer neighbour"
    );
    // A name CIVVIS genuinely does not have still answers None rather than
    // guessing at the nearest thing.
    assert!(
        civvis_node_name(&rules.districts, "DISTRICT_NOT_A_REAL_ONE", "DISTRICT_").is_none(),
        "an unknown district must not resolve to something plausible"
    );
}

/// ★★★★★ A district Civilization VI has built must be ON the reconstructed city.
///
/// `StateDistrict` was defined, carried on `StateCity`, handed to
/// `civvis_production_item` to locate a production plot, and used in tests —
/// and never written onto a city. `grep '\.districts\.insert' src/mirror.rs`
/// found nothing. So every Campus, Holy Site and Commercial Hub the real game had
/// built was invisible, and the city read as bare ground: the same shape as the
/// improvements gap, where a mirror showing an undeveloped empire made CIVVIS
/// re-order what it already had.
#[test]
fn the_districts_a_city_has_built_reach_the_board() {
    let historical: StateDistrict = serde_json::from_value(serde_json::json!({
        "type": "DISTRICT_CAMPUS", "x": 5, "y": 6, "pillaged": false
    }))
    .unwrap();
    assert!(
        historical.complete,
        "pre-completion-bit event streams keep their historical completed semantics"
    );
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![
            plot(5, 5, "TERRAIN_GRASS"),
            plot(5, 6, "TERRAIN_GRASS"),
            plot(6, 6, "TERRAIN_GRASS"),
        ],
    }]);
    let mut state = StateSnapshot {
        turn: 30,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Canberra".to_string(),
        x: 5,
        y: 5,
        pop: 8,
        districts: vec![
            // The centre is implicit in CIVVIS and must NOT be inserted.
            StateDistrict {
                kind: "DISTRICT_CITY_CENTER".to_string(),
                x: 5,
                y: 5,
                pillaged: false,
                complete: true,
                ..StateDistrict::default()
            },
            StateDistrict {
                kind: "DISTRICT_CAMPUS".to_string(),
                x: 5,
                y: 6,
                pillaged: true,
                complete: true,
                ..StateDistrict::default()
            },
        ],
        ..StateCity::default()
    });

    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    let city = recon
        .game
        .cities
        .values()
        .find(|c| c.owner == 0)
        .expect("the seat's city must be on the board");
    let city_id = city.id;

    assert_eq!(
        city.districts.get(Name::new("campus")).copied(),
        Some(crate::hex::offset_to_axial(5, 6)),
        "a built district must reach the board, on the plot the export named"
    );
    // ⚠ `found_city_for` gives a native CIVVIS city `Districts::default()`, so the
    // centre is implicit. Inserting it would put a district on the board that
    // CIVVIS's own games never have — checked in the source, not assumed.
    assert!(
        !city.districts.contains_key(Name::new("city_center")),
        "the city centre stays implicit, as it is in an ordinary CIVVIS game"
    );

    let campus = crate::hex::offset_to_axial(5, 6);
    let campus_tile = &recon.game.map.tiles[&campus];
    assert_eq!(campus_tile.district.as_deref(), Some("campus"));
    assert!(
        campus_tile.pillaged,
        "district pillage state must reach its tile"
    );
    assert!(
        recon.game.valid_improvements(0, campus).is_empty(),
        "a completed district must never be offered to a Builder"
    );
    let mut bare = recon.game.clone();
    let tile = bare.map.tiles.get_mut(&campus).unwrap();
    tile.district = None;
    tile.pillaged = false;
    tile.owner_city = Some(city_id);
    assert!(
        !bare.valid_improvements(0, campus).is_empty(),
        "the fixture must otherwise be improvable, or the rejection proves nothing"
    );

    // Incremental sync must preserve the distinction between a placed
    // foundation and a completed district.
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    state.turn += 1;
    state.cities[0].producing = Some("DISTRICT_HOLY_SITE".to_string());
    state.cities[0].districts.push(StateDistrict {
        kind: "DISTRICT_HOLY_SITE".to_string(),
        x: 6,
        y: 6,
        pillaged: false,
        complete: false,
        ..StateDistrict::default()
    });
    mirror.sync(&snapshot, &state, 0);
    let holy_site = crate::hex::offset_to_axial(6, 6);
    let tile = &mirror.game.map.tiles[&holy_site];
    assert!(tile.district.is_none());
    assert_eq!(
        tile.district_foundation
            .as_ref()
            .map(|foundation| foundation.district.as_str()),
        Some("holy_site")
    );
    assert!(!mirror.game.cities[&mirror.cid_of[&1]]
        .districts
        .contains_key(Name::new("holy_site")));
    assert!(mirror.game.valid_improvements(0, holy_site).is_empty());

    state.turn += 1;
    state.cities[0].producing = None;
    state.cities[0].districts[2].complete = true;
    mirror.sync(&snapshot, &state, 0);
    let tile = &mirror.game.map.tiles[&holy_site];
    assert_eq!(tile.district.as_deref(), Some("holy_site"));
    assert!(tile.district_foundation.is_none());
    assert!(mirror.game.cities[&mirror.cid_of[&1]]
        .districts
        .contains_key(Name::new("holy_site")));

    // An omitted fog/public roster is unknown, not evidence that permanent
    // infrastructure vanished.
    state.turn += 1;
    state.cities[0].districts.clear();
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(
        mirror.game.map.tiles[&campus].district.as_deref(),
        Some("campus")
    );
    assert_eq!(
        mirror.game.map.tiles[&holy_site].district.as_deref(),
        Some("holy_site")
    );

    state.turn += 1;
    state.cities.clear();
    mirror.sync(&snapshot, &state, 0);
    assert!(mirror.game.map.tiles[&campus].district.is_none());
    assert!(mirror.game.map.tiles[&holy_site].district.is_none());
}

/// ★★★★★ A walled city Civilization VI reports as UNDAMAGED must not read as razed.
///
/// `wall_hp` was never written, so it kept its 0 default while `city_max_wall_hp`
/// summed the walls the city had — and CIVVIS's gate is `wall_hp < max`. Every
/// walled city therefore looked destroyed forever. Replaying run
/// `civvis-20260801T065721Z` showed **47 turns** wanting
/// `PROJECT_REPAIR_OUTER_DEFENSES` while the exported defence was RISING.
#[test]
fn a_walled_city_reported_undamaged_is_not_read_as_razed() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS")],
    }]);
    let build = |wall_damage: f64| {
        let mut state = StateSnapshot {
            turn: 30,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Rome".to_string(),
            x: 3,
            y: 3,
            pop: 6,
            buildings: vec!["BUILDING_WALLS".to_string()],
            damage: 0.0,
            max_damage: 200.0,
            wall_damage,
            max_wall_damage: 100.0,
            ..StateCity::default()
        });
        let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
        let city = recon
            .game
            .cities
            .values()
            .find(|c| c.owner == 0)
            .expect("the seat's city must be on the board")
            .clone();
        let max = recon.game.city_max_wall_hp(&city);
        (city.wall_hp, max)
    };

    let (hp, max) = build(0.0);
    // ⚠ The precondition. With no walls modelled `max` is 0 and `wall_hp < max`
    // is false for any hp, so the test would pass for the wrong reason.
    assert!(
        max > 0,
        "the fixture city must actually have walls, or this proves nothing"
    );
    assert_eq!(
        hp, max,
        "an undamaged walled city must read at FULL wall hp"
    );

    let (hurt, max2) = build(20.0);
    assert_eq!(hurt, max2 - 20, "reported damage must come off the wall hp");
    assert!(hurt < max2, "a damaged city must still be repairable");

    // Damage beyond the wall total must floor at zero, not go negative:
    // `damage` is a `try` read in Lua and cannot be trusted to be in range.
    let (floored, _) = build(9_999.0);
    assert_eq!(floored, 0, "wall hp must clamp at zero");
}

#[test]
fn city_health_is_refreshed_on_every_live_sync() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 30,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Rome".to_string(),
        x: 3,
        y: 3,
        pop: 6,
        buildings: vec!["BUILDING_WALLS".to_string()],
        damage: 0.0,
        max_damage: 200.0,
        wall_damage: 0.0,
        max_wall_damage: 100.0,
        ..StateCity::default()
    });
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let cid = mirror.cid_of[&1];
    assert_eq!(mirror.game.cities[&cid].hp, 200);
    assert_eq!(mirror.game.cities[&cid].wall_hp, 100);

    state.turn += 1;
    state.cities[0].damage = 50.0;
    state.cities[0].wall_damage = 40.0;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.cities[&cid].hp, 150);
    assert_eq!(mirror.game.cities[&cid].wall_hp, 60);
    assert_eq!(mirror.game.city_max_wall_hp(&mirror.game.cities[&cid]), 100);
}

#[test]
fn city_capture_reconciles_both_rosters_and_ownership() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 20,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![
            plot(3, 3, "TERRAIN_GRASS"),
            plot(4, 3, "TERRAIN_GRASS"),
            plot(6, 3, "TERRAIN_GRASS"),
        ],
    }]);
    let city = |id, name: &str, x| StateCity {
        id,
        name: name.to_string(),
        x,
        y: 3,
        pop: 5,
        ..StateCity::default()
    };
    let mut state = StateSnapshot {
        turn: 20,
        ..StateSnapshot::default()
    };
    state.cities.push(city(10, "Home", 3));
    state.cities[0].districts.push(StateDistrict {
        kind: "DISTRICT_CAMPUS".to_string(),
        x: 4,
        y: 3,
        pillaged: false,
        complete: true,
        ..StateDistrict::default()
    });
    state.rivals.push(StateRival {
        player: 3,
        civ: "CIVILIZATION_ROME".to_string(),
        cities: vec![city(20, "Rome", 6)],
        ..StateRival::default()
    });
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);

    state.turn += 1;
    state.cities = vec![city(20, "Rome", 6)];
    state.rivals[0].cities = vec![city(10, "Home", 3)];
    mirror.sync(&snapshot, &state, 0);

    let ours = mirror
        .game
        .city_at(crate::hex::offset_to_axial(6, 3))
        .unwrap();
    let theirs = mirror
        .game
        .city_at(crate::hex::offset_to_axial(3, 3))
        .unwrap();
    assert_eq!(mirror.game.cities[&ours].owner, 0);
    assert_eq!(mirror.game.cities[&theirs].owner, 1);
    assert_eq!(mirror.cid_of.get(&20), Some(&ours));
    assert!(!mirror.cid_of.contains_key(&10));
    assert_eq!(mirror.game.player_city_ids(0), vec![ours]);
    let campus = crate::hex::offset_to_axial(4, 3);
    assert_eq!(
        mirror.game.map.tiles[&campus].district.as_deref(),
        Some("campus")
    );
    assert!(
        mirror.game.cities[&theirs]
            .districts
            .contains_key(Name::new("campus")),
        "a public rival record omits infrastructure; capture must preserve what was known"
    );
}

#[test]
fn a_razed_own_city_does_not_survive_in_the_mirror() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 20,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 20,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 10,
        name: "Home".to_string(),
        x: 3,
        y: 3,
        pop: 5,
        ..StateCity::default()
    });
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    assert_eq!(mirror.game.player_city_ids(0).len(), 1);

    state.turn += 1;
    state.cities.clear();
    mirror.sync(&snapshot, &state, 0);
    assert!(mirror.game.player_city_ids(0).is_empty());
    assert!(mirror
        .game
        .city_at(crate::hex::offset_to_axial(3, 3))
        .is_none());
}

/// ★★★ THE CAPTURE DECISION CROSSES. `captured_from` (`GetJustConqueredFrom`,
/// present while `GetNextCapturedCity()` names the city) and `original_owner`
/// (`GetOriginalOwner`) land on the mirrored city as seats, so the board
/// offers the same Keep / Raze / Liberate the shipped popup does — and stops
/// offering them when the export no longer carries the decision.
#[test]
fn a_captured_citys_pending_disposition_reaches_the_board_and_clears() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 20,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS"), plot(7, 3, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 20,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 10,
        name: "Home".to_string(),
        x: 3,
        y: 3,
        pop: 5,
        capital: true,
        ..StateCity::default()
    });
    // Taken this turn from Rome (Firaxis player 3); founded by Greece (5).
    state.cities.push(StateCity {
        id: 20,
        name: "Antium".to_string(),
        x: 7,
        y: 3,
        pop: 4,
        captured_from: Some(3),
        original_owner: Some(5),
        ..StateCity::default()
    });
    for (player, civ) in [(3, "CIVILIZATION_ROME"), (5, "CIVILIZATION_GREECE")] {
        state.rivals.push(StateRival {
            player,
            civ: civ.to_string(),
            at_war: player == 3,
            ..StateRival::default()
        });
    }
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let antium = mirror
        .game
        .city_at(crate::hex::offset_to_axial(7, 3))
        .unwrap();
    assert_eq!(
        mirror.game.cities[&antium].captured_from,
        Some(1),
        "Rome sits on seat 1"
    );
    assert_eq!(
        mirror.game.cities[&antium].original_owner, 2,
        "Greece sits on seat 2"
    );
    let pending = mirror.game.legal_city_disposition_actions(0);
    for action in [
        crate::game::Action::KeepCity { city: antium },
        crate::game::Action::RazeCity { city: antium },
        crate::game::Action::LiberateCity { city: antium },
    ] {
        assert!(
            pending.contains(&action),
            "{action:?} missing from {pending:?}"
        );
    }

    // The host took a directive: the export no longer names the city, and
    // the board stops asking.
    state.turn += 1;
    state.cities[1].captured_from = None;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.cities[&antium].captured_from, None);
    assert_eq!(mirror.game.cities[&antium].original_owner, 2);
    assert!(mirror.game.legal_city_disposition_actions(0).is_empty());

    // A loser the board has no seat for leaves the flag clear rather than
    // pointing the engine at a player that is not there.
    state.turn += 1;
    state.cities[1].captured_from = Some(9);
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.cities[&antium].captured_from, None);
}

/// ★★★★ A rival's unique unit must reach the board as what it REPLACES.
///
/// `UNIT_NORWEGIAN_LONGSHIP` was dropped on every turn it was visible on live run
/// `civvis-20260801T145302Z` — CIVVIS models no Norwegian uniques — so an enemy
/// warship was not on the board at all. A Longship replaces a Galley, which
/// CIVVIS does model.
#[test]
fn a_rivals_unique_unit_lands_as_what_it_replaces() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 12,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(2, 2, "TERRAIN_OCEAN"), plot(4, 4, "TERRAIN_GRASS")],
    }]);
    let build = |kind: &str, base: Option<&str>| {
        let mut state = StateSnapshot {
            turn: 12,
            ..StateSnapshot::default()
        };
        state.units.push(StateUnit {
            id: 7,
            kind: kind.to_string(),
            base: base.map(|b| b.to_string()),
            x: 2,
            y: 2,
            hp: 100.0,
            ..StateUnit::default()
        });
        rebuild_from_state(&snapshot, &state, 4, 1, 500, 0)
    };

    // ⚠ Precondition: the unique must genuinely be untranslatable, or this test
    // passes for the wrong reason.
    let bare = build("UNIT_NORWEGIAN_LONGSHIP", None);
    assert!(
        bare.game.units.is_empty(),
        "the fixture must be a unit CIVVIS cannot name, or the fallback is untested"
    );

    let with_base = build("UNIT_NORWEGIAN_LONGSHIP", Some("UNIT_GALLEY"));
    let unit = with_base
        .game
        .units
        .values()
        .next()
        .expect("a unique with a known base must reach the board");
    assert_eq!(unit.kind.as_str(), "galley", "it lands as what it replaces");
    // ⚠ And it must SAY it approximated. A collapsed distinction that nobody can
    // see is the failure the mapping rule names.
    assert!(
        with_base
            .dropped_units
            .iter()
            .any(|d| d.contains("approximated_as_galley")),
        "the approximation must be reported, not silent: {:?}",
        with_base.dropped_units
    );

    // A base CIVVIS also cannot name must still not invent a unit.
    let nonsense = build("UNIT_NORWEGIAN_LONGSHIP", Some("UNIT_NOT_A_REAL_UNIT"));
    assert!(
        nonsense.game.units.is_empty(),
        "an unknown base must not be guessed at"
    );
}

/// ★★★★★ Georgia's Khevsureti replaces Man-at-Arms, but the live rival-unit
/// export omits both `base` and `class`. Keep it on the threat board through the
/// explicit host spelling rather than letting a nearby army disappear.
#[test]
fn a_georgian_khevsureti_is_planted_as_a_man_at_arms() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 12,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(4, 4, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 12,
        ..StateSnapshot::default()
    };
    state.units.push(StateUnit {
        id: 17,
        kind: "UNIT_GEORGIAN_KHEVSURETI".to_string(),
        x: 4,
        y: 4,
        hp: 100.0,
        ..StateUnit::default()
    });

    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    let unit = rebuilt
        .game
        .units
        .values()
        .next()
        .expect("the Khevsureti must reach the reconstructed board");
    assert_eq!(unit.kind.as_str(), "man_at_arms");
    assert!(
        rebuilt.unmapped.is_empty(),
        "the explicit role bridge is a recognized approximation: {:?}",
        rebuilt.unmapped
    );
    assert!(
        rebuilt.dropped_units.is_empty(),
        "the mapped unit must not be reported as dropped: {:?}",
        rebuilt.dropped_units
    );
}

/// ★★★★★ A STANDALONE unique — no `UnitReplaces` row — must land by its class.
///
/// Run `civvis-20260801T175955Z` was lost with two `UNIT_MAPUCHE_MALON_RAIDER`
/// two tiles from the final city, dropped as untranslatable: the conquering
/// army was not on CIVVIS's board at all. `base` cannot save it (there is no
/// base); `class` must.
#[test]
fn a_standalone_unique_lands_by_its_promotion_class() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 12,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(2, 2, "TERRAIN_GRASS"), plot(4, 4, "TERRAIN_GRASS")],
    }]);
    let build = |kind: &str, class: Option<&str>| {
        let mut state = StateSnapshot {
            turn: 12,
            ..StateSnapshot::default()
        };
        state.units.push(StateUnit {
            id: 9,
            kind: kind.to_string(),
            class: class.map(|c| c.to_string()),
            x: 2,
            y: 2,
            hp: 100.0,
            ..StateUnit::default()
        });
        rebuild_from_state(&snapshot, &state, 4, 1, 500, 0)
    };

    // ⚠ Precondition: with neither base nor class the unit must genuinely drop,
    // or the fallback under test is not what put it on the board.
    let bare = build("UNIT_MAPUCHE_MALON_RAIDER", None);
    assert!(
        bare.game.units.is_empty(),
        "the fixture must be a unit CIVVIS cannot name, or the fallback is untested"
    );

    let classed = build(
        "UNIT_MAPUCHE_MALON_RAIDER",
        Some("PROMOTION_CLASS_LIGHT_CAVALRY"),
    );
    let unit = classed
        .game
        .units
        .values()
        .next()
        .expect("a standalone unique with a known class must reach the board");
    assert_eq!(
        unit.kind.as_str(),
        "horseman",
        "it lands as the class representative"
    );
    assert!(
        classed
            .dropped_units
            .iter()
            .any(|d| d.contains("approximated_as_horseman_from_light_cavalry")),
        "the approximation must be reported, not silent: {:?}",
        classed.dropped_units
    );

    // A class CIVVIS has no representative for must still not invent a unit.
    let nonsense = build(
        "UNIT_MAPUCHE_MALON_RAIDER",
        Some("PROMOTION_CLASS_NOT_REAL"),
    );
    assert!(
        nonsense.game.units.is_empty(),
        "an unknown class must not be guessed at"
    );

    // RANGED_CAVALRY was missing from the first fallback table. Preserve a
    // representative for an otherwise-unmodelled standalone unique.
    let ranged_unique = build(
        "UNIT_EXAMPLE_RANGED_RIDER",
        Some("PROMOTION_CLASS_RANGED_CAVALRY"),
    );
    let unit = ranged_unique
        .game
        .units
        .values()
        .next()
        .expect("a ranged-cavalry unique must reach the board");
    assert_eq!(unit.kind.as_str(), "saka_horse_archer");

    // Keshig is now modelled exactly; an exact name must outrank the class
    // approximation so its distinct strength and upgrade path survive.
    let keshig = build(
        "UNIT_MONGOLIAN_KESHIG",
        Some("PROMOTION_CLASS_RANGED_CAVALRY"),
    );
    let unit = keshig
        .game
        .units
        .values()
        .next()
        .expect("a modelled Keshig must reach the board");
    assert_eq!(unit.kind.as_str(), "keshig");

    // ⚠ And a REPLACING unique keeps preferring its base: class must only be
    // the rung below `base`, or a Longship would land as a generic hull even
    // when the ruleset models what it replaces.
    let mut state = StateSnapshot {
        turn: 12,
        ..StateSnapshot::default()
    };
    state.units.push(StateUnit {
        id: 10,
        kind: "UNIT_NORWEGIAN_LONGSHIP".to_string(),
        base: Some("UNIT_GALLEY".to_string()),
        class: Some("PROMOTION_CLASS_NAVAL_MELEE".to_string()),
        x: 2,
        y: 2,
        hp: 100.0,
        ..StateUnit::default()
    });
    let both = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    let unit = both
        .game
        .units
        .values()
        .next()
        .expect("the base rung must still fire");
    assert_eq!(unit.kind.as_str(), "galley", "base outranks class");
}

/// ★★★★★ The game speed Civilization VI is running must reach the board.
///
/// The ladder plays `GAMESPEED_ONLINE`, whose costs are HALF of Standard, and a
/// mirrored game kept `GameSpeed::Standard` because nothing read the field —
/// so every tech, civic, district and unit cost CIVVIS reasoned about was
/// double what the game would charge, on every turn of every run.
#[test]
fn the_game_speed_civ6_is_running_reaches_the_board() {
    assert_eq!(
        civvis_game_speed("GAMESPEED_ONLINE"),
        Some(crate::setup::GameSpeed::Online),
        "the export's GameSpeedType must map onto CIVVIS's own speed"
    );
    // ⚠ The two must actually DIFFER in cost, or this fix is decoration.
    assert_ne!(
        crate::setup::GameSpeed::Online.scale(100.0),
        crate::setup::GameSpeed::Standard.scale(100.0),
        "Online and Standard must price differently for this to matter"
    );
    assert_eq!(
        civvis_game_speed("GAMESPEED_NOT_A_SPEED"),
        None,
        "an unknown speed must leave the default alone, not guess"
    );

    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![plot(3, 3, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 30,
        ..StateSnapshot::default()
    };
    state.seat.speed = "GAMESPEED_ONLINE".to_string();
    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    assert_eq!(
        recon.game.game_speed,
        crate::setup::GameSpeed::Online,
        "a reconstruction must run at the speed Civilization VI reported"
    );
}

/// ★★★★★ A city Civilization VI says is AT its district cap must stop offering
/// district sites on the board.
///
/// Run `civvis-20260801T065721Z` (Rome, 195 turns, defeat) discarded **157**
/// district requests through `build_no_plot`, and **157 of 157** were made while
/// the city was at or over Civilization VI's population cap:
///
/// ```text
/// city      pop  specialty  cap=ceil(pop/3)   requests
/// Ravenna     4          3                2        79
/// Gao         5          3                2        23
/// Ostia       8          4                3        19
/// ```
///
/// CIVVIS models the cap correctly and always has — `Game::district_sites`
/// computes `1 + (pop - 1) / 3`, the same 1/4/7 ladder Civilization VI uses. So
/// the rule is not the defect; the only way CIVVIS can ask anyway is if the
/// MIRRORED city carries the wrong population or is missing the districts it has
/// already built. This test pins both through the reconstruction rather than
/// asserting the rule in isolation, which `Game`'s own tests already do.
///
/// ⚠ Two-sided on purpose. "No sites" passes trivially when the city owns no
/// workable ground, so the under-cap case must FIRST prove a site is offered.
#[test]
fn a_city_at_its_civ6_district_cap_offers_no_more_sites() {
    // A city needs workable ground before `district_sites` can offer anything.
    // ⚠ Ownership is not decoration here. A mirrored city works only the ground
    // the export says it owns, so plots left at `o: -1` give it none and
    // `district_sites` is empty for every district regardless of the cap.
    let plots: Vec<_> = (0..12)
        .flat_map(|y| (0..12).map(move |x| (x, y)))
        .map(|(x, y)| {
            let mut p = plot(x, y, "TERRAIN_GRASS");
            if (x - 5).abs() <= 3 && (y - 5).abs() <= 3 {
                p.o = 0;
            }
            p
        })
        .collect();
    let build = |districts: Vec<&str>, pop: i32| {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 30,
            width: 12,
            height: 12,
            chunk: 1,
            plots: plots.clone(),
        }]);
        let mut state = StateSnapshot {
            turn: 30,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Ravenna".to_string(),
            x: 5,
            y: 5,
            pop,
            districts: districts
                .iter()
                .enumerate()
                .map(|(i, kind)| StateDistrict {
                    kind: (*kind).to_string(),
                    x: 4 + i as i32 % 3,
                    y: 4,
                    pillaged: false,
                    complete: true,
                    ..StateDistrict::default()
                })
                .collect(),
            ..StateCity::default()
        });
        let mut recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
        // Districts are tech- and civic-gated, and a reconstruction starts with
        // neither, so without this the precondition fails on unlocks rather than
        // on anything to do with the cap.
        let techs: Vec<Name> = recon
            .game
            .rules
            .techs
            .keys()
            .map(|t| Name::new(t.as_str()))
            .collect();
        let civics: Vec<Name> = recon
            .game
            .rules
            .civics
            .keys()
            .map(|c| Name::new(c.as_str()))
            .collect();
        for tech in techs {
            recon.game.players[0].techs.insert(tech);
        }
        for civic in civics {
            recon.game.players[0].civics.insert(civic);
        }
        recon
    };

    // Ravenna's real shape: population 4, so the cap is 2.
    let under = build(vec!["DISTRICT_CITY_CENTER"], 4);
    let (&cid, city) = under
        .game
        .cities
        .iter()
        .find(|(_, c)| c.owner == 0)
        .expect("the seat's city must be on the board");
    assert_eq!(
        city.pop, 4,
        "the mirrored city must carry the population Civilization VI reported"
    );

    // ⚠ DISCOVERED, not hardcoded — placement rules differ per district, so
    // naming one risks failing on siting rather than on the cap.
    let probe = under
        .game
        .rules
        .districts
        .iter()
        .filter(|(_, spec)| spec.specialty)
        .map(|(name, _)| Name::new(name.as_str()))
        .find(|name| !under.game.district_sites(cid, name).is_empty())
        .expect("a pop-4 city under its cap must be able to site SOME specialty district");

    // Same city, same population, but three specialty districts already built.
    let at_cap = build(
        vec![
            "DISTRICT_CITY_CENTER",
            "DISTRICT_CAMPUS",
            "DISTRICT_HOLY_SITE",
            "DISTRICT_INDUSTRIAL_ZONE",
        ],
        4,
    );
    let (&capped, city) = at_cap
        .game
        .cities
        .iter()
        .find(|(_, c)| c.owner == 0)
        .expect("the seat's city must be on the board");
    let built = city
        .districts
        .keys()
        .filter(|name| at_cap.game.rules.districts[*name].specialty)
        .count();
    assert_eq!(
        built, 3,
        "every specialty district Civilization VI has built must reach the board — \
             a city that reads as bare ground is exactly how CIVVIS asked 79 times"
    );
    assert!(
        at_cap.game.district_sites(capped, probe).is_empty(),
        "population 4 allows 1 + (4-1)/3 = 2 specialty districts and this city has 3, \
             so CIVVIS must stop choosing {probe}"
    );
}

/// ★★★★★ An enemy city under fog must stay on the board — the SAME defect as the
/// tile memory, one field over, and I only fixed the tiles.
///
/// Measured on live run `civvis-20260801T045406Z` at turn 198, at war and losing:
/// 7 enemy cities in the export, all revealed, on land and unoccupied; **7 placed
/// on the reconstruction** (`follow.log`: "7 rival cities"); and **1** visible in
/// the seated observation. `grep -c remembered_cities src/mirror.rs` answered 0.
///
/// ⚠ `findWarTarget` needs a revealed rival city, and "no enemy city is ever
/// revealed … domination is arithmetically impossible" is a standing note in this
/// project. The cities were on the board the whole time; the seat could not
/// remember them.
///
/// ⚠ Asserted through `observation_player_view`, never against
/// `remembered_cities`: a test that counted the memory map would pass on a memory
/// the viewer never consults — exactly the trap the tile-memory test had to avoid.
#[test]
fn an_enemy_city_under_fog_stays_on_the_board() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 20,
        height: 20,
        chunk: 1,
        plots: (4..9)
            .flat_map(|x| (4..9).map(move |y| plot(x, y, "TERRAIN_GRASS")))
            .collect(),
    }]);
    let mut state = StateSnapshot {
        turn: 30,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Canberra".to_string(),
        x: 4,
        y: 4,
        pop: 4,
        ..StateCity::default()
    });
    state.rivals.push(StateRival {
        player: 3,
        at_war: true,
        cities: vec![StateCity {
            id: 2,
            name: "Berlin".to_string(),
            x: 8,
            y: 8,
            pop: 6,
            ..StateCity::default()
        }],
        ..StateRival::default()
    });

    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    let enemy = recon
        .game
        .cities
        .values()
        .find(|c| c.owner != 0)
        .expect("the rival city must be planted on the board");
    let enemy_pos = enemy.pos;

    // The seat has no unit near Berlin, so it is fogged — precisely the case that
    // used to erase it.
    let visible = recon.game.player_visibility(0);
    assert!(
        !visible.contains(&enemy_pos),
        "the enemy city must genuinely be under fog for this to mean anything"
    );

    let view = crate::obs::observation_player_view(&recon.game, 0);
    let cities = view["cities"].as_array().expect("a city list");
    let names: Vec<&str> = cities.iter().filter_map(|c| c["name"].as_str()).collect();
    assert!(
        names.contains(&"Berlin"),
        "a fogged enemy city the seat has seen must still be on the board — this \
             is what made domination unreachable: {names:?}"
    );
}

/// ★★★★ A trader cannot be walked in Civilization VI, and CIVVIS kept trying.
///
/// CIVVIS's ruleset gives `trader` 2 moves; Civ 6 gives it
/// `AiType="UNITTYPE_TRADE"` and reports `moves: 0` on every export. Granting it
/// full ruleset movement made CIVVIS plan steps the host refuses every time:
/// measured with the `move_refused` instrument on run
/// `civvis-20260801T065721Z`, ONE trader produced **22 of 33** move refusals by
/// turn 70, shuffling between four tiles for 38 turns.
/// ★★★★ A SPY CANNOT BE WALKED EITHER, and unlike the trader the export gives
/// it real movement points — so the ruleset value cannot be trusted here.
///
/// Measured over every run recorded on 2026-08-03: **893 of 1,197 refused
/// adjacent moves (75%) were `UNIT_SPY`**, all on our own territory, with
/// single spies stuck and re-ordered for 43 to 81 consecutive turns.
#[test]
fn a_spy_is_given_no_movement_even_though_civ6_reports_some() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 20,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 20,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Canberra".to_string(),
        x: 5,
        y: 5,
        pop: 4,
        ..StateCity::default()
    });
    // ⚠ The precondition that makes this test worth having: Civilization VI
    // exports a spy WITH movement (1, 2 and 3 were all observed), so nothing
    // in the export tells the bridge this unit cannot walk.
    state.units.push(StateUnit {
        id: 5439532,
        kind: "UNIT_SPY".to_string(),
        x: 5,
        y: 6,
        moves: 2.0,
        ..StateUnit::default()
    });
    state.units.push(StateUnit {
        id: 5439533,
        kind: "UNIT_WARRIOR".to_string(),
        x: 5,
        y: 5,
        ..StateUnit::default()
    });

    let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let moves_of = |board: &LiveMirror, want: &str| -> Option<f64> {
        board
            .game
            .units
            .values()
            .find(|u| u.kind.as_str() == want)
            .map(|u| u.moves_left)
    };
    assert_eq!(
        moves_of(&mirror, "spy"),
        Some(0.0),
        "a spy must be given no walking movement — Civilization VI refuses every \
             MOVE_TO for one however many movement points it reports"
    );
    assert!(
        moves_of(&mirror, "warrior").is_some_and(|m| m > 0.0),
        "every other unit keeps its ruleset movement"
    );
}

#[test]
fn an_embarked_unit_keeps_dynamic_fresh_turn_movement() {
    let mut plots = (3..=9)
        .flat_map(|x| (3..=9).map(move |y| plot(x, y, "TERRAIN_GRASS")))
        .collect::<Vec<_>>();
    plots
        .iter_mut()
        .find(|site| site.x == 6 && site.y == 5)
        .expect("the embarked unit's plot is in the fixture")
        .t = Some("TERRAIN_COAST".to_string());
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 20,
        width: 12,
        height: 12,
        chunk: 1,
        plots,
    }]);
    let mut state = StateSnapshot {
        turn: 20,
        techs: vec![
            "TECH_SAILING".to_string(),
            "TECH_SHIPBUILDING".to_string(),
            "TECH_CARTOGRAPHY".to_string(),
            "TECH_SQUARE_RIGGING".to_string(),
            "TECH_STEAM_POWER".to_string(),
        ],
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Canberra".to_string(),
        x: 5,
        y: 5,
        pop: 4,
        capital: true,
        ..StateCity::default()
    });
    state.units.push(StateUnit {
        id: 42,
        kind: "UNIT_SETTLER".to_string(),
        x: 6,
        y: 5,
        moves: 0.0,
        ..StateUnit::default()
    });

    let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let uid = *mirror.uid_of.get(&42).expect("the Settler is mirrored");
    let unit = &mirror.game.units[&uid];
    let static_moves = mirror.game.rules.units["settler"].moves;
    let dynamic_moves = mirror.game.unit_max_moves(uid);
    assert!(
        dynamic_moves > static_moves,
        "the test needs a real embarked movement bonus"
    );
    assert_eq!(
        unit.moves_left, dynamic_moves,
        "fresh-turn mirror movement must include dynamic embarked bonuses"
    );
    let land_step = mirror
        .game
        .nbrs(unit.pos)
        .into_iter()
        .find(|pos| {
            mirror
                .game
                .map
                .get(*pos)
                .is_some_and(|tile| !mirror.game.rules.is_water(tile))
        })
        .expect("the coast has a revealed land neighbor");
    assert!(
        mirror.game.can_move(uid, land_step),
        "the dynamic allowance must pay the first disembark step"
    );
}

/// The seat's strategic stockpiles reach the board: a Bombard needs Niter,
/// a Trebuchet is obsolete once a Bombard can be built. The won game
/// civvis-20260816T054344Z ordered a Trebuchet the host refused on 29 turns
/// because the board had no Niter and no Bombard.
#[test]
fn the_seats_strategic_stockpiles_reach_the_board() {
    let plots = (3..=9)
        .flat_map(|x| (3..=9).map(move |y| plot(x, y, "TERRAIN_GRASS")))
        .collect::<Vec<_>>();
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 120,
        width: 12,
        height: 12,
        chunk: 1,
        plots,
    }]);
    let mut state = StateSnapshot {
        turn: 120,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Rome".to_string(),
        x: 5,
        y: 5,
        pop: 8,
        capital: true,
        ..StateCity::default()
    });
    state.strategic_resources = Some(BTreeMap::from([
        ("RESOURCE_NITER".to_string(), 40.0),
        ("RESOURCE_IRON".to_string(), 12.0),
        ("RESOURCE_UNOBTAINIUM".to_string(), 3.0),
    ]));
    let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let game = &mirror.game;
    assert_eq!(game.strategic_stockpile(0, crate::name!("niter")), 40.0);
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 12.0);
    assert_eq!(game.strategic_stockpile(0, crate::name!("horses")), 0.0);
    assert!(
        mirror
            .unmapped
            .iter()
            .any(|issue| issue == "strategic_resource:RESOURCE_UNOBTAINIUM"),
        "a resource the ruleset does not know is reported: {:?}",
        mirror.unmapped
    );

    // And nothing stocked reads as nothing, not as a deserialisation failure.
    state.strategic_resources = None;
    let empty = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    assert_eq!(
        empty.game.strategic_stockpile(0, crate::name!("niter")),
        0.0
    );
    let parsed: StateSnapshot = serde_json::from_str(r#"{"turn":5,"strategic_resources":[]}"#)
        .expect("an empty stockpile list still parses");
    assert!(
        parsed.strategic_resources.is_none()
            || parsed
                .strategic_resources
                .as_ref()
                .is_some_and(|m| m.is_empty())
    );
}

/// A Great Person is not a unit CIVVIS models, but the ground it stands on
/// is occupied all the same. Run civvis-20260816T003229Z: the founded
/// zero-charge Prophet stood beside the capital for 130 turns and a Builder
/// was ordered onto its tile on 25 consecutive turns.
#[test]
fn a_great_persons_plot_is_occupied_ground_the_builder_routes_around() {
    let plots = (3..=9)
        .flat_map(|x| (3..=9).map(move |y| plot(x, y, "TERRAIN_GRASS")))
        .collect::<Vec<_>>();
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 80,
        width: 12,
        height: 12,
        chunk: 1,
        plots,
    }]);
    let mut state = StateSnapshot {
        turn: 80,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Rome".to_string(),
        x: 5,
        y: 5,
        pop: 4,
        capital: true,
        ..StateCity::default()
    });
    state.units.push(StateUnit {
        id: 7,
        kind: "UNIT_BUILDER".to_string(),
        x: 5,
        y: 5,
        moves: 2.0,
        build_charges: Some(2),
        ..StateUnit::default()
    });
    state.units.push(StateUnit {
        id: 9,
        kind: "UNIT_GREAT_PROPHET".to_string(),
        x: 6,
        y: 5,
        moves: 0.0,
        ..StateUnit::default()
    });
    state.rivals.push(StateRival {
        civ: "CIVILIZATION_SWEDEN".to_string(),
        units: vec![StateUnit {
            id: 11,
            kind: "UNIT_GREAT_GENERAL".to_string(),
            x: 8,
            y: 8,
            moves: 0.0,
            ..StateUnit::default()
        }],
        ..StateRival::default()
    });

    let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let game = &mirror.game;
    let prophet_plot = crate::hex::offset_to_axial(6, 5);
    let general_plot = crate::hex::offset_to_axial(8, 8);
    assert!(
        !mirror.uid_of.contains_key(&9),
        "the Prophet is still not a unit on the board"
    );
    assert_eq!(
        game.great_person_plots.get(&prophet_plot),
        Some(&0),
        "but its plot is recorded as ground the seat's own Great Person holds"
    );
    assert!(
        game.great_person_plots
            .get(&general_plot)
            .is_some_and(|owner| *owner != 0),
        "and a rival's Great Person is recorded to its owner"
    );
    assert!(
        game.valid_improvements(0, prophet_plot).is_empty(),
        "the plot offers a Builder nothing, so it is never chosen as a target"
    );
    let uid = *mirror.uid_of.get(&7).expect("the Builder is mirrored");
    assert!(
        !game.can_move(uid, prophet_plot),
        "and the Builder cannot step onto it, as Firaxis will refuse the step"
    );
    let open = game
        .nbrs(game.units[&uid].pos)
        .into_iter()
        .find(|pos| *pos != prophet_plot && game.map.get(*pos).is_some())
        .expect("the capital has another neighbour");
    assert!(
        game.can_move(uid, open),
        "the neighbouring plots without a Great Person stay open"
    );
}

/// `range_attack_refused` and `war_refused` for the CURRENT turn become the
/// state's `refused_strikes`, and the mirror files them on the board as
/// `Game::blocked_strikes` in CIVVIS ids and axial tiles. Last turn's refusals
/// and the DeclareWar `war_refused` (which names a `target` player, not a
/// unit and plot) stay out.
#[test]
fn a_strike_the_host_refused_this_turn_reaches_the_board() {
    let dir = std::env::temp_dir().join(format!("civvis_strikes_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("events.jsonl");
    std::fs::write(
        &path,
        concat!(
            r#"{"kind":"range_attack_refused","turn":40,"unit":3342338,"unit_kind":"UNIT_ARCHER","x":7,"y":5,"moves":2,"attacks":1,"activity":"ACTIVITY_AWAKE","why":"No line of sight [p4r]"}"#,
            "\n",
            r#"{"kind":"war_refused","turn":40,"unit":5111818,"verb":"ATTACK","x":8,"y":5,"players":[3],"target_owner":3}"#,
            "\n",
            r#"{"kind":"war_refused","turn":40,"target":3,"at_war":false,"has_met":true}"#,
            "\n",
            r#"{"kind":"range_attack_refused","turn":39,"unit":3342338,"x":6,"y":5,"why":"Target is out of range [p4r]"}"#,
            "\n",
        ),
    )
    .expect("write events");

    let refused = refused_strikes_on(&path, 40);
    assert_eq!(
        refused,
        [(3342338, 7, 5), (5111818, 8, 5)].into_iter().collect(),
        "this turn's two named pairs, and neither last turn's nor the DeclareWar refusal"
    );
    assert!(
        refused_strikes_on(&path, 41).is_empty(),
        "per turn, not cumulative"
    );

    let unit_ids: std::collections::BTreeMap<u32, i64> = [(7u32, 3342338i64), (9u32, 5111818i64)]
        .into_iter()
        .collect();
    let blocked = blocked_strikes_from(&refused, &unit_ids);
    assert_eq!(
        blocked,
        [
            (7u32, crate::hex::offset_to_axial(7, 5)),
            (9u32, crate::hex::offset_to_axial(8, 5)),
        ]
        .into_iter()
        .collect(),
        "translated to CIVVIS unit ids and axial tiles"
    );
    let unmapped: std::collections::BTreeMap<u32, i64> = [(7u32, 1i64)].into_iter().collect();
    assert!(
        blocked_strikes_from(&refused, &unmapped).is_empty(),
        "a refusal for a unit the board does not carry gates nothing"
    );

    // Through the mirror: the exported unit's refusal lands on its board id.
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| plot(x, y, "TERRAIN_GRASS")))
            .collect(),
    }]);
    let mut state = StateSnapshot {
        turn: 40,
        units: vec![StateUnit {
            id: 3342338,
            kind: "UNIT_ARCHER".to_string(),
            x: 5,
            y: 5,
            hp: 100.0,
            moves: 2.0,
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };
    state.refused_strikes = refused;
    let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let uid = mirror
        .civ6_of
        .iter()
        .find_map(|(uid, civ6)| (*civ6 == 3342338).then_some(*uid))
        .expect("the archer is mirrored");
    assert_eq!(
        *mirror.game.blocked_strikes,
        [(uid, crate::hex::offset_to_axial(7, 5))]
            .into_iter()
            .collect(),
        "the archer's refused shot is on the board; the unmapped unit's is not"
    );
    assert!(mirror
        .game
        .strike_blocked(uid, crate::hex::offset_to_axial(7, 5)));
    assert!(!mirror
        .game
        .strike_blocked(uid, crate::hex::offset_to_axial(6, 5)));

    let _ = std::fs::remove_dir_all(&dir);
}

/// A `preview` event for the CURRENT turn becomes the state's `host_previews`
/// and reaches the mirrored board as `Game::host_previews`, readable through
/// `Game::host_preview(uid, target, ranged)`. Last turn's answers, a later
/// answer for the same key, and a unit the board does not carry are handled.
#[test]
fn a_host_strike_preview_reaches_the_board() {
    let dir = std::env::temp_dir().join(format!("civvis_previews_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("events.jsonl");
    std::fs::write(
        &path,
        concat!(
            r#"{"kind":"preview","turn":40,"frame":0,"unit":3342338,"verb":"RANGE_ATTACK","x":7,"y":5,"preview":{"attacker_strength":25,"defender_strength":15,"damage_to_attacker":0,"damage_to_defender":31,"defender_wall_damage":0}}"#,
            "\n",
            r#"{"kind":"preview","turn":40,"frame":1,"unit":3342338,"verb":"RANGE_ATTACK","x":7,"y":5,"preview":{"attacker_strength":25,"defender_strength":12,"damage_to_attacker":0,"damage_to_defender":38}}"#,
            "\n",
            r#"{"kind":"preview","turn":40,"frame":0,"unit":5111818,"verb":"ATTACK","x":8,"y":5,"preview":{"attacker_strength":20,"defender_strength":20,"damage_to_attacker":24,"damage_to_defender":26,"defender_wall_damage":0}}"#,
            "\n",
            r#"{"kind":"preview","turn":39,"frame":0,"unit":3342338,"verb":"RANGE_ATTACK","x":6,"y":5,"preview":{"attacker_strength":25,"defender_strength":15,"damage_to_attacker":0,"damage_to_defender":30}}"#,
            "\n",
            r#"{"kind":"combat","turn":40,"attacker":{"id":3342338},"preview":{"damage_to_defender":31}}"#,
            "\n",
        ),
    )
    .expect("write events");

    let previews = host_previews_on(&path, 40);
    assert_eq!(
        previews.len(),
        2,
        "two keys this turn; last turn's and the combat's stay out"
    );
    let archer = &previews[&(3342338, 7, 5, "RANGE_ATTACK".to_string())];
    assert_eq!(
        (
            archer.defender_strength,
            archer.damage_to_defender,
            archer.defender_wall_damage
        ),
        (12.0, 38, 0),
        "the frame-1 answer replaces frame 0's, and an absent field lands as zero"
    );
    assert!(
        host_previews_on(&path, 41).is_empty(),
        "per turn, not cumulative"
    );

    let unit_ids: std::collections::BTreeMap<u32, i64> = [(7u32, 3342338i64)].into_iter().collect();
    let filed = host_previews_from(&previews, &unit_ids);
    assert_eq!(
        filed.keys().copied().collect::<Vec<_>>(),
        vec![(7u32, crate::hex::offset_to_axial(7, 5), true)],
        "translated to the CIVVIS unit id, an axial tile and the ranged flag; the unmapped unit is dropped"
    );

    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| plot(x, y, "TERRAIN_GRASS")))
            .collect(),
    }]);
    let mut state = StateSnapshot {
        turn: 40,
        units: vec![StateUnit {
            id: 3342338,
            kind: "UNIT_ARCHER".to_string(),
            x: 5,
            y: 5,
            hp: 100.0,
            moves: 2.0,
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };
    state.host_previews = previews;
    let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let uid = mirror
        .civ6_of
        .iter()
        .find_map(|(uid, civ6)| (*civ6 == 3342338).then_some(*uid))
        .expect("the archer is mirrored");
    let target = crate::hex::offset_to_axial(7, 5);
    assert_eq!(
        mirror.game.host_preview(uid, target, true),
        Some((25.0, 12.0, 0, 38)),
        "the host's own price of the shot is on the board"
    );
    assert_eq!(
        mirror.game.host_preview(uid, target, false),
        None,
        "the melee answer was never asked for"
    );
    assert_eq!(
        mirror
            .game
            .host_preview(uid, crate::hex::offset_to_axial(6, 5), true),
        None,
        "last turn's answer is not this turn's"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_promotion_the_host_refused_is_not_offered_again() {
    let dir = std::env::temp_dir().join(format!("civvis_promo_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("events.jsonl");
    std::fs::write(
        &path,
        concat!(
            r#"{"kind":"promotion_refused","unit":3342338,"promotion":"PROMOTION_TRANSLATOR","turn":40}"#,
            "\n",
            r#"{"kind":"promotion_refused","unit":3342338,"promotion":"PROMOTION_TRANSLATOR","turn":41}"#,
            "\n",
            r#"{"kind":"promotion_refused","unit":5111818,"promotion":"PROMOTION_ECHELON","turn":42}"#,
            "\n",
            r#"{"kind":"promotion_refused","unit":3342338,"promotion":"PROMOTION_CHAPLAIN","turn":90}"#,
            "\n",
        ),
    )
    .expect("write events");

    let refused = refused_promotions_through(&path, Some(50));
    assert_eq!(
        refused.get(&3342338).map(|names| names.len()),
        Some(1),
        "the turn limit keeps the turn-90 Chaplain refusal out"
    );
    assert!(
        refused[&3342338].contains("PROMOTION_TRANSLATOR"),
        "the refused promotion is recorded under its Civilization VI unit id"
    );
    assert!(refused[&5111818].contains("PROMOTION_ECHELON"));

    let later = refused_promotions_through(&path, Some(120));
    assert_eq!(
        later[&3342338].len(),
        2,
        "both distinct refusals are in hand once the game reaches turn 90"
    );

    let rules = crate::rules::Rules::embedded();
    let unit_ids: std::collections::BTreeMap<u32, i64> = [(7u32, 3342338i64), (9u32, 5111818i64)]
        .into_iter()
        .collect();
    let blocked = blocked_promotions_from(&later, &unit_ids, &rules);
    assert!(
        blocked[&7].contains(&crate::name::Name::new("translator")),
        "the host name PROMOTION_TRANSLATOR is TRANSLATED to the CIVVIS rule name, \
             not interned raw — `available_promotions` compares CIVVIS names"
    );
    assert!(blocked[&9].contains(&crate::name::Name::new("echelon")));
    assert!(
        !blocked.contains_key(&11),
        "a unit the host never refused carries no block"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_trader_is_given_no_movement_because_civ6_gives_it_none() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 20,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 20,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Canberra".to_string(),
        x: 5,
        y: 5,
        pop: 4,
        ..StateCity::default()
    });
    state.units.push(StateUnit {
        id: 786439,
        kind: "UNIT_TRADER".to_string(),
        x: 5,
        y: 6,
        ..StateUnit::default()
    });
    state.units.push(StateUnit {
        id: 786440,
        kind: "UNIT_WARRIOR".to_string(),
        x: 5,
        y: 5,
        ..StateUnit::default()
    });

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);

    let moves_of = |board: &LiveMirror, want: &str| -> Option<f64> {
        board
            .game
            .units
            .values()
            .find(|u| u.kind.as_str() == want)
            .map(|u| u.moves_left)
    };
    assert_eq!(
        moves_of(&mirror, "trader"),
        Some(0.0),
        "a trader must be given no movement — Civilization VI reports moves: 0 for \
             it on every export, and every walk CIVVIS planned for one was refused"
    );
    // ⚠ And nothing else is grounded by this. A warrior keeps the movement the
    // ruleset gives it; the fix is about one unit class, not about movement.
    assert!(
        moves_of(&mirror, "warrior").is_some_and(|m| m > 0.0),
        "every other unit keeps its ruleset movement"
    );

    // `civvis_orders --serve --fresh-board` follows this exact construction
    // path and never calls `sync`, so the constructor must carry the rule.
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(moves_of(&mirror, "trader"), Some(0.0));
}

#[test]
fn active_trade_routes_follow_the_host_and_keep_the_visible_trader() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 20,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| plot(x, y, "TERRAIN_GRASS")))
            .collect(),
    }]);
    let mut state = StateSnapshot {
        turn: 20,
        seat: Seat {
            city_states: 1,
            ..Seat::default()
        },
        civics: vec!["CIVIC_FOREIGN_TRADE".to_string()],
        cities: vec![
            StateCity {
                id: 7,
                name: "Roma".to_string(),
                x: 5,
                y: 5,
                pop: 3,
                capital: true,
                loyalty: 100.0,
                ..StateCity::default()
            },
            StateCity {
                id: 8,
                name: "Antium".to_string(),
                x: 6,
                y: 6,
                pop: 3,
                loyalty: 100.0,
                ..StateCity::default()
            },
        ],
        units: vec![StateUnit {
            id: 42,
            kind: "UNIT_TRADER".to_string(),
            x: 6,
            y: 6,
            moves: 0.0,
            ..StateUnit::default()
        }],
        trade_routes: vec![StateTradeRoute {
            trader: 42,
            origin: 8,
            destination: 7,
            origin_x: 6,
            origin_y: 6,
            destination_x: 5,
            destination_y: 5,
            posts_own: Some(2),
            posts_foreign: Some(1),
            // The host's Trade Overview is authoritative here: a route
            // can earn from a destination district that this seat has not
            // revealed, which the model must not invent from the fog.
            yields: Some(crate::rules::Yields {
                food: 2.0,
                production: 3.0,
                gold: 7.0,
                science: 5.0,
                culture: 11.0,
                faith: 13.0,
            }),
            ..StateTradeRoute::default()
        }],
        // Firaxis allocates city ids per player. This city-state's first city
        // deliberately has the same id as our Antium.
        minors: vec![StateMinor {
            player: 6,
            civ: "CIVILIZATION_ZANZIBAR".to_string(),
            cities: vec![StateCity {
                id: 8,
                name: "Zanzibar".to_string(),
                x: 9,
                y: 9,
                pop: 3,
                ..StateCity::default()
            }],
            ..StateMinor::default()
        }],
        ..StateSnapshot::default()
    };

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let trader = mirror.uid_of[&42];
    assert!(mirror.game.units.contains_key(&trader));
    assert_eq!(mirror.game.active_routes(0), 1);
    assert!(mirror.active_trade_route_traders.contains(&42));
    assert_eq!(mirror.game.routes[0].origin, mirror.cid_of[&8]);
    assert_eq!(mirror.game.routes[0].dest, mirror.cid_of[&7]);
    assert_eq!(
        mirror.game.cities[&mirror.game.routes[0].origin].owner, 0,
        "a colliding city-state id must not steal the route origin"
    );
    // The host's own path and its Trading Posts stand in for the model's
    // straight-line walk, and survive a save.
    let key = (mirror.game.routes[0].origin, mirror.game.routes[0].dest);
    assert_eq!(mirror.game.observed_route_posts.get(&key), Some(&(2, 1)));
    let host_route = crate::rules::Yields {
        food: 2.0,
        production: 3.0,
        gold: 7.0,
        science: 5.0,
        culture: 11.0,
        faith: 13.0,
    };
    assert_eq!(
        mirror.game.observed_route_yields.get(&key),
        Some(&host_route)
    );
    // The host's total replaces the model's complete route calculation,
    // rather than being added to it — otherwise an unseen Campus earns
    // twice. Removing the route leaves exactly its six host values behind.
    let origin = mirror.game.routes[0].origin;
    let routed = mirror.game.city_yields(origin);
    let mut no_route = mirror.game.clone();
    no_route.routes.clear();
    let baseline = no_route.city_yields(origin);
    for (label, observed, got, base) in [
        ("food", host_route.food, routed.food, baseline.food),
        (
            "production",
            host_route.production,
            routed.production,
            baseline.production,
        ),
        ("gold", host_route.gold, routed.gold, baseline.gold),
        (
            "science",
            host_route.science,
            routed.science,
            baseline.science,
        ),
        (
            "culture",
            host_route.culture,
            routed.culture,
            baseline.culture,
        ),
        ("faith", host_route.faith, routed.faith, baseline.faith),
    ] {
        assert!(
            ((got - base) - observed).abs() < 1e-9,
            "the host's {label} replaces the route model: {base} + {observed} != {got}"
        );
    }
    let saved: crate::game::Game =
        serde_json::from_str(&serde_json::to_string(&mirror.game).unwrap()).unwrap();
    assert_eq!(saved.observed_route_posts.get(&key), Some(&(2, 1)));
    assert_eq!(saved.observed_route_yields.get(&key), Some(&host_route));

    // The next authoritative state is the only thing allowed to complete a
    // route.  A persistent mirror must stop counting it immediately once the
    // host reports it gone, rather than waiting for CIVVIS's guessed duration.
    state.turn = 21;
    state.trade_routes.clear();
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.active_routes(0), 0);
    assert!(mirror.active_trade_route_traders.is_empty());
    assert!(mirror.game.units.contains_key(&trader));
    assert!(mirror.game.observed_route_posts.is_empty());
    assert!(mirror.game.observed_route_yields.is_empty());
}

#[test]
fn a_city_carries_the_religion_it_follows_and_the_one_converting_it() {
    // ⚠ THIS FIELD EXISTED AND WAS NEVER FILLED. `religion` was null on all
    // 26,954 city records ever exported — the schema had it, the mod never sent
    // it, and nothing failed. So the test is not "does the struct have a field",
    // it is "does the export shape actually deserialize into one".
    let raw = r#"{
            "id": 7, "name": "Nidaros", "x": 12, "y": 9, "pop": 6,
            "buildings": ["BUILDING_MONUMENT"],
            "religion": "RELIGION_CATHOLICISM",
            "religion_next": "RELIGION_BUDDHISM",
            "religion_turns": 4
        }"#;
    let city: StateCity = serde_json::from_str(raw).expect("city shape parses");
    assert_eq!(city.religion.as_deref(), Some("RELIGION_CATHOLICISM"));
    // The level alone cannot distinguish a city holding steady from one about
    // to flip, which is the `loyalty` / `loyalty_per_turn` lesson again.
    assert_eq!(city.religion_next.as_deref(), Some("RELIGION_BUDDHISM"));
    assert_eq!(city.religion_turns, 4);

    // An unconverted city omits them rather than sending an index, and must
    // still parse — "could not ask" and "follows nothing" both read as None.
    let bare = r#"{"id": 8, "name": "Ålesund", "x": 1, "y": 2, "pop": 1}"#;
    let plain: StateCity = serde_json::from_str(bare).expect("bare city parses");
    assert!(plain.religion.is_none() && plain.religion_next.is_none());
    assert_eq!(plain.religion_turns, 0);
}

#[test]
fn an_actionable_conversion_clock_warns_without_inventing_a_majority() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![
            plot(5, 5, "TERRAIN_GRASS"),
            plot(10, 5, "TERRAIN_GRASS"),
            plot(15, 5, "TERRAIN_GRASS"),
        ],
    }]);
    let state = StateSnapshot {
        turn: 30,
        cities: vec![
            StateCity {
                id: 1,
                name: "Faithless".to_string(),
                x: 5,
                y: 5,
                pop: 4,
                religion_next: Some("RELIGION_BUDDHISM".to_string()),
                religion_turns: 20,
                ..StateCity::default()
            },
            StateCity {
                id: 2,
                name: "Catholic".to_string(),
                x: 10,
                y: 5,
                pop: 4,
                religion: Some("RELIGION_CATHOLICISM".to_string()),
                religion_next: Some("RELIGION_BUDDHISM".to_string()),
                religion_turns: 20,
                ..StateCity::default()
            },
            StateCity {
                id: 3,
                name: "Distant".to_string(),
                x: 15,
                y: 5,
                pop: 4,
                religion_next: Some("RELIGION_HINDUISM".to_string()),
                religion_turns: 21,
                ..StateCity::default()
            },
        ],
        ..StateSnapshot::default()
    };
    let rebuilt = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let city = |name: &str| {
        rebuilt
            .game
            .cities
            .values()
            .find(|city| city.name == name)
            .unwrap_or_else(|| panic!("missing mirrored city {name}"))
    };

    let faithless = city("Faithless");
    assert_eq!(faithless.pressure.get("Buddhism").copied(), Some(1.0));
    assert_eq!(rebuilt.game.city_religion(faithless), None);

    let catholic = city("Catholic");
    assert_eq!(catholic.pressure.get("Catholicism").copied(), Some(100.0));
    assert_eq!(catholic.pressure.get("Buddhism").copied(), Some(60.0));
    assert_eq!(rebuilt.game.city_religion(catholic), Some("Catholicism"));

    let distant = city("Distant");
    assert!(!distant.pressure.contains_key("Hinduism"));
}

/// ★★★★ A border that grows after the mirror is built must still be learned.
///
/// `apply_territory` ran only in `rebuild_from_state`, which a persistent mirror
/// calls once — at construction. Every border that grew afterwards stayed unowned
/// on CIVVIS's board for the rest of the game. Measured on live run
/// `civvis-20260801T012454Z` at turn 43: **28 of 243** paired plots were owned in
/// Civilization VI and unowned in CIVVIS, and **none** the other way.
///
/// ⚠ Asserted through `valid_improvements`, not against `owner_city` directly,
/// because that is where the cost lands: the function returns an empty list for a
/// tile whose `owner_city` is None, so a builder on ground the seat really owns is
/// offered nothing to build. A test that only compared the ownership field would
/// pass on an ownership nothing consults.
#[test]
fn a_border_that_grows_after_construction_is_still_learned() {
    let founded = |x: i32, y: i32| StateCity {
        id: 1,
        name: "Nidaros".to_string(),
        x,
        y,
        pop: 4,
        ..StateCity::default()
    };
    // Turn 4: one plot revealed and owned, and the city that owns it.
    let owned = |x: i32, y: i32, owner: i32| {
        let mut p = plot(x, y, "TERRAIN_GRASS");
        p.o = owner;
        p
    };
    let first = Snapshot::from_chunks(&[TilesChunk {
        turn: 4,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![owned(5, 5, 0), owned(5, 6, -1)],
    }]);
    let mut state = StateSnapshot {
        turn: 4,
        ..StateSnapshot::default()
    };
    state.cities.push(founded(5, 5));

    let mut mirror = LiveMirror::new(&first, &state, 4, 1, 500, 0);
    let grown = crate::hex::offset_to_axial(5, 6);
    assert!(
        mirror
            .game
            .map
            .get(grown)
            .is_some_and(|t| t.owner_city.is_none()),
        "the plot starts unowned, which is what the export said on turn 4"
    );

    // Turn 8: the border has grown over it. Nothing else about the world changed.
    let later = Snapshot::from_chunks(&[TilesChunk {
        turn: 8,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![owned(5, 5, 0), owned(5, 6, 0)],
    }]);
    state.turn = 8;
    mirror.sync(&later, &state, 0);

    assert!(
        mirror
            .game
            .map
            .get(grown)
            .is_some_and(|t| t.owner_city.is_some()),
        "a border that grew after construction must be learned — this is the whole \
             defect, and before the fix it stayed unowned for the rest of the game"
    );
    // And the consequence that actually costs games: ground the seat owns must
    // offer a builder something to do on it.
    assert!(
        !mirror.game.valid_improvements(0, grown).is_empty(),
        "owned ground must offer improvements; an unowned tile offers none, which \
             is how a stale border silently stops an empire developing"
    );
}

/// ★★★★★ THE EXPORT IS THE HOST'S VISIBILITY ANSWER, AND WE RE-DERIVED IT.
///
/// Civilization VI exports a rival's units only under CURRENT visibility, the
/// bridge plants exactly those, and then `player_vision_now` recomputes what
/// the seat can see from this engine's sight radii on a reconstructed map.
/// Where the two disagree, an enemy the host is showing us is invisible to the
/// agent deciding whether to shoot it — and `ForcePosture` only reaches
/// `Engage` through `g.sees(..) && battlefront_unit_visible(..)`.
///
/// ⚠⚠⚠ Measured on live run `civvis-20260803T191900Z` across the 49 turns of
/// Kongo's war (t203-250), which cost Arpinum and Arretium and ended the game
/// at 479 against the winner's 1214: an enemy was in the export on 49 of 49
/// turns, our units stood adjacent to one on 95 unit-turns and within range 2
/// on 197. **37 attacks were issued** -- 81% of the shots the host was showing
/// the army were declined, and the force logged "still gathering" instead.
///
/// ⚠ The second assertion is the one that keeps this honest. The inference
/// "a foreign unit is on the board, so we must be able to see it" is sound
/// ONLY for a mirrored board. Applied to an ordinary game it would hand every
/// AI perfect vision of the world, so the set must stay empty there.
#[test]
fn a_rival_the_host_exported_is_visible_however_sight_is_derived() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 12,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(15, 15, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 12,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 5,
        y: 5,
        pop: 4,
        ..StateCity::default()
    });
    state.units.push(StateUnit {
        id: 10,
        kind: "UNIT_WARRIOR".to_string(),
        x: 5,
        y: 5,
        hp: 100.0,
        ..StateUnit::default()
    });
    // Ten hexes away — far outside anything our own sight model reaches, and
    // in the export only because Civilization VI can see it.
    state.rivals.push(StateRival {
        player: 3,
        at_war: true,
        units: vec![StateUnit {
            id: 20,
            kind: "UNIT_WARRIOR".to_string(),
            x: 15,
            y: 15,
            hp: 100.0,
            ..StateUnit::default()
        }],
        ..StateRival::default()
    });

    let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let enemy = crate::hex::offset_to_axial(15, 15);
    let empty = crate::hex::offset_to_axial(18, 18);

    assert!(
        mirror
            .game
            .units
            .values()
            .any(|unit| unit.owner != 0 && unit.pos == enemy),
        "the exported rival must reach the board at all — the rest of this test \
             is about whether the agent is allowed to notice it"
    );
    assert!(
        mirror.game.player_can_see(0, enemy),
        "a unit Civilization VI exported is a unit Civilization VI is showing \
             us; before this fix the engine re-derived sight on a reconstructed map \
             and answered no, and the army declined 81% of its shots in a war it lost"
    );
    assert!(
        !mirror.game.player_can_see(0, empty),
        "and only that ground: far tiles with nothing exported on them must stay \
             dark, or the repair is just omniscience wearing a fix's clothes"
    );

    // ★ The invariant that keeps ordinary play honest. A native game holds the
    // FULL simulation, so foreign units on the board prove nothing about sight.
    let native = crate::game::Game::new(4, 20, 20, 7, 500, 0);
    assert!(
        native.host_observed.is_empty(),
        "an ordinary CIVVIS game must leave this set empty; reading the board the \
             mirrored way there would give every AI player perfect vision"
    );
}

/// ★★★★★ A RIVAL'S BORDER IS INVISIBLE PRECISELY BECAUSE IT IS IN THE WAY.
///
/// `can_enter` reads `territory_owner_at`, which resolves a plot through
/// `owner_city -> cities -> owner`. A rival whose cities this seat has never
/// SEEN owns no city on the mirrored board, so their border resolves to `None`
/// and reads as free ground — and we cannot see the city that would fix that,
/// because the border is what stops us walking to it.
///
/// ⚠⚠⚠ Measured on live run `civvis-20260803T191900Z` (Rome, SETTLER, small).
/// Scout `196608` reached offset (12,24) on turn 42 and was ordered
/// `MOVE_TO (11,24)` — one hex — on **74 separate turns** without ever moving.
/// (11,24) is exported `o: 4`: Kongo's, with no war and no open borders.
/// **81 of 670 `MOVE_TO` orders targeted foreign ground and all 81 were
/// counted `applied`** — a blocked move is a silent no-op, not a refusal, so
/// every turn read as healthy while the empire went blind. Exploration
/// flatlined at 283 of 3404 tiles, no rival city was seen in 96 snapshots,
/// `plan.target_city` stayed `None`, and forty turns of `strategy=conquest`
/// with `war_legal=9` produced no war at all.
///
/// ⚠ Asserted through `can_move`, not against `closed_borders` directly, for
/// the same reason the border-growth test above asserts through
/// `valid_improvements`: the field only matters where it is consulted, and a
/// test on the field alone would pass on a set nothing reads.
/// ★★★★ A met major's border plot whose city cannot be safely resolved is
/// a city in the fog. Run civvis-20260826T030045Z founded Lugdunum five
/// tiles from Germany's border; the only visible German city was ten tiles
/// away, so the settle-site forecast passed it before the host reported
/// −22 Loyalty a turn. `unseen_major_borders` names a rival with no city on
/// the board, a plot beyond every known city ownership ring, and a fifth-ring
/// plot whose reported owner could equally be a nearer unseen city. A minor's
/// ground and a plot securely inside a known rival city's ring are not in it.
#[test]
fn a_met_majors_border_without_a_safe_city_attribution_is_recorded_as_unseen() {
    let owned = |x: i32, y: i32, owner: i32| {
        let mut p = plot(x, y, "TERRAIN_GRASS");
        p.o = owner;
        p
    };
    let mut plots = vec![owned(5, 5, 0)];
    // Rival 3 owns (5,7) and (5,8) and none of their cities is in sight.
    plots.push(owned(5, 7, 3));
    plots.push(owned(5, 8, 3));
    // Rival 4 owns (14, 5) beside their known city at (15, 5), (10, 5) on
    // the fifth ring of that city, and (5, 12) ten tiles from it. The
    // latter two could belong to a closer city we cannot see.
    plots.push(owned(15, 5, 4));
    plots.push(owned(14, 5, 4));
    plots.push(owned(10, 5, 4));
    plots.push(owned(5, 12, 4));
    // A city-state owns (9, 9); minors exert no loyalty pressure and are
    // not majors.
    plots.push(owned(9, 9, 7));
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 12,
        width: 20,
        height: 20,
        chunk: 1,
        plots,
    }]);
    let mut state = StateSnapshot {
        turn: 12,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 5,
        y: 5,
        pop: 4,
        ..StateCity::default()
    });
    state.rivals.push(StateRival {
        player: 3,
        at_war: false,
        cities: Vec::new(),
        ..StateRival::default()
    });
    state.rivals.push(StateRival {
        player: 4,
        at_war: false,
        cities: vec![StateCity {
            id: 2,
            name: "Hue".to_string(),
            x: 15,
            y: 5,
            pop: 5,
            ..StateCity::default()
        }],
        ..StateRival::default()
    });
    state.minors.push(StateMinor {
        player: 7,
        civ: "CIVILIZATION_KUMASI".to_string(),
        ..StateMinor::default()
    });

    let mirror = LiveMirror::new(&snapshot, &state, 5, 1, 500, 0);
    let at = |x: i32, y: i32| crate::hex::offset_to_axial(x, y);
    let unseen = &mirror.game.unseen_major_borders;
    assert!(
        unseen.contains(&at(5, 7)) && unseen.contains(&at(5, 8)),
        "a met major with no city on the board: its ground is an unseen border, got {unseen:?}"
    );
    assert!(
        unseen.contains(&at(5, 12)),
        "ten tiles from the only known city of theirs: a city we cannot see owns it"
    );
    assert!(
        unseen.contains(&at(10, 5)),
        "the fifth ring is ambiguous: a nearer unseen city can own it"
    );
    assert!(
        !unseen.contains(&at(14, 5)),
        "beside their known city: the forecast can count that city"
    );
    assert!(
        !unseen.contains(&at(9, 9)),
        "a minor's ground presses no loyalty and is not a major's border"
    );
    assert!(
        !unseen.contains(&at(5, 5)),
        "our own ground is never an unseen border"
    );
}

#[test]
fn a_rival_border_whose_city_is_unseen_still_stops_the_unit() {
    let owned = |x: i32, y: i32, owner: i32| {
        let mut p = plot(x, y, "TERRAIN_GRASS");
        p.o = owner;
        p
    };
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 12,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![owned(5, 5, 0), owned(5, 6, 3), owned(4, 5, -1)],
    }]);
    let mut state = StateSnapshot {
        turn: 12,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 5,
        y: 5,
        pop: 4,
        ..StateCity::default()
    });
    state.units.push(StateUnit {
        id: 10,
        kind: "UNIT_SCOUT".to_string(),
        x: 5,
        y: 5,
        hp: 100.0,
        ..StateUnit::default()
    });
    // Met, nameable, and NOT at war — but not one of their cities is in sight,
    // which is the whole condition. `cities` is deliberately empty.
    state.rivals.push(StateRival {
        player: 3,
        at_war: false,
        cities: Vec::new(),
        ..StateRival::default()
    });

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let theirs = crate::hex::offset_to_axial(5, 6);
    let neutral = crate::hex::offset_to_axial(4, 5);
    let scout = *mirror
        .game
        .player_unit_ids(0)
        .first()
        .expect("the scout must reach the board");

    assert!(
        mirror
            .game
            .map
            .get(theirs)
            .is_some_and(|t| t.owner_city.is_none()),
        "their plot has no owning city on this board — that is the premise, not \
             the defect: we have never seen the city that holds it"
    );
    assert!(
        !mirror.game.can_move(scout, theirs),
        "a rival's ground must stop the unit even when the mirror cannot name the \
             city that owns it — before this fix the step was legal on CIVVIS's board, \
             was ordered 74 times on one live run, and silently did nothing every time"
    );
    assert!(
        mirror.game.can_move(scout, neutral),
        "genuinely neutral ground must stay open, or the fix would seal the empire \
             in instead of the border out"
    );

    // ★ And war must OPEN it again on the next sync. The seat is named, so its
    // diplomacy is answerable even with its cities unseen; sealing ground we have
    // just declared war on would lock our own invasion out — which is a worse
    // failure than the one being repaired.
    state.rivals[0].at_war = true;
    state.turn = 13;
    mirror.sync(&snapshot, &state, 0);
    let scout = *mirror
        .game
        .player_unit_ids(0)
        .first()
        .expect("the scout survives the sync");
    assert!(
        !mirror.game.closed_borders.contains(&theirs),
        "war opens the border: the seal is recomputed from the export every turn \
             and must not outlive the peace that justified it"
    );
    assert!(
        mirror.game.can_move(scout, theirs),
        "once at war the unit must be able to cross — the repair must not cost us \
             the invasion it exists to make possible"
    );
}

/// ★★★★★ THE HOST'S MENU IS THE CATALOGUE. A city whose exported
/// `buildable` lacks a Spearman never offers one, however legal the board's
/// own rules say it is; its `purchasable` price is the price the buy lane
/// pays; a queue two deep reads as two deep; a complete district offer is
/// the whole site list; and an export without the keys leaves the board
/// exactly as before.
#[test]
fn a_city_offers_and_prices_only_what_the_hosts_menus_list() {
    let owned = |x: i32, y: i32| {
        let mut p = plot(x, y, "TERRAIN_GRASS");
        p.o = 0;
        p
    };
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 12,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![
            owned(5, 5),
            owned(5, 6),
            owned(6, 5),
            owned(4, 5),
            owned(5, 4),
        ],
    }]);
    let row = |t: &str, c: f64, p: f64| StateMenuItem {
        t: t.to_string(),
        c,
        p,
        ..StateMenuItem::default()
    };
    let mut state = StateSnapshot {
        turn: 12,
        techs: vec![
            "TECH_BRONZE_WORKING".to_string(),
            "TECH_WRITING".to_string(),
            "TECH_POTTERY".to_string(),
        ],
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 5,
        y: 5,
        pop: 4,
        producing: Some("UNIT_WARRIOR".to_string()),
        buildable: Some(vec![
            row("UNIT_WARRIOR", 40.0, 5.0),
            row("BUILDING_MONUMENT", 61.0, 7.0),
            row("BUILDING_GRANARY", 65.0, 8.0),
            StateMenuItem {
                t: "DISTRICT_CAMPUS".to_string(),
                c: 54.0,
                p: 9.0,
                n: Some(1),
                s: Some(vec![StateMenuPlot { x: 5, y: 6 }]),
                ..StateMenuItem::default()
            },
        ]),
        purchasable: Some(vec![
            StatePurchaseItem {
                t: "BUILDING_GRANARY".to_string(),
                g: Some(340.0),
                f: None,
            },
            StatePurchaseItem {
                t: "UNIT_BUILDER".to_string(),
                g: Some(210.0),
                f: Some(105.0),
            },
        ]),
        queue: Some(vec![StateQueueItem {
            t: "UNIT_SETTLER".to_string(),
            f: None,
            pr: Some(3.0),
        }]),
        ..StateCity::default()
    });

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let cid = mirror.game.player_city_ids(0)[0];
    let unit = |name: &str| crate::game::Item::Unit {
        unit: crate::name::Name::new(name),
    };
    let spearman = unit("spearman");
    let warrior = unit("warrior");
    let monument = crate::game::Item::Building {
        building: crate::name!("monument"),
    };
    assert!(
        mirror.game.can_produce(0, cid, &warrior),
        "a listed item stays producible"
    );
    assert!(
        !mirror.game.can_produce(0, cid, &spearman),
        "the host did not list a Spearman: not producible, however legal \
             the board's own rules say it is"
    );
    let menu = mirror.game.producible_items(0, cid);
    assert!(
        menu.contains(&warrior) && !menu.contains(&spearman),
        "the menu every chooser reads agrees: {menu:?}"
    );
    assert_eq!(
        mirror.game.item_cost_for_city(0, cid, &monument),
        61.0,
        "the planner prices from the host's cost"
    );
    assert_eq!(mirror.game.host_production_turns(cid, &monument), Some(7.0));

    // A complete district offer is the whole site list, and a site off it
    // is not producible.
    let campus = crate::name!("campus");
    let offered = crate::hex::offset_to_axial(5, 6);
    let sites = mirror.game.district_sites(cid, campus);
    assert_eq!(sites, vec![offered], "sites: {sites:?}");
    let elsewhere = crate::game::Item::District {
        district: campus,
        pos: crate::hex::offset_to_axial(6, 5),
    };
    assert!(!mirror.game.can_produce(0, cid, &elsewhere));

    // The queue behind the head.
    assert_eq!(
        mirror.game.cities[&cid].queue,
        vec![warrior.clone(), unit("settler")],
        "the head and the queue behind it"
    );

    // The host's price is the price, and off the purchase menu is not for
    // sale — through the pricers the lanes call and the enumeration.
    assert_eq!(
        mirror
            .game
            .building_purchase_cost(0, cid, "granary", "gold"),
        Some(340.0)
    );
    assert_eq!(
        mirror
            .game
            .building_purchase_cost(0, cid, "granary", "faith"),
        None
    );
    assert_eq!(
        mirror
            .game
            .building_purchase_cost(0, cid, "monument", "gold"),
        None
    );
    assert_eq!(
        mirror.game.unit_purchase_cost(0, cid, "builder", "gold"),
        Some(210.0)
    );
    assert_eq!(
        mirror.game.unit_purchase_cost(0, cid, "builder", "faith"),
        Some(105.0)
    );
    assert_eq!(
        mirror.game.unit_purchase_cost(0, cid, "warrior", "gold"),
        None
    );
    let buys_granary = |game: &crate::game::Game| {
        game.legal_actions_within(0, crate::game::ActionFamilies::PURCHASES)
            .iter()
            .any(|action| {
                matches!(
                    action,
                    crate::game::Action::BuyBuilding { city, building, currency }
                        if *city == cid && building == "granary" && currency == "gold"
                )
            })
    };
    mirror.game.players[0].gold = 339.0;
    assert!(
        !buys_granary(&mirror.game),
        "339 Gold does not buy a 340 Granary"
    );
    mirror.game.players[0].gold = 340.0;
    assert!(
        buys_granary(&mirror.game),
        "the buy lane enumerates from the host's price"
    );

    // The rebuild path reads the same menus.
    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    let fresh = rebuilt.game.player_city_ids(0)[0];
    assert!(!rebuilt.game.can_produce(0, fresh, &spearman));
    assert_eq!(
        rebuilt
            .game
            .building_purchase_cost(0, fresh, "granary", "gold"),
        Some(340.0)
    );
    assert_eq!(rebuilt.game.cities[&fresh].queue.len(), 2);

    // An EMPTY buildable list is a failed read, not a city that can build
    // nothing; an empty PURCHASE list is the host saying nothing is for sale.
    state.cities[0].buildable = Some(Vec::new());
    state.cities[0].purchasable = Some(Vec::new());
    state.turn = 13;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        mirror.game.can_produce(0, cid, &spearman),
        "an empty menu gates nothing"
    );
    assert_eq!(
        mirror
            .game
            .building_purchase_cost(0, cid, "granary", "gold"),
        None
    );

    // Without the keys — an older mod — the board is exactly as before.
    state.cities[0].buildable = None;
    state.cities[0].purchasable = None;
    state.cities[0].queue = None;
    state.turn = 14;
    mirror.sync(&snapshot, &state, 0);
    assert!(mirror.game.host_buildable.is_empty());
    assert!(mirror.game.host_purchasable.is_empty());
    assert!(mirror.game.host_district_plots.is_empty());
    assert!(mirror.game.can_produce(0, cid, &spearman));
    assert_eq!(mirror.game.cities[&cid].queue, vec![warrior]);
    assert!(
        mirror
            .game
            .building_purchase_cost(0, cid, "granary", "gold")
            .is_some(),
        "the model's own price is back"
    );
}

/// The host's spelling of every production family comes back to the key
/// the gate reads, tier included; a name the board does not model is
/// dropped rather than guessed.
#[test]
fn host_menu_rows_translate_to_the_gates_keys() {
    let rules = crate::rules::Rules::embedded();
    let key = |civ6: &str, tier: Option<u8>| host_production_key(&rules, civ6, tier);
    assert_eq!(key("UNIT_WARRIOR", None).as_deref(), Some("unit:warrior"));
    assert_eq!(
        key("UNIT_WARRIOR", Some(1)).as_deref(),
        Some("formation:warrior:1")
    );
    assert_eq!(
        key("UNIT_WARRIOR", Some(2)).as_deref(),
        Some("formation:warrior:2")
    );
    assert_eq!(
        key("BUILDING_LIBRARY", None).as_deref(),
        Some("building:library")
    );
    assert_eq!(
        key("BUILDING_PYRAMIDS", None).as_deref(),
        Some("wonder:pyramids")
    );
    assert_eq!(
        key("DISTRICT_GOVERNMENT", None).as_deref(),
        Some("district:government_plaza")
    );
    assert_eq!(
        key("PROJECT_ENHANCE_DISTRICT_CAMPUS", None).as_deref(),
        Some("project:campus_research_grants")
    );
    assert_eq!(key("UNIT_NOT_A_UNIT", None), None);
}

/// A board with one met rival on seat 1, for the host-diplomacy tests.
fn diplomacy_board(turn: u32, rival: StateRival) -> (Snapshot, StateSnapshot) {
    let owned = |x: i32, y: i32, owner: i32| {
        let mut p = plot(x, y, "TERRAIN_GRASS");
        p.o = owner;
        p
    };
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![owned(5, 5, 0), owned(9, 9, 3)],
    }]);
    let mut state = StateSnapshot {
        turn,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 5,
        y: 5,
        pop: 4,
        ..StateCity::default()
    });
    state.units.push(StateUnit {
        id: 10,
        kind: "UNIT_WARRIOR".to_string(),
        x: 5,
        y: 5,
        hp: 100.0,
        ..StateUnit::default()
    });
    state.rivals.push(rival);
    (snapshot, state)
}

fn diplomacy_actions(game: &crate::game::Game) -> Vec<crate::game::Action> {
    game.legal_actions_within(0, crate::game::ActionFamilies::DIPLOMACY)
}

fn formal_war_legal(game: &crate::game::Game) -> bool {
    diplomacy_actions(game).iter().any(|action| {
        matches!(
            action,
            crate::game::Action::DeclareWarWithCasusBelli { player: 1, casus_belli }
                if casus_belli == "formal_war"
        )
    })
}

/// ★★★★★ The host's DENOUNCED state and its grievance ledger land on the
/// board, on the right sides, with the host's own clock — and a later
/// NEUTRAL export clears them. FIDELITY.md "The one-to-one map", item 1:
/// every war, peace and denounce decision was made blind to this.
#[test]
fn a_host_denouncement_and_its_grievances_land_on_the_board_and_a_neutral_export_clears_them() {
    let rival = StateRival {
        player: 3,
        can_declare: true,
        diplomatic_state: Some("DIPLO_STATE_DENOUNCED".to_string()),
        our_denounce_turn: Some(38),
        their_denounce_turn: Some(0),
        denounce_time_limit: Some(30),
        our_grievances_against_them: Some(0.0),
        grievances_against_us: Some(50.0),
        ..StateRival::default()
    };
    let (snapshot, mut state) = diplomacy_board(40, rival);
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let game = &mirror.game;
    assert_eq!(
        game.relationship_state(0, 1),
        "denounced",
        "the host's DENOUNCED must be the board's"
    );
    assert_eq!(
        game.players[0].denounced_since.get(&1).copied(),
        Some(38),
        "the denouncement started when the host says it did"
    );
    assert_eq!(
        game.players[0].denounced_until.get(&1).copied(),
        Some(38 + 30 + 1),
        "and runs the host's DenounceTimeLimit (+1, DiplomacyActionView.lua:1500)"
    );
    assert!(
        !game.players[1].denounced_until.contains_key(&0),
        "a denounce turn of 0 on their side means they did not denounce us"
    );
    assert_eq!(
        game.players[1].grievances.get(&0).copied(),
        Some(50.0),
        "50 grievances against us sit on THEIR ledger under our seat"
    );
    assert!(
        !game.players[0].grievances.contains_key(&1),
        "and none on ours: one signed balance per pair"
    );
    assert!(
        !formal_war_legal(game),
        "two turns into our own denouncement the Formal War wait has not \
             matured — the board waits the host's five turns, not a faked one"
    );
    assert!(
        !diplomacy_actions(game)
            .iter()
            .any(|action| matches!(action, crate::game::Action::Denounce { player: 1 })),
        "an active denouncement cannot be repeated"
    );

    // Five host turns on: the Formal War the engine's own rule allows.
    state.turn = 43;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        formal_war_legal(&mirror.game),
        "at since + 5 the host's clock opens the Formal War"
    );

    // The host now says NEUTRAL with a clean ledger and no permission.
    state.turn = 44;
    state.rivals[0].diplomatic_state = Some("DIPLO_STATE_NEUTRAL".to_string());
    state.rivals[0].our_denounce_turn = Some(38);
    state.rivals[0].grievances_against_us = Some(0.0);
    state.rivals[0].can_declare = false;
    mirror.sync(&snapshot, &state, 0);
    let game = &mirror.game;
    assert_eq!(game.relationship_state(0, 1), "neutral");
    assert!(!game.players[0].denounced_until.contains_key(&1));
    assert!(!game.players[0].denounced_since.contains_key(&1));
    assert!(!game.players[1].grievances.contains_key(&0));
    assert!(game.alliance_with(0, 1).is_none() && !game.are_friends(0, 1));
}

/// ALLIED level 2 → an `alliances` entry on both seats with the host's
/// kind, level and expiry past this turn; NEUTRAL afterwards clears it.
#[test]
fn a_host_alliance_lands_with_its_kind_level_and_expiry_and_clears_on_neutral() {
    let rival = StateRival {
        player: 3,
        diplomatic_state: Some("DIPLO_STATE_ALLIED".to_string()),
        alliance_type: Some("ALLIANCE_MILITARY".to_string()),
        alliance_level: Some(2),
        alliance_turns_left: Some(12),
        ..StateRival::default()
    };
    let (snapshot, mut state) = diplomacy_board(60, rival);
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let game = &mirror.game;
    let alliance = game
        .alliance_with(0, 1)
        .expect("the host's alliance is on our seat");
    assert_eq!(alliance.kind, "military");
    assert_eq!(alliance.level, 2);
    assert_eq!(
        alliance.ends,
        60 + 12 + 1,
        "ends past this turn, at the host's expiry"
    );
    assert!(game.are_allied(1, 0), "and on theirs");
    assert!(game.are_friends(0, 1), "an alliance implies the friendship");
    assert_eq!(game.relationship_state(0, 1), "allied");
    assert!(
        !diplomacy_actions(game).iter().any(|action| matches!(
            action,
            crate::game::Action::DeclareWar { player: 1 }
                | crate::game::Action::Denounce { player: 1 }
        )),
        "no war and no denouncement against an ally"
    );

    state.turn = 61;
    state.rivals[0].diplomatic_state = Some("DIPLO_STATE_NEUTRAL".to_string());
    state.rivals[0].alliance_type = None;
    state.rivals[0].alliance_level = None;
    state.rivals[0].alliance_turns_left = None;
    mirror.sync(&snapshot, &state, 0);
    let game = &mirror.game;
    assert!(game.alliance_with(0, 1).is_none() && game.alliance_with(1, 0).is_none());
    assert!(!game.are_friends(0, 1));
    assert_eq!(game.relationship_state(0, 1), "neutral");
}

/// DECLARED_FRIEND → `friends_until` both sides from the host's
/// friendship turn, and the board withholds war and denouncement the way
/// the host does; NEUTRAL clears it.
#[test]
fn a_host_declared_friendship_lands_and_bars_war_until_a_neutral_export() {
    let rival = StateRival {
        player: 3,
        can_declare: false,
        diplomatic_state: Some("DIPLO_STATE_DECLARED_FRIEND".to_string()),
        friendship_turn: Some(30),
        denounce_time_limit: Some(30),
        ..StateRival::default()
    };
    let (snapshot, mut state) = diplomacy_board(40, rival);
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let game = &mirror.game;
    assert!(game.are_friends(0, 1) && game.are_friends(1, 0));
    assert_eq!(
        game.players[0].friends_until.get(&1).copied(),
        Some(60),
        "friendship runs from the host's turn for DenounceTimeLimit turns (:1511)"
    );
    assert_eq!(game.relationship_state(0, 1), "declared_friend");
    assert!(
        !diplomacy_actions(game).iter().any(|action| matches!(
            action,
            crate::game::Action::DeclareWar { player: 1 }
                | crate::game::Action::DeclareWarWithCasusBelli { player: 1, .. }
                | crate::game::Action::Denounce { player: 1 }
        )),
        "the host refuses war and denouncement against a declared friend; so must the board"
    );

    state.turn = 41;
    state.rivals[0].diplomatic_state = Some("DIPLO_STATE_NEUTRAL".to_string());
    state.rivals[0].friendship_turn = Some(0);
    mirror.sync(&snapshot, &state, 0);
    assert!(!mirror.game.are_friends(0, 1) && !mirror.game.are_friends(1, 0));
    assert_eq!(mirror.game.relationship_state(0, 1), "neutral");
}

/// Missions, promises, visibility and the grant WE make cross both ways
/// and clear when the next export withdraws them; the same rules on the
/// rebuild path.
#[test]
fn host_missions_promises_visibility_and_our_grant_cross_both_ways() {
    let rival = StateRival {
        player: 3,
        diplomatic_state: Some("DIPLO_STATE_FRIENDLY".to_string()),
        embassy_at: Some(true),
        delegation_at: Some(false),
        their_embassy: Some(false),
        their_delegation: Some(true),
        promises_made: Some(vec!["DONT_SETTLE_NEAR_ME".to_string()]),
        promises_received: Some(vec![
            "DONT_SPY_ON_ME".to_string(),
            "DONT_CONVERT_MY_CITIES".to_string(),
        ]),
        visibility: Some(2),
        their_visibility_on_us: Some(1),
        open_borders_granted: Some(true),
        ..StateRival::default()
    };
    let (snapshot, mut state) = diplomacy_board(50, rival);
    // Rebuild path first: the same helper, a fresh board.
    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    assert_eq!(
        rebuilt
            .game
            .diplomatic_mission_to(0, 1)
            .map(|m| m.kind.as_str()),
        Some("embassy")
    );
    assert_eq!(rebuilt.game.diplomatic_visibility(0, 1), 2.0);

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let game = &mirror.game;
    assert_eq!(
        game.diplomatic_mission_to(0, 1).map(|m| m.kind.as_str()),
        Some("embassy"),
        "our Resident Embassy at their court"
    );
    assert_eq!(
        game.diplomatic_mission_to(1, 0).map(|m| m.kind.as_str()),
        Some("delegation"),
        "their Delegation at ours"
    );
    assert!(
        game.players[0]
            .promises
            .get(&1)
            .is_some_and(|book| book.contains_key("no_settling")),
        "a promise WE made sits on our ledger under their seat, as the engine kind"
    );
    let theirs = game.players[1]
        .promises
        .get(&0)
        .expect("their promises to us");
    assert!(theirs.contains_key("no_spying") && theirs.contains_key("no_conversion"));
    assert_eq!(
        game.diplomatic_visibility(0, 1),
        2.0,
        "the host's visibility level outranks the board's derivation"
    );
    assert_eq!(game.diplomatic_visibility(1, 0), 1.0);
    assert!(
        game.players[0]
            .open_borders_until
            .get(&1)
            .is_some_and(|until| *until > 50),
        "the Open Borders we grant is on our seat under theirs"
    );
    assert_eq!(
        game.relationship_state(0, 1),
        "neutral",
        "FRIENDLY is no treaty"
    );

    state.turn = 51;
    state.rivals[0].embassy_at = Some(false);
    state.rivals[0].their_delegation = Some(false);
    state.rivals[0].promises_made = Some(Vec::new());
    state.rivals[0].promises_received = Some(Vec::new());
    state.rivals[0].visibility = None;
    state.rivals[0].their_visibility_on_us = Some(0);
    state.rivals[0].open_borders_granted = Some(false);
    mirror.sync(&snapshot, &state, 0);
    let game = &mirror.game;
    assert!(game.diplomatic_mission_to(0, 1).is_none());
    assert!(game.diplomatic_mission_to(1, 0).is_none());
    assert!(!game.players[0].promises.contains_key(&1));
    assert!(!game.players[1].promises.contains_key(&0));
    assert!(
        !game.players[0].observed_visibility.contains_key(&1),
        "a reading the host would not give falls back to the derivation"
    );
    assert_eq!(game.diplomatic_visibility(1, 0), 0.0);
    assert!(!game.players[0].open_borders_until.contains_key(&1));
}

/// ⚠ The old mod exports no `diplomatic_state`: the `can_declare`
/// permission fake is written exactly as before on both paths, and a new
/// export that says NEUTRAL while the host permits a declaration still
/// carries it — the bridge has no `Denounce` order, so this is the only
/// path by which a permitted war is ever declared.
#[test]
fn an_export_without_diplomatic_state_keeps_the_can_declare_fallback() {
    let rival = StateRival {
        player: 3,
        can_declare: true,
        ..StateRival::default()
    };
    let (snapshot, mut state) = diplomacy_board(40, rival);
    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    assert_eq!(
        rebuilt.game.players[0].denounced_until.get(&1).copied(),
        Some(41),
        "rebuild path: the permission fake, unchanged"
    );
    assert!(formal_war_legal(&rebuilt.game));
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    assert_eq!(
        mirror.game.players[0].denounced_until.get(&1).copied(),
        Some(41)
    );
    assert!(
        mirror.game.players[0].grievances.is_empty()
            && mirror.game.players[1].grievances.is_empty()
            && mirror.game.players[0].observed_visibility.is_empty(),
        "nothing else is invented for an export that does not carry it"
    );

    // Withdrawn permission clears the fake (sync path).
    state.turn = 41;
    state.rivals[0].can_declare = false;
    mirror.sync(&snapshot, &state, 0);
    assert!(!mirror.game.players[0].denounced_until.contains_key(&1));

    // A NEUTRAL export with the permission keeps the fake alive.
    state.turn = 42;
    state.rivals[0].can_declare = true;
    state.rivals[0].diplomatic_state = Some("DIPLO_STATE_NEUTRAL".to_string());
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(
        mirror.game.players[0].denounced_until.get(&1).copied(),
        Some(43)
    );
    assert!(
        formal_war_legal(&mirror.game),
        "the host permits a declaration; the board must still have a maturing path to one"
    );
}

#[test]
fn a_bought_open_borders_grant_unseals_the_rival_ground() {
    let owned = |x: i32, y: i32, owner: i32| {
        let mut p = plot(x, y, "TERRAIN_GRASS");
        p.o = owner;
        p
    };
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 12,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![owned(5, 5, 0), owned(5, 6, 3), owned(5, 7, 3)],
    }]);
    let mut state = StateSnapshot {
        turn: 12,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 5,
        y: 5,
        pop: 4,
        ..StateCity::default()
    });
    state.units.push(StateUnit {
        id: 10,
        kind: "UNIT_SCOUT".to_string(),
        x: 5,
        y: 5,
        hp: 100.0,
        ..StateUnit::default()
    });
    state.rivals.push(StateRival {
        player: 3,
        at_war: false,
        cities: Vec::new(),
        ..StateRival::default()
    });

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let theirs = crate::hex::offset_to_axial(5, 6);
    assert!(
        mirror.game.closed_borders.contains(&theirs),
        "without a grant the fogged border stays sealed — the premise of the lane"
    );
    assert_eq!(
        mirror.game.sealed_border_owners.get(&1).copied(),
        Some(2),
        "the seal must name its owner and its size, or the buy lane cannot \
             know whom to pay: seat 1 seals both exported plots"
    );

    // The host reports the purchase: this rival now grants us Open
    // Borders. The next sync must stop sealing exactly the ground the
    // seat just paid to cross, and the shopping list must go quiet so
    // the lane never pays the same rival twice.
    state.rivals[0].open_borders = Some(true);
    state.turn = 13;
    mirror.sync(&snapshot, &state, 0);
    let scout = *mirror
        .game
        .player_unit_ids(0)
        .first()
        .expect("the scout survives the sync");
    assert!(
        !mirror.game.closed_borders.contains(&theirs),
        "an explicit grant opens the border: sealing ground the seat just \
             bought passage through would waste exactly what it paid for"
    );
    assert!(
        mirror.game.can_move(scout, theirs),
        "the bought passage must be walkable on the planning board, or the \
             gold buys a fact the planner never uses"
    );
    assert!(
        mirror.game.sealed_border_owners.is_empty(),
        "a granted rival leaves the shopping list, got {:?}",
        mirror.game.sealed_border_owners
    );

    // And a lapsed agreement re-seals on the next export, the same
    // assigned-not-extended rule as war and the seal itself.
    state.rivals[0].open_borders = Some(false);
    state.turn = 14;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        mirror.game.closed_borders.contains(&theirs),
        "a lapsed grant must not leave the border open forever"
    );
    assert_eq!(
        mirror.game.sealed_border_owners.get(&1).copied(),
        Some(2),
        "a lapsed grant puts the rival back on the shopping list"
    );
}

/// docs/FIDELITY.md "The one-to-one map", item 8: a rival's techs and
/// civics crossed as COUNTS, so its era, roster and border were guesses,
/// and Early Empire had to be exported as one bit. With the names on the
/// seat the border is derived the way a native game derives it, and the
/// bit is only the override.
#[test]
fn a_rivals_tree_by_name_seats_its_civics_and_derives_its_border() {
    let owned = |x: i32, y: i32, owner: i32| {
        let mut p = plot(x, y, "TERRAIN_GRASS");
        p.o = owner;
        p
    };
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![owned(5, 5, 0), owned(5, 6, 3), owned(5, 7, 3)],
    }]);
    let mut state = StateSnapshot {
        turn: 30,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 5,
        y: 5,
        pop: 4,
        ..StateCity::default()
    });
    state.rivals.push(StateRival {
        player: 3,
        tech_names: Some(vec![
            "TECH_POTTERY".to_string(),
            "TECH_BRONZE_WORKING".to_string(),
        ]),
        civic_names: Some(vec![
            "CIVIC_CODE_OF_LAWS".to_string(),
            "CIVIC_EARLY_EMPIRE".to_string(),
        ]),
        enforces_borders: None,
        ..StateRival::default()
    });

    // Rebuild path.
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let early_empire = crate::name::Name::new("early_empire");
    let bronze = crate::name::Name::new("bronze_working");
    assert!(
        mirror.game.players[1].civics.contains(&early_empire),
        "the rival's civic names must land on its seat: {:?}",
        mirror.game.players[1].civics
    );
    assert!(
        mirror.game.players[1].techs.contains(&bronze),
        "the rival's tech names must land on its seat: {:?}",
        mirror.game.players[1].techs
    );
    assert_eq!(
        mirror.game.players[1].borders_enforced, None,
        "with the tree on the seat and no bit, the civic decides — the native rule"
    );
    assert!(
        mirror.game.enforces_borders(1),
        "Early Empire on the seat enforces the border without the override"
    );

    // Sync path: the tree is ASSIGNED from every export, so a tree without
    // Early Empire opens the border again.
    state.rivals[0].civic_names = Some(vec!["CIVIC_CODE_OF_LAWS".to_string()]);
    state.turn = 31;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        !mirror.game.players[1].civics.contains(&early_empire),
        "assigned, not merged: a civic the host no longer lists must leave the seat"
    );
    assert!(
        !mirror.game.enforces_borders(1),
        "a rival without Early Empire has no border to enforce"
    );

    // The host's own bit stays the override over the derived answer.
    state.rivals[0].enforces_borders = Some(true);
    state.turn = 32;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.players[1].borders_enforced, Some(true));
    assert!(
        mirror.game.enforces_borders(1),
        "the exported bit wins over the tree"
    );

    // Neither key (an older export): enforced, the conservative answer —
    // unchanged behaviour.
    state.rivals[0] = StateRival {
        player: 3,
        ..StateRival::default()
    };
    state.turn = 33;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.players[1].borders_enforced, Some(true));
    assert!(mirror.game.enforces_borders(1));
}

/// The religion lane of the victory tracker counted a rival converted only
/// from the rival cities the board happened to hold. The shipped screen
/// asks the host (`GetReligionInMajorityOfCities`,
/// `GetNumCitiesFollowingReligion`), which counts cities the seat has
/// never seen; so does the board now, on both import paths.
#[test]
fn a_rivals_religion_lane_reads_the_hosts_majority_and_city_count() {
    let owned = |x: i32, y: i32, owner: i32| {
        let mut p = plot(x, y, "TERRAIN_GRASS");
        p.o = owner;
        p
    };
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 90,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![owned(5, 5, 0), owned(5, 6, 3)],
    }]);
    let mut state = StateSnapshot {
        turn: 90,
        founded_religion: Some("RELIGION_CATHOLICISM".to_string()),
        cities_following_religion: Some(3),
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 5,
        y: 5,
        pop: 6,
        ..StateCity::default()
    });
    // No rival city on the board at all: everything the lane knows about
    // this rival comes from the host's two numbers.
    state.rivals.push(StateRival {
        player: 3,
        religion: Some("RELIGION_CATHOLICISM".to_string()),
        cities_following_religion: Some(12),
        techs_researched: Some(31),
        techs: -1.0,
        ..StateRival::default()
    });

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    assert_eq!(
        mirror.game.players[0].religion.as_deref(),
        Some("Catholicism"),
        "the founded religion seats on our player, the premise of the lane"
    );
    assert_eq!(
        mirror.game.majority_religion_of(1),
        Some("Catholicism"),
        "the host's majority religion is the board's answer for a rival"
    );
    assert!(mirror.game.civ_follows_religion(1, "Catholicism"));
    assert_eq!(
        mirror.game.cities_following_religion(1),
        12,
        "the host's cities-following count reaches the religion-progress reader"
    );
    assert_eq!(mirror.game.cities_following_religion(0), 3, "and ours");
    let races = mirror.game.victory_races(1, 0);
    assert_eq!(races.cities_following_religion, 12);
    assert_eq!(
        races.techs, 31,
        "the science lane's own count wins over the loop count when it crossed"
    );
    assert_eq!(
        mirror.game.victory_races(0, 0).converted_civs,
        1,
        "a rival the host calls converted counts toward our Religious Victory \
             even with none of its cities on the board"
    );

    // Missing keys (an older export) on the sync path: the observed map
    // is rebuilt from each snapshot, so nothing is carried forward and
    // the board's own count — no rival cities — answers.
    state.rivals[0] = StateRival {
        player: 3,
        ..StateRival::default()
    };
    state.cities_following_religion = None;
    state.turn = 91;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.majority_religion_of(1), None);
    assert_eq!(mirror.game.cities_following_religion(1), 0);
    assert_eq!(mirror.game.victory_races(0, 0).converted_civs, 0);
    assert_eq!(
        mirror.game.cities_following_religion(0),
        0,
        "no host count and no converted city on the board"
    );
}

/// The per-rival tourist term, the rival's Era Score and its outgoing
/// routes: each lands where an existing reader already looks
/// (`visiting_tourists_from`, `Player::era_score`, `game.routes`).
#[test]
fn a_rivals_tourists_era_score_and_routes_reach_the_board() {
    let owned = |x: i32, y: i32, owner: i32| {
        let mut p = plot(x, y, "TERRAIN_GRASS");
        p.o = owner;
        p
    };
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 60,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![owned(5, 5, 0), owned(5, 6, 3), owned(8, 8, 3)],
    }]);
    let mut state = StateSnapshot {
        turn: 60,
        // No aggregate from the host, so `foreign_tourists` sums the
        // per-rival terms — the only place the pair is read.
        foreign_tourists: f64::NAN,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 5,
        y: 5,
        pop: 6,
        ..StateCity::default()
    });
    let rival_city = |id: i64, name: &str, x: i32, y: i32| StateCity {
        id,
        name: name.to_string(),
        x,
        y,
        pop: 3,
        ..StateCity::default()
    };
    let route = |ox: i32, oy: i32, dx: i32, dy: i32, player: i32| StateRivalRoute {
        origin_x: ox,
        origin_y: oy,
        destination_x: dx,
        destination_y: dy,
        destination_player: player,
    };
    state.rivals.push(StateRival {
        player: 3,
        tourists_visiting_us: Some(7),
        era_score: Some(9),
        cities: vec![rival_city(20, "Nubt", 5, 6), rival_city(21, "Meroe", 8, 8)],
        trade_routes: Some(vec![
            // Domestic, both ends on the board.
            route(5, 6, 8, 8, 3),
            // Into our capital — the same route `incoming_routes` would
            // seat, so it must not be doubled.
            route(5, 6, 5, 5, 0),
            // To a city that is not on the board: skipped, not guessed.
            route(5, 6, 9, 9, 3),
        ]),
        ..StateRival::default()
    });

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    assert_eq!(
        mirror.game.foreign_tourists(0),
        7,
        "the host's per-rival draw is what the culture lane sums"
    );
    assert_eq!(mirror.game.players[1].era_score, 9);
    let nubt = mirror
        .game
        .city_at(crate::hex::offset_to_axial(5, 6))
        .expect("the rival city is planted");
    let meroe = mirror
        .game
        .city_at(crate::hex::offset_to_axial(8, 8))
        .expect("the rival city is planted");
    let roma = mirror
        .game
        .city_at(crate::hex::offset_to_axial(5, 5))
        .expect("our city is planted");
    let rival_routes: Vec<(u32, u32)> = mirror
        .game
        .routes
        .iter()
        .filter(|route| route.owner == 1)
        .map(|route| (route.origin, route.dest))
        .collect();
    assert_eq!(
        rival_routes,
        vec![(nubt, meroe), (nubt, roma)],
        "both routes with known ends are seated on the rival's seat, the \
             third is skipped"
    );

    // Sync path, same export: rebuilt from scratch, never doubled.
    state.turn = 61;
    mirror.sync(&snapshot, &state, 0);
    let rival_route_count =
        |game: &crate::game::Game| game.routes.iter().filter(|route| route.owner == 1).count();
    assert_eq!(rival_route_count(&mirror.game), 2);
    assert_eq!(mirror.game.foreign_tourists(0), 7);

    // Missing keys: no rival routes, no per-rival term, era score kept
    // from the last export that carried it (the seat's own slot).
    state.rivals[0].trade_routes = None;
    state.rivals[0].tourists_visiting_us = None;
    state.rivals[0].era_score = None;
    state.turn = 62;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(rival_route_count(&mirror.game), 0);
    assert_eq!(mirror.game.foreign_tourists(0), 0);
    assert_eq!(mirror.game.players[1].era_score, 9);
}

/// Run civvis-20260826T184456Z: 122 military `MOVE_TO`s into a
/// non-suzerain city-state's land, 4 % arrived; 36 into a city-state we
/// were Suzerain of, 51 % arrived. The city was in view every time, so the
/// fogged seal never applied and `has_open_borders` read a civic the
/// board never fills.
#[test]
fn a_city_states_visible_land_is_shut_until_suzerainty_war_or_no_border() {
    let owned = |x: i32, y: i32, owner: i32| {
        let mut p = plot(x, y, "TERRAIN_GRASS");
        p.o = owner;
        p
    };
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![owned(5, 5, 0), owned(5, 6, 7), owned(5, 7, 7)],
    }]);
    let mut state = StateSnapshot {
        turn: 40,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 5,
        y: 5,
        pop: 4,
        ..StateCity::default()
    });
    state.units.push(StateUnit {
        id: 10,
        kind: "UNIT_SCOUT".to_string(),
        x: 5,
        y: 5,
        hp: 100.0,
        ..StateUnit::default()
    });
    state.minors.push(StateMinor {
        player: 7,
        civ: "CIVILIZATION_KABUL".to_string(),
        suzerain: -1,
        cities: vec![StateCity {
            id: 70,
            name: "Kabul".to_string(),
            x: 5,
            y: 7,
            pop: 3,
            ..StateCity::default()
        }],
        ..StateMinor::default()
    });
    let theirs = crate::hex::offset_to_axial(5, 6);
    let scout_of = |mirror: &LiveMirror| {
        *mirror
            .game
            .player_unit_ids(0)
            .first()
            .expect("the scout is on the board")
    };

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    assert!(
        !mirror.game.closed_borders.contains(&theirs),
        "attributed ground is not the fogged seal's business"
    );
    assert!(
        !mirror.game.can_move(scout_of(&mirror), theirs),
        "a city-state we do not hold shuts its land to a scout, exactly as \
             the host did 118 times in one game"
    );

    state.minors[0].suzerain = 0;
    state.turn = 41;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        mirror.game.can_move(scout_of(&mirror), theirs),
        "the Suzerain walks through"
    );

    state.minors[0].suzerain = -1;
    state.minors[0].at_war = true;
    state.turn = 42;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        mirror.game.can_move(scout_of(&mirror), theirs),
        "war opens the ground"
    );

    state.minors[0].at_war = false;
    state.minors[0].enforces_borders = Some(false);
    state.turn = 43;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        mirror.game.can_move(scout_of(&mirror), theirs),
        "a city-state without Early Empire has no border to enforce"
    );

    state.minors[0].enforces_borders = Some(true);
    state.turn = 44;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        !mirror.game.can_move(scout_of(&mirror), theirs),
        "and the border returns the turn the host reports it"
    );
}

/// The same game: 37 military steps into a rival's closed border with its
/// city in view, 0 arrived; 7 of 11 arrived once at war. And the seat
/// that shuts the most visible ground is the one the passage-purchase
/// lane should be paying, which only fogged ground used to name.
#[test]
fn a_rivals_visible_land_is_shut_without_a_grant_and_named_for_the_buy_lane() {
    let owned = |x: i32, y: i32, owner: i32| {
        let mut p = plot(x, y, "TERRAIN_GRASS");
        p.o = owner;
        p
    };
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 50,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![owned(5, 5, 0), owned(5, 6, 3), owned(5, 7, 3)],
    }]);
    let mut state = StateSnapshot {
        turn: 50,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 5,
        y: 5,
        pop: 4,
        ..StateCity::default()
    });
    state.units.push(StateUnit {
        id: 10,
        kind: "UNIT_SCOUT".to_string(),
        x: 5,
        y: 5,
        hp: 100.0,
        ..StateUnit::default()
    });
    state.rivals.push(StateRival {
        player: 3,
        at_war: false,
        cities: vec![StateCity {
            id: 30,
            name: "Mbanza Kongo".to_string(),
            x: 5,
            y: 7,
            pop: 5,
            ..StateCity::default()
        }],
        ..StateRival::default()
    });
    let theirs = crate::hex::offset_to_axial(5, 6);
    let scout_of = |mirror: &LiveMirror| {
        *mirror
            .game
            .player_unit_ids(0)
            .first()
            .expect("the scout is on the board")
    };

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    assert!(
        !mirror.game.closed_borders.contains(&theirs),
        "ground attributed to a visible city is gated by diplomacy, not sealed"
    );
    assert!(
        !mirror.game.can_move(scout_of(&mirror), theirs),
        "no grant, no war: the host refused every one of 37 such steps"
    );
    assert_eq!(
        mirror.game.sealed_border_owners.get(&1).copied(),
        Some(2),
        "the rival shutting two visible plots is on the buy lane's list, \
             got {:?}",
        mirror.game.sealed_border_owners
    );

    state.rivals[0].open_borders = Some(true);
    state.turn = 51;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        mirror.game.can_move(scout_of(&mirror), theirs),
        "a bought grant opens the visible border too"
    );
    assert!(
        mirror.game.sealed_border_owners.is_empty(),
        "and retires the purchase trigger, got {:?}",
        mirror.game.sealed_border_owners
    );

    state.rivals[0].open_borders = Some(false);
    state.rivals[0].enforces_borders = Some(false);
    state.turn = 52;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        mirror.game.can_move(scout_of(&mirror), theirs),
        "a rival that has not reached Early Empire has no border yet"
    );
    assert!(
        mirror.game.sealed_border_owners.is_empty(),
        "nothing to buy from a seat with no border, got {:?}",
        mirror.game.sealed_border_owners
    );

    state.rivals[0].enforces_borders = Some(true);
    state.rivals[0].at_war = true;
    state.turn = 53;
    mirror.sync(&snapshot, &state, 0);
    assert!(
        mirror.game.can_move(scout_of(&mirror), theirs),
        "war opens it — the repair must not seal our own invasion out"
    );
}

/// `tools/civ6_yield_drift.py` on run civvis-20260826T184456Z: the
/// model's production and gold read 1.20× the host's, science, culture
/// and faith 1.08×, food 1.00× — King's `ai_yield_pct` to the digit,
/// paid to the one seat the host never pays it to.
#[test]
fn the_mirrored_seat_is_the_human_and_takes_no_ai_handicap() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 10,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 10,
        ..StateSnapshot::default()
    };
    state.seat.difficulty = "DIFFICULTY_KING".to_string();
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 5,
        y: 5,
        pop: 2,
        ..StateCity::default()
    });
    state.rivals.push(StateRival {
        player: 3,
        ..StateRival::default()
    });
    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    assert_eq!(recon.game.difficulty, "king");
    assert!(
        recon.game.is_human_seat(0),
        "the mirrored seat is the human"
    );
    let ours = recon.game.handicap_yield_pct(0);
    assert_eq!(
        (ours.production, ours.gold, ours.science),
        (0.0, 0.0, 0.0),
        "the human takes no yield handicap at King"
    );
    let theirs = recon.game.handicap_yield_pct(1);
    assert!(
        theirs.production > 0.0 && theirs.science > 0.0,
        "the rival seats keep the AI bonus the host pays them, got {theirs:?}"
    );
}

/// `live_divergence::combat_pairs` resolves BOTH sides of a `combat` event
/// through the mirror's id maps; foreign units never had one, so every
/// unit-vs-unit fight read "no pairs".
#[test]
fn foreign_units_keep_their_host_ids_on_both_paths() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 20,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![
            plot(5, 5, "TERRAIN_GRASS"),
            plot(5, 6, "TERRAIN_GRASS"),
            plot(7, 7, "TERRAIN_GRASS"),
        ],
    }]);
    let mut state = StateSnapshot {
        turn: 20,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 5,
        y: 5,
        pop: 3,
        ..StateCity::default()
    });
    state.units.push(StateUnit {
        id: 10,
        kind: "UNIT_WARRIOR".to_string(),
        x: 5,
        y: 5,
        hp: 100.0,
        ..StateUnit::default()
    });
    state.hostiles.push(StateUnit {
        id: 77,
        kind: "UNIT_WARRIOR".to_string(),
        player: 63,
        x: 5,
        y: 6,
        hp: 80.0,
        ..StateUnit::default()
    });

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let barbarian = *mirror
        .foreign_uid_of
        .get(&77)
        .expect("the barbarian's host id is remembered on the rebuild path");
    assert_ne!(mirror.game.units[&barbarian].owner, 0, "and it is not ours");
    assert!(
        !mirror.uid_of.contains_key(&77),
        "foreign ids stay out of `uid_of`, which the sync path prunes against our units"
    );

    // The next export: that barbarian is gone, a different one stands
    // elsewhere. The map follows the board, which is rebuilt wholesale.
    state.turn = 21;
    state.hostiles = vec![StateUnit {
        id: 78,
        kind: "UNIT_SLINGER".to_string(),
        player: 63,
        x: 7,
        y: 7,
        hp: 100.0,
        ..StateUnit::default()
    }];
    mirror.sync(&snapshot, &state, 0);
    assert!(
        !mirror.foreign_uid_of.contains_key(&77),
        "a foreign unit the host no longer reports leaves the map"
    );
    let slinger = *mirror
        .foreign_uid_of
        .get(&78)
        .expect("the new barbarian's host id is remembered on the sync path");
    assert_eq!(
        mirror.game.units[&slinger].pos,
        crate::hex::offset_to_axial(7, 7)
    );
    assert!(
        mirror.uid_of.contains_key(&10),
        "our own unit keeps its map"
    );
}

/// `Plot:IsFreshWater()` crossed as `fw` since the tiles export began and
/// nothing read it; a city centre on a lake the export names
/// `TERRAIN_COAST` derived no fresh water and housed 2, not 5.
#[test]
fn the_hosts_fresh_water_answer_sets_the_city_housing_floor() {
    let mut wet = plot(5, 5, "TERRAIN_GRASS");
    wet.fw = Some(true);
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![wet, plot(5, 6, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 30,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 5,
        y: 5,
        pop: 3,
        loyalty: 100.0,
        ..StateCity::default()
    });
    let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let pos = crate::hex::offset_to_axial(5, 5);
    assert_eq!(mirror.game.observed_fresh_water.get(&pos), Some(&true));
    let city = mirror
        .game
        .cities
        .values()
        .find(|city| city.owner == 0)
        .expect("Roma is on the board");
    assert!(
        mirror.game.city_housing_sources(city).water >= 5.0,
        "the host says fresh water, so the centre houses 5 (no river or lake on the board)"
    );

    // A plot that carries no `fw` (an older export, a test fixture) overrides nothing.
    let dry = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
    }]);
    let plain = LiveMirror::new(&dry, &state, 4, 1, 500, 0);
    assert!(plain.game.observed_fresh_water.is_empty());
    let city = plain
        .game
        .cities
        .values()
        .find(|city| city.owner == 0)
        .expect("Roma is on the board");
    assert!(plain.game.city_housing_sources(city).water < 5.0);
}

/// The seat's own tourism per turn was the one culture-victory figure the
/// export never carried; the board now reads the host's and the model's
/// figure stays behind `tourism_per_turn_model` for the instrument.
#[test]
fn the_seats_own_tourism_per_turn_crosses_and_lapses() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 90,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 90,
        tourism_per_turn: Some(12.5),
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 5,
        y: 5,
        pop: 3,
        ..StateCity::default()
    });
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    assert_eq!(
        mirror.game.tourism_per_turn(0),
        12.5,
        "the host's figure is the board's"
    );
    assert!(
        mirror.game.tourism_per_turn_model(0) < 12.5,
        "the model has no great works to earn 12.5 from"
    );

    state.turn = 91;
    state.tourism_per_turn = None;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(
        mirror.game.tourism_per_turn(0),
        mirror.game.tourism_per_turn_model(0),
        "an export without the key leaves the model's figure"
    );
}

#[test]
fn sync_discards_units_that_only_civvis_simulated_from_production() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 4,
        width: 12,
        height: 12,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 4,
        ..StateSnapshot::default()
    };
    state.units.push(StateUnit {
        id: 42,
        kind: "UNIT_WARRIOR".to_string(),
        x: 5,
        y: 5,
        hp: 73.0,
        fortified: true,
        fortify_turns: 2,
        ..StateUnit::default()
    });

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let phantom = mirror
        .game
        .spawn_test_unit("archer", 0, crate::hex::offset_to_axial(5, 6));
    assert!(
        !mirror.civ6_of.contains_key(&phantom),
        "CIVVIS can simulate a queued production result before Firaxis creates it"
    );

    state.turn = 5;
    mirror.sync(&snapshot, &state, 0);

    assert!(
        !mirror.game.units.contains_key(&phantom),
        "the next live state must remove a locally simulated unit with no Civ VI id"
    );
    assert_eq!(
        mirror
            .game
            .units
            .values()
            .filter(|unit| unit.owner == 0)
            .count(),
        1,
        "only the exported warrior remains; otherwise CIVVIS plans with a phantom army"
    );
    let warrior = mirror
        .game
        .units
        .values()
        .find(|unit| unit.owner == 0)
        .unwrap();
    assert_eq!(warrior.hp, 73);
    assert!(
        warrior.fortified,
        "sync must not overwrite the observed fortification"
    );
    assert_eq!(warrior.fortify_turns, 2);
}

#[test]
fn live_units_keep_firaxis_charges_promotions_experience_and_religion() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 93,
        width: 12,
        height: 12,
        chunk: 1,
        plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(6, 5, "TERRAIN_GRASS")],
    }]);
    let state = StateSnapshot {
        turn: 93,
        units: vec![
            StateUnit {
                id: 91,
                kind: "UNIT_APOSTLE".to_string(),
                x: 5,
                y: 5,
                xp: Some(37),
                level: Some(2),
                promotions: Some(vec!["PROMOTION_TRANSLATOR".to_string()]),
                build_charges: Some(0),
                spread_charges: Some(2),
                religion: Some("RELIGION_CATHOLICISM".to_string()),
                ..StateUnit::default()
            },
            StateUnit {
                id: 92,
                kind: "UNIT_BUILDER".to_string(),
                x: 6,
                y: 5,
                build_charges: Some(0),
                spread_charges: Some(0),
                ..StateUnit::default()
            },
        ],
        ..StateSnapshot::default()
    };

    let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let apostle = mirror
        .game
        .units
        .values()
        .find(|unit| unit.owner == 0)
        .expect("the Apostle is mirrored");
    assert_eq!(apostle.xp, 37);
    assert_eq!(apostle.level, 2);
    assert_eq!(apostle.charges, 2);
    assert_eq!(apostle.religion.as_deref(), Some("Catholicism"));
    assert_eq!(
        apostle
            .promotions
            .iter()
            .map(|promotion| (*promotion).as_str())
            .collect::<Vec<_>>(),
        vec!["translator"]
    );
    let builder = mirror
        .game
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "builder")
        .expect("the zero-charge Builder is mirrored");
    assert_eq!(
        builder.charges, 0,
        "a host-reported zero must clear the builder's default charges"
    );
}

#[test]
fn firaxis_promotion_prefix_aliases_land_on_modelled_nodes() {
    assert_eq!(
        civvis_unit_promotion_name("PROMOTION_MONK_COBRA_STRIKE"),
        "cobra_strike"
    );
    assert_eq!(
        civvis_unit_promotion_name("PROMOTION_SUPER_CARRIER"),
        "supercarrier"
    );
    assert_eq!(
        civvis_unit_promotion_name("PROMOTION_SURF_ROCK"),
        "surf_band"
    );
    assert_eq!(
        civvis_unit_promotion_name("PROMOTION_SPY_ACE_DRIVER"),
        "ace_driver"
    );
    assert_eq!(
        civvis_unit_promotion_name("PROMOTION_SPY_GUERILLA_LEADER"),
        "guerrilla_leader"
    );
    // Every espionage promotion the bridge writes out has to come back in
    // under the name the ruleset actually holds, or an observed Spy loses
    // its promotions to `unmapped`.
    let rules = crate::rules::Rules::embedded();
    for promotion in crate::game::Game::SPY_PROMOTIONS {
        let host = if promotion == "guerrilla_leader" {
            "PROMOTION_SPY_GUERILLA_LEADER".to_string()
        } else {
            format!("PROMOTION_SPY_{}", promotion.to_ascii_uppercase())
        };
        let name = civvis_unit_promotion_name(&host);
        assert_eq!(name, promotion, "{host} does not round-trip");
        assert!(
            rules.promotions.contains_key(&name),
            "{name} is not in the ruleset"
        );
    }
}

#[test]
fn a_hostile_lands_on_the_barbarian_seat_and_not_on_dormant_free_cities() {
    // ⚠ The roster has TWO players carrying `is_barbarian`, and only one of them
    // is alive. Measured on run `civvis-20260731T172058Z`: all nine barbarians
    // were owned by seat 4, Free Cities, `alive = false`, while `barb_pid` was
    // seat 5. Nothing reported it — a planted unit never reaches `dropped_units`,
    // and the seat it landed on is barbarian by flag.
    let chunks = vec![TilesChunk {
        turn: 4,
        width: 8,
        height: 8,
        chunk: 1,
        plots: (0..8)
            .flat_map(|x| {
                (0..8).map(move |y| Plot {
                    x,
                    y,
                    im: None,
                    t: Some("TERRAIN_GRASS".to_string()),
                    f: None,
                    r: None,
                    o: -1,
                    w: false,
                    i: false,
                    fw: None,
                    rv: 0,
                    ri: false,
                    ct: None,
                    cl: -1,
                    p: false,
                    d: None,
                    dc: None,
                    wo: None,
                    rt: None,
                    rp: false,
                    yl: None,
                    ap: None,
                    np: false,
                    vis: false,
                })
            })
            .collect(),
    }];
    let snapshot = Snapshot::from_chunks(&chunks);
    let mut state = StateSnapshot {
        turn: 4,
        ..StateSnapshot::default()
    };
    state.hostiles.push(StateUnit {
        kind: "UNIT_WARRIOR".to_string(),
        x: 3,
        y: 3,
        ..StateUnit::default()
    });

    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    assert_eq!(
        recon.placed_rival_units, 1,
        "the hostile must reach the board"
    );

    let barb = recon
        .game
        .barb_pid
        .expect("a mirrored roster has a barbarian seat");
    let owner = recon
        .game
        .units
        .values()
        .find(|unit| unit.owner != 0)
        .map(|unit| unit.owner)
        .expect("the hostile must be on the board");
    assert_eq!(
        owner, barb,
        "a barbarian belongs to barb_pid, not to whichever seat carries the flag first"
    );
    assert!(
        recon.game.players[owner].alive,
        "and that seat must be alive — Free Cities is dormant until a revolt"
    );
    assert!(
        !recon.game.players[owner].is_free_city,
        "Free Cities is not the barbarian seat, however its flags read"
    );
}

/// ★★★★ A Free Cities unit is a Free Cities unit, with its wounds. `hostiles[]`
/// carries both players' units and every entry used to land on `barb_pid` at
/// full health (docs/FIDELITY.md, "The one-to-one map", item 3): the army that
/// took four cities on run civvis-20260802T064240Z was mirrored as barbarians.
#[test]
fn a_free_cities_hostile_lands_on_the_free_cities_seat_with_its_hp() {
    let snapshot = open_grass_board(8);
    let mut state = StateSnapshot {
        turn: 40,
        ..StateSnapshot::default()
    };
    // An older mod: the Free Cities actor is exported under `minors[]` (met,
    // nothing of it in view) and its units carry only its `player`.
    state.minors.push(StateMinor {
        player: 62,
        civ: "CIVILIZATION_FREE_CITIES".to_string(),
        at_war: true,
        ..StateMinor::default()
    });
    state.hostiles.push(StateUnit {
        id: 501,
        kind: "UNIT_WARRIOR".to_string(),
        x: 3,
        y: 3,
        player: 62,
        hp: 40.0,
        ..StateUnit::default()
    });
    state.hostiles.push(StateUnit {
        id: 502,
        kind: "UNIT_WARRIOR".to_string(),
        x: 5,
        y: 5,
        player: 63,
        hp: 100.0,
        ..StateUnit::default()
    });

    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    assert_eq!(recon.placed_rival_units, 2, "both hostiles reach the board");
    let barb = recon
        .game
        .barb_pid
        .expect("a mirrored roster has a barbarian seat");
    let free = recon
        .game
        .players
        .iter()
        .find(|player| player.is_free_city)
        .expect("a mirrored roster has a Free Cities seat");
    let at = |game: &crate::game::Game, x: i32, y: i32| -> crate::game::Unit {
        let pos = crate::hex::offset_to_axial(x, y);
        game.units
            .values()
            .find(|unit| unit.pos == pos)
            .cloned()
            .unwrap_or_else(|| panic!("a unit stands at {x},{y}"))
    };
    let taker = at(&recon.game, 3, 3);
    assert_eq!(taker.owner, free.id, "player 62 is the Free Cities seat");
    assert_eq!(taker.hp, 40, "and it crosses with its damage");
    assert!(free.alive, "a seat holding units is alive");
    assert!(recon.game.is_at_war(0, free.id));
    assert_eq!(
        at(&recon.game, 5, 5).owner,
        barb,
        "player 63 is still the barbarian seat"
    );
    assert_ne!(barb, free.id);

    // The persistent mirror routes the same way, and on a current mod the
    // unit's own `free` flag is enough — no `minors[]` entry needed.
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let free_host_uid = *mirror
        .foreign_uid_of
        .get(&501)
        .expect("the Free Cities host id is mapped");
    assert_eq!(
        mirror.game.host_unit_facts[&free_host_uid].civ6_id,
        Some(501),
        "the stable Civ 6 identity survives even when optional unit facts are absent"
    );
    state.minors.clear();
    state.hostiles[0].free = true;
    state.hostiles[0].hp = 25.0;
    state.hostiles[1].hp = 60.0;
    mirror.sync(&snapshot, &state, 0);
    let free_id = mirror
        .game
        .players
        .iter()
        .find(|player| player.is_free_city)
        .map(|player| player.id)
        .expect("the Free Cities seat survives a sync");
    let taker = at(&mirror.game, 3, 3);
    assert_eq!(taker.owner, free_id, "sync: `free` alone seats the unit");
    assert_eq!(taker.hp, 25, "sync: the fresh reading of its damage");
    assert!(
        mirror.game.players[free_id].alive,
        "sync: the seat stays alive"
    );
    let barbarian = at(&mirror.game, 5, 5);
    assert_eq!(
        barbarian.owner,
        mirror.game.barb_pid.expect("barb_pid"),
        "sync: the barbarian still lands on barb_pid"
    );
    assert_eq!(
        barbarian.hp, 60,
        "sync: a hostile the REBUILD planted is replaced by the fresh reading — \
             the tracked lists used to start empty and its construction hp stood"
    );

    // A unit the Free Cities actor's own `units[]` already carries is planted
    // once, from the actor's record, not a second time from `hostiles[]`.
    state.minors.push(StateMinor {
        player: 62,
        civ: "CIVILIZATION_FREE_CITIES".to_string(),
        units: vec![StateUnit {
            id: 501,
            kind: "UNIT_WARRIOR".to_string(),
            x: 3,
            y: 3,
            hp: 25.0,
            ..StateUnit::default()
        }],
        ..StateMinor::default()
    });
    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    assert_eq!(
        recon.placed_rival_units, 2,
        "the actor's copy and the barbarian"
    );
    assert!(
        recon
            .dropped_units
            .iter()
            .all(|note| !note.contains(":tile_taken")),
        "no duplicate is planted and dropped: {:?}",
        recon.dropped_units
    );
    assert_eq!(at(&recon.game, 3, 3).owner, free.id);
}

/// ★★★★ Amani's Envoys were counted twice. The host's `minors[].envoys` is
/// `GetTokensReceived`, which already carries an established Ambassador's +2
/// (run civvis-20260826T184456Z: La Venta 5 → 7 at t145 frame 1, the export
/// in which she first read `established: true` there, no envoy order to that
/// player), and `Game::envoys_at` added `city_state_envoys` again — board 9,
/// host 7, board Suzerain where the host reported a tie (`suzerain -1`,
/// `most_envoys 7`), `civ6_mirror_check.py`: `la venta envoys Civ6=7
/// CIVVIS=9; suzerain Civ6=-1 CIVVIS=0`.
#[test]
fn an_established_amani_is_not_added_to_the_hosts_envoy_count_again() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 150,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot(6, 6, "TERRAIN_PLAINS")],
    }]);
    let la_venta = |envoys: i64, suzerain: i32, most_envoys: i64| StateMinor {
        player: 9,
        civ: "CIVILIZATION_LA_VENTA".to_string(),
        envoys,
        suzerain,
        most_envoys,
        cities: vec![StateCity {
            id: 65_536,
            name: "La Venta".to_string(),
            x: 6,
            y: 6,
            pop: 5,
            capital: true,
            ..StateCity::default()
        }],
        ..StateMinor::default()
    };
    let amani = |established: bool| StateGovernor {
        kind: "GOVERNOR_THE_AMBASSADOR".to_string(),
        city: 65_536,
        city_player: 9,
        x: 6,
        y: 6,
        established,
        turns_on_site: if established { 5 } else { 2 },
        turns_to_establish: 5,
        promotions: vec!["GOVERNOR_PROMOTION_AMBASSADOR_MESSENGER".to_string()],
        ..StateGovernor::default()
    };
    let seat_of = |game: &crate::game::Game| -> usize {
        game.players
            .iter()
            .find(|player| player.is_minor && player.civ == "La Venta")
            .map(|player| player.id)
            .expect("La Venta minor seat")
    };

    // t150 of the run: host 7, no Suzerain, Amani established there.
    let state = StateSnapshot {
        turn: 150,
        minors: vec![la_venta(7, -1, 7)],
        governors: Some(vec![amani(true)]),
        ..StateSnapshot::default()
    };
    let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
    let minor = seat_of(&recon.game);
    assert_eq!(
        recon.game.amani_envoy_terms(0, minor),
        (2.0, 1.0),
        "the fixture establishes Amani in La Venta on the board"
    );
    assert_eq!(
        recon.game.envoys_at(0, minor),
        7,
        "the board answers the host's 7"
    );
    assert_eq!(
        recon.game.suzerain_of(minor),
        None,
        "the host's tie at 7 is a tie on the board too"
    );
    let mut mirror = LiveMirror::new(&snapshot, &state, 6, 1, 250, 0);
    mirror.sync(&snapshot, &state, 0);
    let minor = seat_of(&mirror.game);
    assert_eq!(mirror.game.envoys_at(0, minor), 7, "sync: 7");
    assert_eq!(mirror.game.suzerain_of(minor), None, "sync: no Suzerain");

    // The host names us Suzerain at 7: still 7, and ours.
    let state = StateSnapshot {
        turn: 150,
        minors: vec![la_venta(7, 0, 7)],
        governors: Some(vec![amani(true)]),
        ..StateSnapshot::default()
    };
    let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
    let minor = seat_of(&recon.game);
    assert_eq!(recon.game.envoys_at(0, minor), 7);
    assert_eq!(recon.game.suzerain_of(minor), Some(0));

    // Amani on site but not yet established (t144: host 5): 5, nothing subtracted.
    let state = StateSnapshot {
        turn: 150,
        minors: vec![la_venta(5, -1, 8)],
        governors: Some(vec![amani(false)]),
        ..StateSnapshot::default()
    };
    let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
    let minor = seat_of(&recon.game);
    assert_eq!(recon.game.amani_envoy_terms(0, minor), (0.0, 1.0));
    assert_eq!(recon.game.envoys_at(0, minor), 5);
    assert_eq!(recon.game.suzerain_of(minor), None);

    // No governor record at all: the host's number is stored as it is.
    let state = StateSnapshot {
        turn: 150,
        minors: vec![la_venta(7, 0, 7)],
        ..StateSnapshot::default()
    };
    let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
    let minor = seat_of(&recon.game);
    assert_eq!(recon.game.envoys_at(0, minor), 7);
    assert_eq!(recon.game.suzerain_of(minor), Some(0));
}

/// `ri` (`Plot:IsRiver`) is the one river bit the Lua says is not derivable
/// from `rv`: a segment whose Firaxis holder is an unrevealed neighbour reads
/// `rv = 0` while the plot is riverside. Exported since the river work of
/// 2026-08-01 and read by nothing until now.
#[test]
fn the_hosts_riverside_bit_marks_a_plot_whose_river_edge_is_unrevealed() {
    let mut wet = plot(4, 4, "TERRAIN_GRASS");
    wet.ri = true;
    let dry = plot(5, 4, "TERRAIN_GRASS");
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 3,
        width: 10,
        height: 10,
        chunk: 1,
        plots: vec![wet, dry],
    }]);
    let recon = rebuild_from_state(&snapshot, &StateSnapshot::default(), 4, 1, 500, 0);
    let wet = &recon.game.map.tiles[&crate::hex::offset_to_axial(4, 4)];
    assert!(wet.has_river(), "`ri` alone makes the plot riverside");
    assert!(
        wet.river_edges.iter().all(|edge| !*edge),
        "and invents no crossing on any edge"
    );
    let dry = &recon.game.map.tiles[&crate::hex::offset_to_axial(5, 4)];
    assert!(!dry.has_river(), "a plot without the bit stays dry");

    // The persistent mirror re-reads it every sync, and clears it when the
    // export stops saying so.
    let mut mirror = LiveMirror::new(&snapshot, &StateSnapshot::default(), 4, 1, 500, 0);
    assert!(mirror.game.map.tiles[&crate::hex::offset_to_axial(4, 4)].has_river());
    let mut plots = vec![plot(4, 4, "TERRAIN_GRASS"), plot(5, 4, "TERRAIN_GRASS")];
    plots[1].ri = true;
    let again = Snapshot::from_chunks(&[TilesChunk {
        turn: 4,
        width: 10,
        height: 10,
        chunk: 1,
        plots,
    }]);
    mirror.sync(&again, &StateSnapshot::default(), 0);
    assert!(!mirror.game.map.tiles[&crate::hex::offset_to_axial(4, 4)].has_river());
    assert!(mirror.game.map.tiles[&crate::hex::offset_to_axial(5, 4)].has_river());
}

/// `embarked` (`Unit:IsEmbarked`) crossed on every unit and the mirror kept
/// deriving embarkation from "its tile is water". The host's flag wins while
/// the unit stands where it was read; a unit the board moves, and an older
/// export with no flag, derive from the tile as before.
#[test]
fn the_hosts_embarked_flag_wins_over_the_tile_while_the_unit_stands_there() {
    let mut plots = (3..=9)
        .flat_map(|x| (3..=9).map(move |y| plot(x, y, "TERRAIN_GRASS")))
        .collect::<Vec<_>>();
    plots
        .iter_mut()
        .find(|site| site.x == 6 && site.y == 5)
        .expect("the unit's plot is in the fixture")
        .t = Some("TERRAIN_COAST".to_string());
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 20,
        width: 12,
        height: 12,
        chunk: 1,
        plots,
    }]);
    let mut state = StateSnapshot {
        turn: 20,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Canberra".to_string(),
        x: 5,
        y: 5,
        pop: 4,
        capital: true,
        ..StateCity::default()
    });
    state.units.push(StateUnit {
        id: 42,
        kind: "UNIT_WARRIOR".to_string(),
        x: 6,
        y: 5,
        moves: 0.0,
        embarked: Some(false),
        ..StateUnit::default()
    });

    let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let uid = *mirror.uid_of.get(&42).expect("the Warrior is mirrored");
    let unit = mirror.game.units[&uid].clone();
    assert!(
        mirror
            .game
            .rules
            .is_water(&mirror.game.map.tiles[&unit.pos]),
        "the fixture stands the unit on water"
    );
    assert!(
        !mirror.game.is_embarked(&unit),
        "the host said not embarked, and the host wins over the tile"
    );
    let moved = crate::game::Unit {
        pos: crate::hex::offset_to_axial(6, 6),
        ..unit.clone()
    };
    assert!(
        !mirror.game.is_embarked(&moved),
        "moved onto grass, the tile derives: not embarked"
    );
    let mut coast_again = unit.clone();
    coast_again.host_embarked = None;
    assert!(
        mirror.game.is_embarked(&coast_again),
        "with no host reading the tile derives: embarked on coast"
    );

    // The flag is re-read on every sync, and an older export (no flag) derives.
    let mut mirror = mirror;
    state.units[0].embarked = None;
    mirror.sync(&snapshot, &state, 0);
    let uid = *mirror
        .uid_of
        .get(&42)
        .expect("the Warrior survives the sync");
    assert!(
        mirror.game.is_embarked(&mirror.game.units[&uid]),
        "derived: embarked"
    );
    state.units[0].embarked = Some(true);
    mirror.sync(&snapshot, &state, 0);
    let uid = *mirror
        .uid_of
        .get(&42)
        .expect("the Warrior survives the sync");
    assert!(
        mirror.game.is_embarked(&mirror.game.units[&uid]),
        "the host agrees"
    );
}

/// Replays a recorded live run at one turn and prints seat 0's delegation at
/// every met city-state beside the host's, so the Amani correction can be read
/// off a real export rather than a fixture. Ignored by default; run it as
///
///     CIVVIS_REPLAY_EVENTS=<run>/events.jsonl CIVVIS_REPLAY_TURN=150 \
///         cargo test --lib -- --ignored --nocapture \
///         replay_city_state_envoys_against_the_host
///
/// Run civvis-20260826T184456Z at t150 before this change, per
/// `civ6_mirror_check.py`: `la venta envoys Civ6=7 CIVVIS=9; suzerain Civ6=-1
/// CIVVIS=0`. After: host 7, board 7, no Suzerain on either side.
#[test]
#[ignore]
fn replay_city_state_envoys_against_the_host() {
    let Ok(path) = std::env::var("CIVVIS_REPLAY_EVENTS") else {
        return;
    };
    let turn = std::env::var("CIVVIS_REPLAY_TURN")
        .ok()
        .and_then(|turn| turn.parse().ok());
    let events = std::path::Path::new(&path);
    let snapshot = snapshot_from_events_at(events, turn).expect("a snapshot at that turn");
    let state = state_from_events(events, turn).expect("a state at that turn");
    let players = if state.seat.players > 0 {
        state.seat.players
    } else {
        8
    };
    let max_turns = if state.seat.max_turns > 0 {
        state.seat.max_turns as u32
    } else {
        500
    };
    let mirror = LiveMirror::new(&snapshot, &state, players, 1, max_turns, 0);
    let mut mismatches = Vec::new();
    for (minor, seat) in minor_actor_assignments(&mirror.game, &state) {
        if !minor.is_city_state() {
            continue;
        }
        let host = minor.envoys.max(0);
        let board = mirror.game.envoys_at(0, seat);
        let board_suzerain = mirror.game.suzerain_of(seat);
        let amani = mirror.game.amani_envoy_terms(0, seat);
        println!(
            "t{} {:<30} envoys host={host} board={board} amani={amani:?} \
                 suzerain host={} board={board_suzerain:?}",
            state.turn, minor.civ, minor.suzerain
        );
        if board != host || (board_suzerain == Some(0)) != (minor.suzerain == 0) {
            mismatches.push(minor.civ.clone());
        }
    }
    assert!(
        mismatches.is_empty(),
        "the board disagrees with the host at {mismatches:?}"
    );
}

fn open_grass_board(side: i32) -> Snapshot {
    let chunks = vec![TilesChunk {
        turn: 4,
        width: side,
        height: side,
        chunk: 1,
        plots: (0..side)
            .flat_map(|x| {
                (0..side).map(move |y| Plot {
                    x,
                    y,
                    im: None,
                    t: Some("TERRAIN_GRASS".to_string()),
                    f: None,
                    r: None,
                    o: -1,
                    w: false,
                    i: false,
                    fw: None,
                    rv: 0,
                    ri: false,
                    ct: None,
                    cl: -1,
                    p: false,
                    d: None,
                    dc: None,
                    wo: None,
                    rt: None,
                    rp: false,
                    yl: None,
                    ap: None,
                    np: false,
                    vis: false,
                })
            })
            .collect(),
    }];
    Snapshot::from_chunks(&chunks)
}

#[test]
fn a_city_states_city_reaches_the_board_and_blocks_the_ring_civ6_refuses() {
    // ★★★★ The defect in one board: run civvis-20260801T224944Z was refused
    // founding six times, every one `can_start=false,no_reasons`, and every
    // early one 2-3 tiles from a city-state city the export never mentioned.
    // `can_found_city`'s four-tile floor was correct and blind — the city it
    // needed was structurally absent, because `rivals` is built from
    // `GetAliveMajorIDs`.
    let snapshot = open_grass_board(12);
    let mut state = StateSnapshot {
        turn: 4,
        ..StateSnapshot::default()
    };
    state.units.push(StateUnit {
        kind: "UNIT_SETTLER".to_string(),
        x: 4,
        y: 6,
        ..StateUnit::default()
    });
    state.minors.push(StateMinor {
        player: 7,
        civ: "CIVILIZATION_KABUL".to_string(),
        cities: vec![StateCity {
            id: 5,
            name: "Kabul".to_string(),
            x: 6,
            y: 6,
            ..StateCity::default()
        }],
        ..StateMinor::default()
    });

    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    assert_eq!(
        recon.placed_minor_cities, 1,
        "the city-state's city must be planted"
    );
    let minor_city = recon
        .game
        .cities
        .values()
        .find(|city| city.owner != 0)
        .expect("the minor's city must be on the board");
    let seat = minor_city.owner;
    assert!(
        recon.game.players[seat].is_minor,
        "a city-state seats as a minor"
    );
    assert!(
        seat >= 4,
        "a minor must never take a 1..n seat — those indices are the \
             DeclareWar-to-Civ-6-id mapping and a minor in the middle would aim a \
             declaration at the wrong civilization"
    );
    let (uid, _) = recon
        .game
        .units
        .iter()
        .find(|(_, unit)| unit.owner == 0)
        .expect("our settler must be on the board");
    assert!(
        !recon.game.can_found_city(*uid),
        "two tiles from Kabul the four-tile floor must refuse — before this \
             fix the city was invisible and CIVVIS aimed here every time"
    );
}

#[test]
fn an_unplanted_known_city_still_blocks_its_settlement_ring() {
    // Firaxis keeps a met city-state in the state roster even when its
    // centre has not arrived in the terrain feed. Its nearby revealed
    // tiles must still inherit the four-tile founding floor.
    let centre = (6, 6);
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 4,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| plot(x, y, "TERRAIN_GRASS")))
            .filter(|plot| (plot.x, plot.y) != centre)
            .collect(),
    }]);
    let settler_offset = (4, 6);
    let settler_pos = crate::hex::offset_to_axial(settler_offset.0, settler_offset.1);
    let mut state = StateSnapshot {
        turn: 4,
        ..StateSnapshot::default()
    };
    state.units.push(StateUnit {
        id: 17,
        kind: "UNIT_SETTLER".to_string(),
        x: settler_offset.0,
        y: settler_offset.1,
        ..StateUnit::default()
    });

    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let settler = mirror.uid_of[&17];
    assert!(
        mirror.game.can_found_city(settler),
        "without a reported city the fixture must be legal"
    );

    state.turn = 5;
    state.minors.push(StateMinor {
        player: 7,
        civ: "CIVILIZATION_KABUL".to_string(),
        cities: vec![StateCity {
            id: 5,
            name: "Kabul".to_string(),
            x: centre.0,
            y: centre.1,
            ..StateCity::default()
        }],
        ..StateMinor::default()
    });
    mirror.sync(&snapshot, &state, 0);

    assert!(
        mirror
            .game
            .city_at(crate::hex::offset_to_axial(centre.0, centre.1))
            .is_none(),
        "the fixture deliberately omits Kabul's terrain centre"
    );
    assert!(mirror.game.blocked_city_sites.contains(&settler_pos));
    assert!(
        !mirror.game.can_found_city(settler),
        "a persistent mirror must reject the host-illegal nearby site"
    );

    let fresh = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    let fresh_settler = fresh
        .unit_ids
        .iter()
        .find_map(|(uid, civ6)| (*civ6 == 17).then_some(*uid))
        .expect("the fresh board must retain the settler");
    assert!(fresh
        .game
        .city_at(crate::hex::offset_to_axial(centre.0, centre.1))
        .is_none());
    assert!(fresh.game.blocked_city_sites.contains(&settler_pos));
    assert!(
        !fresh.game.can_found_city(fresh_settler),
        "a fresh-board decision must receive the same prohibition"
    );

    let legal = crate::hex::offset_to_axial(2, 6);
    let control = mirror.game.spawn_test_unit("settler", 0, legal);
    assert!(!mirror.game.blocked_city_sites.contains(&legal));
    assert!(
        mirror.game.can_found_city(control),
        "exactly four tiles from Kabul remains a legal city site"
    );
}

/// ★★★★ WHAT STANDS ON A RIVAL'S GROUND CROSSES WITH THE PLOTS.
///
/// A rival city record carries no districts, so a rival's economy and
/// defence were modelled from population alone. The tiles export now names
/// the district (`d`) and wonder (`wo`) on any revealed plot; the mirror
/// puts them on the owning rival city and rebuilds them from every export,
/// so a razed district does not linger.
#[test]
fn a_rivals_districts_and_wonders_cross_with_the_plots() {
    let side = 20;
    let mut plots: Vec<Plot> = (0..side)
        .flat_map(|x| {
            (0..side).map(move |y| Plot {
                x,
                y,
                im: None,
                t: Some("TERRAIN_GRASS".to_string()),
                f: None,
                r: None,
                o: -1,
                w: false,
                i: false,
                fw: None,
                rv: 0,
                ri: false,
                ct: None,
                cl: -1,
                p: false,
                d: None,
                dc: None,
                wo: None,
                rt: None,
                rp: false,
                yl: None,
                ap: None,
                np: false,
                vis: false,
            })
        })
        .collect();
    // The rival (Civ 6 player 3) owns a centre at (10,10), a Campus at
    // (11,10) and the Pyramids at (10,11); we sit far away at (2,2).
    for plot in plots.iter_mut() {
        match (plot.x, plot.y) {
            (10, 10) => {
                plot.o = 3;
                plot.d = Some("DISTRICT_CITY_CENTER".to_string());
            }
            (11, 10) => {
                plot.o = 3;
                plot.d = Some("DISTRICT_CAMPUS".to_string());
                plot.dc = Some(true);
            }
            (10, 11) => {
                plot.o = 3;
                plot.d = Some("DISTRICT_WONDER".to_string());
                plot.wo = Some("BUILDING_PYRAMIDS".to_string());
            }
            // A PLACED Encampment: `GetDistrictType` names it, `IsComplete`
            // says no. It is not on the board until it is built.
            (9, 10) => {
                plot.o = 3;
                plot.d = Some("DISTRICT_ENCAMPMENT".to_string());
                plot.dc = Some(false);
            }
            // An older export says nothing about completion: planted.
            (10, 9) => {
                plot.o = 3;
                plot.d = Some("DISTRICT_HOLY_SITE".to_string());
            }
            (11, 11) => {
                plot.o = 3;
            }
            (2, 2) => {
                plot.o = 0;
            }
            _ => {}
        }
    }
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 60,
        width: side,
        height: side,
        chunk: 1,
        plots,
    }]);
    let city = |id, name: &str, x, y| StateCity {
        id,
        name: name.to_string(),
        x,
        y,
        pop: 5,
        loyalty: 100.0,
        ..StateCity::default()
    };
    let mut state = StateSnapshot {
        turn: 60,
        ..StateSnapshot::default()
    };
    let mut rome = city(1, "Rome", 2, 2);
    rome.capital = true;
    state.cities.push(rome);
    state.rivals.push(StateRival {
        player: 3,
        civ: "CIVILIZATION_SCOTLAND".to_string(),
        cities: vec![city(3, "Stirling", 10, 10)],
        ..StateRival::default()
    });
    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    let stirling = recon.known_city_ids[&3];
    let campus = crate::hex::offset_to_axial(11, 10);
    let pyramids = crate::hex::offset_to_axial(10, 11);
    assert_eq!(
        recon.game.cities[&stirling]
            .districts
            .get(crate::name!("campus")),
        Some(&campus)
    );
    assert_eq!(
        recon.game.map.tiles[&campus].district.as_deref(),
        Some("campus")
    );
    assert_eq!(
        recon.game.cities[&stirling]
            .wonders
            .get(&crate::name!("pyramids")),
        Some(&pyramids)
    );
    assert_eq!(
        recon.game.map.tiles[&pyramids].wonder.as_deref(),
        Some("pyramids")
    );
    let encampment = crate::hex::offset_to_axial(9, 10);
    assert!(
        recon.game.cities[&stirling]
            .districts
            .get(crate::name!("encampment"))
            .is_none(),
        "a placed, unbuilt district is not on the board"
    );
    assert!(recon.game.map.tiles[&encampment].district.is_none());
    let holy_site = crate::hex::offset_to_axial(10, 9);
    assert_eq!(
        recon.game.cities[&stirling]
            .districts
            .get(crate::name!("holy_site")),
        Some(&holy_site),
        "an export without the flag is read as it always was"
    );
    // Our own city takes nothing from this path.
    let rome_id = recon.game.player_city_ids(0)[0];
    assert!(recon.game.cities[&rome_id].districts.is_empty());
}

#[test]
fn a_settler_does_not_found_a_city_that_population_pressure_will_erase() {
    // Geometry reproduces the live failure at a smaller offset: the doomed
    // site is eight tiles from our population-six city and six from the rival's,
    // while the control site is four from us and twelve from them.
    let snapshot = open_grass_board(40);
    let city = |id, name: &str, x, y| StateCity {
        id,
        name: name.to_string(),
        x,
        y,
        pop: 6,
        ..StateCity::default()
    };
    let mut state = StateSnapshot {
        turn: 45,
        ..StateSnapshot::default()
    };
    let mut rome = city(1, "Rome", 30, 2);
    rome.pop = 9;
    rome.capital = true;
    state.cities.extend([rome, city(2, "Ostia", 18, 10)]);
    state.rivals.push(StateRival {
        player: 3,
        civ: "CIVILIZATION_SCOTLAND".to_string(),
        cities: vec![city(3, "Stirling", 14, 13)],
        ..StateRival::default()
    });
    state.units.extend([
        StateUnit {
            id: 10,
            kind: "UNIT_SETTLER".to_string(),
            x: 10,
            y: 10,
            ..StateUnit::default()
        },
        StateUnit {
            id: 11,
            kind: "UNIT_SETTLER".to_string(),
            x: 16,
            y: 6,
            ..StateUnit::default()
        },
    ]);

    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    let doomed = crate::hex::offset_to_axial(10, 10);
    let supported = crate::hex::offset_to_axial(16, 6);
    let supported_settler = recon
        .unit_ids
        .iter()
        .find_map(|(unit, civ6)| (*civ6 == 11).then_some(*unit))
        .expect("the supported Settler must cross the mirror");
    let doomed_settler = recon
        .unit_ids
        .iter()
        .find_map(|(unit, civ6)| (*civ6 == 10).then_some(*unit))
        .expect("the doomed Settler must cross the mirror");
    let stirling = recon.known_city_ids[&3];
    assert_eq!(recon.placed_rival_cities, 1);
    assert_eq!(
        recon.game.wdist(doomed, recon.game.cities[&stirling].pos),
        6
    );
    let stirling_owner = recon.game.cities[&stirling].owner;
    assert_ne!(stirling_owner, 0);
    assert!(!recon.game.players[stirling_owner].is_minor);
    assert!(!recon.game.players[stirling_owner].is_barbarian);
    assert_eq!(recon.game.cities[&stirling].pop, 6);
    assert!(!recon.game.same_team(0, stirling_owner));
    assert!(
        recon
            .game
            .wdist(doomed, recon.game.cities[&recon.known_city_ids[&1]].pos)
            > 9
    );
    let mut forecast = recon.game.clone();
    Arc::make_mut(&mut forecast.blocked_city_sites).remove(&doomed);
    assert!(forecast.can_found_city(doomed_settler));
    let forecast_city = forecast.found_city_for(0, doomed, None);
    let forecast_loyalty = forecast.city_loyalty_per_turn(&forecast.cities[&forecast_city]);
    assert_eq!(
        recon
            .game
            .wdist(doomed, recon.game.cities[&recon.known_city_ids[&2]].pos),
        8
    );
    assert!(
        recon.game.blocked_city_sites.contains(&doomed),
        "a city forecast at {forecast_loyalty:+.1} Loyalty/turn with stronger visible \
             foreign pressure must not consume the Settler"
    );
    assert!(
        !recon.game.blocked_city_sites.contains(&supported),
        "the filter must preserve a nearby domestically supported alternative"
    );
    assert!(
        recon.game.can_found_city(supported_settler),
        "the safe control site must remain immediately settleable"
    );
}

/// ⚠ The export carries a rival's units ONLY under current visibility, so a
/// unit arriving here is one the HOST has already let the seat see — its own
/// detection rules included. Re-deriving Naval Raider stealth on the mirror
/// vetoed that ground truth: run `civvis-20260807T162004Z`, turns 237–251,
/// `UNITDATA ⚠ UNIT_NUCLEAR_SUBMARINE@(4, 36) count Civ6=1 CIVVIS=0` — the
/// sub was planted, then hidden from the seat's board, orders and threat
/// reads because no destroyer of ours stood beside it (#1362).
#[test]
fn a_visible_rival_naval_raider_is_not_hidden_by_our_own_stealth_rule() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 240,
        width: 30,
        height: 30,
        chunk: 1,
        plots: vec![plot(20, 9, "TERRAIN_COAST"), plot(5, 5, "TERRAIN_GRASS")],
    }]);
    let mut state = StateSnapshot {
        turn: 240,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Rome".to_string(),
        x: 5,
        y: 5,
        pop: 6,
        capital: true,
        ..StateCity::default()
    });
    state.rivals.push(StateRival {
        player: 5,
        civ: "CIVILIZATION_AMERICA".to_string(),
        // The exact unit shape from the live export under `rivals[4]`.
        units: vec![serde_json::from_str(
            r#"{"build_charges": 0, "class": "PROMOTION_CLASS_NAVAL_RAIDER",
                    "combat": 80, "fortified": false, "fortify_turns": 0, "hp": 100,
                    "kind": "UNIT_NUCLEAR_SUBMARINE", "level": 1, "moves": 0,
                    "promotions": [], "ranged": 85, "spread_charges": 0,
                    "x": 20, "y": 9, "xp": 0}"#,
        )
        .expect("the issue's unit shape deserializes")],
        ..StateRival::default()
    });

    let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let sub = recon
        .game
        .units
        .values()
        .find(|unit| unit.kind == "nuclear_submarine")
        .expect("the exported submarine must be planted, not dropped");
    assert_ne!(sub.owner, 0, "it is the rival's unit");
    assert!(
        recon.game.unit_visible_to(sub.id, 0),
        "the host proved the seat can see this raider; the mirror's own \
             stealth model must not veto it"
    );
    // End to end: the seat's fogged board dump — what the planner and the
    // mirror checker read — must carry the unit.
    let view = crate::obs::observation_player_view(&recon.game, 0);
    assert!(
        view["units"]
            .as_array()
            .expect("units array")
            .iter()
            .any(|unit| unit["type"] == "nuclear_submarine"),
        "the raider must appear on the seat's board"
    );
}

#[test]
fn a_seated_but_cityless_minors_ground_is_still_blocked() {
    // Borders are visible before centres are: a city-state's territory can
    // arrive while its city is still under fog. A seat we can NAME but that
    // holds no city must not read as free land — that is the same hole the
    // unattributable-owner arm closes for minors we cannot name.
    let mut chunks = vec![TilesChunk {
        turn: 4,
        width: 8,
        height: 8,
        chunk: 1,
        plots: (0..8)
            .flat_map(|x| {
                (0..8).map(move |y| Plot {
                    x,
                    y,
                    im: None,
                    t: Some("TERRAIN_GRASS".to_string()),
                    f: None,
                    r: None,
                    o: if (5..=6).contains(&x) && (5..=6).contains(&y) {
                        7
                    } else {
                        -1
                    },
                    w: false,
                    i: false,
                    fw: None,
                    rv: 0,
                    ri: false,
                    ct: None,
                    cl: -1,
                    p: false,
                    d: None,
                    dc: None,
                    wo: None,
                    rt: None,
                    rp: false,
                    yl: None,
                    ap: None,
                    np: false,
                    vis: false,
                })
            })
            .collect(),
    }];
    let snapshot = Snapshot::from_chunks(&std::mem::take(&mut chunks));
    let mut state = StateSnapshot {
        turn: 4,
        ..StateSnapshot::default()
    };
    state.minors.push(StateMinor {
        player: 7,
        civ: "CIVILIZATION_KABUL".to_string(),
        ..StateMinor::default()
    });

    let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    assert_eq!(recon.placed_minor_cities, 0, "no city was visible to plant");
    let pos = crate::hex::offset_to_axial(5, 5);
    assert!(
        recon.game.blocked_city_sites.contains(&pos),
        "ground a named-but-cityless minor owns must stay unfoundable"
    );
}

#[test]
fn persistent_sync_keeps_a_scythian_horse_archer_on_the_board() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 4,
        width: 8,
        height: 8,
        chunk: 1,
        plots: (0..8)
            .flat_map(|x| (0..8).map(move |y| plot(x, y, "TERRAIN_GRASS")))
            .collect(),
    }]);
    let mut state = StateSnapshot {
        turn: 4,
        ..StateSnapshot::default()
    };
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);

    state.turn = 5;
    state.hostiles.push(StateUnit {
        kind: "UNIT_SCYTHIAN_HORSE_ARCHER".to_string(),
        x: 3,
        y: 3,
        ..StateUnit::default()
    });
    mirror.sync(&snapshot, &state, 0);

    let barb = mirror
        .game
        .barb_pid
        .expect("the mirrored roster has barbarians");
    assert!(mirror
        .game
        .units
        .values()
        .any(|unit| { unit.owner == barb && unit.kind == "saka_horse_archer" }));
    assert!(
        !mirror
            .unmapped
            .contains(&"UNIT_SCYTHIAN_HORSE_ARCHER".to_string()),
        "a real Firaxis unit must not disappear after persistent sync"
    );
}

#[test]
fn a_unique_great_person_is_a_modelling_gap_not_a_bridge_defect() {
    // Gran Colombia's Great General keeps its own name, so the `UNIT_GREAT_*`
    // prefix does not catch it, and it was being reported as `untranslatable` —
    // which reads as "add a vocabulary entry" when there is no entry to add.
    assert!(
        is_great_person("UNIT_COMANDANTE_GENERAL"),
        "a civilization's unique Great Person is still a Great Person"
    );
    assert!(
        is_great_person("UNIT_GREAT_GENERAL"),
        "and the prefix still works"
    );
    assert!(
        !is_great_person("UNIT_AZTEC_EAGLE_WARRIOR"),
        "a genuinely untranslatable unit must stay a bridge defect"
    );
}

/// The ordered host-state step list, per mode, exactly as
/// `HOST_STATE_STEPS` declares it.
///
/// ⚠⚠ A STEP THAT LANDS ON ONE PASS ONLY IS THE BUG THE TABLE EXISTS TO
/// PREVENT. `rebuild_from_state` and `Mirror::sync` used to carry a private
/// copy of this each — ~1,000 lines apiece, 26 and 25 `apply_*` helpers —
/// and a one-to-one map of a new Civilization VI reading written into one
/// body and not the other desynced a live seat with nothing to see. If this
/// test fails the list moved: say so here, per mode, or put the step back.
#[test]
fn host_state_step_list_is_the_recorded_order() {
    use super::{host_step_names, HostPhase, MirrorMode};
    let rebuild = |phase| host_step_names(MirrorMode::Rebuild, phase);
    let sync = |phase| host_step_names(MirrorMode::Sync, phase);

    assert_eq!(
        rebuild(HostPhase::Empire),
        [
            "game_speed",
            "seat_victories",
            "difficulty",
            "human_seat",
            "map_script",
            "refused_site_blocks",
            "identity",
        ]
    );
    assert_eq!(sync(HostPhase::Empire), ["identity"]);

    // ⚠ `host_gold` sits either side of `host_maintenance` depending on the
    // pass. Both orders are what shipped, and neither helper reads what the
    // other writes.
    assert_eq!(
        rebuild(HostPhase::Economy),
        [
            "turn_and_score",
            "max_turns",
            "host_gold",
            "host_maintenance",
            "faith_and_dvp",
            "congress_dvp",
            "host_competitions",
            "diplomatic_favor",
            "mirrored_envoys_free",
            "player_religion",
        ]
    );
    assert_eq!(
        sync(HostPhase::Economy),
        [
            "turn_and_score",
            "host_maintenance",
            "host_gold",
            "faith_and_dvp",
            "congress_dvp",
            "host_competitions",
            "diplomatic_favor",
            "mirrored_envoys_free",
            "player_religion",
        ]
    );

    assert_eq!(rebuild(HostPhase::Refresh), [] as [&str; 0]);
    assert_eq!(
        sync(HostPhase::Refresh),
        ["terrain", "territory", "city_memory"]
    );

    // ⚠ `trade_routes` likewise: before the terrain passes on the sync,
    // after `city_memory` on the rebuild.
    assert_eq!(
        rebuild(HostPhase::Board),
        [
            "terrain",
            "territory",
            "tile_memory",
            "city_memory",
            "trade_routes",
            "governor_state",
            "host_envoys",
            "great_person_points",
            "strategic_stockpiles",
            "player_ages",
            "host_congress",
            "host_climate",
            "observed_host_metrics",
            "loyalty_doomed_sites",
        ]
    );
    assert_eq!(
        sync(HostPhase::Board),
        [
            "trade_routes",
            "terrain",
            "territory",
            "tile_memory",
            "city_memory",
            "governor_state",
            "host_envoys",
            "great_person_points",
            "strategic_stockpiles",
            "player_ages",
            "host_congress",
            "host_climate",
            "observed_host_metrics",
            "loyalty_doomed_sites",
        ]
    );

    let finish = ["player_ages", "record_host_observed"];
    assert_eq!(rebuild(HostPhase::Finish), finish);
    assert_eq!(sync(HostPhase::Finish), finish);
}

/// The whole divergence between the two passes, named in one place.
///
/// The only `apply_*` helper one pass calls and the other does not is
/// `apply_seat_victories`, here as `seat_victories`: the host's enabled
/// victory conditions cannot change mid-game, so a sync never re-reads them.
/// Everything else on these two lists is board setup the rebuild does once
/// (`game_speed`, `difficulty`, `human_seat`, `map_script`, `max_turns`, the
/// refusal ledgers) or the sync-only mid-pass over ground revealed this turn.
#[test]
fn the_two_host_state_passes_differ_only_where_recorded() {
    use super::{host_step_names, HostPhase, MirrorMode};
    let mut rebuild_only = Vec::new();
    let mut sync_only = Vec::new();
    for phase in [
        HostPhase::Empire,
        HostPhase::Economy,
        HostPhase::Refresh,
        HostPhase::Board,
        HostPhase::Finish,
    ] {
        let on_rebuild = host_step_names(MirrorMode::Rebuild, phase);
        let on_sync = host_step_names(MirrorMode::Sync, phase);
        for name in &on_rebuild {
            if !on_sync.contains(name) {
                rebuild_only.push(format!("{phase:?}/{name}"));
            }
        }
        for name in &on_sync {
            if !on_rebuild.contains(name) {
                sync_only.push(format!("{phase:?}/{name}"));
            }
        }
    }
    assert_eq!(
        rebuild_only,
        [
            "Empire/game_speed",
            "Empire/seat_victories",
            "Empire/difficulty",
            "Empire/human_seat",
            "Empire/map_script",
            "Empire/refused_site_blocks",
            "Economy/max_turns",
        ]
    );
    assert_eq!(
        sync_only,
        [
            "Refresh/terrain",
            "Refresh/territory",
            "Refresh/city_memory"
        ]
    );
}

/// Neither pass may apply a host-state reading of its own at the top level.
///
/// ⚠⚠ THIS IS THE GUARD, not the taste. The two bodies are still ~900-line
/// twins around the step calls, and a new `apply_*` dropped into one of them
/// by hand is exactly the edit that used to ship half-done. Read out of the
/// source so it cannot be satisfied by editing the table alone: a top-level
/// `apply_*` in either body fails here until it is a step in
/// `HOST_STATE_STEPS` with a mode mask on it.
#[test]
fn both_host_state_passes_walk_only_the_step_table() {
    const SOURCE: &str = include_str!("../mirror.rs");
    let walk = ["Empire", "Economy", "Refresh", "Board", "Finish"];
    assert_eq!(
        top_level_host_state_effects(SOURCE, "pub fn rebuild_from_state(", 0),
        walk,
        "rebuild_from_state applies something outside `HOST_STATE_STEPS`"
    );
    assert_eq!(
        top_level_host_state_effects(SOURCE, "    pub fn sync(&mut self, snapshot:", 4),
        walk,
        "Mirror::sync applies something outside `HOST_STATE_STEPS`"
    );
}

/// The phases a function body drives, and any `apply_*` it calls as a
/// statement of its own, in source order.
///
/// Nested calls are left out on purpose: the per-city, per-unit, per-rival
/// and per-minor loops apply readings to ONE entity at a time and are not
/// whole-board steps. `rustfmt` guarantees the closing brace sits at the
/// signature's own indent, which is how the body is bounded.
fn top_level_host_state_effects(source: &str, signature: &str, indent: usize) -> Vec<String> {
    let lines: Vec<&str> = source.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.starts_with(signature))
        .unwrap_or_else(|| panic!("no function starting `{signature}` in mirror.rs"));
    let closing = format!("{}}}", " ".repeat(indent));
    let end = start
        + 1
        + lines[start + 1..]
            .iter()
            .position(|line| *line == closing)
            .unwrap_or_else(|| panic!("`{signature}` never closes in mirror.rs"));
    let statement = " ".repeat(indent + 4);
    let mut effects = Vec::new();
    for line in &lines[start..end] {
        if line.trim_start().starts_with("//") {
            continue;
        }
        if let Some(rest) = line.split("HostPhase::").nth(1) {
            effects.push(rest.trim_end_matches([',', ')', ';']).to_string());
        } else if line.starts_with(&statement) && !line[indent + 4..].starts_with(' ') {
            for word in line.split(|c: char| !c.is_alphanumeric() && c != '_') {
                if word.starts_with("apply_") {
                    effects.push(word.to_string());
                }
            }
        }
    }
    effects
}

/// ★★★★★ A CITY-STATE'S CIVILIAN SHARING A PLOT WAS SILENTLY DROPPED.
///
/// `sync` plants hostiles first (behind their own collision guard), rivals
/// second with NO guard — so a major's Trader stacks on a barbarian and the
/// board sees it — and minors third behind `&& !self.game.units.values()
/// .any(|live| live.pos == pos)`. A city-state's unit sharing a plot
/// therefore never reached the board at all: not in `dropped_units`, not in
/// `unmapped`, simply absent.
///
/// A unit the mirror cannot see is a unit no veto can refuse to shoot.
/// Measured 2026-08-29: `civvis-20260827T145140Z` t52 struck the plot
/// Bologna's TRADER stood on and `civvis-20260829T022207Z` t66 the plot
/// Kumasi's did — both a surprise war on the host, neither visible to
/// `Game::peaceful_foreign_unit_at`. `rebuild_from_state` has always planted
/// minors through the same `plant_unit` as rivals; `sync` was the odd one out.
#[test]
fn a_minors_unit_is_planted_on_a_plot_a_barbarian_already_holds() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![
            plot(5, 5, "TERRAIN_GRASS"),
            plot(6, 6, "TERRAIN_PLAINS"),
            plot(7, 6, "TERRAIN_PLAINS"),
        ],
    }]);
    let mut state = StateSnapshot {
        turn: 30,
        minors: vec![StateMinor {
            player: 6,
            civ: "CIVILIZATION_KABUL".to_string(),
            suzerain: -1,
            cities: vec![StateCity {
                id: 70,
                name: "Kabul".to_string(),
                x: 6,
                y: 6,
                pop: 4,
                ..StateCity::default()
            }],
            units: vec![StateUnit {
                id: 71,
                kind: "UNIT_TRADER".to_string(),
                x: 7,
                y: 6,
                hp: 100.0,
                ..StateUnit::default()
            }],
            ..StateMinor::default()
        }],
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 1,
        name: "Roma".to_string(),
        x: 5,
        y: 5,
        pop: 3,
        ..StateCity::default()
    });
    state.hostiles.push(StateUnit {
        id: 90,
        kind: "UNIT_WARRIOR".to_string(),
        player: 63,
        x: 7,
        y: 6,
        hp: 100.0,
        ..StateUnit::default()
    });

    let mut mirror = LiveMirror::new(&snapshot, &state, 6, 1, 250, 0);
    // The sync path is the one that runs every live turn, and the one that
    // used to drop the Trader.
    mirror.sync(&snapshot, &state, 0);

    let pos = crate::hex::offset_to_axial(7, 6);
    let trader = *mirror
        .foreign_uid_of
        .get(&71)
        .expect("the minor's Trader reaches the board even though a barbarian holds the plot");
    assert_eq!(mirror.game.units[&trader].pos, pos);
    assert!(
        mirror
            .game
            .units
            .values()
            .any(|unit| unit.pos == pos && Some(unit.owner) == mirror.game.barb_pid),
        "the barbarian is still there too"
    );
    assert!(
        !mirror.game.is_at_war(0, mirror.game.units[&trader].owner),
        "and we are at peace with its owner"
    );
    assert!(
        mirror.game.peaceful_foreign_unit_at(0, pos),
        "so the strike veto can see it"
    );
}

/// docs/FIDELITY.md, "an enemy's attacks left, formation tier and embarked
/// flag cross" (2026-09-01). `hostiles[]` never carried `formation`, so an
/// enemy Corps or Army stood on the board as a plain unit — 10 or 17 CS short
/// of the figure its flag shows. The tier reaches a planted hostile through
/// the same `apply_unit_observation` the seat's own units use, and
/// `Game::unit_strength` prices it there, attacking and defending alike.
#[test]
fn a_hostile_exported_as_an_army_is_priced_seventeen_above_a_plain_one() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 8,
        width: 8,
        height: 8,
        chunk: 1,
        plots: (0..8)
            .flat_map(|x| (0..8).map(move |y| plot(x, y, "TERRAIN_GRASS")))
            .collect(),
    }]);
    let priced = |formation: Option<i32>| {
        let state = StateSnapshot {
            turn: 8,
            hostiles: vec![StateUnit {
                id: 131072,
                kind: "UNIT_WARRIOR".to_string(),
                x: 3,
                y: 3,
                hp: 100.0,
                formation,
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        };
        let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let uid = mirror.foreign_uid_of[&131072];
        let unit = &mirror.game.units[&uid];
        assert_eq!(
            Some(unit.owner),
            mirror.game.barb_pid,
            "a hostile lands on the barbarian seat"
        );
        (
            unit.formation,
            mirror.game.unit_strength(unit, false),
            mirror.game.unit_strength(unit, true),
        )
    };
    let (plain_tier, plain_attack, plain_defence) = priced(Some(0));
    let (army_tier, army_attack, army_defence) = priced(Some(2));
    assert_eq!((plain_tier, army_tier), (0, 2));
    assert_eq!(
        army_attack - plain_attack,
        17.0,
        "an Army attacks at +17 CS"
    );
    assert_eq!(army_defence - plain_defence, 17.0, "and defends at +17 CS");
    let (corps_tier, corps_attack, _) = priced(Some(1));
    assert_eq!(
        (corps_tier, corps_attack - plain_attack),
        (1, 10.0),
        "a Corps at +10 CS"
    );
    let (absent_tier, absent_attack, _) = priced(None);
    assert_eq!(
        (absent_tier, absent_attack),
        (0, plain_attack),
        "an older export without the key: the board's own tier, priced as before"
    );
    let (sentinel_tier, sentinel_attack, _) = priced(Some(-1));
    assert_eq!(
        (sentinel_tier, sentinel_attack),
        (0, plain_attack),
        "the mod's -1 is unknown — not an Army, not a claim"
    );
}

/// The same note: `attacks_remaining` (`GetAttacksRemaining`, the shipped
/// SelectedUnit read) now crosses for a foreign unit and
/// `apply_foreign_unit_strikes` sets the planted unit's `attacks_left` on
/// every foreign planting site — hostiles and rivals, rebuild and sync. An
/// enemy that had struck this turn used to read as one that could still
/// strike. Absent, the fresh-turn allowance stands exactly as before.
#[test]
fn a_hostiles_attacks_remaining_reaches_the_planted_unit_on_both_paths() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 8,
        width: 8,
        height: 8,
        chunk: 1,
        plots: (0..8)
            .flat_map(|x| (0..8).map(move |y| plot(x, y, "TERRAIN_GRASS")))
            .collect(),
    }]);
    let state = |turn: u32, attacks: Option<i32>| StateSnapshot {
        turn,
        hostiles: vec![StateUnit {
            id: 131072,
            kind: "UNIT_WARRIOR".to_string(),
            x: 3,
            y: 3,
            hp: 100.0,
            attacks_remaining: attacks,
            ..StateUnit::default()
        }],
        rivals: vec![StateRival {
            player: 1,
            units: vec![StateUnit {
                id: 65537,
                kind: "UNIT_ARCHER".to_string(),
                x: 5,
                y: 5,
                hp: 100.0,
                attacks_remaining: attacks,
                ..StateUnit::default()
            }],
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    let attacks_of = |mirror: &LiveMirror, id: i64| {
        let uid = *mirror
            .foreign_uid_of
            .get(&id)
            .expect("the foreign unit is on the board");
        mirror.game.units[&uid].attacks_left
    };

    let mut mirror = LiveMirror::new(&snapshot, &state(8, Some(0)), 4, 1, 250, 0);
    assert_eq!(
        attacks_of(&mirror, 131072),
        0,
        "rebuild: the hostile has already struck"
    );
    assert_eq!(
        attacks_of(&mirror, 65537),
        0,
        "rebuild: so has the rival's Archer"
    );
    mirror.sync(&snapshot, &state(9, Some(1)), 0);
    assert_eq!(attacks_of(&mirror, 131072), 1, "sync: a strike in hand");
    assert_eq!(attacks_of(&mirror, 65537), 1);
    mirror.sync(&snapshot, &state(10, Some(0)), 0);
    assert_eq!(attacks_of(&mirror, 131072), 0, "sync: spent again");
    assert_eq!(attacks_of(&mirror, 65537), 0);
    mirror.sync(&snapshot, &state(11, None), 0);
    assert_eq!(
        attacks_of(&mirror, 131072),
        1,
        "an export without the key: the fresh-turn allowance, as before"
    );
    assert_eq!(attacks_of(&mirror, 65537), 1);

    let absent = LiveMirror::new(&snapshot, &state(8, None), 4, 1, 250, 0);
    assert_eq!(
        (attacks_of(&absent, 131072), attacks_of(&absent, 65537)),
        (1, 1),
        "rebuild without the key: unchanged"
    );
}
