//! `city-state-harvest`: take a soft city-state city beside the empire.
//!
//! ## Why (live Immortal, 2026-11-02 20:00Z-23:40Z, 17 games)
//!
//! The Lv 7 gap is population, not science per citizen. At turn 100 the
//! median rival held 56 citizens in 7 cities against our 36 in 5, while its
//! science per citizen (2.07 against 1.61) sat within the Immortal +24%
//! handicap. None of the 17 games captured a city. At turn 80 the median seat
//! had two city-state cities within eight tiles of its own, at population
//! 5-12 (our own cities averaged about 5.5), whose public military was 18-110
//! against our 240-566. Nothing in the controller aims at one for its own
//! sake: an assigned Domination seat prefers capitals by design
//! (`domination_chooses_a_required_capital_before_a_cheaper_city_state`), and
//! the opening conquest (`early_conquest.rs`) and the city campaign
//! (`city_campaign.rs`) skip minors.
//!
//! A city-state carries no major-civilization handicap (the
//! `HIGH_DIFFICULTY_*` modifiers attach to `TRAIT_LEADER_MAJOR_CIV`), so its
//! garrison fights at par; at Immortal and above it starts walled
//! (`Eras.xml`: `MinorOnly="true" MinDifficulty="DIFFICULTY_IMMORTAL"
//! Building="BUILDING_WALLS"`).
//!
//! ## What it does
//!
//! While no other overlay owns the war (no timed-war appointment), no war
//! with a major is running and the empire is past [`HARVEST_FROM_TURN`] with
//! two cities, it names the best city-state city that is:
//!
//! - owned by a city-state we have MET, or by the Free Cities (a city that
//!   rebelled on loyalty: across 72 recorded runs of 11-02, 24 of our cities
//!   flipped and 8 were taken back), its centre EXPLORED (fog-honest: the
//!   `players[pid].explored` idiom `early_conquest.rs` uses);
//! - within [`HARVEST_REACH_TILES`] of one of our cities;
//! - at population [`HARVEST_MIN_POP`] or more;
//! - held by a city-state with no suzerain but us, at most
//!   [`HARVEST_MAX_OUR_ENVOYS`] of our envoys (an invested suzerainty is
//!   worth more than the city: a scientific city-state with nine of our
//!   envoys pays about +16 Science), and public military at most
//!   [`HARVEST_MAX_POWER_SHARE`] of ours.
//!
//! The named city becomes the plan's Conquest target for the window
//! ([`AdvancedAi::apply_city_state_harvest_to_strategy`], the same overlay
//! shape as `apply_war_plan_to_strategy`), so the shipped declaration gate,
//! staging ring, siege train and breaker purchase do the work. The name is
//! released when the city changes hands, when a major is at war with us, when
//! it was not declared on within [`HARVEST_DECLARE_PATIENCE`] turns, or when
//! the war has run [`HARVEST_SIEGE_PATIENCE`] turns without the capture. A
//! release without a capture waits [`HARVEST_COOLDOWN`] turns before another
//! name.
//!
//! Off, every entry point returns before reading the board.

use super::*;
use crate::think;

/// The earliest turn a city-state is named (Online speed).
pub(crate) const HARVEST_FROM_TURN: u32 = 40;
/// Tiles from the nearest of our cities to the city-state city.
pub(crate) const HARVEST_REACH_TILES: i32 = 6;
/// The city-state city's least population worth a war.
pub(crate) const HARVEST_MIN_POP: i32 = 4;
/// Our envoys at the city-state above which its suzerainty is kept.
pub(crate) const HARVEST_MAX_OUR_ENVOYS: i64 = 1;
/// The city-state's public military as a share of ours.
pub(crate) const HARVEST_MAX_POWER_SHARE: f64 = 0.35;
/// Turns from the name to the declaration before the name is released.
pub(crate) const HARVEST_DECLARE_PATIENCE: u32 = 15;
/// Turns from the declaration to the capture before the name is released.
pub(crate) const HARVEST_SIEGE_PATIENCE: u32 = 20;
/// Turns after a release without a capture before another name.
pub(crate) const HARVEST_COOLDOWN: u32 = 25;

/// The city-state city the harvest is aimed at.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HarvestTarget {
    pub(crate) player: usize,
    pub(crate) city: u32,
    pub(crate) named_turn: u32,
    pub(crate) declared_turn: Option<u32>,
}

impl AdvancedAi {
    fn harvest_count(g: &mut Game, pid: usize, key: &str) {
        *g.players[pid]
            .counters
            .entry(format!("city_state_harvest:{key}"))
            .or_insert(0) += 1;
    }

    fn harvest_major_war(g: &Game, pid: usize) -> bool {
        g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && g.is_at_war(pid, player.id)
        })
    }

    /// The softest qualifying city-state city, or `None`.
    pub(crate) fn city_state_harvest_candidate(&self, g: &Game, pid: usize) -> Option<(usize, u32)> {
        let ours = g.player_city_ids(pid);
        if ours.len() < 2 {
            return None;
        }
        let our_power = g.military_power(pid);
        if our_power <= 0.0 {
            return None;
        }
        let mut best: Option<(f64, usize, u32)> = None;
        // A Free Cities seat carries `is_barbarian` and is always at war; a
        // city of ours that rebelled into it is the cheapest city on the
        // board (live Immortal 221554Z: Okayama, population 7, flipped at
        // turn 90 with no defender and was never retaken).
        for minor in g
            .players
            .iter()
            .filter(|p| p.alive && ((p.is_minor && !p.is_barbarian) || p.is_free_city))
        {
            let m = minor.id;
            if !g.has_met(pid, m) {
                continue;
            }
            if g.suzerain_of(m).is_some_and(|suzerain| suzerain != pid) {
                continue;
            }
            if g.envoys_at(pid, m) > HARVEST_MAX_OUR_ENVOYS {
                continue;
            }
            if g.military_power(m) > HARVEST_MAX_POWER_SHARE * our_power {
                continue;
            }
            if !g.is_at_war(pid, m) && !self.campaign_target_legal(g, pid, m) {
                continue;
            }
            for cid in g.player_city_ids(m) {
                let city = &g.cities[&cid];
                if city.pop < HARVEST_MIN_POP || !g.players[pid].explored.contains(&city.pos) {
                    continue;
                }
                let distance = ours
                    .iter()
                    .map(|own| g.wdist(g.cities[own].pos, city.pos))
                    .min()
                    .unwrap_or(i32::MAX);
                if distance > HARVEST_REACH_TILES {
                    continue;
                }
                // Population is the prize; distance and walls are the bill.
                let score = city.pop as f64 * 10.0
                    - distance as f64 * 4.0
                    - city.wall_hp.max(0) as f64 / 20.0
                    - g.military_power(m) / our_power * 30.0;
                let better = best.as_ref().map_or(true, |(top, top_m, top_c)| {
                    score > *top || (score == *top && (m, cid) < (*top_m, *top_c))
                });
                if better {
                    best = Some((score, m, cid));
                }
            }
        }
        best.map(|(_, m, cid)| (m, cid))
    }

    /// Name, keep or release the harvest target, before the plan is read.
    pub(crate) fn maintain_city_state_harvest(&mut self, g: &mut Game, pid: usize) {
        if !self.city_state_harvest {
            return;
        }
        if let Some(mut target) = self.city_state_harvest_target.take() {
            let owner = g.cities.get(&target.city).map(|city| city.owner);
            let at_war = g.is_at_war(pid, target.player);
            if at_war && target.declared_turn.is_none() {
                target.declared_turn = Some(g.turn);
                Self::harvest_count(g, pid, "declared");
            }
            let release = if owner == Some(pid) {
                Self::harvest_count(g, pid, "captured");
                self.city_state_harvest_cooldown = 0;
                Some("the city is ours")
            } else if owner != Some(target.player) {
                Some("the city changed hands")
            } else if Self::harvest_major_war(g, pid) {
                Some("a major is at war with us")
            } else if target.declared_turn.is_none()
                && g.turn >= target.named_turn + HARVEST_DECLARE_PATIENCE
            {
                Some("no declaration within the patience")
            } else if target
                .declared_turn
                .is_some_and(|declared| g.turn >= declared + HARVEST_SIEGE_PATIENCE)
            {
                Some("the siege ran out of patience")
            } else {
                None
            };
            if let Some(why) = release {
                if owner != Some(pid) {
                    Self::harvest_count(g, pid, "released");
                    self.city_state_harvest_cooldown = g.turn + HARVEST_COOLDOWN;
                }
                if self.journal().wants(crate::reasoning::Level::Strategy) {
                    let name = g
                        .cities
                        .get(&target.city)
                        .map(|city| city.name.clone())
                        .unwrap_or_default();
                    think!(self.journal(), Military, Strategy,
                           "City-state harvest releases {}", name;
                           "{why}, {} turns after it was named", g.turn - target.named_turn);
                }
                self.plan = None;
            } else {
                self.city_state_harvest_target = Some(target);
            }
            return;
        }
        if g.turn < HARVEST_FROM_TURN
            || g.turn < self.city_state_harvest_cooldown
            || self.war_plan.is_some()
            || Self::harvest_major_war(g, pid)
            || g.emergency_objective(pid).is_some()
        {
            return;
        }
        let Some((player, city)) = self.city_state_harvest_candidate(g, pid) else {
            return;
        };
        Self::harvest_count(g, pid, "named");
        if self.journal().wants(crate::reasoning::Level::Strategy) {
            let c = &g.cities[&city];
            think!(self.journal(), Military, Strategy,
                   "City-state harvest names {}", c.name;
                   "pop {}, walls {}, its military {:.0} against our {:.0}, {} of our envoys",
                   c.pop, c.wall_hp, g.military_power(player), g.military_power(pid),
                   g.envoys_at(pid, player); c.pos);
        }
        self.city_state_harvest_target = Some(HarvestTarget {
            player,
            city,
            named_turn: g.turn,
            declared_turn: g.is_at_war(pid, player).then_some(g.turn),
        });
        self.plan = None;
    }

    /// The overlay: while a harvest is named, the plan is a Conquest of that
    /// city. A home emergency (`Recovery`) and a timed-war appointment keep
    /// their own posture.
    pub(crate) fn apply_city_state_harvest_to_strategy(&self, plan: &mut StrategicPlan) {
        if !self.city_state_harvest || self.war_plan.is_some() {
            return;
        }
        let Some(target) = &self.city_state_harvest_target else {
            return;
        };
        if plan.strategy != GrandStrategy::Recovery {
            plan.strategy = GrandStrategy::Conquest;
            plan.target_player = Some(target.player);
            plan.target_city = Some(target.city);
            plan.rush = false;
        }
    }
}

#[cfg(test)]
mod tests;
