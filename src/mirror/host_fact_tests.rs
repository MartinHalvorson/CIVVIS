use super::*;

fn host_grass(x: i32, y: i32) -> Plot {
    Plot {
        x,
        y,
        t: Some("TERRAIN_GRASS".to_string()),
        f: None,
        r: None,
        o: 0,
        w: false,
        i: false,
        fw: None,
        im: None,
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

/// World Games is one of the ways a diplomatic race moves without a vote:
/// Firaxis grants `PROJECT_TRAIN_ATHLETES` only to active members, and the
/// bridge used to discard the exact tracker that says so.
#[test]
fn world_games_tracker_opens_then_retires_the_host_project() {
    let raw = r#"{
            "turn": 182,
            "emergencies": [{
                "type": "EMERGENCY_WORLD_GAMES",
                "target": 2,
                "turns_left": 8,
                "begun": true,
                "scores": [
                    {"player": 0, "score": 50, "tier": 2},
                    {"player": 2, "score": 100, "tier": 1}
                ],
                "ours": {"member": true, "score": 50, "tier": 2}
            }]
        }"#;
    let mut state = state_from_json(raw).expect("the competition tracker parses");
    assert!(
        state.schema_gaps.is_empty(),
        "the recognized tracker must not be filed as discarded schema: {:?}",
        state.schema_gaps
    );
    assert_eq!(state.emergencies.as_ref().unwrap()[0].target, 2);
    state.cities.push(StateCity {
        id: 1,
        name: "Rome".to_string(),
        x: 3,
        y: 3,
        pop: 5,
        capital: true,
        ..StateCity::default()
    });
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: state.turn,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![host_grass(3, 3)],
    }]);
    let mut mirror = LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
    let city = mirror.game.player_city_ids(0)[0];
    let athletes = crate::game::Item::Project {
        project: crate::name!("train_athletes"),
    };
    let competition = mirror
        .game
        .host_competition(0, "EMERGENCY_WORLD_GAMES")
        .expect("our active World Games score race reaches the board");
    assert_eq!(competition.ours, 50.0);
    assert_eq!(competition.leader, 100.0);
    assert!(
        mirror.game.can_produce(0, city, &athletes),
        "an active member can run the host-granted athlete project"
    );
    assert!(
        mirror.game.producible_items(0, city).contains(&athletes),
        "the active project appears even after the menu is cached"
    );

    // An older control mod did not export this field. Its omission cannot
    // be treated as a completed event, because a persistent mirror may
    // have learned about World Games before the mod was refreshed.
    let completed = state.emergencies.take();
    state.turn += 1;
    mirror.sync(&snapshot, &state, 0);
    assert!(mirror.game.can_produce(0, city, &athletes));

    // `TurnsLeft < 0` is the host's completed marker. It must withdraw
    // the project and invalidate the menu cached above rather than leave
    // CIVVIS repeatedly ordering a project Firaxis no longer accepts.
    state.turn += 1;
    state.emergencies = completed;
    state.emergencies.as_mut().unwrap()[0].turns_left = -1;
    mirror.sync(&snapshot, &state, 0);
    assert!(mirror
        .game
        .host_competition(0, "EMERGENCY_WORLD_GAMES")
        .is_none());
    assert!(!mirror.game.can_produce(0, city, &athletes));
    assert!(!mirror.game.producible_items(0, city).contains(&athletes));

    // A fresh non-member board is equally closed: the project is a host
    // effect, not part of CIVVIS's ordinary ruleset.
    state.emergencies.as_mut().unwrap()[0].turns_left = 8;
    state.emergencies.as_mut().unwrap()[0].ours.member = false;
    let inactive = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0).game;
    let inactive_city = inactive.player_city_ids(0)[0];
    assert!(!inactive.can_produce(0, inactive_city, &athletes));
}

/// Civilization VI's production names must reach CIVVIS's queue as real items.
///
/// ⚠ The export shipped a raw HASH for the whole project, so this path was dead
/// and every city read as idle — CIVVIS then chose production from scratch each
/// turn, blind to work already underway.
#[test]
fn civ6_production_names_become_civvis_queue_items() {
    let rules = crate::rules::Rules::shared();
    let settler = civvis_production_item(&rules, Some("UNIT_SETTLER"), &[], None);
    assert!(
        matches!(settler, Some(crate::game::Item::Unit { .. })),
        "UNIT_SETTLER should map to a CIVVIS unit build, got {settler:?}"
    );
    let monument = civvis_production_item(&rules, Some("BUILDING_MONUMENT"), &[], None);
    assert!(
        matches!(monument, Some(crate::game::Item::Building { .. })),
        "BUILDING_MONUMENT should map to a CIVVIS building, got {monument:?}"
    );
    let theater =
        civvis_production_item(&rules, Some("PROJECT_ENHANCE_DISTRICT_THEATER"), &[], None);
    assert_eq!(
        theater,
        Some(crate::game::Item::Project {
            project: crate::name!("theater_square_festival"),
        })
    );
    assert_eq!(
        civvis_production_item(&rules, Some("PROJECT_TRAIN_ATHLETES"), &[], None),
        Some(crate::game::Item::Project {
            project: crate::name!("train_athletes"),
        }),
        "the host's active World Games queue must remain visibly committed"
    );

    // ⚠ Refusing to guess is the point. A wrong item tells CIVVIS a city is busy
    // with something it is not, which SUPPRESSES a real production decision —
    // worse than the repeated one this fixes.
    assert!(civvis_production_item(&rules, Some("UNIT_NOT_A_REAL_THING"), &[], None).is_none());
    assert!(civvis_production_item(&rules, Some(""), &[], None).is_none());
    assert!(civvis_production_item(&rules, None, &[], None).is_none());
    // A district still refuses when the export did not say WHERE — inventing a
    // plot would place it on arbitrary ground, which is the one thing worse
    // than repeating the order.
    assert!(civvis_production_item(&rules, Some("DISTRICT_CAMPUS"), &[], None).is_none());
    // ...and resolves once the plot is carried, which is what stops a city
    // building a district from reading as idle for sixty turns.
    let campus = civvis_production_item(
        &rules,
        Some("DISTRICT_CAMPUS"),
        &[StateDistrict {
            kind: "DISTRICT_CAMPUS".into(),
            x: 12,
            y: 7,
            pillaged: false,
            complete: false,
            ..StateDistrict::default()
        }],
        None,
    );
    match campus {
        // ⚠ AXIAL, not the offset the export sent. Mixing the two is this
        // bridge's oldest trap and nothing complains, because both are pairs of
        // small integers.
        Some(crate::game::Item::District { pos, .. }) => {
            assert_eq!(pos, crate::hex::offset_to_axial(12, 7));
        }
        other => panic!("a district with a plot should be an Item::District: {other:?}"),
    }
    // A plot for a DIFFERENT district does not answer for this one.
    assert!(civvis_production_item(
        &rules,
        Some("DISTRICT_CAMPUS"),
        &[StateDistrict {
            kind: "DISTRICT_HOLY_SITE".into(),
            x: 3,
            y: 4,
            pillaged: false,
            complete: false,
            ..StateDistrict::default()
        }],
        None,
    )
    .is_none());

    // ★ A wonder under construction is a busy city. `BUILDING_HAGIA_SOPHIA` is
    // not a `rules.buildings` row and used to fall through to None — the first
    // live wonder the seat ever started was replaced by a University the next
    // turn because the mirror seeded Rome's queue empty. With a centre it is a
    // placed marker; without one (block-key translation) it still names the
    // wonder.
    let centre = crate::hex::offset_to_axial(20, 9);
    match civvis_production_item(&rules, Some("BUILDING_HAGIA_SOPHIA"), &[], Some(centre)) {
        Some(crate::game::Item::Wonder { wonder, pos }) => {
            assert_eq!(wonder, crate::name!("hagia_sophia"));
            assert_eq!(pos, centre, "the placeholder plot is the city centre");
        }
        other => panic!("an in-progress wonder should be an Item::Wonder: {other:?}"),
    }
    assert!(matches!(
        civvis_production_item(&rules, Some("BUILDING_HAGIA_SOPHIA"), &[], None),
        Some(crate::game::Item::Wonder { .. })
    ));
    // And an ordinary building is still a building, not a wonder.
    assert!(matches!(
        civvis_production_item(&rules, Some("BUILDING_LIBRARY"), &[], Some(centre)),
        Some(crate::game::Item::Building { .. })
    ));
}

/// ⚠ THE REGRESSION THIS EXISTS TO PREVENT, PINNED AS A TEST.
///
/// The mod's `encode` emits `[]` for an empty Lua table — it takes the array
/// branch whenever `#v == n`, and an empty table satisfies that with both
/// zero. `great_person_points` shipped in #983 as a plain `BTreeMap`, every
/// player has no Great Person points on turn 1, and serde refusing a
/// sequence took **the whole StateSnapshot** down with it, not just the
/// field. Three consecutive live attempts reported "no revealed terrain or
/// no state yet" and 0 orders from turn 1, stalled at turn 6 on an
/// unanswered research prompt, and were killed by the watchdog.
///
/// The empty array must parse, and — this is the part that actually
/// mattered — everything *around* it must survive.
#[test]
fn an_empty_great_person_table_arrives_as_a_json_array_and_must_not_lose_the_board() {
    let raw = r#"{"turn": 92, "gold": 140, "science": 7.5,
                      "great_person_points": [],
                      "great_person_offers": [],
                      "techs": ["TECH_POTTERY"]}"#;
    let state: StateSnapshot =
        serde_json::from_str(raw).expect("an empty map encoded as [] must still parse");
    assert_eq!(
        state.great_person_points,
        Some(BTreeMap::new()),
        "an empty array is an empty race, not a missing field"
    );
    assert_eq!(state.turn, 92, "and the rest of the board must survive it");
    assert_eq!(state.gold, 140);
    assert_eq!(state.techs, vec!["TECH_POTTERY".to_string()]);
    assert!(
        state
            .great_person_offers
            .as_ref()
            .is_some_and(BTreeMap::is_empty),
        "the same Lua empty-map trap must not lose a new named-offer field"
    );
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 92,
        width: 4,
        height: 4,
        chunk: 1,
        plots: vec![host_grass(2, 2)],
    }]);
    let rebuilt = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0).game;
    assert_eq!(
        rebuilt.players[0].live_great_person_offers,
        Some(BTreeSet::new()),
        "an empty host table means no class is recruitable, not an old export"
    );
    assert!(
        !rebuilt.great_person_class_offered_now(0, "scientist"),
        "the native roster must not reopen a truly empty host screen"
    );

    // The populated and absent forms keep working.
    let populated: StateSnapshot = serde_json::from_str(
        r#"{"turn": 3, "great_person_points": {"GREAT_PERSON_CLASS_SCIENTIST": 18.0}}"#,
    )
    .expect("a populated map parses");
    assert_eq!(
        populated.great_person_points.unwrap()["GREAT_PERSON_CLASS_SCIENTIST"],
        18.0
    );
    let absent: StateSnapshot =
        serde_json::from_str(r#"{"turn": 3}"#).expect("an absent field parses");
    assert_eq!(absent.great_person_points, None);
}

/// Housing must survive the wire, including the empty and absent forms.
///
/// This is the field that gates the population every yield is a linear
/// function of, so it is worth pinning that it parses rather than assuming
/// it — the last host field I added took every live game down because an
/// empty value serialised in a shape serde would not read (#983 → #996).
/// ⚠ The eureka discount must survive the wire, and an older mod that sends
/// neither field must still parse — a hard error here takes the WHOLE
/// StateSnapshot down, not just this field (#983 → #996).
#[test]
fn the_eureka_reaches_the_planner_from_the_host() {
    let raw = r#"{"turn": 40, "techs": ["TECH_POTTERY"],
                      "boosted_techs": ["TECH_WRITING", "TECH_MASONRY"],
                      "boosted_civics": ["CIVIC_CRAFTSMANSHIP"]}"#;
    let state: StateSnapshot = serde_json::from_str(raw).expect("boosts parse");
    assert_eq!(state.boosted_techs, ["TECH_WRITING", "TECH_MASONRY"]);
    assert_eq!(state.boosted_civics, ["CIVIC_CRAFTSMANSHIP"]);

    // An empty list is the ordinary case on turn 1 and must be a SEQUENCE.
    let empty: StateSnapshot =
        serde_json::from_str(r#"{"turn": 1, "boosted_techs": [], "boosted_civics": []}"#)
            .expect("an empty boost list parses");
    assert!(empty.boosted_techs.is_empty());

    // And an older mod that sends neither field still parses.
    let absent: StateSnapshot =
        serde_json::from_str(r#"{"turn": 1}"#).expect("an older mod still parses");
    assert!(absent.boosted_techs.is_empty());
    assert!(absent.boosted_civics.is_empty());
}

/// A completed strategic project disappears from every city queue, so this
/// must be a player-history field rather than an inference from production.
///
/// On the turn-251 supervised live game, the fresh board saw zero completed
/// projects and spent five cities' production repeatedly on Manhattan Project.
/// The live export needs to preserve the host's player-wide completion ledger
/// so the existing science and nuclear-roadmap gates can skip it.
#[test]
fn completed_strategic_projects_cross_the_live_bridge_without_false_mars_progress() {
    let raw = r#"{"turn": 205, "science_projects": [
            "PROJECT_MANHATTAN_PROJECT",
            "PROJECT_OPERATION_IVY",
            "PROJECT_LAUNCH_EARTH_SATELLITE",
            "PROJECT_LAUNCH_MOON_LANDING",
            "PROJECT_LAUNCH_MARS_BASE",
            "PROJECT_LAUNCH_EXOPLANET_EXPEDITION"
        ]}"#;
    let mut state = state_from_json(raw).expect("the strategic project wire parses");
    assert!(
        state.schema_gaps.is_empty(),
        "the new wire key is recognized"
    );
    assert_eq!(
        state.science_projects,
        Some(vec![
            "PROJECT_MANHATTAN_PROJECT".to_string(),
            "PROJECT_OPERATION_IVY".to_string(),
            "PROJECT_LAUNCH_EARTH_SATELLITE".to_string(),
            "PROJECT_LAUNCH_MOON_LANDING".to_string(),
            "PROJECT_LAUNCH_MARS_BASE".to_string(),
            "PROJECT_LAUNCH_EXOPLANET_EXPEDITION".to_string(),
        ])
    );

    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 205,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![host_grass(3, 3)],
    }]);
    let expected = BTreeSet::from([
        "manhattan_project".to_string(),
        "operation_ivy".to_string(),
        "launch_earth_satellite".to_string(),
        "launch_moon_landing".to_string(),
        "launch_mars_colony".to_string(),
        "exoplanet_expedition".to_string(),
    ]);
    let rebuilt = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    assert_eq!(rebuilt.game.players[0].science_projects, expected);
    assert!(
        !rebuilt
            .unmapped
            .iter()
            .any(|issue| issue.starts_with("science_project:")),
        "every strategic project on the supported wire must survive: {:?}",
        rebuilt.unmapped
    );

    // The persistent path must use the same truth and retain it if an older
    // mod is later reloaded and does not yet know the field.
    let before_export = StateSnapshot {
        turn: 204,
        ..StateSnapshot::default()
    };
    let mut mirror = LiveMirror::new(&snapshot, &before_export, 2, 1, 250, 0);
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.players[0].science_projects, expected);
    state.turn += 1;
    state.science_projects = None;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(
        mirror.game.players[0].science_projects, expected,
        "an absent field means an older mod, not that history was erased"
    );

    // Base Civ VI reports Mars as three independent components. CIVVIS has
    // one Mars-colony milestone, so two components are progress but not
    // completion; all three are the one truthful completion transition.
    let partial_mars = vec![
        "PROJECT_LAUNCH_MARS_REACTOR".to_string(),
        "PROJECT_LAUNCH_MARS_HABITATION".to_string(),
    ];
    let mut issues = Vec::new();
    let partial = completed_strategic_projects(Some(&partial_mars), &mut issues)
        .expect("an explicit host list answers");
    assert!(issues.is_empty());
    assert!(!partial.contains("launch_mars_colony"));
    let full_mars = vec![
        "PROJECT_LAUNCH_MARS_REACTOR".to_string(),
        "PROJECT_LAUNCH_MARS_HABITATION".to_string(),
        "PROJECT_LAUNCH_MARS_HYDROPONICS".to_string(),
    ];
    assert!(completed_strategic_projects(Some(&full_mars), &mut issues)
        .expect("an explicit host list answers")
        .contains("launch_mars_colony"));
}

#[test]
fn housing_reaches_the_planner_from_the_host() {
    let raw = r#"{"id": 1, "x": 3, "y": 4, "pop": 12,
                      "housing": 14.0, "housing_from_improvements": 5.0}"#;
    let city: StateCity = serde_json::from_str(raw).expect("housing parses");
    assert_eq!(city.housing, Some(14.0));
    assert_eq!(city.housing_from_improvements, Some(5.0));
    assert_eq!(city.pop, 12, "and the rest of the city survives it");

    // A host that cannot answer sends -1 through `try`, and an older mod
    // sends nothing at all. Neither may cost us the city.
    let refused: StateCity =
        serde_json::from_str(r#"{"id": 1, "x": 3, "y": 4, "pop": 12, "housing": -1}"#)
            .expect("a refused housing read still parses");
    assert_eq!(refused.housing, Some(-1.0));
    assert_eq!(refused.pop, 12);

    let absent: StateCity = serde_json::from_str(r#"{"id": 1, "x": 3, "y": 4, "pop": 12}"#)
        .expect("an older mod that sends no housing still parses");
    assert_eq!(absent.housing, None);
    assert_eq!(absent.pop, 12);
}

/// The Great Person race the planner prices against must actually exist.
///
/// `district_project_value` reads `players[pid].gpp` for this empire and
/// every rival, and awards up to 150 for closing on a leader and 240 for
/// overtaking one. Before this the field was never written from a live
/// game, so both sides of every one of those comparisons were 0.0 — which
/// is why the Campus research project, whose entire payoff is Great
/// Scientist points, was chosen 7 times against 131 for the other district
/// projects across five live runs.
#[test]
fn great_person_points_reach_the_planner_from_the_host() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 92,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![host_grass(3, 3)],
    }]);
    let mut points = BTreeMap::new();
    points.insert("GREAT_PERSON_CLASS_SCIENTIST".to_string(), 118.0);
    points.insert("GREAT_PERSON_CLASS_WRITER".to_string(), 12.5);
    // A class Civilization VI could add that CIVVIS has never heard of must
    // be reported, not silently dropped.
    points.insert("GREAT_PERSON_CLASS_ASTRONAUT".to_string(), 4.0);
    points.insert("NOT_A_GREAT_PERSON_CLASS".to_string(), 9.0);
    let state = StateSnapshot {
        turn: 92,
        great_person_points: Some(points),
        ..StateSnapshot::default()
    };
    let report = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let game = &report.game;

    assert_eq!(
        game.players[0].gpp.get("scientist").copied(),
        Some(118.0),
        "the Scientist race is what the Campus project is played for"
    );
    assert_eq!(game.players[0].gpp.get("writer").copied(), Some(12.5));
    assert_eq!(
        game.players[0].gpp.get("astronaut").copied(),
        Some(4.0),
        "an unfamiliar class still translates by its suffix"
    );
    assert!(
        report
            .unmapped
            .iter()
            .any(|issue| issue.contains("NOT_A_GREAT_PERSON_CLASS")),
        "a class that does not carry the prefix must be reported: {:?}",
        report.unmapped
    );
    assert!(
        !game.players[0].gpp.contains_key("not_a_great_person_class"),
        "and must not be invented into the race"
    );
}

/// The live recruit COST must land on the class's current person, so the
/// planner's `gp_cost - points` gate answers with the live game's number.
/// Run civvis-20260815T033823Z: 45 `gp_cannot_recruit` refusals because
/// the ask was priced by CIVVIS's market formula instead of the timeline
/// the order is judged by.
#[test]
fn live_recruit_costs_reprice_the_current_great_person() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 92,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![host_grass(3, 3)],
    }]);
    let mut costs = BTreeMap::new();
    costs.insert("GREAT_PERSON_CLASS_SCIENTIST".to_string(), 385.0);
    costs.insert("NOT_A_GREAT_PERSON_CLASS".to_string(), 9.0);
    let state = StateSnapshot {
        turn: 92,
        great_person_costs: Some(costs),
        ..StateSnapshot::default()
    };
    let report = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let game = &report.game;

    assert_eq!(
        game.gp_cost(0, "scientist"),
        385.0,
        "the gate must quote the live timeline, not the market formula"
    );
    assert!(
        report
            .unmapped
            .iter()
            .any(|issue| issue.contains("great_person_cost_class:NOT_A_GREAT_PERSON_CLASS")),
        "an unprefixed class must be reported: {:?}",
        report.unmapped
    );

    // An older mod that sends no costs must parse to None, so the import
    // never runs and the engine's own offer pricing stays in charge. (The
    // offer map itself is NOT empty after a rebuild — the engine prices
    // its own market — so absence is asserted on the wire, not the map.)
    let bare: StateSnapshot =
        serde_json::from_str(r#"{"turn": 3}"#).expect("an absent field parses");
    assert_eq!(bare.great_person_costs, None);
}

#[test]
fn physical_great_people_without_activation_plots_reach_production_planning() {
    let mut game = crate::game::Game::new_full(1, 20, 14, 95_104, 80, 0, false);
    let mut state = StateSnapshot {
        units: vec![StateUnit {
            id: 77,
            kind: "UNIT_GREAT_SCIENTIST".to_string(),
            great_person: Some(StateGreatPerson {
                individual: Some("GREAT_PERSON_INDIVIDUAL_HILDEGARD_OF_BINGEN".to_string()),
                class: Some("GREAT_PERSON_CLASS_SCIENTIST".to_string()),
                required_district: Some("DISTRICT_HOLY_SITE".to_string()),
                charges: 1,
                can_activate: false,
                activation_plots: Vec::new(),
                empty_slots: None,
            }),
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };
    let mut unmapped = Vec::new();

    apply_great_person_points(&mut game, &state, &mut unmapped);

    assert!(unmapped.is_empty(), "the stock class and district both map");
    assert_eq!(game.players[0].live_great_person_activation_needs.len(), 1);
    let need = &game.players[0].live_great_person_activation_needs[0];
    assert_eq!(need.kind, "scientist");
    assert_eq!(need.individual.as_deref(), Some("hildegard_of_bingen"));
    assert_eq!(need.required_district.as_deref(), Some("holy_site"));

    state.units[0]
        .great_person
        .as_mut()
        .unwrap()
        .activation_plots
        .push(StateActivationPlot {
            x: 8,
            y: 5,
            distance: 2,
            ..StateActivationPlot::default()
        });
    apply_great_person_points(&mut game, &state, &mut unmapped);
    assert!(
        game.players[0]
            .live_great_person_activation_needs
            .is_empty(),
        "a host-valid destination clears the production demand immediately"
    );
}

/// A highlighted plot is a *place*, not a *use*. Firaxis highlights a
/// cultural person's district whether or not a compatible Great Work slot
/// is free, so seven Writers/Artists/Musicians stood on one Theater plot
/// for thirty-plus turns on run civvis-20260817T010950Z while the old
/// plots-non-empty gate read them as needing nothing. The host's own
/// empty-slot count is the tiebreaker: zero compatible slots anywhere is
/// a production need exactly as surely as no plot at all.
#[test]
fn a_slot_starved_person_with_highlighted_plots_is_still_a_need() {
    let mut game = crate::game::Game::new_full(1, 20, 14, 95_104, 80, 0, false);
    let person = |empty_slots: Option<u32>, can_activate: bool| StateGreatPerson {
        individual: Some("GREAT_PERSON_INDIVIDUAL_MARK_TWAIN".to_string()),
        class: Some("GREAT_PERSON_CLASS_WRITER".to_string()),
        required_district: None,
        charges: 0,
        can_activate,
        activation_plots: vec![StateActivationPlot {
            x: 25,
            y: 23,
            distance: 0,
            ..StateActivationPlot::default()
        }],
        empty_slots,
    };
    let mut state = StateSnapshot {
        units: vec![StateUnit {
            id: 90,
            kind: "UNIT_GREAT_WRITER".to_string(),
            great_person: Some(person(Some(0), false)),
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };
    let mut unmapped = Vec::new();

    apply_great_person_points(&mut game, &state, &mut unmapped);
    assert_eq!(
        game.players[0].live_great_person_activation_needs.len(),
        1,
        "zero empty slots with highlighted plots is a need"
    );
    assert_eq!(
        game.players[0].live_great_person_activation_needs[0].kind,
        "writer"
    );

    // Slots free: the highlighted plot really is actionable — no need.
    state.units[0].great_person = Some(person(Some(3), false));
    apply_great_person_points(&mut game, &state, &mut unmapped);
    assert!(game.players[0]
        .live_great_person_activation_needs
        .is_empty());

    // An older mod that cannot count slots sends nothing: old behaviour,
    // no need while plots are listed.
    state.units[0].great_person = Some(person(None, false));
    apply_great_person_points(&mut game, &state, &mut unmapped);
    assert!(game.players[0]
        .live_great_person_activation_needs
        .is_empty());

    // And the host saying "activate now" outranks its slot arithmetic.
    state.units[0].great_person = Some(person(Some(0), true));
    apply_great_person_points(&mut game, &state, &mut unmapped);
    assert!(game.players[0]
        .live_great_person_activation_needs
        .is_empty());
}

/// The nine Great People of live run `civvis-20260822T020434Z`, and the
/// gap they fell through.
///
/// Three Artists, three Writers, three Musicians and a Scientist stood in
/// Rome at turn 231 with NOT ONE ORDER between them in the whole game.
/// The test above closed the `empty_slots == Some(0)` case; these nine
/// were never in it. Their exports read **24, 4 and 2** empty slots —
/// compatible slots the EMPIRE owns — while every plot the host offered
/// them read `slot_open: false`, tile by tile: nowhere this person can
/// put a work. The needs machinery saw a non-empty plot list and a
/// non-zero count and concluded there was nothing to build, so no city
/// ever started the Amphitheater or Museum that would have seated them.
#[test]
fn every_offered_plot_full_is_a_need_however_many_slots_the_empire_owns() {
    let mut game = crate::game::Game::new_full(1, 20, 14, 95_104, 80, 0, false);
    // As exported at turn 231: three of the Writer's plots, all closed.
    let closed = |x: i32, y: i32, distance: i32| StateActivationPlot {
        x,
        y,
        distance,
        slot_open: Some(false),
    };
    let writer = |empty_slots: Option<u32>| StateGreatPerson {
        individual: Some("GREAT_PERSON_INDIVIDUAL_HG_WELLS".to_string()),
        class: Some("GREAT_PERSON_CLASS_WRITER".to_string()),
        required_district: None,
        charges: 0,
        can_activate: false,
        activation_plots: vec![closed(67, 14, 12), closed(65, 25, 1), closed(64, 27, 2)],
        empty_slots,
    };
    let mut state = StateSnapshot {
        units: vec![StateUnit {
            id: 10_092_559,
            kind: "UNIT_GREAT_WRITER".to_string(),
            great_person: Some(writer(Some(24))),
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };
    let mut unmapped = Vec::new();

    apply_great_person_points(&mut game, &state, &mut unmapped);
    assert_eq!(
        game.players[0].live_great_person_activation_needs.len(),
        1,
        "twenty-four slots the empire owns and none this Writer can reach"
    );
    assert_eq!(
        game.players[0].live_great_person_activation_needs[0].kind,
        "writer"
    );

    // One reachable slot and the need is gone — the empire has somewhere
    // to seat them and should not spend production on another building.
    state.units[0]
        .great_person
        .as_mut()
        .unwrap()
        .activation_plots[1]
        .slot_open = Some(true);
    apply_great_person_points(&mut game, &state, &mut unmapped);
    assert!(
        game.players[0]
            .live_great_person_activation_needs
            .is_empty(),
        "a reachable slot is not a reason to build capacity"
    );

    // ⚠ And an older control mod, which sends `slot_open` on no plot at
    // all, keeps exactly the behaviour it had: `None` is an absence, not
    // a claim, and must never be read as "full".
    let mut older = writer(None);
    for plot in &mut older.activation_plots {
        plot.slot_open = None;
    }
    state.units[0].great_person = Some(older);
    apply_great_person_points(&mut game, &state, &mut unmapped);
    assert!(
        game.players[0]
            .live_great_person_activation_needs
            .is_empty(),
        "an unknowing export must not manufacture a need"
    );
}

/// The government HISTORY must reach the planner, so a return switch is
/// priced at its real Anarchy cost instead of free. Run
/// civvis-20260815T012010Z: 127 guard blocks and 15 deck-refusal turns
/// from the planner re-proposing a used government (deck and all) that
/// its history-less board believed was a fresh, free switch.
#[test]
fn used_governments_reach_the_planners_history() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 92,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![host_grass(3, 3)],
    }]);
    let state = StateSnapshot {
        turn: 92,
        government: Some("GOVERNMENT_MONARCHY".to_string()),
        used_governments: vec![
            "GOVERNMENT_CHIEFDOM".to_string(),
            "GOVERNMENT_OLIGARCHY".to_string(),
            "GOVERNMENT_MONARCHY".to_string(),
            "GOVERNMENT_FROM_A_FUTURE_EXPANSION".to_string(),
        ],
        ..StateSnapshot::default()
    };
    let report = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    let game = &report.game;

    for used in ["chiefdom", "oligarchy", "monarchy"] {
        assert!(
            game.players[0].past_governments.contains(used),
            "{used} must be in the seeded history"
        );
    }
    assert!(
        report
            .unmapped
            .iter()
            .any(|issue| issue.contains("GOVERNMENT_FROM_A_FUTURE_EXPANSION")),
        "an unknown government must be reported: {:?}",
        report.unmapped
    );

    // An older mod that sends no history must not invent one.
    let bare: StateSnapshot =
        serde_json::from_str(r#"{"turn": 3}"#).expect("an absent field parses");
    assert!(bare.used_governments.is_empty());
}

/// A host offer's class does not tell us the infrastructure it needs.
///
/// Run civvis-20260815T042826Z recruited Hildegard of Bingen into an
/// empire with three Campuses but no Holy Site, then Mary Leakey into the
/// same Theater-less science empire. They had zero activation plots for
/// 190 and 74 turns respectively. The live offer's required district must
/// therefore gate every way the reconstructed game can claim it, while a
/// later host state that does have the district must immediately reopen the
/// ordinary class race.
#[test]
fn live_offer_district_blocker_prevents_an_unusable_scientist_race() {
    let wire = state_from_json(
        r#"{"turn":92,"great_person_offers":{"GREAT_PERSON_CLASS_SCIENTIST":{"individual":"GREAT_PERSON_INDIVIDUAL_HILDEGARD_OF_BINGEN","required_district":"DISTRICT_HOLY_SITE"}}}"#,
    )
    .expect("the Lua offer shape parses");
    assert!(
        wire.schema_gaps.is_empty(),
        "the recognized offer stays quiet"
    );
    let wire_offer = wire
        .great_person_offers
        .as_ref()
        .and_then(|offers| offers.get("GREAT_PERSON_CLASS_SCIENTIST"))
        .expect("the named Scientist offer crosses the wire");
    assert_eq!(
        wire_offer.required_district.as_deref(),
        Some("DISTRICT_HOLY_SITE")
    );
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 92,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![host_grass(2, 3), host_grass(3, 3), host_grass(4, 3)],
    }]);
    let mut offers = BTreeMap::new();
    offers.insert(
        "GREAT_PERSON_CLASS_SCIENTIST".to_string(),
        StateGreatPersonOffer {
            individual: Some("GREAT_PERSON_INDIVIDUAL_HILDEGARD_OF_BINGEN".to_string()),
            required_district: Some("DISTRICT_HOLY_SITE".to_string()),
        },
    );
    let campus_only = StateSnapshot {
        turn: 92,
        cities: vec![StateCity {
            id: 65_536,
            name: "Rome".to_string(),
            x: 3,
            y: 3,
            pop: 6,
            capital: true,
            districts: vec![StateDistrict {
                kind: "DISTRICT_CAMPUS".to_string(),
                x: 4,
                y: 3,
                ..StateDistrict::default()
            }],
            ..StateCity::default()
        }],
        great_person_offers: Some(offers.clone()),
        ..StateSnapshot::default()
    };
    let mut game = rebuild_from_state(&snapshot, &campus_only, 2, 1, 250, 0).game;

    assert_eq!(
        game.players[0].live_great_person_offers,
        Some(["scientist".to_string()].into_iter().collect()),
        "the host's named screen, not CIVVIS's whole roster, is the live offer set"
    );
    assert!(game.great_person_class_offered_now(0, "scientist"));
    assert!(
        !game.great_person_class_offered_now(0, "merchant"),
        "a class omitted from Firaxis's table cannot receive a local order"
    );

    let blocker = game
        .live_great_person_offer_blocker(0, "scientist")
        .expect("Hildegard cannot use a Campus as a Holy Site");
    assert!(blocker.contains("HILDEGARD_OF_BINGEN"));
    assert!(blocker.contains("DISTRICT_HOLY_SITE"));
    assert!(
        !game.can_activate_current_great_person(0, "scientist"),
        "the generic Campus Scientist must yield to the live named offer"
    );
    let cost = game.gp_cost(0, "scientist");
    game.players[0].gpp.insert("scientist".to_string(), cost);
    assert!(
        game.apply(
            0,
            &crate::game::Action::RecruitGreatPerson {
                kind: "scientist".to_string(),
            },
        )
        .is_err(),
        "even a ready-point automatic claim must share the live blocker"
    );

    let mut with_holy_site = campus_only.clone();
    with_holy_site.cities[0].districts.push(StateDistrict {
        kind: "DISTRICT_HOLY_SITE".to_string(),
        x: 2,
        y: 3,
        ..StateDistrict::default()
    });
    let reopened = rebuild_from_state(&snapshot, &with_holy_site, 2, 1, 250, 0).game;
    assert!(
        reopened
            .live_great_person_offer_blocker(0, "scientist")
            .is_none(),
        "the necessary district removes only the live hard blocker"
    );
    assert!(
        reopened.can_activate_current_great_person(0, "scientist"),
        "the ordinary Campus-targeted model resumes once Firaxis's condition holds"
    );

    let mut city_center_only = campus_only;
    city_center_only
        .great_person_offers
        .as_mut()
        .and_then(|offers| offers.get_mut("GREAT_PERSON_CLASS_SCIENTIST"))
        .expect("the test offer remains present")
        .required_district = Some("DISTRICT_CITY_CENTER".to_string());
    let centre_open = rebuild_from_state(&snapshot, &city_center_only, 2, 1, 250, 0).game;
    assert!(
        centre_open
            .live_great_person_offer_blocker(0, "scientist")
            .is_none(),
        "every exported city has its implicit City Center even though it is not in districts"
    );

    let bare: StateSnapshot =
        serde_json::from_str(r#"{"turn": 3}"#).expect("an older control mod still parses");
    assert!(bare.great_person_offers.is_none());
}

#[test]
fn firaxis_governors_replace_inferred_titles_roster_and_promotions() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 92,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![host_grass(3, 3)],
    }]);
    let state = StateSnapshot {
        turn: 92,
        governor_points: Some(4),
        governor_points_spent: Some(4),
        governors: Some(vec![
            StateGovernor {
                kind: "GOVERNOR_THE_DEFENDER".to_string(),
                city: 65_536,
                city_player: 0,
                x: 3,
                y: 3,
                established: true,
                turns_on_site: 20,
                turns_to_establish: 3,
                promotions: vec![
                    "GOVERNOR_PROMOTION_REDOUBT".to_string(),
                    "GOVERNOR_PROMOTION_GARRISON_COMMANDER".to_string(),
                    "GOVERNOR_PROMOTION_DEFENSE_LOGISTICS".to_string(),
                ],
                ..StateGovernor::default()
            },
            StateGovernor {
                kind: "GOVERNOR_THE_RESOURCE_MANAGER".to_string(),
                city: -1,
                promotions: vec![
                    "GOVERNOR_PROMOTION_RESOURCE_MANAGER_GROUNDBREAKER".to_string(),
                    "GOVERNOR_PROMOTION_RESOURCE_MANAGER_SURPLUS_LOGISTICS".to_string(),
                ],
                ..StateGovernor::default()
            },
            StateGovernor {
                kind: "GOVERNOR_THE_EDUCATOR".to_string(),
                city: -1,
                promotions: vec!["GOVERNOR_PROMOTION_EDUCATOR_LIBRARIAN".to_string()],
                ..StateGovernor::default()
            },
        ]),
        cities: vec![StateCity {
            id: 65_536,
            name: "Capital".to_string(),
            x: 3,
            y: 3,
            pop: 6,
            capital: true,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };

    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    let player = &rebuilt.game.players[0];
    let victor = player
        .governor_roster
        .get("victor")
        .expect("Victor crosses from the Firaxis roster");
    assert!(victor.city.is_some());
    assert!(victor.promotions.contains("garrison_commander"));
    assert!(victor.promotions.contains("defense_logistics"));
    let magnus = &player.governor_roster["magnus"];
    assert_eq!(
        magnus.promotions.iter().cloned().collect::<Vec<_>>(),
        vec!["surplus_logistics".to_string()]
    );
    assert!(player.governor_roster["pingala"].promotions.is_empty());
    assert!(rebuilt
        .unmapped
        .iter()
        .all(|issue| !issue.ends_with(":governor_promotion")));
    assert_eq!(player.governor_titles_spent, 4);
    assert_eq!(rebuilt.game.governor_titles(0), 4);
    assert_eq!(rebuilt.game.governor_titles_available(0), 0);
    assert!(rebuilt.game.legal_actions(0).iter().all(|action| !matches!(
        action,
        crate::game::Action::AssignGovernor { .. }
            | crate::game::Action::AppointGovernor { .. }
            | crate::game::Action::ReassignGovernor { .. }
            | crate::game::Action::PromoteGovernor { .. }
    )));
}

/// ★★★★★ The number that decides whether the empire notices it is going
/// broke. `--fresh-board` rebuilds the mirror every turn, so the derived
/// rate has no predecessor and reads 0 forever; live run
/// `civvis-20260810T191050Z` sat at a zero treasury for its last 75 turns,
/// lost its army to non-payment and went from six cities to two.
#[test]
fn the_hosts_net_income_survives_a_board_rebuilt_from_scratch() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 110,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![host_grass(3, 3)],
    }]);
    let state = StateSnapshot {
        turn: 110,
        gold: 0,
        gold_per_turn: Some(-14.0),
        cities: vec![StateCity {
            id: 65_536,
            name: "Roma".to_string(),
            x: 3,
            y: 3,
            pop: 9,
            capital: true,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };

    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    assert_eq!(
        rebuilt.game.players[0].gold_per_turn, -14.0,
        "a rebuilt board must still know the empire is losing 14 gold a turn"
    );
}

/// A host that does not answer must not be read as break-even.
#[test]
fn an_unanswered_net_income_does_not_become_zero() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![host_grass(3, 3)],
    }]);
    let mut state = StateSnapshot {
        turn: 40,
        gold: 120,
        gold_per_turn: None,
        cities: vec![StateCity {
            id: 65_536,
            name: "Roma".to_string(),
            x: 3,
            y: 3,
            pop: 4,
            capital: true,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let silent = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    state.gold_per_turn = Some(0.0);
    let break_even = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    assert_eq!(
        break_even.game.players[0].gold_per_turn, 0.0,
        "a real 0 is break-even and must be applied"
    );
    // The silent case must be left at whatever the board already held rather
    // than being told, wrongly, that the books balance.
    assert!(silent.game.players[0].gold_per_turn.is_finite());
}

#[test]
fn firaxis_era_score_and_age_thresholds_reach_the_board() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 92,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![host_grass(3, 3)],
    }]);
    let state = StateSnapshot {
        turn: 92,
        era_score: Some(31),
        era_score_baseline: Some(12),
        normal_age_threshold: Some(20),
        golden_age_threshold: Some(40),
        world_era: Some(2),
        cities: vec![StateCity {
            id: 65_536,
            name: "Roma".to_string(),
            x: 3,
            y: 3,
            pop: 6,
            capital: true,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };

    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    let player = &rebuilt.game.players[0];
    assert_eq!(player.era_score, 31);
    assert_eq!(player.era_score_baseline, 12);
    assert_eq!(player.normal_age_threshold, 20);
    assert_eq!(player.golden_age_threshold, 40);
    assert_eq!(rebuilt.game.world_era, 2);
    // The point of carrying them: 31 sits between Firaxis's two thresholds,
    // so this is a Normal age. Against `Player::default` (12 and 26) the
    // same empire read as GOLDEN, which is the fiction this closes.
    assert!(player.era_score >= player.normal_age_threshold);
    assert!(player.era_score < player.golden_age_threshold);
}

#[test]
fn an_unanswered_era_getter_leaves_the_board_alone() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![host_grass(3, 3)],
    }]);
    // `try(...)` in the mod yields -1 when a getter is missing on the build.
    // A -1 must not become an era score, and it must not zero a threshold —
    // that would be a worse lie than the default it replaced.
    let state = StateSnapshot {
        turn: 40,
        era_score: Some(-1),
        normal_age_threshold: Some(-1),
        golden_age_threshold: Some(-1),
        world_era: Some(-1),
        cities: vec![StateCity {
            id: 65_536,
            name: "Roma".to_string(),
            x: 3,
            y: 3,
            pop: 4,
            capital: true,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };

    let silent = StateSnapshot {
        era_score: None,
        normal_age_threshold: None,
        golden_age_threshold: None,
        world_era: None,
        ..state.clone()
    };

    let refused = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    let absent = rebuild_from_state(&snapshot, &silent, 4, 1, 250, 0);
    assert_eq!(
        refused.game.players[0].normal_age_threshold, absent.game.players[0].normal_age_threshold,
        "a -1 answer must leave the threshold exactly where no answer leaves it"
    );
    assert_eq!(
        refused.game.players[0].golden_age_threshold,
        absent.game.players[0].golden_age_threshold
    );
    assert_eq!(refused.game.world_era, absent.game.world_era);
}

/// The defect this file's `apply_encampment_health` exists for: a city that
/// owns a HEALTHY Encampment must not be able to produce `repair_encampment`.
///
/// Before the fix `encampment_hp` was 0 on every mirrored board, the gate
/// `encampment_hp < 100` passed forever, the AI queued the repair every turn,
/// the bridge discarded it as a project Civ 6 does not have, and the city
/// built nothing for the rest of the game.
#[test]
fn a_healthy_encampment_cannot_be_repaired_forever() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 67,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (2..8)
            .flat_map(|x| (2..8).map(move |y| host_grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 67,
        cities: vec![StateCity {
            id: 65_536,
            name: "Ravenna".to_string(),
            x: 4,
            y: 4,
            pop: 10,
            capital: true,
            districts: vec![StateDistrict {
                kind: "DISTRICT_ENCAMPMENT".to_string(),
                x: 5,
                y: 4,
                pillaged: false,
                complete: true,
                // Firaxis's own reading for an undamaged Encampment.
                damage: 0,
                max_damage: 100,
                wall_damage: 0,
                max_wall_damage: 0,
            }],
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };

    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    let cid = *rebuilt.city_ids.keys().next().expect("the city was placed");
    // ⚠ Without this the `can_produce` assertion below passes VACUOUSLY: a
    // board where the Encampment never landed also refuses the repair, for
    // an entirely different reason.
    assert!(
        rebuilt.game.cities[&cid]
            .districts
            .contains_key(Name::new("encampment")),
        "the fixture must actually place the Encampment, or the refusal below \
             proves nothing"
    );
    assert_eq!(
        rebuilt.game.cities[&cid].encampment_hp, 100,
        "an undamaged Encampment is at full health, not the 0 the default left"
    );
    let repair = crate::game::Item::Project {
        project: crate::name::Name::new("repair_encampment"),
    };
    assert!(
        !rebuilt.game.can_produce(0, cid, &repair),
        "a healthy Encampment must not offer a repair — this is the order that \
             was discarded every turn while the city built nothing"
    );
}

/// The other half: a genuinely damaged Encampment must still be repairable,
/// or the fix would have traded one silent failure for another.
#[test]
fn a_damaged_encampment_is_still_worth_repairing() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 67,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (2..8)
            .flat_map(|x| (2..8).map(move |y| host_grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 67,
        cities: vec![StateCity {
            id: 65_536,
            name: "Ravenna".to_string(),
            x: 4,
            y: 4,
            pop: 10,
            capital: true,
            districts: vec![StateDistrict {
                kind: "DISTRICT_ENCAMPMENT".to_string(),
                x: 5,
                y: 4,
                pillaged: false,
                complete: true,
                damage: 60,
                max_damage: 100,
                wall_damage: 0,
                max_wall_damage: 0,
            }],
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };

    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    let cid = *rebuilt.city_ids.keys().next().expect("the city was placed");
    assert_eq!(rebuilt.game.cities[&cid].encampment_hp, 40);
}

/// A host that does not answer must leave the Encampment FULL, never 0.
/// The asymmetry is the point: a wrong "healthy" costs one skipped repair, a
/// wrong "destroyed" costs the city's whole production for the rest of the
/// game.
#[test]
fn an_unanswered_encampment_reads_healthy_not_destroyed() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 67,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (2..8)
            .flat_map(|x| (2..8).map(move |y| host_grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 67,
        cities: vec![StateCity {
            id: 65_536,
            name: "Ravenna".to_string(),
            x: 4,
            y: 4,
            pop: 10,
            capital: true,
            districts: vec![StateDistrict {
                kind: "DISTRICT_ENCAMPMENT".to_string(),
                x: 5,
                y: 4,
                pillaged: false,
                complete: true,
                // Every getter unanswered, as an older mod build would send.
                ..StateDistrict::default()
            }],
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };

    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    let cid = *rebuilt.city_ids.keys().next().expect("the city was placed");
    assert_eq!(rebuilt.game.cities[&cid].encampment_hp, 100);
}

#[test]
fn a_zero_era_score_is_a_reading_not_a_missing_answer() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 3,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![host_grass(3, 3)],
    }]);
    let state = StateSnapshot {
        turn: 3,
        era_score: Some(0),
        normal_age_threshold: Some(11),
        golden_age_threshold: Some(25),
        cities: vec![StateCity {
            id: 65_536,
            name: "Roma".to_string(),
            x: 3,
            y: 3,
            pop: 1,
            capital: true,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };

    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    let player = &rebuilt.game.players[0];
    assert_eq!(player.era_score, 0);
    assert_eq!(player.normal_age_threshold, 11);
    // On turn 3 with nothing scored yet Rome is genuinely BELOW the normal
    // threshold, which is the Dark Age warning the board could never show.
    assert!(player.era_score < player.normal_age_threshold);
}

#[test]
fn firaxis_escort_formation_survives_the_fresh_board_rebuild() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 93,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![host_grass(3, 3)],
    }]);
    let state = StateSnapshot {
        turn: 93,
        units: vec![
            StateUnit {
                id: 501,
                kind: "UNIT_SETTLER".to_string(),
                x: 3,
                y: 3,
                hp: 100.0,
                formation_count: 2,
                ..StateUnit::default()
            },
            StateUnit {
                id: 502,
                kind: "UNIT_WARRIOR".to_string(),
                x: 3,
                y: 3,
                hp: 100.0,
                formation_count: 2,
                ..StateUnit::default()
            },
        ],
        ..StateSnapshot::default()
    };

    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    let uid_for = |host| {
        rebuilt
            .unit_ids
            .iter()
            .find_map(|(uid, observed)| (*observed == host).then_some(*uid))
            .expect("host unit crosses into the mirror")
    };
    let settler = uid_for(501);
    let warrior = uid_for(502);

    assert_eq!(rebuilt.game.units[&settler].linked_to, Some(warrior));
    assert_eq!(rebuilt.game.units[&warrior].linked_to, Some(settler));
}

/// ★★★★★ A CORPS EXPORTED BY THE HOST HAS TO ARRIVE AS A CORPS.
///
/// #2373 wired `Action::CombineUnits` to Firaxis's two merge commands and
/// chooses between them by reading this exact field off the mirror:
/// `UNITCOMMAND_FORM_CORPS` for two standard units, `UNITCOMMAND_FORM_ARMY`
/// for a standard unit joining a Corps. The live seat runs `--fresh-board`,
/// so the mirror is rebuilt from the host export every turn — and until this
/// change the export said nothing about the tier, every unit was
/// reconstructed at 0, and the seat could only ever ask for a Corps. That is
/// not a near miss: the two are different commands behind different civics
/// (Nationalism and Mobilization), so an existing Corps was being sent an
/// order the host must refuse.
///
/// The escort count rides alongside and is a DIFFERENT mechanism: a Corps is
/// one unit and reports `formation_count` 1, so it must not be linked to
/// anything by the escort reconstruction directly below.
#[test]
fn a_host_corps_and_army_survive_the_fresh_board_rebuild() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 140,
        width: 12,
        height: 12,
        chunk: 1,
        plots: vec![
            host_grass(3, 3),
            host_grass(4, 3),
            host_grass(5, 3),
            host_grass(6, 3),
        ],
    }]);
    let swordsman = |id: i64, x: i32, formation: Option<i32>| StateUnit {
        id,
        kind: "UNIT_SWORDSMAN".to_string(),
        x,
        y: 3,
        hp: 100.0,
        formation,
        ..StateUnit::default()
    };
    let state = StateSnapshot {
        turn: 140,
        units: vec![
            swordsman(601, 3, Some(1)),
            swordsman(602, 4, Some(2)),
            swordsman(603, 5, Some(0)),
            // The mod's "asked, could not answer". Unknown, never standard.
            swordsman(604, 6, Some(-1)),
        ],
        ..StateSnapshot::default()
    };

    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    let uid_for = |host| {
        rebuilt
            .unit_ids
            .iter()
            .find_map(|(uid, observed)| (*observed == host).then_some(*uid))
            .expect("host unit crosses into the mirror")
    };

    assert_eq!(
        rebuilt.game.units[&uid_for(601)].formation,
        1,
        "a host Corps must arrive as a Corps, or CombineUnits sends FORM_CORPS \
             at a unit that already is one"
    );
    assert_eq!(rebuilt.game.units[&uid_for(602)].formation, 2);
    assert_eq!(rebuilt.game.units[&uid_for(603)].formation, 0);

    // A Corps is ONE unit. The escort reconstruction reads `formation_count`,
    // which stays 1 here, so nothing may be linked — the two mechanisms share
    // a word and nothing else.
    for host in [601, 602, 603, 604] {
        assert_eq!(
            rebuilt.game.units[&uid_for(host)].linked_to,
            None,
            "the merge tier must not be mistaken for an escort stack"
        );
    }
}

/// ⚠ THE SENTINEL MUST NOT READ AS STANDARD.
///
/// `GetDefenseStrength` answered −1 for the whole project's life because its
/// fallback was indistinguishable from an answer. The formation tier is the
/// same shape of risk with a worse failure: 0 is a legal tier, so a fallback
/// of 0 would silently claim every unit is standard on any build where the
/// accessor is missing — and the board would keep asking for a Corps forever
/// with nothing to show it was guessing. Only 0..=2 is a reading; the mod's
/// −1, an absent field, and anything out of range leave the board alone.
#[test]
fn an_unreadable_formation_tier_never_flattens_a_corps_to_standard() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 140,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![host_grass(3, 3)],
    }]);
    let state = StateSnapshot {
        turn: 140,
        units: vec![StateUnit {
            id: 701,
            kind: "UNIT_SWORDSMAN".to_string(),
            x: 3,
            y: 3,
            hp: 100.0,
            formation: Some(2),
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };
    let mut rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    let uid = *rebuilt
        .unit_ids
        .keys()
        .next()
        .expect("the host unit crosses into the mirror");
    assert_eq!(rebuilt.game.units[&uid].formation, 2);

    for unknown in [None, Some(-1), Some(3), Some(i32::MIN)] {
        let observed = StateUnit {
            formation: unknown,
            ..state.units[0].clone()
        };
        let unit = rebuilt.game.units.get_mut(&uid).expect("addressable");
        apply_unit_observation(
            unit,
            &observed,
            ObservedUnitProgress {
                promotions: None,
                religion: None,
            },
        );
        assert_eq!(
            rebuilt.game.units[&uid].formation, 2,
            "{unknown:?} is an unknown tier, not a claim that the Army is a \
                 plain unit"
        );
    }
}

#[test]
fn every_civvis_governor_and_promotion_round_trips_through_firaxis_ids() {
    let rules = crate::rules::Rules::embedded();
    for (governor, spec) in rules.governors.iter() {
        let host = civ6_governor_name(governor)
            .unwrap_or_else(|| panic!("{governor} needs a Firaxis Governor type"));
        assert_eq!(civvis_governor_name(host), Some(governor.as_str()));
        for promotion in spec.promotions.keys() {
            let host = civ6_governor_promotion(promotion)
                .unwrap_or_else(|| panic!("{governor}.{promotion} needs a Firaxis promotion type"));
            assert_eq!(
                civvis_governor_promotion(host),
                Some(promotion.as_str()),
                "{governor}.{promotion} must round-trip"
            );
        }
    }
}

/// The wire format is the risk here, not the arithmetic: the Lua field names and
/// the serde names have to agree or every read silently returns its sentinel and
/// the empire reconstructs as perfectly happy. This deserializes the exact shape
/// the mod emits.
#[test]
fn the_host_amenity_ledger_crosses_the_bridge_and_names_the_shortfall() {
    let city: StateCity = serde_json::from_str(
        r#"{"id":65536,"name":"Kabasa","pop":15,"x":3,"y":4,
                "amenities":3,"amenities_needed":7,"happiness":2,
                "happiness_yield_mult":-20,
                "amenities_luxuries":2,"amenities_entertainment":1,
                "amenities_civics":0,"amenities_city_states":0,
                "amenities_war_weariness":0,"amenities_bankruptcy":0}"#,
    )
    .expect("the mod's city record deserializes");
    assert_eq!(city.amenities, 3.0, "the field name must match the Lua key");
    assert_eq!(city.amenities_needed, 7.0);
    assert_eq!(city.happiness_yield_mult, -20.0);
    assert_eq!(city.amenities_luxuries, 2.0);
    assert_eq!(host_city_amenity_surplus(&city), Some(-4));

    let state = StateSnapshot {
        turn: 214,
        cities: vec![city],
        ..Default::default()
    };
    let report = host_amenity_report(&state);
    assert!(
        report.contains("net -4"),
        "the sign and size must survive: {report}"
    );
    assert!(report.contains("(1 short)"), "{report}");
    assert!(
        report.contains("host_yield_pct"),
        "the host's own figure is the whole point of the line: {report}"
    );
    assert!(report.contains("luxuries 2"), "{report}");
}

/// ⚠ A mirror built before this export must NOT read as a happy empire, and
/// UNKNOWN ARRIVES IN TWO SHAPES: absent becomes `f64::NAN` via `unknown_metric`,
/// while a host read that failed arrives as the mod's `-1`. This asserts both are
/// rejected — a reader testing only `!= -1.0` would let `NAN` through and a
/// reader testing only `< 0.0` would let it through the other way, since every
/// comparison against `NAN` is false.
#[test]
fn a_host_that_never_reported_amenities_says_nothing_rather_than_zero() {
    let silent: StateCity = serde_json::from_str(r#"{"id":65536,"name":"Kabasa","x":3,"y":4}"#)
        .expect("a pre-export city record still deserializes");
    assert!(
        silent.amenities.is_nan(),
        "an absent amenity read defaults to the unknown_metric sentinel, not zero"
    );
    assert!(silent.amenities_needed.is_nan());
    assert!(silent.happiness_yield_mult.is_nan());
    assert_eq!(host_city_amenity_surplus(&silent), None);

    let state = StateSnapshot {
        turn: 40,
        cities: vec![silent],
        ..Default::default()
    };
    assert_eq!(
        host_amenity_report(&state),
        "",
        "silence must print nothing, not a surplus of zero"
    );

    // The other shape: the host was asked and could not answer.
    let failed: StateCity = serde_json::from_str(
        r#"{"id":65536,"name":"Kabasa","x":3,"y":4,
                "amenities":-1,"amenities_needed":-1,"happiness_yield_mult":-1}"#,
    )
    .expect("a failed host read still deserializes");
    let state = StateSnapshot {
        turn: 40,
        cities: vec![failed],
        ..Default::default()
    };
    assert_eq!(
        host_amenity_report(&state),
        "",
        "the mod's -1 must be refused as firmly as an absent field"
    );
    assert_eq!(host_city_amenity_surplus(&state.cities[0]), None);
}

/// The wire format is the risk: a Lua key that does not match the serde name
/// silently returns the default and the empire reads as correctly holding zero.
#[test]
fn unspent_envoys_cross_the_bridge_and_name_the_suzerainties_we_hold() {
    let state: StateSnapshot = serde_json::from_str(
        r#"{"turn":214,"envoys_free":7,
                "minors":[
                  {"player":8,"civ":"CIVILIZATION_YEREVAN","envoys":3,"suzerain":0},
                  {"player":9,"civ":"CIVILIZATION_VILNIUS","envoys":1,"suzerain":3},
                  {"player":10,"civ":"CIVILIZATION_KABUL","envoys":0}]}"#,
    )
    .expect("the mod's state record deserializes");
    assert_eq!(
        state.envoys_free,
        Some(7),
        "the field name must match the Lua key"
    );

    let report = host_envoy_report(&state);
    assert!(report.contains("unspent 7"), "{report}");
    assert!(report.contains("placed 4"), "{report}");
    // ⚠ Exactly one: seat 0 is ours, seat 3 is a rival's, and the third
    // city-state has no suzerain at all and defaults to -1.
    assert!(
        report.contains("suzerain 1/3"),
        "an unclaimed city-state must not count as ours: {report}"
    );
}

/// ⚠ A mirror built before this export must not read as an empire correctly
/// holding no envoys — that is the instrument inventing good news.
#[test]
fn a_host_that_never_reported_envoys_says_nothing_rather_than_zero() {
    let silent: StateSnapshot =
        serde_json::from_str(r#"{"turn":40,"minors":[]}"#).expect("deserializes");
    assert_eq!(silent.envoys_free, None);
    assert_eq!(host_envoy_report(&silent), "");

    // The other shape: the host was asked and could not answer.
    let failed: StateSnapshot =
        serde_json::from_str(r#"{"turn":40,"envoys_free":-1,"minors":[]}"#).expect("deserializes");
    assert_eq!(
        host_envoy_report(&failed),
        "",
        "the mod's -1 must be refused as firmly as an absent field"
    );
}

/// ⚠ `GetHappinessNonFoodYieldModifier` is a PERCENTAGE and is NEGATIVE when the
/// empire is unhappy — first live reading was -10 and -20, not 0.90 and 0.80. The
/// original filter kept only `>= 0.0`, which discarded every real reading: an
/// instrument that drops exactly the case it exists to measure.
#[test]
fn the_host_happiness_figure_is_a_negative_percentage_and_survives_the_filter() {
    let city = |name: &str, pct: f64| -> StateCity {
        serde_json::from_str(&format!(
            r#"{{"id":1,"name":"{name}","x":1,"y":1,"amenities":2,"amenities_needed":5,
                     "happiness_yield_mult":{pct},"amenities_luxuries":2,
                     "amenities_entertainment":0}}"#
        ))
        .expect("deserializes")
    };
    let state = StateSnapshot {
        turn: 111,
        cities: vec![city("Krakow", -10.0), city("Wroclaw", -20.0)],
        ..Default::default()
    };
    let report = host_amenity_report(&state);
    assert!(
        report.contains("host_yield_pct -15%"),
        "a taxed empire must report its tax, not be filtered away: {report}"
    );
    assert!(report.contains("(2 short)"), "{report}");
}

/// The host ledger must be a planning input, not merely an observability
/// note.  At the same time, pinning the raw host value would make an Arena
/// appear to supply nothing in counterfactual scoring, so the bridge keeps
/// a delta and proves that a modeled repair still lifts the calibrated band.
#[test]
fn host_amenity_deficit_calibrates_planning_without_freezing_arena_gain() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 84,
        width: 16,
        height: 16,
        chunk: 1,
        plots: (0..16)
            .flat_map(|x| (0..16).map(move |y| host_grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 84,
        cities: vec![StateCity {
            id: 65_536,
            name: "Roma".to_string(),
            x: 7,
            y: 7,
            pop: 12,
            amenities: 0.0,
            amenities_needed: 6.0,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let mut rebuilt = rebuild_from_state(&snapshot, &state, 2, 91_001, 250, 0);
    let cid = rebuilt
        .game
        .city_at(crate::hex::offset_to_axial(7, 7))
        .expect("the reported city is mirrored");

    let before = rebuilt
        .game
        .city_amenity_surplus(&rebuilt.game.cities[&cid]);
    assert_eq!(before, -6, "the host's own deficit directs the planner");
    assert!(
        rebuilt
            .game
            .observed_city_amenity_adjustments
            .contains_key(&cid),
        "a known host ledger must not remain a diagnostic-only field"
    );
    let saved =
        serde_json::to_string(&rebuilt.game).expect("the live calibration remains save-compatible");
    let restored: crate::game::Game =
        serde_json::from_str(&saved).expect("the calibration round-trips through a save");
    assert_eq!(
        restored.city_amenity_surplus(&restored.cities[&cid]),
        -6,
        "a saved mirror must not forget the host's current happiness band"
    );

    let site = rebuilt.game.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != rebuilt.game.cities[&cid].pos)
        .expect("the city has a legal neighboring plot");
    let expected_gain = (rebuilt.game.rules.districts["entertainment_complex"].amenity
        + rebuilt.game.rules.buildings["arena"].amenity)
        .round() as i64;
    {
        let city = rebuilt.game.cities.get_mut(&cid).unwrap();
        city.districts
            .insert(crate::name!("entertainment_complex"), site);
        city.buildings.push(crate::name!("arena"));
    }
    rebuilt.game.map.tiles.get_mut(&site).unwrap().district =
        Some(crate::name!("entertainment_complex"));

    let after = rebuilt
        .game
        .city_amenity_surplus(&rebuilt.game.cities[&cid]);
    assert_eq!(
        after - before,
        expected_gain,
        "the additive host correction must retain the Entertainment Complex and Arena's modeled Amenities"
    );

    let mut unavailable = state.clone();
    unavailable.cities[0].amenities = f64::NAN;
    unavailable.cities[0].amenities_needed = f64::NAN;
    let mut unmapped = Vec::new();
    apply_observed_city_economy(&mut rebuilt.game, &unavailable, None, &mut unmapped);
    assert!(
        !rebuilt
            .game
            .observed_city_amenity_adjustments
            .contains_key(&cid),
        "a later unavailable host query must clear rather than preserve a stale deficit"
    );
}
