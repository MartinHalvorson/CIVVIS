use super::*;

fn order() -> IssuedOrder {
    IssuedOrder {
        kind: "unit".into(),
        subject: Some(100),
        verb: Some("AIR_ATTACK".into()),
        pos: Some((5, 4)),
    }
}

fn anonymous_combat() -> serde_json::Value {
    // Native Bomber 8650780 at turns 183/186/189/194: anonymous negative-ID
    // defender, no defender position/health/damage. The old ledger invented
    // a killed district, but that is neither a harm nor a failure witness.
    serde_json::json!({
        "kind":"combat", "turn":30, "ours":true,
        "attacker":{"player":0,"id":100,"type":"unit"},
        "defender":{"player":-1,"id":-1,"type":"district","gone":true},
        "defender_killed":true
    })
}

fn verdict(order: &IssuedOrder, evidence: &[serde_json::Value]) -> Verdict {
    let (tiles, state) = tests::local_barbarian_defense_board();
    verify_unit_order(
        order,
        30,
        &state,
        &state,
        &tiles,
        evidence,
        LaterFrames::default(),
    )
}

#[test]
fn anonymous_air_combat_does_not_prove_failure_or_success() {
    assert_eq!(
        verdict(&order(), &[anonymous_combat()]),
        Verdict::Unverifiable
    );
}

#[test]
fn anonymous_air_effect_does_not_earn_a_refusal_cooldown() {
    let order = order();
    let check = OrderCheck {
        verdict: verdict(&order, &[anonymous_combat()]),
        order: order.clone(),
    };
    let mut refusals = HostOrderRefusals::default();
    for turn in 30..35 {
        refusals.observe(std::slice::from_ref(&check), turn);
    }
    assert!(refusals.seen.is_empty());
    assert_eq!(
        refusals.withheld(
            &Order {
                kind: "unit",
                subject: order.subject,
                verb: order.verb,
                pos: order.pos,
            },
            35
        ),
        None
    );
}

#[test]
fn explicit_air_refusal_remains_a_failure_despite_anonymous_combat() {
    let refusal = serde_json::json!({
        "kind":"range_attack_refused", "turn":30, "unit":100,
        "verb":"AIR_ATTACK", "x":5,"y":4,"why":"can_start=false,no_reasons [p4r]"
    });
    assert_eq!(
        verdict(&order(), &[anonymous_combat(), refusal]),
        Verdict::Failed("host_refused_strike".into())
    );
}

#[test]
fn anonymous_receipt_is_qualified_by_attacker_turn_and_ownership() {
    let receipt = anonymous_combat();
    let mut mismatches = Vec::new();
    let mut changed = receipt.clone();
    changed["attacker"]["id"] = 101.into();
    mismatches.push(changed);
    let mut changed = receipt.clone();
    changed["turn"] = 29.into();
    mismatches.push(changed);
    let mut changed = receipt.clone();
    changed["ours"] = false.into();
    mismatches.push(changed);
    let mut changed = receipt.clone();
    changed.as_object_mut().unwrap().remove("ours");
    mismatches.push(changed);
    let mut changed = receipt.clone();
    changed["kind"] = "strike".into();
    mismatches.push(changed);
    let mut changed = receipt.clone();
    changed["defender"]["player"] = 1.into();
    changed["defender"]["id"] = 42.into();
    mismatches.push(changed);
    let mut changed = receipt;
    changed.as_object_mut().unwrap().remove("defender");
    mismatches.push(changed);
    for evidence in mismatches {
        assert_eq!(
            verdict(&order(), &[evidence.clone()]),
            Verdict::Failed("target_unharmed".into()),
            "{evidence}"
        );
    }
}

#[test]
fn ordinary_strikes_are_not_reclassified_by_anonymous_air_receipts() {
    for verb in ["ATTACK", "RANGE_ATTACK"] {
        let order = IssuedOrder {
            verb: Some(verb.into()),
            ..order()
        };
        assert_eq!(
            verdict(&order, &[anonymous_combat()]),
            Verdict::Failed("target_unharmed".into())
        );
    }
}

#[test]
fn absent_receipt_and_positive_exact_combat_keep_their_existing_verdicts() {
    assert_eq!(
        verdict(&order(), &[]),
        Verdict::Failed("target_unharmed".into())
    );
    let exact = serde_json::json!({
        "kind":"combat", "turn":30, "ours":true,
        "attacker":{"id":100,"player":0},
        "defender":{"id":200,"player":63,"x":5,"y":4}
    });
    assert_eq!(
        verdict(&order(), &[exact, anonymous_combat()]),
        Verdict::Verified
    );
}
