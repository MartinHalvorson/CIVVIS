//! `culture-road-builds-its-triggers`: a Culture seat lays the Farms and
//! builds the Market a near civic of its road to Cold War is waiting on, when
//! that costs less than the Culture the Inspiration saves.
//!
//! **Why.** Of the 130 road civics six live Deity Kristina games completed,
//! 27 arrived inspired (21%). `culture-road-earns-its-inspirations` orders the
//! road by when each trigger lands and researches the trigger technology; it
//! cannot make a trigger land. Each road Inspiration was ranked by the Online
//! Culture it saves (40% of half the civic's printed cost) against what its
//! trigger costs, from the installed `Expansion2_Civics.xml` Boosts rows and
//! the state of the same six games at each civic's start:
//!
//! | civic | trigger | Culture | trigger cost | shipped |
//! |---|---|---:|---|---|
//! | Feudalism | 6 Farms | 60 | 1-3 Builder charges (4-9 Farms stood; 3-6 charges in hand) | **Farm premium** |
//! | Guilds | 2 Markets | 84 | 60 Production a Market, Commercial Hubs standing without one in 3 of 6 | **Market claim, one short** |
//! | Mobilization | 3 Corps | 308 | merging pairs the seat holds | order only (gene above) |
//! | Colonialism, Mass Media | Astronomy, Radio | 160, 308 | research order | gene above |
//! | Natural History | Archaeological Museum | 210 | 145 Production | `culture-digs-every-site` (#5001) builds it; ordered by the gene above |
//! | Divine Right (off the road) | 2 Temples | 68 | 120 Production, Holy Sites rare | #5006 buys Shrines and Temples with Gold |
//! | Humanism, Enlightenment, Mercantilism | Great Artist, 3 GP, Great Merchant | 120-144 | Faith patronage of 300-600 | skipped: Faith on Great People, #5001's area |
//! | Medieval Faires | 4 trade routes | 84 | capacity was 1-2 in every game | skipped: needs Commercial Hubs or Harbors |
//! | Recorded History | 2 Campuses | 35 | a district, 22-46 turns early | skipped |
//! | Scorched Earth | 2 Field Cannons | 242 | 330 Production | skipped |
//! | Drama and Poetry | a wonder | 22 | 90+ Production | skipped |
//! | Civil Service, Urbanization, Early Empire | city size 10, 15, empire 6 | 60, 242, 14 | growth | order only |
//! | Political Philosophy, Foreign Trade | 3 city-states, a continent | 22, 8 | exploration | skipped: exploration |
//! | Nationalism, Defensive Tactics | casus belli, a declaration on us | 202, 35 | a war | skipped |
//! | Diplomatic Service | an alliance | 120 | diplomacy | skipped |
//! | Civil Engineering | 7 district kinds | 202 | several districts | skipped |
//! | Cold War | Nuclear Fission | 437 | never legal before Cold War | skipped |
//!
//! **What the gene does**, for an active Culture target only. A road civic is
//! *near* while it is in research, or open with every prerequisite known or in
//! research, and its Inspiration is not in hand.
//!
//! 1. **Farms.** A Builder job that lays the improvement a near civic counts
//!    (Feudalism's Farm) earns [`ROAD_IMPROVEMENT_PREMIUM`] while the empire is
//!    short of it and holds at least as many Builder charges as it is short.
//! 2. **Markets.** When a near civic counts Markets and the empire is exactly
//!    one short, nothing is queued, and the fastest city that can build one
//!    finishes inside [`super::culture_road_inspirations::ROAD_TRIGGER_HORIZON`]
//!    for less Production than the Inspiration's Culture, that city claims a
//!    Market over a deferrable queue head (the Theater claim's own rule).
//!
//! Off, nothing changes.

use super::culture_road_inspirations::ROAD_TRIGGER_HORIZON;
use super::{AdvancedAi, StrategicPlan, VictoryTarget};
use crate::game::{Action, Game, Item};
use crate::name::Name;
use crate::reasoning::plain;
use crate::think;
use crate::Pos;

/// The premium on a Builder job that lays a near civic's counted improvement:
/// above any ordinary tile improvement and a luxury connection (14), below a
/// strategic deposit (30) in `improvement_value_with_appeal`.
pub(super) const ROAD_IMPROVEMENT_PREMIUM: f64 = 20.0;

/// The buildings the claim may queue. A Temple is #5006's (Gold buys Shrines
/// and Temples) and a Museum is #5001's; neither is claimed here.
const ROAD_TRIGGER_BUILDINGS: [&str; 1] = ["market"];

/// The civic whose prerequisite closure is the road.
const ROAD_GOAL: &str = "cold_war";

impl AdvancedAi {
    /// On, an active Culture target, and the lane still open.
    pub(super) fn culture_road_triggers_active(&self, g: &Game) -> bool {
        self.culture_road_builds_its_triggers
            && !self.lane_lost
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
    }

    /// Is `civic` a road civic in research, or open with every prerequisite
    /// known or in research, its Inspiration not in hand?
    pub(super) fn culture_road_trigger_civic_near(
        &self,
        g: &Game,
        pid: usize,
        civic: &str,
    ) -> bool {
        let player = &g.players[pid];
        let Some(spec) = g.rules.civics.get(civic) else {
            return false;
        };
        if spec.boost.is_none()
            || player.civics.contains(&Name::new(civic))
            || Self::boost_in_hand(g, pid, civic, false)
            || !self.civic_leads_to(g, civic, ROAD_GOAL)
        {
            return false;
        }
        player.civic.as_deref() == Some(civic)
            || spec.requires.iter().all(|req| {
                player.civics.contains(req) || player.civic.as_deref() == Some(req.as_str())
            })
    }

    /// The near road civics whose trigger is `trigger`, with how many of it
    /// the empire is still short.
    fn culture_road_trigger_shortfall(
        &self,
        g: &Game,
        pid: usize,
        trigger: &str,
    ) -> Vec<(Name, i64)> {
        g.rules
            .civics
            .iter()
            .filter_map(|(civic, spec)| {
                let boost = spec.boost.as_ref()?;
                if boost.trigger != trigger
                    || !self.culture_road_trigger_civic_near(g, pid, civic.as_str())
                {
                    return None;
                }
                let (have, need) = g.boost_progress(pid, boost);
                let short = need.max(1) - have;
                (short > 0).then_some((*civic, short))
            })
            .collect()
    }

    /// The premium on a Builder job laying `improvement` at `pos`: see the
    /// module. Zero with the gene off.
    pub(super) fn culture_road_builder_premium(
        &self,
        g: &Game,
        pos: Pos,
        improvement: &str,
    ) -> f64 {
        if !self.culture_road_builds_its_triggers {
            return 0.0;
        }
        let Some(pid) = g
            .map
            .get(pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|cid| g.cities.get(&cid))
            .map(|city| city.owner)
        else {
            return 0.0;
        };
        if !self.culture_road_triggers_active(g) {
            return 0.0;
        }
        let trigger = format!("improvement:{improvement}");
        let short = self
            .culture_road_trigger_shortfall(g, pid, &trigger)
            .into_iter()
            .map(|(_, short)| short)
            .max();
        let Some(short) = short else {
            return 0.0;
        };
        let charges: i64 = g
            .units
            .values()
            .filter(|unit| unit.owner == pid && unit.kind == "builder")
            .map(|unit| i64::from(unit.charges.max(0)))
            .sum();
        if charges >= short {
            ROAD_IMPROVEMENT_PREMIUM
        } else {
            0.0
        }
    }

    /// See the module: a near civic one Market short claims the fastest
    /// city's Market. Exact no-op with the gene off or for another target.
    pub(super) fn claim_culture_road_triggers(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.culture_road_triggers_active(g) {
            return;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        });
        let horizon = g.standard_duration(ROAD_TRIGGER_HORIZON) as f64;
        for building in ROAD_TRIGGER_BUILDINGS {
            let family = Name::new(building);
            let item = Item::Building { building: family };
            for (civic, short) in
                self.culture_road_trigger_shortfall(g, pid, &format!("building:{building}"))
            {
                let queued = g
                    .player_city_ids(pid)
                    .into_iter()
                    .filter(|cid| {
                        g.cities[cid].queue.iter().any(|queued| {
                            matches!(queued, Item::Building { building }
                                if g.building_is_family(*building, family))
                        })
                    })
                    .count() as i64;
                if short - queued != 1 {
                    continue;
                }
                let value =
                    Self::boost_frac(g, civic.as_str(), false) * g.civic_cost(civic.as_str());
                let choice = g
                    .player_city_ids(pid)
                    .into_iter()
                    .filter_map(|cid| {
                        let city = &g.cities[&cid];
                        if plan.threatened_city == Some(cid)
                            || (city.last_attacked > 0
                                && g.turn.saturating_sub(city.last_attacked) <= 4)
                            || !g.can_produce(pid, cid, &item)
                        {
                            return None;
                        }
                        if let Some(head) = city.queue.first() {
                            if !Self::culture_theater_may_defer(g, pid, cid, head, at_major_war) {
                                return None;
                            }
                        }
                        let cost = (g.item_cost_for_city(pid, cid, &item)
                            - g.item_invested_production(cid, &item))
                        .max(0.0);
                        let turns = g
                            .host_production_turns(cid, &item)
                            .unwrap_or_else(|| cost / g.city_yields(cid).production.max(0.5));
                        (turns <= horizon && cost <= value).then_some((turns, cost, cid))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.2.cmp(&b.2)));
                let Some((turns, cost, cid)) = choice else {
                    continue;
                };
                if g.apply(
                    pid,
                    &Action::Produce {
                        city: cid,
                        item: item.clone(),
                    },
                )
                .is_ok()
                {
                    think!(self.journal(), Cities, Decision,
                        "{} claims a {} for {}'s inspiration", g.cities[&cid].name, plain(building), plain(&civic);
                        "culture-road-builds-its-triggers: the last {} of {}'s trigger, {:.0} production in \
                         {:.0} turns against {:.0} culture saved",
                        plain(building), plain(&civic), cost, turns, value);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
