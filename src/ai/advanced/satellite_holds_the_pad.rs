//! `satellite-holds-the-pad`: once a Spaceport of ours stands and our Earth
//! Satellite has not landed, the best-producing pad city keeps Launch Earth
//! Satellite at the head of its queue after every ordinary writer and claim.
//!
//! The window between the two shipped holds is open:
//! `launch-pad-holds-its-queue` holds the first pad only until it stands, and
//! `launch-commits-the-endgame` commits the launch schedule only once the
//! Earth Satellite has landed. Live King, 11-01/02, 22 games with a standing
//! pad:
//! - From the pad standing (and the project being legal) to the Satellite
//!   landing took a median 7 turns, but 14 to 30 in four games, and 13 of the
//!   22 pad cities switched off the Satellite twice or more in that window.
//! - civvis-20261102T131016Z (Byzantium) stood its pad at t187 and started
//!   the Satellite at t190, then built a reactor conversion, a Builder, a Jet
//!   Bomber, a Broadcast Center and a Helicopter; the Satellite landed near
//!   t217, the Moon Landing began at t217, and the seat lost Science at t223
//!   while leading the field's Science at t200 (430 against 384).
//! - civvis-20261102T101533Z switched 10 times and took 27 turns.
//!
//! Not when the city is the plan's threatened city, was attacked in the last
//! [`SATELLITE_HOLD_ATTACK_MEMORY`] turns, or can be struck next turn, and
//! not over a head item the host finishes this turn. Off, nothing runs.

use super::*;

/// Turns since the pad city was last attacked under which the hold leaves
/// the queue to the defence.
pub(super) const SATELLITE_HOLD_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// The pad city and the Earth Satellite the hold would put at its head:
    /// `None` with the gene off, once the Satellite has landed, when no
    /// standing pad of ours can build it, or while a pad already leads with
    /// it.
    pub(super) fn satellite_hold(&self, g: &Game, pid: usize) -> Option<(u32, Item)> {
        // One memo for the whole sweep; see `Game::query_memo`.
        let _memo = g.query_memo();
        if !self.satellite_holds_the_pad || self.base.minor || self.base.barb {
            return None;
        }
        if g.players[pid]
            .science_projects
            .contains("launch_earth_satellite")
        {
            return None;
        }
        let item = Item::Project {
            project: crate::name!("launch_earth_satellite"),
        };
        let pads: Vec<u32> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| Self::city_has_spaceport(g, *cid) && g.can_produce(pid, *cid, &item))
            .collect();
        if pads
            .iter()
            .any(|cid| g.cities[cid].queue.first() == Some(&item))
        {
            return None;
        }
        let cid = pads.into_iter().max_by(|left, right| {
            g.city_yields(*left)
                .production
                .total_cmp(&g.city_yields(*right).production)
                .then_with(|| right.cmp(left))
        })?;
        Some((cid, item))
    }

    /// See the module: put the Earth Satellite back at the head of the best
    /// pad city. Exact no-op while the gene is off.
    pub(super) fn claim_satellite_holds_the_pad(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let Some((cid, item)) = self.satellite_hold(g, pid) else {
            return;
        };
        let city = &g.cities[&cid];
        let displaced = city.queue.first().cloned();
        let attacked = city.last_attacked > 0
            && g.turn.saturating_sub(city.last_attacked) <= SATELLITE_HOLD_ATTACK_MEMORY;
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
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            let waits = displaced.map_or_else(
                || "an empty queue".to_string(),
                |head| Self::plain_item(&head),
            );
            think!(self.journal(), Economy, Decision,
                "{} keeps the Earth Satellite at the head", g.cities[&cid].name;
                "satellite-holds-the-pad: our pad stands and the Satellite has not landed; {waits} waits");
        }
    }
}

#[cfg(test)]
mod tests;
