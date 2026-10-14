//! `war-target-prices-the-walls`: at peace, the Domination campaign names
//! the rival whose capital our FIELDED siege guns take soonest, not the one
//! the elective ranking reaches first.
//!
//! A rival's cities stand behind 400 walls from its Steel (Urban Defenses),
//! and against that pool our guns read 2-12 a shot at defenses of 80-125.
//! Over the 15 strong Immortal games of 10-12/13 the decisive siege opened
//! 7-44 turns after its target had 400 walls in 8 of 13, while 9 of the 15
//! games still held a rival under 400 walls at turn 150, where the same guns
//! read 14-36 a shot. Live Immortal 074455Z besieged Edinburgh (Scotland,
//! Steel at 113) from turn 133 while Gaul's cities stood at 100 walls to
//! turn 150.
//!
//! The reading is visible: a seen (or last-seen) city's wall pool, its
//! defensive strength, and our best fielded siege unit's blow. A capture is
//! priced as the land march from our nearest city plus the breach at that
//! blow by our fielded guns; a capital we cannot reach by land counts only
//! with an amphibious train armed, at [`WALL_PRICE_CROSSING_TURNS`]. The
//! pick runs only at peace, behind every counter and contender, and keeps
//! a train already mustered on the plan's objective.

use super::siege_train::{SiegeStage, MUSTER_BREACH_FAR};
use super::{AdvancedAi, VictoryTarget};
use crate::game::{expected_damage, Game};
use crate::Pos;
use std::collections::{BTreeMap, VecDeque};

/// A blow under this opens no wall in a war's time: a Bombard against an
/// Urban Defenses city of 100 strikes about 5.
pub(super) const WALL_PRICE_MIN_HIT: f64 = 4.0;
/// Tiles a land army covers a turn.
const WALL_PRICE_MARCH_MOVES: f64 = 2.0;
/// Land steps the march is searched over.
const WALL_PRICE_LAND_REACH: usize = 40;
/// The extra turns of an amphibious crossing.
pub(super) const WALL_PRICE_CROSSING_TURNS: f64 = 8.0;
/// The most fielded guns credited to one breach.
const WALL_PRICE_GUNS_MAX: usize = 6;
/// The turns a capture must save to move the campaign.
pub(super) const WALL_PRICE_MARGIN_TURNS: f64 = 5.0;
/// The breach of walls our blow cannot open.
pub(super) const WALL_PRICE_NEVER: f64 = 999.0;
/// The longest breach a capital may ask and still count as breachable: an
/// Urban Defenses capital at our Bombards' 5-7 a shot asks 28 turns and more
/// (live Immortal 194603Z, Washington at 7.1 a shot), and the campaign must
/// not move onto one because the elective pick's capital is unknown.
pub(super) const WALL_PRICE_BREACH_MAX: f64 = 12.0;

/// One capital's capture, priced.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CaptureEstimate {
    pub(super) city: u32,
    pub(super) walls: i32,
    pub(super) hit: f64,
    pub(super) guns: usize,
    pub(super) march: f64,
    pub(super) breach: f64,
}

impl CaptureEstimate {
    pub(super) fn turns(&self) -> f64 {
        self.march + self.breach
    }
}

impl AdvancedAi {
    /// Our best fielded siege unit's ranged strength, and how many we field.
    fn fielded_siege_guns(g: &Game, pid: usize) -> (f64, usize) {
        g.units
            .values()
            .filter(|unit| unit.owner == pid)
            .map(|unit| &g.rules.units[unit.kind])
            .filter(|spec| spec.class == "military" && spec.siege && spec.has_ranged_attack())
            .fold((0.0, 0), |(best, count), spec| {
                (best.max(spec.ranged_attack_strength()), count + 1)
            })
    }

    /// The wall pool we know for `cid`: in sight, else the last-seen record
    /// of the same city; `None` for a city we have never seen.
    fn seen_wall_pool(g: &Game, pid: usize, cid: u32) -> Option<i32> {
        let city = g.cities.get(&cid)?;
        if g.player_visibility(pid).contains(&city.pos) {
            return Some(city.wall_hp);
        }
        g.players[pid]
            .remembered_cities
            .get(&cid)
            .filter(|memory| memory.owner == city.owner && memory.pos == city.pos)
            .map(|memory| memory.wall_hp)
    }

    /// Dry land steps from our cities, searched to [`WALL_PRICE_LAND_REACH`].
    fn land_steps_from_our_cities(g: &Game, pid: usize) -> BTreeMap<Pos, usize> {
        let dry = |pos: Pos| {
            g.map
                .get(pos)
                .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        };
        let mut steps = BTreeMap::new();
        let mut queue = VecDeque::new();
        for city in g.player_city_ids(pid) {
            let pos = g.cities[&city].pos;
            steps.insert(pos, 0);
            queue.push_back(pos);
        }
        while let Some(current) = queue.pop_front() {
            let at = steps[&current];
            if at >= WALL_PRICE_LAND_REACH {
                continue;
            }
            for next in g.nbrs(current) {
                if !steps.contains_key(&next) && dry(next) {
                    steps.insert(next, at + 1);
                    queue.push_back(next);
                }
            }
        }
        steps
    }

    /// The turns to take `cid`: the march to it and the breach of its seen
    /// walls by `guns` (our best fielded blow and count). `None` for a city
    /// never seen, or one out of reach by land with no amphibious train.
    pub(super) fn capital_capture_estimate(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        land: &BTreeMap<Pos, usize>,
        guns: (f64, usize),
    ) -> Option<CaptureEstimate> {
        let city = g.cities.get(&cid)?;
        let walls = Self::seen_wall_pool(g, pid, cid)?;
        let by_land = g
            .wdisk(city.pos, 1)
            .into_iter()
            .filter_map(|pos| land.get(&pos))
            .min()
            .copied()
            .filter(|_| self.rival_reachable_by_land(g, pid, city.owner));
        let march = match by_land {
            Some(steps) => steps as f64 / WALL_PRICE_MARCH_MOVES,
            None if self.amphibious_siege_train || self.amphibious_siege_train_2 => {
                let reach = g
                    .player_city_ids(pid)
                    .iter()
                    .map(|ours| g.wdist(g.cities[ours].pos, city.pos))
                    .min()?;
                f64::from(reach) / WALL_PRICE_MARCH_MOVES + WALL_PRICE_CROSSING_TURNS
            }
            None => return None,
        };
        let (attack, count) = guns;
        let hit = if count > 0 {
            expected_damage(attack, g.city_strength(cid))
        } else {
            0.0
        };
        let fielded = count.clamp(1, WALL_PRICE_GUNS_MAX);
        let breach = if walls <= 0 {
            0.0
        } else if hit < WALL_PRICE_MIN_HIT {
            WALL_PRICE_NEVER
        } else {
            f64::from(walls) / (hit * fielded as f64)
        };
        Some(CaptureEstimate {
            city: cid,
            walls,
            hit,
            guns: count,
            march,
            breach,
        })
    }

    /// `war-target-prices-the-walls`: at peace, the rival whose capital our
    /// fielded guns take at least [`WALL_PRICE_MARGIN_TURNS`] sooner than
    /// `default`'s, the rival the elective chain would name next. `None`
    /// with the gene off, outside the Domination lane, while a siege is
    /// invested, while a train stands within `MUSTER_BREACH_FAR` of the
    /// plan's objective, or when no capital beats the default by the margin.
    pub(super) fn wall_priced_war_target(
        &self,
        g: &Game,
        pid: usize,
        default: Option<usize>,
    ) -> Option<usize> {
        if !self.war_target_prices_the_walls
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return None;
        }
        if self.sieges.values().any(|siege| {
            matches!(
                siege.stage,
                SiegeStage::Invest | SiegeStage::Reduce | SiegeStage::Take
            )
        }) {
            return None;
        }
        // A train already gathering on the plan's objective keeps its war: a
        // land force aimed at it with a member inside the muster radius.
        let mustered = self
            .plan
            .as_ref()
            .and_then(|plan| plan.target_city)
            .and_then(|cid| g.cities.get(&cid))
            .filter(|city| city.owner != pid)
            .is_some_and(|city| {
                self.force_groups.iter().any(|group| {
                    group.domain == super::ForceDomain::Land
                        && g.wdist(group.objective, city.pos) <= 1
                        && group.units.iter().any(|uid| {
                            g.units.get(uid).is_some_and(|unit| {
                                unit.owner == pid
                                    && g.wdist(unit.pos, city.pos) <= MUSTER_BREACH_FAR
                            })
                        })
                })
            });
        if mustered {
            crate::think!(self.journal(), Strategy, Detail,
                "War target keeps the plan's war";
                "a land force aimed at the plan's objective has a member within {MUSTER_BREACH_FAR} tiles");
            return None;
        }
        let land = Self::land_steps_from_our_cities(g, pid);
        let guns = Self::fielded_siege_guns(g, pid);
        let estimate = |rival: usize| {
            self.domination_capital_target_for(g, pid, Some(rival))
                .and_then(|(_, city)| self.capital_capture_estimate(g, pid, city, &land, guns))
        };
        let priced = g
            .players
            .iter()
            .filter(|player| {
                player.id != pid
                    && player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    && !player.is_free_city
            })
            .map(|player| player.id)
            .filter(|rival| {
                !g.is_at_war(pid, *rival)
                    && self.campaign_target_legal(g, pid, *rival)
                    && self.war_policy_target_feasible(g, pid, *rival)
            })
            .filter_map(|rival| {
                estimate(rival)
                    .filter(|capture| capture.breach <= WALL_PRICE_BREACH_MAX)
                    .map(|capture| (rival, capture))
            })
            .min_by(|left, right| {
                left.1
                    .turns()
                    .total_cmp(&right.1.turns())
                    .then(left.0.cmp(&right.0))
            });
        let default_turns = default
            .and_then(estimate)
            .map_or(WALL_PRICE_NEVER, |capture| capture.turns());
        let default_name = default.map_or("no rival", |other| g.players[other].civ.as_str());
        let Some((rival, best)) = priced else {
            crate::think!(self.journal(), Strategy, Detail,
                "War target keeps {default_name}";
                "no rival capital our fielded siege guns can breach in reach; {default_name} reads {default_turns:.1} turns");
            return None;
        };
        if Some(rival) == default || best.turns() + WALL_PRICE_MARGIN_TURNS > default_turns {
            crate::think!(self.journal(), Strategy, Detail,
                "War target keeps {default_name}";
                "{default_name} reads {default_turns:.1} turns; the cheapest capital, {}'s, reads {:.1} \
                 ({} walls, {:.1} a shot, {:.1} to breach, {:.1} to march)",
                g.players[rival].civ, best.turns(), best.walls, best.hit, best.breach, best.march);
            return None;
        }
        let name = g
            .cities
            .get(&best.city)
            .map(|city| city.name.clone())
            .unwrap_or_default();
        let against = default.map_or_else(
            || "no capital".to_string(),
            |other| format!("{}'s {default_turns:.1}", g.players[other].civ),
        );
        crate::think!(self.journal(), Strategy, Strategy,
            "War target prices the walls: {}", g.players[rival].civ;
            "{name} reads {} walls; our best fielded siege gun hits {:.1} a shot ({} fielded): \
             {:.1} turns to breach and {:.1} to march, {:.1} against {against}",
            best.walls, best.hit, best.guns, best.breach, best.march, best.turns());
        Some(rival)
    }
}

#[cfg(test)]
mod tests;
