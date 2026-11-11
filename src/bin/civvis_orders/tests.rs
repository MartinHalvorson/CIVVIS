#[test]
fn startup_genome_keeps_default_and_forced_identity_without_withholds() {
    let requested: Vec<String> = include_str!("../../../deploy/live-force-on.txt")
        .trim()
        .split(',')
        .map(str::to_string)
        .collect();
    let forced = super::forced_live_treatments(&requested).expect("compiled force bundle");
    for selected in [&[][..], forced.as_slice()] {
        let mut ai = civvis::ai::AdvancedAi::new();
        super::configure_live_bridge(&mut ai, selected, &[]).expect("valid default arm");
        let reported = super::configured_live_treatments(selected, &[]);
        assert_eq!(
            reported,
            civvis::ai::gene_ledger::deployment_treatments_with_forced_live(selected),
            "default startup identity stays in the same registry order"
        );
        assert!(ai.counter_in_lane && reported.contains(&"counter-in-lane"));
        assert!(ai.parallel_settlers && reported.contains(&"parallel-settlers"));
    }
}

#[test]
fn royal_society_contribution_reaches_the_native_command_at_its_intended_district() {
    use civvis::mirror::StateDistrict;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 150,
        width: 8,
        height: 8,
        chunk: 1,
        plots: (0..8)
            .flat_map(|x| (0..8).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 150,
        cities: vec![StateCity {
            id: 65536,
            x: 2,
            y: 2,
            pop: 8,
            capital: true,
            producing: Some("PROJECT_LAUNCH_EARTH_SATELLITE".into()),
            districts: vec![StateDistrict {
                kind: "DISTRICT_SPACEPORT".into(),
                x: 4,
                y: 4,
                complete: true,
                ..StateDistrict::default()
            }],
            ..StateCity::default()
        }],
        units: vec![StateUnit {
            id: 131073,
            kind: "UNIT_BUILDER".into(),
            x: 4,
            y: 4,
            build_charges: Some(3),
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };
    let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 3, 1, 250, 0);
    let unit = *mirror
        .civ6_of
        .iter()
        .find(|(_, id)| **id == 131073)
        .unwrap()
        .0;
    let city = mirror.cid_of[&65536];
    let action = Action::ContributeProject { unit, city };
    // The planning apply consumes the Builder. Translation needs the observed
    // native actor, rather than a unit that no longer exists in the simulation.
    mirror.game.units.remove(&unit);
    let order = translate(&action, &mirror, &state).expect("mapped contribution");
    assert_eq!(
        (order.kind, order.subject, order.verb.as_deref(), order.pos),
        (
            "unit",
            Some(131073),
            Some("CONTRIBUTE_PROJECT"),
            Some((4, 4))
        )
    );
    assert!(
        unverifiable_unit_verb("CONTRIBUTE_PROJECT"),
        "request acceptance or a missing builder is not a verified production gain"
    );
    for variant in 0..9 {
        let mut bad = state.clone();
        match variant {
            0 => bad.units.clear(),
            1 => bad.units[0].x = 5,
            2 => bad.units[0].build_charges = Some(0),
            3 => bad.units[0].kind = "UNIT_SCOUT".into(),
            4 => bad.cities[0].districts[0].pillaged = true,
            5 => bad.cities[0].districts[0].complete = false,
            6 => bad.cities[0].producing = Some("BUILDING_RESEARCH_LAB".into()),
            7 => bad.units[0].build_charges = None,
            _ => bad.units[0].build_charges = Some(-1),
        }
        assert!(
            translate(&action, &mirror, &bad).is_none(),
            "unsafe variant {variant}"
        );
    }
    assert!(translate(
        &Action::ContributeProject {
            unit,
            city: u32::MAX
        },
        &mirror,
        &state
    )
    .is_none());
}

#[test]
fn startup_genome_tracks_multiple_repeated_host_withholds_without_forcing() {
    let withheld = vec![
        "counter-in-lane".to_string(),
        "parallel-settlers".to_string(),
        "counter-in-lane".to_string(),
    ];
    let mut ai = civvis::ai::AdvancedAi::new();
    super::configure_live_bridge(&mut ai, &[], &withheld).expect("valid repeated controls");
    let reported = super::configured_live_treatments(&[], &withheld);
    assert!(!ai.counter_in_lane && !reported.contains(&"counter-in-lane"));
    assert!(!ai.parallel_settlers && !reported.contains(&"parallel-settlers"));
    assert!(ai.era_paced_expansion && reported.contains(&"era-paced-expansion"));
}

#[test]
fn startup_genome_reports_mixed_host_and_forced_repair_withholds() {
    let requested: Vec<String> = include_str!("../../../deploy/live-force-on.txt")
        .trim()
        .split(',')
        .map(str::to_string)
        .collect();
    let forced = super::forced_live_treatments(&requested).expect("compiled force bundle");
    // The bundle forces `siege-commitment` only while the ledger holds it
    // off; either way it ships, and withholding it must remove it.
    assert!(
        forced.contains(&"siege-commitment")
            || civvis::ai::gene_ledger::deployment_treatments().contains(&"siege-commitment")
    );
    let withheld = vec![
        "counter-in-lane".to_string(),
        "siege-commitment".to_string(),
        "counter-in-lane".to_string(),
    ];
    let mut ai = civvis::ai::AdvancedAi::new();
    super::configure_live_bridge(&mut ai, &forced, &withheld).expect("valid mixed arm");
    let reported = super::configured_live_treatments(&forced, &withheld);
    assert!(!ai.counter_in_lane && !reported.contains(&"counter-in-lane"));
    assert!(!ai.siege_commitment && !reported.contains(&"siege-commitment"));
    assert!(ai.parallel_settlers && reported.contains(&"parallel-settlers"));
}

#[test]
fn startup_genome_reports_a_withheld_counter_response_under_the_force_bundle() {
    let requested: Vec<String> = include_str!("../../../deploy/live-force-on.txt")
        .trim()
        .split(',')
        .map(str::to_string)
        .collect();
    let forced = super::forced_live_treatments(&requested).expect("compiled force bundle");
    let withheld = vec!["counter-in-lane".to_string()];
    let mut ai = civvis::ai::AdvancedAi::targeting(civvis::ai::VictoryTarget::Domination);
    super::configure_live_bridge(&mut ai, &forced, &withheld).expect("valid OFF arm");
    assert!(
        !ai.counter_in_lane,
        "the configured controller is actually OFF"
    );
    assert_eq!(
        ai.victory_target(),
        Some(civvis::ai::VictoryTarget::Domination)
    );
    assert!(
        !super::configured_live_treatments(&forced, &withheld).contains(&"counter-in-lane"),
        "startup identity must describe the configured OFF controller"
    );
}

/// ★★★★★ EVERY ITEM THE BOARD CAN ORDER COMES BACK FROM ITS HOST SPELLING
/// TO THE SAME KEY. `Game::can_produce` gates on the host's exported menu,
/// translated through `mirror::host_production_key`; an orderable item
/// whose outbound name did not translate back would be gated off every
/// live board forever. Checked for the names the shipped game has
/// (`data/civ6_type_names.json`): a CIVVIS-only item can be on no host
/// menu and can be ordered nowhere either.
#[test]
fn every_orderable_item_round_trips_through_the_host_menu_key() {
    use civvis::game::Item;
    let rules = civvis::rules::Rules::embedded();
    let shipped: std::collections::BTreeSet<String> =
        serde_json::from_str(include_str!("../../../data/civ6_type_names.json"))
            .expect("the type-name snapshot is a JSON list");
    let mut items: Vec<(Item, String)> = Vec::new();
    for unit in rules.units.keys() {
        items.push((Item::Unit { unit: *unit }, format!("unit:{unit}")));
    }
    for building in rules.buildings.keys() {
        items.push((
            Item::Building {
                building: *building,
            },
            format!("building:{building}"),
        ));
    }
    for wonder in rules.wonders.keys() {
        items.push((
            Item::Wonder {
                wonder: *wonder,
                pos: (0, 0),
            },
            format!("wonder:{wonder}"),
        ));
    }
    for district in rules.districts.keys() {
        items.push((
            Item::District {
                district: *district,
                pos: (0, 0),
            },
            format!("district:{district}"),
        ));
    }
    for project in rules.projects.keys() {
        items.push((
            Item::Project { project: *project },
            format!("project:{project}"),
        ));
    }
    let mut broken = Vec::new();
    let mut checked = 0;
    for (item, key) in items {
        let Some(host) = super::civ6_build_name(&item) else {
            continue;
        };
        if !shipped.contains(&host) {
            continue;
        }
        checked += 1;
        let back = civvis::mirror::host_production_key(&rules, &host, None);
        if back.as_deref() != Some(key.as_str()) {
            broken.push(format!("{host} -> {back:?}, the gate wants {key}"));
        }
    }
    assert!(
        checked > 200,
        "only {checked} orderable names are in the snapshot"
    );
    assert!(
        broken.is_empty(),
        "host spellings that do not come back to the board's key:\n{}",
        broken.join("\n")
    );
}

/// The lane list is the enum's, in the enum's order, with `civvis` in front.
///
/// ⚠ The usage line and the Python launchers' `choices` are both generated
/// from this const, so it is the one place the six names are written. If a
/// seventh `VictoryTarget` is added this fails until the const names it, and
/// `test_civ6_play.py` then fails until the launchers do.
#[test]
fn the_usage_line_offers_exactly_the_targets_the_engine_implements() {
    let expected: Vec<&str> = std::iter::once("civvis")
        .chain(
            civvis::ai::VictoryTarget::ALL
                .iter()
                .map(|target| target.as_str()),
        )
        .collect();
    assert_eq!(
        super::VICTORY_LANES.split('|').collect::<Vec<_>>(),
        expected
    );
}

/// A bare `civvis_orders` must resolve the launcher chain's lane to a real
/// target — not merely name it. Three of the six spellings resolved to
/// nothing until 2026-08-17, which is why this asserts the resolution and
/// not just the string.
///
#[test]
fn direct_default_uses_the_safe_launcher_lane() {
    assert_eq!(super::DEFAULT_VICTORY, "science");
    assert!(super::VICTORY_LANES
        .split('|')
        .any(|lane| lane == super::DEFAULT_VICTORY));
    assert_eq!(
        super::victory_lane(super::DEFAULT_VICTORY).and_then(|ai| ai.victory_target()),
        Some(civvis::ai::VictoryTarget::Science)
    );
}

/// ⚠⚠ THREE OF THESE SIX RESOLVED TO NOTHING UNTIL 2026-08-17. The live seat
/// matched four names by hand while `VictoryTarget` had six, so Culture,
/// Religion and Diplomacy exited with code 2 — and because `advanced.rs`
/// gates each lane's own machinery on being targeted at it, no other setting
/// could reach them either.
#[test]
fn every_named_lane_builds_the_agent_it_names() {
    for target in civvis::ai::VictoryTarget::ALL {
        let ai = super::victory_lane(target.as_str())
            .unwrap_or_else(|| panic!("{} is in the usage line", target.as_str()));
        assert_eq!(ai.victory_target(), Some(target));
    }
    // The aliases `VictoryTarget::from_str` already accepted now work on the
    // command line as well, so a run asking for `religion` is not refused.
    for (spelling, target) in [
        ("religion", civvis::ai::VictoryTarget::Religion),
        ("diplomacy", civvis::ai::VictoryTarget::Diplomacy),
        ("conquest", civvis::ai::VictoryTarget::Domination),
    ] {
        assert_eq!(
            super::victory_lane(spelling).and_then(|ai| ai.victory_target()),
            Some(target),
            "{spelling}"
        );
    }
    // `civvis` is the absence of a target, not one of them.
    assert_eq!(
        super::victory_lane("civvis").and_then(|ai| ai.victory_target()),
        None
    );
    // And a name that is not a lane stays an error rather than quietly
    // becoming Domination, which is what the `--fresh-ai` copy of this match
    // used to do.
    assert!(super::victory_lane("religous").is_none());
}

/// ★★★★★ A CONTROL ARM THE LIVE HARNESS CAN ACTUALLY RUN.
///
/// Every live-bridge repair has a `live_without_*` arm in `src/elo.rs`, but
/// `ai_eval` plays headless CIVVIS where several of them cannot act at all —
/// `closed_borders` and `host_observed` are populated only by `mirror.rs`,
/// and the 20-axis composite measures +9 Elo (CI −53..+71) against plain
/// `advanced`. The live harness, meanwhile, had no way to hold a treatment
/// off: `civ6_play.py` starts `--serve` and takes whatever the binary does.
/// So the one regime where these mechanisms fire had no control arm.
///
/// ⚠ The unknown-name case is asserted deliberately. A typo that silently
/// produced a control identical to the treatment would report a null that
/// looks exactly like a real one.
#[test]
fn an_opt_in_the_ledger_turns_on_can_be_withheld_and_one_it_leaves_off_cannot() {
    // A screen's off arm for a ledger-on opt-in is `--without`; before this
    // the flag refused every opt-in, so the 30-odd opt-ins the ledger
    // deploys had no live control at all (`docs/LIVE_SCREEN.md`).
    let deployed_opt_in = civvis::ai::GENES
        .iter()
        .find(|gene| {
            gene.opt_in() && civvis::ai::gene_ledger::ledger_default_on(gene.tag) == Some(true)
        })
        .map(|gene| gene.tag);
    if let Some(tag) = deployed_opt_in {
        let mut ai = civvis::ai::AdvancedAi::new();
        ai.enable_live_bridge();
        withhold_live_treatment(&mut ai, tag)
            .expect("an opt-in the ledger turns on is a withholdable arm");
        assert!(
            withholdable_treatments()
                .split(", ")
                .any(|name| name == tag),
            "the usage line must list the arm it accepts"
        );
    }
    let held_opt_in = civvis::ai::GENES
        .iter()
        .find(|gene| {
            gene.opt_in() && civvis::ai::gene_ledger::ledger_default_on(gene.tag) != Some(true)
        })
        .map(|gene| gene.tag)
        .expect("the registry holds at least one opt-in the ledger leaves off");
    let mut ai = civvis::ai::AdvancedAi::new();
    ai.enable_live_bridge();
    let refused = withhold_live_treatment(&mut ai, held_opt_in)
        .expect_err("an opt-in already off is not an arm and must not pass silently");
    assert!(refused.contains("unknown --without treatment"), "{refused}");
    let production = civvis::ai::GENES
        .iter()
        .find(|gene| gene.production())
        .map(|gene| gene.tag)
        .expect("the registry holds production genes");
    withhold_live_treatment(&mut ai, production)
        .expect_err("a production gene has no live control and stays refused");
}

#[test]
fn a_live_treatment_can_be_withheld_by_name() {
    let mut ai = civvis::ai::AdvancedAi::new();
    // The universe, not the deployment genome: the ledger already holds
    // unproven genes off, and this test is about the withholding path.
    ai.enable_live_bridge_universe();
    assert!(
        ai.blind_objective_strength,
        "the composite must set the flag before we can meaningfully hold it off"
    );

    withhold_live_treatment(&mut ai, "blind-objective-strength")
        .expect("a registered treatment can be withheld");
    assert!(
        !ai.blind_objective_strength,
        "--without must actually clear the flag, or the control arm is the treatment"
    );

    // Holding one off must not disturb its neighbours.
    assert!(
        ai.blind_objective_units,
        "withholding one treatment must leave the rest of the composite intact"
    );

    assert!(ai.amenity_project_preemption);
    withhold_live_treatment(&mut ai, "amenity-project-preemption")
        .expect("the amenity repair's control arm is registered");
    assert!(
        !ai.amenity_project_preemption,
        "the named Amenity control must hold the queue handoff off"
    );

    assert!(ai.live_wonder_race);
    withhold_live_treatment(&mut ai, "live-wonder-race")
        .expect("the wonder race's control arm is registered");
    assert!(
        !ai.live_wonder_race,
        "the named wonder-race control must hold it off"
    );

    assert!(ai.amenity_district_path);
    withhold_live_treatment(&mut ai, "amenity-district-path")
        .expect("the amenity path's control arm is registered");
    assert!(
        !ai.amenity_district_path,
        "the named amenity-path control must hold it off"
    );

    assert!(ai.expansion_before_prophet);
    withhold_live_treatment(&mut ai, "expansion-before-prophet")
        .expect("the Prophet deferral's control arm is registered");
    assert!(
        !ai.expansion_before_prophet,
        "the named Prophet-deferral control must hold it off"
    );

    assert!(ai.no_elective_war);
    withhold_live_treatment(&mut ai, "no-elective-war")
        .expect("the elective-war stand-down's control arm is registered");
    assert!(
        !ai.no_elective_war,
        "the named elective-war control must hold it off"
    );

    assert!(ai.fog_land_capacity);
    withhold_live_treatment(&mut ai, "fog-land-capacity")
        .expect("the fogged-capacity control arm is registered");
    assert!(
        !ai.fog_land_capacity,
        "the named fogged-capacity control must hold it off"
    );

    assert!(ai.one_launch_pad);
    withhold_live_treatment(&mut ai, "one-launch-pad")
        .expect("the one-launch-pad control arm is registered");
    assert!(
        !ai.one_launch_pad,
        "the named one-launch-pad control must hold it off"
    );
    assert!(ai.naval_recon());
    withhold_live_treatment(&mut ai, "naval-recon")
        .expect("the naval-recon control arm is registered");
    assert!(
        !ai.naval_recon(),
        "the named naval-recon control must hold it off"
    );

    assert!(ai.counter_in_lane);
    withhold_live_treatment(&mut ai, "counter-in-lane")
        .expect("the counter-in-lane control arm is registered");
    assert!(
        !ai.counter_in_lane,
        "the named counter-in-lane control must hold it off"
    );
    assert!(ai.era_paced_expansion);
    withhold_live_treatment(&mut ai, "era-paced-expansion")
        .expect("the era-paced-expansion control arm is registered");
    assert!(
        !ai.era_paced_expansion,
        "the named era-pace control must hold it off"
    );

    assert!(ai.tally_culture);
    withhold_live_treatment(&mut ai, "tally-culture")
        .expect("the tally-culture control arm is registered");
    assert!(
        !ai.tally_culture,
        "the named tally-culture control must hold it off"
    );
    assert!(ai.culture_building_debt);
    withhold_live_treatment(&mut ai, "culture-building-debt")
        .expect("the culture-building-debt control arm is registered");
    assert!(
        !ai.culture_building_debt,
        "the named culture-building-debt control must hold it off"
    );
    assert!(ai.frontier_loyalty);
    withhold_live_treatment(&mut ai, "frontier-loyalty")
        .expect("the frontier-loyalty control arm is registered");
    assert!(
        !ai.frontier_loyalty,
        "the named frontier-loyalty control must hold it off"
    );

    assert!(ai.settler_target_hysteresis);
    withhold_live_treatment(&mut ai, "settler-target-hysteresis")
        .expect("the settler-target-hysteresis control arm is registered");
    assert!(
        !ai.settler_target_hysteresis,
        "the named settler-target-hysteresis control must hold it off"
    );

    assert!(ai.tally_great_people);
    withhold_live_treatment(&mut ai, "tally-great-people")
        .expect("the tally-great-people control arm is registered");
    assert!(
        !ai.tally_great_people,
        "the named tally-great-people control must hold it off"
    );

    assert!(ai.barbarian_scouts_are_scouts);
    withhold_live_treatment(&mut ai, "barbarian-scouts-are-scouts")
        .expect("the barbarian-scout control arm is registered");
    assert!(
        !ai.barbarian_scouts_are_scouts,
        "the named barbarian-scout control must hold it off"
    );

    assert!(ai.camp_party());
    withhold_live_treatment(&mut ai, "camp-party")
        .expect("the camp-party control arm is registered");
    assert!(
        !ai.camp_party(),
        "the named camp-party control must hold it off"
    );

    assert!(ai.buildings_before_projects);
    withhold_live_treatment(&mut ai, "buildings-before-projects")
        .expect("the buildings-before-projects control arm is registered");
    assert!(
        !ai.buildings_before_projects,
        "the named buildings-before-projects control must hold it off"
    );

    let bad = withhold_live_treatment(&mut ai, "no-such-treatment");
    assert!(
        bad.is_err(),
        "an unknown name must be a hard error; a silent no-op would report a \
             null that is indistinguishable from a real one"
    );
    assert!(
        bad.unwrap_err().contains("come-ashore"),
        "the error must name what this binary can actually withhold"
    );
}

/// ⚠⚠ ELEVEN SHIPPED TREATMENTS HAD NO CONTROL ARM ON THE ONLY HARNESS
/// WHERE THEY FIRE, and nothing said so: this binary matched 57
/// hand-written names against a 68-row table, and its usage string was a
/// third, shorter copy again. `deny_while_targeted`,
/// `live_religious_purchase`, `live_trader_route`,
/// `loyalty_policy_defence`, `peacetime_deterrence`, `ranged_line_of_sight`,
/// `recorded_tactical_step`, `slot_kind_tiebreak` and `strike_opening` were
/// unwithholdable — including the religious-purchase repair, on the lane
/// this engine finishes fastest.
///
/// The list cannot be short again: it is the table.
#[test]
fn every_registered_live_treatment_can_be_withheld() {
    for gene in civvis::ai::GENES.iter().filter(|gene| gene.live()) {
        let mut ai = civvis::ai::AdvancedAi::new();
        ai.enable_live_bridge();
        withhold_live_treatment(&mut ai, gene.tag).unwrap_or_else(|error| {
            panic!(
                "{} is a live gene but not withholdable: {error}",
                gene.field
            )
        });
    }
}

/// And the usage line is the same list, so a name the binary accepts is
/// never one the error message hides: every live gene, and every opt-in
/// the ledger turns on (`withholdable`), in registry order.
#[test]
fn the_usage_line_names_every_treatment_the_binary_accepts() {
    let listed: Vec<String> = super::withholdable_treatments()
        .split(", ")
        .map(str::to_string)
        .collect();
    let registered: Vec<String> = civvis::ai::GENES
        .iter()
        .filter(|gene| {
            gene.live()
                || (gene.opt_in()
                    && civvis::ai::gene_ledger::ledger_default_on(gene.tag) == Some(true))
        })
        .map(|gene| gene.tag.to_string())
        .collect();
    assert_eq!(listed, registered);
    for gene in civvis::ai::GENES.iter().filter(|gene| gene.live()) {
        assert!(
            listed.contains(&gene.tag.to_string()),
            "{} is live and unlisted",
            gene.tag
        );
    }
}

/// The ledger's best genome remains the default, but a verification arm
/// needs a precise way to restore one held row and record a real contrast.
/// The host says why a step did not happen; the checks now say it back.
#[test]
fn a_refused_step_carries_the_hosts_own_reason() {
    let evidence = vec![
        serde_json::json!({
            "kind": "move_refused", "turn": 42, "unit": 7,
            "x": 3, "y": 4, "dest_impassable": true
        }),
        serde_json::json!({
            "kind": "move_refused", "turn": 42, "unit": 9,
            "x": 5, "y": 6, "dest_water": true
        }),
        serde_json::json!({
            "kind": "move_refused", "turn": 42, "unit": 11, "x": 1, "y": 1
        }),
    ];
    assert_eq!(
        super::move_refusal_reason(&evidence, 42, 7).as_deref(),
        Some("host_refused_impassable")
    );
    assert_eq!(
        super::move_refusal_reason(&evidence, 42, 9).as_deref(),
        Some("host_refused_water")
    );
    assert_eq!(
        super::move_refusal_reason(&evidence, 42, 11).as_deref(),
        Some("host_refused_move")
    );
    assert_eq!(
        super::move_refusal_reason(&evidence, 43, 7),
        None,
        "a refusal belongs to the turn it happened on"
    );
    assert_eq!(super::move_refusal_reason(&evidence, 42, 8), None);
    assert_eq!(
        super::move_refusal_reason(&[], 42, 7),
        None,
        "and a run whose mod says nothing keeps the position diff"
    );
    assert!(
        super::EVIDENCE_KINDS.contains(&"move_refused"),
        "the reader has to be asked for the kind before it can read it"
    );
    assert!(
        super::EVIDENCE_KINDS.contains(&"move_noop")
            && super::EVIDENCE_KINDS.contains(&"move_fallback"),
        "the queue's no-op answer is evidence too"
    );
    // A leg the host accepted and never walked names the host's reason
    // ahead of the plainer refusal on the same unit and turn.
    let noop = vec![
        serde_json::json!({
            "kind": "move_noop", "turn": 42, "unit": 7,
            "from": [3, 4], "want": [4, 4], "why": "zoc"
        }),
        serde_json::json!({
            "kind": "move_refused", "turn": 42, "unit": 7, "x": 4, "y": 4
        }),
        serde_json::json!({
            "kind": "move_noop", "turn": 42, "unit": 9, "from": [1, 1], "want": [2, 1]
        }),
    ];
    assert_eq!(
        super::move_refusal_reason(&noop, 42, 7).as_deref(),
        Some("host_noop_zoc")
    );
    assert_eq!(
        super::move_refusal_reason(&noop, 42, 9).as_deref(),
        Some("host_noop_unknown"),
        "a no-op with no reason is still a no-op, not a position diff"
    );
    assert!(
        super::EVIDENCE_KINDS.contains(&"settler_capture_escape"),
        "a safety rewrite must reach the postcondition verifier"
    );
}

#[test]
fn a_live_arm_forces_held_treatments_and_tolerates_deployment_members() {
    // The exact held live genes move with the deployment selection.
    let held_live = civvis::ai::gene_ledger::ledger_held_live_treatments()
        .first()
        .copied()
        .expect("the explicit genome leaves live treatments off");
    let forced = super::forced_live_treatments(&[held_live.to_string(), held_live.to_string()])
        .expect("a named ledger-held gene is a valid live arm");
    assert_eq!(forced, vec![held_live], "the arm is deduplicated");

    let other_held_live = civvis::ai::gene_ledger::ledger_held_live_treatments()
        .iter()
        .copied()
        .find(|tag| *tag != held_live)
        .expect("the explicit genome leaves more than one live treatment off");

    let mut ai = civvis::ai::AdvancedAi::new();
    super::configure_live_bridge(&mut ai, &forced, &[])
        .expect("the validated arm configures the live controller");
    assert!(
        civvis::ai::gene_ledger::deployment_treatments_with_forced_live(&forced)
            .contains(&held_live),
        "the requested gene is restored in the arm's genome"
    );
    assert!(
        !civvis::ai::gene_ledger::deployment_treatments_with_forced_live(&forced)
            .contains(&other_held_live),
        "another held gene stays off until the experiment names it"
    );

    let host_only = super::forced_live_treatments(&["parallel-settlers".to_string()])
        .expect_err("a treatment already in the live genome cannot form a force-on arm");
    assert!(host_only.contains("deployment genome"), "{host_only}");
    assert!(
        host_only.contains("host-only") && host_only.contains("--without"),
        "a host-only row says the seat already plays it and what withholds it: {host_only}"
    );
    let deployed_screenable = civvis::ai::GENES
        .iter()
        .find(|gene| !gene.host_only() && civvis::ai::ledger_default_on(gene.tag) == Some(true))
        .map(|gene| gene.tag)
        .expect("the deployment genome selects a screenable gene");
    let refused = super::forced_live_treatments(&[deployed_screenable.to_string()])
        .expect_err("a deployed screenable gene cannot form a force-on arm either");
    assert!(
        refused.contains("deployment genome") && !refused.contains("host-only"),
        "{refused}"
    );

    let deployed_opt_in = civvis::ai::GENES
        .iter()
        .find(|gene| gene.opt_in() && civvis::ai::ledger_default_on(gene.tag) == Some(true))
        .map(|gene| gene.tag)
        .expect("the deployment genome selects an opt-in alongside the held treatment");
    let mixed = super::forced_live_treatments(&[
        deployed_opt_in.to_string(),
        held_live.to_string(),
        deployed_opt_in.to_string(),
    ])
    .expect("a wider profile may repeat a gene deployment already supplies");
    assert_eq!(mixed, vec![held_live]);
    assert!(
        civvis::ai::gene_ledger::deployment_treatments().contains(&deployed_opt_in),
        "the regression needs a deployment-provided member alongside the held treatment"
    );

    // A held-off opt-in is the other thing an arm may name: it is the
    // only route a gene priced on the arena has to the live board
    // before the whole-game screen answers.
    let opt_in = civvis::ai::gene_ledger::ledger_held_opt_ins()
        .first()
        .copied()
        .expect("the registry holds opt-ins the ledger has not turned on");
    let seated = super::forced_live_treatments(&[opt_in.to_string()])
        .expect("a held-off opt-in is a valid live arm");
    assert_eq!(seated, vec![opt_in]);
    let mut seat = civvis::ai::AdvancedAi::new();
    super::configure_live_bridge(&mut seat, &seated, &[])
        .expect("the opt-in arm configures the live controller");
    assert!(
        civvis::ai::gene_ledger::deployment_treatments_with_forced_live(&seated).contains(&opt_in),
        "the arm's genome names the opt-in it seated"
    );
    let mut ordinary = civvis::ai::AdvancedAi::new();
    super::configure_live_bridge(&mut ordinary, &[], &[])
        .expect("the ordinary live seat configures");
    assert!(
        !civvis::ai::gene_ledger::deployment_treatments().contains(&opt_in),
        "and seating it changes nothing about the deployment genome"
    );
    let unknown = super::forced_live_treatments(&["no-such-treatment".to_string()])
        .expect_err("a typo cannot silently become the deployment arm");
    assert!(unknown.contains(held_live), "{unknown}");
}

use super::*;

/// See `hold_deals_beside_an_alliance`: beside an alliance ask, every
/// sale, purchase and second alliance ask waits; other orders stand, and a
/// batch with no alliance ask is untouched.
#[test]
fn an_alliance_ask_is_the_batch_s_only_deal_session() {
    let order = |kind: &'static str, subject: i64, verb: &str| Order {
        kind,
        subject: Some(subject),
        verb: Some(verb.to_string()),
        pos: None,
    };
    let mut orders = vec![
        order("sell", 3, "RESOURCE_NITER=10"),
        order("friendship", 2, "DECLARE_FRIEND"),
        order("alliance", 2, "ALLIANCE_CULTURAL"),
        order("buy", 1, "OPEN_BORDERS"),
        order("alliance", 4, "ALLIANCE_RESEARCH"),
        order("envoy", 9, "ENVOY"),
    ];
    assert_eq!(hold_deals_beside_an_alliance(&mut orders), 3);
    let left: Vec<(&str, i64)> = orders
        .iter()
        .map(|order| (order.kind, order.subject.unwrap()))
        .collect();
    assert_eq!(left, vec![("friendship", 2), ("alliance", 2), ("envoy", 9)]);
    let mut quiet = vec![
        order("sell", 3, "RESOURCE_NITER=10"),
        order("buy", 1, "OPEN_BORDERS"),
    ];
    assert_eq!(hold_deals_beside_an_alliance(&mut quiet), 0);
    assert_eq!(quiet.len(), 2);
}

/// See `drop_deals_beside_war_or_peace`: a sale to the rival we declare on
/// is dropped so the declaration gets the diplomacy session; deals with
/// anyone else, and the war itself, stand.
#[test]
fn a_deal_is_dropped_beside_a_war_or_peace_with_the_same_rival() {
    let order = |kind: &'static str, subject: i64, verb: &str| Order {
        kind,
        subject: Some(subject),
        verb: Some(verb.to_string()),
        pos: None,
    };
    let mut orders = vec![
        order("sell", 1, "RESOURCE_AMBER=1"),
        order("war", 1, "DECLARE_FORMAL_WAR"),
        order("sell", 2, "FAVOR=20"),
        order("buy", 3, "LUXURY_ANY"),
        order("peace", 3, "MAKE_PEACE"),
    ];
    assert_eq!(drop_deals_beside_war_or_peace(&mut orders), 2);
    let left: Vec<(&str, i64)> = orders
        .iter()
        .map(|order| (order.kind, order.subject.unwrap()))
        .collect();
    assert_eq!(left, vec![("war", 1), ("sell", 2), ("peace", 3)]);
    let mut quiet = vec![order("sell", 2, "FAVOR=20")];
    assert_eq!(drop_deals_beside_war_or_peace(&mut quiet), 0);
    assert_eq!(quiet.len(), 1);
}
use civvis::mirror::{
    Plot, Snapshot, StateActivationPlot, StateCity, StateDistrict, StateEmergency,
    StateEmergencyOurs, StateEmergencyScore, StateGovernor, StateGreatPerson, StateGreatWork,
    StateMinor, StateRival, StateSnapshot, StateTradeRoute, StateUnit, TilesChunk,
};

fn default_decision_arm() -> DecisionArm<'static> {
    DecisionArm {
        war_from_plan: false,
        forced_on: &[],
        withheld: &[],
    }
}

#[test]
fn host_peace_backoff_matches_firaxis_retry_window() {
    let mut retries = HostPeaceRetries::default();
    let mut state = StateSnapshot {
        turn: 58,
        rivals: vec![StateRival {
            player: 2,
            at_war: true,
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    let peace = || Order {
        kind: "peace",
        subject: Some(2),
        verb: Some("MAKE_PEACE".to_string()),
        pos: None,
    };

    let (orders, deferred) = defer_host_peace_retries(vec![peace()], &state, &mut retries);
    assert_eq!(orders.len(), 1, "the first host request is legal");
    assert!(deferred.is_empty());

    for turn in 59..63 {
        state.turn = turn;
        let (orders, deferred) = defer_host_peace_retries(vec![peace()], &state, &mut retries);
        assert!(
            orders.is_empty(),
            "turn {turn} remains inside host cooldown"
        );
        assert_eq!(deferred, vec![2]);
    }

    state.turn = 63;
    let (orders, deferred) = defer_host_peace_retries(vec![peace()], &state, &mut retries);
    assert_eq!(orders.len(), 1, "the fifth turn is legal again");
    assert!(deferred.is_empty());
}

#[test]
fn an_affordable_direct_city_state_war_peaces_before_envoys() {
    let mut state = StateSnapshot {
        turn: 58,
        envoys_free: Some(4),
        // This is the current Suzerain, but we are at peace with it.  The
        // city-state war is therefore direct and a minor peace can release it.
        rivals: vec![StateRival {
            player: 2,
            at_war: false,
            ..StateRival::default()
        }],
        minors: vec![StateMinor {
            player: 9,
            civ: "CIVILIZATION_GENEVA".to_string(),
            at_war: true,
            suzerain: 2,
            envoys: 1,
            most_envoys: 4,
            ..StateMinor::default()
        }],
        ..StateSnapshot::default()
    };
    let mut proposed = Vec::new();
    assert_eq!(
        queue_city_state_envoy_reclaim(&mut proposed, &state, None),
        Some((9, 4)),
        "four held envoys raise our one to five and take Geneva"
    );
    assert_eq!(proposed.len(), 1);
    assert_eq!(proposed[0].kind, "peace");
    assert_eq!(proposed[0].subject, Some(9));
    assert!(
        proposed
            .iter()
            .all(|order| !(order.kind == "envoy" && order.subject == Some(9))),
        "Firaxis cannot accept an envoy to the city-state until a later state frame"
    );

    // A direct minor peace uses the same host cooldown as a major.  Without
    // this the next bridge frame would re-submit an operation Firaxis has
    // already accepted but not yet reflected.
    let mut retries = HostPeaceRetries::default();
    let (sent, deferred) = defer_host_peace_retries(proposed, &state, &mut retries);
    assert_eq!(sent.len(), 1);
    assert!(deferred.is_empty());

    state.turn = 59;
    let mut retry = Vec::new();
    assert_eq!(
        queue_city_state_envoy_reclaim(&mut retry, &state, None),
        Some((9, 4))
    );
    let (sent, deferred) = defer_host_peace_retries(retry, &state, &mut retries);
    assert!(sent.is_empty(), "the host has not confirmed peace yet");
    assert_eq!(deferred, vec![9]);

    // Once a fresh export says the war ended, retry memory clears and the
    // normal live planner may legally choose its envoy placements.
    state.turn = 60;
    state.minors[0].at_war = false;
    let _ = defer_host_peace_retries(Vec::new(), &state, &mut retries);
    assert!(retries.permits(9, state.turn));
}

#[test]
fn submitted_city_state_reclaim_reserves_only_the_suzerainty_cost() {
    let envoy = |subject| Order {
        kind: "envoy",
        subject: Some(subject),
        verb: Some("GIVE_INFLUENCE_TOKEN".to_string()),
        pos: None,
    };
    let mut orders = vec![
        Order {
            kind: "peace",
            subject: Some(9),
            verb: Some("MAKE_PEACE".to_string()),
            pos: None,
        },
        envoy(7),
        // The planner should never make this during war, but preserving it
        // would defeat the host-boundary guarantee the reserve provides.
        envoy(9),
        envoy(8),
        envoy(10),
    ];
    assert_eq!(
        reserve_envoys_for_submitted_reclaim(&mut orders, 9, 5, 3),
        2,
        "one invalid target and one generic surplus order wait for the next frame"
    );
    let envoys: Vec<i64> = orders
        .iter()
        .filter(|order| order.kind == "envoy")
        .filter_map(|order| order.subject)
        .collect();
    assert_eq!(envoys, vec![7, 8], "two surplus envoys still invest now");
    assert!(
        orders
            .iter()
            .any(|order| order.kind == "peace" && order.subject == Some(9)),
        "the submitted peace itself remains intact"
    );
}

#[test]
fn city_state_envoy_reclaim_skips_derived_wars_and_short_pools() {
    let mut state = StateSnapshot {
        turn: 58,
        envoys_free: Some(3),
        rivals: vec![StateRival {
            player: 2,
            at_war: false,
            ..StateRival::default()
        }],
        minors: vec![StateMinor {
            player: 9,
            civ: "CIVILIZATION_GENEVA".to_string(),
            at_war: true,
            suzerain: 2,
            envoys: 1,
            most_envoys: 4,
            ..StateMinor::default()
        }],
        ..StateSnapshot::default()
    };
    let mut proposed = Vec::new();
    assert_eq!(
        queue_city_state_envoy_reclaim(&mut proposed, &state, None),
        None,
        "do not end a war when the pool still cannot win the city-state"
    );
    assert!(proposed.is_empty());

    state.envoys_free = Some(4);
    state.rivals[0].at_war = true;
    assert_eq!(
        queue_city_state_envoy_reclaim(&mut proposed, &state, None),
        None,
        "a city-state follows its hostile Suzerain; direct peace would not unlock envoys"
    );
    assert!(proposed.is_empty());
}

/// `city-state-before-the-first-capital`: the city-state the early conquest
/// opening declared on is never traded back for Envoys while the opening
/// stands, even with the pool to take its suzerainty; any other direct
/// city-state war still is.
#[test]
fn the_conquest_openings_city_state_war_is_not_reclaimed_for_envoys() {
    // The opening's city, in the mirror's axial coordinates.
    let opening_tile = (14, 12);
    let (x, y) = civvis::hex::axial_to_offset(opening_tile.0, opening_tile.1);
    let minor = |player: usize, civ: &str, city: Option<(i32, i32)>| StateMinor {
        player,
        civ: civ.to_string(),
        at_war: true,
        envoys: 0,
        most_envoys: 0,
        cities: city
            .map(|(x, y)| {
                vec![StateCity {
                    x,
                    y,
                    ..StateCity::default()
                }]
            })
            .unwrap_or_default(),
        ..StateMinor::default()
    };
    let mut state = StateSnapshot {
        turn: 41,
        envoys_free: Some(6),
        minors: vec![
            minor(9, "CIVILIZATION_GENEVA", Some((x, y))),
            minor(10, "CIVILIZATION_HATTUSA", Some((x + 5, y))),
        ],
        ..StateSnapshot::default()
    };
    let spared = city_state_opening_host_minor(&state, opening_tile);
    assert_eq!(spared, Some(9), "the opening's tile names Geneva");

    let mut proposed = Vec::new();
    assert_eq!(
        queue_city_state_envoy_reclaim(&mut proposed, &state, spared),
        Some((10, 3)),
        "the other direct city-state war is still reclaimed"
    );
    assert!(
        proposed
            .iter()
            .all(|order| !(order.kind == "peace" && order.subject == Some(9))),
        "no peace for the opening's own target"
    );

    // With only the opening's target at war, nothing is reclaimed at all.
    state.minors.truncate(1);
    let mut proposed = Vec::new();
    assert_eq!(
        queue_city_state_envoy_reclaim(&mut proposed, &state, spared),
        None
    );
    assert!(proposed.is_empty());
    // Off (no opening), the shipped reclaim makes that peace.
    assert_eq!(
        queue_city_state_envoy_reclaim(&mut proposed, &state, None),
        Some((9, 3))
    );
    assert_eq!(proposed[0].kind, "peace");
    assert_eq!(proposed[0].subject, Some(9));
}

#[test]
fn planned_major_peace_reserves_envoys_to_reclaim_its_derived_city_state() {
    let state = StateSnapshot {
        turn: 58,
        envoys_free: Some(5),
        rivals: vec![StateRival {
            player: 2,
            at_war: true,
            ..StateRival::default()
        }],
        minors: vec![StateMinor {
            player: 9,
            civ: "CIVILIZATION_GENEVA".to_string(),
            at_war: true,
            suzerain: 2,
            envoys: 1,
            most_envoys: 4,
            ..StateMinor::default()
        }],
        ..StateSnapshot::default()
    };
    let peace = Order {
        kind: "peace",
        subject: Some(2),
        verb: Some("MAKE_PEACE".to_string()),
        pos: None,
    };
    let envoy = |subject| Order {
        kind: "envoy",
        subject: Some(subject),
        verb: Some("GIVE_INFLUENCE_TOKEN".to_string()),
        pos: None,
    };
    let mut proposed = vec![peace, envoy(7), envoy(9)];

    assert_eq!(
        planned_suzerain_peace_envoy_reclaim(&proposed, &state),
        Some((2, 9, 4)),
        "the existing peace offer releases Geneva only after it succeeds"
    );
    let mut retries = HostPeaceRetries::default();
    let (submitted, deferred_peace) = defer_host_peace_retries(proposed, &state, &mut retries);
    assert!(deferred_peace.is_empty());
    proposed = submitted;
    assert_eq!(
        reserve_envoys_for_submitted_reclaim(&mut proposed, 9, 5, 4),
        1,
        "the invalid envoy to the still-hostile minor waits with the four-token claim"
    );
    let retained: Vec<i64> = proposed
        .iter()
        .filter(|order| order.kind == "envoy")
        .filter_map(|order| order.subject)
        .collect();
    assert_eq!(
        retained,
        vec![7],
        "the one surplus envoy may still invest now"
    );
    assert!(
            proposed
                .iter()
                .any(|order| order.kind == "peace" && order.subject == Some(2)),
            "the planner's major peace, rather than a fabricated city-state peace, crosses the host boundary"
        );

    let no_peace = vec![envoy(7)];
    assert_eq!(
        planned_suzerain_peace_envoy_reclaim(&no_peace, &state),
        None,
        "a valuable city-state alone never asks a major to end a campaign"
    );
    let conflicting_orders = vec![
        Order {
            kind: "peace",
            subject: Some(2),
            verb: Some("MAKE_PEACE".to_string()),
            pos: None,
        },
        Order {
            kind: "war",
            subject: Some(2),
            verb: Some("DECLARE".to_string()),
            pos: None,
        },
    ];
    assert_eq!(
        planned_suzerain_peace_envoy_reclaim(&conflicting_orders, &state),
        None,
        "a contradictory order set cannot turn an Envoy reserve into a war-ending decision"
    );
}

#[test]
fn direct_city_state_reclaim_crosses_the_host_boundary_as_peace_only() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 58,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 58,
        military: 100.0,
        envoys_free: Some(4),
        cities: vec![StateCity {
            id: 65_536,
            name: "Rome".to_string(),
            x: 2,
            y: 2,
            pop: 5,
            capital: true,
            ..StateCity::default()
        }],
        rivals: vec![StateRival {
            player: 2,
            at_war: false,
            ..StateRival::default()
        }],
        minors: vec![StateMinor {
            player: 9,
            civ: "CIVILIZATION_GENEVA".to_string(),
            at_war: true,
            military: 1.0,
            suzerain: 2,
            envoys: 1,
            most_envoys: 4,
            cities: vec![StateCity {
                id: 65_537,
                name: "Geneva".to_string(),
                x: 8,
                y: 8,
                pop: 3,
                capital: true,
                ..StateCity::default()
            }],
            ..StateMinor::default()
        }],
        ..StateSnapshot::default()
    };
    let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 3, 1, 250, 0);
    let mut ai = civvis::ai::AdvancedAi::new();
    let mut ours = std::collections::BTreeMap::new();
    let reply: serde_json::Value = serde_json::from_str(&decide(
        &mut mirror,
        &mut ai,
        &snapshot,
        &state,
        default_decision_arm(),
        DecisionMemory {
            ours: &mut ours,
            host_peace_retries: &mut HostPeaceRetries::default(),
            host_move_refusals: &mut HostMoveRefusals::default(),
            host_order_refusals: &mut HostOrderRefusals::default(),
            air_assault_continuation: &mut air_assault_continuation::Continuation::default(),
            governor_exchange: &mut governor_exchange::Continuation::default(),
        },
    ))
    .expect("the decision is JSON");
    let orders = reply["orders"].as_array().expect("orders are an array");
    assert!(
        orders
            .iter()
            .any(|order| { order["kind"] == "peace" && order["subject"] == 9 }),
        "an affordable direct city-state war must request peace: {reply}"
    );
    assert!(
        orders
            .iter()
            .all(|order| { !(order["kind"] == "envoy" && order["subject"] == 9) }),
        "the same host batch must not try to envoy a still-warring city-state: {reply}"
    );
    assert!(
        reply["note"]
            .as_str()
            .is_some_and(|note| note.contains("envoy_reclaim_peace=9 needed=4")),
        "the bridge must identify this intentional peace-to-envoy handoff: {reply}"
    );

    // The host resolves the peace asynchronously.  The first later state
    // that says it succeeded is where the ordinary envoy planner regains
    // legal access to Geneva and spends the prepared pool.
    let mut confirmed_state = state.clone();
    confirmed_state.turn = 59;
    confirmed_state.minors[0].at_war = false;
    let mut confirmed_mirror =
        civvis::mirror::LiveMirror::new(&snapshot, &confirmed_state, 3, 1, 250, 0);
    let mut confirmed_ai = civvis::ai::AdvancedAi::new();
    let mut confirmed_ours = std::collections::BTreeMap::new();
    let confirmed: serde_json::Value = serde_json::from_str(&decide(
        &mut confirmed_mirror,
        &mut confirmed_ai,
        &snapshot,
        &confirmed_state,
        default_decision_arm(),
        DecisionMemory {
            ours: &mut confirmed_ours,
            host_peace_retries: &mut HostPeaceRetries::default(),
            host_move_refusals: &mut HostMoveRefusals::default(),
            host_order_refusals: &mut HostOrderRefusals::default(),
            air_assault_continuation: &mut air_assault_continuation::Continuation::default(),
            governor_exchange: &mut governor_exchange::Continuation::default(),
        },
    ))
    .expect("the confirmed decision is JSON");
    let envoys = confirmed["orders"]
        .as_array()
        .expect("confirmed orders are an array")
        .iter()
        .filter(|order| order["kind"] == "envoy" && order["subject"] == 9)
        .count();
    assert_eq!(
        envoys, 4,
        "the confirmed peace frame spends the full suzerainty pool: {confirmed}"
    );
}

/// Live King 20260930T211803Z: 86 refused air strikes were a bomber that
/// had already flown that turn, re-ordered on a replan frame whose export
/// still read it unspent. A sortie the host refused does not spend it.
#[test]
fn an_aircraft_that_flew_on_an_earlier_frame_is_spent_for_the_turn() {
    let (snapshot, mut state) = production_board();
    state.turn = 40;
    state.units.push(civvis::mirror::StateUnit {
        id: 4242,
        kind: "UNIT_BOMBER".to_string(),
        x: state.cities[0].x,
        y: state.cities[0].y,
        hp: 100.0,
        moves: 10.0,
        ..civvis::mirror::StateUnit::default()
    });
    let sortie = |x, y| IssuedOrder {
        kind: "unit".to_string(),
        subject: Some(4242),
        verb: Some("AIR_ATTACK".to_string()),
        pos: Some((x, y)),
    };
    let spent = |mirror: &civvis::mirror::LiveMirror| {
        let uid = mirror.uid_of[&4242];
        let unit = &mirror.game.units[&uid];
        unit.attacks_left == 0 && unit.moves_left <= 0.0
    };
    let mut strikes = HostAirStrikes::default();
    strikes.observe(40, &[sortie(6, 6)]);
    let mut replan = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    assert!(!spent(&replan), "precondition: the export reads it fresh");
    strikes.apply(&mut replan, 40);
    assert!(spent(&replan), "the frame-0 sortie spent it");

    // The host refused that sortie: the bomber may still fly elsewhere.
    state.refused_strikes.insert((4242, 6, 6));
    let mut refused = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    strikes.apply(&mut refused, 40);
    assert!(!spent(&refused), "a refused sortie spends nothing");

    // The next turn starts fresh.
    state.refused_strikes.clear();
    let mut next = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    strikes.apply(&mut next, 41);
    assert!(!spent(&next));
}

/// Run civvis-20260826T184456Z: every one of the 31 refused city strikes
/// was a replan-frame re-issue by a city whose frame-0 strike had landed.
/// The export carries no per-city "attacks remaining", so the decider's
/// own earlier frames are the record, and every board built for the turn
/// must read it.
#[test]
fn a_city_that_fired_on_an_earlier_frame_holds_its_strike_for_the_turn() {
    let (snapshot, mut state) = production_board();
    state.turn = 40;
    let city = &mut state.cities[0];
    city.producing = None;
    city.buildings = vec!["BUILDING_WALLS".to_string()];
    city.damage = 0.0;
    city.max_damage = 200.0;
    city.wall_damage = 0.0;
    city.max_wall_damage = 100.0;
    let host_city = state.cities[0].id;
    let strike = |kind: &str| IssuedOrder {
        kind: kind.to_string(),
        subject: Some(host_city),
        verb: None,
        pos: Some((5, 6)),
    };
    let can_strike = |mirror: &civvis::mirror::LiveMirror| {
        let city = mirror.cid_of[&host_city];
        mirror.game.city_can_strike(&mirror.game.cities[&city])
    };

    let mut strikes = HostCityStrikes::default();
    strikes.observe(40, &[strike("city_strike")]);

    let mut replan = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    assert!(
        can_strike(&replan),
        "precondition: a walled city on a fresh board may strike"
    );
    strikes.apply(&mut replan, 40);
    assert!(
        !can_strike(&replan),
        "the frame-0 strike is spent on the replan frame's board"
    );
    let city = replan.cid_of[&host_city];
    assert_eq!(replan.game.cities[&city].extra_strikes_used, 0);
    assert!(
        !replan.game.cities[&city].encampment_struck,
        "a city strike does not spend the encampment's"
    );

    // A second strike on the same turn (Victor's Embrasure) is counted so
    // `city_can_strike` reads the extra-strike rule exactly as it would
    // for strikes applied on the board itself.
    strikes.observe(40, &[strike("city_strike"), strike("encampment_strike")]);
    let mut third = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    strikes.apply(&mut third, 40);
    let city = third.cid_of[&host_city];
    assert_eq!(third.game.cities[&city].extra_strikes_used, 1);
    assert!(third.game.cities[&city].encampment_struck);

    // The next turn's board starts with the strike available again, and
    // observing the new turn forgets the old one.
    let mut next = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    strikes.apply(&mut next, 41);
    assert!(
        can_strike(&next),
        "a board built for the next turn is untouched"
    );
    strikes.observe(41, &[]);
    let mut fresh = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    strikes.apply(&mut fresh, 40);
    assert!(can_strike(&fresh), "the old turn's strikes are forgotten");
}

#[test]
fn fresh_board_retains_firaxis_city_repair_cooldown_after_damage() {
    let (snapshot, mut state) = production_board();
    state.turn = 40;
    let city = &mut state.cities[0];
    city.producing = None;
    city.buildings = vec!["BUILDING_WALLS".to_string()];
    city.damage = 0.0;
    city.max_damage = 200.0;
    city.wall_damage = 0.0;
    city.max_wall_damage = 100.0;

    let mut cooldowns = HostCityAttackCooldowns::default();
    cooldowns.observe(&state);

    // This mirrors Ravenna on the measured t125 frame: its wall and
    // garrison both took a fresh hit, so the host's repair project cannot
    // start even though a fresh model board normally permits it.
    state.turn = 41;
    state.cities[0].damage = 18.0;
    state.cities[0].wall_damage = 68.0;
    cooldowns.observe(&state);

    let repair = civvis::game::Item::Project {
        project: civvis::name::Name::new("repair_outer_defenses"),
    };
    let unchecked = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let city = unchecked.cid_of[&7];
    assert!(
        unchecked.game.can_produce(0, city, &repair),
        "precondition: a fresh board alone loses the host hit timestamp"
    );

    let mut tracked = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    cooldowns.apply(&mut tracked);
    let city = tracked.cid_of[&7];
    assert_eq!(tracked.game.cities[&city].last_attacked, 41);
    assert!(
        !tracked.game.can_produce(0, city, &repair),
        "the host cooldown must prevent the repair Civ 6 would reject"
    );

    // No further damage leaves the last-hit turn intact. Firaxis permits
    // the repair again on the third later turn, matching `Game::can_produce`.
    for turn in 42..=44 {
        state.turn = turn;
        cooldowns.observe(&state);
    }
    let mut cooled = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    cooldowns.apply(&mut cooled);
    let city = cooled.cid_of[&7];
    assert_eq!(cooled.game.cities[&city].last_attacked, 41);
    assert!(
        cooled.game.can_produce(0, city, &repair),
        "the exact three-turn Firaxis cooldown must eventually expire"
    );
}

/// One land patch is enough: the override reads cities, not terrain.
/// ★★★★ A move the host accepts and never performs is a fact about the
/// ground, and it must reach the mirror's speculative frontier.
///
/// Run civvis-20260816T093036Z: the only Scout stood on (12,12) for 28
/// consecutive turns of `MOVE_TO (9,11)` with no `move_refused` and 193 plots
/// revealed by t50. The mirror kept assuming the unrevealed (9,11) traversable
/// on every rebuild, so every replan chose the same dead line. Here a scout is
/// ordered from (4,5) onto the unknown ring beyond the revealed 8×8, is still
/// standing on (4,5) next turn, and the destination stops being assumed
/// traversable — while a same-turn frame keeps the memory, a unit that DID
/// move proves nothing, and revealed terrain is never overridden.
#[test]
fn a_frontier_plot_the_host_will_not_walk_onto_stops_being_traversable() {
    let (snapshot, mut state) = production_board();
    state.turn = 13;
    state.units = vec![StateUnit {
        id: 900,
        kind: "UNIT_SCOUT".to_string(),
        x: 4,
        y: 5,
        hp: 100.0,
        moves: 3.0,
        ..StateUnit::default()
    }];
    // (8,5) is the first unrevealed column past the 8×8 board: frontier ground.
    let dead_dest = (8, 5);
    let dead_pos = civvis::hex::offset_to_axial(dead_dest.0, dead_dest.1);
    let seen_pos = civvis::hex::offset_to_axial(6, 5);

    let mut refusals = HostMoveRefusals::default();
    let orders = vec![
        Order {
            kind: "unit",
            subject: Some(900),
            verb: Some("MOVE_TO".to_string()),
            pos: Some(dead_dest),
        },
        // A non-move for the same unit and a move for an unknown unit are noise.
        Order {
            kind: "unit",
            subject: Some(900),
            verb: Some("FORTIFY".to_string()),
            pos: None,
        },
        Order {
            kind: "unit",
            subject: Some(901),
            verb: Some("MOVE_TO".to_string()),
            pos: Some((7, 7)),
        },
    ];
    refusals.record(&orders, &state, &std::collections::BTreeMap::new());
    assert_eq!(
        refusals.sent.len(),
        1,
        "only the scout's move is remembered"
    );

    // A second frame on the SAME turn cannot judge the move yet and must not
    // forget it.
    refusals.observe(&state, &[]);
    assert_eq!(refusals.sent.len(), 1, "a same-turn frame keeps the memory");
    assert!(refusals.dead.is_empty());

    // Next turn, still on the tile it was ordered from: the destination is dead.
    state.turn = 14;
    refusals.observe(&state, &[]);
    assert!(refusals.sent.is_empty(), "a judged move is consumed");
    assert_eq!(
        refusals.dead.iter().copied().collect::<Vec<_>>(),
        vec![dead_dest],
        "a unit that never left its tile proves the destination unwalkable"
    );

    let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 2);
    assert_eq!(mirror.game.map.tiles[&dead_pos].terrain.as_str(), "unknown");
    assert!(
        mirror.game.map.tiles[&dead_pos].assumed_traversable,
        "fixture precondition: the frontier plot starts out assumed traversable"
    );
    refusals.apply(&mut mirror);
    assert!(
        !mirror.game.map.tiles[&dead_pos].assumed_traversable,
        "the proved-dead frontier plot stops being assumed traversable"
    );
    assert!(
        !mirror
            .game
            .rules
            .is_passable(&mirror.game.map.tiles[&dead_pos]),
        "and the planner's paths route around it"
    );

    // A move that DID progress proves nothing, and a revealed plot is never
    // overridden even if it once sat in the dead set.
    refusals.record(
        &[Order {
            kind: "unit",
            subject: Some(900),
            verb: Some("MOVE_TO".to_string()),
            pos: Some((6, 5)),
        }],
        &state,
        &std::collections::BTreeMap::new(),
    );
    state.turn = 15;
    state.units[0].x = 5;
    refusals.observe(&state, &[]);
    assert_eq!(
        refusals.dead.len(),
        1,
        "a unit that moved marks nothing dead"
    );
    refusals.dead.insert((6, 5));
    let before = mirror.game.map.tiles[&seen_pos].assumed_traversable;
    refusals.apply(&mut mirror);
    assert_eq!(
        mirror.game.map.tiles[&seen_pos].terrain.as_str(),
        "grassland"
    );
    assert_eq!(
        mirror.game.map.tiles[&seen_pos].assumed_traversable, before,
        "revealed terrain keeps its own truth"
    );
    assert!(mirror
        .game
        .rules
        .is_passable(&mirror.game.map.tiles[&seen_pos]));
}

/// A current host tells us when an accepted move could not start. That is
/// not a terrain observation: moving allowances, a temporary ZoC, and
/// stacked units can all produce the same answer. The live run ending at
/// turn 207 had 80 of these, so turning each one into a dead fog tile made
/// later routes less accurate rather than more accurate.
#[test]
fn a_named_transient_move_noop_does_not_retire_frontier_ground() {
    let (_snapshot, mut state) = production_board();
    state.turn = 13;
    state.units = vec![StateUnit {
        id: 900,
        kind: "UNIT_SCOUT".to_string(),
        x: 4,
        y: 5,
        hp: 100.0,
        moves: 3.0,
        ..StateUnit::default()
    }];
    let destination = (8, 5);
    let mut refusals = HostMoveRefusals::default();
    let move_to = || Order {
        kind: "unit",
        subject: Some(900),
        verb: Some("MOVE_TO".to_string()),
        pos: Some(destination),
    };

    refusals.record(&[move_to()], &state, &std::collections::BTreeMap::new());
    state.turn = 14;
    refusals.observe(
        &state,
        &[Order {
            kind: "order_failed",
            subject: Some(900),
            verb: Some("unit:MOVE_TO host_noop_cannot_start".to_string()),
            pos: Some((13, -1)),
        }],
    );
    assert!(
        refusals.dead.is_empty(),
        "a temporary host operation failure must not invent impassable terrain"
    );
    assert!(
        refusals.pending_probes.is_empty(),
        "the exact probe was answered; it is not a stale long-route probe"
    );

    // A real path failure remains useful map evidence on a subsequent
    // exact probe, and a direct host impassability answer does too.
    refusals.record(&[move_to()], &state, &std::collections::BTreeMap::new());
    state.turn = 15;
    refusals.observe(
        &state,
        &[Order {
            kind: "order_failed",
            subject: Some(900),
            verb: Some("unit:MOVE_TO host_noop_no_path".to_string()),
            pos: Some((14, -1)),
        }],
    );
    assert_eq!(
        refusals.dead.iter().copied().collect::<Vec<_>>(),
        vec![destination],
        "a named no-path answer still retires the exact frontier step"
    );
    assert!(HostMoveRefusals::exact_probe_proves_dead(
        &[Order {
            kind: "order_failed",
            subject: Some(900),
            verb: Some("unit:MOVE_TO host_refused_impassable".to_string()),
            pos: Some((15, -1)),
        }],
        15,
        900,
    ));
}

/// A coalesced `MOVE_TO` keeps the host fast, but the failure feedback must
/// verify the first speculative hop that was hidden inside it. Otherwise a
/// rejected three-hex Scout route either retires an unproven valid tile or
/// keeps choosing the same blocked opening again.
#[test]
fn a_stalled_coalesced_walk_probes_then_retires_its_first_unknown_step() {
    let (snapshot, mut state) = production_board();
    state.turn = 13;
    state.units = vec![StateUnit {
        id: 900,
        kind: "UNIT_SCOUT".to_string(),
        x: 6,
        y: 5,
        hp: 100.0,
        moves: 3.0,
        ..StateUnit::default()
    }];
    // The first local step is known ground. The second is the first unknown
    // frontier plot, then the coalescer carries the Scout a further hex.
    let planned = vec![
        unit_order(900, "MOVE_TO", Some((7, 5))),
        unit_order(900, "MOVE_TO", Some((8, 5))),
        unit_order(900, "MOVE_TO", Some((9, 5))),
    ];
    let probes = first_unknown_coalesced_steps(&planned, &snapshot);
    assert_eq!(probes.get(&900), Some(&(8, 5)));

    let (orders, deferred, coalesced) = coalesce_unit_paths(planned, false);
    assert_eq!(deferred, 0);
    assert_eq!(coalesced, 2);
    assert_eq!(
        orders[0].pos,
        Some((9, 5)),
        "the host still gets the full walk"
    );

    let mut refusals = HostMoveRefusals::default();
    refusals.record(&orders, &state, &probes);
    state.turn = 14;
    refusals.observe(&state, &[]);
    assert!(
        refusals.dead.is_empty(),
        "a long route has not proved which unknown hop blocked it"
    );
    assert_eq!(
        refusals
            .pending_probes
            .get(&900)
            .map(|probe| (probe.from, probe.target)),
        Some(((6, 5), (8, 5))),
        "the next outgoing move must test the first unknown hop exactly"
    );

    // The next plan may again want the distant destination, but the bridge
    // reduces just this known-failed route to its exact frontier probe.
    let mut retry = vec![unit_order(900, "MOVE_TO", Some((9, 5)))];
    assert_eq!(
        refusals.cap_pending_frontier_moves(&mut retry, &state, &probes),
        1
    );
    assert_eq!(retry[0].pos, Some((8, 5)));
    refusals.record(&retry, &state, &std::collections::BTreeMap::new());
    state.turn = 15;
    refusals.observe(&state, &[]);
    assert_eq!(
        refusals.dead.iter().copied().collect::<Vec<_>>(),
        vec![(8, 5)],
        "only a failed exact probe retires the hidden unknown hop"
    );

    // A later replan with another frontier opening must remain its own
    // decision; stale feedback may narrow a repeated route, never replace
    // a new tactical direction.
    refusals.pending_probes.insert(
        900,
        HostFrontierProbe {
            from: (6, 5),
            target: (8, 5),
        },
    );
    let mut different_route = vec![unit_order(900, "MOVE_TO", Some((7, 6)))];
    let different_steps = std::collections::BTreeMap::from([(900, (7, 6))]);
    assert_eq!(
        refusals.cap_pending_frontier_moves(&mut different_route, &state, &different_steps),
        0
    );
    assert_eq!(different_route[0].pos, Some((7, 6)));
    assert!(
        refusals.pending_probes.is_empty(),
        "a changed route clears the stale probe instead of being overwritten"
    );

    let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 2);
    let first_unknown = civvis::hex::offset_to_axial(8, 5);
    let host_destination = civvis::hex::offset_to_axial(9, 5);
    assert!(mirror.game.map.tiles[&first_unknown].assumed_traversable);
    assert!(mirror.game.map.tiles[&host_destination].assumed_traversable);
    refusals.apply(&mut mirror);
    assert!(
        !mirror.game.map.tiles[&first_unknown].assumed_traversable,
        "the next fresh mirror routes around the exact failed frontier hop"
    );
    assert!(
        mirror.game.map.tiles[&host_destination].assumed_traversable,
        "unproven downstream frontier remains available to future exploration"
    );
}

#[test]
fn a_same_turn_replan_drops_stale_unit_replays_but_keeps_new_tactics() {
    let (_snapshot, mut state) = production_board();
    state.turn = 42;
    state.frame = 0;
    let mut refusals = HostMoveRefusals::default();
    let mut ours = std::collections::BTreeMap::new();
    let mut first = vec![
        unit_order(7, "MOVE_TO", Some((3, 3))),
        unit_order(7, "FORTIFY", None),
        unit_order(8, "CAPTURE", Some((4, 4))),
    ];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut first, &state, &mut ours),
        0,
        "the first frame is allowed to issue its complete plan"
    );
    assert_eq!(first.len(), 3);

    state.frame = 1;
    let mut replan = vec![
        unit_order(7, "MOVE_TO", Some((3, 3))),
        unit_order(7, "FORTIFY", None),
        unit_order(8, "CAPTURE", Some((4, 4))),
        // A combat frame may legitimately need another strike at the same
        // target, so attack verbs are intentionally outside the filter.
        unit_order(9, "ATTACK", Some((5, 5))),
        // A changed destination is a new route decision and must survive.
        unit_order(7, "MOVE_TO", Some((3, 4))),
    ];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut replan, &state, &mut ours),
        3,
        "only exact stale MOVE_TO/CAPTURE/FORTIFY replays are removed"
    );
    assert_eq!(replan.len(), 2);
    assert!(replan.iter().any(|order| {
        order.subject == Some(9)
            && order.verb.as_deref() == Some("ATTACK")
            && order.pos == Some((5, 5))
    }));
    assert!(replan.iter().any(|order| {
        order.subject == Some(7)
            && order.verb.as_deref() == Some("MOVE_TO")
            && order.pos == Some((3, 4))
    }));

    // The suppression scope is one turn. A fresh turn may retry the old
    // destination using the newly authoritative host state.
    state.turn = 43;
    state.frame = 0;
    let mut next_turn = vec![unit_order(7, "MOVE_TO", Some((3, 3)))];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut next_turn, &state, &mut ours),
        0
    );
    assert_eq!(next_turn.len(), 1);
}

/// Unit 7 steps onto the plot unit 8 is leaving. The host drops 7's request
/// while 8 still stands there, so the replan's repeat of it must go out.
fn chained_move_frames() -> (civvis::mirror::StateSnapshot, HostMoveRefusals) {
    let (_snapshot, mut state) = production_board();
    state.turn = 42;
    state.frame = 0;
    state.units = vec![placed_unit(7, 3, 2), placed_unit(8, 3, 3)];
    let mut refusals = HostMoveRefusals::default();
    let mut ours = std::collections::BTreeMap::new();
    let mut first = vec![
        unit_order(8, "MOVE_TO", Some((4, 4))),
        unit_order(7, "MOVE_TO", Some((3, 3))),
    ];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut first, &state, &mut ours),
        0
    );
    state.frame = 1;
    (state, refusals)
}

fn placed_unit(id: i64, x: i32, y: i32) -> civvis::mirror::StateUnit {
    civvis::mirror::StateUnit {
        id,
        kind: "UNIT_WARRIOR".to_string(),
        x,
        y,
        ..civvis::mirror::StateUnit::default()
    }
}

#[test]
fn a_move_onto_a_plot_its_occupant_left_is_sent_again_once() {
    let (mut state, mut refusals) = chained_move_frames();
    let mut ours = std::collections::BTreeMap::new();
    state.units = vec![placed_unit(7, 3, 2), placed_unit(8, 4, 4)];
    let mut replan = vec![unit_order(7, "MOVE_TO", Some((3, 3)))];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut replan, &state, &mut ours),
        0,
        "the plot has cleared and the unit has not moved"
    );
    assert_eq!(replan.len(), 1);

    state.frame = 2;
    let mut again = vec![unit_order(7, "MOVE_TO", Some((3, 3)))];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut again, &state, &mut ours),
        1,
        "one retry only"
    );
}

#[test]
fn a_cleared_chained_move_goes_again_when_the_replan_leaves_its_unit_out() {
    let (mut state, mut refusals) = chained_move_frames();
    state.units = vec![placed_unit(7, 3, 2), placed_unit(8, 4, 4)];
    let mut replan = vec![unit_order(8, "FORTIFY", None)];
    assert_eq!(
        refusals.resend_cleared_chained_moves(&mut replan, &state),
        1
    );
    assert!(replan.iter().any(|order| {
        order.subject == Some(7)
            && order.verb.as_deref() == Some("MOVE_TO")
            && order.pos == Some((3, 3))
    }));
    let mut later = Vec::new();
    assert_eq!(
        refusals.resend_cleared_chained_moves(&mut later, &state),
        0,
        "one resend only"
    );
}

#[test]
fn a_chained_move_is_not_resent_over_a_new_order_or_on_the_first_frame() {
    let (mut state, mut refusals) = chained_move_frames();
    state.units = vec![placed_unit(7, 3, 2), placed_unit(8, 4, 4)];
    let mut replan = vec![unit_order(7, "FORTIFY", None)];
    assert_eq!(
        refusals.resend_cleared_chained_moves(&mut replan, &state),
        0,
        "the replan has given the unit something else to do"
    );
    assert_eq!(replan.len(), 1);

    state.frame = 0;
    let mut first = Vec::new();
    assert_eq!(refusals.resend_cleared_chained_moves(&mut first, &state), 0);
}

#[test]
fn a_chained_move_stays_a_replay_while_the_plot_is_taken_or_the_unit_went() {
    let mut ours = std::collections::BTreeMap::new();

    let (mut state, mut refusals) = chained_move_frames();
    state.units = vec![placed_unit(7, 3, 2), placed_unit(8, 3, 3)];
    let mut occupied = vec![unit_order(7, "MOVE_TO", Some((3, 3)))];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut occupied, &state, &mut ours),
        1,
        "the occupant is still there"
    );

    let (mut state, mut refusals) = chained_move_frames();
    state.units = vec![placed_unit(7, 3, 3), placed_unit(8, 4, 4)];
    let mut arrived = vec![unit_order(7, "MOVE_TO", Some((3, 3)))];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut arrived, &state, &mut ours),
        1,
        "the unit got there, so the frame is the ordinary stale replay"
    );
}

#[test]
fn an_explicitly_refused_build_releases_the_same_turn_queue_lease() {
    let (_, mut state) = production_board();
    let mut refusals = HostMoveRefusals::default();
    let mut ours = std::collections::BTreeMap::new();
    let mut first = vec![production_order(7, "DISTRICT_CAMPUS")];
    refusals.suppress_same_turn_replays(&mut first, &state, &mut ours);
    state.frame = 1;
    state
        .refused_production
        .insert(7, ["DISTRICT_CAMPUS".into()].into_iter().collect());
    let mut retry = vec![
        production_order(7, "DISTRICT_CAMPUS"),
        production_order(7, "UNIT_BUILDER"),
    ];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut retry, &state, &mut ours),
        1
    );
    assert_eq!(retry[0].verb.as_deref(), Some("UNIT_BUILDER"));
    assert_eq!(ours.get(&7).map(String::as_str), Some("UNIT_BUILDER"));
    let mut competing = vec![production_order(7, "UNIT_TRADER")];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut competing, &state, &mut ours),
        1
    );
}

#[test]
fn no_path_feedback_retries_local_steps_for_one_turn_only() {
    let (_, mut state) = production_board();
    state.units = vec![StateUnit {
        id: 900,
        kind: "UNIT_SCOUT".into(),
        x: 4,
        y: 5,
        hp: 100.0,
        moves: 3.0,
        ..Default::default()
    }];
    let unit = state.units[0].id;
    let from = (state.units[0].x, state.units[0].y);
    let turn = state.turn;
    let mut refusals = HostMoveRefusals::default();
    refusals.record(
        &[unit_order(unit, "MOVE_TO", Some((from.0 + 3, from.1)))],
        &state,
        &Default::default(),
    );
    state.turn += 1;
    refusals.observe(
        &state,
        &[Order {
            kind: "order_failed",
            subject: Some(unit),
            verb: Some("unit:MOVE_TO host_noop_no_path".into()),
            pos: Some((turn as i32, -1)),
        }],
    );
    assert_eq!(refusals.local_retry.get(&unit), Some(&(state.turn, from)));
    let retry_steps = vec![
        unit_order(unit, "MOVE_TO", Some((5, 5))),
        unit_order(unit, "MOVE_TO", Some((6, 5))),
    ];
    let local = refusals.local_retry.keys().copied().collect();
    let (orders, _, coalesced) =
        coalesce_unit_paths_except(retry_steps, true, &local, &Default::default());
    assert_eq!(coalesced, 0);
    assert_eq!(orders.len(), 2);
    state.frame = 1;
    refusals.observe(&state, &[]);
    assert_eq!(refusals.local_retry.len(), 1);
    state.turn += 1;
    refusals.observe(&state, &[]);
    assert!(refusals.local_retry.is_empty());
}

#[test]
fn the_final_city_queue_in_one_batch_survives_the_replay_filter() {
    let (_, state) = production_board();
    let mut refusals = HostMoveRefusals::default();
    let mut ours = std::collections::BTreeMap::new();
    let mut orders = vec![
        production_order(7, "PROJECT_ENHANCE_DISTRICT_THEATER"),
        production_order(8, "BUILDING_LIGHTHOUSE"),
        unit_order(9, "MOVE_TO", Some((4, 5))),
        production_order(7, "UNIT_CUIRASSIER"),
    ];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut orders, &state, &mut ours),
        1
    );
    assert_eq!(orders.len(), 3);
    assert_eq!(orders[0].subject, Some(8));
    assert_eq!(orders[1].kind, "unit");
    assert_eq!(orders[2].verb.as_deref(), Some("UNIT_CUIRASSIER"));
    assert_eq!(ours.get(&7).map(String::as_str), Some("UNIT_CUIRASSIER"));
    let mut replan = vec![production_order(7, "UNIT_BUILDER")];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut replan, &state, &mut ours),
        1
    );
    assert!(
        replan.is_empty(),
        "a later call still cannot replace the sent choice"
    );
    assert_eq!(ours.get(&7).map(String::as_str), Some("UNIT_CUIRASSIER"));
}

#[test]
fn a_same_turn_replan_keeps_the_first_city_queue_and_ownership() {
    let (_snapshot, mut state) = production_board();
    state.turn = 43;
    state.frame = 0;
    let mut refusals = HostMoveRefusals::default();
    let mut ours = std::collections::BTreeMap::new();
    let mut first = vec![
        production_order(7, "UNIT_SETTLER"),
        production_order(8, "UNIT_BUILDER"),
    ];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut first, &state, &mut ours),
        0,
        "the first frame may choose production for every city"
    );
    assert_eq!(first.len(), 2);
    assert_eq!(ours.get(&7).map(String::as_str), Some("UNIT_SETTLER"));
    assert_eq!(ours.get(&8).map(String::as_str), Some("UNIT_BUILDER"));

    state.frame = 1;
    let mut replan = vec![
        production_order(7, "UNIT_WARRIOR"),
        // Even an identical request is stale once the first queue was sent.
        production_order(8, "UNIT_BUILDER"),
        production_order(8, "UNIT_WARRIOR"),
        production_order(9, "UNIT_ARCHER"),
    ];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut replan, &state, &mut ours),
        3,
        "later frames cannot replace a city queue already sent this turn"
    );
    assert_eq!(replan.len(), 1);
    assert_eq!(replan[0].subject, Some(9));
    assert_eq!(ours.get(&7).map(String::as_str), Some("UNIT_SETTLER"));
    assert_eq!(ours.get(&8).map(String::as_str), Some("UNIT_BUILDER"));

    // The next authoritative turn is a new production decision.
    state.turn = 44;
    state.frame = 0;
    let mut next_turn = vec![production_order(7, "UNIT_WARRIOR")];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut next_turn, &state, &mut ours),
        0
    );
    assert_eq!(next_turn.len(), 1);
    assert_eq!(ours.get(&7).map(String::as_str), Some("UNIT_WARRIOR"));
}

#[test]
fn a_same_turn_replan_preserves_one_working_deal_per_rival() {
    let (_snapshot, mut state) = production_board();
    state.turn = 44;
    state.frame = 0;
    let mut refusals = HostMoveRefusals::default();
    let mut ours = std::collections::BTreeMap::new();
    let mut first = vec![
        Order {
            kind: "sell",
            subject: Some(2),
            verb: Some("RESOURCE_HORSES=10".to_string()),
            pos: Some((200, 0)),
        },
        Order {
            kind: "buy",
            subject: Some(3),
            verb: Some("OPEN_BORDERS".to_string()),
            pos: Some((60, 0)),
        },
    ];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut first, &state, &mut ours),
        0,
        "the first frame is allowed to issue its complete deal plan"
    );

    state.frame = 1;
    let mut replan = vec![
        Order {
            kind: "sell",
            subject: Some(2),
            verb: Some("RESOURCE_HORSES=10".to_string()),
            pos: Some((200, 0)),
        },
        Order {
            kind: "buy",
            subject: Some(3),
            verb: Some("OPEN_BORDERS".to_string()),
            pos: Some((60, 0)),
        },
        Order {
            kind: "sell",
            subject: Some(2),
            verb: Some("RESOURCE_HORSES=10".to_string()),
            pos: Some((220, 0)),
        },
        // Non-deal orders retain their existing same-turn semantics.
        Order {
            kind: "research",
            subject: None,
            verb: Some("TECH_WRITING".to_string()),
            pos: None,
        },
    ];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut replan, &state, &mut ours),
        3,
        "a changed offer still competes with this rival's pending deal"
    );
    assert_eq!(replan.len(), 1);
    assert!(replan.iter().any(|order| {
        order.kind == "research" && order.verb.as_deref() == Some("TECH_WRITING")
    }));

    state.turn = 45;
    state.frame = 0;
    let mut next_turn = vec![Order {
        kind: "sell",
        subject: Some(2),
        verb: Some("RESOURCE_HORSES=10".to_string()),
        pos: Some((200, 0)),
    }];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut next_turn, &state, &mut ours),
        0,
        "a fresh turn may retry the old offer after the host's cooldown"
    );
}

fn production_board() -> (Snapshot, StateSnapshot) {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 1,
        width: 20,
        height: 20,
        chunk: 1,
        // `Plot` carries serde defaults for everything but the coordinates, so
        // deserializing is cheaper than naming a dozen fields the test does not
        // care about — and it cannot drift as new ones are added.
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
        turn: 1,
        cities: vec![StateCity {
            id: 7,
            name: "Ottawa".to_string(),
            x: 4,
            y: 4,
            pop: 4,
            producing: Some("UNIT_WARRIOR".to_string()),
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    (snapshot, state)
}

#[test]
fn host_production_finish_gate_requires_authoritative_metrics() {
    let mut city = StateCity {
        producing: Some("UNIT_WARRIOR".to_string()),
        production: 8.0,
        production_cost: 40.0,
        production_progress: 32.0,
        production_turns: 1.0,
        ..StateCity::default()
    };
    assert!(host_production_finishes_this_turn(&city));

    city.production_progress = 10.0;
    city.production_turns = 4.0;
    assert!(!host_production_finishes_this_turn(&city));

    city.production_progress = -1.0;
    city.production_turns = -1.0;
    assert!(!host_production_finishes_this_turn(&city));
}

/// ⚠⚠ The defect: the mod ladder answers `ENDTURN_BLOCKING_PRODUCTION`, the next
/// rebuild seeds that choice into CIVVIS's queue as work in progress, and CIVVIS —
/// which only produces for a city whose queue is empty — says nothing. Measured at
/// 13% of turns carrying a produce order against 100% carrying research and civic.
/// The largest waste class on the ladder must say WHICH unit it is.
///
/// `self_tile_move` totalled 25,387 across 43 live runs as one anonymous
/// number. Named, five replayed runs answered it in one pass: 249 events,
/// **100% military**, zero settlers — which is the opposite of what the
/// expansion-tempo reading predicted and sends the repair somewhere else.
///
/// ⚠ Two-sided: the key must carry the kind AND keep the `self_tile_move`
/// prefix, because existing readers match on that prefix and a bare total
/// has to stay recoverable.
#[test]
fn a_self_tile_move_names_the_unit_that_asked_for_it() {
    let (snapshot, state) = local_barbarian_defense_board();
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
    // ⚠ DISCOVERED, not hardcoded: the point is that whatever unit the board
    // carries, its kind reaches the key. Naming one would make this fail on a
    // fixture change rather than on anything to do with the instrument.
    let unit = *mirror
        .game
        .units
        .keys()
        .next()
        .expect("this board carries units");
    let kind = mirror.game.units[&unit].kind.to_string();
    assert!(
        !kind.is_empty(),
        "fixture precondition: the unit has a kind"
    );

    let key = self_tile_move_key(&mirror, unit);
    assert!(
        key.starts_with("self_tile_move"),
        "the prefix is load-bearing for every existing reader, got {key}"
    );
    assert_eq!(key, format!("self_tile_move:{kind}"));

    // A unit the bridge cannot map back still has to be counted, not dropped:
    // an unnamed refusal is the exact failure this key exists to end.
    let missing = mirror.game.units.keys().copied().max().unwrap_or(0) + 1_000;
    assert_eq!(self_tile_move_key(&mirror, missing), "self_tile_move");
}

/// ★★★★★ THE DEFERRED PRODUCTION HINT NEVER FIRED IN ANY RECORDED GAME.
/// `append_next_production_hints` previews the next build on the board the
/// agent has already taken its turn on — and `take_turn` ends with
/// `EndTurn`, so `current` has moved to the next seat and every `Produce`
/// the preview tries is refused "not your turn". Zero `produce_next` orders
/// across the 2026-08-16..19 ladder; 70–174 mod-ladder `build` picks per
/// game answered the production prompt instead, ~85% displaced by CIVVIS's
/// own order one turn later. The preview must run on a board where it is
/// still this seat's turn.
#[test]
fn the_next_production_hint_survives_the_seat_having_ended_its_turn() {
    let (snapshot, mut state) = production_board();
    // The exported city finishes its Warrior this turn.
    state.cities[0].production = 8.0;
    state.cities[0].production_cost = 40.0;
    state.cities[0].production_progress = 36.0;
    state.cities[0].production_turns = 1.0;
    assert!(host_production_finishes_this_turn(&state.cities[0]));
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
    let mut planned = mirror.game.clone();
    let mut ai = civvis::ai::AdvancedAi::new();
    ai.enable_live_bridge();
    // The agent takes its turn on the planning board, exactly as `decide`
    // does, and the board's seat moves on.
    ai.take_turn(&mut planned, 0);
    assert_ne!(
        planned.current, 0,
        "precondition: the seat has ended its turn"
    );

    let mut orders = Vec::new();
    let mut ours = std::collections::BTreeMap::new();
    let hinted =
        append_next_production_hints(&ai, &planned, &mirror, &state, &mut orders, &mut ours);
    assert_eq!(
        hinted,
        1,
        "the finishing city gets a hint: {:?}",
        orders.iter().map(Order::to_json).collect::<Vec<_>>()
    );
    let hint = orders
        .iter()
        .find(|order| order.kind == "produce_next")
        .expect("a produce_next order");
    assert_eq!(hint.subject, Some(7));
    assert!(hint.verb.as_deref().is_some_and(|v| !v.is_empty()));
    assert!(
        ours.get(&7)
            .is_some_and(|value| value.starts_with(DEFERRED_PRODUCTION_PREFIX)),
        "the lease is remembered until the host consumes it: {ours:?}"
    );
    // And the preview never touched the caller's board.
    assert_ne!(planned.current, 0);
}

fn released_queue_progress_board(producing: Option<&str>) -> (Snapshot, StateSnapshot) {
    let (snapshot, mut state) = production_board();
    state.cities[0].producing = producing.map(str::to_string);
    state.cities[0].production = 6.0;
    state.cities[0].production_cost = 55.0;
    state.cities[0].production_progress = 17.0;
    state.cities[0].production_turns = 7.0;
    (snapshot, state)
}

#[test]
fn released_queue_progress_stays_with_the_original_settler() {
    use civvis::game::Item;
    let (snapshot, state) = released_queue_progress_board(Some("UNIT_SETTLER"));
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
    let cid = mirror.cid_of[&7];
    let mut planned = mirror.game.clone();
    let ours = std::collections::BTreeMap::from([(7, "UNIT_BUILDER".to_string())]);
    assert_eq!(
        release_foreign_production(&mut planned, &mirror.cid_of, &state, &ours),
        1
    );
    assert!(planned.cities[&cid].queue.is_empty());
    assert_eq!(
        planned.cities[&cid].production, 0.0,
        "the Settler's 17 paid production is not free overflow for another item"
    );
    assert_eq!(
        planned.cities[&cid].production_progress.get("unit:settler"),
        Some(&17.0)
    );
    planned
        .apply(
            0,
            &Action::Produce {
                city: cid,
                item: Item::Unit {
                    unit: "scout".into(),
                },
            },
        )
        .expect("a legal unrelated replacement");
    assert_eq!(planned.cities[&cid].production, 0.0);
    planned
        .apply(
            0,
            &Action::Produce {
                city: cid,
                item: Item::Unit {
                    unit: "settler".into(),
                },
            },
        )
        .expect("resume the original Settler");
    assert_eq!(planned.cities[&cid].production, 17.0);
    assert_eq!(mirror.game.cities[&cid].production, 17.0);
    assert_eq!(
        mirror.game.cities[&cid].queue[0],
        Item::Unit {
            unit: "settler".into()
        }
    );
}

#[test]
fn released_queue_progress_keeps_other_paused_work_separate() {
    use civvis::game::Item;
    let (snapshot, state) = released_queue_progress_board(Some("UNIT_SETTLER"));
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
    let cid = mirror.cid_of[&7];
    let mut planned = mirror.game.clone();
    planned
        .cities
        .get_mut(&cid)
        .unwrap()
        .production_progress
        .insert("building:monument".to_string(), 11.0);
    planned
        .cities
        .get_mut(&cid)
        .unwrap()
        .queue
        .push(Item::Unit {
            unit: "warrior".into(),
        });
    assert_eq!(
        release_foreign_production(
            &mut planned,
            &mirror.cid_of,
            &state,
            &std::collections::BTreeMap::new()
        ),
        1
    );
    assert_eq!(
        planned.cities[&cid].production_progress.get("unit:settler"),
        Some(&17.0),
        "releasing the head keeps its own paid work"
    );
    assert!(planned.cities[&cid].queue.is_empty());
    assert!(!planned.cities[&cid]
        .production_progress
        .contains_key("unit:warrior"));
    planned
        .apply(
            0,
            &Action::Produce {
                city: cid,
                item: Item::Building {
                    building: "monument".into(),
                },
            },
        )
        .expect("resume a separately paid Monument");
    assert_eq!(planned.cities[&cid].production, 11.0);
}

#[test]
fn released_queue_progress_leaves_owned_work_active() {
    let (snapshot, state) = released_queue_progress_board(Some("UNIT_SETTLER"));
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
    let cid = mirror.cid_of[&7];
    let mut planned = mirror.game.clone();
    let ours = std::collections::BTreeMap::from([(7, "UNIT_SETTLER".to_string())]);
    assert_eq!(
        release_foreign_production(&mut planned, &mirror.cid_of, &state, &ours),
        0
    );
    assert_eq!(planned.cities[&cid].production, 17.0);
    assert_eq!(planned.cities[&cid].queue, mirror.game.cities[&cid].queue);
}

#[test]
fn released_queue_progress_leaves_real_idle_overflow_available() {
    use civvis::game::Item;
    let (snapshot, state) = released_queue_progress_board(None);
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
    let cid = mirror.cid_of[&7];
    let mut planned = mirror.game.clone();
    assert_eq!(
        release_foreign_production(
            &mut planned,
            &mirror.cid_of,
            &state,
            &std::collections::BTreeMap::new()
        ),
        0
    );
    planned
        .apply(
            0,
            &Action::Produce {
                city: cid,
                item: Item::Unit {
                    unit: "scout".into(),
                },
            },
        )
        .expect("a legal item may use actual unassigned overflow");
    assert_eq!(planned.cities[&cid].production, 17.0);
}

#[test]
fn a_build_civvis_did_not_choose_is_handed_back_to_it() {
    let (snapshot, state) = production_board();
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
    let cid = *mirror
        .cid_of
        .get(&7)
        .expect("the exported city is mirrored");
    let mut planned = mirror.game.clone();
    assert!(
        !planned.cities[&cid].queue.is_empty(),
        "precondition: the rebuild seeds the queue from `producing`"
    );

    // Nothing recorded for this city, so the warrior is the ladder's pick.
    let ours = std::collections::BTreeMap::new();
    assert_eq!(
        release_foreign_production(&mut planned, &mirror.cid_of, &state, &ours),
        1
    );
    assert!(
        planned.cities[&cid].queue.is_empty(),
        "a choice CIVVIS did not make must not read as its own plan"
    );
    assert!(
        !mirror.game.cities[&cid].queue.is_empty(),
        "and the authoritative mirror must be left as the last exported state"
    );
}

#[test]
fn host_upgraded_production_keeps_its_existing_commitment() {
    for (ordered, observed, keep) in [
        ("UNIT_TREBUCHET", "UNIT_BOMBARD", true),
        ("UNIT_CATAPULT", "UNIT_BOMBARD", true),
        ("UNIT_WARRIOR", "UNIT_SWORDSMAN", true),
        ("UNIT_BOMBARD", "UNIT_TREBUCHET", false),
        ("UNIT_ARCHER", "UNIT_BOMBARD", false),
        ("BUILDING_LIBRARY", "UNIT_BOMBARD", false),
        ("UNIT_UNKNOWN", "UNIT_BOMBARD", false),
    ] {
        let (snapshot, mut state) = production_board();
        state.cities[0].producing = Some(observed.to_string());
        state.cities[0].production_progress = 67.0;
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
        let cid = mirror.cid_of[&7];
        let mut planned = mirror.game.clone();
        let queue = planned.cities[&cid].queue.clone();
        assert!(!queue.is_empty());
        let ours = std::collections::BTreeMap::from([(7_i64, ordered.to_string())]);
        assert_eq!(
            release_foreign_production(&mut planned, &mirror.cid_of, &state, &ours),
            usize::from(!keep),
            "{ordered} -> {observed}"
        );
        assert_eq!(planned.cities[&cid].queue.is_empty(), !keep);
        assert_eq!(mirror.game.cities[&cid].queue, queue);
    }
}

/// The other half, and what keeps this from re-creating the thrash the queue
/// seeding was added to fix: once CIVVIS's own choice is what the city is
/// building, the seed stands and it is left alone.
#[test]
fn civvis_own_choice_is_left_alone_so_the_override_is_self_limiting() {
    let (snapshot, state) = production_board();
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
    let cid = *mirror
        .cid_of
        .get(&7)
        .expect("the exported city is mirrored");
    let mut planned = mirror.game.clone();

    let mut ours = std::collections::BTreeMap::new();
    ours.insert(7_i64, "UNIT_WARRIOR".to_string());
    assert_eq!(
        release_foreign_production(&mut planned, &mirror.cid_of, &state, &ours),
        0
    );
    assert!(
        !planned.cities[&cid].queue.is_empty(),
        "work in progress CIVVIS asked for must survive, or it re-chooses every turn"
    );
}

#[test]
fn a_deferred_production_hint_preserves_a_slow_host_queue() {
    let (snapshot, mut state) = production_board();
    state.cities[0].production = 8.0;
    state.cities[0].production_cost = 40.0;
    state.cities[0].production_progress = 32.0;
    state.cities[0].production_turns = 1.0;
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
    let cid = *mirror
        .cid_of
        .get(&7)
        .expect("the exported city is mirrored");
    let mut planned = mirror.game.clone();
    let mut ours = std::collections::BTreeMap::from([(
        7_i64,
        format!("{DEFERRED_PRODUCTION_PREFIX}UNIT_SETTLER"),
    )]);

    assert_eq!(settle_deferred_production_hints(&mut ours, &state), (0, 0));
    assert_eq!(
        release_foreign_production(&mut planned, &mirror.cid_of, &state, &ours),
        0,
        "a near-finished old queue stays intact until its blocker consumes the lease"
    );
    assert!(!planned.cities[&cid].queue.is_empty());

    state.cities[0].production_progress = 10.0;
    state.cities[0].production_turns = 4.0;
    assert_eq!(settle_deferred_production_hints(&mut ours, &state), (0, 1));
    assert_eq!(
        release_foreign_production(&mut planned, &mirror.cid_of, &state, &ours),
        1,
        "a stale lease is released once the host is no longer approaching its blocker"
    );
}

/// ★★★★ A DECIDER RESTARTED MID-GAME MUST NOT HAND BACK EVERY CITY. A fresh
/// process holds an empty `ours`, so on its first turn all work in progress
/// read as foreign and was re-decided (run civvis-20260816T213447Z, t139 and
/// t157: the Taj Mahal at 188/460 dropped, seven Rangers ordered). Adopting
/// the host's current item as ours on that first turn leaves the queue alone,
/// keeps a plan this process already ordered, and skips idle cities.
#[test]
fn a_restarted_decider_adopts_the_hosts_production_as_its_own() {
    let (snapshot, mut state) = production_board();
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
    let cid = *mirror
        .cid_of
        .get(&7)
        .expect("the exported city is mirrored");
    let mut planned = mirror.game.clone();

    let mut ours = std::collections::BTreeMap::new();
    assert_eq!(
        adopt_host_production(&mut ours, &state),
        1,
        "one city building"
    );
    assert_eq!(ours.get(&7).map(String::as_str), Some("UNIT_WARRIOR"));
    assert_eq!(
        release_foreign_production(&mut planned, &mirror.cid_of, &state, &ours),
        0,
        "the adopted item is not released"
    );
    assert!(!planned.cities[&cid].queue.is_empty());

    // An entry this process already ordered wins over the host's reading.
    let mut ours = std::collections::BTreeMap::new();
    ours.insert(7_i64, "UNIT_SETTLER".to_string());
    assert_eq!(adopt_host_production(&mut ours, &state), 0);
    assert_eq!(ours.get(&7).map(String::as_str), Some("UNIT_SETTLER"));

    // An idle city adopts nothing.
    state.cities[0].producing = None;
    let mut ours = std::collections::BTreeMap::new();
    assert_eq!(adopt_host_production(&mut ours, &state), 0);
    assert!(ours.is_empty());
}

#[test]
fn an_emergency_wall_replacement_overrides_stale_production_ownership() {
    let (snapshot, mut state) = production_board();
    state.cities[0].producing = Some("BUILDING_WALLS".to_string());
    state.cities[0].max_wall_damage = 0.0;
    state.rivals = vec![StateRival {
        player: 3,
        at_war: true,
        units: vec![StateUnit {
            kind: "UNIT_WARRIOR".to_string(),
            x: 5,
            y: 4,
            ..StateUnit::default()
        }],
        ..StateRival::default()
    }];
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
    let cid = *mirror
        .cid_of
        .get(&7)
        .expect("the exported city is mirrored");
    let mut planned = mirror.game.clone();
    let mut ours = std::collections::BTreeMap::from([(7_i64, "DISTRICT_CAMPUS".to_string())]);

    assert_eq!(adopt_emergency_wall_production(&mut ours, &state), 1);
    assert_eq!(ours.get(&7).map(String::as_str), Some("BUILDING_WALLS"));
    assert_eq!(
        release_foreign_production(&mut planned, &mirror.cid_of, &state, &ours),
        0,
        "the host's emergency queue must not be cleared by the stale Campus lease"
    );
    assert!(
        !planned.cities[&cid].queue.is_empty(),
        "the threatened city's wall queue remains available to finish"
    );
}

#[test]
fn an_unthreatened_wall_queue_does_not_capture_civvis_ownership() {
    let (_snapshot, mut state) = production_board();
    state.cities[0].producing = Some("BUILDING_WALLS".to_string());
    state.cities[0].max_wall_damage = 0.0;
    let mut ours = std::collections::BTreeMap::from([(7_i64, "DISTRICT_CAMPUS".to_string())]);
    assert_eq!(adopt_emergency_wall_production(&mut ours, &state), 0);
    assert_eq!(ours.get(&7).map(String::as_str), Some("DISTRICT_CAMPUS"));
}

/// A city building nothing has nothing to hand back, and must not be counted.
#[test]
fn an_idle_city_is_not_reported_as_released() {
    let (snapshot, mut state) = production_board();
    state.cities[0].producing = None;
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 500, 0);
    let mut planned = mirror.game.clone();
    let ours = std::collections::BTreeMap::new();
    assert_eq!(
        release_foreign_production(&mut planned, &mirror.cid_of, &state, &ours),
        0
    );
}

pub(super) fn local_barbarian_defense_board() -> (Snapshot, StateSnapshot) {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 30,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| (x, y)))
            .map(|(x, y)| {
                serde_json::from_value::<Plot>(serde_json::json!({
                    "x": x, "y": y, "t": "TERRAIN_GRASS"
                }))
                .expect("a plot with serde defaults deserializes")
            })
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 30,
        cities: vec![StateCity {
            id: 7,
            name: "Rome".to_string(),
            x: 4,
            y: 4,
            pop: 4,
            ..StateCity::default()
        }],
        units: vec![
            StateUnit {
                id: 100,
                kind: "UNIT_SLINGER".to_string(),
                x: 4,
                y: 5,
                hp: 100.0,
                moves: 2.0,
                ..StateUnit::default()
            },
            StateUnit {
                id: 101,
                kind: "UNIT_WARRIOR".to_string(),
                x: 5,
                y: 5,
                hp: 100.0,
                moves: 2.0,
                ..StateUnit::default()
            },
            StateUnit {
                id: 102,
                kind: "UNIT_SPEARMAN".to_string(),
                x: 6,
                y: 4,
                hp: 100.0,
                moves: 2.0,
                ..StateUnit::default()
            },
        ],
        hostiles: vec![StateUnit {
            kind: "UNIT_WARRIOR".to_string(),
            x: 5,
            y: 4,
            hp: 64.0,
            combat: 20.0,
            moves: 0.0,
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };
    (snapshot, state)
}

/// The same exposed-defender geometry, but a current war against a rival
/// supplies the defender rather than the aggregate barbarian export.
fn local_war_enemy_board() -> (Snapshot, StateSnapshot) {
    let (snapshot, mut state) = local_barbarian_defense_board();
    let hostile = state
        .hostiles
        .pop()
        .expect("the local defense fixture carries one barbarian");
    state.rivals.push(StateRival {
        player: 3,
        at_war: true,
        units: vec![hostile],
        ..StateRival::default()
    });
    (snapshot, state)
}

fn finishing_survival_field(with_archer: bool, ranged_primary: bool) -> civvis::mirror::LiveMirror {
    let (snapshot, mut state) = local_barbarian_defense_board();
    state.units.retain(|unit| unit.id == 101);
    state.units[0].hp = 56.0;
    if ranged_primary {
        state.units.push(StateUnit {
            id: 102,
            kind: "UNIT_ARCHER".to_string(),
            x: 5,
            y: 6,
            hp: 100.0,
            moves: 2.0,
            combat: 15.0,
            ranged: 25.0,
            ..StateUnit::default()
        });
    }
    state.hostiles[0].id = 200;
    state.hostiles[0].hp = 1.0;
    if with_archer {
        state.hostiles.push(StateUnit {
            id: 201,
            kind: "UNIT_ARCHER".to_string(),
            x: 5,
            y: 2,
            hp: 100.0,
            combat: 15.0,
            ranged: 25.0,
            moves: 0.0,
            ..StateUnit::default()
        });
    }
    civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0)
}

#[test]
fn one_native_damage_roll_does_not_prove_a_live_finishing_kill() {
    let mirror = finishing_survival_field(false, false);
    let uid = mirror.uid_of[&101];
    let target = mirror.foreign_uid_of[&200];
    let mut found = false;
    for hp in 20..60 {
        let mut game = mirror.game.clone();
        game.units.get_mut(&uid).unwrap().hp = 100;
        game.units.get_mut(&target).unwrap().hp = hp;
        let (attack, defense) = game.melee_exchange_strengths(uid, target).unwrap();
        let low_damage = (civvis::game::expected_damage(attack, defense) * 0.8)
            .round()
            .clamp(1.0, 100.0) as i32;
        let candidates = live_finishing_candidates(
            &game,
            0,
            target,
            &mirror.civ6_of,
            &std::collections::BTreeSet::new(),
        );
        if low_damage >= hp || !candidates.iter().any(|candidate| candidate.kills) {
            continue;
        }
        found = true;
        let volley = finish_live_war_units(&mut game, 0, &mirror.civ6_of);
        assert_eq!(volley.targets, 0,
                "a sampled kill cannot reserve the lone attacker when a lower damage roll leaves the target alive");
        assert!(volley.actions.is_empty());
        break;
    }
    assert!(
        found,
        "the fixture must include a sampled kill that a lower roll does not finish"
    );
}

#[test]
fn finishing_volley_respects_selected_wounded_withdrawals() {
    let mut mirror = finishing_survival_field(true, false);
    let uid = mirror.uid_of[&101];
    mirror.game.units.get_mut(&uid).unwrap().hp = 40;
    let mut off = mirror.game.clone();
    assert_eq!(
        finish_live_war_units(&mut off, 0, &mirror.civ6_of).targets,
        1
    );
    let mut ai = civvis::ai::AdvancedAi::new();
    ai.enable_wounded_out_of_reach();
    let mut on = mirror.game.clone();
    let volley = finish_live_war_units_excluding(
        &mut on,
        0,
        &mirror.civ6_of,
        &std::collections::BTreeSet::new(),
        &ai,
    );
    assert!(volley.actions.is_empty());
    assert_eq!(on.units[&uid].hp, 40);
    assert_eq!(on.units[&uid].pos, mirror.game.units[&uid].pos);
}

#[test]
fn finishing_survival_policy_rejects_the_kill_before_a_visible_archer_reply() {
    let mirror = finishing_survival_field(true, false);
    let mut old = mirror.game.clone();
    assert_eq!(
        finish_live_war_units(&mut old, 0, &mirror.civ6_of).targets,
        1
    );
    let mut ai = civvis::ai::AdvancedAi::new();
    ai.enable_doomed_blow_veto_2();
    let mut planned = mirror.game.clone();
    let volley = finish_live_war_units_excluding(
        &mut planned,
        0,
        &mirror.civ6_of,
        &std::collections::BTreeSet::new(),
        &ai,
    );
    assert!(
        volley.actions.is_empty(),
        "the pre-pass must not spend a striker the survival policy rejects"
    );
    assert_eq!(planned.units.len(), mirror.game.units.len());
    assert_eq!(planned.log.len(), mirror.game.log.len());
}

#[test]
fn finishing_survival_policy_keeps_a_kill_without_the_archer_reply() {
    let mut mirror = finishing_survival_field(false, false);
    let mut ai = civvis::ai::AdvancedAi::new();
    ai.enable_doomed_blow_veto_2();
    let volley = finish_live_war_units_excluding(
        &mut mirror.game,
        0,
        &mirror.civ6_of,
        &std::collections::BTreeSet::new(),
        &ai,
    );
    assert_eq!(volley.targets, 1);
    assert_eq!(volley.actions.len(), 1);
}

#[test]
fn finishing_survival_policy_also_covers_version_one() {
    let mut mirror = finishing_survival_field(true, false);
    let mut ai = civvis::ai::AdvancedAi::new();
    ai.enable_doomed_blow_veto();
    let volley = finish_live_war_units_excluding(
        &mut mirror.game,
        0,
        &mirror.civ6_of,
        &std::collections::BTreeSet::new(),
        &ai,
    );
    assert!(volley.actions.is_empty());
    assert_eq!(volley.survival_rejections, 1);
}

#[test]
fn finishing_survival_policy_does_not_reserve_an_unsafe_melee_followup() {
    let mirror = finishing_survival_field(true, true);
    let mut old = mirror.game.clone();
    let old_volley = finish_live_war_units(&mut old, 0, &mirror.civ6_of);
    assert_eq!(old_volley.reserves, 1);
    let guard = *mirror.civ6_of.iter().find(|(_, id)| **id == 101).unwrap().0;
    let mut planned = mirror.game.clone();
    let mut ai = civvis::ai::AdvancedAi::new();
    ai.enable_doomed_blow_veto_2();
    let volley = finish_live_war_units_excluding(
        &mut planned,
        0,
        &mirror.civ6_of,
        &std::collections::BTreeSet::new(),
        &ai,
    );
    assert_eq!(volley.targets, 1);
    assert_eq!(volley.reserves, 0);
    assert_eq!(volley.actions.len(), 1);
    assert!(matches!(volley.actions[0], Action::Ranged { .. }));
    assert_eq!(
        planned.units[&guard].moves_left,
        mirror.game.units[&guard].moves_left
    );
    assert_eq!(
        planned.units[&guard].attacks_left,
        mirror.game.units[&guard].attacks_left
    );
}

#[test]
fn finishing_survival_policy_keeps_a_safe_reserve() {
    let mut mirror = finishing_survival_field(false, true);
    let mut ai = civvis::ai::AdvancedAi::new();
    ai.enable_doomed_blow_veto_2();
    let volley = finish_live_war_units_excluding(
        &mut mirror.game,
        0,
        &mirror.civ6_of,
        &std::collections::BTreeSet::new(),
        &ai,
    );
    assert_eq!(volley.targets, 1);
    assert_eq!(volley.reserves, 1);
    assert_eq!(volley.actions.len(), 2);
}

#[test]
fn finishing_survival_policy_prices_the_whole_volley_after_its_kill() {
    let (snapshot, mut state) = local_barbarian_defense_board();
    state.units[0].hp = 30.0;
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 13, 250, 0);
    let mut old = mirror.game.clone();
    let old_volley = finish_live_war_units(&mut old, 0, &mirror.civ6_of);
    assert!(old_volley.actions.len() > 1);
    assert_eq!(old_volley.reserves, 1);
    let primary = &old_volley.actions[..old_volley.actions.len() - old_volley.reserves];
    let mut ai = civvis::ai::AdvancedAi::new();
    ai.enable_doomed_blow_veto_2();
    assert!(!ai.live_finishing_actions_survive(&mirror.game, 0, &old_volley.actions[..1]));
    assert!(ai.live_finishing_actions_survive(&mirror.game, 0, primary));
    let mut planned = mirror.game.clone();
    let volley = finish_live_war_units_excluding(
        &mut planned,
        0,
        &mirror.civ6_of,
        &std::collections::BTreeSet::new(),
        &ai,
    );
    assert_eq!(volley.targets, 1);
    assert_eq!(volley.actions, old_volley.actions);
}

#[test]
fn dialogue_never_declares_war_rides_every_bridge_reply_only_when_selected() {
    let directives = |enable: bool| {
        let (snapshot, state) = local_barbarian_defense_board();
        let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let mut ai = victory_lane("domination").unwrap();
        if enable {
            ai.enable_dialogue_never_declares_war();
        }
        let mut continuation = air_assault_continuation::Continuation::default();
        let reply: serde_json::Value = serde_json::from_str(&decide(
            &mut mirror,
            &mut ai,
            &snapshot,
            &state,
            default_decision_arm(),
            DecisionMemory {
                ours: &mut std::collections::BTreeMap::new(),
                host_peace_retries: &mut HostPeaceRetries::default(),
                host_move_refusals: &mut HostMoveRefusals::default(),
                host_order_refusals: &mut HostOrderRefusals::default(),
                air_assault_continuation: &mut continuation,
                governor_exchange: &mut governor_exchange::Continuation::default(),
            },
        ))
        .unwrap();
        reply["orders"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|order| {
                order["kind"] == "combat_policy" && order["verb"] == "DIALOGUE_NEVER_DECLARES_WAR"
            })
            .count()
    };
    assert_eq!(directives(false), 0, "off: the reply is unchanged");
    assert_eq!(
        directives(true),
        1,
        "on: every reply renews the AutoClose lease, strikes or not"
    );
}

#[test]
fn finishing_survival_policy_does_not_restore_the_unsafe_wounded_followup() {
    let (snapshot, mut state) = local_barbarian_defense_board();
    state.cities[0].x = 5;
    state.cities[0].y = 5;
    state.units.retain(|unit| unit.id == 101);
    state.units[0].hp = 56.0;
    state.units.push(StateUnit {
        id: 102,
        kind: "UNIT_WARRIOR".to_string(),
        x: 6,
        y: 4,
        hp: 100.0,
        moves: 2.0,
        ..StateUnit::default()
    });
    state.hostiles[0].id = 200;
    state.hostiles[0].hp = 50.0;
    state.hostiles.push(StateUnit {
        id: 201,
        kind: "UNIT_ARCHER".to_string(),
        x: 5,
        y: 2,
        hp: 100.0,
        combat: 15.0,
        ranged: 25.0,
        moves: 0.0,
        ..StateUnit::default()
    });
    let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let mut old = mirror.game.clone();
    let old_volley = finish_live_war_units(&mut old, 0, &mirror.civ6_of);
    let old_strikers: Vec<_> = old_volley
        .actions
        .iter()
        .filter_map(|action| {
            if let Action::Attack { unit, .. } = action {
                Some(mirror.civ6_of[unit])
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        old_strikers,
        vec![102, 101],
        "the original pre-pass spends the wounded finisher"
    );
    let mut ai = victory_lane("science").unwrap();
    ai.enable_doomed_blow_veto_2();
    // `enable_battle_planner_2` clears version one only; clear whichever
    // later version the published batch rule ships so v2 plays alone.
    ai.disable_battle_planner_3();
    ai.enable_battle_planner_2();
    let reply: serde_json::Value = serde_json::from_str(&decide(
        &mut mirror,
        &mut ai,
        &snapshot,
        &state,
        default_decision_arm(),
        DecisionMemory {
            ours: &mut std::collections::BTreeMap::new(),
            host_peace_retries: &mut HostPeaceRetries::default(),
            host_move_refusals: &mut HostMoveRefusals::default(),
            host_order_refusals: &mut HostOrderRefusals::default(),
            air_assault_continuation: &mut air_assault_continuation::Continuation::default(),
            governor_exchange: &mut governor_exchange::Continuation::default(),
        },
    ))
    .unwrap();
    assert_eq!(
        reply["orders"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|order| {
                order["kind"] == "combat_policy" && order["verb"] == "DOOMED_BLOW_VETO"
            })
            .count(),
        1,
        "the full bridge reply must carry the selected survival policy"
    );
    let strikers: Vec<_> = reply["orders"]
        .as_array()
        .unwrap()
        .iter()
        // ⚠ A `preview` order carries verb ATTACK too, but it is a
        // question: `battle-planner-3` asks the host to price a pair and
        // nothing moves. Counted as strikes, two previews read as the
        // unsafe [102, 101, 101] whenever the batch rule ships v3.
        .filter(|order| {
            order["kind"] != "preview"
                && order["verb"] == "ATTACK"
                && order["x"] == 5
                && order["y"] == 4
        })
        .map(|order| order["subject"].as_i64().unwrap())
        .collect();
    if strikers.contains(&101) {
        // A stricter reply reserve can leave the healthy warrior in
        // cover and allow only the garrison's nonlethal opening blow.
        // That is safe: it cannot kill and advance onto the archer's
        // exposed tile. The healthy warrior may then finish, which is the
        // reverse of the unsafe order. The garrison must never follow it.
        assert_eq!(strikers[0], 101, "no wounded followup: {reply}");
        assert_eq!(
            strikers.iter().filter(|striker| **striker == 101).count(),
            1,
            "no wounded followup: {reply}"
        );
        let ours = mirror
            .civ6_of
            .iter()
            .find(|(_, host)| **host == 101)
            .unwrap()
            .0;
        let target = civvis::hex::offset_to_axial(5, 4);
        let defender = *mirror.game.unit_ids_at(target).first().unwrap();
        let (att, def) = mirror
            .game
            .melee_exchange_strengths(*ours, defender)
            .unwrap();
        let upper_damage = (30.0 * ((att - def) / 25.0).exp() * 1.2)
            .round()
            .clamp(1.0, 100.0);
        assert!(
            upper_damage < f64::from(mirror.game.units[&defender].hp),
            "even the high roll must leave the defender alive"
        );
        let mut after = mirror.game.clone();
        after
            .apply(
                0,
                &Action::Attack {
                    unit: *ours,
                    target,
                },
            )
            .unwrap();
        assert!(after.units.contains_key(&defender));
        assert!(after.units[ours].hp > 0);
        let city = after
            .city_at(after.units[ours].pos)
            .expect("still sheltered in the city");
        assert_eq!(after.cities[&city].owner, 0);
    } else {
        assert_eq!(
            strikers,
            vec![102],
            "only the healthy striker may advance: {reply}"
        );
    }
    assert!(reply["note"]
        .as_str()
        .unwrap()
        .contains("war_unit_finishing_survival_rejections=1"));
}

#[test]
fn local_barbarian_finishing_volley_keeps_exported_hp_and_proves_the_kill() {
    let (snapshot, state) = local_barbarian_defense_board();
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let barbarian = mirror.game.barb_pid.unwrap();
    let hostile = mirror
        .game
        .units
        .values()
        .find(|unit| unit.owner == barbarian)
        .unwrap()
        .id;
    let mut planned = mirror.game.clone();

    let volley = finish_live_war_units(&mut planned, 0, &mirror.civ6_of);
    assert_eq!(volley.targets, 1);
    assert!(
        volley.actions.len() >= 2,
        "the host/model safety margin keeps at least two direct attackers: {:?}",
        volley.actions
    );
    assert!(
        !planned.units.contains_key(&hostile),
        "the exact planning model must prove the target dies before pre-empting movement"
    );
    assert_eq!(
        mirror.game.units[&hostile].hp, 64,
        "the exported board must retain Firaxis's actual damage"
    );
}

#[test]
fn dying_barbarian_gets_a_direct_attack_and_a_reserve_before_any_movement() {
    let (snapshot, mut state) = local_barbarian_defense_board();
    state.hostiles[0].hp = 8.0;
    let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let mut ai = civvis::ai::AdvancedAi::new();
    let mut ours = std::collections::BTreeMap::new();

    let reply: serde_json::Value = serde_json::from_str(&decide(
        &mut mirror,
        &mut ai,
        &snapshot,
        &state,
        default_decision_arm(),
        DecisionMemory {
            ours: &mut ours,
            host_peace_retries: &mut HostPeaceRetries::default(),
            host_move_refusals: &mut HostMoveRefusals::default(),
            host_order_refusals: &mut HostOrderRefusals::default(),
            air_assault_continuation: &mut air_assault_continuation::Continuation::default(),
            governor_exchange: &mut governor_exchange::Continuation::default(),
        },
    ))
    .unwrap();
    let attacks = reply["orders"]
        .as_array()
        .unwrap()
        .iter()
        .take_while(|order| {
            matches!(order["verb"].as_str(), Some("ATTACK" | "RANGE_ATTACK"))
                && order["x"] == 5
                && order["y"] == 4
        })
        .collect::<Vec<_>>();
    assert_eq!(
        attacks.len(),
        2,
        "one modelled kill and one host-side reserve must lead the order list: {}",
        reply["orders"]
    );
    assert!(reply["note"]
        .as_str()
        .unwrap()
        .contains("war_unit_finishing_volleys=1 attacks=2 reserves=1"));
}

#[test]
fn dying_war_enemy_gets_a_direct_attack_and_a_reserve_before_any_movement() {
    let (snapshot, mut state) = local_war_enemy_board();
    state.rivals[0].units[0].hp = 8.0;
    let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let (target, owner, target_pos) = mirror
        .game
        .units
        .values()
        .find(|unit| unit.owner != 0)
        .map(|unit| (unit.id, unit.owner, unit.pos))
        .expect("the rival warrior is mirrored");
    assert_ne!(Some(owner), mirror.game.barb_pid);
    assert!(mirror.game.is_at_war(0, owner));

    let mut planned = mirror.game.clone();
    let volley = finish_live_war_units(&mut planned, 0, &mirror.civ6_of);
    assert_eq!(volley.targets, 1);
    assert!(
        volley.actions.len() >= 2,
        "one modelled kill and one host-side reserve must be retained: {:?}",
        volley.actions
    );
    assert!(
        !planned.units.contains_key(&target),
        "a current-war unit on 8 health must be removed on the private board before movement"
    );

    let mut ai = civvis::ai::AdvancedAi::new();
    let mut ours = std::collections::BTreeMap::new();
    let reply: serde_json::Value = serde_json::from_str(&decide(
        &mut mirror,
        &mut ai,
        &snapshot,
        &state,
        default_decision_arm(),
        DecisionMemory {
            ours: &mut ours,
            host_peace_retries: &mut HostPeaceRetries::default(),
            host_move_refusals: &mut HostMoveRefusals::default(),
            host_order_refusals: &mut HostOrderRefusals::default(),
            air_assault_continuation: &mut air_assault_continuation::Continuation::default(),
            governor_exchange: &mut governor_exchange::Continuation::default(),
        },
    ))
    .unwrap();
    let (x, y) = civvis::hex::axial_to_offset(target_pos.0, target_pos.1);
    let attacks = reply["orders"]
        .as_array()
        .unwrap()
        .iter()
        .take_while(|order| {
            matches!(order["verb"].as_str(), Some("ATTACK" | "RANGE_ATTACK"))
                && order["x"] == x
                && order["y"] == y
        })
        .collect::<Vec<_>>();
    assert_eq!(
        attacks.len(),
        2,
        "the finishing volley must lead every ordinary movement order: {}",
        reply["orders"]
    );
    assert!(reply["note"]
        .as_str()
        .unwrap()
        .contains("war_unit_finishing_volleys=1 attacks=2 reserves=1"));
}

#[test]
fn a_wounded_peacetime_enemy_does_not_preempt_the_tactical_planner() {
    let (snapshot, mut state) = local_war_enemy_board();
    state.rivals[0].at_war = false;
    state.rivals[0].units[0].hp = 8.0;
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let target = mirror
        .game
        .units
        .values()
        .find(|unit| unit.owner != 0)
        .expect("the rival warrior is mirrored");
    assert!(!mirror.game.is_at_war(0, target.owner));
    let target = target.id;
    let mut planned = mirror.game.clone();

    let volley = finish_live_war_units(&mut planned, 0, &mirror.civ6_of);
    assert_eq!(volley.targets, 0);
    assert!(volley.actions.is_empty());
    assert!(
        planned.units.contains_key(&target),
        "an enemy outside a current war must stay with the ordinary planner"
    );
}

#[test]
fn healthy_barbarian_does_not_preempt_the_ordinary_tactical_planner() {
    let (snapshot, state) = local_barbarian_defense_board();
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let barbarian = mirror.game.barb_pid.unwrap();
    let hostile = mirror
        .game
        .units
        .values()
        .find(|unit| unit.owner == barbarian)
        .unwrap()
        .id;
    let mut planned = mirror.game.clone();
    planned.units.get_mut(&hostile).unwrap().hp = 100;

    let volley = finish_live_war_units(&mut planned, 0, &mirror.civ6_of);
    assert_eq!(volley.targets, 0);
    assert!(volley.actions.is_empty());
    assert_eq!(planned.units[&hostile].hp, 100);
}

#[test]
fn reachable_melee_finish_collapses_approach_and_attack_into_one_host_order() {
    let (snapshot, mut state) = local_barbarian_defense_board();
    state.hostiles[0].hp = 1.0;
    state.units.retain(|unit| unit.id == 102);
    let chariot = state.units.iter_mut().find(|unit| unit.id == 102).unwrap();
    chariot.kind = "UNIT_HEAVY_CHARIOT".to_string();
    chariot.x = 5;
    chariot.y = 6;
    chariot.moves = 4.0;

    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let chariot = mirror
        .civ6_of
        .iter()
        .find_map(|(unit, civ6)| (*civ6 == 102).then_some(*unit))
        .unwrap();
    let target = civvis::hex::offset_to_axial(5, 4);
    assert!(
        mirror.game.wdist(mirror.game.units[&chariot].pos, target) > 1,
        "the host-order collapse is only meaningful when an approach is required"
    );
    let mut planned = mirror.game.clone();

    let volley = finish_live_war_units(&mut planned, 0, &mirror.civ6_of);
    assert_eq!(volley.targets, 1);
    assert_eq!(
        volley.actions,
        vec![Action::Attack {
            unit: chariot,
            target
        }],
        "the live host receives its native move-to-enemy operation, not a \
             deferred MOVE_TO followed by an attack next turn"
    );
    assert_eq!(
        planned.units[&chariot].pos, target,
        "the private CIVVIS board still simulates and scores the full approach"
    );
}

/// The same blow from beyond this turn's movement is no finishing line:
/// the approach would stop short, so nothing is sent and the private
/// board does not move the unit.
#[test]
fn a_melee_finish_beyond_this_turns_movement_is_not_proposed() {
    let (snapshot, mut state) = local_barbarian_defense_board();
    state.hostiles[0].hp = 1.0;
    state.units.retain(|unit| unit.id == 102);
    let chariot = state.units.iter_mut().find(|unit| unit.id == 102).unwrap();
    chariot.kind = "UNIT_HEAVY_CHARIOT".to_string();
    chariot.x = 5;
    chariot.y = 9;
    chariot.moves = 1.0;

    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let chariot = mirror
        .civ6_of
        .iter()
        .find_map(|(unit, civ6)| (*civ6 == 102).then_some(*unit))
        .unwrap();
    let target = civvis::hex::offset_to_axial(5, 4);
    let start = mirror.game.units[&chariot].pos;
    assert!(
        !mirror
            .game
            .reachable(chariot)
            .iter()
            .any(|pos| mirror.game.wdist(*pos, target) <= 1),
        "one movement point cannot end beside the target"
    );
    let mut planned = mirror.game.clone();

    let volley = finish_live_war_units(&mut planned, 0, &mirror.civ6_of);
    assert!(volley.actions.is_empty(), "{:?}", volley.actions);
    assert_eq!(planned.units[&chariot].pos, start);
}

/// A strike the host refused this turn (`range_attack_refused` /
/// `war_refused`) reaches the mirrored board as `Game::blocked_strikes`,
/// and a later frame of the same turn neither enumerates, proves nor
/// applies the identical pair — while the same unit's other targets and
/// other units' strikes at the same target stay open.
#[test]
fn a_strike_the_host_refused_is_not_proposed_again_this_turn() {
    let (snapshot, mut state) = local_barbarian_defense_board();
    let open = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let by_host = |mirror: &civvis::mirror::LiveMirror, host: i64| {
        mirror
            .civ6_of
            .iter()
            .find_map(|(unit, civ6)| (*civ6 == host).then_some(*unit))
            .unwrap()
    };
    let target = civvis::hex::offset_to_axial(5, 4);
    let warrior = by_host(&open, 101);
    let strike = Action::Attack {
        unit: warrior,
        target,
    };
    assert!(
        open.game.legal_actions(0).contains(&strike),
        "the adjacent warrior's melee strike is legal on the open board"
    );
    let mut planned = open.game.clone();
    let volley = finish_live_war_units(&mut planned, 0, &open.civ6_of);
    assert!(
        volley.actions.contains(&strike),
        "the finishing pre-pass sends the warrior's blow on the open board: {:?}",
        volley.actions
    );

    // Frame 1 of the same turn: the host refused exactly that pair.
    state.frame = 1;
    state.refused_strikes.insert((101, 5, 4));
    let blocked = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let warrior = by_host(&blocked, 101);
    let strike = Action::Attack {
        unit: warrior,
        target,
    };
    assert!(
        blocked.game.blocked_strikes.contains(&(warrior, target)),
        "the refused pair reaches the board in CIVVIS ids and axial tiles"
    );
    assert!(
        !blocked.game.legal_actions(0).contains(&strike),
        "the refused strike is not enumerated again"
    );
    assert!(blocked.game.strike_blocked(warrior, target));
    let mut replan = blocked.game.clone();
    assert!(
            replan.apply(0, &strike).is_err(),
            "the applier refuses it too, so a pre-pass proving it on a private board cannot re-issue it"
        );
    let volley = finish_live_war_units(&mut replan, 0, &blocked.civ6_of);
    let re_issued = volley.actions.iter().any(|action| {
        matches!(
            action,
            Action::Attack { unit, target: to } | Action::Ranged { unit, target: to }
                if *unit == warrior && *to == target
        )
    });
    assert!(
        !re_issued,
        "the replan frame does not re-issue the refused strike: {:?}",
        volley.actions
    );
    // The block is the exact pair, not the unit and not the target.
    let slinger = by_host(&blocked, 100);
    assert!(
        !blocked.game.blocked_strikes.contains(&(slinger, target)),
        "another unit's strike at the same target is not blocked"
    );
    assert!(
        !blocked
            .game
            .blocked_strikes
            .iter()
            .any(|(unit, to)| *unit == warrior && *to != target),
        "the same unit's other targets are not blocked"
    );
}

#[test]
fn condemn_verification_requires_the_native_target_removal() {
    let (tiles, before) = local_barbarian_defense_board();
    let order = IssuedOrder {
        kind: "unit".to_string(),
        subject: Some(101),
        verb: Some("CONDEMN_HERETIC".to_string()),
        pos: None,
    };
    let verdict = |after: &civvis::mirror::StateSnapshot, evidence: &[serde_json::Value]| {
        verify_unit_order(
            &order,
            30,
            &before,
            after,
            &tiles,
            evidence,
            LaterFrames::default(),
        )
    };
    assert!(EVIDENCE_KINDS.contains(&"condemn_removed"));
    let removal = serde_json::json!({
        "kind": "condemn_removed", "turn": 30, "unit": 101,
        "target_player": 2, "target": 201, "x": 4, "y": 4
    });
    assert!(matches!(
        verdict(&before, std::slice::from_ref(&removal)),
        Verdict::Verified
    ));
    assert!(matches!(verdict(&before, &[]),
            Verdict::Failed(reason) if reason == "no_condemn_removal"));
    for (key, wrong) in [
        ("turn", serde_json::json!(29)),
        ("unit", serde_json::json!(102)),
        ("kind", serde_json::json!("unit_lost")),
        ("target", serde_json::Value::Null),
        ("target_player", serde_json::Value::Null),
    ] {
        let mut unrelated = removal.clone();
        unrelated[key] = wrong;
        assert!(matches!(verdict(&before, &[unrelated]), Verdict::Failed(_)));
    }
    let mut actor_lost = before.clone();
    actor_lost.units.retain(|unit| unit.id != 101);
    assert!(matches!(verdict(&actor_lost, &[]), Verdict::Failed(_)));
}

/// An ATTACK/RANGE_ATTACK that left its target unharmed verifies with the
/// host's reason when the host gave one this turn for exactly this pair:
/// `would_declare_war` for a war-starter refusal, the host's `why` as a
/// token for a ranged refusal, and `target_unharmed` when it gave none.
#[test]
fn a_refused_strike_verifies_with_the_hosts_reason() {
    let (tiles, before) = local_barbarian_defense_board();
    let after = before.clone();
    let verdict = |verb: &str, evidence: &[serde_json::Value]| {
        let order = IssuedOrder {
            kind: "unit".to_string(),
            subject: Some(101),
            verb: Some(verb.to_string()),
            pos: Some((5, 4)),
        };
        verify_unit_order(
            &order,
            30,
            &before,
            &after,
            &tiles,
            evidence,
            LaterFrames::default(),
        )
    };
    let event = |json: &str| serde_json::from_str::<serde_json::Value>(json).unwrap();

    assert!(matches!(
        verdict("ATTACK", &[]),
        Verdict::Failed(reason) if reason == "target_unharmed"
    ));
    let war = event(
        r#"{"kind":"war_refused","turn":30,"unit":101,"verb":"ATTACK","x":5,"y":4,"players":[3],"target_owner":3}"#,
    );
    assert!(matches!(
        verdict("ATTACK", std::slice::from_ref(&war)),
        Verdict::Failed(reason) if reason == "would_declare_war"
    ));
    let los = event(
        r#"{"kind":"range_attack_refused","turn":30,"unit":101,"x":5,"y":4,"moves":2,"attacks":1,"activity":"ACTIVITY_AWAKE","why":"No line of sight [p4r]"}"#,
    );
    assert!(matches!(
        verdict("RANGE_ATTACK", std::slice::from_ref(&los)),
        Verdict::Failed(reason) if reason == "host_refused_no_line_of_sight"
    ));
    let wordless = event(
        r#"{"kind":"range_attack_refused","turn":30,"unit":101,"x":5,"y":4,"why":"can_start=false,no_reasons [p4r]"}"#,
    );
    assert!(matches!(
        verdict("RANGE_ATTACK", std::slice::from_ref(&wordless)),
        Verdict::Failed(reason) if reason == "host_refused_strike"
    ));
    // Damage at the requested plot is not enough: another unit may have
    // caused it while this attack was refused. The refusal remains the
    // authoritative outcome for the requested attacker/target pair.
    let mut collateral = before.clone();
    collateral.hostiles[0].hp = 40.0;
    let collateral_combat = event(
        r#"{"kind":"combat","turn":30,"attacker":{"id":202},"defender":{"id":0,"x":5,"y":4}}"#,
    );
    let collateral_order = IssuedOrder {
        kind: "unit".to_string(),
        subject: Some(101),
        verb: Some("RANGE_ATTACK".to_string()),
        pos: Some((5, 4)),
    };
    assert!(matches!(
        verify_unit_order(
            &collateral_order,
            30,
            &before,
            &collateral,
            &tiles,
            std::slice::from_ref(&collateral_combat),
            LaterFrames::default(),
        ),
        Verdict::Failed(reason) if reason == "attacker_unproven"
    ));
    assert!(matches!(
        verify_unit_order(
            &collateral_order,
            30,
            &before,
            &collateral,
            &tiles,
            &[wordless.clone(), collateral_combat],
            LaterFrames::default(),
        ),
        Verdict::Failed(reason) if reason == "host_refused_strike"
    ));
    // Another plot's, another turn's or another unit's refusal is not ours.
    let elsewhere = event(
        r#"{"kind":"range_attack_refused","turn":30,"unit":101,"x":6,"y":4,"why":"Target is out of range [p4r]"}"#,
    );
    let last_turn = event(
        r#"{"kind":"war_refused","turn":29,"unit":101,"verb":"ATTACK","x":5,"y":4,"players":[3]}"#,
    );
    let theirs = event(
        r#"{"kind":"war_refused","turn":30,"unit":100,"verb":"ATTACK","x":5,"y":4,"players":[3]}"#,
    );
    assert!(matches!(
        verdict("ATTACK", &[elsewhere, last_turn, theirs]),
        Verdict::Failed(reason) if reason == "target_unharmed"
    ));
    // A refusal beside a combat on the ledger: the refusal was an earlier
    // frame's and the combat is the proof the strike landed.
    let fought = event(
        r#"{"kind":"combat","turn":30,"attacker":{"id":101},"defender":{"id":0,"x":5,"y":4}}"#,
    );
    assert!(matches!(
        verdict("ATTACK", &[war, fought]),
        Verdict::Verified
    ));
}

/// A move onto an enemy civilian crosses as `CAPTURE`, the attack-modifier
/// MOVE_TO the host needs to enter an occupied plot; a move onto open
/// ground stays `MOVE_TO`. Measured reason: 65 bare `MOVE_TO` orders at a
/// barbarian-held settler's tile across 273 live runs, zero captures.
#[test]
fn a_move_onto_an_enemy_civilian_crosses_as_a_capture() {
    let (snapshot, mut state) = local_barbarian_defense_board();
    let settler = &mut state.hostiles[0];
    settler.kind = "UNIT_SETTLER".to_string();
    settler.combat = 0.0;
    settler.hp = 100.0;
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let barbarian = mirror.game.barb_pid.unwrap();
    assert!(
        mirror
            .game
            .units
            .values()
            .any(|unit| unit.owner == barbarian && unit.kind == "settler"),
        "the barbarian-held settler must reach the board"
    );
    let warrior = mirror
        .civ6_of
        .iter()
        .find_map(|(unit, civ6)| (*civ6 == 101).then_some(*unit))
        .unwrap();

    let capture = translate(
        &Action::Move {
            unit: warrior,
            to: civvis::hex::offset_to_axial(5, 4),
        },
        &mirror,
        &state,
    )
    .expect("the capture crosses");
    assert_eq!(capture.verb.as_deref(), Some("CAPTURE"));
    assert_eq!(capture.pos, Some((5, 4)));
    assert_eq!(capture.subject, Some(101));

    let walk = translate(
        &Action::Move {
            unit: warrior,
            to: civvis::hex::offset_to_axial(3, 3),
        },
        &mirror,
        &state,
    )
    .expect("the walk crosses");
    assert_eq!(walk.verb.as_deref(), Some("MOVE_TO"));
}

/// A raider walks and then pillages (`air_surge/raids.rs`), so the plot
/// it ends the turn on witnesses the PILLAGE as well as the one it
/// started on. A still-unpillaged pair of plots is a failure.
#[test]
fn a_pillage_is_verified_on_the_plot_the_unit_ends_on() {
    let (_, before) = local_barbarian_defense_board();
    let order = IssuedOrder {
        kind: "unit".to_string(),
        subject: Some(101),
        verb: Some("PILLAGE".to_string()),
        pos: None,
    };
    let board = |pillaged: Option<(i32, i32)>| {
        Snapshot::from_chunks(&[TilesChunk {
            turn: 30,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| (x, y)))
                .map(|(x, y)| {
                    serde_json::from_value::<Plot>(serde_json::json!({
                        "x": x, "y": y, "t": "TERRAIN_GRASS",
                        "p": pillaged == Some((x, y)),
                    }))
                    .expect("a plot with serde defaults deserializes")
                })
                .collect(),
        }])
    };
    let mut walked = before.clone();
    let raider = walked.units.iter_mut().find(|unit| unit.id == 101).unwrap();
    raider.x = 7;
    raider.y = 5;
    let verdict = |tiles: &Snapshot| {
        verify_unit_order(
            &order,
            30,
            &before,
            &walked,
            tiles,
            &[],
            LaterFrames::default(),
        )
    };
    assert!(matches!(verdict(&board(Some((7, 5)))), Verdict::Verified));
    assert!(matches!(verdict(&board(Some((5, 5)))), Verdict::Verified));
    assert!(matches!(
        verdict(&board(None)),
        Verdict::Failed(reason) if reason == "not_pillaged"
    ));
}

/// A capture is verified by the unit standing where the civilian stood,
/// or by the civilian gone from there; a unit still beside it failed.
#[test]
fn a_capture_is_verified_by_arrival_or_by_the_civilian_gone() {
    let (tiles, mut before) = local_barbarian_defense_board();
    let settler = &mut before.hostiles[0];
    settler.kind = "UNIT_SETTLER".to_string();
    settler.combat = 0.0;
    let order = IssuedOrder {
        kind: "unit".to_string(),
        subject: Some(101),
        verb: Some("CAPTURE".to_string()),
        pos: Some((5, 4)),
    };

    let mut arrived = before.clone();
    arrived.hostiles.clear();
    let warrior = arrived
        .units
        .iter_mut()
        .find(|unit| unit.id == 101)
        .unwrap();
    warrior.x = 5;
    warrior.y = 4;
    assert!(matches!(
        verify_unit_order(
            &order,
            30,
            &before,
            &arrived,
            &tiles,
            &[],
            LaterFrames::default()
        ),
        Verdict::Verified
    ));

    let mut gone = before.clone();
    gone.hostiles.clear();
    assert!(matches!(
        verify_unit_order(
            &order,
            30,
            &before,
            &gone,
            &tiles,
            &[],
            LaterFrames::default()
        ),
        Verdict::Verified
    ));

    let stood = before.clone();
    assert!(matches!(
        verify_unit_order(&order, 30, &before, &stood, &tiles, &[], LaterFrames::default()),
        Verdict::Failed(reason) if reason == "not_captured"
    ));
}

/// `Action::Swap` crosses as `SWAP` aimed at the PARTNER'S tile — the
/// positional pair the shipped `Civ6Common.lua:160-161` hands
/// `UNITOPERATION_SWAP_UNITS` — instead of falling through to
/// `unit_action_untranslated`.
#[test]
fn a_coastal_raid_crosses_as_its_verb_aimed_at_the_shore() {
    let (snapshot, state) = local_barbarian_defense_board();
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let (unit, host) = mirror.civ6_of.iter().next().map(|(u, h)| (*u, *h)).unwrap();
    let target = mirror.game.units[&unit].pos;
    let raid = translate(&Action::CoastalRaid { unit, target }, &mirror, &state)
        .expect("the raid crosses");
    assert_eq!(raid.kind, "unit");
    assert_eq!(raid.subject, Some(host));
    assert_eq!(raid.verb.as_deref(), Some("COASTAL_RAID"));
    assert_eq!(
        raid.pos,
        Some(civvis::hex::axial_to_offset(target.0, target.1))
    );
}

/// The two launches are different calls in the host: a unit operation for a
/// carrier, and a city command aimed from the silo's plot.
#[test]
fn a_launch_crosses_as_the_call_its_platform_needs() {
    let (snapshot, state) = local_barbarian_defense_board();
    let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let (host_city, city) = mirror.cid_of.iter().next().map(|(h, c)| (*h, *c)).unwrap();
    let home = mirror.game.cities[&city].pos;
    let strike = Action::WmdStrike {
        city,
        target: home,
        thermonuclear: false,
    };
    assert!(
        translate(&strike, &mirror, &state).is_none(),
        "no platform, no order"
    );

    let (unit, host_unit) = mirror.civ6_of.iter().next().map(|(u, h)| (*u, *h)).unwrap();
    let carrier = mirror.game.units.get_mut(&unit).unwrap();
    carrier.kind = civvis::name!("bomber");
    carrier.pos = home;
    carrier.attacks_left = 1;
    carrier.moves_left = 10.0;
    let flown = translate(&strike, &mirror, &state).expect("a bomber in range launches");
    assert_eq!(flown.kind, "unit");
    assert_eq!(flown.subject, Some(host_unit));
    assert_eq!(flown.verb.as_deref(), Some("WMD_NUCLEAR_DEVICE"));
    assert_eq!(
        flown.pos,
        Some(civvis::hex::axial_to_offset(home.0, home.1))
    );

    mirror.game.units.get_mut(&unit).unwrap().attacks_left = 0;
    let tile = mirror.game.map.tiles.get_mut(&home).unwrap();
    tile.improvement = Some(civvis::name!("missile_silo"));
    tile.pillaged = false;
    let owned = &mut mirror.game.cities.get_mut(&city).unwrap().owned_tiles;
    if !owned.contains(&home) {
        owned.push(home);
    }
    let thermo = Action::WmdStrike {
        city,
        target: home,
        thermonuclear: true,
    };
    let silo = translate(&thermo, &mirror, &state).expect("a silo launches");
    let (x, y) = civvis::hex::axial_to_offset(home.0, home.1);
    assert_eq!(silo.kind, "wmd_strike");
    assert_eq!(silo.subject, Some(host_city));
    assert_eq!(
        silo.verb.as_deref(),
        Some(format!("{x}:{y}:WMD_THERMONUCLEAR_DEVICE").as_str())
    );
}

#[test]
fn a_swap_crosses_as_the_swap_verb_aimed_at_the_partners_tile() {
    let (snapshot, state) = local_barbarian_defense_board();
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let by_host = |host: i64| {
        mirror
            .civ6_of
            .iter()
            .find_map(|(unit, civ6)| (*civ6 == host).then_some(*unit))
            .unwrap()
    };
    let warrior = by_host(101);
    let slinger = by_host(100);
    assert_eq!(
        mirror.game.wdist(
            mirror.game.units[&warrior].pos,
            mirror.game.units[&slinger].pos
        ),
        1,
        "the board's two friendly units are adjacent, as a swap requires"
    );

    let swap = translate(
        &Action::Swap {
            unit: warrior,
            other: slinger,
        },
        &mirror,
        &state,
    )
    .expect("the swap crosses");
    assert_eq!(swap.kind, "unit");
    assert_eq!(swap.subject, Some(101));
    assert_eq!(swap.verb.as_deref(), Some("SWAP"));
    assert_eq!(
        swap.pos,
        Some((4, 5)),
        "the partner's plot, in host offsets"
    );

    // A partner the mirror cannot see has no plot to name.
    assert!(translate(
        &Action::Swap {
            unit: warrior,
            other: u32::MAX,
        },
        &mirror,
        &state,
    )
    .is_none());
}

/// Either half of the exchange verifies a swap: this unit on the partner's
/// former tile, or the partner on ours. Both still where they stood is
/// `not_swapped`.
#[test]
fn a_swap_is_verified_by_either_unit_standing_on_the_others_tile() {
    let (tiles, before) = local_barbarian_defense_board();
    // Warrior 101 at (5,5) swaps with Slinger 100 at (4,5).
    let order = IssuedOrder {
        kind: "unit".to_string(),
        subject: Some(101),
        verb: Some("SWAP".to_string()),
        pos: Some((4, 5)),
    };
    let verdict = |after: &StateSnapshot| {
        verify_unit_order(
            &order,
            30,
            &before,
            after,
            &tiles,
            &[],
            LaterFrames::default(),
        )
    };

    let mut swapped = before.clone();
    for unit in swapped.units.iter_mut() {
        match unit.id {
            101 => (unit.x, unit.y) = (4, 5),
            100 => (unit.x, unit.y) = (5, 5),
            _ => {}
        }
    }
    assert!(matches!(verdict(&swapped), Verdict::Verified));

    // The partner's half alone is enough evidence.
    let mut partner_only = before.clone();
    let slinger = partner_only
        .units
        .iter_mut()
        .find(|unit| unit.id == 100)
        .unwrap();
    (slinger.x, slinger.y) = (5, 5);
    assert!(matches!(verdict(&partner_only), Verdict::Verified));

    let stood = before.clone();
    assert!(matches!(
        verdict(&stood),
        Verdict::Failed(reason) if reason == "not_swapped"
    ));

    let mut gone = before.clone();
    gone.units.retain(|unit| unit.id != 101);
    assert!(matches!(
        verdict(&gone),
        Verdict::Failed(reason) if reason == "unit_gone"
    ));
}

/// The three air actions cross as their own verbs, aimed at the plot the
/// shipped `WorldInput.lua` hands each operation — the target for
/// `AIR_ATTACK` (`:2077`), the new base for `REBASE` (`:2486`), the patrol
/// plot for `PATROL` (`:2418`, the shipped Deploy) — instead of falling
/// through to `unit_action_untranslated`.
#[test]
fn the_air_actions_cross_as_air_attack_rebase_and_patrol() {
    let (snapshot, state) = local_barbarian_defense_board();
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let slinger = mirror
        .civ6_of
        .iter()
        .find_map(|(unit, civ6)| (*civ6 == 100).then_some(*unit))
        .unwrap();
    let plot = |x, y| civvis::hex::offset_to_axial(x, y);
    let crossed = |action: Action| {
        let order = translate(&action, &mirror, &state).expect("the air action crosses");
        (order.kind, order.subject, order.verb, order.pos)
    };
    assert_eq!(
        crossed(Action::AirStrike {
            unit: slinger,
            target: plot(5, 4),
        }),
        (
            "unit",
            Some(100),
            Some("AIR_ATTACK".to_string()),
            Some((5, 4))
        ),
        "the target plot, in host offsets"
    );
    assert_eq!(
        crossed(Action::AirRebase {
            unit: slinger,
            to: plot(4, 4),
        }),
        ("unit", Some(100), Some("REBASE".to_string()), Some((4, 4))),
        "the new base plot"
    );
    assert_eq!(
        crossed(Action::AirPatrol {
            unit: slinger,
            to: plot(7, 7),
        }),
        ("unit", Some(100), Some("PATROL".to_string()), Some((7, 7))),
        "the plot to fly to and intercept from"
    );
    // An aircraft the mirror cannot map has no host id to order.
    assert!(translate(
        &Action::AirStrike {
            unit: u32::MAX,
            target: plot(5, 4),
        },
        &mirror,
        &state,
    )
    .is_none());
}

/// An air strike verifies like a ranged one: the target plot harmed on
/// the next frame — hp down, or the unit gone — is the sortie; an
/// untouched target is `target_unharmed`.
#[test]
fn an_air_attack_is_verified_by_the_harmed_target() {
    let (tiles, before) = local_barbarian_defense_board();
    // Slinger 100 stands in for the aircraft; the barbarian at (5,4) is
    // the target.
    let order = IssuedOrder {
        kind: "unit".to_string(),
        subject: Some(100),
        verb: Some("AIR_ATTACK".to_string()),
        pos: Some((5, 4)),
    };
    let verdict = |after: &StateSnapshot| {
        verify_unit_order(
            &order,
            30,
            &before,
            after,
            &tiles,
            &[],
            LaterFrames::default(),
        )
    };
    let mut harmed = before.clone();
    harmed.hostiles[0].hp = 30.0;
    assert!(matches!(verdict(&harmed), Verdict::Verified));
    let mut destroyed = before.clone();
    destroyed.hostiles.clear();
    assert!(matches!(verdict(&destroyed), Verdict::Verified));
    assert!(matches!(
        verdict(&before),
        Verdict::Failed(reason) if reason == "target_unharmed"
    ));
}

/// A rebase is the aircraft standing on the base plot next frame — the
/// shipped REBASE is a teleport that spends the turn — else `not_rebased`;
/// a patrol (the shipped Deploy) is declared unverifiable because the
/// frame carries no patrol state.
#[test]
fn a_rebase_is_verified_on_the_base_plot_and_a_patrol_is_unverifiable() {
    let (tiles, before) = local_barbarian_defense_board();
    let rebase = IssuedOrder {
        kind: "unit".to_string(),
        subject: Some(100),
        verb: Some("REBASE".to_string()),
        pos: Some((4, 4)),
    };
    let verdict = |order: &IssuedOrder, after: &StateSnapshot| {
        verify_unit_order(
            order,
            30,
            &before,
            after,
            &tiles,
            &[],
            LaterFrames::default(),
        )
    };
    let mut rebased = before.clone();
    let aircraft = rebased.units.iter_mut().find(|u| u.id == 100).unwrap();
    (aircraft.x, aircraft.y) = (4, 4);
    assert!(matches!(verdict(&rebase, &rebased), Verdict::Verified));
    assert!(matches!(
        verdict(&rebase, &before),
        Verdict::Failed(reason) if reason == "not_rebased"
    ));
    let mut gone = before.clone();
    gone.units.retain(|u| u.id != 100);
    assert!(matches!(
        verdict(&rebase, &gone),
        Verdict::Failed(reason) if reason == "unit_gone"
    ));

    let patrol = IssuedOrder {
        verb: Some("PATROL".to_string()),
        ..rebase.clone()
    };
    assert_eq!(verdict(&patrol, &before), Verdict::Unverifiable);
    assert!(unverifiable_unit_verb("PATROL"));
}

#[test]
fn great_people_activate_or_move_to_the_nearest_firaxis_valid_plot() {
    let state = StateSnapshot {
        units: vec![
            StateUnit {
                id: 70,
                kind: "UNIT_GREAT_SCIENTIST".to_string(),
                great_person: Some(StateGreatPerson {
                    charges: 0,
                    can_activate: true,
                    ..StateGreatPerson::default()
                }),
                ..StateUnit::default()
            },
            StateUnit {
                id: 71,
                kind: "UNIT_GREAT_MERCHANT".to_string(),
                great_person: Some(StateGreatPerson {
                    charges: 1,
                    activation_plots: vec![
                        StateActivationPlot {
                            x: 30,
                            y: 20,
                            distance: 7,
                            ..StateActivationPlot::default()
                        },
                        StateActivationPlot {
                            x: 11,
                            y: 12,
                            distance: 2,
                            ..StateActivationPlot::default()
                        },
                    ],
                    ..StateGreatPerson::default()
                }),
                ..StateUnit::default()
            },
        ],
        ..StateSnapshot::default()
    };

    let (orders, stall) = great_person_orders(&state);

    assert_eq!(stall.total(), 0);
    assert_eq!(orders.len(), 2);
    assert_eq!(orders[0].subject, Some(70));
    assert_eq!(orders[0].verb.as_deref(), Some("ACTIVATE_GREAT_PERSON"));
    assert_eq!(orders[1].subject, Some(71));
    assert_eq!(orders[1].verb.as_deref(), Some("MOVE_TO"));
    assert_eq!(orders[1].pos, Some((11, 12)));
}

/// The nine Great People of live run `civvis-20260822T020434Z`, replayed.
///
/// Three Artists, three Writers, three Musicians and a Scientist stood in
/// Rome at turn 231 and `orders.sqlite` holds NOT ONE ORDER for any of
/// them, ever. Their exports are reproduced here field for field: every
/// activation plot at `slot_open: false`, and `empty_slots` at 24, 4 and
/// 2 — never the zero that every escape hatch was gated on.
#[test]
fn the_rome_stack_is_starved_even_though_the_empire_owns_empty_slots() {
    // As exported. The counts are what made this invisible: an empire-wide
    // tally of slots this person cannot reach.
    let cultural = |empty: u32, plots: Vec<(i32, i32, i32)>| StateGreatPerson {
        can_activate: false,
        charges: 0,
        empty_slots: Some(empty),
        activation_plots: plots
            .into_iter()
            .map(|(x, y, distance)| StateActivationPlot {
                x,
                y,
                distance,
                slot_open: Some(false),
            })
            .collect(),
        ..StateGreatPerson::default()
    };
    let writer = cultural(24, vec![(67, 14, 12), (65, 25, 1), (64, 27, 2)]);
    let musician = cultural(4, vec![(65, 26, 1), (67, 32, 6)]);
    let artist = cultural(2, vec![(65, 26, 1)]);
    for (label, person) in [
        ("writer", &writer),
        ("musician", &musician),
        ("artist", &artist),
    ] {
        assert!(
            person.slot_starved(),
            "{label} has 24/4/2 slots the empire owns and none it can reach"
        );
    }

    // The Great Scientist of the same stack: no plots at all, waiting on
    // the Spaceport its `required_district` names. That is a missing
    // DISTRICT, not a missing slot, and it must not read as starved here —
    // the needs machinery has its own branch for it, and a work sale
    // cannot help a Scientist.
    let scientist = StateGreatPerson {
        can_activate: false,
        charges: 1,
        class: Some("GREAT_PERSON_CLASS_SCIENTIST".to_string()),
        required_district: Some("DISTRICT_SPACEPORT".to_string()),
        ..StateGreatPerson::default()
    };
    assert!(!scientist.slot_starved(), "no plot offered is not no slot");

    // The three cases the predicate must NOT change, or it trades one
    // wedge for another:
    let mut can_go = writer.clone();
    can_go.can_activate = true;
    assert!(!can_go.slot_starved(), "a person the host will activate");

    let mut one_open = writer.clone();
    one_open.activation_plots[1].slot_open = Some(true);
    assert!(
        !one_open.slot_starved(),
        "one reachable slot is not starved"
    );

    // ⚠ An older control mod sends no `slot_open` at all. `None` is an
    // absence, never a claim — it kept the benefit of the doubt before
    // this predicate existed and it still does.
    let mut older_mod = writer.clone();
    for plot in &mut older_mod.activation_plots {
        plot.slot_open = None;
    }
    older_mod.empty_slots = None;
    assert!(
        !older_mod.slot_starved(),
        "an unknowing export must not be read as starved"
    );

    // And the whole point: the driver now counts them, and the work-sale
    // arm now sees them, where before both walked past.
    let state = StateSnapshot {
        units: vec![
            StateUnit {
                id: 10092559,
                kind: "UNIT_GREAT_WRITER".to_string(),
                great_person: Some(writer),
                ..StateUnit::default()
            },
            StateUnit {
                id: 9175056,
                kind: "UNIT_GREAT_MUSICIAN".to_string(),
                great_person: Some(musician),
                ..StateUnit::default()
            },
            StateUnit {
                id: 6684677,
                kind: "UNIT_GREAT_ARTIST".to_string(),
                great_person: Some(artist),
                ..StateUnit::default()
            },
        ],
        ..StateSnapshot::default()
    };
    let (orders, stall) = great_person_orders(&state);
    assert!(
        orders.is_empty(),
        "marching to a known-full district is still never the answer"
    );
    assert_eq!(
        stall.no_empty_slot, 3,
        "all three report the real reason, not `no_activation_plot`"
    );
    assert_eq!(stall.no_activation_plot, 0);
}

/// Seven cultural people stood on one Theater plot for thirty-plus turns
/// on run civvis-20260817T010950Z: `GetActivationHighlightPlots` lists the
/// district whether or not a compatible Great Work slot is free, so the
/// cooldown branch swallowed them forever and no one built the slots.
/// With the host's own empty-slot count, a slot-starved person stands
/// still under an explicit counter — and never marches to a full building.
#[test]
fn a_person_with_no_empty_slot_anywhere_stalls_explicitly_not_as_cooldown() {
    let on_plot = StateActivationPlot {
        x: 25,
        y: 23,
        distance: 0,
        ..StateActivationPlot::default()
    };
    let far_plot = StateActivationPlot {
        x: 30,
        y: 20,
        distance: 7,
        ..StateActivationPlot::default()
    };
    let state = StateSnapshot {
        units: vec![
            // Standing on the highlighted Theater, zero writing slots free.
            StateUnit {
                id: 80,
                kind: "UNIT_GREAT_WRITER".to_string(),
                great_person: Some(StateGreatPerson {
                    empty_slots: Some(0),
                    activation_plots: vec![on_plot.clone(), far_plot.clone()],
                    ..StateGreatPerson::default()
                }),
                ..StateUnit::default()
            },
            // Slot-starved and NOT on a plot: marching cannot help either.
            StateUnit {
                id: 81,
                kind: "UNIT_GREAT_ARTIST".to_string(),
                great_person: Some(StateGreatPerson {
                    empty_slots: Some(0),
                    activation_plots: vec![far_plot],
                    ..StateGreatPerson::default()
                }),
                ..StateUnit::default()
            },
            // Slots exist: standing on the plot is the benign cooldown
            // frame, exactly as before this field existed.
            StateUnit {
                id: 82,
                kind: "UNIT_GREAT_MUSICIAN".to_string(),
                great_person: Some(StateGreatPerson {
                    empty_slots: Some(2),
                    activation_plots: vec![on_plot],
                    ..StateGreatPerson::default()
                }),
                ..StateUnit::default()
            },
        ],
        ..StateSnapshot::default()
    };

    let (orders, stall) = great_person_orders(&state);

    assert!(
        orders.is_empty(),
        "no order helps a slot-starved person, and a cooldown is a wait \
             ({} issued)",
        orders.len()
    );
    assert_eq!(stall.no_empty_slot, 2);
    assert_eq!(stall.on_cooldown, 1);
    assert_eq!(stall.no_activation_plot, 0);
    assert_eq!(stall.total(), 3);
}

/// The host answering `can_activate` outranks its slot arithmetic: if the
/// engine will take Activate here and now, press it.
#[test]
fn can_activate_outranks_a_zero_slot_count() {
    let state = StateSnapshot {
        units: vec![StateUnit {
            id: 83,
            kind: "UNIT_GREAT_WRITER".to_string(),
            great_person: Some(StateGreatPerson {
                can_activate: true,
                empty_slots: Some(0),
                ..StateGreatPerson::default()
            }),
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };
    let (orders, stall) = great_person_orders(&state);
    assert_eq!(stall.total(), 0);
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].verb.as_deref(), Some("ACTIVATE_GREAT_PERSON"));
}

#[test]
fn great_person_already_on_an_unusable_activation_plot_waits() {
    let state = StateSnapshot {
        units: vec![StateUnit {
            id: 72,
            kind: "UNIT_GREAT_WRITER".to_string(),
            great_person: Some(StateGreatPerson {
                charges: 0,
                can_activate: false,
                activation_plots: vec![
                    StateActivationPlot {
                        x: 11,
                        y: 12,
                        distance: 0,
                        ..StateActivationPlot::default()
                    },
                    StateActivationPlot {
                        x: 20,
                        y: 18,
                        distance: 7,
                        ..StateActivationPlot::default()
                    },
                ],
                ..StateGreatPerson::default()
            }),
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };

    let (orders, stall) = great_person_orders(&state);

    assert!(orders.is_empty());
    assert_eq!(
        stall.on_cooldown, 1,
        "standing on a plot is the benign wait"
    );
    assert_eq!(
        stall.no_activation_plot, 0,
        "and must not be reported as an unusable Great Person"
    );
}

/// The (25,23) wedge from run civvis-20260817T010950Z: eleven cultural
/// people stood a whole game on one highlighted tile the host KNEW held no
/// compatible empty slot, while highlighted tiles with empty Amphitheater
/// slots waited 2-10 tiles away, and the run ended with zero Great Works.
/// A known-full tile is neither a cooldown to wait out nor a destination
/// to march to; a known-open tile beats an unknown one even when farther.
#[test]
fn a_person_on_a_known_full_tile_marches_to_the_known_open_slot() {
    let full_here = StateActivationPlot {
        x: 25,
        y: 23,
        distance: 0,
        slot_open: Some(false),
    };
    let unknown_near = StateActivationPlot {
        x: 26,
        y: 23,
        distance: 1,
        slot_open: None,
    };
    let open_far = StateActivationPlot {
        x: 30,
        y: 20,
        distance: 7,
        slot_open: Some(true),
    };
    let state = StateSnapshot {
        units: vec![StateUnit {
            id: 84,
            kind: "UNIT_GREAT_WRITER".to_string(),
            great_person: Some(StateGreatPerson {
                charges: 0,
                can_activate: false,
                empty_slots: Some(2),
                activation_plots: vec![full_here, unknown_near, open_far],
                ..StateGreatPerson::default()
            }),
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };

    let (orders, stall) = great_person_orders(&state);

    assert_eq!(stall.on_cooldown, 0, "a known-full tile is not a cooldown");
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].verb.as_deref(), Some("MOVE_TO"));
    assert_eq!(
        orders[0].pos,
        Some((30, 20)),
        "the known-open slot outranks the nearer unknown tile"
    );
}

/// The case the merged counter hid. A Great Person the host offers nowhere
/// to activate is not waiting a frame — they are stranded until the empire
/// builds the district their class needs, which for a Great Scientist is a
/// Campus. Live brain logs reached 3 and 6 under the old key with no way to
/// tell this apart from a Writer pausing for one export.
#[test]
fn a_great_person_with_nowhere_to_activate_is_named_separately() {
    let state = StateSnapshot {
        units: vec![StateUnit {
            id: 73,
            kind: "UNIT_GREAT_SCIENTIST".to_string(),
            great_person: Some(StateGreatPerson {
                charges: 1,
                can_activate: false,
                activation_plots: Vec::new(),
                ..StateGreatPerson::default()
            }),
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };

    let (orders, stall) = great_person_orders(&state);

    assert!(orders.is_empty(), "there is nowhere to send them");
    assert_eq!(
        stall.no_activation_plot, 1,
        "a Great Scientist with no Campus to use is a loss, not a wait"
    );
    assert_eq!(stall.on_cooldown, 0);
    assert_eq!(stall.total(), 1, "the old key keeps its meaning");
}

#[test]
fn religious_unit_actions_reach_firaxis_unit_orders() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 93,
        width: 12,
        height: 12,
        chunk: 1,
        plots: vec![grass(5, 5)],
    }]);
    let state = StateSnapshot {
        turn: 93,
        units: vec![StateUnit {
            id: 91,
            kind: "UNIT_APOSTLE".to_string(),
            x: 5,
            y: 5,
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let unit = *mirror.uid_of.get(&91).expect("the Apostle is mirrored");

    let promotion = translate(
        &Action::Promote {
            unit,
            promotion: civvis::name!("translator"),
        },
        &mirror,
        &state,
    )
    .expect("the promotion crosses");
    assert_eq!(promotion.kind, "unit");
    assert_eq!(promotion.subject, Some(91));
    assert_eq!(
        promotion.verb.as_deref(),
        Some("PROMOTE:PROMOTION_TRANSLATOR")
    );

    let spread = translate(&Action::Spread { unit }, &mirror, &state).expect("the spread crosses");
    assert_eq!(spread.kind, "unit");
    assert_eq!(spread.subject, Some(91));
    assert_eq!(spread.verb.as_deref(), Some("SPREAD_RELIGION"));

    let evangelize = translate(
        &Action::EvangelizeBelief {
            unit,
            belief: civvis::name!("holy_order"),
        },
        &mirror,
        &state,
    )
    .expect("the Apostle belief choice crosses");
    assert_eq!(evangelize.kind, "unit");
    assert_eq!(evangelize.subject, Some(91));
    assert_eq!(
        evangelize.verb.as_deref(),
        Some("EVANGELIZE_BELIEF:BELIEF_HOLY_ORDER")
    );
    assert_eq!(evangelize.pos, None);

    let theological = translate(
        &Action::TheologicalAttack {
            unit,
            target: civvis::hex::offset_to_axial(6, 5),
        },
        &mirror,
        &state,
    )
    .expect("theological combat crosses");
    assert_eq!(theological.kind, "unit");
    assert_eq!(theological.subject, Some(91));
    assert_eq!(theological.verb.as_deref(), Some("ATTACK"));
    assert_eq!(theological.pos, Some((6, 5)));

    for (action, verb) in [
        (Action::HealReligious { unit }, "RELIGIOUS_HEAL"),
        (Action::RemoveHeresy { unit }, "REMOVE_HERESY"),
        (Action::LaunchInquisition { unit }, "LAUNCH_INQUISITION"),
        (Action::ConvertBarbarians { unit }, "CONVERT_BARBARIANS"),
    ] {
        let operation =
            translate(&action, &mirror, &state).expect("a direct religious operation crosses");
        assert_eq!(operation.kind, "unit");
        assert_eq!(operation.subject, Some(91));
        assert_eq!(operation.verb.as_deref(), Some(verb));
        assert_eq!(operation.pos, None);
    }

    // ★★★ And the one order that touches an ENEMY religious unit. It used
    // to fall through this match's `_ => None` and be counted
    // untranslatable, so a rival Apostle standing in our territory —
    // untouchable by ordinary combat, by design — could not be answered by
    // anything the bridge could send. `target_unit` deliberately does not
    // cross: the command is parameterless in Firaxis' own UnitPanel (its
    // `UnitCommands` row carries no `InterfaceMode`) and the engine
    // requires the mover to be standing on the target anyway, so the
    // co-location IS the target.
    let condemn = translate(
        &Action::CondemnHeretic {
            unit,
            target_unit: 4242,
        },
        &mirror,
        &state,
    )
    .expect("the condemnation crosses");
    assert_eq!(condemn.kind, "unit");
    assert_eq!(condemn.subject, Some(91));
    assert_eq!(condemn.verb.as_deref(), Some("CONDEMN_HERETIC"));
    assert_eq!(
        condemn.pos, None,
        "a parameterless command must not invent a destination"
    );
}

/// ★★★ Corps and Army: decided 10,015 times across the live archive and
/// sent zero times, because `CombineUnits` had no arm and fell through
/// `translate`'s `_ => None`. It is two Firaxis commands rather than one —
/// `UNITCOMMAND_FORM_CORPS` and `UNITCOMMAND_FORM_ARMY`,
/// `Gameplay/Data/UnitCommands.xml:20-21` and `:44-45` — and the tier is
/// the caller's to choose, so the verb has to carry which merge was decided
/// rather than leaving the mod to guess it from a board it cannot see.
#[test]
fn combining_units_reaches_the_form_corps_and_form_army_commands() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 118,
        width: 12,
        height: 12,
        chunk: 1,
        plots: vec![grass(4, 4), grass(5, 4)],
    }]);
    let state = StateSnapshot {
        turn: 118,
        units: vec![
            StateUnit {
                id: 41,
                kind: "UNIT_SWORDSMAN".to_string(),
                x: 4,
                y: 4,
                ..StateUnit::default()
            },
            StateUnit {
                id: 42,
                kind: "UNIT_SWORDSMAN".to_string(),
                x: 5,
                y: 4,
                ..StateUnit::default()
            },
        ],
        ..StateSnapshot::default()
    };
    let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let first = *mirror.uid_of.get(&41).expect("the first swordsman mirrors");
    let second = *mirror
        .uid_of
        .get(&42)
        .expect("the second swordsman mirrors");

    let corps = translate(
        &Action::CombineUnits {
            unit: first,
            with: second,
        },
        &mirror,
        &state,
    )
    .expect("two standard units form a Corps on the live seat");
    assert_eq!(corps.kind, "unit");
    assert_eq!(corps.subject, Some(41));
    assert_eq!(corps.verb.as_deref(), Some("FORM_CORPS"));
    assert_eq!(
        corps.pos,
        Some((state.seat.local_player, 42)),
        "x/y are the partner's OWNER and UNIT ID, the pair Firaxis's own \
             WorldInput.lua:2879-2880 puts in PARAM_UNIT_PLAYER/PARAM_UNIT_ID — \
             never a map position"
    );

    // A standard unit joining a Corps is an Army, which is a DIFFERENT
    // command with a different prerequisite civic (Mobilization, not
    // Nationalism). Sending FORM_CORPS here would be refused by the host
    // for a unit that is already one.
    mirror
        .game
        .units
        .get_mut(&first)
        .expect("the mirrored unit is addressable")
        .formation = 1;
    let army = translate(
        &Action::CombineUnits {
            unit: first,
            with: second,
        },
        &mirror,
        &state,
    )
    .expect("a Corps absorbing a standard unit forms an Army");
    assert_eq!(army.verb.as_deref(), Some("FORM_ARMY"));
    assert_eq!(army.subject, Some(41));

    // The partner is half the order. Without a Civilization VI id for it
    // there is nothing to put in PARAM_UNIT_ID, so this must be counted as
    // untranslatable rather than sent as a merge with no one to merge with.
    assert!(
        translate(
            &Action::CombineUnits {
                unit: first,
                with: 9_999,
            },
            &mirror,
            &state,
        )
        .is_none(),
        "an unmapped partner cannot be silently dropped from the command"
    );
}

/// ★★★★★ THE HOST'S OWN CORPS REACHES `FORM_ARMY`, END TO END.
///
/// The test above proves the tier decides the command; it sets the tier by
/// hand. That is exactly the half that was missing in #2373: the live seat
/// runs `--fresh-board`, the mirror is rebuilt from the host export every
/// turn, and the export said nothing about Corps or Army — so every unit was
/// reconstructed at tier 0 and this arm could only ever emit FORM_CORPS. The
/// Army half of the unit-consolidation layer was unreachable live for that
/// one reason.
///
/// This drives the whole chain instead: a `StateUnit` carrying the tier the
/// mod now exports from `Unit:GetMilitaryFormation()`, through
/// `LiveMirror::new`, into the command actually sent. Nothing is set by hand.
#[test]
fn a_corps_the_host_exported_reaches_form_army_from_a_fresh_board() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 152,
        width: 12,
        height: 12,
        chunk: 1,
        plots: vec![grass(4, 4), grass(5, 4)],
    }]);
    // Exactly what the host export now says: 1 = Corps, and the escort stack
    // size stays 1 because a Corps is ONE unit. The two fields share a word
    // and nothing else.
    let state = StateSnapshot {
        turn: 152,
        units: vec![
            StateUnit {
                id: 71,
                kind: "UNIT_SWORDSMAN".to_string(),
                x: 4,
                y: 4,
                formation: Some(1),
                formation_count: 1,
                ..StateUnit::default()
            },
            StateUnit {
                id: 72,
                kind: "UNIT_SWORDSMAN".to_string(),
                x: 5,
                y: 4,
                formation: Some(0),
                formation_count: 1,
                ..StateUnit::default()
            },
        ],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let corps = *mirror.uid_of.get(&71).expect("the Corps mirrors");
    let standard = *mirror.uid_of.get(&72).expect("the swordsman mirrors");
    assert_eq!(
        mirror.game.units[&corps].formation, 1,
        "the host said Corps, so the mirror must hold a Corps"
    );
    assert_eq!(mirror.game.units[&standard].formation, 0);

    let order = translate(
        &Action::CombineUnits {
            unit: corps,
            with: standard,
        },
        &mirror,
        &state,
    )
    .expect("a host Corps absorbing a standard unit is an Army");
    assert_eq!(
        order.verb.as_deref(),
        Some("FORM_ARMY"),
        "a Corps the host itself reported must not be sent FORM_CORPS again — \
             they are different commands behind different civics (Mobilization, \
             not Nationalism), so the host refuses the wrong one"
    );
    assert_eq!(order.subject, Some(71));
    assert_eq!(order.pos, Some((state.seat.local_player, 72)));

    // ⚠ And the same pair with the tier UNREAD stays a Corps request. The
    // mod's −1 means "asked, could not answer"; the mirror keeps the board's
    // own value rather than claiming standard, and a board that knows nothing
    // still asks for the merge it can justify.
    let unknown = StateSnapshot {
        turn: 152,
        units: state
            .units
            .iter()
            .map(|unit| StateUnit {
                formation: Some(-1),
                ..unit.clone()
            })
            .collect(),
        ..StateSnapshot::default()
    };
    let blind = civvis::mirror::LiveMirror::new(&snapshot, &unknown, 4, 1, 250, 0);
    let first = *blind.uid_of.get(&71).expect("the first swordsman mirrors");
    let second = *blind.uid_of.get(&72).expect("the second swordsman mirrors");
    assert_eq!(
        translate(
            &Action::CombineUnits {
                unit: first,
                with: second,
            },
            &blind,
            &unknown,
        )
        .expect("an unread tier still translates")
        .verb
        .as_deref(),
        Some("FORM_CORPS")
    );
}

/// ★★★ Espionage had no route to a live game: all three actions fell
/// through `translate`'s `_ => None`, on top of a `Game::spies` the mirror
/// left empty so nothing was ever chosen to drop.
#[test]
fn spy_missions_reach_firaxis_unit_operations() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 120,
        width: 12,
        height: 12,
        chunk: 1,
        plots: vec![grass(4, 4), grass(6, 5)],
    }]);
    let mut state = StateSnapshot {
        turn: 120,
        spy_capacity: Some(2),
        ..StateSnapshot::default()
    };
    state.units.push(StateUnit {
        id: 77,
        kind: "UNIT_SPY".to_string(),
        x: 4,
        y: 4,
        ..StateUnit::default()
    });
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let spy = *mirror.uid_of.get(&77).expect("the Spy is mirrored");

    // The mission's own name becomes the host's operation, aimed at the
    // target plot the way Firaxis' EspionagePopup aims it.
    let heist = translate(
        &Action::SpyMission {
            spy,
            mission: "great_work_heist".to_string(),
            target: (6, 5),
        },
        &mirror,
        &state,
    )
    .expect("the mission crosses");
    assert_eq!(heist.kind, "unit");
    assert_eq!(heist.subject, Some(77));
    assert_eq!(heist.verb.as_deref(), Some("SPY_GREAT_WORK_HEIST"));
    assert!(
        heist.pos.is_some(),
        "a spy operation without a destination silently does nothing"
    );

    let promote = translate(
        &Action::PromoteSpy {
            spy,
            promotion: civvis::name!("technologist"),
        },
        &mirror,
        &state,
    )
    .expect("the promotion crosses");
    assert!(promote
        .verb
        .as_deref()
        .is_some_and(|verb| verb.starts_with("PROMOTE:")));
}

#[test]
fn culture_operations_cross_as_native_unit_operations() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 156,
        width: 12,
        height: 12,
        chunk: 1,
        plots: vec![grass(5, 5)],
    }]);
    for (kind, improvement, verb) in [
        ("UNIT_ARCHAEOLOGIST", Some("archaeological_dig"), "EXCAVATE"),
        (
            "UNIT_ARCHAEOLOGIST",
            Some("shipwreck_excavation"),
            "EXCAVATE",
        ),
        ("UNIT_NATURALIST", Some("national_park"), "DESIGNATE_PARK"),
        ("UNIT_ROCK_BAND", None, "TOURISM_BOMB"),
        ("UNIT_BUILDER", Some("farm"), "IMPROVE:IMPROVEMENT_FARM"),
        ("UNIT_BUILDER", Some("chop_woods"), "REMOVE_FEATURE"),
        ("UNIT_BUILDER", Some("chop_rainforest"), "REMOVE_FEATURE"),
        ("UNIT_BUILDER", Some("clear_marsh"), "REMOVE_FEATURE"),
        ("UNIT_BUILDER", Some("harvest_resource"), "HARVEST_RESOURCE"),
        ("UNIT_BUILDER", Some("plant_woods"), "PLANT_FOREST"),
    ] {
        let state = StateSnapshot {
            turn: 156,
            units: vec![StateUnit {
                id: 92,
                kind: kind.to_string(),
                x: 5,
                y: 5,
                build_charges: Some(0),
                spread_charges: Some(0),
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        };
        let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let unit = mirror.uid_of[&92];
        let action = match improvement {
            Some(name) => Action::Improve {
                unit,
                improvement: name.into(),
            },
            None => Action::PerformConcert { unit },
        };
        let order = translate(&action, &mirror, &state).expect("Culture action crosses");
        assert_eq!(order.kind, "unit");
        assert_eq!(order.subject, Some(92));
        assert_eq!(order.verb.as_deref(), Some(verb));
        assert_eq!(order.pos, None);
    }
}

#[test]
fn builder_repairs_reach_firaxis_unit_orders() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 94,
        width: 12,
        height: 12,
        chunk: 1,
        plots: vec![grass(5, 5)],
    }]);
    let state = StateSnapshot {
        turn: 94,
        units: vec![StateUnit {
            id: 92,
            kind: "UNIT_BUILDER".to_string(),
            x: 5,
            y: 5,
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let unit = *mirror.uid_of.get(&92).expect("the Builder is mirrored");

    let repair = translate(&Action::RepairImprovement { unit }, &mirror, &state)
        .expect("the builder repair crosses");
    assert_eq!(repair.kind, "unit");
    assert_eq!(repair.subject, Some(92));
    assert_eq!(repair.verb.as_deref(), Some("REPAIR"));
    assert_eq!(repair.pos, None);
}

/// `Action::Pillage` was decided by the doctrine layer and dropped by the
/// bridge; it now crosses as the parameterless host operation.
#[test]
fn a_pillage_crosses_as_the_parameterless_host_operation() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 60,
        width: 12,
        height: 12,
        chunk: 1,
        plots: vec![grass(5, 5)],
    }]);
    let state = StateSnapshot {
        turn: 60,
        units: vec![StateUnit {
            id: 77,
            kind: "UNIT_HORSEMAN".to_string(),
            x: 5,
            y: 5,
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let unit = *mirror.uid_of.get(&77).expect("the Horseman is mirrored");

    let pillage =
        translate(&Action::Pillage { unit }, &mirror, &state).expect("the pillage crosses");
    assert_eq!(pillage.kind, "unit");
    assert_eq!(pillage.subject, Some(77));
    assert_eq!(pillage.verb.as_deref(), Some("PILLAGE"));
    assert_eq!(pillage.pos, None);
}

#[test]
fn contracted_promotion_names_expand_to_the_firaxis_database_ids() {
    assert_eq!(
        civ6_unit_promotion_name("cobra_strike"),
        "PROMOTION_MONK_COBRA_STRIKE"
    );
    assert_eq!(
        civ6_unit_promotion_name("supercarrier"),
        "PROMOTION_SUPER_CARRIER"
    );
    assert_eq!(civ6_unit_promotion_name("surf_band"), "PROMOTION_SURF_ROCK");
}

/// The zero-charge Prophet left on the map after its religion was founded
/// is retired from the bridge, and its hex stops swallowing other units'
/// orders; a Prophet whose seat has NOT founded yet keeps every protection.
#[test]
fn a_founded_zero_charge_prophet_is_retired_and_stops_reserving_its_hex() {
    let ghost = StateUnit {
        id: 851_977,
        kind: "UNIT_GREAT_PROPHET".to_string(),
        x: 59,
        y: 16,
        great_person: Some(StateGreatPerson {
            charges: 0,
            can_activate: false,
            activation_plots: Vec::new(),
            ..StateGreatPerson::default()
        }),
        ..StateUnit::default()
    };
    let founded = StateSnapshot {
        founded_religion: Some("RELIGION_BUDDHISM".to_string()),
        units: vec![ghost.clone()],
        ..StateSnapshot::default()
    };

    let (orders, stall) = great_person_orders(&founded);
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].subject, Some(851_977));
    assert_eq!(orders[0].verb.as_deref(), Some("DELETE"));
    assert_eq!(stall.retired_prophets, 1);
    assert_eq!(stall.total(), 0, "a retirement is not a stall");

    // Its hex no longer black-holes the settler's exit from the capital.
    let settler_move = || Order {
        kind: "unit",
        subject: Some(2_031_627),
        verb: Some("MOVE_TO".to_string()),
        pos: Some((59, 16)),
    };
    let (kept, deferred) = defer_great_person_plot_conflicts(vec![settler_move()], &founded);
    assert_eq!(deferred, 0);
    assert_eq!(kept.len(), 1);

    // Before the religion exists the same unit is a Prophet still to found:
    // no retirement, still a named stall, hex still reserved.
    let unfounded = StateSnapshot {
        founded_religion: None,
        units: vec![ghost],
        ..StateSnapshot::default()
    };
    let (orders, stall) = great_person_orders(&unfounded);
    assert!(orders.is_empty());
    assert_eq!(stall.retired_prophets, 0);
    assert_eq!(stall.no_activation_plot, 1);
    let (kept, deferred) = defer_great_person_plot_conflicts(vec![settler_move()], &unfounded);
    assert_eq!(deferred, 1);
    assert!(kept.is_empty());
}

#[test]
fn great_person_reservation_defers_physical_moves_but_not_trade_routes() {
    let state = StateSnapshot {
        units: vec![
            StateUnit {
                id: 72,
                kind: "UNIT_GREAT_ADMIRAL".to_string(),
                x: 68,
                y: 27,
                great_person: Some(StateGreatPerson {
                    charges: 1,
                    can_activate: true,
                    ..StateGreatPerson::default()
                }),
                ..StateUnit::default()
            },
            StateUnit {
                id: 73,
                kind: "UNIT_GREAT_PROPHET".to_string(),
                x: 68,
                y: 17,
                great_person: Some(StateGreatPerson {
                    can_activate: false,
                    ..StateGreatPerson::default()
                }),
                ..StateUnit::default()
            },
        ],
        ..StateSnapshot::default()
    };
    let (orders, deferred) = defer_great_person_plot_conflicts(
        vec![
            Order {
                kind: "unit",
                subject: Some(10),
                verb: Some("MOVE_TO".to_string()),
                pos: Some((68, 27)),
            },
            Order {
                kind: "unit",
                subject: Some(72),
                verb: Some("ACTIVATE_GREAT_PERSON".to_string()),
                pos: None,
            },
            Order {
                kind: "unit",
                subject: Some(11),
                verb: Some("MOVE_TO".to_string()),
                pos: Some((68, 17)),
            },
            Order {
                kind: "unit",
                subject: Some(12),
                verb: Some("MOVE_TO".to_string()),
                pos: Some((69, 27)),
            },
            Order {
                kind: "unit",
                subject: Some(14),
                verb: Some("TRADE_ROUTE".to_string()),
                pos: Some((68, 27)),
            },
        ],
        &state,
    );

    assert_eq!(deferred, 2);
    assert_eq!(orders.len(), 3);
    assert_eq!(orders[0].subject, Some(72));
    assert_eq!(orders[1].subject, Some(12));
    assert_eq!(orders[2].verb.as_deref(), Some("TRADE_ROUTE"));
}

#[test]
fn firaxis_religion_state_reaches_a_found_religion_order() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 89,
        width: 16,
        height: 16,
        chunk: 1,
        plots: (0..16)
            .flat_map(|x| (0..16).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 89,
        pantheon: Some("BELIEF_DIVINE_SPARK".to_string()),
        prophet_pending: true,
        founded_religions: vec![
            "RELIGION_ORTHODOXY".to_string(),
            "RELIGION_CATHOLICISM".to_string(),
            "RELIGION_ISLAM".to_string(),
        ],
        taken_religion_beliefs: vec!["BELIEF_WORK_ETHIC".to_string(), "BELIEF_TITHE".to_string()],
        cities: vec![StateCity {
            id: 7,
            name: "Krakow".to_string(),
            x: 6,
            y: 6,
            pop: 3,
            capital: true,
            districts: vec![StateDistrict {
                kind: "DISTRICT_HOLY_SITE".to_string(),
                x: 7,
                y: 6,
                pillaged: false,
                complete: true,
                ..StateDistrict::default()
            }],
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 6, 1, 250, 0);

    assert!(mirror.game.players[0].prophet_pending);
    assert_eq!(mirror.game.religions_founded(), 3);
    assert_eq!(mirror.game.max_religions(), 4);
    assert!(mirror.game.players[1]
        .religion_beliefs
        .contains(&"work_ethic".to_string()));
    assert!(mirror.game.players[1]
        .religion_beliefs
        .contains(&"tithe".to_string()));
    let action = mirror
        .game
        .legal_actions(0)
        .into_iter()
        .find(|action| matches!(action, Action::FoundReligion { .. }))
        .expect("a pending Firaxis Prophet must expose a religion choice");
    let order =
        translate(&action, &mirror, &state).expect("a CIVVIS religion choice must reach Firaxis");

    assert_eq!(order.kind, "religion");
    assert_eq!(order.subject, None);
    assert!(order
        .verb
        .as_deref()
        .is_some_and(|verb| verb.starts_with("BELIEF_") && verb.contains(",BELIEF_")));
}

#[test]
fn purchases_preserve_currency_formation_and_district_plot() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 92,
        width: 16,
        height: 16,
        chunk: 1,
        plots: (0..16)
            .flat_map(|x| (0..16).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 92,
        cities: vec![StateCity {
            id: 77,
            name: "Wroclaw".to_string(),
            x: 6,
            y: 6,
            pop: 4,
            capital: true,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 6, 1, 250, 0);
    let city = mirror.game.player_city_ids(0)[0];
    let unit = translate(
        &Action::Buy {
            city,
            unit: civvis::name!("missionary"),
            formation: 2,
            currency: "faith".to_string(),
        },
        &mirror,
        &state,
    )
    .expect("faith unit purchase crosses");
    assert_eq!(unit.kind, "purchase_faith");
    assert_eq!(unit.subject, Some(77));
    assert_eq!(unit.verb.as_deref(), Some("UNIT_MISSIONARY"));
    assert_eq!(unit.pos, Some((2, -1)));

    let walls = translate(
        &Action::BuyBuilding {
            city,
            building: civvis::name!("medieval_walls"),
            currency: "gold".to_string(),
        },
        &mirror,
        &state,
    )
    .expect("a wall purchase crosses through the same building vocabulary");
    assert_eq!(walls.kind, "purchase");
    assert_eq!(walls.subject, Some(77));
    assert_eq!(walls.verb.as_deref(), Some("BUILDING_CASTLE"));

    let pos = (7, 6);
    let district = translate(
        &Action::BuyDistrict {
            city,
            district: civvis::name!("campus"),
            pos,
            currency: "gold".to_string(),
        },
        &mirror,
        &state,
    )
    .expect("placed district purchase crosses");
    assert_eq!(district.kind, "purchase");
    assert_eq!(district.verb.as_deref(), Some("DISTRICT_CAMPUS"));
    assert_eq!(
        district.pos,
        Some(civvis::hex::axial_to_offset(pos.0, pos.1))
    );
}

#[test]
fn pillaged_district_repair_reuses_the_firaxis_district_type() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 112,
        width: 16,
        height: 16,
        chunk: 1,
        plots: (0..16)
            .flat_map(|x| (0..16).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 112,
        cities: vec![StateCity {
            id: 77,
            name: "Ostia".to_string(),
            x: 6,
            y: 6,
            pop: 4,
            capital: true,
            districts: vec![StateDistrict {
                kind: "DISTRICT_CAMPUS".to_string(),
                x: 7,
                y: 6,
                pillaged: true,
                complete: true,
                ..StateDistrict::default()
            }],
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 6, 1, 250, 0);
    let city = mirror.game.player_city_ids(0)[0];
    let pos = civvis::hex::offset_to_axial(7, 6);
    assert!(mirror.game.map.tiles[&pos].pillaged);

    let order = translate(
        &Action::Produce {
            city,
            item: civvis::game::Item::Repair {
                repair: civvis::name!("district"),
                pos,
            },
        },
        &mirror,
        &state,
    )
    .expect("a CIVVIS district repair must reach Firaxis");

    assert_eq!(order.kind, "produce");
    assert_eq!(order.subject, Some(77));
    assert_eq!(order.verb.as_deref(), Some("DISTRICT_CAMPUS"));
    assert_eq!(order.pos, None);
}

#[test]
fn link_and_unlink_orders_preserve_both_firaxis_unit_ids() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 92,
        width: 16,
        height: 16,
        chunk: 1,
        plots: (0..16)
            .flat_map(|x| (0..16).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 92,
        seat: civvis::mirror::Seat {
            local_player: 7,
            ..civvis::mirror::Seat::default()
        },
        units: vec![
            StateUnit {
                id: 501,
                kind: "UNIT_SETTLER".to_string(),
                x: 6,
                y: 6,
                hp: 100.0,
                ..StateUnit::default()
            },
            StateUnit {
                id: 502,
                kind: "UNIT_WARRIOR".to_string(),
                x: 6,
                y: 6,
                hp: 100.0,
                ..StateUnit::default()
            },
        ],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 6, 1, 250, 0);
    let settler = mirror.uid_of[&501];
    let escort = mirror.uid_of[&502];

    let link = translate(
        &Action::LinkUnits {
            unit: settler,
            with: escort,
        },
        &mirror,
        &state,
    )
    .expect("formation order crosses");
    assert_eq!(link.kind, "unit");
    assert_eq!(link.subject, Some(501));
    assert_eq!(link.verb.as_deref(), Some("ENTER_FORMATION"));
    assert_eq!(link.pos, Some((7, 502)));

    let unlink = translate(&Action::UnlinkUnits { unit: settler }, &mirror, &state)
        .expect("formation exit crosses");
    assert_eq!(unlink.subject, Some(501));
    assert_eq!(unlink.verb.as_deref(), Some("EXIT_FORMATION"));
    assert_eq!(unlink.pos, None);
}

fn unit_order(subject: i64, verb: &str, pos: Option<(i32, i32)>) -> Order {
    Order {
        kind: "unit",
        subject: Some(subject),
        verb: Some(verb.to_string()),
        pos,
    }
}

fn production_order(subject: i64, verb: &str) -> Order {
    Order {
        kind: "produce",
        subject: Some(subject),
        verb: Some(verb.to_string()),
        pos: None,
    }
}

#[test]
fn unit_followups_wait_for_the_next_observed_firaxis_frame() {
    let (orders, deferred, coalesced) = coalesce_unit_paths(
        vec![
            unit_order(42, "MOVE_TO", Some((4, 5))),
            unit_order(42, "IMPROVE:IMPROVEMENT_FARM", None),
            unit_order(99, "FORTIFY", None),
            Order {
                kind: "research",
                subject: None,
                verb: Some("TECH_WRITING".to_string()),
                pos: None,
            },
        ],
        false,
    );

    assert_eq!(deferred, 1);
    assert_eq!(coalesced, 0);
    assert_eq!(orders.len(), 3);
    assert_eq!(orders[0].subject, Some(42));
    assert_eq!(orders[0].verb.as_deref(), Some("MOVE_TO"));
    assert_eq!(orders[0].pos, Some((4, 5)));
    assert_eq!(orders[1].subject, Some(99));
    assert_eq!(orders[2].kind, "research");
}

#[test]
fn wounded_routes_preserve_the_planned_detour_and_queued_followup() {
    let state = civvis::mirror::StateSnapshot {
        units: vec![civvis::mirror::StateUnit {
            id: 7,
            x: 10,
            y: 10,
            hp: 45.0,
            combat: 35.0,
            ..Default::default()
        }],
        hostiles: vec![civvis::mirror::StateUnit {
            id: 9,
            x: 11,
            y: 10,
            hp: 100.0,
            combat: 35.0,
            ..Default::default()
        }],
        ..Default::default()
    };
    let local = wounded_local_routes(&state);
    assert!(local.contains(&7));
    let planned = || {
        vec![
            unit_order(7, "MOVE_TO", Some((10, 9))),
            unit_order(7, "MOVE_TO", Some((11, 8))),
            unit_order(7, "FORTIFY", None),
            unit_order(8, "MOVE_TO", Some((20, 9))),
            unit_order(8, "MOVE_TO", Some((21, 8))),
        ]
    };
    let (orders, deferred, coalesced) =
        coalesce_unit_paths_except(planned(), true, &local, &Default::default());
    assert_eq!(deferred, 0);
    assert_eq!(coalesced, 1, "healthy travel still coalesces");
    assert_eq!(orders[0].pos, Some((10, 9)));
    assert_eq!(orders[1].pos, Some((11, 8)));
    assert_eq!(orders[2].verb.as_deref(), Some("FORTIFY"));
    let (old_host, deferred, _) =
        coalesce_unit_paths_except(planned(), false, &local, &Default::default());
    assert_eq!(deferred, 2);
    assert_eq!(
        old_host
            .iter()
            .filter(|order| order.subject == Some(7))
            .count(),
        1
    );
    assert_eq!(old_host[0].pos, Some((10, 9)));
}

#[test]
fn local_route_guard_requires_a_wounded_fighter_and_visible_enemy() {
    let mut state = civvis::mirror::StateSnapshot {
        units: vec![civvis::mirror::StateUnit {
            id: 7,
            x: 10,
            y: 10,
            hp: 100.0,
            combat: 35.0,
            ..Default::default()
        }],
        rivals: vec![civvis::mirror::StateRival {
            at_war: true,
            units: vec![civvis::mirror::StateUnit {
                x: 11,
                y: 10,
                combat: 35.0,
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert!(wounded_local_routes(&state).is_empty());
    state.units[0].hp = 45.0;
    assert!(wounded_local_routes(&state).contains(&7));
    state.rivals[0].at_war = false;
    assert!(wounded_local_routes(&state).is_empty());
    state.rivals[0].at_war = true;
    state.units[0].combat = 0.0;
    assert!(wounded_local_routes(&state).is_empty());
    state.units[0].combat = 35.0;
    state.rivals[0].units[0].x = 30;
    assert!(wounded_local_routes(&state).is_empty());
}

/// The found carries the hex the planned walk leaves the settler on —
/// the last step of its contiguous walk — or, founding without moving,
/// where the host says it stands. A found that already has a site and a
/// unit the export does not know are left alone.
#[test]
fn a_found_city_row_carries_the_site_the_walk_ends_on() {
    let state = civvis::mirror::StateSnapshot {
        units: vec![
            civvis::mirror::StateUnit {
                id: 7,
                x: 10,
                y: 10,
                ..Default::default()
            },
            civvis::mirror::StateUnit {
                id: 8,
                x: 30,
                y: 31,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut orders = vec![
        unit_order(7, "MOVE_TO", Some((11, 10))),
        unit_order(7, "MOVE_TO", Some((12, 11))),
        unit_order(7, "FOUND_CITY", None),
        unit_order(8, "FOUND_CITY", None),
        unit_order(9, "FOUND_CITY", None),
        unit_order(10, "FOUND_CITY", Some((1, 1))),
    ];
    assert_eq!(stamp_found_sites(&mut orders, &state), 2);
    assert_eq!(orders[2].pos, Some((12, 11)), "the walk's last hex");
    assert_eq!(orders[3].pos, Some((30, 31)), "founds where it stands");
    assert_eq!(orders[4].pos, None, "a unit the export does not know");
    assert_eq!(orders[5].pos, Some((1, 1)), "an existing site is kept");
    // Stamped before the fold, the site survives coalescing and the
    // walk-with-a-follow-up is never cut.
    let (orders, _, coalesced) = coalesce_unit_paths(orders, true);
    assert_eq!(coalesced, 1);
    assert_eq!(orders[0].pos, Some((12, 11)));
    assert_eq!(orders[1].verb.as_deref(), Some("FOUND_CITY"));
    assert_eq!(orders[1].pos, Some((12, 11)));
}

/// `--assume-seat` names the capabilities a replay should pretend the
/// mod advertised; a typo or non-boolean value is an error.
#[test]
fn assumed_seat_capabilities_are_named_and_checked() {
    let mut seat = civvis::mirror::Seat::default();
    let names: Vec<String> = ["order_queue", "replan_frames", " tile_delta "]
        .iter()
        .map(|name| name.to_string())
        .collect();
    assume_seat_capabilities(&mut seat, &names).unwrap();
    assert!(seat.order_queue);
    assert!(seat.replan_frames);
    assert!(seat.tile_delta);
    assert!(!seat.moves_at_turn_start, "not named: not assumed");
    assert!(assume_seat_capabilities(&mut seat, &["order_que".to_string()]).is_err());
    assert!(assume_seat_capabilities(&mut seat, &["replan_frames=2".to_string()]).is_err());
}

/// A settler with two movement points logs two hex steps; the host must be
/// asked for the WHOLE walk, or it spends every turn on the first hex.
#[test]
fn a_units_planned_walk_becomes_one_move_to_its_furthest_hex() {
    let (orders, deferred, coalesced) = coalesce_unit_paths(
        vec![
            unit_order(7, "MOVE_TO", Some((10, 10))),
            unit_order(7, "MOVE_TO", Some((11, 10))),
            unit_order(7, "MOVE_TO", Some((12, 11))),
            unit_order(7, "FOUND_CITY", None),
        ],
        false,
    );

    assert_eq!(coalesced, 2, "two later steps folded into the first");
    assert_eq!(
        deferred, 1,
        "the founding still waits for the arrival frame"
    );
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].subject, Some(7));
    assert_eq!(orders[0].verb.as_deref(), Some("MOVE_TO"));
    assert_eq!(orders[0].pos, Some((12, 11)));
}

/// Only the CONTIGUOUS prefix of moves is a walk. A move logged after an act
/// was planned from where the act leaves the unit, so it waits like the act.
#[test]
fn a_move_after_an_act_is_not_folded_into_the_walk() {
    let (orders, deferred, coalesced) = coalesce_unit_paths(
        vec![
            unit_order(3, "MOVE_TO", Some((1, 1))),
            unit_order(3, "MOVE_TO", Some((2, 1))),
            unit_order(3, "RANGE_ATTACK", Some((4, 1))),
            unit_order(3, "MOVE_TO", Some((1, 1))),
        ],
        false,
    );

    assert_eq!(coalesced, 1);
    assert_eq!(deferred, 2);
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].pos, Some((2, 1)));
}

/// Melee stays conservative: the strike is not turned into a MOVE_TO onto the
/// defender, and a unit whose first order is the strike sends only the strike.
#[test]
fn a_melee_strike_terminates_the_walk_and_leads_alone_when_first() {
    let (orders, deferred, coalesced) = coalesce_unit_paths(
        vec![
            unit_order(5, "MOVE_TO", Some((8, 8))),
            unit_order(5, "ATTACK", Some((9, 8))),
            unit_order(6, "ATTACK", Some((9, 8))),
            unit_order(6, "MOVE_TO", Some((9, 8))),
        ],
        false,
    );

    assert_eq!(coalesced, 0);
    assert_eq!(deferred, 2);
    assert_eq!(orders.len(), 2);
    assert_eq!(orders[0].subject, Some(5));
    assert_eq!(orders[0].verb.as_deref(), Some("MOVE_TO"));
    assert_eq!(orders[0].pos, Some((8, 8)));
    assert_eq!(orders[1].subject, Some(6));
    assert_eq!(orders[1].verb.as_deref(), Some("ATTACK"));
}

/// Units are folded independently even when their steps interleave, and each
/// keeps the slot of its FIRST order so the batch's relative order — which the
/// host executes sequentially — is preserved.
#[test]
fn interleaved_units_fold_independently_and_keep_their_batch_slots() {
    let (orders, deferred, coalesced) = coalesce_unit_paths(
        vec![
            unit_order(1, "MOVE_TO", Some((0, 1))),
            unit_order(2, "MOVE_TO", Some((5, 5))),
            unit_order(1, "MOVE_TO", Some((0, 2))),
            Order {
                kind: "produce",
                subject: Some(65_536),
                verb: Some("UNIT_SETTLER".to_string()),
                pos: None,
            },
            unit_order(2, "MOVE_TO", Some((6, 5))),
            unit_order(2, "MOVE_TO", Some((7, 5))),
            unit_order(1, "IMPROVE:IMPROVEMENT_MINE", None),
        ],
        false,
    );

    assert_eq!(coalesced, 3);
    assert_eq!(deferred, 1);
    assert_eq!(orders.len(), 3);
    assert_eq!((orders[0].subject, orders[0].pos), (Some(1), Some((0, 2))));
    assert_eq!((orders[1].subject, orders[1].pos), (Some(2), Some((7, 5))));
    assert_eq!(orders[2].kind, "produce");
}

/// (subject, verb, position) of one host order, for asserting a sequence.
type Sequenced<'a> = (Option<i64>, &'a str, Option<(i32, i32)>);

/// With a queueing mod (`seat.order_queue`), the walk still folds into one
/// MOVE_TO and every later order for that unit rides along in sequence —
/// the strike after the step, the fortify after the strike, a move after an
/// act as its own order — instead of waiting a turn.
#[test]
fn a_sequenced_turn_keeps_every_followup_behind_the_folded_walk() {
    let (orders, deferred, coalesced) = coalesce_unit_paths(
        vec![
            unit_order(3, "MOVE_TO", Some((1, 1))),
            unit_order(3, "MOVE_TO", Some((2, 1))),
            unit_order(3, "RANGE_ATTACK", Some((4, 1))),
            unit_order(3, "FORTIFY", None),
            unit_order(5, "MOVE_TO", Some((8, 8))),
            unit_order(5, "ATTACK", Some((9, 8))),
            unit_order(5, "MOVE_TO", Some((8, 7))),
            unit_order(6, "ATTACK", Some((9, 8))),
        ],
        true,
    );

    assert_eq!(deferred, 0, "nothing waits for the next frame");
    assert_eq!(coalesced, 1, "only the contiguous walk folds");
    let sequence: Vec<Sequenced<'_>> = orders
        .iter()
        .map(|order| {
            (
                order.subject,
                order.verb.as_deref().unwrap_or(""),
                order.pos,
            )
        })
        .collect();
    assert_eq!(
        sequence,
        vec![
            (Some(3), "MOVE_TO", Some((2, 1))),
            (Some(3), "RANGE_ATTACK", Some((4, 1))),
            (Some(3), "FORTIFY", None),
            (Some(5), "MOVE_TO", Some((8, 8))),
            (Some(5), "ATTACK", Some((9, 8))),
            (Some(5), "MOVE_TO", Some((8, 7))),
            (Some(6), "ATTACK", Some((9, 8))),
        ]
    );
    assert_eq!(sequenced_unit_followups(&orders), 4);
}

/// The same list against a mod without the queue is exactly the old
/// behaviour: the capability decides, not the wish.
#[test]
fn without_the_capability_the_followups_still_wait() {
    let list = || {
        vec![
            unit_order(3, "MOVE_TO", Some((1, 1))),
            unit_order(3, "RANGE_ATTACK", Some((4, 1))),
        ]
    };
    let (orders, deferred, _) = coalesce_unit_paths(list(), false);
    assert_eq!((orders.len(), deferred), (1, 1));
    let (orders, deferred, _) = coalesce_unit_paths(list(), true);
    assert_eq!((orders.len(), deferred), (2, 0));
    assert_eq!(sequenced_unit_followups(&orders), 1);
}

#[test]
fn governor_followups_wait_for_the_next_observed_firaxis_frame() {
    let (orders, deferred) = defer_governor_followups(vec![
        Order {
            kind: "governor_appoint",
            subject: Some(65_536),
            verb: Some("GOVERNOR_THE_DEFENDER".to_string()),
            pos: Some((0, -1)),
        },
        Order {
            kind: "governor_promote",
            subject: None,
            verb: Some("GOVERNOR_THE_DEFENDER,GOVERNOR_PROMOTION_GARRISON_COMMANDER".to_string()),
            pos: None,
        },
        Order {
            kind: "research",
            subject: None,
            verb: Some("TECH_WRITING".to_string()),
            pos: None,
        },
    ]);

    assert_eq!(deferred, 1);
    assert_eq!(orders.len(), 2);
    assert_eq!(orders[0].kind, "governor_appoint");
    assert_eq!(orders[1].kind, "research");
}

#[test]
fn governor_actions_use_firaxis_types_and_city_owner_ids() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 40,
        seat: civvis::mirror::Seat {
            local_player: 7,
            ..civvis::mirror::Seat::default()
        },
        cities: vec![StateCity {
            id: 65_536,
            name: "Capital".to_string(),
            x: 6,
            y: 6,
            pop: 5,
            capital: true,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let city = mirror.game.player_city_ids(0)[0];

    let appoint = translate(
        &Action::AppointGovernor {
            governor: civvis::name!("victor"),
            city,
        },
        &mirror,
        &state,
    )
    .expect("a known Governor and city translate");
    assert_eq!(appoint.kind, "governor_appoint");
    assert_eq!(appoint.subject, Some(65_536));
    assert_eq!(appoint.verb.as_deref(), Some("GOVERNOR_THE_DEFENDER"));
    assert_eq!(appoint.pos, Some((7, -1)));

    let promote = translate(
        &Action::PromoteGovernor {
            governor: civvis::name!("victor"),
            promotion: civvis::name!("garrison_commander"),
        },
        &mirror,
        &state,
    )
    .expect("a known promotion translates");
    assert_eq!(promote.kind, "governor_promote");
    assert_eq!(
        promote.verb.as_deref(),
        Some("GOVERNOR_THE_DEFENDER,GOVERNOR_PROMOTION_GARRISON_COMMANDER")
    );
}

#[test]
fn great_person_claims_cross_the_bridge() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 40,
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);

    let recruit = translate(
        &Action::RecruitGreatPerson {
            kind: "scientist".to_string(),
        },
        &mirror,
        &state,
    )
    .expect("a recruit decision translates");
    assert_eq!(recruit.kind, "gp_recruit");
    assert_eq!(recruit.subject, None);
    assert_eq!(
        recruit.verb.as_deref(),
        Some("GREAT_PERSON_CLASS_SCIENTIST")
    );

    let gold = translate(
        &Action::PatronizeGreatPerson {
            kind: "merchant".to_string(),
            currency: "gold".to_string(),
        },
        &mirror,
        &state,
    )
    .expect("a gold patronage translates");
    assert_eq!(gold.kind, "gp_patronize");
    assert_eq!(gold.verb.as_deref(), Some("GREAT_PERSON_CLASS_MERCHANT"));

    let faith = translate(
        &Action::PatronizeGreatPerson {
            kind: "general".to_string(),
            currency: "faith".to_string(),
        },
        &mirror,
        &state,
    )
    .expect("a faith patronage translates");
    assert_eq!(faith.kind, "gp_patronize_faith");
    assert_eq!(faith.verb.as_deref(), Some("GREAT_PERSON_CLASS_GENERAL"));
}

#[test]
fn dedication_choices_cross_with_firaxis_commemoration_names() {
    use civvis::name::Name;

    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 40,
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);

    for (civvis, firaxis) in [
        ("monumentality", "COMMEMORATION_INFRASTRUCTURE"),
        ("pen_brush_and_voice", "COMMEMORATION_CULTURAL"),
        ("free_inquiry", "COMMEMORATION_SCIENTIFIC"),
        ("exodus_of_the_evangelists", "COMMEMORATION_RELIGIOUS"),
        ("hic_sunt_dracones", "COMMEMORATION_EXPLORATION"),
        ("reform_the_coinage", "COMMEMORATION_ECONOMIC"),
        ("to_arms", "COMMEMORATION_MILITARY"),
        ("heartbeat_of_steam", "COMMEMORATION_INDUSTRIAL"),
        ("bodyguard_of_lies", "COMMEMORATION_ESPIONAGE"),
        ("sky_and_stars", "COMMEMORATION_AERONAUTICAL"),
        ("wish_you_were_here", "COMMEMORATION_TOURISM"),
        ("automaton_warfare", "COMMEMORATION_AUTOMATON"),
    ] {
        let order = translate(
            &Action::ChooseDedication {
                dedication: Name::new(civvis),
            },
            &mirror,
            &state,
        )
        .expect("a known dedication translates");
        assert_eq!(order.kind, "dedication");
        assert_eq!(order.subject, None);
        assert_eq!(order.verb.as_deref(), Some(firaxis));
        assert_eq!(order.pos, None);
    }
    assert!(translate(
        &Action::ChooseDedication {
            dedication: Name::new("not_a_dedication"),
        },
        &mirror,
        &state,
    )
    .is_none());
}

#[test]
fn a_city_strike_names_its_city_and_target() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 40,
        cities: vec![StateCity {
            id: 65_536,
            name: "Capital".to_string(),
            x: 6,
            y: 6,
            pop: 5,
            capital: true,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let city = mirror.game.player_city_ids(0)[0];

    let strike = translate(
        &Action::CityStrike {
            city,
            target: (5, 6),
        },
        &mirror,
        &state,
    )
    .expect("a strike from a mapped city translates");
    assert_eq!(strike.kind, "city_strike");
    assert_eq!(strike.subject, Some(65_536));
    assert_eq!(strike.pos, Some(civvis::hex::axial_to_offset(5, 6)));

    let encampment = translate(
        &Action::EncampmentStrike {
            city,
            target: (7, 5),
        },
        &mirror,
        &state,
    )
    .expect("an encampment strike from a mapped city translates");
    assert_eq!(encampment.kind, "encampment_strike");
    assert_eq!(encampment.subject, Some(65_536));
    assert_eq!(encampment.pos, Some(civvis::hex::axial_to_offset(7, 5)));
}

/// The captured city's three dispositions cross as kind `city` — subject
/// = the host city id, verb = the `CityDestroyDirectives` name the shipped
/// `RazeCity.lua` buttons set, pos = the city plot — instead of falling
/// through to the host's default.
#[test]
fn a_captured_citys_disposition_crosses_as_keep_raze_or_liberate() {
    let (snapshot, state) = local_barbarian_defense_board();
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let rome = mirror.game.player_city_ids(0)[0];
    for (action, verb) in [
        (Action::KeepCity { city: rome }, "KEEP"),
        (Action::RazeCity { city: rome }, "RAZE"),
        (Action::LiberateCity { city: rome }, "LIBERATE"),
    ] {
        let order = translate(&action, &mirror, &state).expect("the disposition crosses");
        assert_eq!(
            (order.kind, order.subject, order.verb.as_deref(), order.pos),
            ("city", Some(7), Some(verb), Some((4, 4))),
            "{verb}: host city 7 on its plot, in host offsets"
        );
    }
    // A city the mirror cannot map has no host id to decide for.
    assert!(translate(&Action::KeepCity { city: u32::MAX }, &mirror, &state).is_none());
}

#[test]
fn delegations_and_embassies_name_the_firaxis_seat() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 40,
        rivals: vec![StateRival {
            player: 4,
            civ: "CIVILIZATION_PERSIA".to_string(),
            leader: "LEADER_NADER_SHAH".to_string(),
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);

    let delegation = translate(&Action::SendDelegation { player: 1 }, &mirror, &state)
        .expect("a delegation to a mapped rival translates");
    assert_eq!(delegation.kind, "delegation");
    assert_eq!(delegation.subject, Some(4));
    assert_eq!(delegation.verb.as_deref(), Some("DIPLOMATIC_DELEGATION"));

    let denounce = translate(&Action::Denounce { player: 1 }, &mirror, &state)
        .expect("a denouncement of a mapped rival translates");
    assert_eq!(denounce.kind, "denounce");
    assert_eq!(denounce.subject, Some(4));
    assert_eq!(denounce.verb.as_deref(), Some("DENOUNCE"));

    for (casus_belli, statement) in [
        ("formal_war", "DECLARE_FORMAL_WAR"),
        ("formal", "DECLARE_FORMAL_WAR"),
        ("surprise_war", "DECLARE_SURPRISE_WAR"),
        ("holy_war", "DECLARE_HOLY_WAR"),
        ("liberation_war", "DECLARE_LIBERATION_WAR"),
        ("reconquest_war", "DECLARE_RECONQUEST_WAR"),
        ("protectorate_war", "DECLARE_PROTECTORATE_WAR"),
        ("colonial_war", "DECLARE_COLONIAL_WAR"),
        ("territorial_war", "DECLARE_TERRITORIAL_WAR"),
        ("golden_age_war", "DECLARE_GOLDEN_AGE_WAR"),
        ("retribution_war", "DECLARE_WAR_OF_RETRIBUTION"),
        ("ideological_war", "DECLARE_IDEOLOGICAL_WAR"),
    ] {
        let war = translate(
            &Action::DeclareWarWithCasusBelli {
                player: 1,
                casus_belli: casus_belli.into(),
            },
            &mirror,
            &state,
        )
        .expect("a supported declaration retains its native session");
        assert_eq!(war.subject, Some(4));
        assert_eq!(war.verb.as_deref(), Some(statement));
    }
    for unsupported in ["joint_war", "unknown_war"] {
        assert!(translate(
            &Action::DeclareWarWithCasusBelli {
                player: 1,
                casus_belli: unsupported.into(),
            },
            &mirror,
            &state,
        )
        .is_none());
    }

    let embassy = translate(&Action::SendEmbassy { player: 1 }, &mirror, &state)
        .expect("an embassy to a mapped rival translates");
    assert_eq!(embassy.kind, "delegation");
    assert_eq!(embassy.subject, Some(4));
    assert_eq!(embassy.verb.as_deref(), Some("RESIDENT_EMBASSY"));

    // An unmet seat still crosses, with no subject: the Lua side names it
    // `delegation_target_unmapped` instead of the bridge dropping it here.
    let unmet = translate(&Action::SendDelegation { player: 3 }, &mirror, &state)
        .expect("an unmapped rival still crosses for the ledger");
    assert_eq!(unmet.subject, None);
}

/// A friendship and nothing else crosses as the host's `DECLARE_FRIEND`
/// session (DiplomacyActionView.lua:472-473); the planner's alliance
/// bundle crosses as the agent's `alliance` arm (friendship first, then
/// the ALLIANCE agreement); a deal that bundles passage or gold has no
/// host counterpart and stays skipped.
#[test]
fn a_friendship_only_deal_crosses_as_a_declare_friend_session() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 40,
        rivals: vec![StateRival {
            player: 4,
            civ: "CIVILIZATION_PERSIA".to_string(),
            leader: "LEADER_NADER_SHAH".to_string(),
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let deal = |open_borders: bool,
                alliance: Option<&str>,
                give_gold: f64,
                request_gold: f64,
                peace: bool| Action::ProposeDeal {
        player: 1,
        give_gold,
        request_gold,
        open_borders,
        friendship: true,
        peace,
        alliance: alliance.map(str::to_string),
    };

    let order = translate(&deal(false, None, 0.0, 0.0, false), &mirror, &state)
        .expect("a friendship-only deal to a mapped rival translates");
    assert_eq!(order.kind, "friendship");
    assert_eq!(order.subject, Some(4));
    assert_eq!(order.verb.as_deref(), Some("DECLARE_FRIEND"));
    assert_eq!(order.pos, None);

    // The planner's alliance bundle (`propose_strategic_alliance` sets
    // friendship and, under `no_free_passage` off, passage with it).
    for (kind, host) in [
        ("research", "ALLIANCE_RESEARCH"),
        ("cultural", "ALLIANCE_CULTURAL"),
        ("religious", "ALLIANCE_RELIGIOUS"),
        ("military", "ALLIANCE_MILITARY"),
        ("economic", "ALLIANCE_ECONOMIC"),
    ] {
        for open_borders in [false, true] {
            let order = translate(
                &deal(open_borders, Some(kind), 0.0, 0.0, false),
                &mirror,
                &state,
            )
            .expect("an alliance bundle crosses");
            assert_eq!(order.kind, "alliance");
            assert_eq!(order.subject, Some(4));
            assert_eq!(order.verb.as_deref(), Some(host));
        }
    }
    assert!(
        translate(
            &deal(false, Some("imperial"), 0.0, 0.0, false),
            &mirror,
            &state
        )
        .is_none(),
        "an alliance kind the host has no type for is skipped"
    );
    assert!(
        translate(
            &deal(false, Some("research"), 30.0, 0.0, false),
            &mirror,
            &state
        )
        .is_none(),
        "an alliance with gold bundled stays skipped"
    );

    for (label, bundled) in [
        ("passage", deal(true, None, 0.0, 0.0, false)),
        ("gold given", deal(false, None, 30.0, 0.0, false)),
        ("gold asked", deal(false, None, 0.0, 30.0, false)),
    ] {
        assert!(
            translate(&bundled, &mirror, &state).is_none(),
            "a friendship with {label} stays skipped"
        );
    }
    let peace = translate(&deal(false, None, 0.0, 0.0, true), &mirror, &state)
        .expect("a peace deal still crosses");
    assert_eq!(
        peace.kind, "peace",
        "peace outranks the friendship it carries"
    );
}

#[test]
fn planner_sales_cross_as_sell_orders_with_a_floor() {
    use civvis::game::DealItems;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 144,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 144,
        rivals: vec![StateRival {
            player: 4,
            civ: "CIVILIZATION_PERSIA".to_string(),
            leader: "LEADER_NADER_SHAH".to_string(),
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);

    // The exact quote run civvis-20260818T083142Z dropped at t156: one
    // surplus dyes copy for 84 gold and 1.1 a turn.
    let mut offer = DealItems::default();
    offer.resources.insert("dyes".to_string(), 1);
    let request = DealItems {
        gold: 84.0,
        gold_per_turn: 1.1,
        ..DealItems::default()
    };
    let (offer_copy, request_copy) = (offer.clone(), request.clone());
    let sale = translate(
        &Action::Trade {
            player: 1,
            offer: Box::new(offer),
            request: Box::new(request),
        },
        &mirror,
        &state,
    )
    .expect("a luxury sale to a mapped rival translates");
    assert_eq!(sale.kind, "sell");
    assert_eq!(sale.subject, Some(4));
    assert_eq!(sale.verb.as_deref(), Some("RESOURCE_DYES=1"));
    // The floor is our own walk-away by the mirrored board's book: the
    // ask (84 + 25 × 1.1) less our net gain on the quote — not half the
    // ask, which is what it was until 2026-08-24.
    let (my_value, _) = mirror
        .game
        .trade_utilities(0, 1, &offer_copy, &request_copy);
    let asked = 84.0 + 25.0 * 1.1;
    // A mirrored board that cannot price the copy answers non-finite,
    // and the floor is then the whole ask.
    let net = if my_value.is_finite() { my_value } else { 0.0 };
    let walk_away = ((asked - net).ceil() as i32).max(SALE_FLOOR_MIN);
    assert_eq!(sale.pos, Some((walk_away, 0)));
    assert!(
        walk_away > 56,
        "the old half-ask floor of 56 undercut our own book: {walk_away}"
    );

    // A favor block and a strategic block ride the same verb, favor last,
    // and a tiny ask still carries the minimum floor.
    let mut mixed = DealItems::default();
    mixed.resources.insert("iron".to_string(), 10);
    mixed.resources.insert("dyes".to_string(), 1);
    mixed.diplomatic_favor = 10.0;
    let sale = translate(
        &Action::Trade {
            player: 1,
            offer: Box::new(mixed),
            request: Box::new(DealItems {
                gold: 18.0,
                ..DealItems::default()
            }),
        },
        &mirror,
        &state,
    )
    .expect("a mixed sale translates");
    assert_eq!(
        sale.verb.as_deref(),
        Some("RESOURCE_DYES=1,RESOURCE_IRON=10,FAVOR=10")
    );
    // A tiny ask carries our walk-away, never less than the minimum.
    let floor = sale.pos.unwrap().0;
    assert!((SALE_FLOOR_MIN..=18).contains(&floor), "floor {floor}");

    // An unmapped seat still crosses for the ledger, subject-less, like
    // the delegation arm: the Lua side names it `sell_target_unmapped`.
    let favor = DealItems {
        diplomatic_favor: 10.0,
        ..DealItems::default()
    };
    let unmapped = translate(
        &Action::Trade {
            player: 3,
            offer: Box::new(favor),
            request: Box::new(DealItems {
                gold: 30.0,
                ..DealItems::default()
            }),
        },
        &mirror,
        &state,
    )
    .expect("an unmapped buyer still crosses");
    assert_eq!(unmapped.subject, None);
}

#[test]
fn a_passage_purchase_crosses_as_a_buy_order_with_a_ceiling() {
    use civvis::game::DealItems;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 90,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 90,
        rivals: vec![StateRival {
            player: 4,
            civ: "CIVILIZATION_KONGO".to_string(),
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);

    // Their Open Borders for our gold: the one purchase with an arm.
    let buy = translate(
        &Action::Trade {
            player: 1,
            offer: Box::new(DealItems {
                gold: 60.0,
                ..DealItems::default()
            }),
            request: Box::new(DealItems {
                open_borders: true,
                ..DealItems::default()
            }),
        },
        &mirror,
        &state,
    )
    .expect("a passage purchase from a mapped rival translates");
    assert_eq!(buy.kind, "buy");
    assert_eq!(buy.subject, Some(4));
    assert_eq!(buy.verb.as_deref(), Some("OPEN_BORDERS"));
    // The ceiling is what the passage is worth to us by the mirrored
    // board's book: the offer plus our net gain on the quote — not the
    // offer and half again, which is what it was until 2026-08-24.
    let worth = (mirror.game.passage_gold_value(0).ceil() as i32).clamp(1, BORDER_BUY_CEILING_MAX);
    assert_eq!(buy.pos, Some((worth, 0)));
    assert!(
        (28..90).contains(&worth),
        "the book, not the offer and half again: {worth}"
    );

    // Per-turn gold rides the same 25× book, and the ceiling never
    // crosses the cap however rich the offer.
    let rich = translate(
        &Action::Trade {
            player: 1,
            offer: Box::new(DealItems {
                gold: 200.0,
                gold_per_turn: 4.0,
                ..DealItems::default()
            }),
            request: Box::new(DealItems {
                open_borders: true,
                ..DealItems::default()
            }),
        },
        &mirror,
        &state,
    )
    .expect("a rich passage purchase still translates");
    // A rich offer does not raise the ceiling: passage is worth what the
    // book says, and never more than the cap.
    assert_eq!(rich.pos, buy.pos);
    assert!(rich.pos.unwrap().0 <= BORDER_BUY_CEILING_MAX);

    // A mutual swap is a different agreement and stays skipped, named.
    assert!(translate(
        &Action::Trade {
            player: 1,
            offer: Box::new(DealItems {
                open_borders: true,
                ..DealItems::default()
            }),
            request: Box::new(DealItems {
                open_borders: true,
                ..DealItems::default()
            }),
        },
        &mirror,
        &state,
    )
    .is_none());

    // Passage plus a luxury on either side is not this arm's shape.
    let mut sweetened = DealItems {
        gold: 60.0,
        ..DealItems::default()
    };
    sweetened.resources.insert("dyes".to_string(), 1);
    assert!(translate(
        &Action::Trade {
            player: 1,
            offer: Box::new(sweetened),
            request: Box::new(DealItems {
                open_borders: true,
                ..DealItems::default()
            }),
        },
        &mirror,
        &state,
    )
    .is_none());

    // A free ask carries no ceiling and does not cross.
    assert!(translate(
        &Action::Trade {
            player: 1,
            offer: Box::new(DealItems::default()),
            request: Box::new(DealItems {
                open_borders: true,
                ..DealItems::default()
            }),
        },
        &mirror,
        &state,
    )
    .is_none());
}

#[test]
fn purchases_and_other_deal_shapes_stay_untranslated() {
    use civvis::game::DealItems;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 84,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 84,
        rivals: vec![StateRival {
            player: 2,
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let trade = |offer: DealItems, request: DealItems| Action::Trade {
        player: 1,
        offer: Box::new(offer),
        request: Box::new(request),
    };

    // Buying tea for gold (t84 of the same run) is not a sale.
    let mut tea = DealItems::default();
    tea.resources.insert("tea".to_string(), 1);
    let purchase = trade(
        DealItems {
            gold: 75.0,
            gold_per_turn: 1.4,
            ..DealItems::default()
        },
        tea,
    );
    assert!(translate(&purchase, &mirror, &state).is_none());

    // A Great Work sale is a different validation and stays skipped.
    let mut work = DealItems::default();
    work.great_works.insert("religious_art".to_string(), 1);
    let art = trade(
        work,
        DealItems {
            gold: 103.0,
            gold_per_turn: 2.7,
            ..DealItems::default()
        },
    );
    assert!(translate(&art, &mirror, &state).is_none());

    // A resource-for-resource swap asks for something other than gold.
    let mut dyes = DealItems::default();
    dyes.resources.insert("dyes".to_string(), 1);
    let mut silk = DealItems::default();
    silk.resources.insert("silk".to_string(), 1);
    assert!(translate(&trade(dyes, silk), &mirror, &state).is_none());

    // Open Borders for gold is an agreement, not a sale.
    let borders = trade(
        DealItems {
            open_borders: true,
            ..DealItems::default()
        },
        DealItems {
            gold: 40.0,
            ..DealItems::default()
        },
    );
    assert!(translate(&borders, &mirror, &state).is_none());

    // Nothing offered is nothing sold.
    assert!(translate(
        &trade(
            DealItems::default(),
            DealItems {
                gold: 40.0,
                ..DealItems::default()
            }
        ),
        &mirror,
        &state
    )
    .is_none());
}

#[test]
fn a_peace_tribute_rides_only_a_routed_offer_and_is_capped() {
    let mut state = StateSnapshot {
        turn: 120,
        gold: 1_000,
        ..StateSnapshot::default()
    };
    assert_eq!(
        peace_tribute_cap(&state, false),
        0,
        "a second front, fatigue, a religious lane or an envoy reclaim is white peace"
    );
    assert_eq!(
        peace_tribute_cap(&state, true),
        250,
        "a quarter of the treasury"
    );
    state.gold = 5_000;
    assert_eq!(
        peace_tribute_cap(&state, true),
        PEACE_TRIBUTE_MAX as i32,
        "and never more than the maximum"
    );
    state.gold = 50;
    assert_eq!(peace_tribute_cap(&state, true), 12);
    state.gold = 0;
    assert_eq!(peace_tribute_cap(&state, true), 0);
}

#[test]
fn favor_is_bought_before_a_contested_session() {
    // Game 126 at turn 217: 0 Favor, 249 Gold, the session four turns
    // out, Sweden at 15 points and at war with us, two rivals at peace.
    let state = StateSnapshot {
        turn: 217,
        favor: Some(0.0),
        gold: 249,
        dvp: Some(11),
        congress_turns_left: Some(4),
        rivals: vec![
            StateRival {
                player: 2,
                dvp: Some(6),
                ..StateRival::default()
            },
            StateRival {
                player: 3,
                dvp: Some(15),
                at_war: true,
                ..StateRival::default()
            },
            StateRival {
                player: 4,
                dvp: Some(9),
                ..StateRival::default()
            },
        ],
        ..StateSnapshot::default()
    };

    // The gene off, nothing.
    let mut orders = Vec::new();
    assert_eq!(
        append_favor_buy_order(false, &state, &mut orders),
        Some("favor_buy_hold:gene")
    );
    assert!(orders.is_empty());

    // On: (249 - 100) / 8 = 18 points affordable is under the block
    // minimum; at 400 Gold, 37 points from the highest-DVP rival at peace.
    assert_eq!(
        append_favor_buy_order(true, &state, &mut orders),
        Some("favor_buy_hold:treasury")
    );
    let mut rich = state.clone();
    rich.gold = 400;
    assert_eq!(append_favor_buy_order(true, &rich, &mut orders), None);
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].kind, "buy");
    assert_eq!(orders[0].subject, Some(4), "not the rival at war");
    assert_eq!(orders[0].verb.as_deref(), Some("FAVOR=37"));
    assert_eq!(orders[0].pos, Some((296, 0)));

    // A rival with a deal already heading its way is passed over; with
    // none left, nobody sells.
    assert_eq!(append_favor_buy_order(true, &rich, &mut orders), None);
    assert_eq!(orders[1].subject, Some(2));
    assert_eq!(
        append_favor_buy_order(true, &rich, &mut orders),
        Some("favor_buy_hold:no_seller")
    );

    // A deep treasury buys the block cap, never past the target.
    let mut deep = rich.clone();
    deep.gold = 2_000;
    let mut orders = Vec::new();
    append_favor_buy_order(true, &deep, &mut orders);
    assert_eq!(orders[0].verb.as_deref(), Some("FAVOR=80"));
    deep.favor = Some(70.0);
    let mut orders = Vec::new();
    append_favor_buy_order(true, &deep, &mut orders);
    assert_eq!(orders[0].verb.as_deref(), Some("FAVOR=30"));
    deep.favor = Some(85.0);
    assert_eq!(
        append_favor_buy_order(true, &deep, &mut Vec::new()),
        Some("favor_buy_hold:banked")
    );

    // No contender, a far session, or our own sale this turn: nothing.
    let mut calm = rich.clone();
    calm.dvp = Some(4);
    for rival in &mut calm.rivals {
        rival.dvp = Some(9);
    }
    assert_eq!(
        append_favor_buy_order(true, &calm, &mut Vec::new()),
        Some("favor_buy_hold:no_contender")
    );
    let mut far = rich.clone();
    far.congress_turns_left = Some(12);
    assert_eq!(
        append_favor_buy_order(true, &far, &mut Vec::new()),
        Some("favor_buy_hold:session_far")
    );
    let mut selling = vec![Order {
        kind: "sell",
        subject: Some(2),
        verb: Some("FAVOR=40".to_string()),
        pos: Some((80, 0)),
    }];
    assert_eq!(
        append_favor_buy_order(true, &rich, &mut selling),
        Some("favor_buy_hold:selling")
    );
}

/// ★ 091939Z's bank on 090948Z's treasury: 117 favor, 19 Gold at -4 a turn
/// (under five turns of it), every rival past the vote hold. A drained
/// treasury sells past the hold, off the cadence, down to twenty; a solvent
/// one, or one facing a rival at twelve points, holds as before.
#[test]
fn a_drained_treasury_sells_its_favor_past_the_vote_hold() {
    let rival = |player: usize, gold: f64, dvp: i64| StateRival {
        player,
        gold,
        dvp: Some(dvp),
        ..StateRival::default()
    };
    let held = StateSnapshot {
        turn: 100,
        favor: Some(117.0),
        gold: 19,
        gold_per_turn: Some(-4.0),
        rivals: vec![rival(2, 900.0, 5), rival(3, 400.0, 6)],
        ..StateSnapshot::default()
    };
    assert_ne!(held.turn % FAVOR_SALE_CADENCE, FAVOR_SALE_PHASE);
    assert!(favor_banked_for_votes(&held));
    let science = Some(("science", None));

    // Solvent, the hold stands.
    let mut solvent = held.clone();
    solvent.gold_per_turn = Some(4.0);
    let mut orders = Vec::new();
    assert_eq!(
        append_favor_sale_order(science, &solvent, &mut orders, 2.0),
        Some("favor_hold:contender")
    );
    // A deficit the treasury covers for ten turns is not a drain.
    let mut covered = held.clone();
    covered.gold = 40;
    assert_eq!(
        append_favor_sale_order(science, &covered, &mut orders, 2.0),
        Some("favor_hold:contender")
    );
    assert!(orders.is_empty());

    // Drained: 97 of the 117 go, to the richer rival, off the cadence.
    assert_eq!(
        append_favor_sale_order(science, &held, &mut orders, 2.0),
        None
    );
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].kind, "sell");
    assert_eq!(orders[0].subject, Some(2));
    assert_eq!(orders[0].verb.as_deref(), Some("FAVOR=97"));
    // And the planner's own favor quote is no longer struck.
    let mut quoted = vec![Order {
        kind: "sell",
        subject: Some(2),
        verb: Some("FAVOR=10".to_string()),
        pos: Some((10, 0)),
    }];
    assert_eq!(hold_planner_favor_sales(science, &held, &mut quoted), 0);
    assert_eq!(quoted.len(), 1);

    // The reserve is twenty: 30 favor has too little above it to sell.
    let mut thin = held.clone();
    thin.favor = Some(30.0);
    let mut none = Vec::new();
    assert_eq!(
        append_favor_sale_order(science, &thin, &mut none, 2.0),
        Some("favor_hold:reserve")
    );

    // A rival at twelve points holds even a drained bank.
    let mut match_point = held.clone();
    match_point.rivals[1].dvp = Some(FAVOR_DRAIN_HOLD_DVP);
    assert_eq!(
        append_favor_sale_order(science, &match_point, &mut none, 2.0),
        Some("favor_hold:contender")
    );
    assert_eq!(
        hold_planner_favor_sales(science, &match_point, &mut quoted),
        1
    );

    // A diplomatic racer keeps its bank whatever the treasury reads.
    assert_eq!(
        append_favor_sale_order(
            Some(("diplomacy", Some("diplomatic"))),
            &held,
            &mut none,
            2.0
        ),
        Some("favor_hold:diplomatic_lane")
    );
}

#[test]
fn idle_favor_sells_on_every_plan_that_reports_one() {
    // The bank the live seat actually holds: 300 favor at t141 with three
    // met rivals — one rich, one richer but two points short of the
    // twenty-point win, one at war with us.
    let contended = StateSnapshot {
        turn: 141,
        favor: Some(300.0),
        rivals: vec![
            StateRival {
                player: 2,
                gold: 250.0,
                dvp: Some(3),
                ..StateRival::default()
            },
            StateRival {
                player: 4,
                gold: 1300.0,
                dvp: Some(17),
                ..StateRival::default()
            },
            StateRival {
                player: 5,
                gold: 900.0,
                dvp: Some(2),
                at_war: true,
                ..StateRival::default()
            },
        ],
        ..StateSnapshot::default()
    };
    let science = Some(("science", None));

    // A rival at 17 DVP makes the whole bank votes: nothing sells, ours
    // or the planner's. See `FAVOR_HOLD_DVP`.
    let mut orders = Vec::new();
    assert_eq!(
        append_favor_sale_order(science, &contended, &mut orders, 2.0),
        Some("favor_hold:contender")
    );
    assert!(orders.is_empty());
    let mut quoted = vec![Order {
        kind: "sell",
        subject: Some(4),
        verb: Some("FAVOR=10".to_string()),
        pos: Some((10, 0)),
    }];
    assert_eq!(
        hold_planner_favor_sales(science, &contended, &mut quoted),
        1
    );
    assert!(quoted.is_empty());
    let mut at_four = contended.clone();
    at_four.rivals.retain(|rival| rival.player != 4);
    at_four.rivals[0].dvp = Some(FAVOR_HOLD_DVP);
    assert_eq!(
        append_favor_sale_order(science, &at_four, &mut orders, 2.0),
        Some("favor_hold:contender")
    );

    // Below it, the surplus sells as before.
    let mut state = contended.clone();
    state.rivals.retain(|rival| rival.player != 4);
    assert_eq!(
        append_favor_sale_order(science, &state, &mut orders, 2.0),
        None
    );
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].kind, "sell");
    // Not the one at war: the rich rival at 3 DVP.
    assert_eq!(orders[0].subject, Some(2));
    // 300 − 120 reserve = 180 surplus, capped at one block of 150, at a
    // floor of a gold a point.
    assert_eq!(orders[0].verb.as_deref(), Some("FAVOR=150"));
    // ... at the engine's own book per point (2.0 here), not a flat Gold.
    assert_eq!(orders[0].pos, Some((300, 0)));

    // A Diplomacy STRATEGY on a seat assigned some other lane sells its
    // surplus: the agent's ballot for that seat banks for denial only.
    // Run civvis-20260819T054901Z banked 566 points to t222 and cast none
    // of them, losing to the rival diplomatic victory it was saving for.
    let mut sold = Vec::new();
    assert_eq!(
        append_favor_sale_order(Some(("diplomacy", None)), &state, &mut sold, 2.0),
        None
    );
    assert_eq!(sold.len(), 1, "and the surplus reaches a buyer");
    assert_eq!(sold[0].verb.as_deref(), Some("FAVOR=150"));

    // A seat ASSIGNED the Diplomatic lane holds every point: its ballot
    // (`CivvisDiploRacer`) buys its own participation points and claims
    // with them, and purchased votes register now.
    let racer = Some(("expansion", Some("diplomatic")));
    let mut kept = Vec::new();
    assert_eq!(
        append_favor_sale_order(racer, &state, &mut kept, 2.0),
        Some("favor_hold:diplomatic_lane")
    );
    assert!(kept.is_empty());
    let mut racer_quote = vec![Order {
        kind: "sell",
        subject: Some(2),
        verb: Some("FAVOR=10".to_string()),
        pos: Some((10, 0)),
    }];
    assert_eq!(hold_planner_favor_sales(racer, &state, &mut racer_quote), 1);
    assert!(racer_quote.is_empty());

    // No plan report still holds: an unknown intent is not licence to sell.
    let mut held = Vec::new();
    assert_eq!(
        append_favor_sale_order(None, &state, &mut held, 2.0),
        Some("favor_hold:no_plan")
    );
    assert!(held.is_empty());

    // Off the cadence, nothing; inside the reserve, nothing.
    let mut off_cadence = state.clone();
    off_cadence.turn = 142;
    assert_eq!(
        append_favor_sale_order(science, &off_cadence, &mut held, 2.0),
        Some("favor_hold:cadence")
    );
    let mut small = state.clone();
    small.favor = Some(139.0);
    assert_eq!(
        append_favor_sale_order(science, &small, &mut held, 2.0),
        Some("favor_hold:reserve")
    );
    // The last twenty above the reserve still sell, in a smaller block.
    let mut tail = state.clone();
    tail.favor = Some(140.0);
    assert_eq!(
        append_favor_sale_order(science, &tail, &mut held, 2.0),
        None
    );
    assert_eq!(
        held.pop().and_then(|order| order.verb),
        Some("FAVOR=20".to_string())
    );

    // The planner's own favor quote this turn takes precedence.
    let mut planned = vec![Order {
        kind: "sell",
        subject: Some(2),
        verb: Some("FAVOR=10".to_string()),
        pos: Some((10, 0)),
    }];
    assert_eq!(
        append_favor_sale_order(science, &state, &mut planned, 2.0),
        Some("favor_hold:planner_sale")
    );
    assert_eq!(planned.len(), 1);

    // The planner's own ten-favor block is held on a Diplomacy plan and
    // passes on any other; a resource sale is never touched.
    let mut planner = vec![
        Order {
            kind: "sell",
            subject: Some(2),
            verb: Some("FAVOR=10".to_string()),
            pos: Some((10, 0)),
        },
        Order {
            kind: "sell",
            subject: Some(2),
            verb: Some("RESOURCE_DYES=1".to_string()),
            pos: Some((56, 0)),
        },
    ];
    assert_eq!(hold_planner_favor_sales(science, &state, &mut planner), 0);
    assert_eq!(planner.len(), 2);
    // The diplomacy plan lets the planner's own quote stand now: those ten
    // points were held as "two votes at the next session", and the next
    // session cannot take them.
    assert_eq!(
        hold_planner_favor_sales(Some(("diplomacy", None)), &state, &mut planner),
        0
    );
    assert_eq!(planner.len(), 2);
    // A seat with no plan report still holds: unknown intent is not licence.
    assert_eq!(hold_planner_favor_sales(None, &state, &mut planner), 1);
    assert_eq!(planner.len(), 1);
    assert_eq!(planner[0].verb.as_deref(), Some("RESOURCE_DYES=1"));

    // Nobody eligible: every met rival at war or already near the win.
    let mut nobody = state.clone();
    nobody.rivals.remove(0);
    assert_eq!(
        append_favor_sale_order(science, &nobody, &mut held, 2.0),
        Some("favor_hold:no_buyer")
    );
    assert!(held.is_empty());
}

/// The out-of-space fallback: a Writer the host counts ZERO compatible
/// empty slots for triggers the sale of one placed writing work — to the
/// richest peaceful rival that is not the culture front-runner — so the
/// freed slot seats the idle person and the treasury gets the price.
#[test]
fn a_slot_starved_writer_sells_a_placed_writing_work() {
    let starved_writer = StateUnit {
        id: 90,
        kind: "UNIT_GREAT_WRITER".to_string(),
        great_person: Some(StateGreatPerson {
            class: Some("GREAT_PERSON_CLASS_WRITER".to_string()),
            can_activate: false,
            empty_slots: Some(0),
            ..StateGreatPerson::default()
        }),
        ..StateUnit::default()
    };
    let works_city = StateCity {
        great_works: Some(vec![
            StateGreatWork {
                kind: "GREATWORK_YING_1".to_string(),
                object: "GREATWORKOBJECT_LANDSCAPE".to_string(),
                ..StateGreatWork::default()
            },
            StateGreatWork {
                kind: "GREATWORK_POE_1".to_string(),
                object: "GREATWORKOBJECT_WRITING".to_string(),
                ..StateGreatWork::default()
            },
        ]),
        ..StateCity::default()
    };
    let state = StateSnapshot {
        turn: 143, // 143 % 6 == 5, the lane's phase
        units: vec![starved_writer],
        cities: vec![works_city],
        rivals: vec![
            // The culture front-runner is also the richest: skipped, our
            // losses are culture steals and a work in their museum is
            // their tourism twice over.
            StateRival {
                player: 2,
                gold: 1500.0,
                culture: 90.0,
                ..StateRival::default()
            },
            StateRival {
                player: 4,
                gold: 800.0,
                culture: 30.0,
                ..StateRival::default()
            },
            StateRival {
                player: 5,
                gold: 900.0,
                culture: 20.0,
                at_war: true,
                ..StateRival::default()
            },
        ],
        ..StateSnapshot::default()
    };

    let mut orders = Vec::new();
    assert_eq!(append_work_sale_order(&state, &mut orders), None);
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].kind, "sell");
    assert_eq!(
        orders[0].subject,
        Some(4),
        "not the culture leader, not the war"
    );
    assert_eq!(
        orders[0].verb.as_deref(),
        Some("GREATWORK_POE_1=1"),
        "the writing work, not the landscape the writer cannot replace"
    );
    assert_eq!(orders[0].pos, Some((WORK_SALE_FLOOR, 0)));

    // Off the cadence: hold, whoever is starved.
    let mut off = state.clone();
    off.turn = 144;
    let mut held = Vec::new();
    assert_eq!(
        append_work_sale_order(&off, &mut held),
        Some("work_sale_hold:cadence")
    );

    // `None` slots — an older mod that cannot count — is not starvation;
    // neither is a person the host says can activate right now.
    let mut unknown = state.clone();
    unknown.units[0].great_person.as_mut().unwrap().empty_slots = None;
    assert_eq!(
        append_work_sale_order(&unknown, &mut held),
        Some("work_sale_hold:no_starved_person")
    );
    let mut activatable = state.clone();
    activatable.units[0]
        .great_person
        .as_mut()
        .unwrap()
        .can_activate = true;
    assert_eq!(
        append_work_sale_order(&activatable, &mut held),
        Some("work_sale_hold:no_starved_person")
    );

    // A starved Merchant is not a slot consumer and never triggers.
    let mut merchant = state.clone();
    merchant.units[0].great_person.as_mut().unwrap().class =
        Some("GREAT_PERSON_CLASS_MERCHANT".to_string());
    assert_eq!(
        append_work_sale_order(&merchant, &mut held),
        Some("work_sale_hold:no_starved_person")
    );

    // No placed work of the starved kind: only building capacity helps.
    let mut bare = state.clone();
    bare.cities[0].great_works = Some(vec![StateGreatWork {
        kind: "GREATWORK_YING_1".to_string(),
        object: "GREATWORKOBJECT_LANDSCAPE".to_string(),
        ..StateGreatWork::default()
    }]);
    assert_eq!(
        append_work_sale_order(&bare, &mut held),
        Some("work_sale_hold:no_matching_work")
    );

    // A deal already heading to the buyer's seat yields the turn.
    let mut busy = vec![Order {
        kind: "sell",
        subject: Some(4),
        verb: Some("RESOURCE_DYES=1".to_string()),
        pos: Some((30, 0)),
    }];
    assert_eq!(
        append_work_sale_order(&state, &mut busy),
        Some("work_sale_hold:deal_in_flight")
    );
    assert_eq!(busy.len(), 1);

    // The culture leader is still a buyer when they are the ONLY buyer.
    let mut lone = state.clone();
    lone.rivals = vec![StateRival {
        player: 2,
        gold: 1500.0,
        culture: 90.0,
        ..StateRival::default()
    }];
    let mut only = Vec::new();
    assert_eq!(append_work_sale_order(&lone, &mut only), None);
    assert_eq!(only[0].subject, Some(2));
    assert!(held.is_empty());
}

#[test]
fn final_aid_request_gold_gift_takes_an_affordable_first_place_without_raiding_the_bank() {
    let aid = StateEmergency {
        kind: "EMERGENCY_SEND_AID".to_string(),
        target: 4,
        turns_left: AID_GIFT_FINISH_WINDOW,
        begun: true,
        scores: vec![StateEmergencyScore {
            player: 2,
            score: 200.0,
            tier: Some(1),
        }],
        ours: StateEmergencyOurs {
            member: true,
            score: Some(50.0),
            tier: Some(2),
        },
    };
    let state = StateSnapshot {
        // 251 - the 100 reserve is exactly the 151 score needed to pass
        // the 200-point leader from our 50.
        gold: 251,
        emergencies: Some(vec![aid]),
        rivals: vec![StateRival {
            player: 4,
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    // An ordinary deal for the emergency target would consume the sole
    // working-deal slot. The DVP attempt gets it instead; a different
    // rival's passage request stays intact.
    let mut orders = vec![
        Order {
            kind: "sell",
            subject: Some(4),
            verb: Some("RESOURCE_DYES=1".to_string()),
            pos: Some((30, 0)),
        },
        Order {
            kind: "buy",
            subject: Some(8),
            verb: Some("OPEN_BORDERS".to_string()),
            pos: Some((90, 0)),
        },
    ];
    assert_eq!(append_aid_gift_order(&state, &mut orders), None);
    assert_eq!(orders.len(), 2);
    assert_eq!(orders[0].kind, "aid_gift", "the gift goes first");
    assert_eq!(orders[0].subject, Some(4));
    assert_eq!(orders[0].verb.as_deref(), Some("EMERGENCY_SEND_AID"));
    assert_eq!(orders[0].pos, Some((151, 0)), "one point past first");
    assert_eq!(orders[1].subject, Some(8), "other rivals keep their deal");
}

#[test]
fn aid_gifts_require_a_live_authoritative_finish_line_and_safe_recipient() {
    let emergency = || StateEmergency {
        kind: "EMERGENCY_SEND_MILITARY_AID".to_string(),
        target: 4,
        turns_left: 2,
        begun: true,
        scores: vec![StateEmergencyScore {
            player: 2,
            score: 100.0,
            tier: Some(1),
        }],
        ours: StateEmergencyOurs {
            member: true,
            score: Some(50.0),
            tier: Some(2),
        },
    };
    let state = StateSnapshot {
        gold: 300,
        emergencies: Some(vec![emergency()]),
        rivals: vec![StateRival {
            player: 4,
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    let mut orders = Vec::new();
    assert_eq!(append_aid_gift_order(&state, &mut orders), None);
    assert_eq!(orders[0].pos, Some((51, 0)));

    // A real host queue already delivering the same 200-point project by
    // the deadline is worth more than a redundant treasury spend. Unknown
    // or late estimates do not suppress the finish-line fallback.
    let mut queued = state.clone();
    queued.cities = vec![StateCity {
        producing: Some("PROJECT_SEND_AID".to_string()),
        production_turns: 2.0,
        ..StateCity::default()
    }];
    let mut no_order = Vec::new();
    assert_eq!(
        append_aid_gift_order(&queued, &mut no_order),
        Some("aid_gift_hold:no_finishable_lead")
    );
    assert!(no_order.is_empty());
    queued.cities[0].production_turns = 3.0;
    assert_eq!(append_aid_gift_order(&queued, &mut no_order), None);
    assert_eq!(
        no_order[0].kind, "aid_gift",
        "a project that misses the deadline cannot suppress a gift"
    );

    // The tracker is the authorization. A same-shaped World Games score,
    // a pre-begin event, a non-member, an old/finished event, or an event
    // with more than the official 200-project gap can never spend gold.
    let mutations: [fn(&mut StateEmergency); 7] = [
        |event: &mut StateEmergency| event.kind = "EMERGENCY_WORLD_GAMES".to_string(),
        |event: &mut StateEmergency| event.begun = false,
        |event: &mut StateEmergency| event.ours.member = false,
        |event: &mut StateEmergency| event.turns_left = -1,
        |event: &mut StateEmergency| event.turns_left = AID_GIFT_FINISH_WINDOW + 1,
        |event: &mut StateEmergency| event.scores[0].score = 251.0,
        |event: &mut StateEmergency| event.ours.score = None,
    ];
    for mutate in mutations {
        let mut blocked = state.clone();
        mutate(blocked.emergencies.as_mut().unwrap().first_mut().unwrap());
        let mut no_order = Vec::new();
        assert_eq!(
            append_aid_gift_order(&blocked, &mut no_order),
            Some("aid_gift_hold:no_finishable_lead")
        );
        assert!(no_order.is_empty());
    }

    // The 100-Gold reserve and a known peaceful major recipient are both
    // hard guards.  No amount inferred from the tracker overrides either.
    let mut poor = state.clone();
    poor.gold = 150; // only 50 spendable, one short of the 51 gift
    let mut no_order = Vec::new();
    assert_eq!(
        append_aid_gift_order(&poor, &mut no_order),
        Some("aid_gift_hold:no_finishable_lead")
    );
    let mut unknown_target = state.clone();
    unknown_target.rivals.clear();
    assert_eq!(
        append_aid_gift_order(&unknown_target, &mut no_order),
        Some("aid_gift_hold:no_finishable_lead")
    );
    let mut war_target = state.clone();
    war_target.rivals[0].at_war = true;
    assert_eq!(
        append_aid_gift_order(&war_target, &mut no_order),
        Some("aid_gift_hold:no_finishable_lead")
    );

    // A peace, delegation, or war instruction names a different
    // diplomatic transition and has priority over a one-way Gold deal;
    // retaining it proves the fallback never silently changes that plan.
    for kind in ["peace", "delegation", "war"] {
        let planned = Order {
            kind,
            subject: Some(4),
            verb: Some("OTHER_DIPLOMACY".to_string()),
            pos: None,
        };
        let mut conflict = vec![planned];
        assert_eq!(
            append_aid_gift_order(&state, &mut conflict),
            Some("aid_gift_hold:diplomacy_conflict")
        );
        assert_eq!(conflict.len(), 1);
        assert_eq!(conflict[0].kind, kind);
    }
}

#[test]
fn a_siege_blocker_passage_is_bought_before_an_exploration_gate() {
    // `siege-buys-the-passage`: seat 2 has the explorers, seat 1 none, but
    // seat 1's closed borders hold a siege march, so its passage is asked
    // first; without the blocker the explorers' seat wins as before.
    let state = StateSnapshot {
        turn: 91,
        gold: 200,
        civics: vec![
            "CIVIC_CODE_OF_LAWS".to_string(),
            "CIVIC_EARLY_EMPIRE".to_string(),
        ],
        rivals: vec![
            StateRival {
                player: 2,
                ..StateRival::default()
            },
            StateRival {
                player: 4,
                ..StateRival::default()
            },
        ],
        ..StateSnapshot::default()
    };
    let sealed_by: std::collections::BTreeMap<usize, u32> = [(1, 9), (2, 21)].into_iter().collect();
    let exploration: std::collections::BTreeMap<usize, BorderExplorationPriority> = [(
        2,
        BorderExplorationPriority {
            explorers: 2,
            frontier_tiles: 1,
        },
    )]
    .into_iter()
    .collect();
    let mut plain = Vec::new();
    assert_eq!(
        append_border_buy_order(&sealed_by, &exploration, &state, &mut plain, &|_| 40.0),
        None
    );
    assert_eq!(plain[0].subject, Some(4), "no blocker: the explorers' seat");
    let mut sieged = exploration.clone();
    add_siege_passage_targets(&mut sieged, &[1].into_iter().collect());
    let mut orders = Vec::new();
    assert_eq!(
        append_border_buy_order(&sealed_by, &sieged, &state, &mut orders, &|_| 40.0),
        None
    );
    assert_eq!(orders[0].subject, Some(2), "the siege's blocker first");
    assert_eq!(orders[0].verb.as_deref(), Some("OPEN_BORDERS"));
    // Off the lane's phase the exploration ask waits; the siege's does not.
    let mut off_phase = state.clone();
    off_phase.turn = 92;
    let mut waits = Vec::new();
    assert_eq!(
        append_border_buy_order(&sealed_by, &exploration, &off_phase, &mut waits, &|_| 40.0),
        Some("border_buy_hold:cadence")
    );
    let mut asks = Vec::new();
    assert_eq!(
        append_border_buy_order(&sealed_by, &sieged, &off_phase, &mut asks, &|_| 40.0),
        None
    );
    assert_eq!(asks[0].subject, Some(2), "the siege asks off the phase");
}

#[test]
fn border_buys_rank_usable_seals() {
    // Two peaceful borders are sealed, and the third is at war (which opens
    // its ground by itself). The second peaceful border has more explorer
    // pressure, so it wins before raw sealed-tile count.
    let state = StateSnapshot {
        turn: 91, // 91 % 6 == 1, the lane's phase
        gold: 200,
        civics: vec![
            "CIVIC_CODE_OF_LAWS".to_string(),
            "CIVIC_EARLY_EMPIRE".to_string(),
        ],
        rivals: vec![
            StateRival {
                player: 2,
                ..StateRival::default()
            },
            StateRival {
                player: 4,
                ..StateRival::default()
            },
            StateRival {
                player: 5,
                at_war: true,
                ..StateRival::default()
            },
        ],
        ..StateSnapshot::default()
    };
    let sealed_by: std::collections::BTreeMap<usize, u32> =
        [(1, 7), (2, 21), (3, 40)].into_iter().collect();
    let exploration: std::collections::BTreeMap<usize, BorderExplorationPriority> = [
        (
            1,
            BorderExplorationPriority {
                explorers: 1,
                frontier_tiles: 2,
            },
        ),
        (
            2,
            BorderExplorationPriority {
                explorers: 2,
                frontier_tiles: 1,
            },
        ),
    ]
    .into_iter()
    .collect();

    let mut orders = Vec::new();
    assert_eq!(
        append_border_buy_order(&sealed_by, &exploration, &state, &mut orders, &|_| 40.0),
        None
    );
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].kind, "buy");
    // Seat 3 seals the most but is at war. Seat 2 (host player 4) is the
    // useful passage: two explorers can reach its frontier.
    assert_eq!(orders[0].subject, Some(4));
    assert_eq!(orders[0].verb.as_deref(), Some("OPEN_BORDERS"));
    // The engine's book for this passage (40 here) under the treasury's
    // 200 − 60 = 140 and the cap: the price is what passage is worth.
    assert_eq!(orders[0].pos, Some((40, 0)));

    // A grant already in hand retires the trigger; the next-worst seal
    // takes over.
    let mut granted = state.clone();
    granted.rivals[1].open_borders = Some(true);
    let mut next = Vec::new();
    assert_eq!(
        append_border_buy_order(&sealed_by, &exploration, &granted, &mut next, &|_| 40.0),
        None
    );
    assert_eq!(next[0].subject, Some(2));

    // Small seals are not worth a deal window; no majors sealing, no ask.
    let trivial: std::collections::BTreeMap<usize, u32> =
        [(1, BORDER_BUY_MIN_SEALED - 1)].into_iter().collect();
    let mut held = Vec::new();
    assert_eq!(
        append_border_buy_order(&trivial, &exploration, &state, &mut held, &|_| 40.0),
        Some("border_buy_hold:no_seal")
    );
    assert_eq!(
        append_border_buy_order(
            &Default::default(),
            &exploration,
            &state,
            &mut held,
            &|_| 40.0,
        ),
        Some("border_buy_hold:no_seal")
    );
    assert_eq!(
        append_border_buy_order(&sealed_by, &Default::default(), &state, &mut held, &|_| {
            40.0
        },),
        Some("border_buy_hold:no_exploration_need")
    );

    // Off the cadence, without the civic, or too poor to meet a minimum
    // ask: held, with the seal still standing.
    let mut off_cadence = state.clone();
    off_cadence.turn = 92;
    assert_eq!(
        append_border_buy_order(&sealed_by, &exploration, &off_cadence, &mut held, &|_| 40.0,),
        Some("border_buy_hold:cadence")
    );
    let mut no_civic = state.clone();
    no_civic.civics = vec!["CIVIC_CODE_OF_LAWS".to_string()];
    assert_eq!(
        append_border_buy_order(&sealed_by, &exploration, &no_civic, &mut held, &|_| 40.0,),
        Some("border_buy_hold:no_civic")
    );
    let mut poor = state.clone();
    poor.gold = 89; // 89 − 60 reserve = 29, one short of the minimum
    assert_eq!(
        append_border_buy_order(&sealed_by, &exploration, &poor, &mut held, &|_| 40.0,),
        Some("border_buy_hold:treasury")
    );
    assert!(held.is_empty());

    // A deal already heading to the same rival this turn holds the buy —
    // the mod runs one working deal per rival at a time.
    let mut in_flight = vec![Order {
        kind: "sell",
        subject: Some(4),
        verb: Some("FAVOR=10".to_string()),
        pos: Some((10, 0)),
    }];
    assert_eq!(
        append_border_buy_order(&sealed_by, &exploration, &state, &mut in_flight, &|_| 40.0,),
        Some("border_buy_hold:deal_in_flight")
    );
    assert_eq!(in_flight.len(), 1);
}

#[test]
fn border_buys_follow_a_scout_toward_fog_and_unmet_contacts() {
    // Two peaceful majors have enough sealed ground to sell passage. The
    // larger French seal is remote; our Scout is one step from Kongo's
    // border, which is itself beside unknown ground and the lobby still
    // has an uncontacted major and city-states. Host player 3 is omitted
    // from the met-gated rival list, so Kongo remains model seat 2 and
    // France remains model seat 4.
    let mut kongo_border = grass(4, 3);
    kongo_border.o = 2;
    let mut france_border = grass(10, 5);
    france_border.o = 4;
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 91,
        width: 12,
        height: 8,
        chunk: 1,
        plots: vec![grass(3, 3), kongo_border, france_border],
    }]);
    let state = StateSnapshot {
        turn: 91,
        gold: 200,
        civics: vec!["CIVIC_EARLY_EMPIRE".to_string()],
        seat: civvis::mirror::Seat {
            players: 5,
            city_states: 3,
            ..civvis::mirror::Seat::default()
        },
        units: vec![StateUnit {
            id: 91,
            kind: "UNIT_SCOUT".to_string(),
            x: 3,
            y: 3,
            hp: 100.0,
            ..StateUnit::default()
        }],
        rivals: vec![
            StateRival {
                player: 2,
                civ: "CIVILIZATION_KONGO".to_string(),
                enforces_borders: Some(true),
                ..StateRival::default()
            },
            StateRival {
                player: 4,
                civ: "CIVILIZATION_FRANCE".to_string(),
                enforces_borders: Some(true),
                ..StateRival::default()
            },
        ],
        minors: vec![StateMinor {
            player: 7,
            civ: "CIVILIZATION_GENEVA".to_string(),
            ..StateMinor::default()
        }],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 5, 3, 250, 0);
    assert!(
        mirror.uid_of.contains_key(&91),
        "fixture precondition: the Scout reaches the mirrored board"
    );
    let sealed_by: std::collections::BTreeMap<usize, u32> = [(2, 7), (4, 21)].into_iter().collect();
    let exploration = border_buy_exploration_targets(&snapshot, &state, &mirror.game, &sealed_by);
    let kongo = exploration
        .get(&2)
        .expect("the nearby Scout can use Kongo's frontier");
    assert_eq!(kongo.explorers, 1);
    assert!(kongo.frontier_tiles > 0);
    assert!(
        !exploration.contains_key(&4),
        "a larger seal outside the Scout's approach is not an exploration purchase"
    );

    let mut orders = Vec::new();
    assert_eq!(
        append_border_buy_order(&sealed_by, &exploration, &state, &mut orders, &|_| 40.0),
        None
    );
    assert_eq!(orders[0].subject, Some(2));
    assert_eq!(orders[0].verb.as_deref(), Some("OPEN_BORDERS"));
}

#[test]
fn order_targets_keep_host_ids_when_a_met_gated_rival_is_missing() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 100,
        width: 8,
        height: 8,
        chunk: 1,
        plots: (0..8)
            .flat_map(|x| (0..8).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        seat: civvis::mirror::Seat {
            local_player: 0,
            players: 6,
            ..civvis::mirror::Seat::default()
        },
        rivals: vec![
            StateRival {
                player: 1,
                ..StateRival::default()
            },
            StateRival {
                player: 2,
                ..StateRival::default()
            },
            // Host player 3 is not met and is absent from this export.
            StateRival {
                player: 4,
                ..StateRival::default()
            },
        ],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 6, 1, 250, 0);

    assert_eq!(host_player_target(&mirror, &state, 1), Some(1));
    assert_eq!(host_player_target(&mirror, &state, 2), Some(2));
    assert_eq!(host_player_target(&mirror, &state, 3), None);
    assert_eq!(host_player_target(&mirror, &state, 4), Some(4));

    let war = translate(&Action::DeclareWar { player: 4 }, &mirror, &state)
        .expect("a mapped rival's war declaration crosses");
    assert_eq!(war.subject, Some(4));
    let peace = translate(&Action::MakePeace { player: 4 }, &mirror, &state)
        .expect("a mapped rival's peace offer crosses");
    assert_eq!(peace.subject, Some(4));
}

#[test]
fn passage_exploration_stops_when_the_map_and_contacts_are_complete() {
    let complete = Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 2,
        height: 2,
        chunk: 1,
        plots: (0..2)
            .flat_map(|x| (0..2).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let mut state = StateSnapshot {
        seat: civvis::mirror::Seat {
            players: 3,
            city_states: 2,
            ..civvis::mirror::Seat::default()
        },
        rivals: vec![
            StateRival {
                player: 1,
                ..StateRival::default()
            },
            StateRival {
                player: 2,
                ..StateRival::default()
            },
        ],
        minors: vec![
            StateMinor {
                player: 3,
                civ: "CIVILIZATION_GENEVA".to_string(),
                ..StateMinor::default()
            },
            StateMinor {
                player: 4,
                civ: "CIVILIZATION_KABUL".to_string(),
                ..StateMinor::default()
            },
        ],
        ..StateSnapshot::default()
    };
    assert!(
        !exploration_still_matters(&complete, &state),
        "there is no discovery job after every map tile and configured contact is known"
    );
    state.minors.pop();
    assert!(
        exploration_still_matters(&complete, &state),
        "an uncontacted city-state keeps the exploration objective live"
    );
    state.minors.push(StateMinor {
        player: 4,
        civ: "CIVILIZATION_KABUL".to_string(),
        ..StateMinor::default()
    });
    state.rivals.pop();
    assert!(
        exploration_still_matters(&complete, &state),
        "an uncontacted major does too"
    );
}

#[test]
fn luxury_buys_fill_the_amenity_gap() {
    // Two cities one Amenity short and one with a surplus: a deficit of
    // two by the host's own ledger, on the lane's turn, with income to
    // carry a copy and three majors met — one at war, one broke.
    let short = |needed: f64, have: f64| StateCity {
        amenities: have,
        amenities_needed: needed,
        ..StateCity::default()
    };
    let state = StateSnapshot {
        turn: 94, // 94 % 6 == 4, the lane's phase
        gold: 200,
        gold_per_turn: Some(12.0),
        cities: vec![short(5.0, 4.0), short(6.0, 5.0), short(3.0, 4.0)],
        rivals: vec![
            StateRival {
                player: 2,
                gold: 300.0,
                tradeable_luxuries: Some(vec!["RESOURCE_SILK".to_string()]),
                ..StateRival::default()
            },
            StateRival {
                player: 4,
                gold: 500.0,
                tradeable_luxuries: Some(vec!["RESOURCE_AMBER".to_string()]),
                ..StateRival::default()
            },
            StateRival {
                player: 5,
                gold: 900.0,
                at_war: true,
                ..StateRival::default()
            },
            StateRival {
                player: 6,
                tradeable_luxuries: Some(Vec::new()),
                ..StateRival::default()
            },
        ],
        ..StateSnapshot::default()
    };
    assert_eq!(amenity_deficit(&state), 2.0);

    let mut orders = Vec::new();
    assert_eq!(append_luxury_buy_order(&state, &mut orders), None);
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].kind, "buy");
    // The richest major at peace; seat 5 is richer and at war, seat 6's
    // treasury is unknown and reads as nothing.
    assert_eq!(orders[0].subject, Some(4));
    assert_eq!(orders[0].verb.as_deref(), Some("LUXURY_ANY"));
    // 135 + 10·3 = 165, under what 12 a turn carries (25·(12−4) = 200)
    // and the cap.
    assert_eq!(orders[0].pos, Some((165, 0)));

    // A richer rival with no catalogue entry must not consume the only
    // purchase window: the host would answer `buy_no_luxury`.
    let mut no_stock = state.clone();
    no_stock.rivals[1].tradeable_luxuries = Some(Vec::new());
    let mut stock_orders = Vec::new();
    assert_eq!(append_luxury_buy_order(&no_stock, &mut stock_orders), None);
    assert_eq!(stock_orders[0].subject, Some(2));

    // A host export that explicitly reports no stock is different from
    // an older export with no field at all; do not send a blind deal in
    // either case.
    let mut no_known_stock = state.clone();
    for rival in &mut no_known_stock.rivals {
        rival.tradeable_luxuries = Some(Vec::new());
    }
    let mut no_stock_orders = Vec::new();
    assert_eq!(
        append_luxury_buy_order(&no_known_stock, &mut no_stock_orders),
        Some("luxury_buy_hold:no_stock")
    );
    assert!(no_stock_orders.is_empty());
    let mut old_export = state.clone();
    for rival in &mut old_export.rivals {
        rival.tradeable_luxuries = None;
    }
    let mut old_orders = Vec::new();
    assert_eq!(
        append_luxury_buy_order(&old_export, &mut old_orders),
        Some("luxury_buy_hold:no_seller")
    );
    assert!(old_orders.is_empty());

    // The income binds: 8 a turn carries 100 at the 25× book with 4 kept
    // clear; the cap binds a wide empire on a fat income.
    let mut lean = state.clone();
    lean.gold_per_turn = Some(8.0);
    let mut lean_orders = Vec::new();
    assert_eq!(append_luxury_buy_order(&lean, &mut lean_orders), None);
    assert_eq!(lean_orders[0].pos, Some((100, 0)));
    let mut wide = state.clone();
    wide.gold_per_turn = Some(40.0);
    wide.cities = (0..20).map(|_| short(6.0, 5.0)).collect();
    let mut wide_orders = Vec::new();
    assert_eq!(append_luxury_buy_order(&wide, &mut wide_orders), None);
    assert_eq!(wide_orders[0].pos, Some((300, 0)));

    // Content, or with only a ledger the host never sent: no ask.
    let mut held = Vec::new();
    let mut content = state.clone();
    content.cities = vec![short(5.0, 4.0), short(3.0, 4.0)];
    assert_eq!(
        append_luxury_buy_order(&content, &mut held),
        Some("luxury_buy_hold:content")
    );
    let mut unknown = state.clone();
    unknown.cities = vec![
        short(f64::NAN, f64::NAN),
        short(-1.0, -1.0),
        short(9.0, 4.0),
    ];
    assert_eq!(amenity_deficit(&unknown), 5.0);
    unknown.cities.pop();
    assert_eq!(
        append_luxury_buy_order(&unknown, &mut held),
        Some("luxury_buy_hold:content")
    );

    // Off the cadence, income unknown, income too thin for the cheapest
    // ask on record, or the treasury under the reserve: held.
    let mut off_cadence = state.clone();
    off_cadence.turn = 95;
    assert_eq!(
        append_luxury_buy_order(&off_cadence, &mut held),
        Some("luxury_buy_hold:cadence")
    );
    let mut blind = state.clone();
    blind.gold_per_turn = None;
    assert_eq!(
        append_luxury_buy_order(&blind, &mut held),
        Some("luxury_buy_hold:income_unknown")
    );
    let mut thin = state.clone();
    thin.gold_per_turn = Some(5.0); // 25·(5−4) = 25, under the 50 minimum
    assert_eq!(
        append_luxury_buy_order(&thin, &mut held),
        Some("luxury_buy_hold:income")
    );
    // A severe deficit can be the reason income is already below the
    // normal floor. Use only treasury room above the reserve for one
    // emergency ask; ordinary thin income remains held above.
    let mut emergency = thin.clone();
    emergency.gold = 132;
    emergency.gold_per_turn = Some(-3.5);
    emergency.cities = vec![short(8.0, 4.0)];
    let mut emergency_orders = Vec::new();
    assert_eq!(
        append_luxury_buy_order(&emergency, &mut emergency_orders),
        None
    );
    assert_eq!(emergency_orders.len(), 1);
    assert_eq!(emergency_orders[0].subject, Some(4));
    assert_eq!(emergency_orders[0].pos, Some((72, 0)));
    let mut poor = state.clone();
    poor.gold = 59;
    assert_eq!(
        append_luxury_buy_order(&poor, &mut held),
        Some("luxury_buy_hold:treasury")
    );
    assert!(held.is_empty());

    // A deal already heading to the richest seat this turn passes the
    // ask to the next-richest; with every peaceful seat busy, or nobody
    // met, there is nobody to buy from.
    let mut in_flight = vec![Order {
        kind: "sell",
        subject: Some(4),
        verb: Some("FAVOR=10".to_string()),
        pos: Some((10, 0)),
    }];
    assert_eq!(append_luxury_buy_order(&state, &mut in_flight), None);
    assert_eq!(in_flight.len(), 2);
    assert_eq!(in_flight[1].subject, Some(2));
    let mut all_busy: Vec<Order> = [2, 4, 6]
        .into_iter()
        .map(|seat| Order {
            kind: "buy",
            subject: Some(seat),
            verb: Some("OPEN_BORDERS".to_string()),
            pos: Some((40, 0)),
        })
        .collect();
    assert_eq!(
        append_luxury_buy_order(&state, &mut all_busy),
        Some("luxury_buy_hold:no_seller")
    );
    assert_eq!(all_busy.len(), 3);
    let mut alone = state.clone();
    alone.rivals.clear();
    assert_eq!(
        append_luxury_buy_order(&alone, &mut held),
        Some("luxury_buy_hold:no_seller")
    );
}

/// `luxury-swap-asks`: a spare copy of ours for a luxury the seat lacks,
/// at the rival the turn rotates to among those at peace that offer one.
#[test]
fn luxury_swaps_offer_a_spare_for_a_type_we_lack() {
    let short = |needed: f64, have: f64| StateCity {
        amenities: have,
        amenities_needed: needed,
        ..StateCity::default()
    };
    let counts = |pairs: &[(&str, f64)]| {
        Some(
            pairs
                .iter()
                .map(|(name, count)| (name.to_string(), *count))
                .collect(),
        )
    };
    let offering = |player, luxuries: &[&str]| StateRival {
        player,
        tradeable_luxuries: Some(luxuries.iter().map(|name| name.to_string()).collect()),
        ..StateRival::default()
    };
    let state = StateSnapshot {
        turn: 94, // 94 % 6 == 4, the luxury slot; 94 / 6 = 15
        gold: 200,
        gold_per_turn: Some(12.0),
        cities: vec![short(5.0, 4.0), short(6.0, 5.0), short(3.0, 4.0)],
        luxury_counts: counts(&[
            ("RESOURCE_SPICES", 3.0),
            ("RESOURCE_HONEY", 1.0),
            ("RESOURCE_TURTLES", 2.0),
        ]),
        rivals: vec![
            offering(2, &["RESOURCE_SILK"]),
            offering(4, &["RESOURCE_AMBER"]),
            StateRival {
                player: 5,
                at_war: true,
                ..offering(5, &["RESOURCE_JADE"])
            },
            offering(6, &[]),
        ],
        ..StateSnapshot::default()
    };

    // Three spare copies: one goes, the type held most of; the partner
    // is seat 4 (15 % 2 == 1 of seats 2 and 4); Gold up to a copy's
    // worth, 135 + 10·3 = 165, which the treasury and income carry.
    let mut orders = Vec::new();
    assert_eq!(append_luxury_swap_order(&state, &mut orders, None), None);
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].kind, "buy");
    assert_eq!(orders[0].subject, Some(4));
    assert_eq!(
        orders[0].verb.as_deref(),
        Some("LUXURY_ANY;RESOURCE_SPICES=1")
    );
    assert_eq!(orders[0].pos, Some((165, 0)));

    // The next week rotates to the other partner.
    let mut next = state.clone();
    next.turn = 100;
    let mut next_orders = Vec::new();
    assert_eq!(
        append_luxury_swap_order(&next, &mut next_orders, None),
        None
    );
    assert_eq!(next_orders[0].subject, Some(2));

    // Four spare copies or more: two go, one of each of the two largest;
    // a single spare type gives two of itself.
    let mut rich = state.clone();
    rich.luxury_counts = counts(&[("RESOURCE_SPICES", 4.0), ("RESOURCE_TURTLES", 2.0)]);
    let mut rich_orders = Vec::new();
    assert_eq!(
        append_luxury_swap_order(&rich, &mut rich_orders, None),
        None
    );
    assert_eq!(
        rich_orders[0].verb.as_deref(),
        Some("LUXURY_ANY;RESOURCE_SPICES=1,RESOURCE_TURTLES=1")
    );
    let mut one_type = state.clone();
    one_type.luxury_counts = counts(&[("RESOURCE_SPICES", 5.0)]);
    let mut one_orders = Vec::new();
    assert_eq!(
        append_luxury_swap_order(&one_type, &mut one_orders, None),
        None
    );
    assert_eq!(
        one_orders[0].verb.as_deref(),
        Some("LUXURY_ANY;RESOURCE_SPICES=2")
    );

    // A spare another sale gives this turn stays home; the next spare
    // goes instead, and the buyer's seat is busy.
    let mut selling = vec![Order {
        kind: "sell",
        subject: Some(2),
        verb: Some("RESOURCE_SPICES=1".to_string()),
        pos: Some((24, 0)),
    }];
    assert_eq!(append_luxury_swap_order(&state, &mut selling, None), None);
    assert_eq!(selling.len(), 2);
    assert_eq!(selling[1].subject, Some(4));
    assert_eq!(
        selling[1].verb.as_deref(),
        Some("LUXURY_ANY;RESOURCE_TURTLES=1")
    );

    // No Gold to top up: the swap is still asked, at a ceiling of 0.
    let mut broke = state.clone();
    broke.gold = 10;
    broke.gold_per_turn = Some(-2.0);
    let mut broke_orders = Vec::new();
    assert_eq!(
        append_luxury_swap_order(&broke, &mut broke_orders, None),
        None
    );
    assert_eq!(broke_orders[0].pos, Some((0, 0)));

    // Held: content, off the slot, no export of our copies, no spare, or
    // nobody at peace offering a type we lack (a denunciation inside the
    // window either way counts the rival out).
    let mut held = Vec::new();
    let mut content = state.clone();
    content.cities = vec![short(5.0, 4.0), short(3.0, 4.0)];
    assert_eq!(
        append_luxury_swap_order(&content, &mut held, None),
        Some("luxury_swap_hold:content")
    );
    let mut off_slot = state.clone();
    off_slot.turn = 95;
    assert_eq!(
        append_luxury_swap_order(&off_slot, &mut held, None),
        Some("luxury_swap_hold:cadence")
    );
    let mut blind = state.clone();
    blind.luxury_counts = None;
    assert_eq!(
        append_luxury_swap_order(&blind, &mut held, None),
        Some("luxury_swap_hold:counts_unknown")
    );
    let mut lean = state.clone();
    lean.luxury_counts = counts(&[("RESOURCE_SPICES", 1.0), ("RESOURCE_HONEY", 1.0)]);
    assert_eq!(
        append_luxury_swap_order(&lean, &mut held, None),
        Some("luxury_swap_hold:no_spare")
    );
    let mut denounced = state.clone();
    denounced.rivals[0].our_denounce_turn = Some(80);
    denounced.rivals[1].their_denounce_turn = Some(70);
    assert_eq!(
        append_luxury_swap_order(&denounced, &mut held, None),
        Some("luxury_swap_hold:no_partner")
    );
    denounced.rivals[1].their_denounce_turn = Some(60);
    assert_eq!(append_luxury_swap_order(&denounced, &mut held, None), None);
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].subject, Some(4));
}

/// `luxury-swap-asks-2`: asked from one Amenity short, and only of a
/// partner whose board holds none of a spare we put up, that spare
/// first; a partner the board cannot read comes after a known lack.
#[test]
fn luxury_swaps_2_offer_the_spare_the_partner_lacks() {
    let short = |needed: f64, have: f64| StateCity {
        amenities: have,
        amenities_needed: needed,
        ..StateCity::default()
    };
    let offering = |player, luxuries: &[&str]| StateRival {
        player,
        tradeable_luxuries: Some(luxuries.iter().map(|name| name.to_string()).collect()),
        ..StateRival::default()
    };
    let state = StateSnapshot {
        turn: 94, // the luxury slot; week 15
        gold: 200,
        gold_per_turn: Some(12.0),
        // One Amenity short in all: version one holds.
        cities: vec![short(5.0, 4.0), short(3.0, 4.0)],
        luxury_counts: Some(
            [
                ("RESOURCE_SPICES", 3.0),
                ("RESOURCE_HONEY", 1.0),
                ("RESOURCE_TURTLES", 2.0),
            ]
            .iter()
            .map(|(name, count)| (name.to_string(), *count))
            .collect(),
        ),
        rivals: vec![
            offering(2, &["RESOURCE_SILK"]),
            offering(4, &["RESOURCE_AMBER"]),
        ],
        ..StateSnapshot::default()
    };
    let mut held = Vec::new();
    assert_eq!(
        append_luxury_swap_order(&state, &mut held, None),
        Some("luxury_swap_hold:content")
    );

    // Seat 2's land holds Spices, not Turtles; seat 4's holds both. Seat 2
    // is the only fit and gets the Turtles, though version one's
    // rotation would have sent Spices to seat 4.
    let board = |seat: i64, luxury: &str| match (seat, luxury) {
        (2, "RESOURCE_TURTLES") => Some(true),
        _ => Some(false),
    };
    let mut orders = Vec::new();
    assert_eq!(
        append_luxury_swap_order(&state, &mut orders, Some(&board)),
        None
    );
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].subject, Some(2));
    assert_eq!(
        orders[0].verb.as_deref(),
        Some("LUXURY_ANY;RESOURCE_TURTLES=1")
    );
    assert_eq!(orders[0].pos, Some((155, 0)));

    // Both seats fit: the week rotates among them, and a spare the board
    // cannot read goes after a known lack.
    let open = |seat: i64, luxury: &str| match (seat, luxury) {
        (4, "RESOURCE_TURTLES") => Some(true),
        (4, _) => None,
        (2, "RESOURCE_SPICES") => Some(true),
        _ => Some(false),
    };
    let mut rotated = Vec::new();
    assert_eq!(
        append_luxury_swap_order(&state, &mut rotated, Some(&open)),
        None
    );
    assert_eq!(rotated[0].subject, Some(4));
    assert_eq!(
        rotated[0].verb.as_deref(),
        Some("LUXURY_ANY;RESOURCE_TURTLES=1")
    );

    // Four spares or more: two go, both ones the partner lacks; a lone
    // fitting type with one spare gives one copy.
    let mut rich = state.clone();
    rich.luxury_counts = Some(
        [
            ("RESOURCE_SPICES", 4.0),
            ("RESOURCE_TURTLES", 2.0),
            ("RESOURCE_JADE", 2.0),
        ]
        .iter()
        .map(|(name, count)| (name.to_string(), *count))
        .collect(),
    );
    let lacks_two = |_: i64, luxury: &str| Some(luxury != "RESOURCE_SPICES");
    let mut rich_orders = Vec::new();
    assert_eq!(
        append_luxury_swap_order(&rich, &mut rich_orders, Some(&lacks_two)),
        None
    );
    assert_eq!(
        rich_orders[0].verb.as_deref(),
        Some("LUXURY_ANY;RESOURCE_JADE=1,RESOURCE_TURTLES=1")
    );
    let mut rich_one = Vec::new();
    assert_eq!(
        append_luxury_swap_order(&rich, &mut rich_one, Some(&board)),
        None
    );
    assert_eq!(rich_one[0].subject, Some(2));
    assert_eq!(
        rich_one[0].verb.as_deref(),
        Some("LUXURY_ANY;RESOURCE_TURTLES=1")
    );

    // Every partner's land holds every spare: held, not asked.
    let full = |_: i64, _: &str| Some(false);
    assert_eq!(
        append_luxury_swap_order(&state, &mut held, Some(&full)),
        Some("luxury_swap_hold:no_fit")
    );
    assert!(held.is_empty());
}

/// `luxury-asks-pay-in-strategics`: with no spare copy that fits, the
/// luxury slot asks a partner at peace that offers a type we lack, paying
/// the smallest lot of the strategic its book prices at a copy's worth.
#[test]
fn luxury_strategic_ask_pays_in_strategics_the_army_does_not_need() {
    let short = |needed: f64, have: f64| StateCity {
        amenities: have,
        amenities_needed: needed,
        ..StateCity::default()
    };
    let offering = |player, luxuries: &[&str]| StateRival {
        player,
        tradeable_luxuries: Some(luxuries.iter().map(|name| name.to_string()).collect()),
        ..StateRival::default()
    };
    let spare = |resource: &str, id: &str, amount: u32, luxury: bool| civvis::ai::BarterSpare {
        resource: resource.to_string(),
        resource_id: id.to_string(),
        amount,
        luxury,
    };
    let state = StateSnapshot {
        turn: 94, // the luxury slot; week 15
        gold: 200,
        gold_per_turn: Some(12.0),
        cities: vec![short(5.0, 4.0), short(6.0, 5.0), short(3.0, 4.0)],
        luxury_counts: Some([("RESOURCE_AMBER".to_string(), 1.0)].into_iter().collect()),
        rivals: vec![
            // Offers only what we hold: no partner.
            offering(2, &["RESOURCE_AMBER"]),
            offering(4, &["RESOURCE_SILK", "RESOURCE_AMBER"]),
            StateRival {
                at_war: true,
                ..offering(5, &["RESOURCE_JADE"])
            },
            offering(6, &["RESOURCE_WINE"]),
        ],
        ..StateSnapshot::default()
    };
    // Niter is worth 20 a unit to every seller, Iron 5; luxury spares are
    // never paid here.
    let book = |_seller: i64, resource: &str, amount: u32| match resource {
        "niter" => 20.0 * f64::from(amount),
        "iron" => 5.0 * f64::from(amount),
        _ => 0.0,
    };
    let spares = vec![
        spare("RESOURCE_COCOA", "cocoa", 2, true),
        spare("RESOURCE_IRON", "iron", 30, false),
        spare("RESOURCE_NITER", "niter", 12, false),
    ];

    // A copy is worth 135 + 10·3 = 165: 9 Niter (180) meets it, where 30
    // Iron (150) does not. Partners 4 and 6; 15 % 2 == 1 picks seat 6.
    // Gold tops up to a copy's worth, which the treasury carries.
    let mut orders = Vec::new();
    assert_eq!(
        append_luxury_strategic_ask(&state, &mut orders, &spares, &book),
        None
    );
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].kind, "buy");
    assert_eq!(orders[0].subject, Some(6));
    assert_eq!(
        orders[0].verb.as_deref(),
        Some("LUXURY_ANY;RESOURCE_NITER=9")
    );
    assert_eq!(orders[0].pos, Some((165, 0)));

    // Short of Niter, the whole Iron spare goes; a broke treasury tops up
    // nothing.
    let iron_only = vec![spare("RESOURCE_IRON", "iron", 30, false)];
    let mut iron_orders = Vec::new();
    let mut broke = state.clone();
    broke.gold = 10;
    broke.gold_per_turn = Some(-2.0);
    assert_eq!(
        append_luxury_strategic_ask(&broke, &mut iron_orders, &iron_only, &book),
        None
    );
    assert_eq!(
        iron_orders[0].verb.as_deref(),
        Some("LUXURY_ANY;RESOURCE_IRON=30")
    );
    assert_eq!(iron_orders[0].pos, Some((0, 0)));

    // Unpriced (the seller's seat cannot be named): a fixed lot.
    let blind_book = |_: i64, _: &str, _: u32| 0.0;
    let mut blind_orders = Vec::new();
    assert_eq!(
        append_luxury_strategic_ask(&state, &mut blind_orders, &iron_only, &blind_book),
        None
    );
    assert_eq!(
        blind_orders[0].verb.as_deref(),
        Some("LUXURY_ANY;RESOURCE_IRON=10")
    );

    // A strategic another sale gives this turn stays home, and its buyer
    // is busy: Iron goes to seat 4.
    let mut selling = vec![Order {
        kind: "sell",
        subject: Some(6),
        verb: Some("RESOURCE_NITER=10".to_string()),
        pos: Some((24, 0)),
    }];
    assert_eq!(
        append_luxury_strategic_ask(&state, &mut selling, &spares, &book),
        None
    );
    assert_eq!(selling.len(), 2);
    assert_eq!(selling[1].subject, Some(4));
    assert_eq!(
        selling[1].verb.as_deref(),
        Some("LUXURY_ANY;RESOURCE_IRON=30")
    );

    // Held: content, off the slot, the slot already asked, no strategic
    // to pay with, nobody at peace offering a type we lack.
    let mut held = Vec::new();
    let mut content = state.clone();
    content.cities = vec![short(3.0, 4.0)];
    assert_eq!(
        append_luxury_strategic_ask(&content, &mut held, &spares, &book),
        Some("luxury_strategic_hold:content")
    );
    let mut off_slot = state.clone();
    off_slot.turn = 95;
    assert_eq!(
        append_luxury_strategic_ask(&off_slot, &mut held, &spares, &book),
        Some("luxury_strategic_hold:cadence")
    );
    let mut asked = vec![Order {
        kind: "buy",
        subject: Some(4),
        verb: Some("LUXURY_ANY;RESOURCE_COCOA=1".to_string()),
        pos: Some((0, 0)),
    }];
    assert_eq!(
        append_luxury_strategic_ask(&state, &mut asked, &spares, &book),
        Some("luxury_strategic_hold:slot_taken")
    );
    let luxury_only = vec![spare("RESOURCE_COCOA", "cocoa", 2, true)];
    assert_eq!(
        append_luxury_strategic_ask(&state, &mut held, &luxury_only, &book),
        Some("luxury_strategic_hold:no_strategic")
    );
    let mut lonely = state.clone();
    lonely.rivals = vec![offering(2, &["RESOURCE_AMBER"])];
    assert_eq!(
        append_luxury_strategic_ask(&lonely, &mut held, &spares, &book),
        Some("luxury_strategic_hold:no_partner")
    );
    assert!(held.is_empty());
}

/// `siege-buys-the-gun-resource`: the Bombard's Niter, asked of the
/// largest offer at peace: the full lot when our side covers it, else one
/// gun's worth, Gold-only when nothing of ours is spare.
#[test]
fn the_guns_resource_is_bought_from_the_largest_offer() {
    let want = |resource: &str, id: &str, amount: u32, minimum: u32| civvis::ai::GunResourceWant {
        resource: resource.to_string(),
        resource_id: id.to_string(),
        amount,
        minimum,
        unit: "bombard".to_string(),
        guns: 3,
        hit: 24.6,
        renewal: false,
    };
    let offers = |pairs: &[(&str, i64)]| {
        Some(
            pairs
                .iter()
                .map(|(resource, amount)| (resource.to_string(), *amount))
                .collect::<std::collections::BTreeMap<_, _>>(),
        )
    };
    // The engine's book for a first strategic: a primary value, then 24
    // a unit. Nothing of ours is spare here.
    let worth = |_: &str, amount: u32| 185.0 + 24.0 * (f64::from(amount) - 1.0);
    let no_barter = |_: i64, _: &str, _: u32| 0.0;
    let state = StateSnapshot {
        turn: 152, // 152 % 3 == 2, the lane's phase
        gold: 500,
        gold_per_turn: Some(20.0),
        rivals: vec![
            StateRival {
                player: 2,
                tradeable_strategics: offers(&[("RESOURCE_NITER", 82)]),
                ..StateRival::default()
            },
            StateRival {
                player: 4,
                tradeable_strategics: offers(&[("RESOURCE_NITER", 70)]),
                ..StateRival::default()
            },
            StateRival {
                player: 5,
                at_war: true,
                tradeable_strategics: offers(&[("RESOURCE_NITER", 200)]),
                ..StateRival::default()
            },
        ],
        ..StateSnapshot::default()
    };
    let wants = vec![want("RESOURCE_NITER", "niter", 50, 20)];
    let buy = |state: &StateSnapshot, orders: &mut Vec<Order>| {
        append_gun_resource_buy_order(&wants, &[], state, orders, &worth, &no_barter)
    };
    let mut orders = Vec::new();
    assert_eq!(buy(&state, &mut orders), None);
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].kind, "buy");
    // The largest offer at peace; seat 5 offers more and is at war.
    assert_eq!(orders[0].subject, Some(2));
    // 440 of treasury over the reserve and 25·(20−4) = 400 of income
    // carry 840; the full 50 Niter books at 1,361, one gun's 20 at 641.
    assert_eq!(orders[0].verb.as_deref(), Some("RESOURCE_NITER=20"));
    assert_eq!(orders[0].pos, Some((641, 0)));

    // A deal already heading to the largest offer: the next one.
    let mut busy = vec![Order {
        kind: "sell",
        subject: Some(2),
        verb: Some("FAVOR=20".to_string()),
        pos: Some((20, 0)),
    }];
    assert_eq!(buy(&state, &mut busy), None);
    assert_eq!(busy[1].subject, Some(4));
    busy.pop();
    busy.push(Order {
        kind: "buy",
        subject: Some(4),
        verb: Some("OPEN_BORDERS".to_string()),
        pos: Some((60, 0)),
    });
    assert_eq!(
        buy(&state, &mut busy),
        Some("gun_resource_buy_hold:deal_in_flight")
    );

    // Fuel for fielded guns: the block is the offer when it is smaller,
    // still at least the minimum.
    let oil_state = StateSnapshot {
        rivals: vec![StateRival {
            player: 3,
            tradeable_strategics: offers(&[("RESOURCE_OIL", 5)]),
            ..StateRival::default()
        }],
        ..state.clone()
    };
    let oil = vec![want("RESOURCE_OIL", "oil", 10, 3)];
    let mut fuel = Vec::new();
    assert_eq!(
        append_gun_resource_buy_order(&oil, &[], &oil_state, &mut fuel, &worth, &no_barter),
        None
    );
    assert_eq!(fuel[0].verb.as_deref(), Some("RESOURCE_OIL=5"));
    assert_eq!(fuel[0].subject, Some(3));

    // An offer under one gun's worth is no seller.
    let thin = StateSnapshot {
        rivals: vec![StateRival {
            player: 2,
            tradeable_strategics: offers(&[("RESOURCE_NITER", 10)]),
            ..StateRival::default()
        }],
        ..state.clone()
    };
    let mut none = Vec::new();
    assert_eq!(
        buy(&thin, &mut none),
        Some("gun_resource_buy_hold:no_seller")
    );
    assert!(none.is_empty());

    // Off the lane's turn, nothing wanted, a treasury that cannot carry
    // the smallest ask, or one that cannot carry one gun's worth.
    let mut held = Vec::new();
    let off_phase = StateSnapshot {
        turn: 151,
        ..state.clone()
    };
    assert_eq!(
        buy(&off_phase, &mut held),
        Some("gun_resource_buy_hold:cadence")
    );
    assert_eq!(
        append_gun_resource_buy_order(&[], &[], &state, &mut held, &worth, &no_barter),
        Some("gun_resource_buy_hold:no_need")
    );
    let broke = StateSnapshot {
        gold: 60,
        gold_per_turn: Some(4.0),
        ..state.clone()
    };
    assert_eq!(
        buy(&broke, &mut held),
        Some("gun_resource_buy_hold:treasury")
    );
    // 240 over the reserve, no income to speak of: 20 Niter book at 641.
    let short = StateSnapshot {
        gold: 300,
        gold_per_turn: Some(4.0),
        ..state.clone()
    };
    assert_eq!(
        buy(&short, &mut held),
        Some("gun_resource_buy_hold:treasury")
    );
    assert!(held.is_empty());
}

/// `siege-buys-the-gun-resource`: with the treasury short, a spare
/// luxury copy and surplus Coal go on our side first, Gold tops up what
/// they leave, and nothing another sale gives this turn is offered.
#[test]
fn the_guns_resource_is_bartered_when_the_treasury_is_short() {
    let want = civvis::ai::GunResourceWant {
        resource: "RESOURCE_NITER".to_string(),
        resource_id: "niter".to_string(),
        amount: 50,
        minimum: 20,
        unit: "bombard".to_string(),
        guns: 3,
        hit: 24.6,
        renewal: false,
    };
    let spare = |resource: &str, id: &str, amount: u32, luxury: bool| civvis::ai::BarterSpare {
        resource: resource.to_string(),
        resource_id: id.to_string(),
        amount,
        luxury,
    };
    let spares = vec![
        spare("RESOURCE_SILK", "silk", 1, true),
        spare("RESOURCE_COAL", "coal", 15, false),
    ];
    let worth = |_: &str, amount: u32| 185.0 + 24.0 * (f64::from(amount) - 1.0);
    // To the seller a Silk copy is 185, a unit of Coal 20.
    let give_worth = |seller: i64, resource: &str, amount: u32| {
        assert_eq!(seller, 2);
        match resource {
            "silk" => 185.0 * f64::from(amount),
            "coal" => 20.0 * f64::from(amount),
            _ => 0.0,
        }
    };
    let state = StateSnapshot {
        turn: 152,
        gold: 300,
        gold_per_turn: Some(4.0), // 240 carried
        rivals: vec![StateRival {
            player: 2,
            tradeable_strategics: Some([("RESOURCE_NITER".to_string(), 82)].into_iter().collect()),
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    let mut orders = Vec::new();
    assert_eq!(
        append_gun_resource_buy_order(
            std::slice::from_ref(&want),
            &spares,
            &state,
            &mut orders,
            &worth,
            &give_worth
        ),
        None
    );
    // The full 50 (1,361) is beyond Silk 185 + all 15 Coal 300 + 240
    // Gold; one gun's 20 (641) is not: 641 − 185 − 300 = 156 Gold.
    assert_eq!(
        orders[0].verb.as_deref(),
        Some("RESOURCE_NITER=20;RESOURCE_SILK=1,RESOURCE_COAL=15")
    );
    assert_eq!(orders[0].pos, Some((156, 0)));

    // A richer seat takes the full lot: Silk 185 and all 15 Coal 300
    // leave 1,361 − 485 = 876 Gold, under the 900 cap it carries.
    let rich = StateSnapshot {
        gold: 1000,
        gold_per_turn: Some(20.0),
        ..state.clone()
    };
    let mut rich_orders = Vec::new();
    append_gun_resource_buy_order(
        std::slice::from_ref(&want),
        &spares,
        &rich,
        &mut rich_orders,
        &worth,
        &give_worth,
    );
    assert_eq!(
        rich_orders[0].verb.as_deref(),
        Some("RESOURCE_NITER=50;RESOURCE_SILK=1,RESOURCE_COAL=15")
    );
    assert_eq!(rich_orders[0].pos, Some((876, 0)));

    // Coal to spare covers one gun's lot with no Gold at all, and only
    // as much Coal goes as the gap needs: 641 − 185 = 456, 23 units.
    let coal_rich = vec![
        spare("RESOURCE_SILK", "silk", 1, true),
        spare("RESOURCE_COAL", "coal", 40, false),
    ];
    let mut barter_only = Vec::new();
    append_gun_resource_buy_order(
        std::slice::from_ref(&want),
        &coal_rich,
        &state,
        &mut barter_only,
        &worth,
        &give_worth,
    );
    assert_eq!(
        barter_only[0].verb.as_deref(),
        Some("RESOURCE_NITER=20;RESOURCE_SILK=1,RESOURCE_COAL=23")
    );
    assert_eq!(barter_only[0].pos, Some((0, 0)));

    // The Silk already on sale this turn stays off our side, and the
    // Gold-free gap the Coal leaves is then beyond the treasury.
    let mut selling = vec![Order {
        kind: "sell",
        subject: Some(4),
        verb: Some("RESOURCE_SILK=1".to_string()),
        pos: Some((40, 0)),
    }];
    assert_eq!(
        append_gun_resource_buy_order(
            std::slice::from_ref(&want),
            &spares,
            &state,
            &mut selling,
            &worth,
            &give_worth
        ),
        Some("gun_resource_buy_hold:treasury")
    );
    assert_eq!(selling.len(), 1);

    // A luxury the host counts one copy of (the other sold away) stays
    // home, whatever the board counts; one it counts two of still goes.
    let sold_away = StateSnapshot {
        luxury_counts: Some([("RESOURCE_SILK".to_string(), 1.0)].into_iter().collect()),
        ..state.clone()
    };
    let mut netted = Vec::new();
    append_gun_resource_buy_order(
        std::slice::from_ref(&want),
        &coal_rich,
        &sold_away,
        &mut netted,
        &worth,
        &give_worth,
    );
    // 641 from Coal alone: 33 units (660), no Gold.
    assert_eq!(
        netted[0].verb.as_deref(),
        Some("RESOURCE_NITER=20;RESOURCE_COAL=33")
    );
    let two_held = StateSnapshot {
        luxury_counts: Some([("RESOURCE_SILK".to_string(), 2.0)].into_iter().collect()),
        ..state.clone()
    };
    let mut kept = Vec::new();
    append_gun_resource_buy_order(
        std::slice::from_ref(&want),
        &coal_rich,
        &two_held,
        &mut kept,
        &worth,
        &give_worth,
    );
    assert_eq!(
        kept[0].verb.as_deref(),
        Some("RESOURCE_NITER=20;RESOURCE_SILK=1,RESOURCE_COAL=23")
    );

    // The Niter itself is never on our side.
    let own = vec![spare("RESOURCE_NITER", "niter", 30, false)];
    let mut none = Vec::new();
    assert_eq!(
        append_gun_resource_buy_order(&[want], &own, &state, &mut none, &worth, &give_worth),
        Some("gun_resource_buy_hold:treasury")
    );
}

/// `siege-buys-the-gun-resource`: a wanted Niter is not offered for
/// sale, alone or in a bundle; other sales and the purchases stand.
#[test]
fn a_wanted_niter_is_not_offered_for_sale() {
    let order = |kind: &'static str, subject: i64, verb: &str| Order {
        kind,
        subject: Some(subject),
        verb: Some(verb.to_string()),
        pos: Some((40, 0)),
    };
    let mut orders = vec![
        order("sell", 2, "RESOURCE_NITER=10"),
        order("sell", 3, "RESOURCE_IRON=10"),
        order("sell", 4, "RESOURCE_DYES=1,RESOURCE_NITER=5"),
        order("sell", 5, "FAVOR=20"),
        order("buy", 6, "RESOURCE_NITER=20"),
    ];
    assert_eq!(
        drop_held_resource_sales(&std::collections::BTreeSet::new(), &mut orders),
        0
    );
    assert_eq!(orders.len(), 5, "nothing held, nothing dropped");
    let held: std::collections::BTreeSet<String> =
        ["RESOURCE_NITER".to_string()].into_iter().collect();
    assert_eq!(drop_held_resource_sales(&held, &mut orders), 2);
    let left: Vec<_> = orders
        .iter()
        .map(|order| (order.kind, order.verb.as_deref().unwrap_or("")))
        .collect();
    assert_eq!(
        left,
        vec![
            ("sell", "RESOURCE_IRON=10"),
            ("sell", "FAVOR=20"),
            ("buy", "RESOURCE_NITER=20"),
        ]
    );
}

/// `siege-buys-the-gun-resource`: the air wing's Aluminum crosses like
/// the guns' Niter: the whole wing's block from the seller that offers
/// it, Gold-only when nothing of ours is spare.
#[test]
fn the_air_wings_aluminum_is_bought_from_a_seller() {
    let want = civvis::ai::GunResourceWant {
        resource: "RESOURCE_ALUMINUM".to_string(),
        resource_id: "aluminum".to_string(),
        amount: 42,
        minimum: 42,
        unit: "bomber".to_string(),
        guns: 2,
        hit: 0.0,
        renewal: false,
    };
    let worth = |_: &str, amount: u32| 10.0 * f64::from(amount);
    let no_barter = |_: i64, _: &str, _: u32| 0.0;
    let state = StateSnapshot {
        turn: 152,
        gold: 1000,
        gold_per_turn: Some(30.0),
        rivals: vec![
            StateRival {
                player: 2,
                tradeable_strategics: Some(
                    [("RESOURCE_NITER".to_string(), 80)].into_iter().collect(),
                ),
                ..StateRival::default()
            },
            StateRival {
                player: 3,
                tradeable_strategics: Some(
                    [("RESOURCE_ALUMINUM".to_string(), 50)]
                        .into_iter()
                        .collect(),
                ),
                ..StateRival::default()
            },
        ],
        ..StateSnapshot::default()
    };
    let mut orders = Vec::new();
    assert_eq!(
        append_gun_resource_buy_order(
            std::slice::from_ref(&want),
            &[],
            &state,
            &mut orders,
            &worth,
            &no_barter
        ),
        None
    );
    assert_eq!(orders[0].subject, Some(3));
    assert_eq!(orders[0].verb.as_deref(), Some("RESOURCE_ALUMINUM=42"));
    assert_eq!(orders[0].pos, Some((420, 0)));
    // 40 Gold over the reserve cannot carry the 420 the wing books at, and
    // nothing smaller fields it.
    let mut held = Vec::new();
    let short = StateSnapshot {
        gold: 100,
        gold_per_turn: Some(4.0),
        ..state.clone()
    };
    assert_eq!(
        append_gun_resource_buy_order(&[want], &[], &short, &mut held, &worth, &no_barter),
        Some("gun_resource_buy_hold:treasury")
    );
}

#[test]
fn city_state_war_and_peace_orders_name_the_firaxis_seat() {
    // Live run civvis-20260816T070212Z reached this shape at t92: CIVVIS
    // chose a staged war on Nazca (host player 13), but translating only
    // through `state.rivals` emitted subject -1 and the control mod refused
    // it as `war_target_unmapped` three times.
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 92,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 92,
        minors: vec![StateMinor {
            player: 13,
            civ: "CIVILIZATION_NAZCA".to_string(),
            cities: vec![StateCity {
                id: 65_536,
                name: "Nazca".to_string(),
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
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
    let nazca = mirror
        .game
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian && !player.is_free_city)
        .expect("Nazca must occupy a mirrored city-state seat")
        .id;

    let war = translate(&Action::DeclareWar { player: nazca }, &mirror, &state)
        .expect("a city-state war crosses the order boundary");
    assert_eq!(war.kind, "war");
    assert_eq!(war.subject, Some(13));

    let peace = translate(&Action::MakePeace { player: nazca }, &mirror, &state)
        .expect("a city-state peace crosses the same boundary");
    assert_eq!(peace.kind, "peace");
    assert_eq!(peace.subject, Some(13));
}

/// ★★★★★ The whole envoy chain inside the bridge: the held count reaches
/// the board, the spend-all controller spends it on the planning clone, and
/// each `SendEnvoy` crosses as an `envoy` order naming Firaxis's minor
/// player id — including a city-state met before its centre is in view,
/// which has no mirrored city to resolve through.
#[test]
fn held_envoys_are_spent_by_the_plan_and_cross_as_envoy_orders() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 60,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 60,
        envoys_free: Some(3),
        cities: vec![StateCity {
            id: 65_536,
            name: "Rome".to_string(),
            x: 2,
            y: 2,
            pop: 5,
            capital: true,
            ..StateCity::default()
        }],
        minors: vec![
            StateMinor {
                player: 9,
                civ: "CIVILIZATION_GENEVA".to_string(),
                envoys: 1,
                suzerain: -1,
                cities: vec![StateCity {
                    id: 65_536,
                    name: "Geneva".to_string(),
                    x: 8,
                    y: 8,
                    pop: 3,
                    capital: true,
                    ..StateCity::default()
                }],
                ..StateMinor::default()
            },
            // Met by contact, centre still in the fog: no city exported.
            StateMinor {
                player: 12,
                civ: "CIVILIZATION_KABUL".to_string(),
                envoys: 0,
                suzerain: -1,
                ..StateMinor::default()
            },
        ],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 2, 250, 0);
    assert_eq!(
        mirror.game.players[0].envoys_free, 3,
        "the held count is mirrored"
    );
    let seats: Vec<usize> = mirror
        .game
        .players
        .iter()
        .filter(|player| player.is_minor && !player.is_barbarian && !player.is_free_city)
        .map(|player| player.id)
        .collect();
    let (geneva, kabul) = (seats[0], seats[1]);
    assert_eq!(mirror.game.players[geneva].civ, "Geneva");
    assert_eq!(mirror.game.players[kabul].civ, "Kabul");

    // Both cross by the seating rule, the unseen one included.
    let order = translate(&Action::SendEnvoy { player: geneva }, &mirror, &state)
        .expect("an envoy to a met city-state crosses");
    assert_eq!(order.kind, "envoy");
    assert_eq!(order.subject, Some(9));
    assert_eq!(order.verb.as_deref(), Some("GIVE_INFLUENCE_TOKEN"));
    let unseen = translate(&Action::SendEnvoy { player: kabul }, &mirror, &state)
        .expect("a city-state met before its centre is in view still gets its envoy");
    assert_eq!(unseen.subject, Some(12));
    // A major seat is not a city-state.
    assert!(translate(&Action::SendEnvoy { player: 1 }, &mirror, &state).is_none());

    // Pin the spend-all policy: this checks order translation, not the
    // tournament-selected defaults for saving envoys for future dividends.
    let mut ai = civvis::ai::AdvancedAi::new();
    ai.enable_live_bridge();
    ai.disable_envoy_building_dividends();
    ai.disable_bank_envoys();
    let mut planned = mirror.game.clone();
    let begin = planned.log.len();
    ai.take_turn(&mut planned, 0);
    let sent: Vec<i64> = planned
        .log
        .since(begin)
        .filter(|(seat, _)| *seat == 0)
        .filter_map(|(_, action)| match action {
            Action::SendEnvoy { .. } => translate(action, &mirror, &state),
            _ => None,
        })
        .filter_map(|order| order.subject)
        .collect();
    assert_eq!(
        sent.len(),
        3,
        "every held envoy is spent on the planning board: {sent:?}"
    );
    assert!(
        sent.iter().all(|host| *host == 9 || *host == 12),
        "{sent:?}"
    );
    assert_eq!(planned.players[0].envoys_free, 0);
    // The real caller supplies its outbound budget before planning. With
    // twenty held envoys, both the planned actions and translated orders
    // must stop at eight; clipping only the latter invents influence.
    let mut capped_state = state.clone();
    capped_state.envoys_free = Some(20);
    let mut capped = civvis::mirror::LiveMirror::new(&snapshot, &capped_state, 2, 2, 250, 0);
    let mut bounded_ai = civvis::ai::AdvancedAi::new();
    bounded_ai.enable_live_bridge();
    bounded_ai.enable_envoys_open_the_siege_road();
    // `decide` re-applies the deployed genome every turn, so a disable
    // here does not survive it; the arm's withholds do. Withhold whichever
    // of the two savers the current selection lets an arm withhold.
    let withheld: Vec<String> = ["envoy-building-dividends", "bank-envoys"]
        .into_iter()
        .filter(|tag| {
            civvis::ai::GENES
                .iter()
                .any(|gene| gene.tag == *tag && withholdable(gene))
        })
        .map(str::to_string)
        .collect();
    let mut ours = std::collections::BTreeMap::new();
    let reply: serde_json::Value = serde_json::from_str(&decide(
        &mut capped,
        &mut bounded_ai,
        &snapshot,
        &capped_state,
        DecisionArm {
            war_from_plan: false,
            forced_on: &[],
            withheld: &withheld,
        },
        DecisionMemory {
            ours: &mut ours,
            host_peace_retries: &mut HostPeaceRetries::default(),
            host_move_refusals: &mut HostMoveRefusals::default(),
            host_order_refusals: &mut HostOrderRefusals::default(),
            air_assault_continuation: &mut air_assault_continuation::Continuation::default(),
            governor_exchange: &mut governor_exchange::Continuation::default(),
        },
    ))
    .expect("bounded decision is JSON");
    assert_eq!(
        reply["orders"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|order| order["kind"] == "envoy")
            .count(),
        ENVOY_ORDERS_PER_TURN
    );
    assert_eq!(
        reply["decision"]["native_actions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|action| action["type"] == "send_envoy")
            .count(),
        ENVOY_ORDERS_PER_TURN
    );
    assert_eq!(
        capped.game.players[0].envoys_free, 20,
        "the authoritative mirror is unchanged by planning"
    );
}

/// The live planner may intentionally retain envoys after it has secured
/// every met city-state past its final yield tier.  That decision has to
/// survive the whole order boundary: `BasicAi` runs later in the same
/// simulated turn, so its historical "highest count" fallback must not
/// turn the bank straight back into `GIVE_INFLUENCE_TOKEN` orders.
#[test]
fn banked_secure_envoys_do_not_cross_the_live_order_boundary() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 60,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 60,
        envoys_free: Some(2),
        cities: vec![StateCity {
            id: 65_536,
            name: "Rome".to_string(),
            x: 2,
            y: 2,
            pop: 5,
            capital: true,
            ..StateCity::default()
        }],
        minors: vec![StateMinor {
            player: 9,
            civ: "CIVILIZATION_AUCKLAND".to_string(),
            envoys: 8,
            most_envoys: 8,
            suzerain: 0,
            cities: vec![StateCity {
                id: 65_537,
                name: "Auckland".to_string(),
                x: 8,
                y: 8,
                pop: 3,
                capital: true,
                ..StateCity::default()
            }],
            ..StateMinor::default()
        }],
        ..StateSnapshot::default()
    };
    let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
    let auckland = mirror
        .game
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian && !player.is_free_city)
        .map(|player| player.id)
        .expect("Auckland must occupy a mirrored city-state seat");
    assert_eq!(mirror.game.envoys_at(0, auckland), 8);
    assert_eq!(mirror.game.suzerain_of(auckland), Some(0));
    assert_eq!(mirror.game.players[0].envoys_free, 2);

    let mut ai = civvis::ai::AdvancedAi::new();
    ai.enable_bank_envoys();
    let mut ours = std::collections::BTreeMap::new();
    let reply: serde_json::Value = serde_json::from_str(&decide(
        &mut mirror,
        &mut ai,
        &snapshot,
        &state,
        default_decision_arm(),
        DecisionMemory {
            ours: &mut ours,
            host_peace_retries: &mut HostPeaceRetries::default(),
            host_move_refusals: &mut HostMoveRefusals::default(),
            host_order_refusals: &mut HostOrderRefusals::default(),
            air_assault_continuation: &mut air_assault_continuation::Continuation::default(),
            governor_exchange: &mut governor_exchange::Continuation::default(),
        },
    ))
    .expect("the decision is JSON");

    assert!(
        reply["orders"]
            .as_array()
            .expect("orders are an array")
            .iter()
            .all(|order| order["kind"] != "envoy"),
        "a safely owned eight-envoy city-state must retain the two-envoy bank: {reply}"
    );
}

/// A suzerain at war levies the city-state's army through the same seat
/// resolution as the envoy; a major seat is not a city-state.
#[test]
fn a_levy_names_the_firaxis_city_state() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 120,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 120,
        minors: vec![StateMinor {
            player: 11,
            civ: "CIVILIZATION_KABUL".to_string(),
            envoys: 4,
            suzerain: 0,
            cities: vec![StateCity {
                id: 65_536,
                name: "Kabul".to_string(),
                x: 8,
                y: 8,
                pop: 5,
                capital: true,
                ..StateCity::default()
            }],
            ..StateMinor::default()
        }],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
    let kabul = mirror
        .game
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian && !player.is_free_city)
        .expect("Kabul occupies the mirrored city-state seat")
        .id;
    assert_eq!(mirror.game.suzerain_of(kabul), Some(0));
    let levy = translate(&Action::LevyMilitary { player: kabul }, &mirror, &state)
        .expect("a levy of a suzerained city-state crosses");
    assert_eq!(levy.kind, "levy");
    assert_eq!(levy.subject, Some(11));
    assert_eq!(levy.verb.as_deref(), Some("LEVY_MILITARY"));
    assert!(translate(&Action::LevyMilitary { player: 1 }, &mirror, &state).is_none());
}

/// The first turn the lane opens on a seat holding fifty envoys must not
/// issue fifty operations: eight cross in plan order, the rest wait for
/// next turn's plan, and nothing else is touched.
#[test]
fn envoy_orders_are_bounded_per_turn_in_plan_order() {
    let envoy = |minor: i64| Order {
        kind: "envoy",
        subject: Some(minor),
        verb: Some("GIVE_INFLUENCE_TOKEN".to_string()),
        pos: None,
    };
    let mut orders: Vec<Order> = (0..12).map(|i| envoy(20 + i)).collect();
    orders.insert(
        0,
        Order {
            kind: "research",
            subject: None,
            verb: Some("TECH_WRITING".to_string()),
            pos: None,
        },
    );
    orders.push(Order {
        kind: "produce",
        subject: Some(65_536),
        verb: Some("BUILDING_LIBRARY".to_string()),
        pos: None,
    });
    let (kept, deferred) = bound_envoy_orders(orders);
    assert_eq!(deferred, 12 - ENVOY_ORDERS_PER_TURN);
    let envoys: Vec<i64> = kept
        .iter()
        .filter(|o| o.kind == "envoy")
        .filter_map(|o| o.subject)
        .collect();
    assert_eq!(
        envoys,
        (20..20 + ENVOY_ORDERS_PER_TURN as i64).collect::<Vec<_>>(),
        "plan order, first eight"
    );
    assert_eq!(
        kept.iter().filter(|o| o.kind != "envoy").count(),
        2,
        "other kinds pass untouched"
    );
    assert_eq!(kept.first().map(|o| o.kind), Some("research"));
    assert_eq!(kept.last().map(|o| o.kind), Some("produce"));
}

#[test]
fn exact_spent_firaxis_titles_suppress_duplicate_governor_orders() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 92,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 92,
        governor_points: Some(4),
        governor_points_spent: Some(4),
        governors: Some(vec![
            StateGovernor {
                kind: "GOVERNOR_THE_DEFENDER".to_string(),
                city: 65_536,
                x: 6,
                y: 6,
                established: true,
                promotions: vec![
                    "GOVERNOR_PROMOTION_GARRISON_COMMANDER".to_string(),
                    "GOVERNOR_PROMOTION_DEFENSE_LOGISTICS".to_string(),
                ],
                ..StateGovernor::default()
            },
            StateGovernor {
                kind: "GOVERNOR_THE_RESOURCE_MANAGER".to_string(),
                city: -1,
                ..StateGovernor::default()
            },
        ]),
        cities: vec![StateCity {
            id: 65_536,
            name: "Capital".to_string(),
            x: 6,
            y: 6,
            pop: 5,
            capital: true,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let mut ai = civvis::ai::AdvancedAi::new();

    let reply: serde_json::Value = serde_json::from_str(&decide(
        &mut mirror,
        &mut ai,
        &snapshot,
        &state,
        default_decision_arm(),
        DecisionMemory {
            ours: &mut Default::default(),
            host_peace_retries: &mut HostPeaceRetries::default(),
            host_move_refusals: &mut HostMoveRefusals::default(),
            host_order_refusals: &mut HostOrderRefusals::default(),
            air_assault_continuation: &mut air_assault_continuation::Continuation::default(),
            governor_exchange: &mut governor_exchange::Continuation::default(),
        },
    ))
    .expect("the decision is JSON");

    assert!(reply["orders"].as_array().unwrap().iter().all(|order| {
        !order["kind"]
            .as_str()
            .is_some_and(|kind| kind.starts_with("governor_"))
    }));
}

#[test]
fn policy_replacements_are_one_complete_host_deck() {
    let mut game = civvis::game::Game::new(2, 12, 12, 1, 50, 0);
    game.players[0]
        .policies
        .insert(civvis::name!("urban_planning"));
    game.players[0].policies.insert(civvis::name!("agoge"));

    let order = policy_deck_order(&game, 0);
    assert_eq!(order.kind, "policy_deck");
    assert_eq!(
        order.verb.as_deref(),
        Some("POLICY_AGOGE,POLICY_URBAN_PLANNING")
    );
}

#[test]
fn policy_deck_waits_for_cards_unlocked_by_the_authoritative_civic() {
    let mut authoritative = civvis::game::Game::new(2, 12, 12, 1, 50, 0);
    authoritative.players[0]
        .policies
        .insert(civvis::name!("urban_planning"));
    let mut planned = authoritative.clone();
    planned.players[0]
        .policies
        .insert(civvis::name!("new_deal"));

    assert_eq!(
        policy_deck_cards_not_currently_legal(&planned, &authoritative, 0),
        vec!["POLICY_NEW_DEAL".to_string()]
    );

    // Once the host export reports Suffrage, the same speculative deck is
    // legal and should cross on the next decision instead of being lost.
    authoritative.players[0]
        .civics
        .insert(civvis::name!("suffrage"));
    assert!(policy_deck_cards_not_currently_legal(&planned, &authoritative, 0).is_empty());

    // Existing cards remain legal even when the mirror has not learned the
    // civic that originally unlocked them.
    planned.players[0]
        .policies
        .remove(&civvis::name!("new_deal"));
    assert!(policy_deck_cards_not_currently_legal(&planned, &authoritative, 0).is_empty());
}

#[test]
fn renamed_units_and_improvements_use_firaxis_type_ids() {
    let winged_hussar = civvis::game::Item::Unit {
        unit: civvis::name!("winged_hussar"),
    };
    let keshig = civvis::game::Item::Unit {
        unit: civvis::name!("keshig"),
    };
    let oromo = civvis::game::Item::Unit {
        unit: civvis::name!("oromo_cavalry"),
    };
    let nau = civvis::game::Item::Unit {
        unit: civvis::name!("nau"),
    };
    let toa = civvis::game::Item::Unit {
        unit: civvis::name!("toa"),
    };
    assert_eq!(
        civ6_build_name(&winged_hussar).as_deref(),
        Some("UNIT_POLISH_HUSSAR")
    );
    assert_eq!(
        civ6_build_name(&keshig).as_deref(),
        Some("UNIT_MONGOLIAN_KESHIG")
    );
    assert_eq!(
        civ6_build_name(&oromo).as_deref(),
        Some("UNIT_ETHIOPIAN_OROMO_CAVALRY")
    );
    assert_eq!(
        civ6_build_name(&nau).as_deref(),
        Some("UNIT_PORTUGUESE_NAU")
    );
    assert_eq!(civ6_build_name(&toa).as_deref(), Some("UNIT_MAORI_TOA"));
    assert_eq!(
        civ6_improvement_type(&civvis::name!("seaside_resort")),
        "IMPROVEMENT_BEACH_RESORT"
    );
    assert_eq!(
        civ6_improvement_type(&civvis::name!("qhapaq_nan")),
        "IMPROVEMENT_MOUNTAIN_ROAD"
    );

    let district = civvis::game::Item::District {
        district: civvis::name!("campus"),
        pos: (7, 4),
    };
    let wonder = civvis::game::Item::Wonder {
        wonder: civvis::name!("pyramids"),
        pos: (3, 8),
    };
    assert_eq!(civ6_build_pos(&district), Some((9, 4)));
    assert_eq!(civ6_build_pos(&wonder), Some((7, 8)));
}

#[test]
fn wall_production_and_repairs_use_firaxis_internal_type_ids() {
    for (civvis, firaxis) in [
        ("ancient_walls", "BUILDING_WALLS"),
        ("medieval_walls", "BUILDING_CASTLE"),
        ("renaissance_walls", "BUILDING_STAR_FORT"),
    ] {
        let building = civvis::game::Item::Building {
            building: civvis::name::Name::new(civvis),
        };
        assert_eq!(civ6_build_name(&building).as_deref(), Some(firaxis));

        let repair = civvis::game::Item::Repair {
            repair: civvis::name::Name::new(civvis),
            pos: (4, 4),
        };
        let game = civvis::game::Game::new(2, 12, 12, 1, 50, 0);
        assert_eq!(
            civ6_live_build_name(&repair, &game).as_deref(),
            Some(firaxis)
        );
    }
}

#[test]
fn exported_lobby_size_and_horizon_override_cli_fallbacks() {
    let state = StateSnapshot {
        seat: civvis::mirror::Seat {
            players: 6,
            max_turns: 250,
            ..civvis::mirror::Seat::default()
        },
        ..StateSnapshot::default()
    };
    assert_eq!(mirror_setup(&state, 4, 500), (6, 250));
    assert_eq!(mirror_setup(&StateSnapshot::default(), 4, 500), (4, 500));
    // No readback: the seat's speed, not the CLI's fixed 250.
    let mut standard = StateSnapshot::default();
    standard.seat.speed = "GAMESPEED_STANDARD".to_string();
    standard.seat.max_turns = -1;
    assert_eq!(mirror_setup(&standard, 4, 250), (4, 500));
    standard.seat.max_turns = 650;
    assert_eq!(mirror_setup(&standard, 4, 250), (4, 650));
}

fn grass(x: i32, y: i32) -> Plot {
    Plot {
        io: None,
        x,
        y,
        t: Some("TERRAIN_GRASS".to_string()),
        f: None,
        r: None,
        o: -1,
        oc: None,
        w: false,
        lk: None,
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
        dh: None,
        wo: None,
        rt: None,
        rp: false,
        yl: None,
        ap: None,
        np: false,
        vis: false,
        flooded: None,
        submerged: None,
    }
}

#[test]
fn a_host_accepted_edge_replays_from_its_reported_source() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 7,
        width: 8,
        height: 8,
        chunk: 1,
        plots: (0..8)
            .flat_map(|x| (0..8).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let mut state = StateSnapshot {
        turn: 7,
        cities: vec![StateCity {
            id: 1,
            name: "Rome".to_string(),
            x: 1,
            y: 1,
            pop: 3,
            capital: true,
            ..StateCity::default()
        }],
        units: vec![StateUnit {
            id: 42,
            kind: "UNIT_WARRIOR".to_string(),
            // The last state can be stale; the observed source below is
            // deliberately a different, adjacent host coordinate.
            x: 2,
            y: 3,
            hp: 100.0,
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };
    let observed = ObservedHostMove {
        turn: 7,
        unit: 42,
        from: (3, 3),
        to: (4, 3),
    };

    assert_eq!(
        audit_host_move(&snapshot, &state, 2, 250, 0, &observed),
        HostMoveAuditVerdict::ModelAllowed,
        "the host source, not the old state position, is replayed"
    );

    state.units.push(StateUnit {
        id: 43,
        kind: "UNIT_WARRIOR".to_string(),
        x: 4,
        y: 3,
        hp: 100.0,
        ..StateUnit::default()
    });
    assert_eq!(
        audit_host_move(&snapshot, &state, 2, 250, 0, &observed),
        HostMoveAuditVerdict::ModelRefused("own_unit".to_string()),
        "a host-accepted edge exposes a model stacking disagreement"
    );
}

#[test]
fn successive_host_moves_do_not_leave_vacated_units_as_audit_blockers() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 8,
        width: 8,
        height: 8,
        chunk: 1,
        plots: (0..8)
            .flat_map(|x| (0..8).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let mut state = StateSnapshot {
        turn: 8,
        cities: vec![StateCity {
            id: 1,
            name: "Rome".to_string(),
            x: 0,
            y: 0,
            pop: 3,
            capital: true,
            ..StateCity::default()
        }],
        units: vec![
            StateUnit {
                id: 42,
                kind: "UNIT_WARRIOR".to_string(),
                x: 2,
                y: 3,
                hp: 100.0,
                ..StateUnit::default()
            },
            StateUnit {
                id: 43,
                kind: "UNIT_WARRIOR".to_string(),
                x: 1,
                y: 3,
                hp: 100.0,
                ..StateUnit::default()
            },
        ],
        ..StateSnapshot::default()
    };
    let first = ObservedHostMove {
        turn: 8,
        unit: 42,
        from: (2, 3),
        to: (3, 3),
    };
    assert_eq!(
        audit_host_move(&snapshot, &state, 2, 250, 0, &first),
        HostMoveAuditVerdict::ModelAllowed
    );
    assert!(advance_observed_host_position(&mut state, &first));

    let second = ObservedHostMove {
        turn: 8,
        unit: 43,
        from: (1, 3),
        to: (2, 3),
    };
    assert_eq!(
        audit_host_move(&snapshot, &state, 2, 250, 0, &second),
        HostMoveAuditVerdict::ModelAllowed,
        "the first mover has already vacated this destination in the shadow"
    );
    assert!(advance_observed_host_position(&mut state, &second));
    assert_eq!(state.units[0].x, 3);
    assert_eq!(state.units[1].x, 2);
}

#[test]
fn combat_shadow_removes_only_the_named_foreign_unit() {
    let mut state = StateSnapshot {
        units: vec![StateUnit {
            id: 7,
            kind: "UNIT_WARRIOR".to_string(),
            ..StateUnit::default()
        }],
        rivals: vec![
            StateRival {
                player: 2,
                units: vec![StateUnit {
                    id: 7,
                    kind: "UNIT_WARRIOR".to_string(),
                    ..StateUnit::default()
                }],
                ..StateRival::default()
            },
            StateRival {
                player: 3,
                units: vec![StateUnit {
                    id: 7,
                    kind: "UNIT_WARRIOR".to_string(),
                    ..StateUnit::default()
                }],
                ..StateRival::default()
            },
        ],
        ..StateSnapshot::default()
    };
    advance_observed_combat_deaths(
        &mut state,
        &serde_json::json!({
            "kind": "combat", "defender_killed": true, "against_us": false,
            "defender": {"type": "unit", "player": 2, "id": 7},
        }),
    );
    assert_eq!(state.units.len(), 1, "our same-numbered unit remains");
    assert!(
        state.rivals[0].units.is_empty(),
        "the dead defender is gone"
    );
    assert_eq!(
        state.rivals[1].units.len(),
        1,
        "a different player's same-numbered unit remains"
    );

    assert!(remove_observed_local_unit(&mut state, 7));
    assert!(
        state.units.is_empty(),
        "local removal uses the local roster only"
    );
}

#[test]
fn host_move_events_require_complete_nonnegative_coordinates() {
    let event = serde_json::json!({
        "kind": "host_move", "turn": 7, "unit": 42,
        "from_x": 3, "from_y": 3, "x": 4, "y": 3,
    });
    assert_eq!(
        observed_host_move(&event),
        Some(ObservedHostMove {
            turn: 7,
            unit: 42,
            from: (3, 3),
            to: (4, 3),
        })
    );
    assert!(
        observed_host_move(&serde_json::json!({
            "kind": "host_move", "turn": 7, "unit": 42,
            "from_x": -1, "from_y": 3, "x": 4, "y": 3,
        }))
        .is_none(),
        "an absent source must stay uncomparable"
    );
    assert!(
        observed_host_move(&serde_json::json!({"kind": "move_refused"})).is_none(),
        "outbound refusal telemetry is not reverse-legality evidence"
    );
}

#[test]
fn deciding_does_not_mutate_the_authoritative_live_mirror() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 4,
        width: 12,
        height: 12,
        chunk: 1,
        plots: (0..12)
            .flat_map(|x| (0..12).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let mut state = StateSnapshot {
        turn: 4,
        ..StateSnapshot::default()
    };
    state.cities.push(StateCity {
        id: 7,
        name: "Roma".to_string(),
        x: 6,
        y: 6,
        pop: 3,
        ..StateCity::default()
    });
    state.units.push(StateUnit {
        id: 42,
        kind: "UNIT_WARRIOR".to_string(),
        x: 6,
        y: 7,
        ..StateUnit::default()
    });

    let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let before = serde_json::to_value(&mirror.game).expect("mirror game serializes");
    let mut ai = civvis::ai::AdvancedAi::new();

    let reply = decide(
        &mut mirror,
        &mut ai,
        &snapshot,
        &state,
        default_decision_arm(),
        DecisionMemory {
            ours: &mut Default::default(),
            host_peace_retries: &mut HostPeaceRetries::default(),
            host_move_refusals: &mut HostMoveRefusals::default(),
            host_order_refusals: &mut HostOrderRefusals::default(),
            air_assault_continuation: &mut air_assault_continuation::Continuation::default(),
            governor_exchange: &mut governor_exchange::Continuation::default(),
        },
    );

    assert!(reply.contains("\"turn\":4"));
    assert_eq!(
        serde_json::to_value(&mirror.game).expect("mirror game serializes"),
        before,
        "planning must not leave an imagined end turn, queue, or produced unit on the live mirror"
    );
    assert_eq!(mirror.civ6_of.len(), 1);
    assert_eq!(mirror.uid_of.len(), 1);
}

#[test]
fn converted_holy_site_never_emits_a_rival_missionary_purchase() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 151,
        width: 16,
        height: 16,
        chunk: 1,
        plots: (0..16)
            .flat_map(|x| (0..16).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 151,
        techs: vec!["TECH_ASTROLOGY".to_string()],
        founded_religion: Some("RELIGION_CATHOLICISM".to_string()),
        founded_religions: vec![
            "RELIGION_CATHOLICISM".to_string(),
            "RELIGION_ISLAM".to_string(),
        ],
        faith: 1_000,
        cities: vec![StateCity {
            id: 65_536,
            name: "Rome".to_string(),
            x: 8,
            y: 8,
            pop: 7,
            capital: true,
            religion: Some("RELIGION_ISLAM".to_string()),
            buildings: vec!["BUILDING_SHRINE".to_string()],
            districts: vec![StateDistrict {
                kind: "DISTRICT_HOLY_SITE".to_string(),
                x: 9,
                y: 8,
                pillaged: false,
                complete: true,
                ..StateDistrict::default()
            }],
            producing: Some("PROJECT_HOLY_SITE_PRAYERS".to_string()),
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let city = mirror.game.player_city_ids(0)[0];
    assert_eq!(
        mirror.game.players[0].religion.as_deref(),
        Some("Catholicism")
    );
    assert_eq!(
        mirror.game.city_religion(&mirror.game.cities[&city]),
        Some("Islam")
    );

    let mut ai = civvis::ai::AdvancedAi::new();
    let reply: serde_json::Value = serde_json::from_str(&decide(
        &mut mirror,
        &mut ai,
        &snapshot,
        &state,
        default_decision_arm(),
        DecisionMemory {
            ours: &mut Default::default(),
            host_peace_retries: &mut HostPeaceRetries::default(),
            host_move_refusals: &mut HostMoveRefusals::default(),
            host_order_refusals: &mut HostOrderRefusals::default(),
            air_assault_continuation: &mut air_assault_continuation::Continuation::default(),
            governor_exchange: &mut governor_exchange::Continuation::default(),
        },
    ))
    .expect("the decision is JSON");

    assert!(
        reply["orders"]
            .as_array()
            .unwrap()
            .iter()
            .all(|order| order["verb"].as_str() != Some("UNIT_MISSIONARY")),
        "a live recommendation must not buy the converted city's rival-faith Missionary"
    );
}

#[test]
fn fresh_mirror_translates_a_zero_movement_trader_to_a_trade_route() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 20,
        width: 16,
        height: 16,
        chunk: 1,
        plots: (0..16)
            .flat_map(|x| (0..16).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 20,
        civics: vec!["CIVIC_FOREIGN_TRADE".to_string()],
        cities: vec![
            StateCity {
                id: 7,
                name: "Roma".to_string(),
                x: 6,
                y: 6,
                pop: 3,
                capital: true,
                ..StateCity::default()
            },
            StateCity {
                id: 8,
                name: "Antium".to_string(),
                x: 7,
                y: 7,
                pop: 3,
                ..StateCity::default()
            },
        ],
        units: vec![StateUnit {
            id: 42,
            kind: "UNIT_TRADER".to_string(),
            x: 7,
            y: 7,
            moves: 0.0,
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };
    let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let trader = mirror.uid_of[&42];

    assert_eq!(mirror.game.trade_capacity(0), 1);
    assert!(mirror
        .game
        .legal_actions(0)
        .iter()
        .any(|action| { matches!(action, Action::TradeRoute { unit, .. } if *unit == trader) }));

    let mut ai = civvis::ai::AdvancedAi::new();
    let reply = decide(
        &mut mirror,
        &mut ai,
        &snapshot,
        &state,
        default_decision_arm(),
        DecisionMemory {
            ours: &mut Default::default(),
            host_peace_retries: &mut HostPeaceRetries::default(),
            host_move_refusals: &mut HostMoveRefusals::default(),
            host_order_refusals: &mut HostOrderRefusals::default(),
            air_assault_continuation: &mut air_assault_continuation::Continuation::default(),
            governor_exchange: &mut governor_exchange::Continuation::default(),
        },
    );

    assert!(
        reply.contains("\"verb\":\"TRADE_ROUTE\"") && reply.contains("\"subject\":42"),
        "a live trader cannot walk, but its legal route must still reach Civ VI: {reply}"
    );
}

#[test]
fn active_firaxis_trade_route_keeps_the_trader_visible_but_out_of_the_plan() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 20,
        width: 16,
        height: 16,
        chunk: 1,
        plots: (0..16)
            .flat_map(|x| (0..16).map(move |y| grass(x, y)))
            .collect(),
    }]);
    let state = StateSnapshot {
        turn: 20,
        civics: vec!["CIVIC_FOREIGN_TRADE".to_string()],
        cities: vec![
            StateCity {
                id: 7,
                name: "Roma".to_string(),
                x: 6,
                y: 6,
                pop: 3,
                capital: true,
                ..StateCity::default()
            },
            StateCity {
                id: 8,
                name: "Antium".to_string(),
                x: 7,
                y: 7,
                pop: 3,
                ..StateCity::default()
            },
        ],
        units: vec![StateUnit {
            id: 42,
            kind: "UNIT_TRADER".to_string(),
            x: 7,
            y: 7,
            moves: 0.0,
            ..StateUnit::default()
        }],
        trade_routes: vec![StateTradeRoute {
            trader: 42,
            origin: 8,
            destination: 7,
            origin_x: 7,
            origin_y: 7,
            destination_x: 6,
            destination_y: 6,
            ..StateTradeRoute::default()
        }],
        ..StateSnapshot::default()
    };
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let trader = mirror.uid_of[&42];

    assert!(
        mirror.game.units.contains_key(&trader),
        "the mirrored map must retain Firaxis's moving trader unit"
    );
    assert_eq!(
        mirror.game.active_routes(0),
        1,
        "the active route must occupy CIVVIS trade capacity and pay its yields"
    );
    assert!(mirror.active_trade_route_traders.contains(&42));

    let mut planning = mirror.game.clone();
    remove_active_route_traders_from_plan(&mut planning, &mirror);
    assert!(
        !planning.units.contains_key(&trader),
        "only the planning clone may consume a trader that Firaxis reports as busy"
    );
    assert_eq!(
        planning.active_routes(0),
        1,
        "removing the visual stand-in must not erase the real route's economic state"
    );
}

/// Every name here was refused by the live mod, and the count is from the ledger.
///
/// The right-hand side is the row in `Cache/DebugGameplay.sqlite`, not a guess from
/// the display name — that distinction is the whole bug. `BUILDING_ART_MUSEUM` reads
/// perfectly and does not exist.
#[test]
fn a_build_order_names_the_row_civilization_vi_actually_ships() {
    use civvis::game::Item;
    use civvis::name::Name;

    let building = |n: &str| civ6_building_type(&Name::new(n));
    let district = |n: &str| {
        civ6_build_name(&Item::District {
            district: Name::new(n),
            pos: (0, 0),
        })
        .expect("a district always names something")
    };
    let project = |n: &str| {
        civ6_build_name(&Item::Project {
            project: Name::new(n),
        })
        .expect("a project always names something")
    };

    // The culture chain. 507 Museum orders across 45 runs, every one refused, and
    // not one city in any of them ever finished a Museum or held a Great Work.
    assert_eq!(building("art_museum"), "BUILDING_MUSEUM_ART");
    assert_eq!(
        building("archaeological_museum"),
        "BUILDING_MUSEUM_ARTIFACT"
    );
    assert_eq!(district("theater_square"), "DISTRICT_THEATER");
    // 310 refusals, and the inbound reader already had to grow a unique-prefix
    // rule to recover this one coming the other way.
    assert_eq!(district("government_plaza"), "DISTRICT_GOVERNMENT");
    assert_eq!(building("national_history_museum"), "BUILDING_GOV_CULTURE");
    // 110 refusals: an entire technology the seat could never select.
    assert_eq!(civ6_tech_name("wheel"), "TECH_THE_WHEEL");

    // The rest of the Government Plaza tier, named for the slot not the subject.
    assert_eq!(building("audience_chamber"), "BUILDING_GOV_TALL");
    assert_eq!(building("ancestral_hall"), "BUILDING_GOV_WIDE");
    assert_eq!(building("warlords_throne"), "BUILDING_GOV_CONQUEST");
    assert_eq!(building("foreign_ministry"), "BUILDING_GOV_CITYSTATES");
    assert_eq!(building("grand_masters_chapel"), "BUILDING_GOV_FAITH");
    assert_eq!(building("intelligence_agency"), "BUILDING_GOV_SPIES");
    assert_eq!(building("royal_society"), "BUILDING_GOV_SCIENCE");
    assert_eq!(building("war_department"), "BUILDING_GOV_MILITARY");

    assert_eq!(
        building("oil_power_plant"),
        "BUILDING_FOSSIL_FUEL_POWER_PLANT"
    );
    assert_eq!(building("nuclear_power_plant"), "BUILDING_POWER_PLANT");
    assert_eq!(
        building("mausoleum_at_halicarnassus"),
        "BUILDING_HALICARNASSUS_MAUSOLEUM"
    );
    assert_eq!(building("statue_of_liberty"), "BUILDING_STATUE_LIBERTY");
    assert_eq!(
        building("university_of_sankore"),
        "BUILDING_UNIVERSITY_SANKORE"
    );

    assert_eq!(
        district("water_park"),
        "DISTRICT_WATER_ENTERTAINMENT_COMPLEX"
    );
    assert_eq!(district("copacabana"), "DISTRICT_WATER_STREET_CARNIVAL");

    assert_eq!(
        project("exoplanet_expedition"),
        "PROJECT_LAUNCH_EXOPLANET_EXPEDITION"
    );
    assert_eq!(project("launch_mars_colony"), "PROJECT_LAUNCH_MARS_BASE");
    assert_eq!(
        project("terrestrial_laser_station"),
        "PROJECT_TERRESTRIAL_LASER"
    );
    assert_eq!(project("lagrange_laser_station"), "PROJECT_ORBITAL_LASER");

    // ⚠ The mechanical path still has to carry the other ~230 names untouched. A
    // translation table that starts rewriting things it was not asked to rewrite is
    // the same defect pointed the other way.
    assert_eq!(building("monument"), "BUILDING_MONUMENT");
    assert_eq!(building("amphitheater"), "BUILDING_AMPHITHEATER");
    assert_eq!(district("campus"), "DISTRICT_CAMPUS");
    assert_eq!(civ6_tech_name("mining"), "TECH_MINING");
    assert_eq!(civ6_civic_name("drama_poetry"), "CIVIC_DRAMA_POETRY");
    assert_eq!(
        project("build_nuclear_device"),
        "PROJECT_BUILD_NUCLEAR_DEVICE"
    );
    // Already fixed before this change; keep it pinned so the table stays whole.
    assert_eq!(building("medieval_walls"), "BUILDING_CASTLE");
    assert_eq!(
        project("campus_research_grants"),
        "PROJECT_ENHANCE_DISTRICT_CAMPUS"
    );
}

/// ⚠ A wonder reaches Civilization VI as a BUILDING, so it must use the building
/// table — #959 added the divergent spellings but left `Item::Wonder` formatting its
/// own name one line away, so all three kept going out wrong. This asserts the two
/// arms agree, which is the property that was actually missing.
#[test]
fn a_wonder_and_a_building_translate_through_the_same_table() {
    use civvis::game::Item;
    use civvis::name::Name;

    for name in [
        "mausoleum_at_halicarnassus",
        "statue_of_liberty",
        "university_of_sankore",
        "stonehenge",
        "great_bath",
    ] {
        let as_wonder = civ6_build_name(&Item::Wonder {
            wonder: Name::new(name),
            pos: (0, 0),
        })
        .expect("a wonder always names something");
        assert_eq!(
            as_wonder,
            civ6_building_type(&Name::new(name)),
            "{name} must translate the same way whichever Item variant carries it"
        );
    }

    assert_eq!(
        civ6_build_name(&Item::Wonder {
            wonder: Name::new("mausoleum_at_halicarnassus"),
            pos: (0, 0)
        })
        .as_deref(),
        Some("BUILDING_HALICARNASSUS_MAUSOLEUM")
    );
    // The plot is part of the decision and must survive the shared table.
    assert_eq!(
        civ6_build_pos(&Item::Wonder {
            wonder: Name::new("stonehenge"),
            pos: (3, 4)
        }),
        Some(civvis::hex::axial_to_offset(3, 4))
    );
}

/// The outbound table must not silently shrink relative to CIVVIS's own ruleset.
///
/// This is the check that would have caught the original defect: it walks every
/// building and district CIVVIS can order and fails on any whose translated name is
/// still the mechanical uppercase when the shipped database has no such row. The
/// known-divergent list is spelled out because CI has no Civilization VI install —
/// if a ruleset entry joins that list, this test is where it gets recorded.
#[test]
fn every_name_civilization_vi_spells_differently_is_translated() {
    use civvis::name::Name;

    // Read out of `Cache/DebugGameplay.sqlite` on 2026-08-02 by comparing every
    // `data/*.json` key against Buildings/Districts/Projects/Technologies.
    const DIVERGENT_BUILDINGS: &[&str] = &[
        "ancient_walls",
        "medieval_walls",
        "renaissance_walls",
        "art_museum",
        "archaeological_museum",
        "oil_power_plant",
        "nuclear_power_plant",
        "mausoleum_at_halicarnassus",
        "statue_of_liberty",
        "university_of_sankore",
        "audience_chamber",
        "ancestral_hall",
        "warlords_throne",
        "foreign_ministry",
        "grand_masters_chapel",
        "intelligence_agency",
        "national_history_museum",
        "royal_society",
        "war_department",
    ];
    const DIVERGENT_DISTRICTS: &[&str] = &[
        "theater_square",
        "government_plaza",
        "water_park",
        "copacabana",
    ];

    for name in DIVERGENT_BUILDINGS {
        let mapped = civ6_building_type(&Name::new(name));
        assert_ne!(
            mapped,
            format!("BUILDING_{}", name.to_ascii_uppercase()),
            "{name} is known to be spelled differently in Civilization VI, so the \
                 mechanical uppercase is exactly the name the mod refuses"
        );
    }
    for name in DIVERGENT_DISTRICTS {
        let mapped = civ6_district_type(&Name::new(name));
        assert_ne!(
            mapped,
            format!("DISTRICT_{}", name.to_ascii_uppercase()),
            "{name} is known to be spelled differently in Civilization VI"
        );
    }
}
/// `aluminum-ask-tops-up-gold`: the Aluminum ask's ceiling is eight Gold
/// a unit, never past the treasury above the floor, and only ever raised.
#[test]
fn the_aluminum_ask_ceiling_tops_up_within_the_treasury() {
    let ask = |verb: &str, gold: i32| super::Order {
        kind: "buy",
        subject: Some(2),
        verb: Some(verb.to_string()),
        pos: Some((gold, 0)),
    };
    let barter = ask("RESOURCE_ALUMINUM=6;RESOURCE_AMBER=1", 0);
    assert_eq!(super::aluminum_ask_ceiling(&barter, 400, 100), Some(48));
    assert_eq!(
        super::aluminum_ask_ceiling(&barter, 120, 100),
        Some(20),
        "the treasury above the floor"
    );
    assert_eq!(
        super::aluminum_ask_ceiling(&barter, 90, 100),
        None,
        "nothing above the floor"
    );
    assert_eq!(
        super::aluminum_ask_ceiling(&ask("RESOURCE_ALUMINUM=6", 60), 400, 100),
        None,
        "a higher ceiling stands"
    );
    assert_eq!(
        super::aluminum_ask_ceiling(&ask("RESOURCE_NITER=10;RESOURCE_AMBER=1", 0), 400, 100),
        None,
        "Aluminum only"
    );
    let sale = super::Order {
        kind: "sell",
        subject: Some(2),
        verb: Some("RESOURCE_ALUMINUM=6".to_string()),
        pos: Some((0, 0)),
    };
    assert_eq!(
        super::aluminum_ask_ceiling(&sale, 400, 100),
        None,
        "buys only"
    );
}

/// `denial-spies-count-their-losses`: the ledger reads our Spy losses
/// from the run's log as it grows, whole lines only, and starts over on a
/// new (shorter) log.
#[test]
fn spy_loss_ledger_reads_the_log_as_it_grows() {
    use std::io::Write;
    let dir = std::env::temp_dir().join(format!(
        "civvis-spy-ledger-{}-{}",
        std::process::id(),
        line!()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("events.jsonl");
    let lost = |turn: u32, kind: &str| {
        format!(
            "{{\"kind\": \"unit_lost\", \"turn\": {turn}, \"unit\": 7, \"unit_kind\": \"{kind}\"}}\n"
        )
    };
    std::fs::write(
        &path,
        format!(
            "{}{}{}",
            lost(119, "UNIT_SPY"),
            lost(120, "UNIT_WARRIOR"),
            "{\"kind\": \"state\", \"turn\": 121}\n"
        ),
    )
    .unwrap();
    let mut ledger = SpyLossLedger::default();
    ledger.refresh(&path);
    assert_eq!(ledger.through(200), vec![119]);

    // A half-written line waits for its newline.
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    let next = lost(129, "UNIT_SPY");
    file.write_all(&next.as_bytes()[..20]).unwrap();
    ledger.refresh(&path);
    assert_eq!(ledger.through(200), vec![119]);
    file.write_all(&next.as_bytes()[20..]).unwrap();
    file.write_all(lost(131, "UNIT_SPY").as_bytes()).unwrap();
    ledger.refresh(&path);
    assert_eq!(ledger.through(200), vec![119, 129, 131]);
    // A replay reads only the losses up to its turn.
    assert_eq!(ledger.through(129), vec![119, 129]);

    // A Spy lifted off the map by our own travel request is no loss.
    file.write_all(
        b"{\"kind\": \"unit_lost\", \"turn\": 133, \"unit\": 9, \"unit_kind\": \"UNIT_SPY\", \"cause\": \"spy_travel\"}\n",
    )
    .unwrap();
    ledger.refresh(&path);
    assert_eq!(ledger.through(200), vec![119, 129, 131]);

    // A new run's shorter log starts the ledger over.
    std::fs::write(&path, lost(5, "UNIT_SPY")).unwrap();
    ledger.refresh(&path);
    assert_eq!(ledger.through(200), vec![5]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `luxury-asks-pay-in-strategics-2`: a partner that answered
/// `barter_short` is skipped, and Horses stay home while Niter is spare;
/// with no other partner and only Horses, version two still asks.
#[test]
fn luxury_strategic_ask_2_skips_the_shorted_partner_and_keeps_the_horses() {
    let short = |needed: f64, have: f64| StateCity {
        amenities: have,
        amenities_needed: needed,
        ..StateCity::default()
    };
    let offering = |player, luxuries: &[&str]| StateRival {
        player,
        tradeable_luxuries: Some(luxuries.iter().map(|name| name.to_string()).collect()),
        ..StateRival::default()
    };
    let spare = |resource: &str, id: &str, amount: u32| civvis::ai::BarterSpare {
        resource: resource.to_string(),
        resource_id: id.to_string(),
        amount,
        luxury: false,
    };
    let state = StateSnapshot {
        turn: 94, // the luxury slot; week 15
        gold: 200,
        gold_per_turn: Some(12.0),
        cities: vec![short(5.0, 4.0), short(6.0, 5.0), short(3.0, 4.0)],
        luxury_counts: Some([("RESOURCE_AMBER".to_string(), 1.0)].into_iter().collect()),
        rivals: vec![
            offering(4, &["RESOURCE_SILK"]),
            offering(6, &["RESOURCE_WINE"]),
        ],
        ..StateSnapshot::default()
    };
    let book = |_seller: i64, resource: &str, amount: u32| match resource {
        "niter" => 20.0 * f64::from(amount),
        "horses" => 40.0 * f64::from(amount),
        _ => 0.0,
    };
    let spares = vec![
        spare("RESOURCE_HORSES", "horses", 20),
        spare("RESOURCE_NITER", "niter", 12),
    ];
    let none = std::collections::BTreeSet::new();

    // Version one: seat 6 (15 % 2 == 1), Horses, the lot that covers the
    // copy most cheaply by the book.
    let mut v1 = Vec::new();
    assert_eq!(
        append_luxury_strategic_ask(&state, &mut v1, &spares, &book),
        None
    );
    assert_eq!(v1[0].subject, Some(6));
    assert_eq!(v1[0].verb.as_deref(), Some("LUXURY_ANY;RESOURCE_HORSES=5"));

    // Version two: Niter, not Horses.
    let mut v2 = Vec::new();
    assert_eq!(
        append_luxury_strategic_ask_2(&state, &mut v2, &spares, &book, &none),
        None
    );
    assert_eq!(v2[0].subject, Some(6));
    assert_eq!(v2[0].verb.as_deref(), Some("LUXURY_ANY;RESOURCE_NITER=9"));

    // Seat 6 shorted: seat 4 is asked instead.
    let shorted: std::collections::BTreeSet<i64> = [6].into_iter().collect();
    let mut skip = Vec::new();
    assert_eq!(
        append_luxury_strategic_ask_2(&state, &mut skip, &spares, &book, &shorted),
        None
    );
    assert_eq!(skip[0].subject, Some(4));

    // Both shorted: nobody is asked.
    let both: std::collections::BTreeSet<i64> = [4, 6].into_iter().collect();
    let mut held = Vec::new();
    assert_eq!(
        append_luxury_strategic_ask_2(&state, &mut held, &spares, &book, &both),
        Some("luxury_strategic_hold:no_partner")
    );

    // Only Horses spare: version two still offers them.
    let horses = vec![spare("RESOURCE_HORSES", "horses", 20)];
    let mut only = Vec::new();
    assert_eq!(
        append_luxury_strategic_ask_2(&state, &mut only, &horses, &book, &none),
        None
    );
    assert_eq!(
        only[0].verb.as_deref(),
        Some("LUXURY_ANY;RESOURCE_HORSES=5")
    );
}

/// `luxury-asks-pay-in-strategics-2`: the ledger keeps the `barter_short`
/// answers of our buy arm, by turn and partner, each with the items the same
/// turn's luxury ask to that partner put up, and nothing else.
#[test]
fn barter_short_ledger_reads_the_shorts_and_what_they_offered() {
    let dir = std::env::temp_dir().join(format!(
        "civvis-barter-ledger-{}-{}",
        std::process::id(),
        line!()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("events.jsonl");
    std::fs::write(
        &path,
        concat!(
            "{\"kind\": \"deal_offer\", \"reason\": \"barter_short\", \"target\": 3, \"turn\": 88, \"verb\": \"LUXURY_ANY\"}\n",
            "{\"kind\": \"order_failed\", \"order_kind\": \"buy\", \"subject\": 3, \"turn\": 88, \"verb\": \"LUXURY_ANY;RESOURCE_HORSES=3\"}\n",
            "{\"kind\": \"deal_offer\", \"reason\": \"asked\", \"target\": 4, \"turn\": 94, \"verb\": \"LUXURY_ANY\"}\n",
            "{\"kind\": \"order_verified\", \"order_kind\": \"buy\", \"subject\": 4, \"turn\": 94, \"verb\": \"LUXURY_ANY;RESOURCE_SALT=1\"}\n",
            "{\"kind\": \"order_failed\", \"order_kind\": \"buy\", \"subject\": 1, \"turn\": 100, \"verb\": \"LUXURY_ANY;RESOURCE_MARBLE=1,RESOURCE_WINE=1\"}\n",
            "{\"kind\": \"deal_offer\", \"reason\": \"barter_short\", \"target\": 1, \"turn\": 100, \"verb\": \"LUXURY_ANY\"}\n",
            "{\"kind\": \"deal_offer\", \"reason\": \"barter_short\", \"target\": 5, \"turn\": 106, \"verb\": \"LUXURY_ANY\"}\n",
        ),
    )
    .unwrap();
    let mut ledger = BarterShortLedger::default();
    ledger.refresh(&path);
    let names = |list: &[&str]| list.iter().map(|name| name.to_string()).collect::<Vec<_>>();
    assert_eq!(
        ledger.through(200),
        vec![
            (88, 3, names(&["RESOURCE_HORSES"])),
            (100, 1, names(&["RESOURCE_MARBLE", "RESOURCE_WINE"])),
            (106, 5, Vec::new()),
        ]
    );
    assert_eq!(
        ledger.through(95),
        vec![(88, 3, names(&["RESOURCE_HORSES"]))]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// `luxury-asks-pay-in-strategics-2`: a luxury whose swap shorted stays off
/// our side; with no other spare the swap holds `no_spare`, the hold the
/// strategic ask runs on.
#[test]
fn luxury_swap_2_keeps_the_stale_copy_home() {
    let short = |needed: f64, have: f64| StateCity {
        amenities: have,
        amenities_needed: needed,
        ..StateCity::default()
    };
    let state = StateSnapshot {
        turn: 94,
        gold: 200,
        cities: vec![short(5.0, 3.0), short(6.0, 4.0), short(3.0, 4.0)],
        luxury_counts: Some([("RESOURCE_MARBLE".to_string(), 2.0)].into_iter().collect()),
        rivals: vec![StateRival {
            player: 4,
            tradeable_luxuries: Some(vec!["RESOURCE_SILK".to_string()]),
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    let none = std::collections::BTreeSet::new();
    let mut fresh = Vec::new();
    assert_eq!(
        append_luxury_swap_order_2(&state, &mut fresh, None, &none),
        None
    );
    assert_eq!(
        fresh[0].verb.as_deref(),
        Some("LUXURY_ANY;RESOURCE_MARBLE=1")
    );
    let stale: std::collections::BTreeSet<String> =
        ["RESOURCE_MARBLE".to_string()].into_iter().collect();
    let mut held = Vec::new();
    assert_eq!(
        append_luxury_swap_order_2(&state, &mut held, None, &stale),
        Some("luxury_swap_hold:no_spare")
    );
    assert!(held.is_empty());
}

/// `captives-come-home`: the captor holding most of our Spies is asked on its
/// own cadence turn, at peace only, for Gold over the reserve up to the cap;
/// a short treasury or a deal already in flight to it holds the ask.
#[test]
fn captive_ransom_asks_the_captor_on_its_turn_within_the_treasury() {
    let rival = |player: usize, captives: Option<u32>, at_war: bool| StateRival {
        player,
        captives_of_ours: captives,
        at_war,
        ..StateRival::default()
    };
    // Turn 12: player 2's cadence turn (12 % 5 == 2 % 5); player 7 shares it.
    let mut state = StateSnapshot {
        turn: 12,
        gold: 420,
        rivals: vec![
            rival(1, Some(3), false),
            rival(2, Some(1), false),
            rival(7, Some(2), false),
            rival(12, Some(4), true),
        ],
        ..StateSnapshot::default()
    };
    let mut orders = Vec::new();
    assert_eq!(append_captive_ransom_order(&state, &mut orders, None), None);
    assert_eq!(orders.len(), 1);
    let ask = &orders[0];
    assert_eq!(ask.kind, "buy");
    assert_eq!(
        ask.subject,
        Some(7),
        "the most-held captor whose turn it is; never one at war with us"
    );
    assert_eq!(ask.verb.as_deref(), Some("CAPTIVE"));
    assert_eq!(ask.pos, Some((RANSOM_CEILING_MAX, 0)), "capped");

    // A deal already heading to that captor holds the ask.
    assert_eq!(
        append_captive_ransom_order(&state, &mut orders, None),
        Some("ransom_hold:deal_in_flight")
    );
    assert_eq!(orders.len(), 1);

    // The ceiling is the treasury over the reserve, and too little asks nothing.
    state.gold = RANSOM_GOLD_RESERVE + 90;
    let mut orders = Vec::new();
    append_captive_ransom_order(&state, &mut orders, None);
    assert_eq!(orders[0].pos, Some((90, 0)));
    state.gold = RANSOM_GOLD_RESERVE + i64::from(RANSOM_CEILING_MIN) - 1;
    let mut orders = Vec::new();
    assert_eq!(
        append_captive_ransom_order(&state, &mut orders, None),
        Some("ransom_hold:treasury")
    );
    assert!(orders.is_empty());

    // Off its cadence turn, or holding none of ours, a rival is not asked.
    state.gold = 420;
    state.turn = 13;
    let mut orders = Vec::new();
    assert_eq!(
        append_captive_ransom_order(&state, &mut orders, None),
        Some("ransom_hold:no_captor")
    );
    state.turn = 12;
    state.rivals = vec![rival(2, Some(0), false), rival(7, None, false)];
    assert_eq!(
        append_captive_ransom_order(&state, &mut orders, None),
        Some("ransom_hold:no_captor")
    );
    assert!(orders.is_empty());
}

/// `captives-come-home-2`: Favor rides with the Gold as `CAPTIVE;FAVOR=n`, and
/// with Favor offered a treasury under the Gold floor still asks.
#[test]
fn captive_ransom_offers_favor_beside_the_gold() {
    let mut state = StateSnapshot {
        turn: 12,
        gold: 420,
        rivals: vec![StateRival {
            player: 7,
            captives_of_ours: Some(1),
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    let mut orders = Vec::new();
    assert_eq!(
        append_captive_ransom_order(&state, &mut orders, Some(90)),
        None
    );
    assert_eq!(orders[0].verb.as_deref(), Some("CAPTIVE;FAVOR=90"));
    assert_eq!(orders[0].pos, Some((RANSOM_CEILING_MAX, 0)));

    state.gold = RANSOM_GOLD_RESERVE + 10;
    let mut orders = Vec::new();
    assert_eq!(
        append_captive_ransom_order(&state, &mut orders, Some(90)),
        None
    );
    assert_eq!(
        orders[0].pos,
        Some((10, 0)),
        "Favor carries an ask the Gold floor would hold"
    );
    let mut orders = Vec::new();
    assert_eq!(
        append_captive_ransom_order(&state, &mut orders, None),
        Some("ransom_hold:treasury")
    );
}

/// `tribute-buys-the-friendship`: the strongest neighbour at peace, stronger
/// than us and too cold for the friendship ask, is sent one whole gift that
/// closes its opinion gap to the Friendly median, on its cadence turn. A gap
/// one gift cannot close, a rival already paid, or a treasury that cannot meet
/// the whole gift sends nothing.
#[test]
fn tribute_closes_the_opinion_gap_of_the_strongest_cold_neighbour() {
    let reason = |text: &str, score: f64| civvis::mirror::StateOpinionModifier {
        text: text.to_string(),
        score,
    };
    let rival = |player: usize, military: f64, mood: &str, total: f64| StateRival {
        player,
        military,
        diplomatic_state: Some(format!("DIPLO_STATE_{mood}")),
        opinion_modifiers: Some(vec![reason("Unknown Reason", total)]),
        ..StateRival::default()
    };
    // Turn 13: player 3's cadence turn (13 % 5 == 3 % 5). Player 3 is the
    // strongest at peace; player 4 is stronger but at war with us.
    let mut at_war = rival(4, 900.0, "WAR", -30.0);
    at_war.at_war = true;
    let mut state = StateSnapshot {
        turn: 13,
        gold: 400,
        military: 200.0,
        rivals: vec![
            rival(1, 300.0, "UNFRIENDLY", -2.0),
            rival(3, 600.0, "UNFRIENDLY", -3.0),
            at_war,
        ],
        ..StateSnapshot::default()
    };
    let mut orders = Vec::new();
    assert_eq!(append_tribute_order(&state, &mut orders), None);
    assert_eq!(orders.len(), 1);
    let gift = &orders[0];
    assert_eq!(gift.kind, "aid_gift");
    assert_eq!(
        gift.subject,
        Some(3),
        "the strongest at peace, never one at war"
    );
    assert_eq!(gift.verb.as_deref(), Some("TRIBUTE"));
    assert_eq!(
        gift.pos,
        Some((80, 0)),
        "8 points from -3 to +5, ten Gold a point"
    );

    // Any other order to that rival this frame holds the gift.
    assert_eq!(
        append_tribute_order(&state, &mut orders),
        Some("tribute_hold:deal_in_flight")
    );
    assert_eq!(orders.len(), 1);

    // A gap of exactly one gift is sent; one point more is out of reach.
    state.rivals[1].opinion_modifiers = Some(vec![reason("Unknown Reason", -5.0)]);
    let mut orders = Vec::new();
    append_tribute_order(&state, &mut orders);
    assert_eq!(orders[0].pos, Some((100, 0)));
    state.rivals[1].opinion_modifiers = Some(vec![reason("Different Governments", -20.0)]);
    let mut orders = Vec::new();
    assert_eq!(
        append_tribute_order(&state, &mut orders),
        Some("tribute_hold:out_of_reach")
    );
    assert!(orders.is_empty());

    // A rival already holding a favorable deal is not paid again.
    state.rivals[1].opinion_modifiers = Some(vec![
        reason("Unknown Reason", -6.0),
        reason("Favorable trade deals to them", 3.0),
    ]);
    assert_eq!(
        append_tribute_order(&state, &mut Vec::new()),
        Some("tribute_hold:already_paid")
    );

    // The gift is whole or not at all, over the reserve.
    state.rivals[1].opinion_modifiers = Some(vec![reason("Unknown Reason", -3.0)]);
    state.gold = TRIBUTE_GOLD_RESERVE + 80;
    assert_eq!(append_tribute_order(&state, &mut Vec::new()), None);
    state.gold = TRIBUTE_GOLD_RESERVE + 79;
    let mut orders = Vec::new();
    assert_eq!(
        append_tribute_order(&state, &mut orders),
        Some("tribute_hold:treasury")
    );
    assert!(orders.is_empty());
    state.gold = 400;

    // A point short still sends the smallest gift.
    state.rivals[1].opinion_modifiers = Some(vec![reason("Unknown Reason", 4.0)]);
    let mut orders = Vec::new();
    append_tribute_order(&state, &mut orders);
    assert_eq!(orders[0].pos, Some((TRIBUTE_MIN_GOLD as i32, 0)));
    state.rivals[1].opinion_modifiers = Some(vec![reason("Unknown Reason", -3.0)]);

    // Off its cadence turn nothing goes out.
    state.turn = 14;
    assert_eq!(
        append_tribute_order(&state, &mut Vec::new()),
        Some("tribute_hold:cadence")
    );
    state.turn = 13;

    // A rival already warm enough, Friendly, denounced, weaker than us, or
    // with no breakdown is not sent tribute.
    state.rivals[1].opinion_modifiers = Some(vec![reason("Declared Friend", 6.0)]);
    assert_eq!(
        append_tribute_order(&state, &mut Vec::new()),
        Some("tribute_hold:no_gap")
    );
    state.rivals[1].opinion_modifiers = Some(vec![reason("Unknown Reason", -3.0)]);
    state.rivals[1].diplomatic_state = Some("DIPLO_STATE_FRIENDLY".to_string());
    assert_eq!(
        append_tribute_order(&state, &mut Vec::new()),
        Some("tribute_hold:courted")
    );
    state.rivals[1].diplomatic_state = Some("DIPLO_STATE_DENOUNCED".to_string());
    assert_eq!(
        append_tribute_order(&state, &mut Vec::new()),
        Some("tribute_hold:denounced")
    );
    state.rivals[1].diplomatic_state = Some("DIPLO_STATE_NEUTRAL".to_string());
    state.military = 700.0;
    assert_eq!(
        append_tribute_order(&state, &mut Vec::new()),
        Some("tribute_hold:not_stronger")
    );
    state.military = 200.0;
    state.rivals[1].opinion_modifiers = None;
    assert_eq!(
        append_tribute_order(&state, &mut Vec::new()),
        Some("tribute_hold:opinion_unknown")
    );
}
