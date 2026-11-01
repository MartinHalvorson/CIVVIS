//! `launch-commits-the-endgame`: once our own Earth Satellite has landed, a
//! seat assigned to any lane but Domination counts as committed to the
//! science endgame, so the dedicated schedule (`schedule_science_endgame`:
//! serial launches, parallel laser stations, the launch queue's authority
//! over generic production) runs for it as it does for a Science seat.
//!
//! **Why (live 6p King, 2026-11-01; -ca's read).**
//! - civvis-20261101T075446Z (Amanitore, assigned Religion; it founded at
//!   t48 and then played the tree) matched the leading rival tech for tech
//!   (77 against 77) and led every launch: Earth Satellite t208, Moon
//!   Landing t215, Mars Colony t226, against the Ottomans' t215, t224 and
//!   t229. It lost the Exoplanet leg: ours launched at t240, theirs at t236
//!   and arrived at t245.
//! - The endgame schedule never engaged. `science_endgame_committed` admits a
//!   seat that has launched only while `raced_target()` is `None`, and an
//!   assigned Religion seat reads `Some(Religion)`; the science drive answers
//!   only a rival's launch, and only with an outright lead. The seat logged
//!   no drive line in the endgame, researched Lasers at t187 as "worth 11 to
//!   the religion plan", ran one Spaceport against the Ottomans' three with
//!   no laser station, and its only pad, idle when the Mars Colony finished
//!   at t226, spent two turns on a Neighborhood before the expedition.
//!
//! **What the gene does.** `science_endgame_committed` and
//! `science_endgame_production_committed` also admit a seat whose Earth
//! Satellite has landed, whatever lane it was assigned, Domination excepted
//! (the endgame's own guard). Nothing changes before the first launch.
//!
//! Off, nothing changes.

use super::*;

impl AdvancedAi {
    /// Whether our own launch commits this seat to the science endgame: the
    /// gene is on and our Earth Satellite has landed.
    pub(super) fn our_launch_commits_the_endgame(&self, g: &Game, pid: usize) -> bool {
        self.launch_commits_the_endgame
            && g.players[pid]
                .science_projects
                .contains("launch_earth_satellite")
    }
}

#[cfg(test)]
mod tests;
