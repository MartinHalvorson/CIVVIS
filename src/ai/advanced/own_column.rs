//! `own-column-is-not-a-refusal`: what the live bridge actually sent.
//!
//! `live-move-refusal-break` proves a host refusal from our own record: the
//! step this turn's plan issued, and next turn's board showing the unit never
//! took it. Two of those records were never the host's answer. The plan is
//! written before the bridge's outbound filters run, and the air-assault
//! barrier withholds every ground order behind a frame's first sortie: live
//! King civvis-20261005T060002Z (G104) deferred 60 orders on frame 0 of t216
//! and 52 on t217, while the frame-0 plan had already recorded each withheld
//! unit's watched step and path trail. The next frame's replan found that
//! unit's own first step "already walked" by the same-turn reversal guard and
//! gave it nothing, and the following turn judged the step the host never saw
//! as a refusal. A Rocket Artillery 13 tiles from Sparta was barred for eight
//! turns on t218 and idled t216-223; another stood 14 turns at (23,20) until
//! it "stands down; it is going nowhere" on t236. And Firaxis checks stacking
//! where a move ends, so a step the planning board freed by walking a column
//! mate away first is refused if the host takes the two orders the other way
//! round: congestion in our own column, not ground the host will not let us
//! walk.
//!
//! The bridge snapshots the movement memory before it plans a frame
//! ([`AdvancedAi::host_frame_movement`]) and reports, once the frame's orders
//! are final, which planned movers crossed and which were withheld
//! ([`AdvancedAi::note_host_moves`]). See `BasicAi::note_host_moves`,
//! `BasicAi::judge_move_refusals` and `BasicAi::observe_unit_motion`.
use super::{AdvancedAi, ObservedMovementMemory};
use crate::game::Game;
use std::collections::BTreeSet;

/// The movement memory as it stood before a live frame was planned. Opaque:
/// the bridge only hands it back.
pub struct HostFrameMovement(ObservedMovementMemory);

impl AdvancedAi {
    /// Whether the gene is on; the bridge asks before it builds the report.
    pub fn own_column_is_not_a_refusal_enabled(&self) -> bool {
        self.own_column_is_not_a_refusal
    }

    /// Snapshot the executed-movement memory before a live frame is planned.
    /// `None` with the gene off, so the off arm pays nothing.
    pub fn host_frame_movement(&self) -> Option<HostFrameMovement> {
        self.own_column_is_not_a_refusal
            .then(|| HostFrameMovement(self.observed_movement_memory()))
    }

    /// Report a live frame's final orders: `sent` are the planned movers whose
    /// move order crossed to Firaxis, `unsent` the planned movers withheld.
    /// Returns how many withheld units had this frame's movement memory rolled
    /// back. Inert with the gene off or without a snapshot.
    pub fn note_host_moves(
        &mut self,
        g: &Game,
        before: Option<HostFrameMovement>,
        sent: &BTreeSet<u32>,
        unsent: &BTreeSet<u32>,
    ) -> usize {
        let Some(HostFrameMovement(before)) = before else {
            return 0;
        };
        self.base
            .note_host_moves(g, &before.paths, &before.watches, sent, unsent)
    }
}

#[cfg(test)]
mod tests;
