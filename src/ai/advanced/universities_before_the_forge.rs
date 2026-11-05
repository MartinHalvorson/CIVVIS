//! `universities-before-the-forge` (opt-in): a short legal University releases
//! the city's Industrial Zone and Workshop producer claims.
//!
//! Native Indonesia Ternate70/0 chose an Industrial Zone while its University
//! was buildable in eleven Online turns. Same-binary cold ON/OFF proposals of
//! the producer claim choose industry/University respectively. An older
//! Majapahit107/0 offered its paused University at one turn, with an idle
//! queue, and chose a Workshop. Those are queue observations, not strength.
//!
//! Factory and power-plant claims keep their existing priority.
//! This does not create a University reservation or disable industry across
//! the empire. The existing Campus-building and ordinary production selectors
//! may use the released queue; other claims retain their order. It applies
//! only to an empty or industrial queue on an explicit Science lane before
//! Rocketry, outside Recovery and local attacks. A one-turn industrial head
//! finishes first. Universities taking more than 24 Standard turns (12 Online)
//! leave the industrial claims unchanged. Completed Universities release no
//! claim. Off, every industrial decision and hold remains unchanged.

use super::*;

const UNIVERSITY_STANDARD_TURNS: u32 = 24;

impl AdvancedAi {
    pub(super) fn university_precedes_industry(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.universities_before_the_forge
            || self.active_victory_target(g) != Some(VictoryTarget::Science)
            || plan.strategy == GrandStrategy::Recovery
            || plan.threatened_city == Some(cid)
            || g.players[pid].techs.contains(&crate::name!("rocketry"))
        {
            return false;
        }
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        if city.owner != pid
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
        {
            return false;
        }
        if let Some(head) = city.queue.first() {
            let industrial = match head {
                Item::District { district, .. } => {
                    g.district_family(*district) == "industrial_zone"
                }
                Item::Building { building } => ["workshop"].iter().any(|family| {
                    building.as_str() == *family
                        || g.rules.buildings.get(building).is_some_and(|spec| {
                            spec.replaces.is_some_and(|other| other.as_str() == *family)
                        })
                }),
                _ => false,
            };
            if !industrial || self.production_build_turns(g, pid, cid, head) <= 1.0 {
                return false;
            }
        }
        let Some(university) = BasicAi::civ_building(g, pid, cid, "university") else {
            return false;
        };
        if !g.can_produce(pid, cid, &university) {
            return false;
        }
        let turns = g
            .host_production_turns(cid, &university)
            .unwrap_or_else(|| self.production_build_turns(g, pid, cid, &university));
        let eligible = turns.is_finite()
            && turns > 0.0
            && turns <= f64::from(g.standard_duration(UNIVERSITY_STANDARD_TURNS));
        if eligible {
            think!(self.journal(), Cities, Detail,
                "{} leaves its industrial claim to its University", city.name;
                "universities-before-the-forge: {turns:.1} turns before Rocketry; ordinary claims and urgent queues keep their order");
        }
        eligible
    }
}

#[cfg(test)]
mod tests;
