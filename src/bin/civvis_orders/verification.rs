//! Order verification: did the host do what each issued order asked?
//!
//! Every check reads two consecutive frames, never a return code.
//! `verify_order_with_context` is the per-kind verifier, with the
//! `VerificationContext` it reads; `verify_orders_with_later_fortifications`
//! runs it over a turn, letting a later frame's fortification count as
//! evidence (`LaterOrderEvidence`); `verify_order` and `verify_orders` are
//! the test entry points. `verdict_rows` turns the checks into the verdict
//! rows written back on the reply (`append_rows_to_reply`),
//! `settle_pending_orders` closes out orders whose frames have arrived, and
//! `audit_orders` is the `--audit` mode that replays a run's events
//! against its orders.
//!
//! Moved verbatim out of `civvis_orders.rs` (2026-11-05), which re-exports
//! everything here, so every path is unchanged. Items the rest of the binary
//! uses are `pub(super)`; the code is otherwise unchanged.

use super::*;

pub(super) struct VerificationContext<'a> {
    later_great_person: bool,
    pub(super) same_turn_orders: &'a [IssuedOrder],
    later_fortified: bool,
    /// The decider moved this unit in a later frame of the same turn, so any
    /// FORTIFY it issued earlier was overridden by its own next decision.
    later_moved: bool,
    later_move_reached: bool,
    /// A later policy-deck replacement in this turn matches the final state,
    /// so this order was superseded by the decider's own re-plan.
    later_policy_deck: bool,
}

/// The next frame's verdict on one order issued on `turn`.
fn verify_order_with_context(
    order: &IssuedOrder,
    turn: u32,
    before: &civvis::mirror::StateSnapshot,
    after: &civvis::mirror::StateSnapshot,
    tiles: &civvis::mirror::Snapshot,
    evidence: &[serde_json::Value],
    context: VerificationContext<'_>,
) -> Verdict {
    if unverifiable_kind(&order.kind) {
        return Verdict::Unverifiable;
    }
    let verb = order.verb.as_deref().unwrap_or("");
    let failed = |why: String| Verdict::Failed(why);
    match order.kind.as_str() {
        "combat_policy" => {
            if evidence.iter().any(|event| {
                event["kind"] == "combat_policy_applied"
                    && event["turn"].as_u64() == Some(u64::from(turn))
                    && event["policy"].as_str() == Some(verb)
            }) {
                Verdict::Verified
            } else {
                failed("combat_policy_not_acknowledged".to_string())
            }
        }
        "unit" => verify_unit_order(
            order,
            turn,
            before,
            after,
            tiles,
            evidence,
            LaterFrames {
                fortified: context.later_fortified,
                moved: context.later_moved,
                move_reached: context.later_move_reached,
            },
        ),
        "museum_swap" | "writing_evacuation" => {
            let evacuating = order.kind == "writing_evacuation";
            let building = if evacuating {
                "BUILDING_AMPHITHEATER"
            } else {
                "BUILDING_MUSEUM_ART"
            };
            let slots = if evacuating { 2 } else { 3 };
            let Some(source_id) = order.subject else {
                return failed("museum_source_missing".to_string());
            };
            let Some((destination_id, destination_slot)) = order.pos else {
                return failed("museum_destination_missing".to_string());
            };
            if source_id == i64::from(destination_id)
                || !(0..slots).contains(&destination_slot)
                || verb.is_empty()
            {
                return failed("museum_order_invalid".to_string());
            }
            let cities = (
                own_city(before, source_id),
                own_city(before, i64::from(destination_id)),
                own_city(after, source_id),
                own_city(after, i64::from(destination_id)),
            );
            let (Some(source), Some(destination), Some(source_after), Some(destination_after)) =
                cities
            else {
                return failed("museum_not_ours".to_string());
            };
            let inventories = (
                source.great_works.as_deref(),
                destination.great_works.as_deref(),
                source_after.great_works.as_deref(),
                destination_after.great_works.as_deref(),
            );
            let (Some(source), Some(destination), Some(source_after), Some(destination_after)) =
                inventories
            else {
                return failed("museum_inventory_unknown".to_string());
            };
            let Some(original) = source
                .iter()
                .find(|work| work.kind == verb && work.building == building)
            else {
                return failed("museum_source_work_missing".to_string());
            };
            let other = destination
                .iter()
                .find(|work| work.building == building && work.slot == destination_slot)
                .map(|work| work.kind.as_str());
            if evacuating && (other.is_some() || original.object != "GREATWORKOBJECT_WRITING") {
                return failed("writing_evacuation_order_invalid".to_string());
            }
            let source_now = source_after
                .iter()
                .find(|work| work.building == building && work.slot == original.slot)
                .map(|work| work.kind.as_str());
            let destination_now = destination_after
                .iter()
                .find(|work| work.building == building && work.slot == destination_slot)
                .map(|work| work.kind.as_str());
            if source_now == other
                && destination_now == Some(verb)
                && !source_after.iter().any(|work| work.kind == verb)
            {
                Verdict::Verified
            } else {
                failed("museum_swap_not_observed".to_string())
            }
        }
        "produce" => {
            let Some(city) = order.subject.and_then(|id| own_city(after, id)) else {
                return failed("city_gone".to_string());
            };
            let unit_appeared = unit_appeared_at_city(before, after, city, verb);
            let purchase_conflict =
                production_unit_appearance_conflicts_with_purchase(order, context.same_turn_orders);
            if city.producing.as_deref() == Some(verb) {
                Verdict::Verified
            } else if building_repair_status_unknown(before, city, verb) {
                Verdict::Unverifiable
            } else if city_holds(city, verb) || (unit_appeared && !purchase_conflict) {
                Verdict::Verified
            } else if unit_appeared && purchase_conflict {
                failed("unit_appearance_ambiguous_purchase".to_string())
            } else {
                failed(format!(
                    "producing={}",
                    city.producing.as_deref().unwrap_or("nothing")
                ))
            }
        }
        "purchase" | "purchase_faith" => {
            let Some(city) = order.subject.and_then(|id| own_city(after, id)) else {
                return failed("city_gone".to_string());
            };
            let repair_unknown = building_repair_status_unknown(before, city, verb);
            let present = (city_holds(city, verb) && !repair_unknown)
                || unit_appeared_at_city(before, after, city, verb);
            let treasury_fell = if order.kind == "purchase_faith" {
                after.faith < before.faith
            } else {
                after.gold < before.gold
            };
            if present || treasury_fell {
                Verdict::Verified
            } else if repair_unknown {
                Verdict::Unverifiable
            } else {
                failed("not_bought".to_string())
            }
        }
        "buy_plot" => {
            let Some(pos) = order.pos else {
                return failed("no_plot".to_string());
            };
            match tiles.plot(pos) {
                Some(plot) if plot.o == after.seat.local_player => Verdict::Verified,
                Some(_) => failed("plot_not_owned".to_string()),
                None => failed("plot_unseen".to_string()),
            }
        }
        "envoy" => {
            let envoys = |state: &civvis::mirror::StateSnapshot| {
                state
                    .minors
                    .iter()
                    .find(|m| Some(m.player as i64) == order.subject)
                    .map(|m| m.envoys)
            };
            let rose = matches!((envoys(before), envoys(after)), (Some(b), Some(a)) if a > b);
            let spent =
                matches!((before.envoys_free, after.envoys_free), (Some(b), Some(a)) if a < b);
            if rose || spent {
                Verdict::Verified
            } else {
                failed("envoys_unchanged".to_string())
            }
        }
        "sell" | "buy" => {
            if deal_answered(evidence, order.subject) {
                Verdict::Verified
            } else {
                failed("no_deal_response".to_string())
            }
        }
        "peace" => {
            let war_now = order.subject.and_then(|p| at_war_with(after, p));
            let war_before = order.subject.and_then(|p| at_war_with(before, p));
            match peace_response(evidence, order.subject) {
                Some(true) if war_now == Some(true) => failed("still_at_war".to_string()),
                Some(_) => Verdict::Verified,
                None if war_before == Some(true) && war_now == Some(false) => Verdict::Verified,
                None => failed("no_peace_response".to_string()),
            }
        }
        "war" => match order.subject.and_then(|p| at_war_with(after, p)) {
            Some(true) => Verdict::Verified,
            Some(false) => failed("not_at_war".to_string()),
            None => failed("player_unseen".to_string()),
        },
        // The host's own record of OUR denouncement (`GetDenounceTurn`, the
        // turn it was made, -1 for none) crosses on every rival since #2590;
        // a session asked this turn lands on the next frame with that turn.
        "denounce" => match order.subject.and_then(|p| our_denounce_turn_of(after, p)) {
            Some(Some(since)) if since >= turn as i64 => Verdict::Verified,
            Some(Some(_)) => failed("not_denounced".to_string()),
            Some(None) => Verdict::Unverifiable,
            None => failed("player_unseen".to_string()),
        },
        // The host's `GetDeclaredFriendshipTurn` crosses on every rival
        // (#2590). The rival answers inside the session, so `not_friends`
        // covers a refusal as well as a session that never opened.
        "friendship" => match order.subject.and_then(|p| friendship_turn_of(after, p)) {
            Some(Some(since)) if since >= turn as i64 => Verdict::Verified,
            Some(Some(_)) => failed("not_friends".to_string()),
            Some(None) => Verdict::Unverifiable,
            None => failed("player_unseen".to_string()),
        },
        // The planner's alliance: the agent asks the declared friendship first
        // and the alliance once it stands (`CivvisAllianceStep`), so either
        // landing proves the order -- the host's diplomatic state reads ALLIED,
        // or a friendship declared this turn crossed. `not_allied` covers a
        // refusal as well as a session that never opened.
        "alliance" => {
            let rival = order
                .subject
                .and_then(|p| after.rivals.iter().find(|r| r.player as i64 == p));
            match rival {
                None => failed("player_unseen".to_string()),
                Some(r) if r.diplomatic_state.as_deref() == Some("DIPLO_STATE_ALLIED") => {
                    Verdict::Verified
                }
                Some(r) if r.friendship_turn.is_some_and(|since| since >= turn as i64) => {
                    Verdict::Verified
                }
                Some(_) => failed("not_allied".to_string()),
            }
        }
        "governor_appoint" | "governor_assign" => {
            let governors =
                |state: &civvis::mirror::StateSnapshot| state.governors.clone().unwrap_or_default();
            let seated = governors(after)
                .iter()
                .any(|g| g.kind == verb && Some(g.city) == order.subject);
            let appointed = order.kind == "governor_appoint"
                && governors(after).iter().any(|g| g.kind == verb)
                && !governors(before).iter().any(|g| g.kind == verb);
            // A neutralized Governor cannot take a post, so its refusal says
            // nothing about the order: no strike toward the refusal cooldown.
            let neutralized = governors(before)
                .iter()
                .chain(governors(after).iter())
                .any(|g| g.kind == verb && g.neutralized_turns > 0);
            if seated || appointed {
                Verdict::Verified
            } else if neutralized {
                Verdict::Unverifiable
            } else if order.kind == "governor_appoint" {
                failed("not_appointed".to_string())
            } else {
                failed("not_assigned".to_string())
            }
        }
        "governor_promote" => {
            let (governor, promotion) = verb.split_once(',').unwrap_or((verb, ""));
            let promoted = after.governors.as_ref().is_some_and(|gs| {
                gs.iter()
                    .any(|g| g.kind == governor && g.promotions.iter().any(|p| p == promotion))
            });
            if promoted {
                Verdict::Verified
            } else {
                failed("not_promoted".to_string())
            }
        }
        "research" => {
            if after.research.as_deref() == Some(verb) || after.techs.iter().any(|t| t == verb) {
                Verdict::Verified
            } else {
                failed(format!(
                    "research={}",
                    after.research.as_deref().unwrap_or("nothing")
                ))
            }
        }
        "civic" => {
            if after.civic.as_deref() == Some(verb) || after.civics.iter().any(|c| c == verb) {
                Verdict::Verified
            } else {
                failed(format!(
                    "civic={}",
                    after.civic.as_deref().unwrap_or("nothing")
                ))
            }
        }
        "policy_deck" => {
            let missing: Vec<&str> = verb
                .split(',')
                .filter(|p| !p.is_empty() && !after.policies.iter().any(|have| have == p))
                .collect();
            if missing.is_empty() {
                Verdict::Verified
            } else if context.later_policy_deck {
                failed("superseded_by_policy_deck".to_string())
            } else if let Some(reason) = policy_deck_deferred_reason(evidence, turn, order) {
                failed(reason)
            } else {
                failed(format!("missing={}", missing.join("+")))
            }
        }
        "government" => {
            if after.government.as_deref() == Some(verb) {
                Verdict::Verified
            } else {
                failed(format!(
                    "government={}",
                    after.government.as_deref().unwrap_or("nothing")
                ))
            }
        }
        "pantheon" => {
            if after.pantheon.as_deref() == Some(verb) {
                Verdict::Verified
            } else {
                failed("no_pantheon".to_string())
            }
        }
        "dedication" => match after.dedications.as_ref() {
            Some(dedications) if dedications.iter().any(|dedication| dedication == verb) => {
                Verdict::Verified
            }
            Some(_) => failed("not_dedicated".to_string()),
            None => Verdict::Unverifiable,
        },
        "religion" => {
            let beliefs_held = verb
                .split(',')
                .filter(|b| !b.is_empty())
                .all(|b| after.religion_beliefs.iter().any(|have| have == b));
            let founded = after.founded_religion.is_some() && before.founded_religion.is_none();
            if founded || beliefs_held {
                Verdict::Verified
            } else {
                failed("not_founded".to_string())
            }
        }
        "gp_recruit" | "gp_patronize" | "gp_patronize_faith" => {
            // A newly recruited person may activate and disappear before the
            // next turn. A later frame proves its arrival without relying on
            // an operation request or an activation claim.
            if great_person_arrived(verb, before, after) || context.later_great_person {
                Verdict::Verified
            } else {
                failed("no_great_person".to_string())
            }
        }
        "city_focus" => citizen_growth::verify(order, after),
        "city" => verify_city_disposition(order, before, after),
        // A silo's verb is `x:y:weapon`, its source tile and then the device.
        "wmd_strike" => {
            let weapon = verb.rsplit(':').next().unwrap_or("");
            if launch_landed(weapon, before, after, order.pos) {
                Verdict::Verified
            } else {
                failed("device_not_spent".to_string())
            }
        }
        "city_strike" | "encampment_strike" | "district_strike" => {
            let harmed = order.pos.is_some_and(|p| target_harmed(before, after, p));
            if harmed
                || order
                    .subject
                    .is_some_and(|id| combat_evidence(evidence, turn, id, order.pos))
            {
                Verdict::Verified
            } else {
                failed("target_unharmed".to_string())
            }
        }
        _ => Verdict::Unverifiable,
    }
}

/// Verify one order without sibling-order context. The single-order helper
/// remains useful for focused postcondition tests; live settlement supplies
/// the full turn context through `verify_orders` below.
#[cfg(test)]
pub(super) fn verify_order(
    order: &IssuedOrder,
    turn: u32,
    before: &civvis::mirror::StateSnapshot,
    after: &civvis::mirror::StateSnapshot,
    tiles: &civvis::mirror::Snapshot,
    evidence: &[serde_json::Value],
) -> Verdict {
    verify_order_with_context(
        order,
        turn,
        before,
        after,
        tiles,
        evidence,
        VerificationContext {
            later_great_person: false,
            same_turn_orders: &[],
            later_fortified: false,
            later_moved: false,
            later_move_reached: false,
            later_policy_deck: false,
        },
    )
}

/// Every order in `pending` against the frame that followed it.
#[cfg(test)]
pub(super) fn verify_orders(
    pending: &PendingOrders,
    after: &civvis::mirror::StateSnapshot,
    tiles: &civvis::mirror::Snapshot,
    evidence: &[serde_json::Value],
    same_turn_orders: &[IssuedOrder],
) -> Vec<OrderCheck> {
    verify_orders_with_later_fortifications(
        pending,
        after,
        tiles,
        evidence,
        same_turn_orders,
        LaterOrderEvidence {
            states: &[],
            fortified: &std::collections::BTreeSet::new(),
            moved: &std::collections::BTreeSet::new(),
            policy_deck: false,
        },
    )
}

/// Verify pending orders with positive evidence from later same-turn frames.
pub(super) struct LaterOrderEvidence<'a> {
    pub(super) states: &'a [civvis::mirror::StateSnapshot],
    pub(super) fortified: &'a std::collections::BTreeSet<i64>,
    pub(super) moved: &'a std::collections::BTreeSet<i64>,
    pub(super) policy_deck: bool,
}

pub(super) fn verify_orders_with_later_fortifications(
    pending: &PendingOrders,
    after: &civvis::mirror::StateSnapshot,
    tiles: &civvis::mirror::Snapshot,
    evidence: &[serde_json::Value],
    same_turn_orders: &[IssuedOrder],
    later: LaterOrderEvidence<'_>,
) -> Vec<OrderCheck> {
    pending
        .orders
        .iter()
        .map(|order| OrderCheck {
            verdict: verify_order_with_context(
                order,
                pending.turn,
                &pending.before,
                after,
                tiles,
                evidence,
                VerificationContext {
                    later_great_person: matches!(
                        order.kind.as_str(),
                        "gp_recruit" | "gp_patronize" | "gp_patronize_faith"
                    ) && later.states.iter().any(|state| {
                        state.turn == pending.turn
                            && state.frame > pending.frame
                            && great_person_arrived(
                                order.verb.as_deref().unwrap_or(""),
                                &pending.before,
                                state,
                            )
                    }),
                    same_turn_orders,
                    later_fortified: order
                        .subject
                        .is_some_and(|id| later.fortified.contains(&id)),
                    later_moved: order.subject.is_some_and(|id| later.moved.contains(&id)),
                    later_move_reached: host_move_postconditions::observed_endpoint(
                        pending,
                        order,
                        later.states,
                    ),
                    later_policy_deck: later.policy_deck,
                },
            ),
            order: order.clone(),
        })
        .collect()
}

/// The rows that carry one turn's verdicts to the mod: one per checked order
/// and one `turn_verified` tally. `x` carries the verified turn because the
/// order row has no spare column; the mod reads it back as `turn`.
pub(super) fn verdict_rows(turn: u32, checks: &[OrderCheck]) -> Vec<Order> {
    let mut rows = Vec::with_capacity(checks.len() + 1);
    let (mut verified, mut failed, mut unverifiable) = (0usize, 0usize, 0usize);
    for check in checks {
        match &check.verdict {
            Verdict::Verified => {
                verified += 1;
                rows.push(Order {
                    kind: "order_verified",
                    subject: check.order.subject,
                    verb: Some(check.order.label()),
                    pos: Some((turn as i32, -1)),
                });
            }
            Verdict::Failed(why) => {
                failed += 1;
                rows.push(Order {
                    kind: "order_failed",
                    subject: check.order.subject,
                    verb: Some(format!("{} {}", check.order.label(), why)),
                    pos: Some((turn as i32, -1)),
                });
            }
            Verdict::Unverifiable => unverifiable += 1,
        }
    }
    rows.push(Order {
        kind: "turn_verified",
        subject: Some(i64::from(turn)),
        verb: Some(format!(
            "issued={} verified={} failed={} unverifiable={}",
            checks.len(),
            verified,
            failed,
            unverifiable
        )),
        pos: None,
    });
    rows
}

/// Append rows to a reply's `orders` array without disturbing the rest.
pub(super) fn append_rows_to_reply(reply: &str, rows: &[Order]) -> String {
    if rows.is_empty() {
        return reply.to_string();
    }
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(reply) else {
        return reply.to_string();
    };
    let Some(orders) = value.get_mut("orders").and_then(|o| o.as_array_mut()) else {
        return reply.to_string();
    };
    for row in rows {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&row.to_json()) {
            orders.push(parsed);
        }
    }
    value.to_string()
}

/// The verdict rows for every pending frame the new frame answers, and the
/// pending list trimmed to what is still waiting.
pub(super) fn settle_pending_orders(
    pending: &mut Vec<PendingOrders>,
    events: &Path,
    after: &civvis::mirror::StateSnapshot,
    tiles: &civvis::mirror::Snapshot,
    refusals: &mut HostOrderRefusals,
) -> Vec<Order> {
    let answered: Vec<PendingOrders> = {
        let (done, waiting): (Vec<_>, Vec<_>) = std::mem::take(pending)
            .into_iter()
            .partition(|p| p.turn < after.turn);
        *pending = waiting;
        done
    };
    if answered.is_empty() {
        return Vec::new();
    }
    let turns: Vec<u32> = answered.iter().flat_map(|p| [p.turn, p.turn + 1]).collect();
    let (evidence, states) = ledger_evidence_and_states(events, &turns);
    let mut rows = Vec::new();
    let mut same_turn_orders: std::collections::BTreeMap<u32, Vec<IssuedOrder>> =
        Default::default();
    for frame in &answered {
        same_turn_orders
            .entry(frame.turn)
            .or_default()
            .extend(frame.orders.iter().cloned());
    }
    let mut by_turn: std::collections::BTreeMap<u32, Vec<OrderCheck>> = Default::default();
    for frame in &answered {
        let context = same_turn_orders
            .get(&frame.turn)
            .map_or(&[][..], Vec::as_slice);
        let later_fortified = later_fortified_units(&states, frame.turn, frame.frame);
        let later_moved = later_moved_units(&answered, frame.turn, frame.frame);
        let later_policy_replan =
            later_policy_deck_matches(&answered, frame.turn, frame.frame, after);
        by_turn
            .entry(frame.turn)
            .or_default()
            .extend(verify_orders_with_later_fortifications(
                frame,
                after,
                tiles,
                &evidence,
                context,
                LaterOrderEvidence {
                    states: &states,
                    fortified: &later_fortified,
                    moved: &later_moved,
                    policy_deck: later_policy_replan,
                },
            ));
    }
    for (turn, checks) in by_turn {
        // The contract's memory reads the checks here, where an `IssuedOrder`
        // is still paired with its own verdict. The verdict ROWS below carry
        // the turn in `pos` and cannot be joined back to the order that earned
        // them, which is why nothing downstream has ever been able to.
        refusals.observe(&checks, turn);
        rows.extend(verdict_rows(turn, &checks));
    }
    rows
}

/// Replay a recorded run's orders against its own frames.
///
/// `orders_path` holds the rows the brain wrote to `orders.sqlite`, one JSON
/// object per line (`turn`, `kind`, `subject`, `verb`, `x`, `y`, and optionally
/// `frame`); the frames
/// and the evidence come from the run's `events.jsonl`. Prints one JSON line:
/// what the checks verified, what failed and why, and the mod's own
/// `orders_reported` for the same turns, so the gap can be measured on a run
/// that was played before the checks existed. The board is not replayed, so
/// the plot-bound checks (`buy_plot`, `PILLAGE`, `REPAIR`) are counted apart
/// as `plot_bound` rather than as failures.
pub(super) fn audit_orders(events: &Path, orders_path: &Path) {
    let mut by_frame: std::collections::BTreeMap<(u32, u32), Vec<IssuedOrder>> = Default::default();
    for line in std::fs::read_to_string(orders_path)
        .unwrap_or_default()
        .lines()
    {
        let Ok(row) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(turn) = row.get("turn").and_then(|t| t.as_u64()) else {
            continue;
        };
        let frame = row
            .get("frame")
            .and_then(|frame| frame.as_u64())
            .unwrap_or(0);
        let kind = row
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or("")
            .to_string();
        if kind.is_empty() || VERDICT_ROW_KINDS.contains(&kind.as_str()) {
            continue;
        }
        let x = row.get("x").and_then(|v| v.as_i64());
        let y = row.get("y").and_then(|v| v.as_i64());
        by_frame
            .entry((turn as u32, frame as u32))
            .or_default()
            .push(IssuedOrder {
                kind,
                subject: row.get("subject").and_then(|v| v.as_i64()),
                verb: row.get("verb").and_then(|v| v.as_str()).map(str::to_string),
                pos: match (x, y) {
                    (Some(x), Some(y)) => Some((x as i32, y as i32)),
                    _ => None,
                },
            });
    }
    let raw = std::fs::read_to_string(events).unwrap_or_default();
    let mut frames: std::collections::BTreeMap<u32, civvis::mirror::StateSnapshot> =
        Default::default();
    let mut frame_states: std::collections::BTreeMap<(u32, u32), civvis::mirror::StateSnapshot> =
        Default::default();
    let mut all_states: Vec<civvis::mirror::StateSnapshot> = Vec::new();
    let mut reported: std::collections::BTreeMap<u32, (i64, i64)> = Default::default();
    let mut evidence: Vec<serde_json::Value> = Vec::new();
    let mut seat: Option<civvis::mirror::Seat> = None;
    for line in raw.lines() {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let kind = event.get("kind").and_then(|k| k.as_str()).unwrap_or("");
        if kind == "seat" {
            seat = serde_json::from_value(event).ok();
        } else if kind == "state" {
            // The opening board of each turn: the one the orders were decided on.
            match serde_json::from_value::<civvis::mirror::StateSnapshot>(event) {
                Ok(mut state) => {
                    if let Some(seat) = &seat {
                        state.seat = seat.clone();
                    }
                    frame_states
                        .entry((state.turn, state.frame))
                        .or_insert_with(|| state.clone());
                    frames.entry(state.turn).or_insert_with(|| state.clone());
                    all_states.push(state);
                }
                Err(why) => eprintln!("audit: state frame skipped: {why}"),
            }
        } else if kind == "turn" && event.get("ctx").and_then(|c| c.as_str()) == Some("agent") {
            if let (Some(turn), Some(seen)) = (
                event.get("turn").and_then(|t| t.as_u64()),
                event.get("orders_seen").and_then(|n| n.as_i64()),
            ) {
                let ok = event
                    .get("orders_reported")
                    .or_else(|| event.get("orders_applied"))
                    .and_then(|n| n.as_i64())
                    .unwrap_or(0);
                let entry = reported.entry(turn as u32).or_insert((0, 0));
                entry.0 += seen;
                entry.1 += ok;
            }
        } else if EVIDENCE_KINDS.contains(&kind) {
            evidence.push(event);
        }
    }
    let tiles = civvis::mirror::Snapshot::from_chunks(&[]);
    let (mut verified, mut failed, mut unverifiable, mut plot_bound, mut unframed) =
        (0, 0, 0, 0, 0);
    let (mut seen, mut ok) = (0i64, 0i64);
    let mut reasons: std::collections::BTreeMap<String, usize> = Default::default();
    let mut by_kind: std::collections::BTreeMap<String, [usize; 3]> = Default::default();
    let mut counted_reported_turns = std::collections::BTreeSet::new();
    let mut same_turn_orders: std::collections::BTreeMap<u32, Vec<IssuedOrder>> =
        Default::default();
    for ((turn, _frame), orders) in &by_frame {
        same_turn_orders
            .entry(*turn)
            .or_default()
            .extend(orders.iter().cloned());
    }
    // Precomputed because the loop below consumes `by_frame`: for each frame,
    // the units the decider moved in a LATER frame of the same turn. See
    // `later_moved_units` for why a FORTIFY they overrode is not a bridge
    // failure.
    let later_moved_by_frame: std::collections::BTreeMap<
        (u32, u32),
        std::collections::BTreeSet<i64>,
    > = by_frame
        .keys()
        .map(|&(turn, frame)| {
            let moved = by_frame
                .iter()
                .filter(|((t, f), _)| *t == turn && *f > frame)
                .flat_map(|(_, orders)| orders.iter())
                .filter(|order| order.verb.as_deref() == Some("MOVE_TO"))
                .filter_map(|order| order.subject)
                .collect();
            ((turn, frame), moved)
        })
        .collect();
    // A later policy deck is only useful as supersession evidence when the
    // final answering frame contains its requested cards. This mirrors the
    // live settlement path while keeping the replay loop free to consume
    // `by_frame` below.
    let later_policy_deck_by_frame: std::collections::BTreeMap<(u32, u32), bool> = by_frame
        .keys()
        .map(|&(turn, frame)| {
            let matched = frames.get(&(turn + 1)).is_some_and(|after| {
                by_frame
                    .iter()
                    .filter(|((t, f), _)| *t == turn && *f > frame)
                    .flat_map(|(_, orders)| orders.iter())
                    .any(|order| policy_deck_cards_present(order, after))
            });
            ((turn, frame), matched)
        })
        .collect();
    for ((turn, frame), orders) in by_frame {
        let Some(before) = frame_states
            .get(&(turn, frame))
            .or_else(|| frames.get(&turn))
        else {
            unframed += orders.len();
            continue;
        };
        let Some(after) = frames.get(&(turn + 1)) else {
            unframed += orders.len();
            continue;
        };
        if counted_reported_turns.insert(turn) {
            if let Some((s, a)) = reported.get(&turn) {
                seen += s;
                ok += a;
            }
        }
        let window: Vec<serde_json::Value> = evidence
            .iter()
            .filter(|e| {
                e.get("turn")
                    .and_then(|t| t.as_u64())
                    .is_some_and(|t| t == u64::from(turn) || t == u64::from(turn) + 1)
            })
            .cloned()
            .collect();
        let pending = PendingOrders {
            turn,
            frame,
            orders,
            before: before.clone(),
        };
        let context = same_turn_orders.get(&turn).map_or(&[][..], Vec::as_slice);
        let later_fortified = later_fortified_units(&all_states, turn, frame);
        let later_moved = later_moved_by_frame
            .get(&(turn, frame))
            .cloned()
            .unwrap_or_default();
        let later_policy_replan = later_policy_deck_by_frame
            .get(&(turn, frame))
            .copied()
            .unwrap_or(false);
        for check in verify_orders_with_later_fortifications(
            &pending,
            after,
            &tiles,
            &window,
            context,
            LaterOrderEvidence {
                states: &all_states,
                fortified: &later_fortified,
                moved: &later_moved,
                policy_deck: later_policy_replan,
            },
        ) {
            let label = match check.order.kind.as_str() {
                "unit" => check
                    .order
                    .label()
                    .split(':')
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(":"),
                kind => kind.to_string(),
            };
            let slot = by_kind.entry(label).or_insert([0, 0, 0]);
            match check.verdict {
                Verdict::Verified => {
                    verified += 1;
                    slot[0] += 1;
                }
                Verdict::Failed(why)
                    if why == "plot_unseen" || why == "not_pillaged" || why == "not_repaired" =>
                {
                    plot_bound += 1;
                }
                Verdict::Failed(why) => {
                    failed += 1;
                    slot[1] += 1;
                    let key = why.split('=').next().unwrap_or(&why).to_string();
                    *reasons.entry(key).or_default() += 1;
                }
                Verdict::Unverifiable => {
                    unverifiable += 1;
                    slot[2] += 1;
                }
            }
        }
    }
    println!(
        "{}",
        serde_json::json!({
            "issued": verified + failed + unverifiable + plot_bound + unframed,
            "verified": verified,
            "failed": failed,
            "unverifiable": unverifiable,
            "plot_bound": plot_bound,
            "unframed": unframed,
            "reported_seen": seen,
            "reported_ok": ok,
            "reasons": reasons,
            "by_kind": by_kind,
        })
    );
}
