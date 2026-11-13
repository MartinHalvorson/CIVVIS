//! `score-seat-raises-wonders` (opt-in): a Score seat keeps a wonder going in
//! its capital, and from turn 90 (Online) in a second city, as a queue claim
//! that acts in the native engine and on the live seat alike.
//!
//! ## Why
//!
//! A wonder is 15 points at the tally and a historic moment of 4 Era Score in
//! its own era, 3 after (`Expansion1_Moments.xml`:
//! `MOMENT_BUILDING_CONSTRUCTED_GAME_ERA_WONDER`, `..._PAST_ERA_WONDER`).
//! Fifteen Rome Score games of 2026-11-12 built 0.25 a game against the
//! rivals' 3-13. The host offered wonders the whole time: in
//! civvis-20261112T190022Z on 158 turns, St Basil's and the Taj Mahal at
//! 11-15 turns in Rome from t100 to the tally. A program that always kept one
//! going in the best city from t40 and two from t90, priced by the host's own
//! turns-to-finish, would have finished 7.7 a game and lost 2.7 races (twelve
//! finished games; Step 1 of the PR).
//!
//! The price `score-seat-plays-for-points` put on a wonder never fired live:
//! the ranking that reads it runs only on an idle queue, and a busy capital's
//! queue goes to a dozen claims (Campus buildings, Builders, repairs, the
//! Plaza, Walls) before it is idle. In the one carrier that lived
//! (civvis-20261112T212311Z) the seat was at war from t26 to the tally. So
//! this is a claim, like the others, with its own displacement rule.
//!
//! ## What the gene does
//!
//! Only on a seat whose assigned target is `VictoryTarget::Score`; an exact
//! no-op off and on every other target. Once a turn, in the claims runner:
//!
//! 1. **Races it holds.** A city racing a wonder (keyed by the city's tile,
//!    not its id: the live mirror renumbers cities) that now holds it has
//!    finished it; one that can no longer build it lost the race (the host
//!    salvages the Production into the city: `LOC_NOTIFICATION_WONDER_FAILED`
//!    in `Notifications_Text.xml`); otherwise the wonder goes back to the head
//!    of its queue when a routine item took it.
//! 2. **Lanes.** One from [`SCORE_WONDER_FIRST_LANE_STANDARD`] (40 at Online)
//!    with [`SCORE_WONDER_MIN_CITIES`] cities, a second from
//!    [`SCORE_WONDER_SECOND_LANE_STANDARD`] (90 at Online) with
//!    [`SCORE_WONDER_SECOND_LANE_CITIES`]. The capital takes the first lane,
//!    the most productive other city the second. Never on a Recovery plan,
//!    in the plan's threatened city or a city attacked in the last four turns.
//! 3. **Which wonder.** Of the wonders the city can build (with a legal site),
//!    finishing within [`SCORE_WONDER_MAX_TURNS`] and before the turn limit,
//!    the one with the most expected points a turn: (15 + the Era Score
//!    moment) x the chance of finishing first / turns. **Race risk:** the
//!    chance is `exp(-turns / pace)`, the pace measured from when the host
//!    stopped offering a wonder in the Rome games: a wonder of the world's era
//!    lasted ~25 turns once offered, one era behind ~50, older ~100 (the AI
//!    rushes the new and leaves the old: the Taj Mahal stood 81 turns and was
//!    taken in 1 of 12 games, Kilwa Kisiwani 36 and taken in 10 of 10). A
//!    race under an even chance is not started.
//! 4. **What it may pass.** An idle queue or a routine head. It never passes
//!    a Settler, Walls, the city's first Campus or Library, a Bath or
//!    Aqueduct, a Granary, a Repair, a Spy, another wonder, a military unit
//!    while a major is at war with us or the empire holds fewer soldiers than
//!    cities, or anything finishing within two turns.
//!
//! Journal (every reason starts `score-seat-raises-wonders:`): each start
//! (wonder, city, turns, points with the Era Score, the chance, what it
//! passed), each loss of a race and each finish. Counters
//! `score_wonder_starts`, `score_wonder_finishes`, `score_wonder_losses`.
use super::*;

/// The first lane opens on this turn, in Standard turns (40 at Online).
pub(super) const SCORE_WONDER_FIRST_LANE_STANDARD: u32 = 60;

/// The second lane opens on this turn, in Standard turns (90 at Online).
pub(super) const SCORE_WONDER_SECOND_LANE_STANDARD: u32 = 135;

/// Cities held before the first lane opens: the first Settlers come first.
pub(super) const SCORE_WONDER_MIN_CITIES: usize = 3;

/// Cities held before the second lane opens.
pub(super) const SCORE_WONDER_SECOND_LANE_CITIES: usize = 6;

/// A race longer than this many turns is not started.
pub(super) const SCORE_WONDER_MAX_TURNS: f64 = 25.0;

/// A race whose chance of finishing first is below this is not started.
pub(super) const SCORE_WONDER_MIN_CHANCE: f64 = 0.5;

/// Turns a wonder of the world's era stays on offer once it appears (see the
/// module). One era behind it lasts twice as long, older four times.
pub(super) const SCORE_WONDER_PACE_THIS_ERA: f64 = 25.0;

/// A head finishing within this many turns is left to finish.
pub(super) const SCORE_WONDER_LET_FINISH_TURNS: f64 = 2.0;

/// Turns since a city was last attacked under which it races nothing.
const SCORE_WONDER_ATTACK_MEMORY: u32 = 4;

/// Points at the tally for a finished wonder (`Game::score_parts`).
const SCORE_WONDER_POINTS: f64 = 15.0;

/// One candidate race: the item, its turns, chance and expected points a turn.
#[derive(Clone, Debug)]
pub(super) struct ScoreWonderPick {
    pub(super) item: Item,
    pub(super) turns: f64,
    pub(super) chance: f64,
    pub(super) moment: u32,
    pub(super) per_turn: f64,
}

impl AdvancedAi {
    /// Whether the gene acts: on, on a Score target.
    pub(super) fn score_wonders_on(&self) -> bool {
        self.score_seat_raises_wonders && self.victory_target == Some(VictoryTarget::Score)
    }

    /// The era a wonder belongs to: its technology's, else its civic's.
    pub(super) fn score_wonder_era(g: &Game, wonder: &Name) -> usize {
        let Some(spec) = g.rules.wonders.get(wonder) else {
            return 0;
        };
        spec.tech
            .as_deref()
            .and_then(|tech| g.rules.techs.get(tech).map(|node| node.era))
            .or_else(|| {
                spec.civic
                    .as_deref()
                    .and_then(|civic| g.rules.civics.get(civic).map(|node| node.era))
            })
            .unwrap_or(0)
    }

    /// The Era Score moment a finished wonder pays: 4 in its own era (or
    /// later), 3 for a wonder of a past era.
    pub(super) fn score_wonder_moment(wonder_era: usize, world_era: usize) -> u32 {
        if wonder_era >= world_era {
            4
        } else {
            3
        }
    }

    /// The chance of finishing `turns` from now before a rival does, at the
    /// pace the Rome games measured for a wonder `world_era - wonder_era` eras
    /// old.
    pub(super) fn score_wonder_chance(wonder_era: usize, world_era: usize, turns: f64) -> f64 {
        let pace = SCORE_WONDER_PACE_THIS_ERA
            * match world_era.saturating_sub(wonder_era) {
                0 => 1.0,
                1 => 2.0,
                _ => 4.0,
            };
        (-turns.max(0.0) / pace).exp()
    }

    /// How many lanes are open this turn.
    pub(super) fn score_wonder_lanes(g: &Game, city_count: usize) -> usize {
        if city_count < SCORE_WONDER_MIN_CITIES
            || g.turn < g.standard_duration(SCORE_WONDER_FIRST_LANE_STANDARD)
        {
            0
        } else if city_count >= SCORE_WONDER_SECOND_LANE_CITIES
            && g.turn >= g.standard_duration(SCORE_WONDER_SECOND_LANE_STANDARD)
        {
            2
        } else {
            1
        }
    }

    /// Whether a city may race at all: not the plan's threatened city and not
    /// attacked lately.
    fn score_wonder_city_safe(g: &Game, plan: &StrategicPlan, cid: u32) -> bool {
        let city = &g.cities[&cid];
        let attacked = city.last_attacked > 0
            && g.turn.saturating_sub(city.last_attacked) <= SCORE_WONDER_ATTACK_MEMORY;
        plan.threatened_city != Some(cid) && !attacked
    }

    /// Whether a major is at war with `pid`.
    fn score_wonder_at_war(g: &Game, pid: usize) -> bool {
        g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && g.is_at_war(pid, player.id)
        })
    }

    /// Whether the race may take `cid`'s queue from its current head (see the
    /// module, point 4). An empty queue may always be taken.
    pub(super) fn score_wonder_may_pass(&self, g: &Game, pid: usize, cid: u32) -> bool {
        let city = &g.cities[&cid];
        let Some(head) = city.queue.first() else {
            return true;
        };
        if self.production_build_turns(g, pid, cid, head) <= SCORE_WONDER_LET_FINISH_TURNS {
            return false;
        }
        match head {
            Item::Unit { unit } => {
                if *unit == "settler" || *unit == "spy" {
                    return false;
                }
                let military = g
                    .rules
                    .units
                    .get(unit)
                    .is_some_and(|spec| spec.class == "military");
                if !military {
                    return true;
                }
                let cities = g.player_city_ids(pid).len();
                let soldiers = g
                    .units
                    .values()
                    .filter(|other| {
                        other.owner == pid
                            && g.rules.units.get(&other.kind).is_some_and(|spec| {
                                spec.class == "military" && spec.domain.is_none()
                            })
                    })
                    .count();
                !Self::score_wonder_at_war(g, pid) && soldiers >= cities
            }
            Item::Building { building } => {
                let walls = g
                    .rules
                    .buildings
                    .get(building)
                    .is_some_and(|spec| spec.outer_defense > 0);
                let first_library = *building == "library"
                    && !city.buildings.iter().any(|built| *built == "library");
                !(walls || first_library || *building == "granary" || *building == "aqueduct")
            }
            Item::District { district, .. } => {
                let first_campus = *district == "campus"
                    && !g.city_has_district_family(city, crate::name!("campus"));
                !(first_campus || *district == "bath" || *district == "aqueduct")
            }
            Item::Formation { .. } => !Self::score_wonder_at_war(g, pid),
            Item::Project { project } => g
                .rules
                .projects
                .get(project)
                .is_some_and(|spec| spec.repeatable),
            Item::Wonder { .. } | Item::Repair { .. } | Item::Product { .. } => false,
        }
    }

    /// The best race `cid` can start (see the module, point 3), skipping
    /// wonders in `taken` (raced elsewhere). `None` when nothing qualifies.
    pub(super) fn score_wonder_pick(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        taken: &BTreeSet<Name>,
    ) -> Option<ScoreWonderPick> {
        let left = if g.max_turns > 0 {
            g.max_turns.saturating_sub(g.turn) as f64
        } else {
            f64::INFINITY
        };
        let founded = g.players[pid].religion.is_some();
        g.producible_items(pid, cid)
            .into_iter()
            .filter_map(|item| {
                let Item::Wonder { wonder, .. } = &item else {
                    return None;
                };
                if taken.contains(wonder) {
                    return None;
                }
                // A Stonehenge-like site is spent once our religion stands.
                let spent = founded
                    && g.rules.wonders.get(wonder).is_some_and(|spec| {
                        spec.effects
                            .get("religion_founding_site")
                            .is_some_and(|value| *value > 0.0)
                    });
                if spent {
                    return None;
                }
                let turns = self.production_build_turns(g, pid, cid, &item);
                if !turns.is_finite() || turns > SCORE_WONDER_MAX_TURNS || turns > left {
                    return None;
                }
                let era = Self::score_wonder_era(g, wonder);
                let chance = Self::score_wonder_chance(era, g.world_era, turns);
                if chance < SCORE_WONDER_MIN_CHANCE {
                    return None;
                }
                let moment = Self::score_wonder_moment(era, g.world_era);
                let per_turn = chance * (SCORE_WONDER_POINTS + f64::from(moment)) / turns.max(1.0);
                Some(ScoreWonderPick {
                    item,
                    turns,
                    chance,
                    moment,
                    per_turn,
                })
            })
            .max_by(|a, b| {
                a.per_turn
                    .total_cmp(&b.per_turn)
                    .then_with(|| format!("{:?}", b.item).cmp(&format!("{:?}", a.item)))
            })
    }

    /// The cities that hold the lanes, in lane order: the capital first, then
    /// the most productive other city. Unsafe cities are skipped.
    fn score_wonder_lane_cities(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        lanes: usize,
    ) -> Vec<u32> {
        let mut cities: Vec<u32> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| Self::score_wonder_city_safe(g, plan, *cid))
            .collect();
        cities.sort_by(|a, b| {
            let (ca, cb) = (&g.cities[a], &g.cities[b]);
            (cb.is_capital && cb.original_owner == pid)
                .cmp(&(ca.is_capital && ca.original_owner == pid))
                .then_with(|| {
                    g.city_yields(*b)
                        .production
                        .total_cmp(&g.city_yields(*a).production)
                })
                .then(a.cmp(b))
        });
        cities.truncate(lanes);
        cities
    }

    /// Resolve the races already held: finished, lost, or still running (and
    /// put back at the head of their queue when a routine item took it).
    fn score_wonder_resolve_races(&mut self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let races: Vec<(Pos, Name)> = self
            .score_wonder_races
            .iter()
            .map(|(pos, wonder)| (*pos, *wonder))
            .collect();
        for (pos, wonder) in races {
            let Some(cid) = g.city_at(pos).filter(|cid| g.cities[cid].owner == pid) else {
                self.score_wonder_races.remove(&pos);
                continue;
            };
            let name = g.cities[&cid].name.clone();
            if g.cities[&cid].wonders.contains_key(&wonder) {
                self.score_wonder_races.remove(&pos);
                *g.players[pid]
                    .counters
                    .entry("score_wonder_finishes".to_string())
                    .or_insert(0) += 1;
                think!(self.journal(), Cities, Decision,
                       "{name} finishes {}", plain(wonder.as_str());
                       "score-seat-raises-wonders: 15 points at the tally and its Era Score \
                        moment");
                continue;
            }
            let item = g
                .producible_items(pid, cid)
                .into_iter()
                .find(|item| matches!(item, Item::Wonder { wonder: w, .. } if *w == wonder));
            let Some(item) = item else {
                self.score_wonder_races.remove(&pos);
                // The progress key carries no site, so any position reads it.
                let invested = g.item_invested_production(cid, &Item::Wonder { wonder, pos });
                *g.players[pid]
                    .counters
                    .entry("score_wonder_losses".to_string())
                    .or_insert(0) += 1;
                think!(self.journal(), Cities, Decision,
                       "{name} loses the {} race", plain(wonder.as_str());
                       "score-seat-raises-wonders: it can no longer be built here (a rival \
                        finished it, or its site went); {invested:.0} Production stood in it, \
                        which the host salvages into the city");
                continue;
            };
            if g.cities[&cid].queue.first() == Some(&item)
                || !Self::score_wonder_city_safe(g, plan, cid)
                || plan.strategy == GrandStrategy::Recovery
                || !self.score_wonder_may_pass(g, pid, cid)
            {
                continue;
            }
            let _ = g.apply(pid, &Action::Produce { city: cid, item });
        }
    }

    /// See the module. `start` is false for the late pass that only keeps the
    /// races standing after the ordinary writers. Exact no-op off.
    pub(super) fn claim_queue_for_the_score_wonder(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        start: bool,
    ) {
        if !self.score_wonders_on() || pid >= g.players.len() {
            return;
        }
        self.score_wonder_resolve_races(g, pid, plan);
        if !start || plan.strategy == GrandStrategy::Recovery {
            return;
        }
        let city_ids = g.player_city_ids(pid);
        let lanes = Self::score_wonder_lanes(g, city_ids.len());
        if lanes == 0 {
            return;
        }
        // A city with a wonder at its head, from this gene or any writer,
        // already holds a lane.
        let mut racing: BTreeSet<Name> = BTreeSet::new();
        let mut held = 0;
        for cid in &city_ids {
            if let Some(Item::Wonder { wonder, .. }) = g.cities[cid].queue.first() {
                racing.insert(*wonder);
                held += 1;
            }
        }
        if held >= lanes {
            return;
        }
        for cid in self.score_wonder_lane_cities(g, pid, plan, lanes) {
            if held >= lanes {
                break;
            }
            if matches!(g.cities[&cid].queue.first(), Some(Item::Wonder { .. }))
                || !self.score_wonder_may_pass(g, pid, cid)
            {
                continue;
            }
            let Some(pick) = self.score_wonder_pick(g, pid, cid, &racing) else {
                continue;
            };
            let passed = g.cities[&cid]
                .queue
                .first()
                .map(Self::plain_item)
                .unwrap_or_else(|| "an idle queue".to_string());
            let Item::Wonder { wonder, .. } = &pick.item else {
                continue;
            };
            let wonder = *wonder;
            let cost = g.item_remaining_cost_for_city(pid, cid, &pick.item);
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: pick.item.clone(),
                },
            )
            .is_err()
            {
                continue;
            }
            held += 1;
            racing.insert(wonder);
            self.score_wonder_races.insert(g.cities[&cid].pos, wonder);
            *g.players[pid]
                .counters
                .entry("score_wonder_starts".to_string())
                .or_insert(0) += 1;
            think!(self.journal(), Cities, Decision,
                   "{} raises {} for the tally", g.cities[&cid].name, plain(wonder.as_str());
                   "score-seat-raises-wonders: {} points ({} and a {} Era Score moment) for \
                    {cost:.0} Production, about {:.0} turns, a {:.0}% chance to finish first; \
                    it goes ahead of {passed}; lane {held} of {lanes}",
                   SCORE_WONDER_POINTS as u32 + pick.moment, SCORE_WONDER_POINTS as u32,
                   pick.moment, pick.turns, pick.chance * 100.0);
        }
    }
}

#[cfg(test)]
mod tests;
