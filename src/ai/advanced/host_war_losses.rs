use super::*;

impl AdvancedAi {
    /// The native mirror supplies war flags but does not replay combats into
    /// Game::wars. Rebuild unit losses from confirmed host deaths instead of
    /// treating an empty simulator ledger as an even exchange. Keys are
    /// (victim's model seat, opponent's model seat); totals are cumulative.
    pub(super) fn observe_host_war_losses(
        &mut self,
        g: &Game,
        state: &crate::mirror::StateSnapshot,
    ) {
        let seats =
            crate::mirror::host_major_seat_map(state, crate::mirror::modeled_major_player_count(g));
        let mut losses = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for death in &state.confirmed_unit_deaths {
            if death.turn > g.turn || death.turn > state.turn {
                continue;
            }
            let Some(opponent) = death.opponent else {
                continue;
            };
            let (Some(&victim), Some(&opponent)) = (seats.get(&death.player), seats.get(&opponent))
            else {
                continue;
            };
            let major = |seat: usize| {
                g.players
                    .get(seat)
                    .is_some_and(|p| !p.is_minor && !p.is_barbarian && !p.is_free_city)
            };
            if victim == opponent
                || !major(victim)
                || !major(opponent)
                || !seen.insert((death.player, death.unit))
            {
                continue;
            }
            *losses.entry((victim, opponent)).or_insert(0) += 1;
        }
        // Some(empty) is significant: no confirmed host losses must not fall
        // back to kills made by the speculative planning pass. Simulated games
        // never call this observer and retain the engine's complete ledger.
        self.host_war_unit_losses = Some(losses);
    }
}

#[cfg(test)]
mod tests;
