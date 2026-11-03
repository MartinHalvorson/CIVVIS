//! `launch-pad-holds-its-queue`: the empire's unfinished first Spaceport goes
//! back to the head of its city's queue after every ordinary writer and claim,
//! and the Spy trainer looks for another city.
//!
//! `launch-pad-keeps-its-rung` (live) prices the placed pad at the first-pad
//! rung again and resumes it ahead of the delegated governor's economy steps.
//! The claims that run outside those steps still take the queue, and the Spy
//! trainer picks the top-Production city, which is the launch city.
//! `science_denial_trains_spies` counts any district at the head as routine
//! work a Spy may displace, the pad included.
//!
//! Live King, 2026-11-01, the pad city's queue between the pad's placement
//! and its finish:
//! - civvis-20261101T085353Z (Religion): Mbwila placed the pad at t175 and
//!   left it at t180 for a Spy ("science-denial-trains-spies: 3 held and 0
//!   queued against 4 needed; 68 Production a turn here"), then a reactor
//!   conversion, a Modern AT for the air surge and Campus Research Grants.
//!   It was back on the pad only from t195, by the Great Person activation
//!   path, and finished at t206: 32 turns for a 900-point pad at 50-69 a turn.
//!   The Earth Satellite landed at 215 against Khmer's 177.
//! - civvis-20261101T081602Z (Culture): Rome placed the pad at t191 and
//!   trained Spies from t193 on; no Earth Satellite by the loss at 241.
//! - civvis-20261101T091422Z-cont5 (Culture, with `launch-pad-keeps-its-rung`):
//!   Jönköping held 811 of 900 at t203 when it trained a Spy against Rome's
//!   space race; `paused-district-finishes` brought the pad back at t205.
//! - civvis-20261101T063953Z (Culture): Stirling had worked its pad at 34 a
//!   turn from t193 when a Neighborhood took the queue at t215; the pad never
//!   finished.
//!
//! While no pad of ours stands, the city carrying the first one
//! (`unfinished_launch_pad_site`) has that pad put back at the
//! head of its queue. It runs after every ordinary writer and claim and
//! before the surprise-defence, declaration and loyalty handoffs, which keep
//! the last word. Not when the city is the plan's threatened city, was
//! attacked in the last `PAD_HOLD_ATTACK_MEMORY` turns, or can be struck
//! next turn, and not over a head item the host finishes this turn. The Spy
//! trainer skips that city, so the Spy is trained in the next-best one rather
//! than queued in the pad city and never finished. A second pad gains
//! nothing, and the hold ends once a pad stands.

use super::*;

/// Turns since the pad city was last attacked under which the hold leaves the
/// queue to the defence. The Spy claim's memory.
pub(super) const PAD_HOLD_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// Whether `cid` carries the first pad the gene holds, before any guard;
    /// the Spy trainer leaves that city's queue alone. `false` with the gene
    /// off.
    pub(super) fn launch_pad_holds_city(&self, g: &Game, pid: usize, cid: u32) -> bool {
        self.launch_pad_holds_its_queue && Self::unfinished_launch_pad_site(g, pid) == Some(cid)
    }

    /// The first pad's city and its item: the Spaceport in the city's queue,
    /// else its foundation on one of the city's tiles. `None` with the gene
    /// off, once a pad stands, or before one is begun.
    pub(super) fn launch_pad_hold(&self, g: &Game, pid: usize) -> Option<(u32, Item)> {
        if !self.launch_pad_holds_its_queue || self.base.minor || self.base.barb {
            return None;
        }
        let cid = Self::unfinished_launch_pad_site(g, pid)?;
        let city = g.cities.get(&cid)?;
        let queued = city.queue.iter().find(|item| {
            matches!(item, Item::District { district, .. } if g.district_family(*district) == "spaceport")
        });
        let item = queued.cloned().or_else(|| {
            city.owned_tiles.iter().find_map(|pos| {
                let foundation = g.map.tiles.get(pos)?.district_foundation.as_ref()?;
                (g.district_family(foundation.district) == "spaceport").then_some(Item::District {
                    district: foundation.district,
                    pos: *pos,
                })
            })
        })?;
        g.can_produce(pid, cid, &item).then_some((cid, item))
    }

    /// See the module: put the first pad back at the head of its city's
    /// queue. Exact no-op while the gene is off.
    pub(super) fn claim_launch_pad_holds_its_queue(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let Some((cid, pad)) = self.launch_pad_hold(g, pid) else {
            return;
        };
        let city = &g.cities[&cid];
        let displaced = city.queue.first().cloned();
        if displaced
            .as_ref()
            .is_some_and(|item| self.launch_pad_spares_repair(g, pid, cid, item))
        {
            return;
        }
        if displaced.as_ref() == Some(&pad) {
            return;
        }
        let attacked = city.last_attacked > 0
            && g.turn.saturating_sub(city.last_attacked) <= PAD_HOLD_ATTACK_MEMORY;
        if plan.threatened_city == Some(cid)
            || attacked
            || Self::imminent_city_attack(g, pid, cid, &g.player_vision_frame(pid))
        {
            return;
        }
        // The head the host finishes this turn keeps its turn.
        if let Some(head) = displaced.as_ref() {
            if g.host_production_turns(cid, head)
                .is_some_and(|turns| turns.is_finite() && turns > 0.0 && turns <= 1.0)
            {
                return;
            }
        }
        let turns = g
            .host_production_turns(cid, &pad)
            .filter(|turns| turns.is_finite() && *turns > 0.0);
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: pad.clone(),
            },
        )
        .is_ok()
        {
            let waits = displaced.map_or_else(
                || "an empty queue".to_string(),
                |item| Self::plain_item(&item),
            );
            let turns = turns.map_or_else(|| "unquoted".to_string(), |turns| format!("{turns:.0}"));
            think!(self.journal(), Economy, Decision,
                "{} keeps its Spaceport at the head", g.cities[&cid].name;
                "launch-pad-holds-its-queue: the empire's first pad, {turns} host turn(s) left; {waits} waits");
        }
    }
}

#[cfg(test)]
mod tests;
