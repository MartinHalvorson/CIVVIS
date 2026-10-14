//! Independently screenable regional loyalty support for governed cities.
//! Guayaquil already had Magnus but rebelled at t120 in the 20260928T032649Z
//! run. Titles arriving separately at t102 and t108 both went to Pingala.

use super::AdvancedAi;
use crate::game::{Action, Game};

impl AdvancedAi {
    pub fn enable_regional_loyalty_governor(&mut self) {
        self.regional_loyalty_governor = true;
    }

    pub fn disable_regional_loyalty_governor(&mut self) {
        self.regional_loyalty_governor = false;
    }

    /// Return the next action and the titles the complete rescue requires.
    /// Saving a single title avoids starting a two-title response only after
    /// the ordinary promotion ladder has already spent its first half.
    pub(super) fn regional_loyalty_governor_action(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<(Action, usize)> {
        if !self.regional_loyalty_governor {
            return None;
        }
        let spec = g.rules.governors.get("victor")?;
        let bonus = *spec
            .promotions
            .get("garrison_commander")?
            .effects
            .get("nearby_city_loyalty")?;
        if bonus <= 0.0 {
            return None;
        }
        let victor = g.players[pid].governor_roster.get("victor");
        if victor.is_some_and(|state| state.promotions.contains("garrison_commander")) {
            // Its existing or pending aura must not be counted a second time.
            return None;
        }
        // Use the slower of native/base and simulated establishment clocks.
        // A live mirror's observed loyalty rate does not recompute after a
        // hypothetical appointment, so price only the documented extra aura.
        let duration = spec
            .establish_turns
            .max(g.standard_duration(spec.establish_turns));
        let delay = victor.map_or(duration, |state| {
            state
                .assigned_turn
                .saturating_add(duration)
                .max(state.disabled_until)
                .saturating_sub(g.turn)
        });
        let endangered = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                let city = &g.cities[cid];
                let rate = g.city_loyalty_per_turn(city);
                g.players[pid].governors.contains(cid)
                    && city.loyalty <= 50.0
                    && rate < -f64::EPSILON
                    && rate + bonus >= 0.0
                    && city.loyalty + rate * f64::from(delay) > 0.0
            })
            .collect::<Vec<_>>();
        if endangered.is_empty() {
            return None;
        }
        let covers = |anchor: u32| {
            let city = g.cities.get(&anchor)?;
            if city.owner != pid {
                return None;
            }
            endangered
                .iter()
                .filter(|cid| g.wdist(city.pos, g.cities[cid].pos) <= 9)
                .map(|cid| {
                    let target = &g.cities[cid];
                    target.loyalty / -g.city_loyalty_per_turn(target)
                })
                .min_by(f64::total_cmp)
        };
        if let Some(state) = victor {
            covers(state.city?)?;
            if !g
                .available_governor_promotions(pid, "victor")
                .iter()
                .any(|promotion| promotion == "garrison_commander")
            {
                return None;
            }
            return Some((
                Action::PromoteGovernor {
                    governor: "victor".into(),
                    promotion: "garrison_commander".into(),
                },
                1,
            ));
        }
        // Do not displace another governor or take loyalty from an existing
        // post. Prefer a stable, ungoverned domestic city covering the city
        // with least remaining headroom; all distance ties are deterministic.
        let anchor = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| !g.players[pid].governors.contains(cid))
            .filter(|cid| g.city_loyalty_per_turn(&g.cities[cid]) >= 0.0)
            .filter_map(|cid| covers(cid).map(|headroom| (cid, headroom)))
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))?
            .0;
        Some((
            Action::AppointGovernor {
                governor: "victor".into(),
                city: anchor,
            },
            2,
        ))
    }
}

#[cfg(test)]
mod tests;
