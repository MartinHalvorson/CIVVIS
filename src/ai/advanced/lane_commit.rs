//! `lane-commit`: from the midpoint of the game an adaptive seat plays for
//! the victory it can land.
//!
//! Operator, 2026-08-24: *"add some logic or some heuristic for steering us
//! towards our best victory condition harder. From the midpoint of the game,
//! we should have the victory in mind and be optimizing towards winning
//! that."*
//!
//! ## What the adaptive seat does without it
//!
//! `assess` puts an adaptive seat — no `--victory`, which is what production
//! ships and what every measured seat in a `gene_screen` game is — on a
//! victory lane only when `victory_focus` reads that lane at **65% or more**,
//! or when nothing else claims the plan. Everything else claims it first:
//! a city short of the target with land still open, a neighbour weak enough
//! to raid, a Prophet still on the table. `docs/VICTORY_GENES.md` measured
//! the result at the deployment shape: the adaptive seat spends **48% of its
//! seat-turns** under Expansion, Conquest or Recovery, and the lane it is
//! racing never reaches the deciders keyed on `victory_target` — the tech
//! beeline, the government, the policy deck, the space race, the wonder
//! lane, the Congress ballot — because those read the operator's assignment
//! and an adaptive seat has none. `victory_focus` itself re-decides every
//! turn from raw progress, so a seat at 50% religion and 48% science can
//! alternate plans on a single conversion.
//!
//! ## What the gene does
//!
//! At the midpoint of the game — half the turn cap, turn 125 on the 250-turn
//! Online standard (`docs/GENE_SCREEN.md`, *One screen*), or
//! [`super::LANE_COMMIT_MIDPOINT_STANDARD`] standard turns when there is no
//! cap — the seat **commits** to one lane, and from then on
//! [`AdvancedAi::raced_target`] answers with it wherever the operator's
//! `victory_target` used to be read alone. The assessment follows it as it
//! follows an assigned lane, after the postures that must come first (a home
//! city under threat, an emergency, a rival at the wire, a war already
//! making progress).
//!
//! The lane is not the one with the most progress. It is the one that
//! **lands before the clock**: each lane's progress is sampled every
//! `LANE_COMMIT_SAMPLE_EVERY` standard turns, its rate is read over the last
//! `LANE_COMMIT_RATE_WINDOW`, and the turn it reaches 100 is projected. The
//! earliest projection that falls inside the cap wins. Under the operator's
//! standing regime (`docs/GENE_SCREEN.md`: *a game that reaches the clock is
//! a SCORE VICTORY*) science lands at a median turn 283 and diplomacy at 285
//! — past a 250-turn cap — so a seat at 45% science with 125 turns left is
//! not racing science, whatever `victory_focus` says; and when no lane lands
//! in time the commitment is **Score**, which is a victory condition, not a
//! consolation. The domination lane is never chosen: it has landed in 0% of
//! screen games at every map and clock
//! ([`super::victory_lane`] and `docs/VICTORY_GENES.md`).
//!
//! The commitment is reviewed every `LANE_COMMIT_REVIEW` standard turns and
//! holds against a marginal challenger: a different lane takes over only
//! when it is projected to land `LANE_COMMIT_SWITCH_MARGIN` standard turns
//! sooner, or when the committed lane has stopped moving and another still
//! lands. The districts, policies and Great People already bought for a lane
//! are the reason a projection a few turns better is not a reason to leave.
//!
//! ## What it does not touch
//!
//! `active_victory_target` stays the operator's: victory denial keeps its
//! adaptive form (a committed seat still counters a rival at the wire —
//! `deny_while_targeted` is about assigned seats), the expansion dispatcher
//! and `pursue_religion` read the assignment as before, and the census field
//! `victory_target` still reports the operator's target so an adaptive seat
//! can be told from a targeted one. On the live bridge, which always passes
//! `--victory <lane>`, the gene is inert by construction.
//!
//! Opt-in, off in both controllers, byte-identical when off; priced by
//! `gene_screen` like every other row of the registry.

use super::{
    AdvancedAi, LaneCommitment, LaneSample, VictoryTarget, LANE_COMMIT_RATE_WINDOW,
    LANE_COMMIT_REVIEW, LANE_COMMIT_SAMPLES, LANE_COMMIT_SAMPLE_EVERY,
    LANE_COMMIT_SWITCH_MARGIN,
};
use crate::game::Game;

/// The lanes a seat can commit to, in the order `lane_progress_table`
/// reports them. Domination is deliberately absent (see the module doc) and
/// Score is the fallback rather than a raced lane.
pub(super) const LANE_COMMIT_LANES: [VictoryTarget; 4] = [
    VictoryTarget::Science,
    VictoryTarget::Culture,
    VictoryTarget::Religion,
    VictoryTarget::Diplomacy,
];

/// One lane's reading at a review: where it stands and when it lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LaneReading {
    pub lane: VictoryTarget,
    pub progress: i32,
    /// The turn the lane is projected to reach 100, if it is moving at all.
    pub projected: Option<u32>,
}

impl LaneReading {
    /// Lands inside the cap — or, with no cap, lands at all.
    fn lands(&self, g: &Game) -> bool {
        self.projected
            .is_some_and(|turn| g.turn_limit().is_none_or(|limit| turn <= limit))
    }
}

impl AdvancedAi {
    /// The lane this seat is playing for: the operator's assignment, or the
    /// lane `lane_commit` committed to. Every decider that used to read
    /// `victory_target` alone reads this.
    pub(super) fn raced_target(&self) -> Option<VictoryTarget> {
        self.victory_target.or(self.committed_lane())
    }

    /// The lane `lane_commit` has committed this seat to, if any.
    pub fn committed_lane(&self) -> Option<VictoryTarget> {
        self.lane_commitment.map(|commitment| commitment.lane)
    }

    /// The whole commitment, for instruments and tests.
    pub fn lane_commitment(&self) -> Option<LaneCommitment> {
        self.lane_commitment
    }

    /// The turn the game is half over: half the cap when there is one,
    /// otherwise half of a Standard game scaled to this speed.
    pub(super) fn lane_commit_midpoint(g: &Game) -> u32 {
        g.turn_limit()
            .map(|limit| limit / 2)
            .unwrap_or_else(|| g.standard_duration(super::LANE_COMMIT_MIDPOINT_STANDARD))
    }

    /// Sample the lanes, and at the midpoint commit — then review on the
    /// cadence. Exact no-op while the gene is off or the operator assigned a
    /// lane. Called once a turn from `take_turn_inner`, before the plan is
    /// assessed, so a fresh commitment is assessed the same turn.
    pub(super) fn maintain_lane_commit(&mut self, g: &Game, pid: usize) {
        if !self.lane_commit || self.victory_target.is_some() {
            return;
        }
        let midpoint = Self::lane_commit_midpoint(g);
        let window = g.standard_duration(LANE_COMMIT_RATE_WINDOW).max(1);
        // Nothing to read until the rate window can reach the midpoint.
        if g.turn.saturating_add(window) < midpoint {
            return;
        }
        let sample_every = g.standard_duration(LANE_COMMIT_SAMPLE_EVERY).max(1);
        let due = self
            .lane_samples
            .back()
            .is_none_or(|last| g.turn.saturating_sub(last.turn) >= sample_every);
        let progress = if due {
            let progress = self.lane_progress_table(g, pid);
            self.lane_samples.push_back(LaneSample {
                turn: g.turn,
                progress,
            });
            while self.lane_samples.len() > LANE_COMMIT_SAMPLES {
                self.lane_samples.pop_front();
            }
            progress
        } else {
            match self.lane_samples.back() {
                Some(last) => last.progress,
                None => return,
            }
        };
        if g.turn < midpoint {
            return;
        }
        let review_due = self.lane_commitment.is_none_or(|commitment| {
            g.turn.saturating_sub(commitment.reviewed)
                >= g.standard_duration(LANE_COMMIT_REVIEW).max(1)
        });
        if !review_due {
            return;
        }
        self.review_lane_commitment(g, progress);
    }

    /// Read every lane, pick the one that lands, and commit or switch.
    pub(super) fn review_lane_commitment(&mut self, g: &Game, progress: [i32; 4]) {
        let readings = self.lane_readings(g, progress);
        let choice = Self::lane_commit_choice(g, &readings);
        let current = self.lane_commitment;
        let (next, because) = match current {
            None => (choice, "the midpoint"),
            Some(held) if held.lane == choice.lane => (choice, ""),
            Some(held) => {
                let held_reading = readings
                    .iter()
                    .copied()
                    .find(|reading| reading.lane == held.lane)
                    .unwrap_or(LaneReading {
                        lane: held.lane,
                        progress: 0,
                        projected: None,
                    });
                let margin = g.standard_duration(LANE_COMMIT_SWITCH_MARGIN);
                if held.lane == VictoryTarget::Score {
                    // Score was the fallback; any lane that lands beats it.
                    (choice, "a lane now lands before the clock")
                } else if !held_reading.lands(g) {
                    (choice, "the committed lane has stopped landing in time")
                } else if choice.lands(g)
                    && choice.projected.is_some_and(|turn| {
                        held_reading
                            .projected
                            .is_some_and(|held_turn| turn.saturating_add(margin) <= held_turn)
                    })
                {
                    (choice, "another lane lands well sooner")
                } else {
                    (
                        LaneReading {
                            lane: held.lane,
                            progress: held_reading.progress,
                            projected: held_reading.projected,
                        },
                        "",
                    )
                }
            }
        };
        let changed = current.is_none_or(|held| held.lane != next.lane);
        self.lane_commitment = Some(LaneCommitment {
            lane: next.lane,
            since: if changed {
                g.turn
            } else {
                current.map_or(g.turn, |held| held.since)
            },
            reviewed: g.turn,
            progress_at_commit: if changed {
                next.progress
            } else {
                current.map_or(next.progress, |held| held.progress_at_commit)
            },
            projected: next.projected,
        });
        if changed {
            // The plan was assessed for a seat with no lane; re-assess now.
            self.plan = None;
            let landing = match next.projected {
                Some(turn) => format!("projected to land on turn {turn}"),
                None => "not projected to land before the clock".to_string(),
            };
            let clock = match g.turn_limit() {
                Some(limit) => format!("the clock runs out on turn {limit}"),
                None => "there is no clock".to_string(),
            };
            let field: Vec<String> = readings
                .iter()
                .map(|reading| {
                    format!(
                        "{} {}%{}",
                        reading.lane.as_str(),
                        reading.progress,
                        reading
                            .projected
                            .map_or(String::new(), |turn| format!(" (turn {turn})"))
                    )
                })
                .collect();
            crate::think!(self.journal(), Strategy, Strategy,
                   "Committing to the {} victory", next.lane.as_str();
                   "{because}: {}% along, {landing}; {clock}; the lanes read {}",
                   next.progress, field.join(", "));
        }
    }

    /// Every raced lane's progress and projected landing turn.
    fn lane_readings(&self, g: &Game, progress: [i32; 4]) -> Vec<LaneReading> {
        LANE_COMMIT_LANES
            .iter()
            .enumerate()
            .filter(|(_, lane)| g.victory_conditions.is_enabled(lane.as_str()))
            .map(|(index, lane)| LaneReading {
                lane: *lane,
                progress: progress[index],
                projected: self.lane_projection(g, index, progress[index]),
            })
            .collect()
    }

    /// The turn a lane reaches 100 at its recent rate: the rise over the
    /// oldest sample inside the rate window, or — for the committed lane —
    /// since the commitment, whichever is faster, so a lane that converted a
    /// rival thirty turns ago and is working on the next is not read as
    /// stalled. `None` when the lane is not moving.
    fn lane_projection(&self, g: &Game, index: usize, now: i32) -> Option<u32> {
        if now >= 100 {
            return Some(g.turn);
        }
        let window = g.standard_duration(LANE_COMMIT_RATE_WINDOW).max(1);
        let floor = g.turn.saturating_sub(window);
        let window_rate = self
            .lane_samples
            .iter()
            .find(|sample| sample.turn >= floor && sample.turn < g.turn)
            .map(|sample| {
                f64::from(now - sample.progress[index]) / f64::from(g.turn - sample.turn)
            });
        let commit_rate = self
            .lane_commitment
            .filter(|commitment| {
                commitment.lane == LANE_COMMIT_LANES[index] && commitment.since < g.turn
            })
            .map(|commitment| {
                f64::from(now - commitment.progress_at_commit)
                    / f64::from(g.turn - commitment.since)
            });
        let rate = match (window_rate, commit_rate) {
            (Some(a), Some(b)) => a.max(b),
            (Some(a), None) | (None, Some(a)) => a,
            (None, None) => return None,
        };
        if rate <= 0.0 {
            return None;
        }
        let remaining = (f64::from(100 - now) / rate).ceil();
        Some(g.turn.saturating_add(remaining.min(f64::from(u32::MAX / 2)) as u32))
    }

    /// The lane to play for: the earliest landing inside the cap (higher
    /// progress breaks a tie, then the table's order); Score when nothing
    /// lands and score is a victory; otherwise the most advanced lane, so a
    /// board with no score victory still races something.
    fn lane_commit_choice(g: &Game, readings: &[LaneReading]) -> LaneReading {
        let landing = readings
            .iter()
            .copied()
            .filter(|reading| reading.lands(g))
            .min_by(|a, b| {
                a.projected
                    .cmp(&b.projected)
                    .then(b.progress.cmp(&a.progress))
            });
        if let Some(reading) = landing {
            return reading;
        }
        if g.victory_conditions.score {
            return LaneReading {
                lane: VictoryTarget::Score,
                progress: 0,
                projected: g.turn_limit(),
            };
        }
        readings
            .iter()
            .copied()
            .rev()
            .max_by(|a, b| a.progress.cmp(&b.progress))
            .unwrap_or(LaneReading {
                lane: VictoryTarget::Score,
                progress: 0,
                projected: None,
            })
    }
}
