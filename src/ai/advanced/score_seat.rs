//! `score-seat-plays-for-points` (opt-in): a seat whose assigned target is the
//! Score Victory spends its production on what the host's tally pays for.
//!
//! ## Why
//!
//! The Score target maps to `GrandStrategy::Expansion` and had almost no code
//! of its own. The first live Rome game on it (`civvis-20261112T190022Z`,
//! Deity, 6 players, Continents, Online, turn limit 170) finished sixth of
//! six at the tally: 529 against Persia's 1,531, the weakest rival 803.
//!
//! - **No wonder.** The rivals finished 13, 9, 9, 5 and 3. The wonder arm
//!   opens its lane on a Score target and then prices the wonder at a flat
//!   +180 raw, which never beats a Library's ~960 (see the comment at
//!   `LIVE_WONDER_RACE_BONUS` in the arm). Worse, opening the lane switches
//!   OFF the live race (`live_race_opens` requires `!lane_opens`), so a Score
//!   seat prices wonders lower than a seat with no target at all. At turn 134
//!   Rome could build Mont St Michel (355 Production) and four 460-Production
//!   wonders at 27 a turn. A wonder is 15 points and a 3-4 Era Score moment
//!   (`Expansion1_Moments.xml`: `MOMENT_BUILDING_CONSTRUCTED_GAME_ERA_WONDER`
//!   4, `..._PAST_ERA_WONDER` 3).
//! - **An army nobody fought.** No war with any major or city-state all game,
//!   no city lost, yet 3,688 of 13,788 Production (27%) went to units (2,680 of
//!   9,944 after turn 100). The land target asked two bodies a city in the
//!   foundation window, then one a city times `peacetime-deterrence`'s 1.5
//!   (Persia fielded 2.4x our power), plus a composition buffer of two: 18
//!   military units for 10 cities at the tally. A unit is 0 points.
//! - **The Throne.** The Government Plaza took the Warlord's Throne, which
//!   pays only on a capture. Either tier-one building is one point; the
//!   Ancestral Hall adds +50% Production toward Settlers in its city and a
//!   Builder for every new city (`plaza-serves-the-scientist`'s list).
//! - **Unfinished work at the tally.** 601 Production stood in queues at
//!   turn 171 that never finished: two Neighborhoods, a Theater Square, a
//!   Campus, an Entertainment Complex and 148 on the Manhattan Project.
//!
//! ## What the gene does
//!
//! Only on a seat whose assigned target is `VictoryTarget::Score`; an exact
//! no-op off and on every other target.
//!
//! 1. **Wonders.** Where the Score lane prices a wonder at +180, it prices it
//!    at the live race's `LIVE_WONDER_RACE_BONUS` scaled by the game's
//!    progress (x1 at the start, x3 at the tally), under the live race's own
//!    development guards: three cities, three buildings in the racing city,
//!    the wonder at most two eras behind the world, one lane plus one per six
//!    cities, and done within `CHEAPEST_WONDER_LANE_MAX_TURNS`. Every refusal
//!    the arm already makes (a wonder queued elsewhere, a threatened city, a
//!    race taking more than 65% of the turns left) still refuses.
//! 2. **The army.** At peace with every major, the land target is one body a
//!    city (the Settler escorts and the barbarian floor still add theirs), and
//!    the composition buffer of two above the land and sea targets is
//!    dropped, so a unit past them is priced as saturated. A declared war, a
//!    threatened city, a barbarian alarm and every claim that names a unit
//!    (walls, frontier defence, the contact fleet, the land eye) are untouched.
//! 3. **The Plaza.** The Plaza's tier list is `plaza-serves-the-scientist`'s:
//!    the Ancestral Hall, then the Intelligence Agency.
//! 4. **The last turns.** Within [`SCORE_SPRINT_TURNS`] of the turn limit an
//!    item that cannot finish before the tally is priced out (a Settler must
//!    also walk to its site), so the queues go to what still scores.
//!
//! Journal: every line's reason starts `score-seat-plays-for-points:`.
use super::*;

/// Turns before the turn limit inside which an item that cannot finish by the
/// tally is worth nothing. Fifteen Online turns is a District or a cheap
/// wonder at the live seat's 20-30 Production.
pub(super) const SCORE_SPRINT_TURNS: u32 = 15;

/// Turns a finished Settler is allowed to walk to its site before the tally.
pub(super) const SCORE_SPRINT_SETTLER_WALK: f64 = 4.0;

/// How often the army line is repeated while nothing it reports changes.
const SCORE_ARMY_LINE_EVERY: u32 = 10;

/// Rough points per 100 Production of what the freed queues buy instead of a
/// unit: a Library (45 Online) is 1 point, a wonder (400) 15 plus its moment,
/// a District 2 and a Settler 5 plus its citizens. Journal arithmetic only.
const SCORE_POINTS_PER_HUNDRED_PRODUCTION: f64 = 2.0;

/// What the gene's journal has already said. Observation only: nothing here
/// feeds a decision.
#[derive(Clone, Default)]
pub(super) struct ScoreSeatJournal {
    /// The last army line: (target, stock target, held, turn written).
    army: Option<(usize, usize, usize, u32)>,
    /// Wonders already reported, by the racing city's tile and the wonder.
    /// Tiles, not city ids: the live mirror renumbers cities between frames.
    wonders: BTreeSet<(Pos, String)>,
    /// The Plaza building already reported.
    plaza: Option<String>,
    /// The turn limit whose last-turns line has been written.
    sprint: Option<u32>,
}

impl AdvancedAi {
    /// Whether the gene acts: on, on a Score target.
    pub(super) fn score_seat_on(&self) -> bool {
        self.score_seat_plays_for_points && self.victory_target == Some(VictoryTarget::Score)
    }

    /// Whether the peacetime army hold applies: the gene acts and no major is
    /// at war with us.
    pub(super) fn score_seat_holds_the_army(&self, active_major_war: bool) -> bool {
        // `score-seat-takes-what-pays`: a paying city the plan names lifts the
        // hold as a war does, from the mustering stage on. `false` with that
        // gene off. See `advanced/score_takes_what_pays.rs`.
        self.score_seat_on() && !(active_major_war || self.score_take_lifts_the_cap())
    }

    /// The land army target under the hold: one body a city, never above the
    /// stock target. Unchanged off, off a Score target, and at war.
    pub(super) fn score_seat_army_target(
        &self,
        city_count: usize,
        active_major_war: bool,
        desired: usize,
    ) -> usize {
        if self.score_seat_holds_the_army(active_major_war) {
            desired.min(city_count)
        } else {
            desired
        }
    }

    /// Whether a city may race a wonder at the live race's price: the gene
    /// acts and the live race's development guards hold. The caller has
    /// already established that the Score lane is open.
    pub(super) fn score_seat_wonder_race_opens(
        &self,
        city_count: usize,
        city_buildings: usize,
        wonder_era: usize,
        world_era: usize,
        wonders_in_flight: usize,
        turns: f64,
    ) -> bool {
        self.score_seat_on()
            && city_count >= 3
            && city_buildings >= 3
            && wonder_era + 2 >= world_era
            && wonders_in_flight < Self::live_wonder_race_lanes(city_count)
            && turns <= CHEAPEST_WONDER_LANE_MAX_TURNS
    }

    /// The wonder's price on the open race, in the arm's raw units: the live
    /// race's bonus at this point of the game.
    pub(super) fn score_seat_wonder_price(g: &Game) -> f64 {
        LIVE_WONDER_RACE_BONUS * Self::live_wonder_race_scale(g)
    }

    /// Turns left in which a finished item still counts at the tally: the
    /// turn limit's own end-of-turn completions are scored (the live tally
    /// row is written the turn after the limit).
    fn score_seat_turns_to_tally(g: &Game) -> Option<u32> {
        (g.max_turns > 0 && g.turn <= g.max_turns).then(|| g.max_turns - g.turn + 1)
    }

    /// Whether `item`, `turns` of production from done, can no longer score:
    /// inside the last [`SCORE_SPRINT_TURNS`] it finishes after the tally (a
    /// Settler also needs [`SCORE_SPRINT_SETTLER_WALK`] turns to found).
    /// Repairs are never priced out: a pillaged district stops its yields.
    pub(super) fn score_seat_misses_the_tally(&self, g: &Game, item: &Item, turns: f64) -> bool {
        if !self.score_seat_on() || matches!(item, Item::Repair { .. }) {
            return false;
        }
        let Some(left) = Self::score_seat_turns_to_tally(g) else {
            return false;
        };
        if left > SCORE_SPRINT_TURNS {
            return false;
        }
        let walk = match item {
            Item::Unit { unit } if unit == "settler" => SCORE_SPRINT_SETTLER_WALK,
            _ => 0.0,
        };
        turns.ceil() + walk > left as f64
    }

    /// The land target the stock scorer would ask at peace, for the journal:
    /// one body a city, two in the foundation window, weighed by the
    /// strongest met major (`peacetime-deterrence`).
    fn score_seat_stock_army_target(&self, g: &Game, pid: usize, city_count: usize) -> usize {
        let base = if self.victory_planning && !self.phase_specialization_active(g) {
            city_count.saturating_mul(2)
        } else {
            city_count
        };
        self.enemy_weighted_army_target(g, pid, base)
    }

    /// The Production cost of the strongest land unit `cid` can train, for
    /// the journal's arithmetic.
    fn score_seat_unit_cost(g: &Game, pid: usize, cid: u32) -> Option<f64> {
        g.rules
            .units
            .iter()
            .filter(|(_, spec)| spec.class == "military" && spec.domain.is_none())
            .filter_map(|(name, spec)| {
                let item = Item::Unit { unit: *name };
                g.can_produce(pid, cid, &item).then(|| {
                    (
                        spec.strength.max(spec.ranged_attack_strength()),
                        g.item_remaining_cost_for_city(pid, cid, &item),
                    )
                })
            })
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, cost)| cost)
    }

    /// Journal what the gene changed this turn. Observation only, after every
    /// production writer has settled the queues. Exact no-op off.
    pub(super) fn journal_score_seat(&mut self, g: &Game, pid: usize, plan: &StrategicPlan) {
        if !self.score_seat_on() || pid >= g.players.len() {
            return;
        }
        self.journal_score_seat_army(g, pid);
        self.journal_score_seat_wonders(g, pid, plan);
        self.journal_score_seat_plaza(g, pid);
        self.journal_score_seat_sprint(g);
    }

    fn journal_score_seat_army(&mut self, g: &Game, pid: usize) {
        let active_major_war = g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && g.is_at_war(pid, player.id)
        });
        if active_major_war {
            return;
        }
        let cities = g.player_city_ids(pid);
        let city_count = cities.len();
        if city_count == 0 {
            return;
        }
        let stock = self.score_seat_stock_army_target(g, pid, city_count);
        let target = self.score_seat_army_target(city_count, false, stock);
        if stock <= target {
            return;
        }
        let counts = self.counts(g, pid);
        let held = counts
            .military
            .saturating_sub(counts.naval + counts.aircraft);
        let repeat = self.score_seat_journal.army.is_none_or(|(t, s, h, turn)| {
            (t, s, h) != (target, stock, held)
                || g.turn.saturating_sub(turn) >= SCORE_ARMY_LINE_EVERY
        });
        if !repeat {
            return;
        }
        self.score_seat_journal.army = Some((target, stock, held, g.turn));
        let not_trained = stock.saturating_sub(held.max(target));
        let unit_cost = cities
            .iter()
            .copied()
            .find(|cid| g.cities[cid].is_capital)
            .or(cities.first().copied())
            .and_then(|cid| Self::score_seat_unit_cost(g, pid, cid))
            .unwrap_or(0.0);
        let freed = not_trained as f64 * unit_cost;
        think!(self.journal(), Cities, Decision,
               "The peacetime army holds at {target} land units, one a city, where the stock \
                target asks {stock}";
               "score-seat-plays-for-points: {held} held; the {not_trained} more the stock \
                would train (about {freed:.0} Production at {unit_cost:.0} each) go to \
                buildings, districts, Settlers and wonders instead, about {:.0} points at \
                {SCORE_POINTS_PER_HUNDRED_PRODUCTION:.0} a hundred; a unit is 0 points",
               freed * SCORE_POINTS_PER_HUNDRED_PRODUCTION / 100.0);
    }

    fn journal_score_seat_wonders(&mut self, g: &Game, pid: usize, plan: &StrategicPlan) {
        if plan.strategy == GrandStrategy::Recovery {
            return;
        }
        let cities = g.player_city_ids(pid);
        let city_count = cities.len();
        let racing: Vec<u32> = cities
            .iter()
            .copied()
            .filter(|cid| matches!(g.cities[cid].queue.first(), Some(Item::Wonder { .. })))
            .collect();
        for cid in &racing {
            let city = &g.cities[cid];
            let Some(item @ Item::Wonder { wonder, .. }) = city.queue.first() else {
                continue;
            };
            let key = (city.pos, wonder.as_str().to_string());
            if self.score_seat_journal.wonders.contains(&key) {
                continue;
            }
            let Some(spec) = g.rules.wonders.get(wonder) else {
                continue;
            };
            let wonder_era = spec
                .tech
                .as_deref()
                .and_then(|tech| g.rules.techs.get(tech).map(|node| node.era))
                .or_else(|| {
                    spec.civic
                        .as_deref()
                        .and_then(|civic| g.rules.civics.get(civic).map(|node| node.era))
                })
                .unwrap_or(0);
            let others = racing.len().saturating_sub(1);
            let turns = self.production_build_turns(g, pid, *cid, item);
            if !self.score_seat_wonder_race_opens(
                city_count,
                city.buildings.len(),
                wonder_era,
                g.world_era,
                others,
                turns,
            ) {
                continue;
            }
            self.score_seat_journal.wonders.insert(key);
            let cost = g.item_remaining_cost_for_city(pid, *cid, item);
            let moment = if wonder_era >= g.world_era { 4 } else { 3 };
            think!(self.journal(), Cities, Decision,
                   "{} races {} for the tally", city.name, Self::plain_item(item);
                   "score-seat-plays-for-points: 15 points and a {moment} Era Score moment for \
                    {cost:.0} Production, about {turns:.0} turns; priced at {:.0} raw where \
                    the stock Score lane priced every wonder at 180, below a Library's ~960, \
                    and built none in civvis-20261112T190022Z",
                   Self::score_seat_wonder_price(g));
        }
    }

    fn journal_score_seat_plaza(&mut self, g: &Game, pid: usize) {
        for cid in g.player_city_ids(pid) {
            let Some(ours) = BasicAi::plaza_building_item_for(g, pid, cid, true) else {
                continue;
            };
            let stock = BasicAi::plaza_building_item_for(g, pid, cid, false);
            if stock.as_ref() == Some(&ours) {
                continue;
            }
            let ours_name = Self::plain_item(&ours);
            if self.score_seat_journal.plaza.as_deref() == Some(ours_name.as_str()) {
                return;
            }
            let stock_name = stock
                .as_ref()
                .map(Self::plain_item)
                .unwrap_or_else(|| "nothing".to_string());
            think!(self.journal(), Cities, Decision,
                   "The Government Plaza in {} takes the {ours_name}, not the {stock_name}",
                   g.cities[&cid].name;
                   "score-seat-plays-for-points: either is 1 point; the Warlord's Throne pays \
                    only on a capture, the Ancestral Hall gives +50% Production toward Settlers \
                    in its city (5 points a city) and a Builder to every new one");
            self.score_seat_journal.plaza = Some(ours_name);
            return;
        }
    }

    fn journal_score_seat_sprint(&mut self, g: &Game) {
        let Some(left) = Self::score_seat_turns_to_tally(g) else {
            return;
        };
        if left > SCORE_SPRINT_TURNS || self.score_seat_journal.sprint == Some(g.max_turns) {
            return;
        }
        self.score_seat_journal.sprint = Some(g.max_turns);
        think!(self.journal(), Cities, Decision,
               "{left} turns to the tally at turn {}: nothing is started that cannot finish",
               g.max_turns;
               "score-seat-plays-for-points: an item finished after the tally is 0 points (a \
                Settler also needs {SCORE_SPRINT_SETTLER_WALK:.0} turns to found); the first \
                Rome game left 601 Production unfinished at turn 171");
    }
}

#[cfg(test)]
mod tests;
