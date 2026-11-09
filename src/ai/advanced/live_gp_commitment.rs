//! Production commitments for a recruited Great Person in the live game: the
//! district and slot building its activation needs, so routine queue claims
//! cannot undo them.

use super::*;

impl AdvancedAi {
    /// Keep a legal queued slot building while a physical cultural person
    /// has no host activation plot. The activation planner uses the same
    /// chain; routine queue claims must not undo its choice on every frame.
    /// An Artifact requirement needs an existing work, not another empty
    /// museum, so it does not reserve a cultural person's slot chain here.
    pub(super) fn live_gp_cultural_building_commitment(
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> bool {
        let Item::Building { building } = item else {
            return false;
        };
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        city.owner == pid
            && city.queue.first() == Some(item)
            && !(g.players[pid].gold_per_turn < -0.5
                && g.players[pid].gold < 100.0 + 25.0 * g.player_city_ids(pid).len() as f64)
            && g.players[pid]
                .live_great_person_activation_needs
                .iter()
                .any(|need| {
                    matches!(need.kind.as_str(), "writer" | "artist" | "musician")
                        && BasicAi::live_great_person_work(need).is_some_and(|work| {
                            work != "artifact"
                                && BasicAi::live_great_person_work_chain(work).iter().any(
                                    |family| {
                                        g.building_is_family(building, Name::new(family))
                                            && BasicAi::live_great_person_work_stage_needed(
                                                g, pid, work, family,
                                            )
                                    },
                                )
                        })
                })
            && Self::production_commitment_is_legal(g, pid, cid, item)
    }

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

#[cfg(test)]
#[path = "live_gp_commitment/cultural_building_tests.rs"]
mod cultural_building_tests;
