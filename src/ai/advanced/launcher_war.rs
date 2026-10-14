//! The war on a space racer: raid its pads, and do not make peace with it.
//!
//! Live Emperor G422 (civvis-20261008T171024Z, pin fa051977c) was at war with
//! Sumeria from turn 145. Sumeria landed the Moon at 185 and the Mars base at
//! 195, and at 195 we offered it peace: "no siege against them is feasible
//! (805 strength against a bill of 2091) and the tide has run against us (-5)
//! over the window: 1307 power against their 1053". Sumeria's military went
//! 239 (175) → 1053 (195) → 1399 (200). A full siege of a pad city is often
//! out of reach, but a district is pillaged by a land unit standing on it or
//! a Bomber's pillage mission — the same effect as a successful Disrupt
//! Rocketry, which the spies managed twice (Ur 185, Isin 193). And once the
//! Exoplanet Expedition is launched, nothing short of conquering the player
//! stops it, so a peace that lets the launch through gives up the race.
//!
//! - `war-raids-the-pads`: while at war with a decisive space racer
//!   (`decisive_space_racers`: two space projects landed or a science race
//!   reading 80), each standing pad of theirs we have seen is pillaged by a
//!   Bomber in range when the pillage lands it intact, else walked to by the
//!   nearest land military unit within [`PAD_RAID_REACH`] that stands off the
//!   campaign's siege ring — the band hunt's envelope: at least
//!   [`PAD_RAIDER_MIN_HP`] health, a walk simulated first, and never one that
//!   leaves the raider in reach of half its health in blows. A raider that
//!   reaches the pad with moves left pillages it; one whose route to the pad
//!   is refused (a unit standing on it) closes on the tile beside it.
//! - `no-peace-with-a-launcher`: no peace is offered to, or accepted from, a
//!   rival with the Mars base landed and the Exoplanet not yet launched while
//!   a pad of theirs we have seen stands — unless our own cities are falling
//!   (one hit within [`FALLING_CITY_TURNS`] turns and at half health or
//!   less), the exit a besieged empire still needs.

use std::collections::BTreeSet;

use super::{AdvancedAi, StrategicPlan};
use crate::game::{Action, ActionFamilies, Game};
use crate::think;
use crate::Pos;

/// A pad raider keeps at least this much health.
const PAD_RAIDER_MIN_HP: i32 = 30;
/// How much of its health a raider may be left exposed to after its walk.
const PAD_RAIDER_DANGER_SHARE: f64 = 0.5;
/// A land siege's ring: bodies this close to the campaign objective stay.
const PAD_RAID_RING: i32 = 2;
/// A land unit within this many tiles of a pad walks to it.
pub(crate) const PAD_RAID_REACH: i32 = 8;
/// The health a Bomber needs to pillage (the host's own floor).
const PAD_BOMBER_MIN_HP: i32 = 50;
/// `launcher-war-ignores-the-edge`: space projects a rival must have landed
/// (the Moon) before its war opens short of the edge.
const LAUNCHER_WAR_STAGES: usize = 2;
/// `launcher-war-ignores-the-edge`: our military, as a share of the racer's
/// steady power, at which its war opens.
pub(crate) const LAUNCHER_WAR_EDGE: f64 = 0.8;
/// Our city counts as falling when hit within this many turns...
const FALLING_CITY_TURNS: u32 = 2;
/// ...and at this much health or less (of 200).
const FALLING_CITY_HP: i32 = 100;

impl AdvancedAi {
    /// A rival past its Mars base and short of the Exoplanet launch.
    fn is_launcher(g: &Game, rival: usize) -> bool {
        g.players.get(rival).is_some_and(|player| {
            player.science_projects.contains("launch_mars_colony")
                && !player.science_projects.contains("exoplanet_expedition")
        })
    }

    /// `rival`'s standing Spaceports we have seen, in position order.
    fn standing_pads(g: &Game, pid: usize, rival: usize) -> Vec<Pos> {
        let explored = &g.players[pid].explored;
        let mut pads: Vec<Pos> = g
            .player_city_ids(rival)
            .into_iter()
            .flat_map(|cid| {
                g.cities[&cid]
                    .districts
                    .iter()
                    .filter(|(district, _)| g.district_family(**district) == "spaceport")
                    .map(|(_, pad)| *pad)
                    .collect::<Vec<_>>()
            })
            .filter(|pad| {
                explored.contains(pad) && g.map.get(*pad).is_some_and(|tile| !tile.pillaged)
            })
            .collect();
        pads.sort();
        pads.dedup();
        pads
    }

    /// Whether a city of ours is falling: hit within
    /// [`FALLING_CITY_TURNS`] turns and down to [`FALLING_CITY_HP`].
    fn our_cities_falling(g: &Game, pid: usize) -> bool {
        g.player_city_ids(pid).iter().any(|cid| {
            let city = &g.cities[cid];
            city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= FALLING_CITY_TURNS
                && city.hp <= FALLING_CITY_HP
        })
    }

    /// `no-peace-with-a-launcher`: whether the war with `rival` is kept —
    /// it is past its Mars base and short of the launch, a pad of its stands,
    /// and no city of ours is falling. `false` with the gene off.
    pub(crate) fn launcher_keeps_the_war(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.no_peace_with_a_launcher
            && g.victory_conditions.science
            && g.players
                .get(rival)
                .is_some_and(|player| player.alive && !player.is_minor && !player.is_barbarian)
            && Self::is_launcher(g, rival)
            && !Self::standing_pads(g, pid, rival).is_empty()
            && !Self::our_cities_falling(g, pid)
    }

    /// `war-raids-the-pads`: plan and apply this frame's pad raids. Returns
    /// the raiders, which the ordinary unit loop must leave alone. Nothing
    /// with the gene off.
    pub(crate) fn plan_pad_raids(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        reserved: &BTreeSet<u32>,
    ) -> BTreeSet<u32> {
        let mut raiders = BTreeSet::new();
        if !self.war_raids_the_pads {
            return raiders;
        }
        let racers: Vec<usize> = self
            .decisive_space_racers(g, pid)
            .into_iter()
            .filter(|rival| g.is_at_war(pid, *rival))
            .collect();
        let objective = plan
            .target_city
            .and_then(|cid| g.cities.get(&cid))
            .map(|city| city.pos);
        for rival in racers {
            for pad in Self::standing_pads(g, pid, rival) {
                if g.map.get(pad).is_none_or(|tile| tile.pillaged) {
                    continue;
                }
                if let Some(bomber) = Self::pad_bomber(g, pid, pad, reserved, &raiders) {
                    let pillage = Action::AirPillage {
                        unit: bomber,
                        target: pad,
                    };
                    if g.apply(pid, &pillage).is_ok() {
                        raiders.insert(bomber);
                        self.pad_raid_landed(g, pid, rival, pad, "a Bomber");
                        continue;
                    }
                } else if let Some((bomber, base)) =
                    Self::pad_bomber_rebase(g, pid, pad, reserved, &raiders)
                {
                    // No Bomber reaches it: one rebases to a base of ours in
                    // range, for next turn's pillage. G432's (41, 11) pad,
                    // the one Persia launched from, stood 14-19 tiles from
                    // our three Bombers and 7 from Quito.
                    if g.apply(
                        pid,
                        &Action::AirRebase {
                            unit: bomber,
                            to: base,
                        },
                    )
                    .is_ok()
                    {
                        raiders.insert(bomber);
                        think!(self.journal(), Military, Decision,
                               "A Bomber rebases toward a Spaceport of {}", g.players[rival].civ;
                               "war-raids-the-pads: the pad stands {} tiles from its new base, in range for the pillage",
                               g.wdist(base, pad);
                               pad);
                    }
                }
                let mut candidates: Vec<(i32, u32)> = g
                    .player_unit_ids(pid)
                    .into_iter()
                    .filter(|uid| {
                        let unit = &g.units[uid];
                        let spec = &g.rules.units[unit.kind];
                        spec.class == "military"
                            && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                            && !reserved.contains(uid)
                            && !raiders.contains(uid)
                            && unit.hp >= PAD_RAIDER_MIN_HP
                            && unit.moves_left > 0.0
                            && !unit.acted
                            && unit.linked_to.is_none()
                            && !g.is_embarked(unit)
                            && !Self::lone_garrison(g, pid, *uid)
                            && objective.is_none_or(|ring| g.wdist(unit.pos, ring) > PAD_RAID_RING)
                            && g.wdist(unit.pos, pad) <= PAD_RAID_REACH
                    })
                    .map(|uid| (g.wdist(g.units[&uid].pos, pad), uid))
                    .collect();
                candidates.sort();
                for (_, uid) in candidates {
                    let Some(step) = Self::pad_raid_step(g, pid, uid, pad) else {
                        continue;
                    };
                    if g.apply(pid, &step).is_err() {
                        continue;
                    }
                    raiders.insert(uid);
                    let on_pad = g
                        .units
                        .get(&uid)
                        .is_some_and(|unit| unit.pos == pad && unit.moves_left > 0.0);
                    if on_pad
                        && g.pillageable_at(pid, pad)
                        && g.apply(pid, &Action::Pillage { unit: uid }).is_ok()
                    {
                        let kind = g.units.get(&uid).map(|unit| unit.kind.to_string());
                        self.pad_raid_landed(g, pid, rival, pad, kind.as_deref().unwrap_or("unit"));
                    } else if let Some(unit) = g.units.get(&uid) {
                        think!(self.journal(), Military, Decision,
                               "{} marches on a Spaceport of {}", crate::reasoning::plain(&unit.kind), g.players[rival].civ;
                               "war-raids-the-pads: {} tiles from the pad; a pillaged pad builds no space project until it is repaired",
                               g.wdist(unit.pos, pad);
                               pad);
                    }
                    break;
                }
            }
        }
        raiders
    }

    /// The Bomber that pillages `pad` and comes back with the host's
    /// pillage health: the pillage simulated first, the healthiest such.
    fn pad_bomber(
        g: &Game,
        pid: usize,
        pad: Pos,
        reserved: &BTreeSet<u32>,
        raiders: &BTreeSet<u32>,
    ) -> Option<u32> {
        g.player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                spec.domain.as_deref() == Some("air")
                    && spec.promotion_class == "air_bomber"
                    && !reserved.contains(uid)
                    && !raiders.contains(uid)
                    && unit.hp >= PAD_BOMBER_MIN_HP
                    && unit.moves_left > 0.0
                    && unit.attacks_left > 0
            })
            .filter_map(|uid| {
                let mut board = g.speculative_clone();
                let pillage = Action::AirPillage {
                    unit: uid,
                    target: pad,
                };
                if board.apply(pid, &pillage).is_err()
                    || board.map.get(pad).is_none_or(|tile| !tile.pillaged)
                {
                    return None;
                }
                let hp = board.units.get(&uid)?.hp;
                (hp >= PAD_RAIDER_MIN_HP).then_some((hp, uid))
            })
            .max_by_key(|(hp, uid)| (*hp, std::cmp::Reverse(*uid)))
            .map(|(_, uid)| uid)
    }

    /// A Bomber out of range of `pad` and a base of ours (a city or an
    /// Aerodrome) in range of it the Bomber can rebase to this turn: the
    /// farthest such base from the pad, the Bomber nearest it.
    fn pad_bomber_rebase(
        g: &Game,
        pid: usize,
        pad: Pos,
        reserved: &BTreeSet<u32>,
        raiders: &BTreeSet<u32>,
    ) -> Option<(u32, Pos)> {
        let bases: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .flat_map(|cid| {
                let city = &g.cities[&cid];
                std::iter::once(city.pos).chain(
                    city.districts
                        .iter()
                        .filter(|(district, _)| g.district_family(**district) == "aerodrome")
                        .map(|(_, pos)| *pos)
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let mut bombers: Vec<(i32, u32)> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                spec.domain.as_deref() == Some("air")
                    && spec.promotion_class == "air_bomber"
                    && !reserved.contains(uid)
                    && !raiders.contains(uid)
                    && unit.hp >= PAD_BOMBER_MIN_HP
                    && unit.moves_left > 0.0
                    && g.wdist(unit.pos, pad) > g.unit_attack_range(*uid)
            })
            .map(|uid| (g.wdist(g.units[&uid].pos, pad), uid))
            .collect();
        bombers.sort();
        bombers.into_iter().find_map(|(_, uid)| {
            let range = g.unit_attack_range(uid);
            let mut usable: Vec<Pos> = bases
                .iter()
                .copied()
                .filter(|base| g.wdist(*base, pad) <= range)
                .filter(|base| {
                    let mut board = g.speculative_clone();
                    board
                        .apply(
                            pid,
                            &Action::AirRebase {
                                unit: uid,
                                to: *base,
                            },
                        )
                        .is_ok()
                })
                .collect();
            usable.sort_by_key(|base| (std::cmp::Reverse(g.wdist(*base, pad)), *base));
            usable.first().map(|base| (uid, *base))
        })
    }

    /// `launcher-war-ignores-the-edge`: whether the war on `rival` opens now
    /// — it has landed two space projects and not launched the Exoplanet, a
    /// pad of its stands, no city of ours is falling, and our military is
    /// [`LAUNCHER_WAR_EDGE`] times its steady power. `false` with the gene
    /// off.
    pub(crate) fn launcher_war_opens(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.launcher_war_ignores_the_edge
            && g.victory_conditions.science
            && Self::science_denial_stages(g, rival) >= LAUNCHER_WAR_STAGES
            && !g.players[rival]
                .science_projects
                .contains("exoplanet_expedition")
            && !Self::standing_pads(g, pid, rival).is_empty()
            && !Self::our_cities_falling(g, pid)
            && g.military_power(pid)
                >= LAUNCHER_WAR_EDGE * self.steady_rival_power(g, rival).max(1.0)
    }

    /// `launcher-war-ignores-the-edge`: the surprise war on `target`, with
    /// its journal line.
    pub(crate) fn launcher_war_opening(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        my_power: f64,
    ) -> Option<Action> {
        let surprise = g
            .legal_actions_within(pid, ActionFamilies::DIPLOMACY)
            .into_iter()
            .find(|action| matches!(action, Action::DeclareWar { player } if *player == target))?;
        think!(self.journal(), Military, Strategy,
               "Opening the war on {}'s space race", g.players[target].civ;
               "launcher-war-ignores-the-edge: {} space projects landed and a pad standing; {my_power:.0} power against their steady {:.0}, and its pads are raided, not besieged",
               Self::science_denial_stages(g, target), self.steady_rival_power(g, target));
        Some(surprise)
    }

    /// The walk toward `pad`, simulated: onto the pad when the route takes
    /// it, else onto the nearest free tile beside it (a trader or a soldier
    /// standing on the pad refuses the route to the pad itself: G422's (48,
    /// 26) pad held a Sumerian trader at turn 193). It must bring the raider
    /// nearer and leave it out of reach of half its health in blows.
    fn pad_raid_step(g: &Game, pid: usize, uid: u32, pad: Pos) -> Option<Action> {
        let start = g.units.get(&uid)?.pos;
        let mut beside: Vec<Pos> = g.nbrs(pad).into_iter().collect();
        beside.sort_by_key(|tile| (g.wdist(start, *tile), *tile));
        std::iter::once(pad).chain(beside).find_map(|goal| {
            let mut board = g.speculative_clone();
            let step = Action::MoveTo {
                unit: uid,
                to: goal,
            };
            if board.apply(pid, &step).is_err() {
                return None;
            }
            let unit = board.units.get(&uid)?;
            if g.wdist(unit.pos, pad) >= g.wdist(start, pad) {
                return None;
            }
            let hp = f64::from(unit.hp);
            (super::battle_planner::strike_danger(&board, pid, unit.pos, uid)
                < hp * PAD_RAIDER_DANGER_SHARE)
                .then_some(step)
        })
    }

    /// The record and journal line of a pad pillaged by `by`.
    fn pad_raid_landed(&self, g: &mut Game, pid: usize, rival: usize, pad: Pos, by: &str) {
        *g.players[pid]
            .counters
            .entry("pad_raid:pillaged".to_string())
            .or_insert(0) += 1;
        think!(self.journal(), Military, Decision,
               "Pillaging a Spaceport of {}", g.players[rival].civ;
               "war-raids-the-pads: {} pillages it; a pillaged pad builds no space project until it is repaired",
               crate::reasoning::plain(by);
               pad);
    }
}

#[cfg(test)]
mod tests;
