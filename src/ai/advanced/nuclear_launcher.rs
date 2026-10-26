//! `nuclear-goal-needs-a-launcher`: no nuclear war goal, so neither the
//! Manhattan Project, Operation Ivy nor a device is valued, and research is
//! not bent toward Nuclear Fission or Fusion for one.
//!
//! The live Civilization VI bridge builds warheads but cannot fire them:
//! `civvis_orders` carries `build_nuclear_device` as
//! `PROJECT_BUILD_NUCLEAR_DEVICE`, but there is no order for
//! `Action::WmdStrike`, and the agent mod reads its devices (`GetWMDs`) with
//! no strike handler. `advanced_wmd_strikes` fires only on a native board.
//!
//! **Why (retained live games to 2026-10-26, 123 runs).**
//! - 26 built nuclear projects: about 20,200 Production over 361 city-turns.
//!   That is Manhattan 10,143, Operation Ivy 4,246 and devices 5,787, about
//!   776 a game.
//! - That Production goes into late, near-won games. Immortal
//!   civvis-20261026T014409Z (27 cities, three of four capitals held) put
//!   Quito on the Manhattan Project and then a device while London, the last
//!   capital, stood behind 400 walls against an army 2.9 times ours.
//!
//! Arm it where nothing can launch a device; a native board keeps its
//! nuclear doctrine. Off, nothing changes.

use super::*;

impl AdvancedAi {
    /// Whether this seat has no way to launch a device: the gene.
    pub(super) fn nuclear_goal_has_no_launcher(&self) -> bool {
        self.nuclear_goal_needs_a_launcher
    }
}
