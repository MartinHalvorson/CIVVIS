//! Schedule the serial launches and parallel acceleration by completion time.
//! Research and construction overlap; a busy launch pad must not prevent the
//! rest of the empire from preparing or running its own laser stations.

use super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::{Action, Game, Item};
use crate::name::Name;

const LAUNCHES: [&str; 4] = [
    "launch_earth_satellite",
    "launch_moon_landing",
    "launch_mars_colony",
    "exoplanet_expedition",
];
const LASERS: [&str; 2] = ["lagrange_laser_station", "terrestrial_laser_station"];

impl AdvancedAi {
    fn science_endgame_committed(&self, g: &Game, pid: usize) -> bool {
        self.victory_planning
            && g.victory_conditions.science
            && (self.raced_target() == Some(VictoryTarget::Science)
                || self.space_race_lane(g, pid)
                || LAUNCHES
                    .iter()
                    .any(|project| g.players[pid].science_projects.contains(*project)))
    }

    pub(super) fn science_endgame_research_goal(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<&'static str> {
        if !self.science_endgame_committed(g, pid)
            || !(g.players[pid]
                .science_projects
                .contains("launch_moon_landing")
                || Self::science_project_is_queued(g, pid, "launch_moon_landing"))
        {
            return None;
        }
        ["nanotechnology", "smart_materials", "offworld_mission"]
            .into_iter()
            .find(|tech| !g.players[pid].techs.contains(&Name::new(tech)))
    }

    /// Use remaining production, local modifiers and whole turn boundaries.
    /// Keeping the incumbent on a tie avoids wasting already-invested work.
    fn science_endgame_turns(g: &Game, pid: usize, cid: u32, item: &Item) -> f64 {
        g.host_production_turns(cid, item)
            .filter(|turns| turns.is_finite() && *turns >= 0.0)
            .unwrap_or_else(|| Self::science_project_build_turns(g, pid, cid, item))
            .ceil()
            .max(1.0)
    }

    /// The old horizon charges the final project's entire printed cost at
    /// raw city production, even with one turn left in its queue. A concrete
    /// launch plus a flight accelerated by already-unlocked lasers can fit
    /// when that estimate refuses to run the production pass at all.
    pub(super) fn science_endgame_launch_fits(&self, g: &Game, pid: usize) -> bool {
        if !self.science_endgame_committed(g, pid)
            || !g.players[pid]
                .science_projects
                .contains("launch_mars_colony")
        {
            return false;
        }
        let _memo = g.query_memo();
        let cities = g.player_city_ids(pid);
        let launch = Item::Project {
            project: Name::new("exoplanet_expedition"),
        };
        let launch_turns = cities
            .iter()
            .copied()
            .filter(|cid| g.can_produce(pid, *cid, &launch))
            .map(|cid| Self::science_endgame_turns(g, pid, cid, &launch))
            .fold(f64::INFINITY, f64::min);
        let remaining = g.max_turns.saturating_sub(g.turn) as f64;
        if launch_turns >= remaining {
            return false;
        }
        let laser = Item::Project {
            project: Name::new(LASERS[0]),
        };
        let periods: Vec<_> = if g.players[pid]
            .techs
            .contains(&crate::name!("offworld_mission"))
            && g.tree_effect(pid, "laser_station_projects") > 0.0
        {
            cities
                .iter()
                .copied()
                .filter(|cid| g.can_produce(pid, *cid, &launch))
                .map(|cid| {
                    (g.item_cost_for_city(pid, cid, &laser)
                        / (g.city_yields(cid).production.max(0.1)
                            * g.item_prod_mult(pid, cid, Some(&laser)).max(0.1)))
                    .ceil()
                    .max(1.0)
                })
                .collect()
        } else {
            Vec::new()
        };
        let mut distance = 0.0;
        // One completed station per city per turn at most; integer periods
        // conservatively ignore overflow instead of inventing extra launches.
        for flight in 1..=200 {
            let speed = 1.0
                + periods
                    .iter()
                    .map(|period| (flight as f64 / period).floor())
                    .sum::<f64>();
            distance += speed;
            if distance >= g.science_victory_points_needed(pid) {
                return launch_turns + flight as f64 <= remaining;
            }
        }
        false
    }

    /// A legal replacement is required before moving a serial launch. This
    /// banks the old city's investment and prevents duplicate launches from
    /// consuming production until the faster city's project finishes.
    fn science_endgame_support(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let plan = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: g.player_city_ids(pid).len(),
            assessed_turn: g.turn,
            rush: false,
        };
        let counts = self.counts(g, pid);
        g.producible_items(pid, cid)
            .into_iter()
            .filter(|item| match item {
                Item::Project { project } => {
                    !LAUNCHES.contains(&project.as_str()) && !LASERS.contains(&project.as_str())
                }
                Item::District { district, .. } => g.district_family(*district) != "spaceport",
                _ => true,
            })
            .map(|item| {
                let value = self.production_value(g, pid, cid, &item, &plan, &counts);
                (value, item)
            })
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, item)| item)
    }

    /// Returns false for the frozen controller and uncommitted opportunists,
    /// which retain their existing idle-queue production policy.
    pub(super) fn schedule_science_endgame(&self, g: &mut Game, pid: usize) -> bool {
        if !self.science_endgame_committed(g, pid) {
            return false;
        }
        let cities = g.player_city_ids(pid);
        if let Some(project) = LAUNCHES
            .into_iter()
            .find(|project| !g.players[pid].science_projects.contains(*project))
        {
            let item = Item::Project {
                project: Name::new(project),
            };
            let incumbent = cities
                .iter()
                .copied()
                .filter(|cid| g.cities[cid].queue.first() == Some(&item))
                .min_by(|a, b| {
                    Self::science_endgame_turns(g, pid, *a, &item)
                        .total_cmp(&Self::science_endgame_turns(g, pid, *b, &item))
                        .then(a.cmp(b))
                });
            let best = {
                let _memo = g.query_memo();
                cities
                    .iter()
                    .copied()
                    .filter(|cid| g.can_produce(pid, *cid, &item))
                    .min_by(|a, b| {
                        Self::science_endgame_turns(g, pid, *a, &item)
                            .total_cmp(&Self::science_endgame_turns(g, pid, *b, &item))
                            .then_with(|| (Some(*b) == incumbent).cmp(&(Some(*a) == incumbent)))
                            .then(a.cmp(b))
                    })
            };
            if let Some(best) = best {
                if Some(best) != incumbent {
                    // Prepare every displaced queue before changing anything.
                    let replacements: Option<Vec<_>> = cities
                        .iter()
                        .copied()
                        .filter(|cid| *cid != best && g.cities[cid].queue.first() == Some(&item))
                        .map(|cid| {
                            self.science_endgame_support(g, pid, cid)
                                .map(|item| (cid, item))
                        })
                        .collect();
                    if let Some(replacements) = replacements {
                        if g.apply(pid, &Action::Produce { city: best, item }).is_ok() {
                            for (city, item) in replacements {
                                let _ = g.apply(pid, &Action::Produce { city, item });
                            }
                        }
                    }
                }
            }
        } else {
            // Both forms boost the same flight. Consult legality separately:
            // a live host may offer only one of them in a particular city.
            for city in cities {
                let best = LASERS
                    .into_iter()
                    .map(|project| Item::Project {
                        project: Name::new(project),
                    })
                    .filter(|item| g.can_produce(pid, city, item))
                    .min_by(|a, b| {
                        Self::science_endgame_turns(g, pid, city, a)
                            .total_cmp(&Self::science_endgame_turns(g, pid, city, b))
                            .then_with(|| {
                                (g.cities[&city].queue.first() == Some(b))
                                    .cmp(&(g.cities[&city].queue.first() == Some(a)))
                            })
                    });
                if let Some(item) = best {
                    if g.cities[&city].queue.first() != Some(&item) {
                        let _ = g.apply(pid, &Action::Produce { city, item });
                    }
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests;
