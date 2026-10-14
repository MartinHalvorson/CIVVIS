//! Diagnostic journal: why a city did not take a Settler while the empire is
//! short of its settlement target. No decision reads anything written here.
//!
//! **Why (live Immortal, 10-11).** Settler starts over live turns 60-100 fell
//! from a median 3-4.5 a game to 0-2.5 across a run of pins, and replaying the
//! same boards with each pin's binary gave byte-identical orders: the count
//! followed the game, not the code. The governor never said why it passed a
//! Settler over, so the cause stayed unreadable. This line names, for a city
//! whose menu best is not a Settler, the first Settler gate that failed —
//! the pipeline, the city's population, the expansion window, the factory
//! coordination, a practical site — or, when the Settler was admitted, the
//! item that outscored it and both values.
//!
//! One Detail line per city every `SETTLER_ADMISSION_JOURNAL_STRIDE` turns,
//! only below the settlement target. Every question it asks is a `&self`
//! read the Settler arm of `production_value` already asked this turn, so
//! orders are unchanged.

use super::*;

/// Turns between one city's diagnostic lines, so the census reads a sample
/// of every game without a line per city per review.
pub(crate) const SETTLER_ADMISSION_JOURNAL_STRIDE: u32 = 3;

impl AdvancedAi {
    /// Journal why `cid`'s production menu did not put a Settler first while
    /// the empire is short of its settlement target. `items` and `scores` are
    /// the menu exactly as the argmax sees it. Reads only.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn journal_settler_admission(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        counts: &EmpireCounts,
        items: &[Item],
        scores: &[f64],
    ) {
        if !g.turn.is_multiple_of(SETTLER_ADMISSION_JOURNAL_STRIDE)
            || !self.journal().wants(crate::reasoning::Level::Detail)
        {
            return;
        }
        let city_count = g.player_city_ids(pid).len();
        let desired = self.settlement_target(plan);
        if city_count + counts.settlers >= desired {
            return;
        }
        let is_settler = |item: &Item| matches!(item, Item::Unit { unit } if unit == "settler");
        // The menu's own argmax: highest score, ties to the lower Debug key.
        let mut best: Option<(f64, String, &Item)> = None;
        for (item, score) in items.iter().zip(scores) {
            let key = format!("{item:?}");
            let replace = best.as_ref().is_none_or(|(old, old_key, _)| {
                *score > *old + 1e-9 || ((*score - *old).abs() < 1e-9 && key < *old_key)
            });
            if replace {
                best = Some((*score, key, item));
            }
        }
        let Some((best_score, _, best_item)) = best else {
            return;
        };
        if is_settler(best_item) {
            return;
        }
        let settler_score = items
            .iter()
            .zip(scores)
            .find(|(item, _)| is_settler(item))
            .map(|(_, score)| *score);
        let reason = match settler_score {
            Some(score) if score > -1_000.0 => format!(
                "outscored by {} ({best_score:.0} vs {score:.0})",
                Self::plain_item(best_item)
            ),
            _ => self.settler_block_reason(
                g,
                pid,
                cid,
                plan,
                counts,
                city_count,
                desired,
                settler_score.is_some(),
            ),
        };
        let city = &g.cities[&cid];
        think!(self.journal(), Cities, Detail, "Settler held in {}", city.name;
               "{reason}; {city_count} cities and {} settler{} against a target of {desired}",
               counts.settlers, if counts.settlers == 1 { "" } else { "s" });
    }

    /// The first gate of the governor's Settler arm that refuses `cid`, in
    /// the arm's own order: the recovery hold, the pipeline, the population,
    /// the host's menu, the expansion window, the factory coordination, a
    /// practical site, the loyalty check, then the city's own site. The last
    /// answer covers the conquest reservation and a threat-driven recovery
    /// this read cannot see. Every call is one the arm already made this
    /// turn.
    #[allow(clippy::too_many_arguments)]
    fn settler_block_reason(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        counts: &EmpireCounts,
        city_count: usize,
        desired: usize,
        on_menu: bool,
    ) -> String {
        if self.plan_city_target
            && plan.strategy == GrandStrategy::Recovery
            && plan.threatened_city.is_some()
        {
            return "the recovery plan holds Settlers while a city is threatened".to_string();
        }
        let allowed = self.settler_pipeline_width(g, pid, desired, city_count, counts.settlers);
        if counts.settlers >= allowed {
            return format!(
                "the Settler pipeline is full ({} in flight of {allowed})",
                counts.settlers
            );
        }
        let city = &g.cities[&cid];
        if city.pop < 2 {
            return format!("population {} is below the Settler's 2", city.pop);
        }
        if !on_menu {
            return "the host offers no Settler here".to_string();
        }
        if !self.settler_expansion_window_open(g, pid, cid) {
            return "the city's expansion window is closed".to_string();
        }
        if self.settler_factory_coordination
            && !self.settler_factory_is_competitive(g, pid, cid, plan, counts)
        {
            return "another city is the coordinated Settler factory".to_string();
        }
        if !self.base.has_practical_settle_site(g, pid) {
            return "no practical settle site".to_string();
        }
        let settler = Item::Unit {
            unit: crate::name!("settler"),
        };
        if self.settlement_safety {
            let turns = self.production_build_turns(g, pid, cid, &settler);
            if Self::settler_queue_loyalty_risk(g, cid, turns).is_some() {
                return "the new city would be at loyalty risk".to_string();
            }
        }
        if self.settler_site_gate || self.rapid_city_expansion_2 {
            if let Err(hold) = self.settler_site_gate(g, pid, city.pos, counts.settlers) {
                return format!("the site gate holds it: {}", hold.describe());
            }
        } else if self.settler_factory_coordination {
            if self.coordinated_settler_site(g, pid, cid).is_none() {
                return "the coordination offers this city no site".to_string();
            }
        } else if self.best_settle_site(g, pid, city.pos, 11).is_none()
            && (!g.players[pid].techs.contains(&crate::name!("shipbuilding"))
                || self
                    .best_settle_site(g, pid, city.pos, g.map.width + g.map.height)
                    .is_none())
        {
            return "no valued site within reach of this city".to_string();
        }
        "a threat or conquest reservation holds it".to_string()
    }
}

#[cfg(test)]
mod tests;
