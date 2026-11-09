use super::*;
use civvis::mirror::{
    Snapshot, StateCity, StateGovernor, StateGreatPerson, StateMinor, StateRival, StateSnapshot,
    StateTradeRoute, StateUnit, TilesChunk,
};

fn order(
    kind: &str,
    subject: Option<i64>,
    verb: Option<&str>,
    pos: Option<(i32, i32)>,
) -> IssuedOrder {
    IssuedOrder {
        kind: kind.to_string(),
        subject,
        verb: verb.map(str::to_string),
        pos,
    }
}

fn unit(id: i64, kind: &str, x: i32, y: i32) -> StateUnit {
    StateUnit {
        id,
        kind: kind.to_string(),
        x,
        y,
        hp: 100.0,
        ..StateUnit::default()
    }
}

fn city(id: i64, x: i32, y: i32, producing: Option<&str>) -> StateCity {
    StateCity {
        id,
        name: format!("city-{id}"),
        x,
        y,
        producing: producing.map(str::to_string),
        ..StateCity::default()
    }
}

fn frame(turn: u32) -> StateSnapshot {
    StateSnapshot {
        turn,
        ..StateSnapshot::default()
    }
}

fn no_tiles() -> Snapshot {
    Snapshot::from_chunks(&[])
}

fn check(
    order: &IssuedOrder,
    before: &StateSnapshot,
    after: &StateSnapshot,
    evidence: &[serde_json::Value],
) -> Verdict {
    verify_order(order, before.turn, before, after, &no_tiles(), evidence)
}

fn failed(why: &str) -> Verdict {
    Verdict::Failed(why.to_string())
}

#[test]
fn excavation_accepts_native_pending_artifact_without_builder_charges() {
    let mut before = frame(166);
    before.seat.local_player = 0;
    let mut actor = unit(6_946_833, "UNIT_ARCHAEOLOGIST", 11, 27);
    actor.build_charges = Some(0);
    before.units.push(actor);
    let mut after = before.clone();
    after.turn = 167;
    let dig = order("unit", Some(6_946_833), Some("EXCAVATE"), None);
    assert_eq!(check(&dig, &before, &after, &[]), failed("no_improvement"));
    let witness = serde_json::json!({
        "kind": "artifact_extraction_pending", "unit": 6_946_833,
        "artifact": 42, "turn": 166, "player": 0
    });
    assert_eq!(check(&dig, &before, &after, &[witness]), Verdict::Verified);
}

#[test]
fn excavation_witness_cannot_credit_other_units_turns_or_operations() {
    let mut before = frame(166);
    before.seat.local_player = 0;
    let mut actor = unit(6_946_833, "UNIT_ARCHAEOLOGIST", 11, 27);
    actor.build_charges = Some(0);
    before.units.push(actor);
    let mut after = before.clone();
    after.turn = 167;
    let dig = order("unit", Some(6_946_833), Some("EXCAVATE"), None);
    let witness = serde_json::json!({
        "kind": "artifact_extraction_pending", "unit": 6_946_833,
        "artifact": 42, "turn": 166, "player": 0
    });
    for (field, value) in [
        ("unit", serde_json::json!(92)),
        ("player", serde_json::json!(1)),
        ("player", serde_json::json!(null)),
        ("turn", serde_json::json!(165)),
        ("turn", serde_json::json!(167)),
        ("artifact", serde_json::json!(-1)),
        ("artifact", serde_json::json!(null)),
        ("artifact", serde_json::json!("42")),
        ("kind", serde_json::json!("deal_closed")),
    ] {
        let mut unrelated = witness.clone();
        unrelated[field] = value;
        assert_eq!(
            check(&dig, &before, &after, &[unrelated]),
            failed("no_improvement"),
            "must attribute the extraction: {field}"
        );
    }
    let farm = order(
        "unit",
        Some(6_946_833),
        Some("IMPROVE:IMPROVEMENT_FARM"),
        None,
    );
    assert_eq!(
        check(&farm, &before, &after, std::slice::from_ref(&witness)),
        failed("no_improvement")
    );
    before.seat.local_player = -1;
    assert_eq!(
        check(&dig, &before, &after, std::slice::from_ref(&witness)),
        failed("no_improvement")
    );
    before.seat.local_player = 0;
    before.units[0].kind = "UNIT_BUILDER".into();
    after.units[0].kind = "UNIT_BUILDER".into();
    assert_eq!(
        check(&dig, &before, &after, &[witness]),
        failed("no_improvement")
    );
}

#[test]
fn excavation_pending_witness_reaches_live_order_settlement() {
    let dir = std::env::temp_dir().join(format!("civvis-excavation-native-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let events = dir.join("events.jsonl");
    std::fs::write(
        &events,
        "{\"kind\": \"artifact_extraction_pending\", \"unit\": 6946833, \"artifact\": 42, \"turn\": 166, \"player\": 0}\n",
    )
    .unwrap();
    let mut before = frame(166);
    before.seat.local_player = 0;
    let mut actor = unit(6_946_833, "UNIT_ARCHAEOLOGIST", 11, 27);
    actor.build_charges = Some(0);
    before.units.push(actor);
    let mut after = before.clone();
    after.turn = 167;
    let mut pending = vec![PendingOrders {
        turn: 166,
        frame: 0,
        orders: vec![order("unit", Some(6_946_833), Some("EXCAVATE"), None)],
        before,
    }];
    let rows = settle_pending_orders(
        &mut pending,
        &events,
        &after,
        &no_tiles(),
        &mut HostOrderRefusals::default(),
    );
    assert!(pending.is_empty());
    assert!(rows
        .iter()
        .any(|row| row.kind == "order_verified" && row.subject == Some(6_946_833)));
    assert!(!rows.iter().any(|row| row.kind == "order_failed"));
    std::fs::remove_dir_all(dir).unwrap();
}

fn event(json: &str) -> serde_json::Value {
    serde_json::from_str(json).expect("test event is JSON")
}

#[test]
fn a_move_is_verified_by_the_unit_standing_closer_not_by_the_return_code() {
    let mut before = frame(42);
    before.units = vec![unit(7, "UNIT_SETTLER", 13, 11)];
    let walk = order("unit", Some(7), Some("MOVE_TO"), Some((17, 11)));

    let mut closer = frame(43);
    closer.units = vec![unit(7, "UNIT_SETTLER", 15, 11)];
    assert_eq!(check(&walk, &before, &closer, &[]), Verdict::Verified);

    let mut arrived = frame(43);
    arrived.units = vec![unit(7, "UNIT_SETTLER", 17, 11)];
    assert_eq!(check(&walk, &before, &arrived, &[]), Verdict::Verified);

    // The measured case: `applied = true`, no refusal, and the settler at
    // (12,9) — farther from (14,11) than it started.
    let mut away = frame(43);
    away.units = vec![unit(7, "UNIT_SETTLER", 9, 11)];
    assert_eq!(check(&walk, &before, &away, &[]), failed("moved_away"));

    let mut stood = frame(43);
    stood.units = vec![unit(7, "UNIT_SETTLER", 13, 11)];
    assert_eq!(check(&walk, &before, &stood, &[]), failed("did_not_move"));
}

#[test]
fn a_move_superseded_by_a_later_same_turn_replan_is_not_a_host_noop() {
    let mut before = frame(42);
    before.units = vec![unit(7, "UNIT_WARRIOR", 13, 11)];
    let walk = order("unit", Some(7), Some("MOVE_TO"), Some((17, 11)));
    let mut after = frame(43);
    // The later frame replanned this unit, but the next-turn snapshot
    // leaves it at the same distance from the earlier target. A position
    // diff alone cannot tell that from a host-side no-op.
    after.units = vec![unit(7, "UNIT_WARRIOR", 13, 11)];

    assert_eq!(
        verify_unit_order(
            &walk,
            before.turn,
            &before,
            &after,
            &no_tiles(),
            &[],
            LaterFrames {
                moved: true,
                ..LaterFrames::default()
            },
        ),
        failed("superseded_by_move")
    );
}

#[test]
fn a_settler_safety_rewrite_verifies_the_host_sent_leg() {
    let mut before = frame(50);
    before.units = vec![unit(7, "UNIT_SETTLER", 21, 21)];
    let walk = order("unit", Some(7), Some("MOVE_TO"), Some((21, 20)));

    let mut after = frame(51);
    after.units = vec![unit(7, "UNIT_SETTLER", 22, 22)];
    let evidence = vec![event(
        r#"{"kind":"settler_capture_escape","turn":50,"settler":7,"want":[21,20],"sent":[22,22]}"#,
    )];
    assert_eq!(
        check(&walk, &before, &after, &evidence),
        Verdict::Verified,
        "the bridge's safety rewrite is successful actuation even when it retreats"
    );
}

#[test]
fn a_settler_consumed_by_its_own_founding_verifies_both_orders() {
    let mut before = frame(30);
    before.units = vec![unit(7, "UNIT_SETTLER", 12, 19)];
    let mut after = frame(31);
    after.cities = vec![city(65_536, 12, 19, Some("UNIT_WARRIOR"))];

    let walk = order("unit", Some(7), Some("MOVE_TO"), Some((12, 19)));
    let found = order("unit", Some(7), Some("FOUND_CITY"), None);
    assert_eq!(check(&walk, &before, &after, &[]), Verdict::Verified);
    assert_eq!(check(&found, &before, &after, &[]), Verdict::Verified);

    let mut still_no_city = frame(31);
    still_no_city.units = vec![unit(7, "UNIT_SETTLER", 12, 19)];
    assert_eq!(
        check(&found, &before, &still_no_city, &[]),
        failed("no_city")
    );
}

#[test]
fn an_attack_requires_its_own_targeted_combat_record() {
    let mut before = frame(50);
    before.units = vec![unit(7, "UNIT_ARCHER", 10, 10)];
    let mut enemy = unit(900, "UNIT_WARRIOR", 11, 10);
    enemy.player = 3;
    before.hostiles = vec![enemy.clone()];
    let strike = order("unit", Some(7), Some("RANGE_ATTACK"), Some((11, 10)));

    let mut wounded = frame(51);
    wounded.units = before.units.clone();
    let mut hurt = enemy.clone();
    hurt.hp = 70.0;
    wounded.hostiles = vec![hurt];
    assert_eq!(
        check(&strike, &before, &wounded, &[]),
        failed("attacker_unproven")
    );

    let mut killed = frame(51);
    killed.units = before.units.clone();
    assert_eq!(
        check(&strike, &before, &killed, &[]),
        failed("attacker_unproven")
    );

    let mut untouched = frame(51);
    untouched.units = before.units.clone();
    untouched.hostiles = vec![enemy.clone()];
    assert_eq!(
        check(&strike, &before, &untouched, &[]),
        failed("target_unharmed")
    );
    let combat = event(
        r#"{"kind":"combat","turn":50,"attacker":{"id":7,"player":0},"defender":{"id":900,"x":11,"y":10}}"#,
    );
    assert_eq!(
        check(&strike, &before, &untouched, &[combat]),
        Verdict::Verified
    );
}

#[test]
fn production_and_purchases_are_read_off_the_city() {
    let mut before = frame(10);
    before.cities = vec![city(65_536, 20, 20, Some("UNIT_WARRIOR"))];
    before.gold = 200;
    let produce = order("produce", Some(65_536), Some("BUILDING_MONUMENT"), None);
    let purchase = order(
        "purchase",
        Some(65_536),
        Some("UNIT_BUILDER"),
        Some((0, -1)),
    );

    let mut queued = frame(11);
    queued.cities = vec![city(65_536, 20, 20, Some("BUILDING_MONUMENT"))];
    queued.gold = 210;
    assert_eq!(check(&produce, &before, &queued, &[]), Verdict::Verified);
    assert_eq!(
        check(&purchase, &before, &queued, &[]),
        failed("not_bought")
    );

    let mut ignored = frame(11);
    ignored.cities = vec![city(65_536, 20, 20, Some("UNIT_WARRIOR"))];
    ignored.gold = 210;
    assert_eq!(
        check(&produce, &before, &ignored, &[]),
        failed("producing=UNIT_WARRIOR")
    );

    let mut bought = frame(11);
    bought.cities = vec![city(65_536, 20, 20, Some("UNIT_WARRIOR"))];
    bought.units = vec![unit(8, "UNIT_BUILDER", 20, 20)];
    bought.gold = 210;
    assert_eq!(check(&purchase, &before, &bought, &[]), Verdict::Verified);

    // A completed item counts as much as a queued one.
    let mut finished = frame(11);
    let mut done = city(65_536, 20, 20, Some("UNIT_WARRIOR"));
    done.buildings = vec!["BUILDING_MONUMENT".to_string()];
    finished.cities = vec![done];
    assert_eq!(check(&produce, &before, &finished, &[]), Verdict::Verified);
}

#[test]
fn a_purchased_unit_cannot_verify_a_same_turn_production_order() {
    let mut before = frame(30);
    before.cities = vec![city(65_536, 20, 20, Some("UNIT_SETTLER"))];

    let mut after = frame(31);
    after.cities = before.cities.clone();
    after.units = vec![unit(8, "UNIT_WARRIOR", 20, 20)];

    let produce = order("produce", Some(65_536), Some("UNIT_WARRIOR"), None);
    let purchase = order(
        "purchase",
        Some(65_536),
        Some("UNIT_WARRIOR"),
        Some((0, -1)),
    );
    let pending = PendingOrders {
        turn: 30,
        frame: 0,
        orders: vec![produce.clone(), purchase.clone()],
        before: before.clone(),
    };

    // Without a sibling purchase, the new unit remains valid completion
    // evidence for the production postcondition.
    assert_eq!(check(&produce, &before, &after, &[]), Verdict::Verified);

    let checks = verify_orders(&pending, &after, &no_tiles(), &[], &pending.orders);
    assert_eq!(checks[0].order, produce);
    assert_eq!(
        checks[0].verdict,
        failed("unit_appearance_ambiguous_purchase")
    );
    assert_eq!(checks[1].order, purchase);
    assert_eq!(checks[1].verdict, Verdict::Verified);
}

#[test]
fn a_later_same_turn_fortify_frame_proves_a_transient_fortify() {
    let mut before = frame(30);
    before.units = vec![unit(7, "UNIT_WARRIOR", 20, 20)];

    // The next turn has already cleared the transient fortified state by
    // moving this unit, so the ordinary next-turn postcondition alone
    // would report `not_fortified`.
    let mut after = frame(31);
    after.units = vec![unit(7, "UNIT_WARRIOR", 21, 20)];

    // Frame 1 is emitted after frame 0's FORTIFY has reached the host.
    let mut same_turn = frame(30);
    same_turn.frame = 1;
    let mut fortified = unit(7, "UNIT_WARRIOR", 20, 20);
    fortified.fortified = true;
    fortified.fortify_turns = 1;
    same_turn.units = vec![fortified];

    let pending = PendingOrders {
        turn: 30,
        frame: 0,
        orders: vec![order("unit", Some(7), Some("FORTIFY"), None)],
        before,
    };
    let later = later_fortified_units(&[same_turn.clone()], 30, pending.frame);
    let checks = verify_orders_with_later_fortifications(
        &pending,
        &after,
        &no_tiles(),
        &[],
        &pending.orders,
        LaterOrderEvidence {
            states: &[],
            fortified: &later,
            moved: &std::collections::BTreeSet::new(),
            policy_deck: false,
        },
    );
    assert_eq!(checks[0].verdict, Verdict::Verified);

    // Evidence from the frame on which the order was issued is not enough
    // to prove that the host accepted it.
    assert!(later_fortified_units(&[same_turn], 30, 1).is_empty());

    // A repeat FORTIFY issued after the unit was already fortified remains
    // a failure even if that old state is visible on a later frame.
    let mut already_fortified = frame(30);
    let mut old = unit(7, "UNIT_WARRIOR", 20, 20);
    old.fortified = true;
    old.fortify_turns = 1;
    already_fortified.units = vec![old];
    let repeated = PendingOrders {
        turn: 30,
        frame: 0,
        orders: vec![order("unit", Some(7), Some("FORTIFY"), None)],
        before: already_fortified,
    };
    let checks = verify_orders_with_later_fortifications(
        &repeated,
        &after,
        &no_tiles(),
        &[],
        &repeated.orders,
        LaterOrderEvidence {
            states: &[],
            fortified: &later,
            moved: &std::collections::BTreeSet::new(),
            policy_deck: false,
        },
    );
    assert_eq!(checks[0].verdict, failed("not_fortified"));

    // ⚠⚠ A FORTIFY THE DECIDER ITSELF OVERRODE IS NOT A BRIDGE FAILURE.
    // `not_fortified` is the second-largest failure reason on the ledger —
    // 3124 of 5193 FORTIFY orders over the runs of 2026-08-30 — and 95% of
    // them had a MOVE_TO for the same unit in the same turn, 61% of those
    // issued AFTER the fortify. Reading that as `not_fortified` sends an
    // auditor after an actuation bug that is not there.
    let mut movable = frame(30);
    movable.units = vec![unit(7, "UNIT_WARRIOR", 20, 20)];
    let overridden = PendingOrders {
        turn: 30,
        frame: 0,
        orders: vec![order("unit", Some(7), Some("FORTIFY"), None)],
        before: movable,
    };
    let mut walked_away = std::collections::BTreeSet::new();
    walked_away.insert(7);
    let checks = verify_orders_with_later_fortifications(
        &overridden,
        &after,
        &no_tiles(),
        &[],
        &overridden.orders,
        LaterOrderEvidence {
            states: &[],
            fortified: &std::collections::BTreeSet::new(),
            moved: &walked_away,
            policy_deck: false,
        },
    );
    assert_eq!(
        checks[0].verdict,
        failed("superseded_by_move"),
        "the decider walked this unit away after asking it to dig in"
    );
}

#[test]
fn a_later_same_turn_policy_replan_is_not_a_missing_card_failure() {
    let before = frame(64);
    let mut after = frame(65);
    after.policies = vec![
        "POLICY_CHARISMATIC_LEADER".to_string(),
        "POLICY_COLONIZATION".to_string(),
        "POLICY_FEUDAL_CONTRACT".to_string(),
        "POLICY_URBAN_PLANNING".to_string(),
    ];
    let earlier = order(
        "policy_deck",
        None,
        Some("POLICY_CHARISMATIC_LEADER,POLICY_COLONIZATION,POLICY_GOD_KING,POLICY_URBAN_PLANNING"),
        None,
    );
    let later = order(
            "policy_deck",
            None,
            Some(
                "POLICY_CHARISMATIC_LEADER,POLICY_COLONIZATION,POLICY_FEUDAL_CONTRACT,POLICY_URBAN_PLANNING",
            ),
            None,
        );
    let pending = PendingOrders {
        turn: 64,
        frame: 0,
        orders: vec![earlier.clone()],
        before: before.clone(),
    };
    let later_pending = PendingOrders {
        turn: 64,
        frame: 2,
        orders: vec![later.clone()],
        before,
    };

    assert_eq!(
        check(&earlier, &pending.before, &after, &[]),
        failed("missing=POLICY_GOD_KING"),
        "without same-turn context, the final state correctly lacks the old card"
    );
    assert!(later_policy_deck_matches(
        &[later_pending],
        64,
        pending.frame,
        &after
    ));
    let checks = verify_orders_with_later_fortifications(
        &pending,
        &after,
        &no_tiles(),
        &[],
        &pending.orders,
        LaterOrderEvidence {
            states: &[],
            fortified: &std::collections::BTreeSet::new(),
            moved: &std::collections::BTreeSet::new(),
            policy_deck: true,
        },
    );
    assert_eq!(checks[0].verdict, failed("superseded_by_policy_deck"));

    // A later deck that did not land must not hide a genuine missing card.
    let mut unchanged = after.clone();
    unchanged.policies = vec![
        "POLICY_CHARISMATIC_LEADER".to_string(),
        "POLICY_COLONIZATION".to_string(),
        "POLICY_URBAN_PLANNING".to_string(),
    ];
    assert!(!later_policy_deck_matches(
        &[PendingOrders {
            turn: 64,
            frame: 2,
            orders: vec![later],
            before: pending.before.clone(),
        }],
        64,
        pending.frame,
        &unchanged
    ));

    let deferred = event(
        r#"{"kind":"policy_deck_deferred","turn":64,"desired":["POLICY_CHARISMATIC_LEADER","POLICY_COLONIZATION","POLICY_GOD_KING","POLICY_URBAN_PLANNING"],"why":"same_turn_transaction_in_flight"}"#,
    );
    assert!(EVIDENCE_KINDS.contains(&"policy_deck_deferred"));
    assert_eq!(
        policy_deck_deferred_reason(std::slice::from_ref(&deferred), 64, &earlier).as_deref(),
        Some("deferred_same_turn_transaction_in_flight")
    );
    assert_eq!(
        check(
            &earlier,
            &pending.before,
            &after,
            std::slice::from_ref(&deferred)
        ),
        failed("deferred_same_turn_transaction_in_flight"),
        "the host's deferred transaction reason is more precise than a missing-card diff"
    );
}

#[test]
fn combat_policy_requires_host_acknowledgement_and_names_lethal_refusals() {
    let before = frame(7);
    let after = frame(8);
    let policy = order("combat_policy", None, Some("DOOMED_BLOW_VETO"), None);
    assert!(EVIDENCE_KINDS.contains(&"combat_policy_applied"));
    assert!(EVIDENCE_KINDS.contains(&"strike_survival_refused"));
    assert_eq!(
        check(&policy, &before, &after, &[]),
        failed("combat_policy_not_acknowledged")
    );
    let ack = event(r#"{"kind":"combat_policy_applied","turn":7,"policy":"DOOMED_BLOW_VETO"}"#);
    assert_eq!(check(&policy, &before, &after, &[ack]), Verdict::Verified);
    let stale = event(r#"{"kind":"combat_policy_applied","turn":6,"policy":"DOOMED_BLOW_VETO"}"#);
    assert_eq!(
        check(&policy, &before, &after, &[stale]),
        failed("combat_policy_not_acknowledged")
    );
    let refusal = event(r#"{"kind":"strike_survival_refused","turn":7,"unit":10,"x":31,"y":42}"#);
    assert_eq!(
        strike_refusal_reason(std::slice::from_ref(&refusal), 7, 10, Some((31, 42))).as_deref(),
        Some("lethal_host_preview")
    );
    assert_eq!(
        strike_refusal_reason(&[refusal], 7, 11, Some((31, 42))),
        None
    );
}

#[test]
fn a_bought_plot_is_verified_by_its_owner_on_the_board() {
    let mut before = frame(20);
    before.seat.local_player = 4;
    let mut after = frame(21);
    after.seat.local_player = 4;
    let buy = order("buy_plot", Some(65_536), None, Some((5, 6)));
    let tiles = |owner: i32| {
        let chunk: TilesChunk = serde_json::from_str(&format!(
            r#"{{"turn":21,"width":10,"height":10,"plots":[{{"x":5,"y":6,"o":{owner}}}]}}"#
        ))
        .expect("a tiles chunk parses");
        Snapshot::from_chunks(&[chunk])
    };
    assert_eq!(
        verify_order(&buy, 20, &before, &after, &tiles(4), &[]),
        Verdict::Verified
    );
    assert_eq!(
        verify_order(&buy, 20, &before, &after, &tiles(-1), &[]),
        failed("plot_not_owned")
    );
    assert_eq!(
        verify_order(&buy, 20, &before, &after, &no_tiles(), &[]),
        failed("plot_unseen")
    );
}

#[test]
fn an_envoy_is_verified_by_the_city_state_count() {
    let minor = |envoys: i64| StateMinor {
        player: 9,
        envoys,
        ..StateMinor::default()
    };
    let mut before = frame(60);
    before.minors = vec![minor(1)];
    let envoy = order("envoy", Some(9), Some("GIVE_INFLUENCE_TOKEN"), None);
    let mut placed = frame(61);
    placed.minors = vec![minor(2)];
    assert_eq!(check(&envoy, &before, &placed, &[]), Verdict::Verified);
    let mut held = frame(61);
    held.minors = vec![minor(1)];
    assert_eq!(
        check(&envoy, &before, &held, &[]),
        failed("envoys_unchanged")
    );
}

/// A denouncement is verified by the host's own `GetDenounceTurn` on the
/// next frame (#2590 exports it as `our_denounce_turn`), never by the
/// session having been asked.
#[test]
fn a_denouncement_is_verified_by_the_hosts_own_denounce_turn() {
    let rival = |since: Option<i64>| StateRival {
        player: 2,
        our_denounce_turn: since,
        ..StateRival::default()
    };
    let mut before = frame(80);
    before.rivals = vec![rival(Some(-1))];
    let denounce = order("denounce", Some(2), Some("DENOUNCE"), None);

    let mut after = frame(81);
    after.rivals = vec![rival(Some(80))];
    assert_eq!(check(&denounce, &before, &after, &[]), Verdict::Verified);

    after.rivals = vec![rival(Some(-1))];
    assert_eq!(
        check(&denounce, &before, &after, &[]),
        failed("not_denounced")
    );

    after.rivals = vec![rival(None)];
    assert_eq!(
        check(&denounce, &before, &after, &[]),
        Verdict::Unverifiable,
        "an older mod that does not export the turn cannot answer"
    );

    after.rivals = Vec::new();
    assert_eq!(
        check(&denounce, &before, &after, &[]),
        failed("player_unseen")
    );
}

/// The planner's alliance is proven by either step landing: ALLIED on the
/// next frame, or the declared friendship it asked first.
#[test]
fn an_alliance_is_verified_by_the_alliance_or_the_friendship_it_asked() {
    let rival = |state: &str, since: Option<i64>| StateRival {
        player: 2,
        diplomatic_state: Some(state.to_string()),
        friendship_turn: since,
        ..StateRival::default()
    };
    let mut before = frame(120);
    before.rivals = vec![rival("DIPLO_STATE_DECLARED_FRIEND", Some(100))];
    let alliance = order("alliance", Some(2), Some("ALLIANCE_RESEARCH"), None);
    let mut after = frame(121);
    after.rivals = vec![rival("DIPLO_STATE_ALLIED", Some(100))];
    assert_eq!(check(&alliance, &before, &after, &[]), Verdict::Verified);
    after.rivals = vec![rival("DIPLO_STATE_DECLARED_FRIEND", Some(120))];
    assert_eq!(
        check(&alliance, &before, &after, &[]),
        Verdict::Verified,
        "the friendship step landed"
    );
    after.rivals = vec![rival("DIPLO_STATE_DECLARED_FRIEND", Some(100))];
    assert_eq!(check(&alliance, &before, &after, &[]), failed("not_allied"));
    after.rivals = vec![rival("DIPLO_STATE_FRIENDLY", None)];
    assert_eq!(check(&alliance, &before, &after, &[]), failed("not_allied"));
    after.rivals = Vec::new();
    assert_eq!(
        check(&alliance, &before, &after, &[]),
        failed("player_unseen")
    );
}

/// Alliance orders cross only on a seat assigned the Diplomatic lane.
#[test]
fn alliance_orders_cross_on_the_diplomatic_lane_or_an_armed_culture_or_science_seat() {
    let batch = || {
        vec![
            Order {
                kind: "alliance",
                subject: Some(2),
                verb: Some("ALLIANCE_RESEARCH".to_string()),
                pos: None,
            },
            Order {
                kind: "sell",
                subject: Some(2),
                verb: Some("RESOURCE_DYES=1".to_string()),
                pos: Some((56, 0)),
            },
        ]
    };
    let mut kept = batch();
    assert_eq!(
        drop_off_lane_alliances(
            Some(("expansion", Some("diplomatic"))),
            false,
            None,
            &mut kept
        ),
        0
    );
    assert_eq!(kept.len(), 2);
    // `culture-seat-allies`: a Culture seat keeps them while the gene is
    // armed, and only a Culture seat.
    let mut kept = batch();
    assert_eq!(
        drop_off_lane_alliances(Some(("culture", Some("culture"))), true, None, &mut kept),
        0
    );
    assert_eq!(kept.len(), 2);
    let mut orders = batch();
    assert_eq!(
        drop_off_lane_alliances(
            Some(("conquest", Some("domination"))),
            true,
            None,
            &mut orders
        ),
        1,
        "the seat genes open the Culture and Science lanes only"
    );
    // `science-seat-allies`: a Science seat keeps them while armed.
    let mut kept = batch();
    assert_eq!(
        drop_off_lane_alliances(Some(("science", Some("science"))), true, None, &mut kept),
        0
    );
    assert_eq!(kept.len(), 2);
    for plan in [
        None,
        Some(("diplomacy", None)),
        Some(("conquest", Some("domination"))),
        Some(("culture", Some("culture"))),
    ] {
        let mut orders = batch();
        assert_eq!(
            drop_off_lane_alliances(plan, false, None, &mut orders),
            1,
            "{plan:?}"
        );
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].kind, "sell");
    }
}

/// `military-alliance-shield`: the shield's own `ALLIANCE_MILITARY` ask
/// crosses on every lane; a stock ask beside it, another kind to the same
/// rival, or a military ask to another rival does not.
#[test]
fn the_shields_military_alliance_crosses_on_every_lane() {
    let alliance = |subject: i64, verb: &str| Order {
        kind: "alliance",
        subject: Some(subject),
        verb: Some(verb.to_string()),
        pos: None,
    };
    for plan in [
        None,
        Some(("conquest", Some("domination"))),
        Some(("science", Some("science"))),
        Some(("culture", Some("culture"))),
    ] {
        let mut orders = vec![
            alliance(3, "ALLIANCE_MILITARY"),
            alliance(3, "ALLIANCE_RESEARCH"),
            alliance(4, "ALLIANCE_MILITARY"),
        ];
        assert_eq!(
            drop_off_lane_alliances(plan, false, Some((3, "ALLIANCE_MILITARY")), &mut orders),
            2,
            "{plan:?}"
        );
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].subject, Some(3));
        assert_eq!(orders[0].verb.as_deref(), Some("ALLIANCE_MILITARY"));
        // No shield ask this turn: every alliance order is held.
        let mut orders = vec![alliance(3, "ALLIANCE_MILITARY")];
        assert_eq!(drop_off_lane_alliances(plan, false, None, &mut orders), 1);
        // `military-alliance-shield-2`: its Economic ask crosses, and only it.
        let mut orders = vec![
            alliance(3, "ALLIANCE_ECONOMIC"),
            alliance(3, "ALLIANCE_MILITARY"),
        ];
        assert_eq!(
            drop_off_lane_alliances(plan, false, Some((3, "ALLIANCE_ECONOMIC")), &mut orders),
            1
        );
        assert_eq!(orders[0].verb.as_deref(), Some("ALLIANCE_ECONOMIC"));
    }
}

/// A friendship is verified by the host's own `GetDeclaredFriendshipTurn`
/// on the next frame; a refusal reads the same as a session never opened.
#[test]
fn a_friendship_is_verified_by_the_hosts_own_friendship_turn() {
    let rival = |since: Option<i64>| StateRival {
        player: 2,
        friendship_turn: since,
        ..StateRival::default()
    };
    let mut before = frame(80);
    before.rivals = vec![rival(Some(-1))];
    let friendship = order("friendship", Some(2), Some("DECLARE_FRIEND"), None);

    let mut after = frame(81);
    after.rivals = vec![rival(Some(80))];
    assert_eq!(check(&friendship, &before, &after, &[]), Verdict::Verified);

    // Refused, or a friendship older than the ask.
    for since in [-1, 50] {
        after.rivals = vec![rival(Some(since))];
        assert_eq!(
            check(&friendship, &before, &after, &[]),
            failed("not_friends")
        );
    }

    after.rivals = vec![rival(None)];
    assert_eq!(
        check(&friendship, &before, &after, &[]),
        Verdict::Unverifiable,
        "an older mod that does not export the turn cannot answer"
    );

    after.rivals = Vec::new();
    assert_eq!(
        check(&friendship, &before, &after, &[]),
        failed("player_unseen")
    );
}

#[test]
fn diplomacy_is_verified_by_the_answer_and_the_war_flag() {
    let rival = |at_war: bool| StateRival {
        player: 2,
        at_war,
        ..StateRival::default()
    };
    let mut before = frame(80);
    before.rivals = vec![rival(true)];
    let sale = order("sell", Some(2), Some("FAVOR=10"), Some((60, 0)));
    let peace = order("peace", Some(2), Some("MAKE_PEACE"), None);
    let war = order("war", Some(2), Some("DECLARE_FORMAL_WAR"), None);

    let mut after = frame(81);
    after.rivals = vec![rival(true)];
    assert_eq!(
        check(&sale, &before, &after, &[]),
        failed("no_deal_response")
    );
    let closed = event(r#"{"kind":"deal_closed","turn":80,"from":2,"gold":60}"#);
    assert_eq!(check(&sale, &before, &after, &[closed]), Verdict::Verified);
    let declined = event(r#"{"kind":"deal_declined","turn":80,"from":2}"#);
    assert_eq!(
        check(&sale, &before, &after, &[declined]),
        Verdict::Verified
    );
    let other_party = event(r#"{"kind":"deal_closed","turn":80,"from":5}"#);
    assert_eq!(
        check(&sale, &before, &after, &[other_party]),
        failed("no_deal_response")
    );

    assert_eq!(
        check(&peace, &before, &after, &[]),
        failed("no_peace_response")
    );
    let accepted = event(r#"{"kind":"peace_response","turn":80,"target":2,"accepted":true}"#);
    assert_eq!(
        check(&peace, &before, &after, std::slice::from_ref(&accepted)),
        failed("still_at_war")
    );
    let mut at_peace = frame(81);
    at_peace.rivals = vec![rival(false)];
    assert_eq!(
        check(&peace, &before, &at_peace, &[accepted]),
        Verdict::Verified
    );
    assert_eq!(check(&peace, &before, &at_peace, &[]), Verdict::Verified);
    let refused = event(r#"{"kind":"peace_response","turn":80,"target":2,"accepted":false}"#);
    assert_eq!(
        check(&peace, &before, &after, &[refused]),
        Verdict::Verified
    );

    let mut calm = frame(80);
    calm.rivals = vec![rival(false)];
    assert_eq!(check(&war, &calm, &after, &[]), Verdict::Verified);
    assert_eq!(check(&war, &calm, &at_peace, &[]), failed("not_at_war"));
}

#[test]
fn governors_are_verified_by_the_seat_they_hold() {
    let governor = |city: i64| StateGovernor {
        kind: "GOVERNOR_THE_DEFENDER".to_string(),
        city,
        ..StateGovernor::default()
    };
    let mut before = frame(25);
    before.governors = Some(vec![]);
    let appoint = order(
        "governor_appoint",
        Some(65_536),
        Some("GOVERNOR_THE_DEFENDER"),
        Some((0, -1)),
    );
    let assign = order(
        "governor_assign",
        Some(65_536),
        Some("GOVERNOR_THE_DEFENDER"),
        Some((0, -1)),
    );

    let mut seated = frame(26);
    seated.governors = Some(vec![governor(65_536)]);
    assert_eq!(check(&appoint, &before, &seated, &[]), Verdict::Verified);
    assert_eq!(check(&assign, &before, &seated, &[]), Verdict::Verified);

    // The asynchronous appointment that never persisted its assignment.
    let mut unseated = frame(26);
    unseated.governors = Some(vec![governor(-1)]);
    assert_eq!(check(&appoint, &before, &unseated, &[]), Verdict::Verified);
    assert_eq!(
        check(&assign, &before, &unseated, &[]),
        failed("not_assigned")
    );

    let mut none = frame(26);
    none.governors = Some(vec![]);
    assert_eq!(
        check(&appoint, &before, &none, &[]),
        failed("not_appointed")
    );

    // Neutralized by a spy: the post cannot stick, and no strike counts.
    let mut neutralized = frame(26);
    neutralized.governors = Some(vec![StateGovernor {
        neutralized_turns: 2,
        ..governor(-1)
    }]);
    assert_eq!(
        check(&assign, &before, &neutralized, &[]),
        Verdict::Unverifiable
    );
}

#[test]
fn recruits_consumed_before_next_turn_are_verified_from_later_frames() {
    for kind in ["gp_recruit", "gp_patronize", "gp_patronize_faith"] {
        for case in [
            "arrived",
            "old_unit",
            "old_frame",
            "other_turn",
            "wrong_class",
            "absent",
        ] {
            let mut before = frame(101);
            let after = frame(102);
            let mut observed = frame(101);
            observed.frame = 1;
            let person = StateUnit {
                id: 3080197,
                great_person: Some(StateGreatPerson {
                    class: Some("GREAT_PERSON_CLASS_SCIENTIST".to_string()),
                    ..StateGreatPerson::default()
                }),
                ..unit(3080197, "UNIT_GREAT_SCIENTIST", 44, 19)
            };
            observed.units = vec![person.clone()];
            match case {
                "old_unit" => before.units = vec![person],
                "old_frame" => observed.frame = 0,
                "other_turn" => observed.turn = 100,
                "wrong_class" => {
                    observed.units[0].great_person.as_mut().unwrap().class =
                        Some("GREAT_PERSON_CLASS_MERCHANT".to_string())
                }
                "absent" => observed.units.clear(),
                _ => {}
            }
            let pending = PendingOrders {
                turn: 101,
                frame: 0,
                before,
                orders: vec![order(
                    kind,
                    None,
                    Some("GREAT_PERSON_CLASS_SCIENTIST"),
                    None,
                )],
            };
            let checks = verify_orders_with_later_fortifications(
                &pending,
                &after,
                &no_tiles(),
                &[],
                &[],
                LaterOrderEvidence {
                    states: &[observed],
                    fortified: &std::collections::BTreeSet::new(),
                    moved: &std::collections::BTreeSet::new(),
                    policy_deck: false,
                },
            );
            assert_eq!(
                checks[0].verdict,
                if case == "arrived" {
                    Verdict::Verified
                } else {
                    failed("no_great_person")
                },
                "{kind}: {case}"
            );
        }
    }
}

#[test]
fn empire_choices_are_read_back_from_the_frame() {
    let before = frame(5);
    let mut after = frame(6);
    after.research = Some("TECH_WRITING".to_string());
    after.civics = vec!["CIVIC_CODE_OF_LAWS".to_string()];
    after.government = Some("GOVERNMENT_CHIEFDOM".to_string());
    after.pantheon = Some("BELIEF_GOD_OF_THE_SEA".to_string());
    after.dedications = Some(vec!["COMMEMORATION_SCIENTIFIC".to_string()]);
    after.policies = vec!["POLICY_DISCIPLINE".to_string(), "POLICY_SURVEY".to_string()];
    after.units = vec![StateUnit {
        id: 55,
        great_person: Some(StateGreatPerson {
            class: Some("GREAT_PERSON_CLASS_SCIENTIST".to_string()),
            ..StateGreatPerson::default()
        }),
        ..unit(55, "UNIT_GREAT_SCIENTIST", 1, 1)
    }];

    let cases = [
        (
            order("research", None, Some("TECH_WRITING"), None),
            Verdict::Verified,
        ),
        (
            order("research", None, Some("TECH_MINING"), None),
            failed("research=TECH_WRITING"),
        ),
        (
            order("civic", None, Some("CIVIC_CODE_OF_LAWS"), None),
            Verdict::Verified,
        ),
        (
            order("government", None, Some("GOVERNMENT_CHIEFDOM"), None),
            Verdict::Verified,
        ),
        (
            order("pantheon", None, Some("BELIEF_GOD_OF_THE_SEA"), None),
            Verdict::Verified,
        ),
        (
            order("dedication", None, Some("COMMEMORATION_SCIENTIFIC"), None),
            Verdict::Verified,
        ),
        (
            order(
                "dedication",
                None,
                Some("COMMEMORATION_INFRASTRUCTURE"),
                None,
            ),
            failed("not_dedicated"),
        ),
        (
            order(
                "policy_deck",
                None,
                Some("POLICY_DISCIPLINE,POLICY_SURVEY"),
                None,
            ),
            Verdict::Verified,
        ),
        (
            order(
                "policy_deck",
                None,
                Some("POLICY_DISCIPLINE,POLICY_URBAN_PLANNING"),
                None,
            ),
            failed("missing=POLICY_URBAN_PLANNING"),
        ),
        (
            order(
                "gp_recruit",
                None,
                Some("GREAT_PERSON_CLASS_SCIENTIST"),
                None,
            ),
            Verdict::Verified,
        ),
        (
            order(
                "gp_recruit",
                None,
                Some("GREAT_PERSON_CLASS_MERCHANT"),
                None,
            ),
            failed("no_great_person"),
        ),
        (
            order(
                "gp_patronize_faith",
                None,
                Some("GREAT_PERSON_CLASS_SCIENTIST"),
                None,
            ),
            Verdict::Verified,
        ),
    ];
    for (order, want) in cases {
        assert_eq!(
            check(&order, &before, &after, &[]),
            want,
            "{}",
            order.label()
        );
    }
}

#[test]
fn a_guru_heal_is_verified_by_its_own_charges() {
    let guru = |heals: i32| {
        let mut unit = unit(7, "UNIT_GURU", 3, 3);
        unit.spread_charges = Some(0);
        unit.heal_charges = Some(heals);
        unit
    };
    let heal = order("unit", Some(7), Some("RELIGIOUS_HEAL"), None);
    let mut before = frame(120);
    before.units = vec![guru(3)];
    let mut spent = frame(121);
    spent.units = vec![guru(2)];
    assert_eq!(
        verify_order(&heal, 120, &before, &spent, &no_tiles(), &[]),
        Verdict::Verified
    );
    let mut unspent = frame(121);
    unspent.units = vec![guru(3)];
    assert_eq!(
        verify_order(&heal, 120, &before, &unspent, &no_tiles(), &[]),
        Verdict::Failed("charge_not_spent".to_string())
    );
}

#[test]
fn unit_states_are_read_back_from_the_unit() {
    let mut before = frame(70);
    let mut builder = unit(3, "UNIT_BUILDER", 4, 4);
    builder.build_charges = Some(3);
    let mut trader = unit(5, "UNIT_TRADER", 6, 6);
    trader.moves = 2.0;
    before.units = vec![
        unit(1, "UNIT_WARRIOR", 2, 2),
        unit(2, "UNIT_WARRIOR", 3, 3),
        builder.clone(),
        trader,
    ];

    let mut after = frame(71);
    let mut fortified = unit(1, "UNIT_WARRIOR", 2, 2);
    fortified.fortify_turns = 1;
    let mut spent = builder.clone();
    spent.build_charges = Some(2);
    after.units = vec![
        fortified,
        unit(2, "UNIT_SWORDSMAN", 3, 3),
        spent,
        unit(5, "UNIT_TRADER", 6, 6),
    ];
    after.trade_routes = vec![StateTradeRoute {
        trader: 5,
        ..StateTradeRoute::default()
    }];

    let cases = [
        (
            order("unit", Some(1), Some("FORTIFY"), None),
            Verdict::Verified,
        ),
        (
            order("unit", Some(2), Some("FORTIFY"), None),
            failed("not_fortified"),
        ),
        (
            order("unit", Some(2), Some("UPGRADE"), None),
            Verdict::Verified,
        ),
        (
            order("unit", Some(1), Some("UPGRADE"), None),
            failed("same_unit_type"),
        ),
        (
            order("unit", Some(3), Some("IMPROVE:IMPROVEMENT_FARM"), None),
            Verdict::Verified,
        ),
        (
            order("unit", Some(5), Some("TRADE_ROUTE"), Some((9, 9))),
            Verdict::Verified,
        ),
        (
            order("unit", Some(1), Some("DELETE"), None),
            failed("still_exists"),
        ),
        (
            order("unit", Some(99), Some("DELETE"), None),
            Verdict::Verified,
        ),
        (
            order("unit", Some(2), Some("PROMOTE:PROMOTION_BATTLECRY"), None),
            failed("not_promoted"),
        ),
        (
            order("unit", Some(1), Some("SKIP_TURN"), None),
            Verdict::Unverifiable,
        ),
        (
            order("unit", Some(1), Some("SPY_TRAVEL_NEW_CITY"), Some((1, 1))),
            Verdict::Unverifiable,
        ),
    ];
    for (order, want) in cases {
        assert_eq!(
            check(&order, &before, &after, &[]),
            want,
            "{}",
            order.label()
        );
    }
}

/// The twin of `every_issued_order_kind_is_checked_or_declared_unverifiable`,
/// for the axis that had no guard.
///
/// ⚠⚠ The kind list has been discovered-not-listed since it was written;
/// the unit VERBS never were. `verify_unit_order` ends in
/// `_ => Verdict::Unverifiable`, so a verb added to `translate` without a
/// matching arm leaves the actuation contract in silence — no failure, no
/// notice, and no row in any rate, because `Unverifiable` orders are
/// excluded from both floors. That is the same shape as the SWAP verb
/// falling through `translate`'s own `_ => None` and being counted
/// `unit_action_untranslated` for as long as it did.
#[test]
fn every_issued_unit_verb_is_checked_or_declared_unverifiable() {
    // Discover, never list: every unit verb this file can put on the wire.
    //
    // ⚠ A byte WINDOW around `kind: "unit"` was tried first and is wrong.
    // It swept in a neighbouring `kind: "war"` literal's DECLARE, and
    // whether it reached a verb at all depended on how many bytes of
    // comment happened to sit between — it found DESIGNATE_PARK on one
    // revision of this file and missed EXCAVATE beside it. So the unit of
    // discovery is the `Order { … }` LITERAL, taken to its balanced brace,
    // which no edit above or below can shift.
    //
    // Two shapes carry a verb: the literal `verb: Some("FORTIFY"…)` inside
    // such a block, and a `let verb = if … { "CAPTURE" } else { "MOVE_TO" }`
    // above one. Both are read. Comments are stripped first — they quote
    // verbs, and they quote ordinary shouted words too ("ZERO captures"),
    // which a bare literal scan cannot tell apart.
    let source = include_str!("../civvis_orders.rs");
    let code: String = source
        .lines()
        .map(|line| match line.find("//") {
            Some(at) if line[..at].matches('"').count() % 2 == 0 => &line[..at],
            _ => line,
        })
        .collect::<Vec<_>>()
        .join("\n");
    fn upper_literals(block: &str, into: &mut std::collections::BTreeSet<String>) {
        for (at, _) in block.match_indices('"') {
            let rest = &block[at + 1..];
            let Some(close) = rest.find('"') else {
                continue;
            };
            let verb = &rest[..close];
            let shaped = (4..=32).contains(&verb.len())
                && verb
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b == b'_' || b.is_ascii_digit())
                && verb.bytes().any(|b| b.is_ascii_uppercase());
            if shaped {
                into.insert(verb.to_string());
            }
        }
    }
    /// The text of one brace-balanced block starting at `from`, never
    /// longer than `LITERAL_CAP`.
    ///
    /// ⚠ The cap is load-bearing. Brace counting over raw source is fooled
    /// by a brace inside a string — `format!("{kind}:{verb}")` is one — and
    /// an unbalanced count runs to the end of the file, sweeping in every
    /// upper-case word of every JSON test fixture on the way. An `Order`
    /// literal is a few hundred bytes, so a block that has not closed by
    /// here did not parse and is truncated rather than trusted.
    const LITERAL_CAP: usize = 600;
    fn balanced(code: &str, from: usize) -> &str {
        let mut depth = 0usize;
        for (offset, ch) in code[from..].char_indices() {
            if offset >= LITERAL_CAP {
                break;
            }
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    return &code[from..from + offset + 1];
                }
            }
        }
        let end = code[from..]
            .char_indices()
            .map(|(offset, ch)| offset + ch.len_utf8())
            .take_while(|offset| *offset <= LITERAL_CAP)
            .last()
            .unwrap_or(0);
        &code[from..from + end]
    }
    let mut verbs: std::collections::BTreeSet<String> = Default::default();
    for (index, _) in code.match_indices("Order {") {
        let block = balanced(&code, index + "Order ".len());
        if !block.contains("kind: \"unit\"") {
            continue;
        }
        upper_literals(block, &mut verbs);
        // A unit order whose verb is a variable took it from the nearest
        // `let verb =` above — and only that one, so a `let verb` feeding
        // a `kind: "city"` order (KEEP, RAZE, LIBERATE) is never swept in.
        if block.contains("verb: Some(verb") {
            // Bounded, for the same reason the block is: the assignment
            // that feeds this order sits directly above it, and an
            // unbounded `rfind` would sweep every literal back to the last
            // `let verb` anywhere in the file.
            let from = index.saturating_sub(LITERAL_CAP);
            if let Some(assign) = code[from..index].rfind("let verb = ") {
                upper_literals(&code[from + assign..index], &mut verbs);
            }
        }
    }
    // Firaxis type names travel in a unit order's `pos` and `subject`
    // payloads; they are not verbs.
    const NOT_A_VERB: &[&str] = &[
        "UNIT_",
        "DISTRICT_",
        "BUILDING_",
        "IMPROVEMENT_",
        "TECH_",
        "CIVIC_",
        "LOC_",
        "COMMEMORATION_",
        "GOVERNOR_",
        "POLICY_",
        "PROJECT_",
        "RESOURCE_",
        "TERRAIN_",
        "FEATURE_",
        "GREAT_PERSON",
        "RELIGION_",
        "BELIEF_",
        "PANTHEON_",
        "LEADER_",
        "CIVILIZATION_",
        "ERA_",
        "GOVERNMENT_",
        "PROMOTION_",
        "MODIFIER_",
        "YIELD_",
        "CIVVIS",
        "UNITOPERATION_",
        "UNITCOMMAND_",
        "OPERATION_",
        "WONDER_",
        "PLAYER_",
        "DIPLOACTION_",
        "TRAIT_",
        "AGENDA_",
        "ABILITY_",
        "UNITAI_",
        "GAMESPEED_",
    ];
    let issued: Vec<String> = verbs
        .into_iter()
        .filter(|verb| !NOT_A_VERB.iter().any(|prefix| verb.starts_with(prefix)))
        .filter(|verb| !verb.starts_with("SPY_"))
        .collect();
    assert!(
        issued.len() > 10,
        "the scan found only {issued:?}; it has stopped seeing the verbs"
    );
    let mut before = frame(1);
    before.units = vec![unit(1, "UNIT_WARRIOR", 1, 1)];
    let after = frame(2);
    for verb in issued {
        let verb = verb.as_str();
        let probe = order("unit", Some(1), Some(verb), Some((9, 9)));
        let verdict = check(&probe, &before, &after, &[]);
        assert!(
            (verdict != Verdict::Unverifiable) != unverifiable_unit_verb(verb),
            "unit verb {verb:?} is neither checked by `verify_unit_order` nor listed in \
                 UNVERIFIABLE_UNIT_VERBS with a reason (got {verdict:?})"
        );
    }
}

#[test]
fn military_formation_receipts_use_tier_instead_of_escort_count() {
    for (verb, tier) in [("FORM_CORPS", 1), ("FORM_ARMY", 2)] {
        let mut before = frame(153);
        before.units = vec![unit(5701651, "UNIT_CUIRASSIER", 40, 24)];
        before.units[0].formation = Some(tier - 1);
        let mut after = frame(154);
        after.units = before.units.clone();
        after.units[0].formation = Some(tier);
        after.units[0].formation_count = 1;
        let form = order("unit", Some(5701651), Some(verb), Some((0, 5832725)));
        assert_eq!(
            check(&form, &before, &after, &[]),
            Verdict::Verified,
            "{verb}"
        );
    }
}

#[test]
fn an_escort_stack_cannot_verify_a_military_formation() {
    let mut before = frame(153);
    before.units = vec![unit(7, "UNIT_CUIRASSIER", 40, 24)];
    let mut after = frame(154);
    after.units = before.units.clone();
    after.units[0].formation = Some(0);
    after.units[0].formation_count = 2;
    let form = order("unit", Some(7), Some("FORM_CORPS"), Some((0, 8)));
    assert!(matches!(
        check(&form, &before, &after, &[]),
        Verdict::Failed(_)
    ));
}

#[test]
fn military_formation_receipts_require_the_requested_known_tier() {
    let mut before = frame(153);
    before.units = vec![unit(7, "UNIT_CUIRASSIER", 40, 24)];
    let mut after = frame(154);
    after.units = before.units.clone();
    after.units[0].formation_count = 2;
    let army = order("unit", Some(7), Some("FORM_ARMY"), Some((0, 8)));
    after.units[0].formation = Some(1);
    assert_eq!(
        check(&army, &before, &after, &[]),
        failed("military_formation_tier_too_low")
    );
    for unknown in [None, Some(-1), Some(3)] {
        after.units[0].formation = unknown;
        assert_eq!(check(&army, &before, &after, &[]), Verdict::Unverifiable);
    }
    after.units.clear();
    assert_eq!(check(&army, &before, &after, &[]), failed("unit_gone"));
}

#[test]
fn escort_receipts_still_use_member_count_independently_of_military_tier() {
    let mut before = frame(153);
    before.units = vec![unit(7, "UNIT_CUIRASSIER", 40, 24)];
    let mut after = frame(154);
    after.units = before.units.clone();
    after.units[0].formation = Some(2);
    let enter = order("unit", Some(7), Some("ENTER_FORMATION"), Some((0, 8)));
    let exit = order("unit", Some(7), Some("EXIT_FORMATION"), None);
    after.units[0].formation_count = 1;
    assert_eq!(
        check(&enter, &before, &after, &[]),
        failed("not_in_formation")
    );
    assert_eq!(check(&exit, &before, &after, &[]), Verdict::Verified);
    after.units[0].formation_count = 2;
    assert_eq!(check(&enter, &before, &after, &[]), Verdict::Verified);
    assert_eq!(
        check(&exit, &before, &after, &[]),
        failed("still_in_formation")
    );
}

fn refused(order: &IssuedOrder, why: &str) -> OrderCheck {
    OrderCheck {
        order: order.clone(),
        verdict: failed(why),
    }
}

fn wire(issued: &IssuedOrder) -> Order {
    Order {
        kind: match issued.kind.as_str() {
            "unit" => "unit",
            other => Box::leak(other.to_string().into_boxed_str()),
        },
        subject: issued.subject,
        verb: issued.verb.clone(),
        pos: issued.pos,
    }
}

#[test]
fn an_order_is_withheld_only_after_the_same_refusal_three_times() {
    let issued = order("unit", Some(1), Some("MOVE_TO"), Some((4, 4)));
    let mut refusals = HostOrderRefusals::default();
    for turn in 1..ORDER_REFUSAL_STRIKES {
        refusals.observe(&[refused(&issued, "did_not_move")], turn);
        assert_eq!(
            refusals.withheld(&wire(&issued), turn + 1),
            None,
            "two strikes is not yet a standing refusal"
        );
    }
    refusals.observe(&[refused(&issued, "did_not_move")], ORDER_REFUSAL_STRIKES);
    assert_eq!(
        refusals.withheld(&wire(&issued), ORDER_REFUSAL_STRIKES + 1),
        Some("did_not_move")
    );
}

/// Replan frames re-send an order within a turn; their refusals are one
/// strike, so only three turns of the same answer strike it out.
#[test]
fn three_replans_in_one_turn_are_one_strike() {
    let issued = order(
        "governor_assign",
        Some(196_610),
        Some("GOVERNOR_THE_DEFENDER"),
        Some((0, -1)),
    );
    let mut refusals = HostOrderRefusals::default();
    let frames = vec![refused(&issued, "not_assigned"); 3];
    refusals.observe(&frames, 1);
    assert_eq!(
        refusals.withheld(&wire(&issued), 2),
        None,
        "one turn, one strike"
    );
    refusals.observe(&frames, 2);
    assert_eq!(
        refusals.withheld(&wire(&issued), 3),
        None,
        "two turns, two strikes"
    );
    refusals.observe(&frames, 3);
    assert_eq!(refusals.withheld(&wire(&issued), 4), Some("not_assigned"));
}

/// A genuinely transient block does not answer the same way three turns
/// running, which is what makes the reason comparison a sufficient test.
#[test]
fn a_changing_reason_restarts_the_count() {
    let issued = order("unit", Some(1), Some("MOVE_TO"), Some((4, 4)));
    let mut refusals = HostOrderRefusals::default();
    refusals.observe(&[refused(&issued, "did_not_move")], 1);
    refusals.observe(&[refused(&issued, "host_refused_impassable")], 2);
    refusals.observe(&[refused(&issued, "did_not_move")], 3);
    assert_eq!(refusals.withheld(&wire(&issued), 4), None);
}

#[test]
fn a_verified_order_clears_its_record_at_once() {
    let issued = order("unit", Some(1), Some("MOVE_TO"), Some((4, 4)));
    let mut refusals = HostOrderRefusals::default();
    for turn in 1..=ORDER_REFUSAL_STRIKES {
        refusals.observe(&[refused(&issued, "did_not_move")], turn);
    }
    assert!(refusals
        .withheld(&wire(&issued), ORDER_REFUSAL_STRIKES + 1)
        .is_some());
    refusals.observe(
        &[OrderCheck {
            order: issued.clone(),
            verdict: Verdict::Verified,
        }],
        ORDER_REFUSAL_STRIKES + 1,
    );
    assert_eq!(
        refusals.withheld(&wire(&issued), ORDER_REFUSAL_STRIKES + 2),
        None
    );
}

/// Re-planning is not punished: the destination is part of the identity.
#[test]
fn a_new_destination_is_a_new_order() {
    let issued = order("unit", Some(1), Some("MOVE_TO"), Some((4, 4)));
    let elsewhere = order("unit", Some(1), Some("MOVE_TO"), Some((5, 5)));
    let mut refusals = HostOrderRefusals::default();
    for turn in 1..=ORDER_REFUSAL_STRIKES {
        refusals.observe(&[refused(&issued, "did_not_move")], turn);
    }
    assert_eq!(
        refusals.withheld(&wire(&elsewhere), ORDER_REFUSAL_STRIKES + 1),
        None
    );
}

#[test]
fn an_unverifiable_answer_neither_strikes_nor_clears() {
    let issued = order("unit", Some(1), Some("SKIP_TURN"), None);
    let mut refusals = HostOrderRefusals::default();
    for turn in 1..=ORDER_REFUSAL_STRIKES {
        refusals.observe(
            &[OrderCheck {
                order: issued.clone(),
                verdict: Verdict::Unverifiable,
            }],
            turn,
        );
    }
    assert_eq!(
        refusals.withheld(&wire(&issued), ORDER_REFUSAL_STRIKES + 1),
        None
    );
}

#[test]
fn the_cooldown_expires_and_the_record_is_swept() {
    let issued = order("unit", Some(1), Some("MOVE_TO"), Some((4, 4)));
    let mut refusals = HostOrderRefusals::default();
    for turn in 1..=ORDER_REFUSAL_STRIKES {
        refusals.observe(&[refused(&issued, "did_not_move")], turn);
    }
    let expiry = ORDER_REFUSAL_STRIKES + ORDER_REFUSAL_COOLDOWN_TURNS;
    assert!(refusals.withheld(&wire(&issued), expiry - 1).is_some());
    assert_eq!(refusals.withheld(&wire(&issued), expiry), None);
    refusals.sweep(expiry);
    assert!(
        refusals.seen.is_empty(),
        "the table tracks the live problem"
    );
}

/// The withheld order is named in the reply rather than simply absent.
#[test]
fn host_war_permission_reopening_clears_only_the_resolved_declaration_refusal() {
    let war = order("war", Some(3), Some("DECLARE_FORMAL_WAR"), None);
    let other_war = order("war", Some(5), Some("DECLARE"), None);
    let step = order("unit", Some(3), Some("MOVE_TO"), Some((4, 4)));
    let mut state = StateSnapshot {
        turn: 98,
        rivals: vec![civvis::mirror::StateRival {
            player: 3,
            can_declare: Some(false),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut refusals = HostOrderRefusals::default();
    // Record the closed window through the real export filter, even with
    // no selected orders. Each refused declaration used the same target.
    withhold_refused_orders(Vec::new(), &state, &mut refusals);
    for back in (0..ORDER_REFUSAL_STRIKES).rev() {
        refusals.observe(
            &[
                refused(&war, "not_at_war"),
                refused(&other_war, "not_at_war"),
                refused(&step, "did_not_move"),
            ],
            98 - back,
        );
    }
    assert!(refusals.withheld(&wire(&war), 99).is_some());
    state.turn = 99;
    state.rivals[0].can_declare = Some(true);
    let (allowed, withheld) = withhold_refused_orders(
        vec![wire(&war), wire(&other_war), wire(&step)],
        &state,
        &mut refusals,
    );
    assert_eq!(allowed.len(), 1);
    assert_eq!(allowed[0].kind, "war");
    assert_eq!(allowed[0].subject, Some(3));
    assert_eq!(withheld.len(), 2);
    // A new failure while the host continues to report permission is
    // still a real problem: do not bypass the ordinary retry bound.
    for back in (0..ORDER_REFUSAL_STRIKES).rev() {
        refusals.observe(&[refused(&war, "not_at_war")], 99 - back);
    }
    state.turn = 100;
    let (allowed, withheld) = withhold_refused_orders(vec![wire(&war)], &state, &mut refusals);
    assert!(allowed.is_empty());
    assert_eq!(withheld.len(), 1);
    state.rivals[0].can_declare = None;
    let (allowed, _) = withhold_refused_orders(vec![wire(&war)], &state, &mut refusals);
    assert!(
        allowed.is_empty(),
        "unknown permission is not proof of reopening"
    );
}

#[test]
fn a_withheld_order_is_reported_by_name() {
    let issued = order("unit", Some(1), Some("MOVE_TO"), Some((4, 4)));
    let mut refusals = HostOrderRefusals::default();
    for turn in 1..=ORDER_REFUSAL_STRIKES {
        refusals.observe(&[refused(&issued, "did_not_move")], turn);
    }
    let other = Order {
        kind: "research",
        subject: None,
        verb: Some("TECH_POTTERY".to_string()),
        pos: None,
    };
    let (allowed, withheld) = withhold_refused_orders(
        vec![wire(&issued), other],
        &StateSnapshot {
            turn: ORDER_REFUSAL_STRIKES + 1,
            ..Default::default()
        },
        &mut refusals,
    );
    assert_eq!(allowed.len(), 1, "the unrelated order still goes");
    assert_eq!(allowed[0].kind, "research");
    assert_eq!(withheld, vec!["unit:MOVE_TO did_not_move".to_string()]);
}

#[test]
fn native_war_type_permissions_reopen_only_the_exact_refused_type() {
    let formal = order("war", Some(3), Some("DECLARE_FORMAL_WAR"), None);
    let surprise = order("war", Some(3), Some("DECLARE"), None);
    let other = order("war", Some(5), Some("DECLARE_FORMAL_WAR"), None);
    let mut state = StateSnapshot {
        turn: 53,
        rivals: vec![civvis::mirror::StateRival {
            player: 3,
            can_declare: Some(false),
            war_declarations: Some(vec![
                civvis::mirror::StateWarDeclaration {
                    statement: "DECLARE_FORMAL_WAR".into(),
                    allowed: Some(false),
                },
                civvis::mirror::StateWarDeclaration {
                    statement: "DECLARE_SURPRISE_WAR".into(),
                    allowed: Some(false),
                },
            ]),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut refusals = HostOrderRefusals::default();
    refusals.observe_war_permissions(&state);
    for back in (0..ORDER_REFUSAL_STRIKES).rev() {
        refusals.observe(
            &[
                refused(&formal, "cannot_declare"),
                refused(&surprise, "cannot_declare"),
                refused(&other, "cannot_declare"),
            ],
            53 - back,
        );
    }
    state.rivals[0].can_declare = Some(true);
    state.rivals[0].war_declarations.as_mut().unwrap()[1].allowed = Some(true);
    refusals.observe_war_permissions(&state);
    assert!(refusals.withheld(&wire(&formal), 54).is_some());
    assert!(refusals.withheld(&wire(&other), 54).is_some());
    assert!(
        refusals.withheld(&wire(&surprise), 54).is_none(),
        "DECLARE aliases only Surprise War"
    );
    state.rivals[0].war_declarations.as_mut().unwrap()[0].allowed = None;
    refusals.observe_war_permissions(&state);
    assert!(refusals.withheld(&wire(&formal), 54).is_some());
    state.rivals[0].war_declarations.as_mut().unwrap()[0].allowed = Some(true);
    refusals.observe_war_permissions(&state);
    assert!(refusals.withheld(&wire(&formal), 54).is_none());
    assert!(refusals.withheld(&wire(&other), 54).is_some());
    for back in (0..ORDER_REFUSAL_STRIKES).rev() {
        refusals.observe(&[refused(&formal, "cannot_declare")], 54 - back);
    }
    refusals.observe_war_permissions(&state);
    assert!(
        refusals.withheld(&wire(&formal), 54).is_some(),
        "unchanged permission cannot loop retries"
    );
}

#[test]
fn every_issued_order_kind_is_checked_or_declared_unverifiable() {
    // Discover, never list: every `kind: "..."` literal this file can emit.
    let source = include_str!("../civvis_orders.rs");
    let mut kinds: std::collections::BTreeSet<&str> = Default::default();
    // An `Order` literal is `kind: <expr>, subject: ...`; the expression is
    // a string literal or an `if` choosing between literals (`purchase` /
    // `purchase_faith`). Every quoted identifier in it that is not a
    // comparison operand (`== "faith"`) names a kind.
    for (index, _) in source.match_indices("kind: ") {
        let rest = &source[index + 6..];
        let Some(end) = rest.find("subject:") else {
            continue;
        };
        let expr = &rest[..end];
        if !(expr.starts_with('"') || expr.starts_with("if "))
            || expr.len() > 200
            || expr.contains(';')
        {
            continue;
        }
        for (at, _) in expr.match_indices('"') {
            let literal = &expr[at + 1..];
            let Some(close) = literal.find('"') else {
                continue;
            };
            let kind = &literal[..close];
            let operand = expr[..at].trim_end().ends_with("==");
            if !kind.is_empty()
                && kind.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
                && !operand
                && !expr[..at].trim_end().ends_with(kind)
            {
                kinds.insert(kind);
            }
        }
    }
    assert!(kinds.len() > 20, "the scan found only {kinds:?}");
    let mut before = frame(1);
    before.units = vec![unit(1, "UNIT_WARRIOR", 1, 1)];
    let mut after = frame(2);
    after.units = before.units.clone();
    for kind in kinds {
        if VERDICT_ROW_KINDS.contains(&kind) {
            continue;
        }
        let probe = if kind == "unit" {
            order("unit", Some(1), Some("MOVE_TO"), Some((1, 1)))
        } else if kind == "dedication" {
            order(
                kind,
                Some(1),
                Some("COMMEMORATION_SCIENTIFIC"),
                Some((1, 1)),
            )
        } else {
            order(kind, Some(1), Some(""), Some((1, 1)))
        };
        let verdict = if kind == "dedication" {
            let mut dedication_after = after.clone();
            dedication_after.dedications = Some(vec!["COMMEMORATION_SCIENTIFIC".to_string()]);
            check(&probe, &before, &dedication_after, &[])
        } else {
            check(&probe, &before, &after, &[])
        };
        assert!(
            (verdict != Verdict::Unverifiable) != unverifiable_kind(kind),
            "order kind {kind:?} is neither checked by `verify_order` nor listed in \
                 UNVERIFIABLE_ORDER_KINDS with a reason (got {verdict:?})"
        );
    }
    for (kind, why) in UNVERIFIABLE_ORDER_KINDS {
        assert!(!why.is_empty(), "{kind} needs a reason");
    }
    for (verb, why) in UNVERIFIABLE_UNIT_VERBS {
        assert!(!why.is_empty(), "{verb} needs a reason");
    }
}

/// KEEP is the city still ours with the host no longer waiting on the
/// decision; RAZE is the city gone or a citizen smaller next turn (or, on
/// a same-turn frame, the decision consumed); LIBERATE is the founder the
/// decision frame named holding the plot.
#[test]
fn a_city_disposition_is_verified_by_who_holds_the_city_next_frame() {
    let mut before = frame(30);
    before.cities = vec![StateCity {
        id: 7,
        name: "Antium".to_string(),
        x: 4,
        y: 4,
        pop: 4,
        // Taken from Firaxis player 3; founded by player 5.
        captured_from: Some(3),
        original_owner: Some(5),
        ..StateCity::default()
    }];
    let order = |verb: &str| IssuedOrder {
        kind: "city".to_string(),
        subject: Some(7),
        verb: Some(verb.to_string()),
        pos: Some((4, 4)),
    };
    let mut kept = frame(31);
    kept.cities = before.cities.clone();
    kept.cities[0].captured_from = None;
    let gone = frame(31);

    assert_eq!(
        check(&order("KEEP"), &before, &kept, &[]),
        Verdict::Verified
    );
    // Still pending: the host did not take the directive.
    assert_eq!(
        check(&order("KEEP"), &before, &before, &[]),
        failed("not_kept")
    );
    assert_eq!(
        check(&order("KEEP"), &before, &gone, &[]),
        failed("city_gone")
    );

    assert_eq!(
        check(&order("RAZE"), &before, &gone, &[]),
        Verdict::Verified
    );
    let mut burning = kept.clone();
    burning.cities[0].pop = 3;
    assert_eq!(
        check(&order("RAZE"), &before, &burning, &[]),
        Verdict::Verified
    );
    let mut same_turn = kept.clone();
    same_turn.turn = 30;
    same_turn.frame = 1;
    assert_eq!(
        check(&order("RAZE"), &before, &same_turn, &[]),
        Verdict::Verified
    );
    assert_eq!(
        check(&order("RAZE"), &before, &kept, &[]),
        failed("not_razed")
    );

    let foreign = |player: usize| {
        let mut after = frame(31);
        after.rivals.push(StateRival {
            player,
            cities: vec![StateCity {
                id: 7,
                x: 4,
                y: 4,
                ..StateCity::default()
            }],
            ..StateRival::default()
        });
        after
    };
    assert_eq!(
        check(&order("LIBERATE"), &before, &foreign(5), &[]),
        Verdict::Verified
    );
    assert_eq!(
        check(&order("LIBERATE"), &before, &foreign(3), &[]),
        failed("liberated_to_another")
    );
    assert_eq!(
        check(&order("LIBERATE"), &before, &kept, &[]),
        failed("not_liberated")
    );
    assert_eq!(
        check(&order("LIBERATE"), &before, &gone, &[]),
        failed("not_liberated")
    );
}

#[test]
fn the_verdict_rows_carry_the_turn_and_a_tally_the_mod_can_re_emit() {
    let checks = vec![
        OrderCheck {
            order: order("unit", Some(7), Some("MOVE_TO"), Some((3, 3))),
            verdict: Verdict::Verified,
        },
        OrderCheck {
            order: order("produce", Some(65_536), Some("BUILDING_MONUMENT"), None),
            verdict: failed("producing=UNIT_WARRIOR"),
        },
        OrderCheck {
            order: order("delegation", Some(2), Some("DIPLOMATIC_DELEGATION"), None),
            verdict: Verdict::Unverifiable,
        },
    ];
    let rows: Vec<String> = verdict_rows(42, &checks)
        .iter()
        .map(Order::to_json)
        .collect();
    assert_eq!(
        rows,
        vec![
            r#"{"kind":"order_verified","subject":7,"verb":"unit:MOVE_TO","x":42,"y":-1}"#,
            r#"{"kind":"order_failed","subject":65536,"verb":"produce:BUILDING_MONUMENT producing=UNIT_WARRIOR","x":42,"y":-1}"#,
            r#"{"kind":"turn_verified","subject":42,"verb":"issued=3 verified=1 failed=1 unverifiable=1","x":null,"y":null}"#,
        ]
    );
}

#[test]
fn the_reply_keeps_its_orders_and_note_when_verdicts_are_appended() {
    let reply = r#"{"turn":43,"orders":[{"kind":"research","subject":null,"verb":"TECH_WRITING","x":null,"y":null}],"note":"roster=1"}"#;
    let rows = verdict_rows(42, &[]);
    let joined = append_rows_to_reply(reply, &rows);
    let value: serde_json::Value = serde_json::from_str(&joined).unwrap();
    assert_eq!(value["note"], "roster=1");
    assert_eq!(value["turn"], 43);
    assert_eq!(value["orders"].as_array().unwrap().len(), 2);
    assert_eq!(value["orders"][1]["kind"], "turn_verified");
    // The verdict rows are not orders: the next turn's pending set excludes them.
    let issued = IssuedOrder::from_reply(&joined);
    assert_eq!(
        issued,
        vec![order("research", None, Some("TECH_WRITING"), None)]
    );
    assert_eq!(append_rows_to_reply(reply, &[]), reply);
}

#[test]
fn pending_orders_are_settled_by_the_next_turn_and_not_by_a_frame_of_the_same_turn() {
    let dir = std::env::temp_dir().join(format!("civvis-postcond-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let events = dir.join("events.jsonl");
    // The relay's own encoding, `": "` spacing included.
    std::fs::write(
        &events,
        "{\"kind\": \"turn\", \"turn\": 42, \"ctx\": \"agent\"}\n\
             {\"kind\": \"deal_closed\", \"turn\": 42, \"from\": 2, \"ctx\": \"agent\"}\n",
    )
    .unwrap();

    let mut before = frame(42);
    before.units = vec![unit(7, "UNIT_WARRIOR", 1, 1)];
    let mut pending = vec![PendingOrders {
        turn: 42,
        frame: 0,
        orders: vec![
            order("unit", Some(7), Some("MOVE_TO"), Some((3, 1))),
            order("sell", Some(2), Some("FAVOR=10"), Some((60, 0))),
        ],
        before,
    }];

    // A combat frame of the same turn answers nothing.
    let mut same_turn = frame(42);
    same_turn.frame = 1;
    assert!(settle_pending_orders(
        &mut pending,
        &events,
        &same_turn,
        &no_tiles(),
        &mut HostOrderRefusals::default(),
    )
    .is_empty());
    assert_eq!(pending.len(), 1);

    let mut next = frame(43);
    next.units = vec![unit(7, "UNIT_WARRIOR", 2, 1)];
    let rows: Vec<String> = settle_pending_orders(
        &mut pending,
        &events,
        &next,
        &no_tiles(),
        &mut HostOrderRefusals::default(),
    )
    .iter()
    .map(Order::to_json)
    .collect();
    assert!(pending.is_empty());
    assert_eq!(
        rows,
        vec![
            r#"{"kind":"order_verified","subject":7,"verb":"unit:MOVE_TO","x":42,"y":-1}"#,
            r#"{"kind":"order_verified","subject":2,"verb":"sell:FAVOR=10","x":42,"y":-1}"#,
            r#"{"kind":"turn_verified","subject":42,"verb":"issued=2 verified=2 failed=0 unverifiable=0","x":null,"y":null}"#,
        ]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn museum_swap_requires_both_native_inventory_slots_not_a_request() {
    let work = |kind: &str| civvis::mirror::StateGreatWork {
        kind: kind.into(),
        building: "BUILDING_MUSEUM_ART".into(),
        slot: 0,
        ..Default::default()
    };
    let mut before = frame(20);
    let mut source = city(10, 1, 1, None);
    source.great_works = Some(vec![work("ART_A")]);
    let mut destination = city(20, 2, 2, None);
    destination.great_works = Some(vec![work("ART_B")]);
    before.cities = vec![source, destination];
    let mut after = before.clone();
    after.turn = 21;
    let swap = order("museum_swap", Some(10), Some("ART_A"), Some((20, 0)));
    let request = serde_json::json!({"kind":"museum_swap_requested","requested":true,"turn":20});
    assert_eq!(
        check(&swap, &before, &after, &[request]),
        failed("museum_swap_not_observed")
    );
    after.cities[1].great_works = Some(vec![work("ART_A")]);
    assert_eq!(
        check(&swap, &before, &after, &[]),
        failed("museum_swap_not_observed")
    );
    after.cities[0].great_works = Some(vec![work("ART_B")]);
    assert_eq!(check(&swap, &before, &after, &[]), Verdict::Verified);
    after.cities[0].great_works = None;
    assert_eq!(
        check(&swap, &before, &after, &[]),
        failed("museum_inventory_unknown")
    );
    after.cities.pop();
    assert_eq!(
        check(&swap, &before, &after, &[]),
        failed("museum_not_ours")
    );
    let mut after = before.clone();
    after.cities[1].great_works = Some(vec![work("ART_A")]);
    after.cities[0].great_works = Some(vec![]);
    assert_eq!(
        check(&swap, &before, &after, &[]),
        failed("museum_swap_not_observed")
    );
    before.cities[1].great_works = Some(vec![]);
    assert_eq!(check(&swap, &before, &after, &[]), Verdict::Verified);
}

#[test]
fn writing_evacuation_requires_source_empty_and_exact_destination_inventory() {
    let writing = civvis::mirror::StateGreatWork {
        kind: "GREATWORK_CERVANTES_1".into(),
        object: "GREATWORKOBJECT_WRITING".into(),
        building: "BUILDING_AMPHITHEATER".into(),
        slot: 0,
        ..Default::default()
    };
    let mut before = frame(20);
    let mut source = city(10, 1, 1, None);
    source.great_works = Some(vec![writing.clone()]);
    let mut destination = city(20, 2, 2, None);
    destination.great_works = Some(vec![]);
    before.cities = vec![source, destination];
    let evacuation = order(
        "writing_evacuation",
        Some(10),
        Some("GREATWORK_CERVANTES_1"),
        Some((20, 0)),
    );
    let mut after = before.clone();
    after.turn = 21;
    let request =
        serde_json::json!({"kind":"writing_evacuation_requested","requested":true,"turn":20});
    assert_eq!(
        check(&evacuation, &before, &after, &[request]),
        failed("museum_swap_not_observed")
    );
    after.cities[1].great_works = Some(vec![writing]);
    assert_eq!(
        check(&evacuation, &before, &after, &[]),
        failed("museum_swap_not_observed")
    );
    after.cities[0].great_works = Some(vec![]);
    assert_eq!(check(&evacuation, &before, &after, &[]), Verdict::Verified);
    after.cities[0].great_works = None;
    assert_eq!(
        check(&evacuation, &before, &after, &[]),
        failed("museum_inventory_unknown")
    );
}
