//! The live host's permission for a policy change on one observed turn, so a
//! forecast turn cannot inherit an old lock or an old free window.

use super::*;

/// The host's permission for a policy transaction on one observed turn.
/// Forecast turns must not inherit an old lock or an old free window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostPolicyWindow {
    pub turn: u32,
    pub open: bool,
}

impl Game {
    pub fn host_policy_change_window(&self, pid: usize) -> Option<bool> {
        self.host_policy_windows
            .get(&pid)
            .filter(|window| window.turn == self.turn)
            .map(|window| window.open)
    }

    /// A current host observation vetoes changes outside a confirmed free
    /// window. Missing legacy observations leave ordinary rules unchanged;
    /// the native controller independently checks permission before execution.
    pub fn host_policy_change_blocked(&self, pid: usize) -> bool {
        self.host_policy_change_window(pid) == Some(false)
    }
}

#[cfg(test)]
mod tests;
