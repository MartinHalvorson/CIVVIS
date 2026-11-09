//! The host's policy-change window as the live state reports it
//! (`GovernmentScreen.lua:1383-1422`).

use super::*;

/// GovernmentScreen.lua:1383-1422 reads these before exposing Confirm.
/// Currency is irrelevant to a zero-cost window; no paid unlock is planned.
#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct StatePolicyChange {
    #[serde(default)]
    pub unlock_cost: Option<f64>,
    #[serde(default)]
    pub made: Option<bool>,
}

pub(super) fn apply(game: &mut crate::game::Game, state: &StateSnapshot) {
    if let Some(observed) = &state.policy_change {
        let open = observed.made == Some(false)
            && observed
                .unlock_cost
                .is_some_and(|cost| cost.is_finite() && cost == 0.0);
        game.host_policy_windows.insert(
            0,
            crate::game::HostPolicyWindow {
                turn: state.turn,
                open,
            },
        );
    } else {
        game.host_policy_windows.remove(&0);
    }
}

#[cfg(test)]
mod tests;
