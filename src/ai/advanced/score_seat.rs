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
//! 1. **Wonders: moved to `score-seat-raises-wonders`.** This gene first
//!    priced a Score lane's wonder at the live race's bonus instead of +180.
//!    It never fired live (the ranking that reads it runs only on an idle
//!    queue) and, in the coordinator's 60-seed paired sim, built 0.10 wonders
//!    a game against 0.08 while costing 0.40 cities (z -2.0): at that price a
//!    ten-turn wonder in a city with three buildings out-bids a Settler. The
//!    price is withdrawn; the wonder program is a queue claim in
//!    `advanced/score_wonders.rs`.
//! 2. **The army: its own gene, `score-seat-trims-the-idle-army`** (opt-in;
//!    acts only with this gene on too). Armed live at 21:04Z on 2026-11-12
//!    as part of this gene, the trim held one body a city from turn 1:
//!    civvis-20261112T211954Z fielded 4-6 combat units at t20-t50, the
//!    Mapuche (223-285 military against our 56-107) declared at t24 and took
//!    Rome by t50. Split out and floored by defence: it acts only from turn
//!    80 (Online), with four cities, at peace with every major and no paying
//!    city named (`score-seat-takes-what-pays`), and while our military is at
//!    least 0.8 of the strongest neighbour's (see
//!    [`SCORE_TRIM_NEIGHBOUR_RATIO`] for the eleven games behind the ratio).
//!    Then the land target is one body a city, four at least, never above
//!    the stock target, and the buffer of two above the land and sea targets
//!    is dropped. A frontier city with no soldier on its tile keeps the stock
//!    pricing. A threatened city, a barbarian alarm, the Settler escorts and
//!    every claim that names a unit (walls, frontier defence, the contact
//!    fleet, the land eye, the deterrent) are untouched.
//! 3. **The Plaza: withdrawn.** It took the Ancestral Hall and then the
//!    Intelligence Agency, which left the Grand Master's Chapel out: the
//!    Chapel is what lets Faith buy land units (`faith_purchase_land_units`
//!    in `data/buildings.json`), and the same sim read 40 less military at the
//!    end and 0.22 more cities lost. The stock list stands.
//! 4. **The last turns.** Within [`SCORE_SPRINT_TURNS`] of the turn limit an
//!    item that cannot finish before the tally is priced out (a Settler must
//!    also walk to its site), so the queues go to what still scores.
//!
//! Journal: every line's reason starts `score-seat-plays-for-points:`, or
//! `score-seat-trims-the-idle-army:` for the army.
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

/// `score-seat-trims-the-idle-army`: the first turn, in Standard turns, on
/// which the trim may act: 80 at Online (`GameSpeed::scale_turns`, 66%). Eleven Rome games of 2026-11-12: five
/// rival declarations came at t24-t51 and four cities fell at t30-t58
/// (Ostia, the capital, Antium, Ostia); the waste the trim was built for came
/// after t100 (game one: Artillery 1,179, Cuirassiers 620, Bombards 457).
pub(super) const SCORE_TRIM_FROM_STANDARD: u32 = 120;

/// `score-seat-trims-the-idle-army`: cities held before the trim acts.
pub(super) const SCORE_TRIM_MIN_CITIES: usize = 4;

/// `score-seat-trims-the-idle-army`: the trimmed land target never falls
/// below this many bodies, nor below one a city.
pub(super) const SCORE_TRIM_MIN_ARMY: usize = 4;

/// `score-seat-trims-the-idle-army`: our military must be at least this share
/// of the strongest neighbour's. In the eleven Rome games every city lost to
/// a major followed a declaration made at 0.12-0.54 of the declarer's
/// military (8 of 9 such declarations cost a city), and no declaration made
/// at 0.85 or more cost one by itself (0 of 9); games nobody declared in sat
/// at 0.17-0.72. 0.8 keeps the margin above every losing ratio.
pub(super) const SCORE_TRIM_NEIGHBOUR_RATIO: f64 = 0.8;

/// `score-seat-trims-the-idle-army`: a major with a known city this close to
/// one of ours is a neighbour (`near-rival-deterrence`'s radius).
pub(super) const SCORE_TRIM_NEIGHBOUR_TILES: i32 = 10;

/// What the gene's journal has already said. Observation only: nothing here
/// feeds a decision.
#[derive(Clone, Default)]
pub(super) struct ScoreSeatJournal {
    /// The last army line: (target, stock target, held, turn written).
    army: Option<(usize, usize, usize, u32)>,
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
    ///
    /// `score-seat-trims-the-idle-army`: `trims` is
    /// [`Self::score_seat_trims_here`]; the target is one body a city with at
    /// least [`SCORE_TRIM_MIN_ARMY`], never above the stock target.
    pub(super) fn score_seat_army_target(
        &self,
        city_count: usize,
        trims: bool,
        desired: usize,
    ) -> usize {
        if trims {
            desired.min(city_count.max(SCORE_TRIM_MIN_ARMY))
        } else {
            desired
        }
    }

    /// `score-seat-trims-the-idle-army`: whether the trim acts empire-wide
    /// this turn. Both genes on, a Score target, the hold in force (no major
    /// at war with us, no paying city named), turn
    /// [`SCORE_TRIM_FROM_STANDARD`] (scaled to the speed) reached,
    /// [`SCORE_TRIM_MIN_CITIES`] cities held, and our military at least
    /// [`SCORE_TRIM_NEIGHBOUR_RATIO`] of every met major's that is at peace
    /// with us and not known to stand far off ([`Self::score_seat_neighbour`]).
    /// The cheap gates run first: this is read inside `production_value`.
    pub(super) fn score_seat_trims_the_army(
        &self,
        g: &Game,
        pid: usize,
        active_major_war: bool,
    ) -> bool {
        self.score_seat_trims_the_idle_army
            && self.score_seat_holds_the_army(active_major_war)
            && g.turn >= g.standard_duration(SCORE_TRIM_FROM_STANDARD)
            && g.player_city_ids(pid).len() >= SCORE_TRIM_MIN_CITIES
            && self.score_seat_neighbour(g, pid).is_none_or(|(_, theirs)| {
                g.military_power(pid) >= SCORE_TRIM_NEIGHBOUR_RATIO * theirs
            })
    }

    /// [`Self::score_seat_trims_the_army`] for one city: a frontier city (a
    /// major's known city within [`SCORE_TRIM_NEIGHBOUR_TILES`]) with no land
    /// soldier on its tile keeps the stock pricing until it is garrisoned.
    pub(super) fn score_seat_trims_here(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        active_major_war: bool,
    ) -> bool {
        self.score_seat_trims_the_army(g, pid, active_major_war)
            && !Self::score_seat_frontier_without_garrison(g, pid, cid)
    }

    /// The strongest met major at peace with us that is not known to stand
    /// far off, and its military: a major counts unless we know at least one
    /// of its cities and every one we know is more than
    /// [`SCORE_TRIM_NEIGHBOUR_TILES`] from all of ours. A major whose cities we
    /// have not seen counts: Portugal took Ostia at t58 of
    /// civvis-20261112T201530Z with none of its cities in sight.
    pub(super) fn score_seat_neighbour(&self, g: &Game, pid: usize) -> Option<(usize, f64)> {
        let ours: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        g.players
            .iter()
            .filter(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && !other.is_free_city
                    && g.has_met(pid, other.id)
                    && !g.same_team(pid, other.id)
            })
            .filter(|other| {
                let mut known = g.cities.values().filter(|city| city.owner == other.id);
                let mut any = false;
                let near = known.any(|city| {
                    any = true;
                    ours.iter()
                        .any(|home| g.wdist(*home, city.pos) <= SCORE_TRIM_NEIGHBOUR_TILES)
                });
                near || !any
            })
            .map(|other| (other.id, g.military_power(other.id)))
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
    }

    /// Whether `cid` stands within [`SCORE_TRIM_NEIGHBOUR_TILES`] of a known
    /// city of another major and has no land soldier of ours on its tile.
    pub(super) fn score_seat_frontier_without_garrison(g: &Game, pid: usize, cid: u32) -> bool {
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        let frontier = g.cities.values().any(|other| {
            other.owner != pid
                && g.players.get(other.owner).is_some_and(|owner| {
                    owner.alive && !owner.is_minor && !owner.is_barbarian && !owner.is_free_city
                })
                && g.wdist(city.pos, other.pos) <= SCORE_TRIM_NEIGHBOUR_TILES
        });
        frontier
            && !g.units.values().any(|unit| {
                unit.owner == pid
                    && unit.pos == city.pos
                    && g.rules
                        .units
                        .get(&unit.kind)
                        .is_some_and(|spec| spec.class == "military" && spec.domain.is_none())
            })
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
    pub(super) fn journal_score_seat(&mut self, g: &Game, pid: usize, _plan: &StrategicPlan) {
        if !self.score_seat_on() || pid >= g.players.len() {
            return;
        }
        self.journal_score_seat_army(g, pid);
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
        if !self.score_seat_trims_the_army(g, pid, active_major_war) {
            return;
        }
        let cities = g.player_city_ids(pid);
        let city_count = cities.len();
        if city_count == 0 {
            return;
        }
        let stock = self.score_seat_stock_army_target(g, pid, city_count);
        let target = self.score_seat_army_target(city_count, true, stock);
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
        let ours = g.military_power(pid);
        let (neighbour, theirs) = self
            .score_seat_neighbour(g, pid)
            .map(|(other, power)| (g.players[other].civ.clone(), power))
            .unwrap_or_else(|| ("no neighbour".to_string(), 0.0));
        think!(self.journal(), Cities, Decision,
               "The idle army holds at {target} land units, one a city, where the stock \
                target asks {stock}";
               "score-seat-trims-the-idle-army: {held} held; military {ours:.0} against \
                {neighbour}'s {theirs:.0} (the trim needs {SCORE_TRIM_NEIGHBOUR_RATIO}), at \
                peace with no city named; the {not_trained} more the stock would train \
                (about {freed:.0} Production at {unit_cost:.0} each) go to buildings, \
                districts, Settlers and wonders instead, about {:.0} points at \
                {SCORE_POINTS_PER_HUNDRED_PRODUCTION:.0} a hundred; a unit is 0 points",
               freed * SCORE_POINTS_PER_HUNDRED_PRODUCTION / 100.0);
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
