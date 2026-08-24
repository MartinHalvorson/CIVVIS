//! `lane-commit`: from the midpoint of the game an adaptive seat plays for
//! the victory it leads the field in.
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
//! or when nothing else claims the plan; and `victory_focus` re-decides
//! every turn from raw progress, so a seat at 50% religion and 48% science
//! alternates plans on a single conversion. The deciders keyed on
//! `victory_target` — the tech beeline, the government, the policy deck, the
//! Culture routing, the wonder lane's strategic value, the space race — read
//! the operator's assignment, and an adaptive seat has none
//! (`docs/VICTORY_GENES.md` §2: the adaptive seat is on a lane for 52% of
//! its seat-turns).
//!
//! ## What the gene does
//!
//! At the midpoint of the game — half the turn cap, turn 125 on the 250-turn
//! Online standard (`docs/GENE_SCREEN.md`, *One screen*), or
//! [`super::LANE_COMMIT_MIDPOINT_STANDARD`] standard turns when there is no
//! cap — the seat **commits** to one lane, and from then on
//! [`AdvancedAi::raced_target`] answers with it wherever a decider resolves
//! *which lane the empire is playing for*. The assessment follows it in
//! place of `victory_focus`'s per-turn pick, after every posture that comes
//! first today: a home city under threat, an emergency, a rival at the wire,
//! a war in progress, a neighbour weak enough to take, a Prophet still on
//! the table.
//!
//! **The lane is the one we lead.** Winning is relative — every rival on the
//! board runs this same planner — so each lane is read for the seat *and for
//! every living major* on one table (`lane_progress_table`: the four
//! readings `victory_focus` ranks, minus the civilization preferences), and
//! the seat commits to the lane it leads that is closest to landing;
//! leading none, the lane it is furthest along in, which is what
//! `victory_focus` would have said, made sticky. Progress is the one scale
//! the lanes share; the size of a lead is not (religion moves twelve points
//! a converted rival, science under one a tech), so it only breaks ties.
//! Domination is never chosen: it has landed in 0% of screen games at every
//! map and clock (`docs/VICTORY_GENES.md`).
//!
//! The commitment is reviewed every `LANE_COMMIT_REVIEW` standard turns and
//! holds against a marginal challenger: a different lane takes over only
//! when the seat has lost the lead in the committed lane and holds it in the
//! challenger, or when the challenger is `LANE_COMMIT_SWITCH_MARGIN` points
//! further along at the same standing. The districts, policies and Great
//! People already bought for a lane are the reason a reading a few points
//! better is not a reason to leave.
//!
//! ## What it does not touch, and why: the first draft's probe
//!
//! The first draft projected each lane's landing turn from its rate and
//! inherited the whole of an assigned lane's semantics — the Congress
//! abstention for non-diplomatic targets, the missionary and Great Work
//! vetoes, the space-race block, the expansion cutoff — and its 24-game
//! fires probe (seeds 26083100..26083123) read **−8.3 pp ± 5.4** on wins.
//! Science victories landed in 7 of those 24 games, at turns 220–250, six of
//! them by *off* seats: a linear projection of the science reading (25 + 30
//! × techs, then discrete project steps) says science cannot land by 250,
//! and the seats it moved to Religion, Culture and Score finished with five
//! fewer techs. Committed seats also held a third of the off seats' Diplomatic
//! Victory Points (3.7 against 9.6 — the abstention), fewer cities and a
//! smaller army (the commitment sat above the elective war `opportunistic-war`
//! is priced `helps` for). So this version keeps `victory_target` — the
//! operator's assignment — on every veto, keeps `active_victory_target` the
//! operator's (victory denial stays adaptive, `pursue_religion` and the
//! expansion dispatcher unchanged, the census still tells an adaptive seat
//! from a targeted one), and routes only the objective resolutions through
//! `raced_target`. On the live bridge, which always passes `--victory
//! <lane>`, the gene is inert by construction.
//!
//! Opt-in, off in both controllers, byte-identical when off; priced by
//! `gene_screen` like every other row of the registry.

use super::{
    AdvancedAi, LaneCommitment, VictoryTarget, LANE_COMMIT_REVIEW, LANE_COMMIT_SWITCH_MARGIN,
};
use crate::game::Game;

/// The lanes a seat can commit to, in the order `lane_progress_table`
/// reports them. Domination is deliberately absent (see the module doc).
pub(super) const LANE_COMMIT_LANES: [VictoryTarget; 4] = [
    VictoryTarget::Science,
    VictoryTarget::Culture,
    VictoryTarget::Religion,
    VictoryTarget::Diplomacy,
];

/// One lane's reading at a review: where the seat stands and how far ahead
/// of the best rival it is on the same table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LaneReading {
    pub lane: VictoryTarget,
    pub progress: i32,
    /// Own progress less the best living major rival's; positive is a lead.
    pub lead: i32,
}

impl LaneReading {
    /// The order a review prefers: a lead over no lead, then the further
    /// progress (the scale the lanes share), then the larger lead, then the
    /// table's order (the caller iterates in table order and keeps the first
    /// maximum).
    fn key(&self) -> (bool, i32, i32) {
        (self.lead >= 0, self.progress, self.lead)
    }
}

impl AdvancedAi {
    /// The lane this seat is playing for: the operator's assignment, or the
    /// lane `lane_commit` committed to. The deciders that resolve an
    /// objective read this; the vetoes an assigned lane carries keep reading
    /// `victory_target` (see the module doc).
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

    /// At the midpoint commit, then review on the cadence. Exact no-op while
    /// the gene is off or the operator assigned a lane. Called once a turn
    /// from `take_turn_inner`, before the plan is assessed, so a fresh
    /// commitment is assessed the same turn.
    pub(super) fn maintain_lane_commit(&mut self, g: &Game, pid: usize) {
        if !self.lane_commit || self.victory_target.is_some() {
            return;
        }
        if g.turn < Self::lane_commit_midpoint(g) {
            return;
        }
        let review_due = self.lane_commitment.is_none_or(|commitment| {
            g.turn.saturating_sub(commitment.reviewed)
                >= g.standard_duration(LANE_COMMIT_REVIEW).max(1)
        });
        if !review_due {
            return;
        }
        let readings = self.lane_readings(g, pid);
        self.review_lane_commitment(g, &readings);
    }

    /// Every enabled lane, read for the seat and for the best living major
    /// rival on the same table.
    pub(super) fn lane_readings(&self, g: &Game, pid: usize) -> Vec<LaneReading> {
        let own = self.lane_progress_table(g, pid);
        let mut best_rival = [0; 4];
        for rival in g
            .players
            .iter()
            .filter(|p| p.id != pid && p.alive && !p.is_minor && !p.is_barbarian)
        {
            let theirs = self.lane_progress_table(g, rival.id);
            for (best, reading) in best_rival.iter_mut().zip(theirs) {
                *best = (*best).max(reading);
            }
        }
        LANE_COMMIT_LANES
            .iter()
            .enumerate()
            .filter(|(_, lane)| g.victory_conditions.is_enabled(lane.as_str()))
            .map(|(index, lane)| LaneReading {
                lane: *lane,
                progress: own[index],
                lead: own[index].saturating_sub(best_rival[index]),
            })
            .collect()
    }

    /// Pick the lane to play for and commit or switch. Visible to the parent
    /// module so tests can drive a review from a fabricated table.
    pub(super) fn review_lane_commitment(&mut self, g: &Game, readings: &[LaneReading]) {
        let Some(choice) = Self::lane_commit_choice(readings) else {
            return;
        };
        let current = self.lane_commitment;
        let held_reading = current.and_then(|held| {
            readings
                .iter()
                .copied()
                .find(|reading| reading.lane == held.lane)
        });
        let (next, because) = match (current, held_reading) {
            (None, _) => (choice, "the midpoint"),
            (Some(_), None) => (choice, "the committed lane is no longer on the board"),
            (Some(_), Some(held)) if held.lane == choice.lane => (held, ""),
            (Some(_), Some(held)) => {
                if held.lead < 0 && choice.lead >= 0 {
                    (choice, "the lead in the committed lane is gone")
                } else if (held.lead >= 0) == (choice.lead >= 0)
                    && choice.progress >= held.progress.saturating_add(LANE_COMMIT_SWITCH_MARGIN)
                {
                    (
                        choice,
                        "another lane is well further along at the same standing",
                    )
                } else {
                    (held, "")
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
            progress: next.progress,
            lead: next.lead,
        });
        if changed {
            // The plan was assessed for a seat with no lane; re-assess now.
            self.plan = None;
            let field: Vec<String> = readings
                .iter()
                .map(|reading| {
                    format!(
                        "{} {}% ({:+} on the field)",
                        reading.lane.as_str(),
                        reading.progress,
                        reading.lead
                    )
                })
                .collect();
            let standing = if next.lead >= 0 {
                format!("leading the field by {} points", next.lead)
            } else {
                format!("{} points behind the leader", -next.lead)
            };
            crate::think!(self.journal(), Strategy, Strategy,
                   "Committing to the {} victory", next.lane.as_str();
                   "{because}: {}% along, {standing}; the lanes read {}",
                   next.progress, field.join(", "));
        }
    }

    /// The lane to play for: among the lanes the seat leads, the furthest
    /// along; leading none, the furthest along of all; the lead, then the
    /// table's order, break what is left. `None` only when no raced lane is
    /// a victory condition on this board.
    fn lane_commit_choice(readings: &[LaneReading]) -> Option<LaneReading> {
        let mut best: Option<LaneReading> = None;
        for reading in readings {
            if best.is_none_or(|held| reading.key() > held.key()) {
                best = Some(*reading);
            }
        }
        best
    }
}
