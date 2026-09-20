use super::*;

impl AdvancedAi {
    /// A recruited person's first usable activation district is a commitment,
    /// even before a turn of production has reached its new foundation.
    pub(super) fn live_gp_district_commitment(g: &Game, pid: usize, item: &Item) -> bool {
        let Item::District { district, .. } = item else {
            return false;
        };
        let family = g.district_family(*district);
        let needed = g.players[pid]
            .live_great_person_activation_needs
            .iter()
            .any(|need| {
                // Match the physical-person planner: these Engineers need a
                // wonder, not their class's default Industrial Zone.
                !(need.kind == "engineer"
                    && matches!(
                        need.individual.as_deref(),
                        Some("imhotep" | "gustave_eiffel")
                    ))
                    && BasicAi::live_great_person_district(need) == Some(family.as_str())
            });
        needed
            && !g
                .player_city_ids(pid)
                .into_iter()
                .any(|cid| g.city_has_district_family(&g.cities[&cid], family))
    }
}

#[cfg(test)]
mod tests;
