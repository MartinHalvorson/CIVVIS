//! What the host refused, so the order channel stops asking.
//!
//! Move refusals: a unit's move the host turned down, the attempt and the
//! frontier probe behind it, the production lease a fortifying queue
//! holds, and `HostMoveRefusals`, which routes later moves around a refused
//! step. Order refusals: the strike count and cooldown before an order is
//! withheld (`ORDER_REFUSAL_STRIKES`, `ORDER_REFUSAL_COOLDOWN_TURNS`), the
//! identity an order is remembered by, `HostOrderRefusals` with its
//! records, and the two filters the turn applies (`withhold_refused_orders`,
//! `defer_host_peace_retries`).
//!
//! Moved verbatim out of `civvis_orders.rs` (2026-11-05), which re-exports
//! everything here, so every path is unchanged. Items the rest of the binary
//! uses are `pub(super)`; the code is otherwise unchanged.

use super::*;

/// Plots the host will not walk a unit onto, learned from moves that went
/// nowhere.
///
/// ★★★★ AN ACCEPTED MOVE THAT NEVER MOVES IS THE MIRROR'S BLIND SPOT. The
/// mirror marks unrevealed plots within the frontier depth as traversable so
/// exploration and expansion can aim outward, and Civilization VI accepts a
/// `MOVE_TO` toward one without a `move_refused` — then, if the plot is really
/// ice, ocean or a mountain behind a hill, walks the unit nowhere. Run
/// civvis-20260816T093036Z: the only Scout stood on (12,12) from t13 to t49
/// ordered `MOVE_TO (9,11)` on 28 consecutive turns; 193 plots revealed by t50.
/// #1735 retires an exploration GOAL the unit stands still for, but the goal
/// varied while the coalesced first step through (9,11) did not, so nothing
/// fired. The host receives a coalesced destination, but CIVVIS still knows
/// each local step that led there. A unit that never starts a long route first
/// retries that exact speculative step; only if Firaxis names that probe an
/// impossible path (or an older host supplies no reason) does it become
/// unwalkable. `cannot_start`, a zone of control, a friendly stack, and an
/// exhausted move allowance are all facts about the moment, not the terrain.
/// The mirror then stops assuming only proved-dead ground is traversable, and
/// every path — exploration, settlers, the army — routes around it. Revealed
/// terrain is never overridden: once the plot is seen, its real terrain governs.
#[derive(Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct HostUnitOrderKey {
    pub(super) verb: String,
    pub(super) pos: Option<(i32, i32)>,
}

#[derive(Default)]
pub(super) struct HostMoveRefusals {
    /// The last `MOVE_TO` sent per host unit: where it stood, the long
    /// destination sent to Firaxis, the first unknown local step, and the turn.
    pub(super) sent: std::collections::BTreeMap<i64, HostMoveAttempt>,
    /// A first unknown hop from a long route the host did not begin. It must
    /// become the next exact `MOVE_TO` before terrain is declared dead.
    pub(super) pending_probes: std::collections::BTreeMap<i64, HostFrontierProbe>,
    /// Speculative plots proved unwalkable, in host offset coordinates.
    pub(super) dead: std::collections::BTreeSet<(i32, i32)>,
    /// Unit orders already emitted during the current turn, keyed by host unit.
    /// Same-turn frames can still carry the previous frame's positions, so an
    /// identical movement command is a replay of a request the host has already
    /// consumed rather than a new route decision.
    pub(super) same_turn_orders:
        std::collections::BTreeMap<i64, std::collections::BTreeSet<HostUnitOrderKey>>,
    /// The diplomacy arm also has one working deal per rival and keeps an ask
    /// pending or cooling down across same-turn frames. An identical
    /// `sell`/`buy` in a later frame therefore cannot be a new deal request.
    same_turn_deal_orders: std::collections::BTreeSet<i64>,
    /// A failed long route gets individually planned steps on the next turn.
    /// This does not label known terrain impassable after a transient failure.
    pub(super) local_retry: std::collections::BTreeMap<i64, (u32, (i32, i32))>,
    /// The first city production request emitted during the current turn,
    /// keyed by Civilization VI city id. Production is a replacement operation:
    /// a later same-turn request competes with the queue already sent to the
    /// host, even when it asks for a different item. Retain its item and frame
    /// so only a refused queue or a fresh observed Wall handoff can replace it.
    pub(super) same_turn_production_orders: std::collections::BTreeMap<i64, HostProductionLease>,
    pub(super) same_turn: Option<u32>,
}

#[derive(Clone, Copy)]
pub(super) struct HostMoveAttempt {
    pub(super) from: (i32, i32),
    pub(super) destination: (i32, i32),
    /// The first unrevealed step CIVVIS simulated before coalescing the whole
    /// path into `destination`, or that destination for a fully revealed route.
    /// It is the actionable feedback target when the host never starts the
    /// route at all.
    frontier_step: (i32, i32),
    pub(super) turn: u32,
}

#[derive(Clone, Copy)]
pub(super) struct HostFrontierProbe {
    pub(super) from: (i32, i32),
    pub(super) target: (i32, i32),
}

pub(super) struct HostProductionLease {
    pub(super) verb: String,
    pub(super) frame: u32,
}

impl HostMoveRefusals {
    /// Drop stale unit, deal, and city-production replays from one turn.
    ///
    /// The host may apply a queued move before the next exported frame catches
    /// up. CIVVIS then plans from the old tile and emits the same `MOVE_TO`
    /// again; the host records that as `from == destination` / `move_refused`.
    /// Waiting for the next turn is the only useful retry because this frame has
    /// no newer authoritative position. FORTIFY, civilian CAPTURE and SWAP have
    /// the same stale-state failure mode (a swap re-derived from the old frame
    /// is aimed at the tile the unit has already taken). The diplomacy arm has the same shape:
    /// its pending/cooldown guards mean a repeated deal order can only be
    /// refused again. City production is also a single host queue: once the
    /// first `produce` request has left, a later same-turn request can only
    /// replace or be refused against that queue. A fresh host readback may
    /// hand a non-fortified queue to Walls once, without reopening routine
    /// production churn. Attack orders are deliberately
    /// excluded: a fresh combat frame may legitimately finish the same target.
    ///
    /// `ours` is repaired when a later production choice is dropped. The map is
    /// consulted before the next frame's speculative board is built; leaving the
    /// discarded choice there would make the host's still-running queue look
    /// foreign and cause another unnecessary replan.
    pub(super) fn suppress_same_turn_replays(
        &mut self,
        orders: &mut Vec<Order>,
        state: &civvis::mirror::StateSnapshot,
        ours: &mut std::collections::BTreeMap<i64, String>,
    ) -> usize {
        if self.same_turn != Some(state.turn) {
            self.same_turn = Some(state.turn);
            self.same_turn_orders.clear();
            self.same_turn_deal_orders.clear();
            self.same_turn_production_orders.clear();
        }
        // A planner may revise a queue several times before this batch is
        // sent (for example, restoring a military build after an economic
        // pass). Only its final choice should acquire the host-turn lease.
        // Later calls keep the sent choice except for a refused queue or
        // an observed non-fortified queue handing off to Walls.
        let final_production: std::collections::BTreeMap<_, _> = orders
            .iter()
            .enumerate()
            .filter(|(_, order)| order.kind == "produce" && order.verb.is_some())
            .filter_map(|(index, order)| order.subject.map(|city| (city, index)))
            .collect();
        let before = orders.len();
        let mut index = 0;
        orders.retain(|order| {
            let keep = order.kind != "produce"
                || order.verb.is_none()
                || order
                    .subject
                    .is_none_or(|city| final_production[&city] == index);
            index += 1;
            keep
        });
        let frame = state.frame;
        let mut dropped = before - orders.len();
        orders.retain(|order| {
            if order.kind == "produce" {
                if let (Some(city), Some(verb)) = (order.subject, order.verb.as_deref()) {
                    let first = match self.same_turn_production_orders.entry(city) {
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            let selected = verb.to_string();
                            entry.insert(HostProductionLease {
                                verb: selected.clone(),
                                frame: state.frame,
                            });
                            // Keep the helper self-contained as well as safe
                            // for the normal translation path, which records
                            // this before reaching the filter.
                            ours.insert(city, selected);
                            None
                        }
                        std::collections::btree_map::Entry::Occupied(mut entry) => {
                            let refused = state.refused_production.get(&city);
                            if entry.get().verb != verb
                                && (refused.is_some_and(|items| {
                                    items.contains(&entry.get().verb) && !items.contains(verb)
                                }) || same_turn_wall_handoff(state, city, entry.get(), verb))
                            {
                                entry.insert(HostProductionLease {
                                    verb: verb.to_string(),
                                    frame: state.frame,
                                });
                                ours.insert(city, verb.to_string());
                                None
                            } else {
                                Some(entry.get().verb.clone())
                            }
                        }
                    };
                    if let Some(first) = first {
                        // A production order replaces the city's queue. The
                        // retained request remains authoritative for this host
                        // turn, including when the replan is still frame 0.
                        ours.insert(city, first);
                        dropped += 1;
                        return false;
                    }
                }
            }

            let Some(unit) = order.subject else {
                return true;
            };
            let Some(verb) = order.verb.as_deref() else {
                return true;
            };
            if order.kind == "unit"
                && matches!(
                    verb,
                    "MOVE_TO" | "CAPTURE" | "FORTIFY" | "SWAP" | "REBASE" | "PATROL"
                )
            {
                let key = HostUnitOrderKey {
                    verb: verb.to_string(),
                    pos: order.pos,
                };
                let seen = self.same_turn_orders.entry(unit).or_default();
                let duplicate = !seen.insert(key);
                if frame > 0 && duplicate {
                    dropped += 1;
                    return false;
                }
            }

            if matches!(order.kind, "sell" | "buy") {
                // Buy and sell share the host's single working deal per rival.
                // Different items in the same frame still compete for it.
                if !self.same_turn_deal_orders.insert(unit) {
                    dropped += 1;
                    return false;
                }
            }
            true
        });
        dropped
    }

    /// Whether the postcondition verifier established that a failed exact
    /// frontier probe is terrain rather than a transient operation failure.
    ///
    /// `settle_pending_orders` reads the host's `move_noop` record before this
    /// method runs. The resulting verdict row is a compact, attributed answer
    /// for the same unit and turn: use it instead of treating every unchanged
    /// position as a mountain. Older hosts did not emit that evidence, so they
    /// keep the existing positional fallback that prevents an explorer from
    /// retrying an unseen impasse forever.
    pub(super) fn exact_probe_proves_dead(verdicts: &[Order], turn: u32, unit: i64) -> bool {
        let reason = verdicts.iter().find_map(|row| {
            (row.kind == "order_failed"
                && row.subject == Some(unit)
                && row.pos == Some((turn as i32, -1)))
            .then_some(row.verb.as_deref())
            .flatten()
            .and_then(|verb| verb.strip_prefix("unit:MOVE_TO "))
        });
        match reason {
            // `GetMoveToPathEx` had no route after the host accepted an exact
            // probe, or the host itself named the destination impassable.
            Some("host_noop_no_path" | "host_refused_impassable") => true,
            // A live run contained 80 `cannot_start` no-ops by turn 207. None
            // proves that the requested fog tile is terrain nobody can enter;
            // the same is true of a temporary ZoC, stack, hostile, or movement
            // allowance failure. Keep those tiles available to the next board.
            Some(_) => false,
            // The original mod had no move-noop evidence. Preserve the
            // position-only fallback for that compatibility path.
            None => true,
        }
    }

    /// Compare this turn's positions with last turn's orders.
    pub(super) fn observe(&mut self, state: &civvis::mirror::StateSnapshot, verdicts: &[Order]) {
        self.local_retry.retain(|_, (turn, _)| *turn >= state.turn);
        let sent = std::mem::take(&mut self.sent);
        for (unit, attempt) in sent {
            // The ladder asks several times per turn; a same-turn frame cannot
            // judge last frame's move yet, so keep it for the turn that can.
            if state.turn <= attempt.turn {
                self.sent.insert(unit, attempt);
                continue;
            }
            if state.turn != attempt.turn.saturating_add(1) {
                self.pending_probes.remove(&unit);
                continue;
            }
            let Some(now) = state.units.iter().find(|u| u.id == unit) else {
                self.pending_probes.remove(&unit);
                continue;
            };
            if (now.x, now.y) == attempt.from
                && attempt.from != attempt.destination
                && verdicts.iter().any(|row| {
                    row.kind == "order_failed"
                        && row.subject == Some(unit)
                        && row.pos == Some((attempt.turn as i32, -1))
                        && row.verb.as_deref() == Some("unit:MOVE_TO host_noop_no_path")
                })
            {
                self.local_retry.insert(unit, (state.turn, attempt.from));
            }
            if (now.x, now.y) != attempt.from || attempt.from == attempt.destination {
                self.pending_probes.remove(&unit);
            } else if attempt.destination == attempt.frontier_step {
                // This was already the exact local step, rather than a long
                // route containing it. A named no-op distinguishes an
                // impassable route from a temporarily unavailable operation.
                self.pending_probes.remove(&unit);
                if Self::exact_probe_proves_dead(verdicts, attempt.turn, unit) {
                    self.dead.insert(attempt.frontier_step);
                }
            } else {
                self.pending_probes.insert(
                    unit,
                    HostFrontierProbe {
                        from: attempt.from,
                        target: attempt.frontier_step,
                    },
                );
            }
        }
    }

    /// Replace a repeated long move with the one unknown hop the host just
    /// failed to begin. This keeps healthy paths coalesced at full speed and
    /// gathers exact terrain evidence only on a failed route.
    pub(super) fn cap_pending_frontier_moves(
        &mut self,
        orders: &mut [Order],
        state: &civvis::mirror::StateSnapshot,
        first_unknown_steps: &std::collections::BTreeMap<i64, (i32, i32)>,
    ) -> usize {
        let pending = std::mem::take(&mut self.pending_probes);
        let mut capped = 0;
        for (unit, probe) in pending {
            let still_at_from = state
                .units
                .iter()
                .find(|candidate| candidate.id == unit)
                .is_some_and(|candidate| (candidate.x, candidate.y) == probe.from);
            if !still_at_from {
                continue;
            }
            // A replan that chose a different opening is already escaping the
            // failed route. Do not overwrite its tactical judgment with stale
            // exploration feedback.
            if first_unknown_steps.get(&unit) != Some(&probe.target) {
                continue;
            }
            if let Some(order) = orders.iter_mut().find(|order| {
                order.kind == "unit"
                    && order.subject == Some(unit)
                    && order.verb.as_deref() == Some("MOVE_TO")
                    && order.pos.is_some()
            }) {
                if order.pos != Some(probe.target) {
                    order.pos = Some(probe.target);
                    capped += 1;
                }
                self.pending_probes.insert(unit, probe);
            }
        }
        capped
    }

    /// Remember where each `MOVE_TO` in `orders` sends which host unit from.
    ///
    /// `frontier_steps` comes from the uncoalesced action log. It identifies the
    /// first unknown hop along an otherwise long host route; a one-step or fully
    /// revealed route falls back to its final destination, preserving the ordinary
    /// refusal behaviour.
    pub(super) fn record(
        &mut self,
        orders: &[Order],
        state: &civvis::mirror::StateSnapshot,
        frontier_steps: &std::collections::BTreeMap<i64, (i32, i32)>,
    ) {
        // A sequenced turn can carry a second MOVE_TO for the same unit, planned
        // from where an earlier act leaves it. Only the FIRST is judged against
        // the unit's exported position next turn; the second would read a unit
        // that walked, struck and walked on as "never moved".
        let mut recorded = std::collections::BTreeSet::new();
        for order in orders {
            if order.kind != "unit" || order.verb.as_deref() != Some("MOVE_TO") {
                continue;
            }
            let (Some(unit), Some(dest)) = (order.subject, order.pos) else {
                continue;
            };
            if !recorded.insert(unit) {
                continue;
            }
            let Some(from) = state
                .units
                .iter()
                .find(|u| u.id == unit)
                .map(|u| (u.x, u.y))
            else {
                continue;
            };
            let frontier_step = self
                .pending_probes
                .get(&unit)
                .filter(|probe| probe.from == from && probe.target == dest)
                .map(|probe| probe.target)
                .or_else(|| frontier_steps.get(&unit).copied())
                .unwrap_or(dest);
            self.sent.insert(
                unit,
                HostMoveAttempt {
                    from,
                    destination: dest,
                    frontier_step,
                    turn: state.turn,
                },
            );
        }
    }

    /// Take the proved-unwalkable plots off the mirror's speculative frontier —
    /// both domains' priors, since the refusal came from whichever unit was sent
    /// and the plot is dead for the sea's scout as much as the land's.
    pub(super) fn apply(&self, mirror: &mut civvis::mirror::LiveMirror) {
        for &(x, y) in &self.dead {
            let pos = civvis::hex::offset_to_axial(x, y);
            if let Some(tile) = mirror.game.map.tiles.get_mut(&pos) {
                if tile.terrain == "unknown" {
                    tile.assumed_traversable = false;
                    tile.assumed_navigable = false;
                }
            }
        }
    }
}

/// ⭐ THE ACTUATION CONTRACT: an order earns a verified outcome or an explained
/// refusal, and one the host has provably refused stops being re-sent.
///
/// ## What was happening instead
///
/// The bridge already checks every order it sends. `settle_pending_orders`
/// pairs each `IssuedOrder` with a [`Verdict`] on the next turn's frame and
/// ships the answers back as `order_verified` / `order_failed` rows. Nothing
/// then reads them. The verdicts are a ledger, and the decision that produced
/// the order never sees one — so an order the host refuses is re-derived from
/// the same persistent agent state next turn, re-translated to a byte-identical
/// `Order`, and sent again. Recorded live: **13.9% of all orders come back
/// `did_not_move`**, and the same refused `MOVE_TO` has been re-issued **eleven
/// turns running**. `suppress_same_turn_replays` cannot see it: it resets on
/// every new turn, so it stops a duplicate within a turn and never a repeat
/// across one.
///
/// ## What this does
///
/// Counts *consecutive identical* failures. An order is identified by
/// everything the host is told — kind, verb, subject and target plot — so a
/// re-planned destination is a different order and starts clean. A failure
/// after [`ORDER_REFUSAL_COOLDOWN_TURNS`] quiet turns starts the count again:
/// the same order failing on turns 86, 101 and 147 is three separate events,
/// not one standing refusal. After
/// [`ORDER_REFUSAL_STRIKES`] failures with the same reason, that exact order is
/// withheld for [`ORDER_REFUSAL_COOLDOWN_TURNS`] host turns and the reply says
/// so by name. Any `Verified` for that identity clears the record immediately.
///
/// ⚠ **The reason is compared, not classified.** `HostMoveRefusals` keeps a
/// hand-written list of two reason strings it trusts as proof of dead ground
/// and rejects the rest as transient. That list is right for the question it
/// asks and would be wrong here, and a hand-written list of reasons is exactly
/// the shape that goes stale as the mod learns to name new ones. A genuinely
/// transient condition — no movement left, a unit in the way this turn — does
/// not produce the *same* reason three turns running; a standing refusal does.
/// So the rule needs no list and cannot fall behind the mod.
///
/// ⚠ `Unverifiable` never counts, in either direction. An order whose effect
/// the frame cannot show is not evidence of anything, so it neither earns a
/// strike nor clears one.
pub(super) const ORDER_REFUSAL_STRIKES: u32 = 3;

/// How long a struck-out order stays withheld, in host turns. Long enough that
/// the agent's own re-planning gets a clear run at the problem, short enough
/// that ground which genuinely opens up — a war ending, a border opening, a
/// blocking unit leaving — is retried inside one era.
pub(super) const ORDER_REFUSAL_COOLDOWN_TURNS: u32 = 10;

/// Everything the host is told about one order. Two orders with the same
/// identity are the same request; anything else is a new one.
pub(super) type OrderIdentity = (String, Option<String>, Option<i64>, Option<(i32, i32)>);

pub(super) fn order_identity(order: &IssuedOrder) -> OrderIdentity {
    (
        order.kind.clone(),
        order.verb.clone(),
        order.subject,
        order.pos,
    )
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RefusalRecord {
    /// Consecutive failures carrying `reason`.
    pub(super) strikes: u32,
    pub(super) reason: String,
    /// The turn the cooldown expires on, once the strikes are spent.
    pub(super) until: Option<u32>,
}

#[derive(Default)]
pub(super) struct HostOrderRefusals {
    pub(super) seen: std::collections::BTreeMap<OrderIdentity, RefusalRecord>,
    /// The turn of each order's latest failure, cleared when it is
    /// verified. See `formation_refusals`.
    pub(super) failed_on: std::collections::BTreeMap<OrderIdentity, u32>,
    war_permissions: std::collections::BTreeMap<i64, bool>,
    typed_war_permissions: std::collections::BTreeMap<(i64, String), bool>,
}

impl HostOrderRefusals {
    /// A changed host permission is new evidence, not another blind retry.
    /// Keep ordinary cooldowns while permission stays true or is unknown.
    pub(super) fn observe_war_permissions(&mut self, state: &civvis::mirror::StateSnapshot) {
        for rival in &state.rivals {
            if let Some(facts) = &rival.war_declarations {
                for fact in facts {
                    let Some(allowed) = fact.allowed else {
                        continue;
                    };
                    let target = rival.player as i64;
                    let prior = self
                        .typed_war_permissions
                        .insert((target, fact.statement.clone()), allowed);
                    if allowed && prior == Some(false) {
                        self.seen.retain(|(kind, verb, subject, _), record| {
                            let statement = match verb.as_deref() {
                                Some("DECLARE") => Some("DECLARE_SURPRISE_WAR"),
                                other => other,
                            };
                            !(kind == "war"
                                && *subject == Some(target)
                                && statement == Some(fact.statement.as_str())
                                && matches!(
                                    record.reason.as_str(),
                                    "not_at_war" | "cannot_declare"
                                ))
                        });
                    }
                }
                // Some different war becoming available is not permission to
                // repeat the exact type the native host just refused.
                continue;
            }
            let Some(allowed) = rival.can_declare else {
                continue;
            };
            let prior = self.war_permissions.insert(rival.player as i64, allowed);
            if allowed && prior == Some(false) {
                self.seen.retain(|(kind, _, subject, _), record| {
                    !(kind == "war"
                        && *subject == Some(rival.player as i64)
                        && matches!(record.reason.as_str(), "not_at_war" | "cannot_declare"))
                });
            }
        }
    }

    /// Read one turn's checks. `turn` is the turn the orders were sent on.
    pub(super) fn observe(&mut self, checks: &[OrderCheck], turn: u32) {
        for check in checks {
            let identity = order_identity(&check.order);
            match &check.verdict {
                // The host did what was asked, so whatever went before is
                // history: this order works.
                Verdict::Verified => {
                    self.seen.remove(&identity);
                    self.failed_on.remove(&identity);
                }
                Verdict::Failed(reason) => {
                    // One strike per order a turn for a city, governor or
                    // diplomacy order: a replan frame re-sends the same order,
                    // and its refusal is the same answer again, not another
                    // turn of it. Counting frames struck an order out inside
                    // one turn -- live King civvis-20261005T141932Z (game 135)
                    // sent Victor's post in three frames of turn 142 and could
                    // not send it again until 152. A unit's order keeps a
                    // strike per check: a plane strikes once a turn, so a
                    // refused strike re-sent in the same turn is futile
                    // (`air_pillage_tests`), and a move's same-turn replays are
                    // already suppressed upstream.
                    let previous = self.failed_on.insert(identity.clone(), turn);
                    let struck_this_turn = check.order.kind != "unit" && previous == Some(turn);
                    let after_a_quiet_spell = previous.is_none_or(|last| {
                        turn.saturating_sub(last) >= ORDER_REFUSAL_COOLDOWN_TURNS
                    });
                    let record = self.seen.entry(identity).or_insert(RefusalRecord {
                        strikes: 0,
                        reason: reason.clone(),
                        until: None,
                    });
                    if record.reason == *reason && !after_a_quiet_spell {
                        if !struck_this_turn {
                            record.strikes += 1;
                        }
                    } else {
                        // A different answer is a different problem, and an
                        // old one is history. The host is not standing on one
                        // refusal, so the count starts again rather than
                        // accumulating across causes or across the whole game.
                        record.reason = reason.clone();
                        record.strikes = 1;
                    }
                    record.until = (record.strikes >= ORDER_REFUSAL_STRIKES)
                        .then_some(turn + ORDER_REFUSAL_COOLDOWN_TURNS);
                }
                Verdict::Unverifiable => {}
            }
        }
    }

    /// Why this order is withheld, if it is.
    pub(super) fn withheld(&self, order: &Order, turn: u32) -> Option<&str> {
        let identity = (
            order.kind.to_string(),
            order.verb.clone(),
            order.subject,
            order.pos,
        );
        self.seen
            .get(&identity)
            .filter(|record| record.until.is_some_and(|until| turn < until))
            .map(|record| record.reason.as_str())
    }

    /// Drop records whose cooldown has run out, so the table tracks the live
    /// problem rather than the whole game's history.
    pub(super) fn sweep(&mut self, turn: u32) {
        self.seen
            .retain(|_, record| record.until.is_none_or(|until| turn < until));
        self.failed_on
            .retain(|_, failed| turn.saturating_sub(*failed) < ORDER_REFUSAL_COOLDOWN_TURNS);
    }
}

/// Withhold the orders that have struck out, and name each one.
///
/// Returns the surviving orders and one `kind:verb reason` line per withheld
/// order, so a refusal that stops being sent is visible in the reply rather
/// than simply absent.
pub(super) fn withhold_refused_orders(
    orders: Vec<Order>,
    state: &civvis::mirror::StateSnapshot,
    refusals: &mut HostOrderRefusals,
) -> (Vec<Order>, Vec<String>) {
    let turn = state.turn;
    refusals.observe_war_permissions(state);
    refusals.sweep(turn);
    let mut allowed = Vec::with_capacity(orders.len());
    let mut withheld = Vec::new();
    for order in orders {
        match refusals.withheld(&order, turn) {
            Some(reason) => withheld.push(format!(
                "{}:{} {}",
                order.kind,
                order.verb.as_deref().unwrap_or("-"),
                reason
            )),
            None => allowed.push(order),
        }
    }
    (allowed, withheld)
}

/// Withhold only peace orders whose known Firaxis cooldown has not expired.
///
/// This is deliberately below translation, so native `MakePeace`, negotiated
/// peace deals, and the report-only mirrored peace path all obey the same host
/// contract. Other diplomacy stays untouched.
pub(super) fn defer_host_peace_retries(
    orders: Vec<Order>,
    state: &civvis::mirror::StateSnapshot,
    retries: &mut HostPeaceRetries,
) -> (Vec<Order>, Vec<i64>) {
    retries.observe(state);
    let mut allowed = Vec::with_capacity(orders.len());
    let mut deferred = Vec::new();
    for order in orders {
        let Some(target) = (order.kind == "peace").then_some(order.subject).flatten() else {
            allowed.push(order);
            continue;
        };
        if retries.permits(target, state.turn) {
            retries.record(target, state.turn);
            allowed.push(order);
        } else {
            deferred.push(target);
        }
    }
    (allowed, deferred)
}
