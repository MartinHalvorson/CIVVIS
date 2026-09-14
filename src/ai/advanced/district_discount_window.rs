//! Prefer a competitive district while its underbuilt-family discount lasts.
//! Research proceeds normally; starting the district locks its actual cost.
//! See docs/eval/2026-09-14-district-discount-window.md.

use super::{AdvancedAi, GrandStrategy, StrategicPlan};
use crate::game::{Game, Item};
use crate::name::Name;
use std::sync::Arc;

impl AdvancedAi {
    pub fn enable_lock_expiring_district_discount(&mut self) {
        self.lock_expiring_district_discount = true;
    }

    pub fn disable_lock_expiring_district_discount(&mut self) {
        self.lock_expiring_district_discount = false;
    }

    /// Credit an expiring price only among ordinary near-equals. A cheap
    /// district that the city does not otherwise want cannot buy priority.
    /// One forecast is shared by the menu; no world is cloned when the gene
    /// is off, no relevant unlock is near completion, or no district competes.
    pub(super) fn adjust_expiring_district_discounts(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        items: &[Item],
        plan: &StrategicPlan,
        scores: &mut [f64],
    ) {
        if !self.lock_expiring_district_discount {
            return;
        }
        debug_assert_eq!(items.len(), scores.len());
        let city = &g.cities[&cid];
        if city.owner != pid
            || !city.queue.is_empty()
            || city.loyalty < 76.0
            || plan.threatened_city == Some(cid)
            || matches!(
                plan.strategy,
                GrandStrategy::Recovery | GrandStrategy::Conquest
            )
            || g.players
                .iter()
                .any(|other| !other.is_minor && g.is_at_war(pid, other.id))
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
        {
            return;
        }
        let player = &g.players[pid];
        let tech = player.research.as_deref().filter(|tech| {
            !player.techs.contains(&Name::new(tech))
                && g.rules.techs.contains_key(tech)
                && player.research_progress >= 0.9 * g.tech_cost(tech)
                && g.rules.districts.values().any(|spec| {
                    spec.specialty
                        && spec
                            .tech
                            .as_ref()
                            .is_some_and(|need| need.as_str() == *tech)
                })
        });
        let civic = player.civic.as_deref().filter(|civic| {
            !player.civics.contains(&Name::new(civic))
                && g.rules.civics.contains_key(civic)
                && player.civic_progress >= 0.9 * g.civic_cost(civic)
                && g.rules.districts.values().any(|spec| {
                    spec.specialty
                        && spec
                            .civic
                            .as_ref()
                            .is_some_and(|need| need.as_str() == *civic)
                })
        });
        if tech.is_none() && civic.is_none() {
            return;
        }
        let best = scores.iter().copied().fold(0.0_f64, f64::max);
        if !best.is_finite() || best <= 0.0 {
            return;
        }
        let candidates: Vec<_> = items
            .iter()
            .zip(scores.iter())
            .enumerate()
            .filter_map(|(i, (item, score))| {
                let Item::District { district, pos } = item else {
                    return None;
                };
                if !score.is_finite()
                    || *score < 0.85 * best
                    || g.map
                        .get(*pos)
                        .is_none_or(|tile| tile.district_foundation.is_some())
                    || !g.can_produce(pid, cid, item)
                {
                    return None;
                }
                let discount = g.district_underbuilt_discount(pid, district, false);
                (discount > 0.0).then_some((i, *district, discount))
            })
            .collect();
        if candidates.is_empty() {
            return;
        }
        let mut forecast = g.clone();
        // A host menu is a quote for the current trees. It must not survive
        // into a hypothetical future quote. Check that the native model can
        // reproduce today's price before trusting its forecast for that item.
        forecast.host_buildable = Arc::default();
        let candidates: Vec<_> = candidates
            .into_iter()
            .filter(|(i, _, _)| {
                (forecast.item_cost_for_city(pid, cid, &items[*i])
                    - g.item_cost_for_city(pid, cid, &items[*i]))
                .abs()
                    <= 1.0
            })
            .collect();
        if let Some(tech) = tech {
            forecast.players[pid].techs.insert(Name::new(tech));
        }
        if let Some(civic) = civic {
            forecast.players[pid].civics.insert(Name::new(civic));
        }
        for (i, district, discount) in candidates {
            let later = forecast.district_underbuilt_discount(pid, &district, false);
            if later >= discount {
                continue;
            }
            let now_cost = g.item_cost_for_city(pid, cid, &items[i]);
            let later_cost = forecast.item_cost_for_city(pid, cid, &items[i]);
            if now_cost <= 0.0 || later_cost <= now_cost {
                continue;
            }
            // At most an 18% preference, so the 85% admission band can only
            // overturn a close decision. Exact engine quotes preserve unique
            // district prices and Government Plaza's smaller discount.
            scores[i] *= (later_cost / now_cost).min(1.18);
        }
    }
}

#[cfg(test)]
mod tests;
