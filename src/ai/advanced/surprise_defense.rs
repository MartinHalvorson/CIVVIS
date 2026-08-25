//! Rapid defensive mobilization after a surprise declaration.
//!
//! A standing war posture is too expensive: the removed broad Conquest
//! production route lost in every measured regime because cities kept paying
//! for soldiers after the urgent moment had passed. The opposite failure is
//! just as concrete, though. A surprise-war target enters the war with the
//! queues, policy deck, treasury and old units of its peaceful plan, while the
//! declarer has already chosen the timing.
//!
//! This gene buys back only that timing edge. For six Standard-speed turns
//! after an active `surprise_war` record names this seat as its target it:
//!
//! - buys at most one land defender in the city nearest the declared attacker
//!   before ordinary strategic spending;
//! - upgrades the standing force while the engine's wartime Gold floor is in
//!   force;
//! - puts the available infantry/ranged and cavalry production cards at the
//!   front of the policy portfolio; and
//! - redirects at most half the empire's cities per turn toward the fastest
//!   credible land defenders until live plus queued land force reaches two per
//!   city.
//!
//! Settler queues are deliberately immune. A surprise war must not silently
//! erase the early expansion policy this controller is meant to retain.
//! Repairs, walls, local defenders and any item one turn from completion are
//! also preserved; switched production is banked by the engine and resumes
//! after the shock. The exact declaration metadata, defender direction and
//! hard time limit make the off-ramp structural rather than advisory.
//!
//! Off by default: registry row `surprise-war-mobilization`.

use super::{AdvancedAi, BasicAi};
use crate::game::{Action, Game, Item};

/// The declaration shock in Standard-speed turns. Six is long enough for the
/// first policy-boosted defender to finish in an ordinary early city, while
/// remaining far shorter than the engine's minimum-war and fatigue clocks.
pub(crate) const SURPRISE_MOBILIZATION_TURNS: u32 = 6;
/// Do not seize more than this many fresh queues in one acting turn. The
/// empire may add another wave next turn if it is still below the force floor.
pub(crate) const SURPRISE_MOBILIZATION_CITY_CAP: usize = 4;
/// A fast obsolete body is not a fast defense. Admit units within this share
/// of the strongest land defender the city can currently train, then choose
/// the quickest among them.
pub(crate) const SURPRISE_DEFENDER_POWER_FLOOR: f64 = 0.80;
/// Immediate purchases leave only the same small treasury floor used by the
/// established wartime upgrade pass.
pub(crate) const SURPRISE_PURCHASE_RESERVE: f64 = 30.0;

/// The one declaration currently authorizing the bounded response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SurpriseDefenseWindow {
    pub(crate) attacker: usize,
    pub(crate) declared: u32,
    pub(crate) ends: u32,
}

impl AdvancedAi {
    /// Return the newest active surprise declaration made *against* this seat.
    /// A war we declared, a formal/casus-belli war, an allied front whose named
    /// target is somebody else, or an old war outside the opening window is
    /// intentionally inert.
    pub(crate) fn surprise_defense_window(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<SurpriseDefenseWindow> {
        if !self.surprise_war_mobilization || self.base.minor || self.base.barb {
            return None;
        }
        let duration = g.standard_duration(SURPRISE_MOBILIZATION_TURNS).max(1);
        g.wars
            .values()
            .filter(|war| {
                war.ended.is_none()
                    && war.target == pid
                    && war.declarer != pid
                    && war.casus_belli.as_deref() == Some("surprise_war")
                    && g.players.get(war.declarer).is_some_and(|attacker| {
                        attacker.alive && !attacker.is_minor && !attacker.is_barbarian
                    })
                    && g.is_at_war(pid, war.declarer)
                    && g.turn < war.started.saturating_add(duration)
            })
            .max_by_key(|war| war.started)
            .map(|war| SurpriseDefenseWindow {
                attacker: war.declarer,
                declared: war.started,
                ends: war.started.saturating_add(duration),
            })
    }

    fn surprise_land_force(&self, g: &Game, pid: usize) -> usize {
        let counts = self.counts(g, pid);
        counts
            .military
            .saturating_sub(counts.naval + counts.aircraft)
    }

    fn surprise_land_target(g: &Game, pid: usize) -> usize {
        g.player_city_ids(pid).len().max(1).saturating_mul(2)
    }

    /// Distance from a city to the closest land asset of the civilization
    /// that chose the declaration timing. Under fog-honest planning hidden
    /// units are absent, so known cities become the stable fallback.
    fn surprise_front_distance(g: &Game, attacker: usize, city: u32) -> i32 {
        let origin = g.cities[&city].pos;
        g.player_unit_ids(attacker)
            .into_iter()
            .filter_map(|unit| {
                let unit = &g.units[&unit];
                let spec = &g.rules.units[unit.kind];
                (spec.class == "military" && !matches!(spec.domain.as_deref(), Some("sea" | "air")))
                    .then_some(g.wdist(origin, unit.pos))
            })
            .chain(
                g.player_city_ids(attacker)
                    .into_iter()
                    .map(|enemy| g.wdist(origin, g.cities[&enemy].pos)),
            )
            .min()
            .unwrap_or(i32::MAX / 4)
    }

    /// The quickest credible local defender. Infantry, anti-cavalry and
    /// ranged bodies are preferred because one policy family accelerates all
    /// three; cavalry remains a fallback when it is the only credible body.
    fn surprise_fast_defender(&self, g: &Game, pid: usize, city: u32) -> Option<(f64, Item)> {
        let candidates: Vec<(Item, f64, bool)> = g
            .producible_items(pid, city)
            .into_iter()
            .filter_map(|item| {
                let Item::Unit { unit } = &item else {
                    return None;
                };
                let spec = &g.rules.units[unit];
                if spec.class != "military"
                    || spec.siege
                    || matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    || (!spec.is_melee_capable() && !spec.has_ranged_attack())
                {
                    return None;
                }
                let power = spec.strength.max(spec.ranged_attack_strength());
                Some((item, power, spec.cavalry))
            })
            .collect();
        let strongest = candidates
            .iter()
            .map(|(_, power, _)| *power)
            .fold(0.0_f64, f64::max);
        if strongest <= 0.0 {
            return None;
        }
        let credible = strongest * SURPRISE_DEFENDER_POWER_FLOOR;
        let has_non_cavalry = candidates
            .iter()
            .any(|(_, power, cavalry)| *power + f64::EPSILON >= credible && !cavalry);
        let production = g.city_yields(city).production.max(0.1);
        candidates
            .into_iter()
            .filter(|(_, power, cavalry)| {
                *power + f64::EPSILON >= credible && (!has_non_cavalry || !cavalry)
            })
            .map(|(item, power, _)| {
                let turns = g.item_remaining_cost_for_city(pid, city, &item) / production;
                (turns, -power, format!("{item:?}"), item)
            })
            .min_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1.total_cmp(&right.1))
                    .then_with(|| left.2.cmp(&right.2))
            })
            .map(|(turns, _, _, item)| (turns, item))
    }

    /// Buy one body where it can answer the declared attacker. This runs
    /// before ordinary strategic purchases and returns whether it consumed
    /// that turn's purchase lane.
    pub(crate) fn surprise_defense_purchase(&self, g: &mut Game, pid: usize) -> bool {
        let Some(window) = self.surprise_defense_window(g, pid) else {
            return false;
        };
        if self.surprise_land_force(g, pid) >= Self::surprise_land_target(g, pid) {
            return false;
        }
        let counts = self.counts(g, pid);
        let want_ranged = counts.melee > counts.ranged;
        let mut cities = g.player_city_ids(pid);
        cities.sort_by_key(|city| {
            (
                Self::surprise_front_distance(g, window.attacker, *city),
                *city,
            )
        });
        cities.into_iter().any(|city| {
            self.base
                .buy_gold_military(g, pid, &[city], SURPRISE_PURCHASE_RESERVE, want_ranged)
        })
    }

    /// Move modernization ahead of ordinary spending during the declaration
    /// shock. It remains safe to run the ordinary late pass as well: upgrades
    /// are consumed and therefore the second pass is an exact no-op.
    pub(crate) fn surprise_defense_modernize(&self, g: &mut Game, pid: usize) {
        if self.surprise_defense_window(g, pid).is_some() {
            BasicAi::upgrade_units(g, pid);
        }
    }

    /// Put the production accelerators and then maintenance relief ahead of a
    /// peaceful victory portfolio for the declaration window only. Successor
    /// cards appear first; the existing policy chooser skips locked cards and
    /// removes obsolete predecessors.
    pub(crate) fn prioritize_surprise_defense_policies(
        &self,
        g: &Game,
        pid: usize,
        desired: &mut Vec<&str>,
    ) {
        if self.surprise_defense_window(g, pid).is_none() {
            return;
        }
        const MOBILIZATION: [&str; 11] = [
            "military_first",
            "grande_armee",
            "feudal_contract",
            "agoge",
            "lightning_warfare",
            "chivalry",
            "maneuver",
            "levee_en_masse",
            "conscription",
            "bastions",
            "retainers",
        ];
        desired.retain(|card| !MOBILIZATION.contains(card));
        desired.splice(0..0, MOBILIZATION);
    }

    /// Redirect the cities closest to the declared attacker, up to half the
    /// empire (and four) in one turn. The target counts queued bodies, so this
    /// cannot refill every queue every turn; it adds only the still-missing
    /// wave.
    pub(crate) fn mobilize_surprise_defense_production(&self, g: &mut Game, pid: usize) -> usize {
        let Some(window) = self.surprise_defense_window(g, pid) else {
            return 0;
        };
        let city_ids = g.player_city_ids(pid);
        let gap =
            Self::surprise_land_target(g, pid).saturating_sub(self.surprise_land_force(g, pid));
        let limit = gap.min(
            city_ids
                .len()
                .div_ceil(2)
                .clamp(1, SURPRISE_MOBILIZATION_CITY_CAP),
        );
        if limit == 0 {
            return 0;
        }

        let mut candidates = Vec::new();
        for city in city_ids {
            if let Some(committed) = g.cities[&city].queue.first() {
                // Expansion is an explicit retained objective, even in the
                // early declaration window.
                if matches!(committed, Item::Unit { unit } if unit == "settler")
                    || Self::active_queue_is_defensive(g, committed)
                {
                    continue;
                }
                let production = g.city_yields(city).production.max(0.1);
                if g.item_remaining_cost_for_city(pid, city, committed) <= production + f64::EPSILON
                {
                    continue;
                }
            }
            let Some((turns, item)) = self.surprise_fast_defender(g, pid, city) else {
                continue;
            };
            candidates.push((
                Self::surprise_front_distance(g, window.attacker, city),
                turns,
                city,
                format!("{item:?}"),
                item,
            ));
        }
        candidates.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.total_cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| left.3.cmp(&right.3))
        });

        let mut changed = 0;
        for (_, _, city, _, item) in candidates.into_iter().take(limit) {
            if g.apply(pid, &Action::Produce { city, item }).is_ok() {
                changed += 1;
            }
        }
        changed
    }
}
