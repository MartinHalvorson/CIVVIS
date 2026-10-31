//! `unmet-majors-call-for-a-ship`: while a living major is unmet and our
//! ships can cross Ocean, the fastest coastal city puts one sea scout at the
//! head of its queue.
//!
//! **Why (live Immortal, 6 majors, Continents, 2026-10-31).** The operator's
//! lane is now 6-major Continents. The first seat,
//! civvis-20261031T003455Z (Ottoman), never built a ship: no Galley, Caravel
//! or Frigate in any state frame to t229, land scouts only. It met player 3
//! at t16, 2 at t29, 1 at t154 and 5 at t177, and never met 4: player 4
//! won on Science at t229 with us on 17 of the twenty Diplomatic points.
//! - Every Disrupt Rocketry of the game (nine) went to the two met rivals,
//!   while 1, 3 and 4 launched the Exoplanet Expedition: a Spy cannot be
//!   posted to a rival we have not met.
//! - The World Congress offers only met majors as targets, so the denial
//!   vote could not name the Diplomatic leader (it fell back to us until
//!   #4251).
//! - An unmet major takes no delegation, embassy, trade route or alliance.
//! - The sea scout of `naval-recon` exists (`BasicAi::best_naval_recon`), but
//!   `reserve_idle_naval_recon` claims only an IDLE queue, and a live seat's
//!   queues are never idle.
//!
//! **What the gene does.** While a living major is unmet, our ships can
//! cross Ocean (`BasicAi::naval_recon_can_cross_ocean`: Cartography or a
//! civilization ability), and the naval eye is the missing arm
//! (`BasicAi::naval_recon_is_the_missing_arm`: no charting ship of ours at
//! sea or queued, water still unseen, a coastal launch city), the coastal
//! city that builds the cheapest sea scout soonest puts it at the head of its
//! queue. The displaced item keeps its progress. A threatened city, or one
//! attacked in the last four turns, is skipped, and so is the live war
//! economy's recovery. One ship at a time.
//!
//! Off, or with every major met, nothing changes.

use super::*;

impl AdvancedAi {
    /// Whether a living major is still unmet by `pid`.
    pub(super) fn contact_ship_unmet_major(g: &Game, pid: usize) -> bool {
        g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && !g.has_met(pid, other.id)
        })
    }

    /// See the module. Exact no-op with the gene off.
    pub(super) fn unmet_majors_call_for_a_ship(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.unmet_majors_call_for_a_ship
            || !Self::contact_ship_unmet_major(g, pid)
            || !crate::ai::BasicAi::naval_recon_can_cross_ocean(g, pid)
            || !self.base.naval_recon_is_the_missing_arm(g, pid)
            || self.live_war_economy_requires_recovery(g, pid, &self.counts(g, pid))
        {
            return;
        }
        let mut best: Option<(f64, u32, String)> = None;
        for city in g.player_city_ids(pid) {
            let current = &g.cities[&city];
            let threatened = plan.threatened_city == Some(city)
                || (current.last_attacked > 0 && g.turn.saturating_sub(current.last_attacked) <= 4);
            if threatened {
                continue;
            }
            let Some(unit) = self.base.best_naval_recon(g, pid, city) else {
                continue;
            };
            let item = Item::Unit {
                unit: Name::new(&unit),
            };
            let turns = g.item_remaining_cost_for_city(pid, city, &item)
                / g.city_yields(city).production.max(1.0);
            let replace = best.as_ref().is_none_or(|(old_turns, old_city, old_unit)| {
                turns + f64::EPSILON < *old_turns
                    || ((turns - *old_turns).abs() <= f64::EPSILON
                        && (city, unit.as_str()) < (*old_city, old_unit.as_str()))
            });
            if replace {
                best = Some((turns, city, unit));
            }
        }
        let Some((turns, city, unit)) = best else {
            return;
        };
        let item = Item::Unit {
            unit: Name::new(&unit),
        };
        let head = g.cities[&city].queue.first().cloned();
        let city_name = g.cities[&city].name.clone();
        let unmet = g
            .players
            .iter()
            .filter(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && !g.has_met(pid, other.id)
            })
            .count();
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Economy, Decision,
                   "{city_name} starts {} to meet the unmet majors", Self::plain_item(&item);
                   "unmet-majors-call-for-a-ship: {unmet} living major(s) unmet and no sea scout \
                    at sea; {turns:.1} turns, ahead of {}",
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
