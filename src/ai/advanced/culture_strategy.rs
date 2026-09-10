//! Culture offense builds the Theater chain and opens tourism markets;
//! defense refuses to finance a rival's finish and raises a reachable bar.

use super::{AdvancedAi, GrandStrategy, VictoryTarget};
use crate::game::{Game, Item, QuickDeal};
use std::collections::BTreeSet;

/// An explicit Culture target must keep valuing Theater Squares and their
/// buildings while the temporary war posture is Expansion, Conquest or
/// Recovery. Those postures are allowed to price defense and expansion first,
/// but letting them erase the theater lane leaves the target with no new
/// tourism sources to host its existing chain.
const CULTURE_TARGETED_THEATER_POSTURE_BONUS: f64 = 850.0;

/// Version one prepares at half the culture-victory bar. The Emperor ladder
/// (`docs/civ6_ladder.json`) lost most of its games to a rival culture
/// finish between turns 155 and 208, and one game with that defence in
/// place still lost at t177: by the time a rival shows 50 percent the
/// Theater chain, the policy cards and the trade denial all arrive late.
pub(super) const CULTURE_THREAT_PRESSURE: i32 = 50;

/// `culture-threat-early`: the same defence, armed at 30 percent.
pub(super) const CULTURE_THREAT_PRESSURE_EARLY: i32 = 30;

impl AdvancedAi {
    /// The rival pressure, as a percent of the global culture-victory bar,
    /// at which a rival becomes a defensive threat. Off, version one's 50.
    pub(super) fn culture_threat_pressure(&self) -> i32 {
        if self.culture_threat_early {
            CULTURE_THREAT_PRESSURE_EARLY
        } else {
            CULTURE_THREAT_PRESSURE
        }
    }

    /// Public counters, not score or our own small domestic total. Firaxis's
    /// WorldRankings.lua:1674-1687 compares visitors with the largest rival
    /// staycationer count. Halfway is a preparation threshold, not a forecast
    /// that the rival will win in a particular number of turns.
    pub(super) fn culture_trade_threats(&self, g: &Game, pid: usize) -> BTreeSet<usize> {
        if !self.victory_planning || !g.victory_conditions.culture {
            return BTreeSet::new();
        }
        let bar = self.culture_threat_pressure();
        self.rival_culture_pressures(g)
            .into_iter()
            .filter(|(rival, pressure)| {
                *rival != pid
                    && !g.same_team(pid, *rival)
                    && g.has_met(pid, *rival)
                    && *pressure >= bar
            })
            .map(|(rival, _)| rival)
            .collect()
    }

    /// Keep useful economic trade and purchases available. Selling passage
    /// raises the buyer's incoming tourism modifier, while selling a Great
    /// Work sacrifices our culture as well as transferring tourism to them.
    pub(super) fn culture_deal_safe(deal: &QuickDeal, threats: &BTreeSet<usize>) -> bool {
        if deal.direction != "sell" || threats.is_empty() {
            return true;
        }
        deal.category != "great_work"
            && !(deal.item == "open_borders" && threats.contains(&deal.partner))
    }

    /// `culture-threat-early`: while a threat exists, nothing at all is sold
    /// to the threatening rival. Version one keeps luxury, strategic and
    /// Gold sales open to it, and every one of them finances the finish it
    /// is defending against. Off, exactly [`AdvancedAi::culture_deal_safe`].
    pub(super) fn culture_deal_allowed(&self, deal: &QuickDeal, threats: &BTreeSet<usize>) -> bool {
        Self::culture_deal_safe(deal, threats)
            && !(self.culture_threat_early
                && deal.direction == "sell"
                && threats.contains(&deal.partner))
    }

    /// `culture-threat-early`: a culture threat is a denunciation candidate.
    /// The denouncement starts the Formal War clock the war desk already
    /// reads (`preferred_war_opening`) and costs the rival the friendship and
    /// alliance routes to our market. One per turn, the most pressing rival
    /// first, on the primitive [`AdvancedAi::denounce_most_pressing`] that
    /// `science-threat-denial` shares: the engine's own legality (met, at
    /// peace, not friends or allied, not already denounced) is read from the
    /// diplomacy family rather than assumed. Returns the denounced rival.
    pub(super) fn culture_threat_denunciation(&self, g: &mut Game, pid: usize) -> Option<usize> {
        if !self.culture_threat_early {
            return None;
        }
        let pressures = self.rival_culture_pressures(g);
        let mut ranked: Vec<usize> = self.culture_trade_threats(g, pid).into_iter().collect();
        ranked.sort_by_key(|rival| (-pressures.get(rival).copied().unwrap_or(0), *rival));
        Self::denounce_most_pressing(g, pid, &ranked)
    }

    /// Late tourism defense is useful even when someone else holds the
    /// domestic bar. Only consider censorship once a threatening opponent
    /// can field Rock Bands; its amenity cost is not an opening-game tax.
    pub(super) fn culture_defense_cards(&self, g: &Game, pid: usize) -> Vec<&'static str> {
        let threats = self.culture_trade_threats(g, pid);
        if threats.is_empty() {
            return Vec::new();
        }
        let mut cards = vec!["future_counter_culture"];
        if threats
            .iter()
            .any(|rival| g.players[*rival].civics.contains(&crate::name!("cold_war")))
        {
            cards.push("music_censorship");
        }
        cards
    }

    /// A seat near the largest domestic total can become the defensive bar.
    /// A seat far below it should use diplomacy and its existing culture floor,
    /// not abandon its chosen victory for a culture race it cannot affect.
    pub(super) fn culture_defense_urgency(&self, g: &Game, pid: usize) -> f64 {
        if !self.victory_planning || !g.victory_conditions.culture {
            return 0.0;
        }
        let ours = g.domestic_tourists(pid).max(0) as f64;
        let bar = self.culture_threat_pressure();
        self.rival_culture_pressures(g)
            .into_iter()
            .filter(|(rival, pressure)| {
                *rival != pid
                    && !g.same_team(pid, *rival)
                    && g.has_met(pid, *rival)
                    && *pressure >= bar
            })
            .map(|(rival, pressure)| {
                let bar = g
                    .players
                    .iter()
                    .filter(|p| p.id != rival && p.alive && !p.is_minor && !p.is_barbarian)
                    .map(|p| g.domestic_tourists(p.id))
                    .max()
                    .unwrap_or(1)
                    .max(1) as f64;
                if ours < bar * 0.75 {
                    0.0
                } else {
                    pressure as f64 / 100.0
                }
            })
            .fold(0.0, f64::max)
    }

    pub(super) fn culture_race_production_bonus(
        &self,
        g: &Game,
        pid: usize,
        item: &Item,
        strategy: GrandStrategy,
        turns: f64,
    ) -> f64 {
        if !self.victory_planning || !g.victory_conditions.culture {
            return 0.0;
        }
        // Filter before looking at the world: most production candidates are
        // not cultural, and a wonder is never an emergency culture building.
        let (culture, chain) = match item {
            Item::Building { building } => {
                let spec = &g.rules.buildings[building];
                if spec.wonder {
                    return 0.0;
                }
                let theater = spec
                    .district
                    .is_some_and(|d| g.district_family(d) == "theater_square");
                let chain = if theater {
                    let posture_bonus = if self.active_victory_target(g)
                        == Some(VictoryTarget::Culture)
                        && strategy != GrandStrategy::Culture
                    {
                        CULTURE_TARGETED_THEATER_POSTURE_BONUS
                    } else {
                        0.0
                    };
                    420.0
                        + spec.great_work_slots.values().sum::<i32>().max(0) as f64 * 60.0
                        + ["writer", "artist", "musician"]
                            .iter()
                            .map(|kind| spec.great_person_points.get(*kind).copied().unwrap_or(0.0))
                            .sum::<f64>()
                            * 80.0
                        + posture_bonus
                } else {
                    0.0
                };
                (spec.yields.culture.max(0.0), chain)
            }
            Item::District { district, .. } if g.district_family(*district) == "theater_square" => {
                let posture_bonus = if self.active_victory_target(g) == Some(VictoryTarget::Culture)
                    && strategy != GrandStrategy::Culture
                {
                    CULTURE_TARGETED_THEATER_POSTURE_BONUS
                } else {
                    0.0
                };
                (2.0, 500.0 + posture_bonus)
            }
            _ => return 0.0,
        };
        if culture <= 0.0 && chain <= 0.0 {
            return 0.0;
        }
        let horizon = ((g
            .max_turns
            .min(g.game_speed.turn_limit())
            .saturating_sub(g.turn) as f64
            - turns)
            .max(0.0)
            / g.standard_duration(80).max(1) as f64)
            .clamp(0.0, 1.0);
        let offense = self.active_victory_target(g) == Some(VictoryTarget::Culture)
            || (self.active_victory_target(g).is_none() && strategy == GrandStrategy::Culture);
        let defense = if culture > 0.0 && turns <= g.standard_duration(30).max(1) as f64 {
            self.culture_defense_urgency(g, pid) * (360.0 + culture * 120.0)
        } else {
            0.0
        };
        horizon * if offense { chain.max(defense) } else { defense }
    }

    /// One route per rival unlocks the modifier; extra destinations in the
    /// same empire do not stack it. City-states, teammates, dead players and
    /// our own cities are not tourism markets. Ordinary route yields remain
    /// separately valued for every destination.
    pub(super) fn culture_route_bonus(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        objective: GrandStrategy,
    ) -> f64 {
        if !self.victory_planning {
            return if objective == GrandStrategy::Culture
                && target != pid
                && !g.has_tourism_trade_route(pid, target)
            {
                12.0 + g.tourism_per_turn(pid).min(400.0)
                    * (25.0 + g.policy_effect(pid, "trade_partner_tourism_pct"))
                    / 100.0
            } else {
                0.0
            };
        }
        let other = &g.players[target];
        if objective != GrandStrategy::Culture
            || !g.victory_conditions.culture
            || target == pid
            || !other.alive
            || other.is_minor
            || other.is_barbarian
            || g.same_team(pid, target)
            || g.has_tourism_trade_route(pid, target)
        {
            return 0.0;
        }
        let bar = g
            .players
            .iter()
            .filter(|p| {
                p.id != pid && p.alive && !p.is_minor && !p.is_barbarian && !g.same_team(pid, p.id)
            })
            .map(|p| g.domestic_tourists(p.id))
            .max()
            .unwrap_or(1)
            .max(1) as f64;
        let resistance = (g.domestic_tourists(target).max(0) as f64 / bar).clamp(0.0, 1.0);
        let modifier = (25.0 + g.policy_effect(pid, "trade_partner_tourism_pct")).max(0.0);
        (12.0 + g.tourism_per_turn(pid).min(400.0) * modifier / 100.0) * (1.0 + resistance)
    }
}

#[cfg(test)]
mod tests;
