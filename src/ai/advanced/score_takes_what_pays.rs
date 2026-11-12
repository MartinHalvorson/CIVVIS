//! `score-seat-takes-what-pays`: a Score seat takes the neighbour's city
//! that buys more points per Production than anything it can build.
//!
//! ## Why (live Deity Rome, 6-player Continents, Online, turn limit 170)
//!
//! The first four Score games of 2026-11-12 ended sixth of six at 529-726
//! against winners at 1,361-1,531. The winner of the first held 13 wonders
//! (195 points) and 144 citizens in 12 cities; ours held no wonder. A city
//! taken moves its score (`Game::score_parts`: 5 a city, 1 a citizen, 2 a
//! district or 4 a unique one, 1 a building, 15 a wonder) from its owner to
//! us. What the seat's queues buy instead is about two points a hundred
//! Production (a Library is 45 Production for 1 point,
//! `score-seat-plays-for-points`).
//!
//! In three of the four games a rival whose military stood at or under ours
//! held an open city (no walls) within twelve tiles of one of ours, worth 15
//! to 38 points, that the melee Rome fielded would take within five turns by
//! the combat formula, for 110-390 Production of new bodies: 4 to 17 points
//! a hundred Production, two to eight times the queues' rate (Timbuktu and
//! Winnipeg in 191211Z t40-70, St. Petersburg and Moscow in 200323Z t60-70,
//! New York in 193301Z t85; city-states Taruga and Wolin). The windows close
//! by about turn 90: past it every candidate's centre read 57-87 against our
//! best melee's 45-55, behind 200-300 walls. The seat declared no war in any
//! of the four games: a Score seat plays an Expansion plan, which never
//! declares. In the 45 Domination games of the same day, the 34 wars we
//! declared on a rival at or under our military lost one city; the 49 on a
//! stronger one lost four.
//!
//! ## What the gene does (Score target only)
//!
//! - **The board.** Every city of a met major or city-state within
//!   [`SCORE_TAKE_REACH`] tiles of one of ours, with no walls standing, is
//!   priced: its points to us (doubled when its owner leads the score
//!   table), and the fewest land melee bodies — our standing ones within the
//!   reach first, then the best melee class a city can train — that bring it
//!   to nothing within [`SCORE_TAKE_TURNS`] turns (`open_city_take_turns`).
//!   Its expected points are those points times a conservative reading of
//!   the capture ([`SCORE_TAKE_OPEN_READING`]: raw near-certain readings on
//!   open cities were realized 37% of the time on 1,200 posed sieges). It
//!   pays at [`SCORE_TAKE_MIN_POINTS`] points or more, at most
//!   [`SCORE_TAKE_MAX_PRODUCTION`] Production of new bodies, and expected
//!   points of at least [`SCORE_TAKE_PER_HUNDRED`] a hundred Production
//!   (what the seat's queues buy). Cities our takers reach within
//!   [`SCORE_TAKE_MUSTER_TURNS`] come first (the nearest open city, not the
//!   capital), then the most expected points a hundred, then wonders.
//! - **The circumstances.** The owner's military at most
//!   [`SCORE_TAKE_POWER_RATIO`] times ours; no city of ours threatened or
//!   falling; no war on another major; a legal target; a city-state only
//!   when we hold no envoy there and its suzerain is no stronger than us;
//!   and the capture landing before the tally (new bodies at half the
//!   empire's Production, the march at two tiles a turn).
//! - **The campaign.** While a city pays under those circumstances, the
//!   plan is Conquest with that city as its objective, so the shipped army
//!   sizing, staging march, Siege row and siege train work on it; the war
//!   machinery gated on Domination answers too (`conquest_gates_open`).
//! - **Muster first, declare second.** The plan names the city at peace;
//!   its staging march walks the corps in. A rival walls an open city about
//!   seven turns after a war on it begins (live `183602Z`: Warsaw open t41-47,
//!   walls 100 at t48), and the clock starts only with the war. So the war
//!   opens only when our takers within [`SCORE_TAKE_STAGE_REACH`] tiles,
//!   each at most [`SCORE_TAKE_MUSTER_TURNS`] turns from the city, take it —
//!   march included — within [`SCORE_TAKE_WALL_WINDOW`] turns and outweigh
//!   the owner's soldiers in sight. It opens through the shipped opening (a
//!   denouncement, then the Formal War), with the treasury, the one-war rule
//!   and the peace treaty still holding it; the Formal War re-reads the
//!   window when it is due.
//! - **After.** A capture is journalled with its points and what the war
//!   cost; the board then names the next paying city or none, and the
//!   shipped peace desk decides.
//!
//! ## The graded versions (`-2`, `-3`) and the grab
//! (`score-seat-grabs-what-pays`)
//!
//! The empire-wide comparison of version one never opened at Deity: live
//! `civvis-20261112T222448Z` read five paying cities at 5-17 expected
//! points a hundred Production (Tula, St. Petersburg, Novgorod, Bangalore)
//! and refused every one on "its owner's military stands above ours", and
//! the 60 paired sims played identically. A Deity neighbour fields 1.4 to
//! 5 times our military all game. The graded versions trade that one test
//! for the circumstances that decide a quick grab, and cap the empire-wide
//! ratio at [`SCORE_TAKE_POWER_RATIO_2`] (`-2`), [`SCORE_TAKE_POWER_RATIO_3`]
//! (`-3`) or not at all (the grab, `score-seat-grabs-what-pays`, whose
//! capture must land within [`SCORE_TAKE_QUICK_TURNS`] turns of the
//! declaration and which asks peace the turn after it; the family holds at
//! most three versions, so the grab is its own tag and overrides any version
//! played beside it). Each adds:
//!
//! - **The local picture.** The owner's soldiers in sight within
//!   [`SCORE_TAKE_LOCAL_REACH`] tiles of the city are no stronger than the
//!   corps; and at the declaration the assault forecast, read as if the war
//!   began this turn, gives a calibrated P(capture within the window) of at
//!   least [`SCORE_TAKE_FORECAST_P`] for the mustered takers.
//! - **The home front.** Every city of ours within
//!   [`SCORE_TAKE_BORDER_REACH`] tiles of one of the owner's holds Walls and
//!   a garrison: a city without is the price of the war.
//! - **The treasury.** It carries the corps' upkeep for
//!   [`SCORE_TAKE_FUNDED_TURNS`] turns, and the shipped war reserve holds.
//! - **After the capture.** The captured centre keeps (or takes) a soldier
//!   for [`SCORE_TAKE_HOLD_TURNS`] standard turns, and peace is offered to
//!   its owner once no second paying city of its stands in reach (the
//!   grab: at once).
//!
//! A city is remembered by its tile and its owner by name: the live mirror
//! rebuilds city and city-state ids every frame.

use super::open_city_objective::open_city_take_turns;
use super::soft_city_window::SOFT_CITY_GARRISON_BONUS;
use super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::{effective_strength, Action, Game, Item};
use crate::think;
use crate::Pos;
use std::collections::BTreeMap;

/// Tiles from one of our cities a candidate may stand.
pub(crate) const SCORE_TAKE_REACH: i32 = 12;
/// Turns, once the force stands at the city, within which it must fall.
pub(crate) const SCORE_TAKE_TURNS: u32 = 5;
/// The most bodies a capture may ask.
pub(crate) const SCORE_TAKE_MAX_BODIES: usize = 6;
/// The least a city must be worth.
pub(crate) const SCORE_TAKE_MIN_POINTS: f64 = 15.0;
/// The most Production the new bodies may cost.
pub(crate) const SCORE_TAKE_MAX_PRODUCTION: f64 = 400.0;
/// Expected points a hundred Production the capture must buy: what the
/// seat's queues buy (`score-seat-plays-for-points` prices them at two).
pub(crate) const SCORE_TAKE_PER_HUNDRED: f64 = 2.0;
/// The conservative reading of a capture of an open city that the
/// arithmetic says falls within the turns: the assault forecast's posed
/// sieges realized a near-certain reading 37% of the time at walls 0.
pub(crate) const SCORE_TAKE_OPEN_READING: f64 = 0.4;
/// The most turns a taker may stand from the city for the declaration.
pub(crate) const SCORE_TAKE_MUSTER_TURNS: u32 = 5;
/// Turns after the war begins before an open city's walls stand: the
/// capture, march included, must land within them.
pub(crate) const SCORE_TAKE_WALL_WINDOW: u32 = 7;
/// The most the owner's military may be, as a multiple of ours.
pub(crate) const SCORE_TAKE_POWER_RATIO: f64 = 1.0;
/// `-2`: the most the owner's military may be, as a multiple of ours.
pub(crate) const SCORE_TAKE_POWER_RATIO_2: f64 = 1.5;
/// `-3`: the most the owner's military may be, as a multiple of ours.
pub(crate) const SCORE_TAKE_POWER_RATIO_3: f64 = 2.5;
/// The grab (`score-seat-grabs-what-pays`): the turns, from the declaration,
/// within which the capture lands (march included); no empire-wide cap.
pub(crate) const SCORE_TAKE_QUICK_TURNS: u32 = 3;
/// Graded: tiles from the city within which the owner's soldiers in sight
/// must be no stronger than the corps.
pub(crate) const SCORE_TAKE_LOCAL_REACH: i32 = 6;
/// Graded: tiles from one of the owner's cities within which a city of ours
/// must hold Walls and a garrison.
pub(crate) const SCORE_TAKE_BORDER_REACH: i32 = 10;
/// Graded: turns of the corps' upkeep the treasury must carry.
pub(crate) const SCORE_TAKE_FUNDED_TURNS: f64 = 15.0;
/// Graded: the least calibrated P(capture within the window) the assault
/// forecast must read for the mustered takers at the declaration. Readings
/// under 0.2 were right (4% realized); this asks a little more than a no.
pub(crate) const SCORE_TAKE_FORECAST_P: f32 = 0.3;
/// Graded: the expected-points factor on the forecast's calibrated reading.
pub(crate) const SCORE_TAKE_FORECAST_FACTOR: f64 = 0.5;
/// Graded: standard turns a captured city's centre keeps a soldier.
pub(crate) const SCORE_TAKE_HOLD_TURNS: u32 = 20;
/// Graded: tiles from a held centre a soldier of ours may come from to fill
/// it.
pub(crate) const SCORE_TAKE_FILL_REACH: i32 = 3;
/// Tiles from the city within which the takers count for the declaration.
pub(crate) const SCORE_TAKE_STAGE_REACH: i32 = 6;
/// Tiles from the city within which the owner's soldiers in sight answer.
pub(crate) const SCORE_TAKE_DEFENDER_REACH: i32 = 4;
/// The share of the empire's Production the new bodies are timed at.
pub(crate) const SCORE_TAKE_PRODUCTION_SHARE: f64 = 0.5;
/// Turns the capture must land ahead of the tally.
pub(crate) const SCORE_TAKE_TALLY_MARGIN: u32 = 2;
/// The health a standing taker needs to count.
pub(crate) const SCORE_TAKE_TAKER_HP: i32 = 50;
/// How often the board line repeats while it names the same city.
const SCORE_TAKE_LINE_EVERY: u32 = 10;

/// A city on the board and what it pays.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScoreTake {
    /// The owner.
    pub(crate) rival: usize,
    /// The city.
    pub(crate) city: u32,
    /// Its score to us.
    pub(crate) points: f64,
    /// The same, doubled when its owner leads the score table.
    pub(crate) swing: f64,
    /// Bodies the capture asks.
    pub(crate) bodies: usize,
    /// Of them, our standing takers within the reach.
    pub(crate) standing: usize,
    /// Production of the new bodies.
    pub(crate) production: f64,
    /// Turns the force takes it in, once it stands there.
    pub(crate) turns: u32,
    /// Its expected swing (times [`SCORE_TAKE_OPEN_READING`]) a hundred
    /// Production.
    pub(crate) per_hundred: f64,
    /// Turns our nearest taker (or a body trained in our nearest city)
    /// walks to the city.
    pub(crate) march: u32,
    /// Its wonders.
    pub(crate) wonders: usize,
    /// Whether its owner is a city-state.
    pub(crate) minor: bool,
}

/// What the gene named last turn and has journalled. Cities by tile and
/// owners by name: the live mirror rebuilds their ids every frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ScoreTakeState {
    /// The city the plan is taking: its tile, its owner's name and its
    /// points when named.
    pub(crate) named: Option<(Pos, String, f64)>,
    /// The war on the named city's owner: its name and our ledger against
    /// it (units lost, theirs, cities lost, theirs) when the war began.
    pub(crate) war_base: Option<(String, (u32, u32, u32, u32))>,
    /// Graded: centres we took, with their former owner's name and the turn
    /// they hold a garrison until.
    pub(crate) held: BTreeMap<Pos, (String, u32)>,
    /// Graded: owners we took a city from in the war still running.
    pub(crate) took_from: BTreeMap<String, u32>,
    /// The last board line: its key and turn.
    pub(crate) line: Option<(Option<u32>, String, u32)>,
    /// The last turn the declaration said it was holding for the muster.
    pub(crate) muster_line: Option<u32>,
}

impl AdvancedAi {
    /// The version this seat plays: 4 (the grab), 3, 2, 1, or 0 with every
    /// version off. A seat plays at most one of the family; the grab
    /// overrides it.
    pub(crate) fn score_take_version(&self) -> u8 {
        if self.score_seat_grabs_what_pays {
            4
        } else if self.score_seat_takes_what_pays_3 {
            3
        } else if self.score_seat_takes_what_pays_2 {
            2
        } else {
            u8::from(self.score_seat_takes_what_pays)
        }
    }

    /// Whether a graded version (`-2`, `-3`) or the grab plays.
    pub(crate) fn score_take_graded(&self) -> bool {
        self.score_take_version() >= 2
    }

    /// The most the owner's military may be, as a multiple of ours.
    pub(crate) fn score_take_power_ratio(&self) -> f64 {
        match self.score_take_version() {
            2 => SCORE_TAKE_POWER_RATIO_2,
            3 => SCORE_TAKE_POWER_RATIO_3,
            4 => f64::INFINITY,
            _ => SCORE_TAKE_POWER_RATIO,
        }
    }

    /// The turns from the declaration within which the capture must land.
    pub(crate) fn score_take_window(&self) -> u32 {
        if self.score_take_version() == 4 {
            SCORE_TAKE_QUICK_TURNS
        } else {
            SCORE_TAKE_WALL_WINDOW
        }
    }

    /// Whether the gene acts for this seat: a version on, and on the Score
    /// lane.
    pub(crate) fn score_take_on(&self, g: &Game) -> bool {
        self.score_take_version() > 0 && self.active_victory_target(g) == Some(VictoryTarget::Score)
    }

    /// The named city as this frame knows it: its owner and id, resolved
    /// from its tile.
    pub(crate) fn score_take_named_city(&self, g: &Game, pid: usize) -> Option<(usize, u32)> {
        let (pos, _, _) = self.score_take_state.named.as_ref()?;
        let cid = g.city_at(*pos)?;
        let owner = g.cities[&cid].owner;
        (owner != pid).then_some((owner, cid))
    }

    /// Whether the plan names a paying city this turn. `false` with the
    /// gene off and off the Score lane.
    pub(crate) fn score_take_names_a_city(&self, g: &Game) -> bool {
        self.score_take_on(g) && self.score_take_state.named.is_some()
    }

    /// The Domination war machinery's gate, widened: Domination, or a Score
    /// seat whose plan names a paying city. Exactly the Domination test with
    /// the gene off.
    pub(crate) fn conquest_gates_open(&self, g: &Game) -> bool {
        self.active_victory_target(g) == Some(VictoryTarget::Domination)
            || self.score_take_names_a_city(g)
    }

    /// For `score-seat-plays-for-points` (the hook its peacetime army cap
    /// reads, beside `active_major_war`): whether the cap is lifted because
    /// the plan names a paying city, from the turn it is named at peace, so
    /// the corps is raised and walked in before the war. `false` with the
    /// gene off; the name is only ever held on the Score lane
    /// (`score_take_observe` clears it elsewhere).
    pub(crate) fn score_take_lifts_the_cap(&self) -> bool {
        self.score_take_version() > 0 && self.score_take_state.named.is_some()
    }

    /// The city's score to its holder: 5, its citizens, 2 a district (4 a
    /// unique one), 1 a building, 15 a wonder (`Game::score_parts`).
    pub(crate) fn score_city_points(g: &Game, cid: u32) -> f64 {
        let Some(city) = g.cities.get(&cid) else {
            return 0.0;
        };
        let districts: f64 = city
            .districts
            .keys()
            .map(|d| {
                if g.rules
                    .districts
                    .get(d)
                    .is_some_and(|spec| spec.unique_to.is_some())
                {
                    4.0
                } else {
                    2.0
                }
            })
            .sum();
        5.0 + f64::from(city.pop)
            + districts
            + city.buildings.len() as f64
            + 15.0 * city.wonders.len() as f64
    }

    /// The major leading the score table, other than us.
    fn score_take_leader(g: &Game, pid: usize) -> Option<usize> {
        g.players
            .iter()
            .filter(|p| p.id != pid && p.alive && !p.is_minor && !p.is_barbarian)
            .max_by_key(|p| (g.score(p.id), std::cmp::Reverse(p.id)))
            .map(|p| p.id)
    }

    /// The feasibility reading, in one place so it can be swapped: the turns
    /// a force of (attack strength, health, first turn it can strike) takes
    /// an open centre of `defence` and `health` in, within `horizon` turns.
    /// Today the capture arithmetic `open-city-is-the-objective` uses
    /// (`open_city_take_turns`); the assault forecast's calibrated reading
    /// can stand here once it answers at peace and for a corps not yet
    /// built.
    pub(crate) fn score_take_turns(
        force: &[(f64, i32, u32)],
        defence: f64,
        health: i32,
        horizon: u32,
    ) -> Option<u32> {
        open_city_take_turns(force, defence, health, horizon)
    }

    /// The best melee body a city of ours can train now: its strength and
    /// Production cost.
    fn score_take_trainable(g: &Game, pid: usize) -> Option<(f64, f64, f64)> {
        let cities = g.player_city_ids(pid);
        g.rules
            .units
            .iter()
            .filter(|(_, spec)| Self::conquest_melee_body(spec))
            .filter_map(|(name, spec)| {
                let item = Item::Unit { unit: *name };
                cities
                    .iter()
                    .copied()
                    .filter(|cid| g.can_produce(pid, *cid, &item))
                    .map(|cid| g.item_remaining_cost_for_city(pid, cid, &item))
                    .min_by(f64::total_cmp)
                    .map(|cost| (spec.strength, cost, spec.maintenance))
            })
            .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.total_cmp(&a.1)))
    }

    /// Our standing land melee within `reach` tiles of `pos` that can take a
    /// city, outside our city centres (garrisons) and civilian escorts:
    /// (unit, attack strength, health, first turn it can strike).
    fn score_take_standing(
        &self,
        g: &Game,
        pid: usize,
        pos: crate::Pos,
        reach: i32,
    ) -> Vec<(u32, f64, i32, u32)> {
        let guards = self.all_reserved_civilian_guards();
        let steps = Self::open_city_land_steps(g, pos, reach.max(1) + 2);
        let mut takers: Vec<(u32, f64, i32, u32)> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| !guards.contains(uid))
            .filter_map(|uid| g.units.get(&uid))
            .filter(|unit| {
                unit.hp >= SCORE_TAKE_TAKER_HP
                    && !g.is_embarked(unit)
                    && Self::conquest_melee_body(&g.rules.units[unit.kind])
                    && g.wdist(unit.pos, pos) <= reach
                    && g.city_at(unit.pos)
                        .is_none_or(|cid| g.cities[&cid].owner != pid)
            })
            .map(|unit| {
                let road = steps
                    .get(&unit.pos)
                    .copied()
                    .unwrap_or_else(|| g.wdist(unit.pos, pos));
                let moves = g.unit_max_moves(unit.id).max(1.0);
                let first = (f64::from(road) / moves).ceil().max(1.0) as u32;
                (unit.id, g.unit_strength(unit, false), unit.hp, first)
            })
            .collect();
        takers.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        takers
    }

    /// The centre as it will fight: its strength, or the strongest of the
    /// owner's soldiers in sight within two tiles plus a garrison's bonus;
    /// and the owner's soldiers in sight within
    /// [`SCORE_TAKE_DEFENDER_REACH`], summed at their health.
    fn score_take_defence(&self, g: &Game, pid: usize, cid: u32) -> (f64, f64) {
        let city = &g.cities[&cid];
        let visible = self.battlefront_visibility(g, pid);
        let defenders: Vec<&crate::game::Unit> = g
            .units
            .values()
            .filter(|unit| unit.owner == city.owner)
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .filter(|unit| g.wdist(unit.pos, city.pos) <= SCORE_TAKE_DEFENDER_REACH)
            .filter(|unit| g.sees(&visible, unit.pos) && g.unit_visible_to(unit.id, pid))
            .collect();
        let garrison = defenders
            .iter()
            .filter(|unit| g.wdist(unit.pos, city.pos) <= 2)
            .map(|unit| g.unit_strength(unit, true) + SOFT_CITY_GARRISON_BONUS)
            .fold(0.0, f64::max);
        let theirs = defenders
            .iter()
            .map(|unit| effective_strength(g.unit_strength(unit, true), unit.hp))
            .sum();
        (g.city_strength(cid).max(garrison), theirs)
    }

    /// Price one city: `None` when it is no candidate (walls standing, out
    /// of reach, nothing takes it within the turns).
    pub(crate) fn score_take_price(&self, g: &Game, pid: usize, cid: u32) -> Option<ScoreTake> {
        let city = g.cities.get(&cid)?;
        let owner = g.players.get(city.owner)?;
        if city.owner == pid || !owner.alive || owner.is_barbarian || city.wall_hp > 0 {
            return None;
        }
        let near = g
            .player_city_ids(pid)
            .into_iter()
            .map(|ours| g.wdist(g.cities[&ours].pos, city.pos))
            .min()?;
        if near > SCORE_TAKE_REACH {
            return None;
        }
        let points = Self::score_city_points(g, cid);
        let swing = if Self::score_take_leader(g, pid) == Some(city.owner) {
            2.0 * points
        } else {
            points
        };
        let (defence, _) = self.score_take_defence(g, pid, cid);
        let standing = self.score_take_standing(g, pid, city.pos, SCORE_TAKE_REACH);
        let trainable = Self::score_take_trainable(g, pid);
        for bodies in 1..=SCORE_TAKE_MAX_BODIES {
            let held = standing.len().min(bodies);
            let new = bodies - held;
            let mut force: Vec<(f64, i32, u32)> = standing
                .iter()
                .take(held)
                .map(|(_, strength, hp, _)| (*strength, *hp, 1))
                .collect();
            if new > 0 {
                let (strength, _, _) = trainable?;
                force.extend(std::iter::repeat_n((strength, 100, 1), new));
            }
            if let Some(turns) = Self::score_take_turns(&force, defence, city.hp, SCORE_TAKE_TURNS)
            {
                let production = new as f64 * trainable.map_or(0.0, |(_, cost, _)| cost);
                let march = standing
                    .first()
                    .map(|(_, _, _, first)| *first)
                    .unwrap_or(u32::MAX)
                    .min((near as u32).div_ceil(2));
                return Some(ScoreTake {
                    rival: city.owner,
                    city: cid,
                    points,
                    swing,
                    bodies,
                    standing: held,
                    production,
                    turns,
                    per_hundred: 100.0 * SCORE_TAKE_OPEN_READING * swing / production.max(1.0),
                    march,
                    wonders: city.wonders.len(),
                    minor: owner.is_minor,
                });
            }
        }
        None
    }

    /// The board: every candidate city, best paying first.
    pub(crate) fn score_take_board(&self, g: &Game, pid: usize) -> Vec<ScoreTake> {
        let mut board: Vec<ScoreTake> = g
            .cities
            .values()
            .filter(|city| {
                city.owner != pid
                    && g.players.get(city.owner).is_some_and(|owner| {
                        owner.alive && !owner.is_barbarian && g.has_met(pid, owner.id)
                    })
            })
            .filter_map(|city| self.score_take_price(g, pid, city.id))
            .collect();
        board.sort_by(|a, b| {
            (a.march > SCORE_TAKE_MUSTER_TURNS)
                .cmp(&(b.march > SCORE_TAKE_MUSTER_TURNS))
                .then(b.per_hundred.total_cmp(&a.per_hundred))
                .then(b.wonders.cmp(&a.wonders))
                .then(a.march.cmp(&b.march))
                .then(a.city.cmp(&b.city))
        });
        board
    }

    /// Whether a priced city pays.
    pub(crate) fn score_take_pays(take: &ScoreTake) -> bool {
        take.points >= SCORE_TAKE_MIN_POINTS
            && take.production <= SCORE_TAKE_MAX_PRODUCTION
            && take.per_hundred >= SCORE_TAKE_PER_HUNDRED
    }

    /// The circumstances: `Err` names the first that fails.
    pub(crate) fn score_take_circumstances(
        &self,
        g: &Game,
        pid: usize,
        take: &ScoreTake,
        threatened: bool,
    ) -> Result<(), String> {
        let ours = g.military_power(pid);
        let ratio = self.score_take_power_ratio();
        if g.military_power(take.rival) > ratio * ours {
            return Err(if ratio <= SCORE_TAKE_POWER_RATIO {
                "its owner's military stands above ours".to_string()
            } else {
                format!("its owner's military stands above {ratio:.1} times ours")
            });
        }
        if threatened || Self::our_cities_falling(g, pid) {
            return Err("a city of ours is threatened".to_string());
        }
        if self
            .one_war_enemies(g, pid)
            .into_iter()
            .any(|enemy| enemy != take.rival)
        {
            return Err("we are at war with another major".to_string());
        }
        let at_war = g.is_at_war(pid, take.rival);
        if !at_war && !self.campaign_target_legal(g, pid, take.rival) {
            return Err("the war would not be legal".to_string());
        }
        if take.minor && !at_war {
            if g.envoys_at(pid, take.rival) > 0 {
                return Err("our envoys stand in that city-state".to_string());
            }
            if let Some(suzerain) = g.suzerain_of(take.rival) {
                if suzerain == pid || g.military_power(suzerain) > ratio * ours {
                    return Err("its suzerain is us or stronger than us".to_string());
                }
            }
        }
        if self.score_take_graded() {
            self.score_take_local(g, pid, take)?;
        }
        let production: f64 = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.city_yields(cid).production)
            .sum();
        let build = (take.production / (SCORE_TAKE_PRODUCTION_SHARE * production).max(1.0)).ceil();
        let march = self
            .score_take_standing(g, pid, g.cities[&take.city].pos, SCORE_TAKE_REACH)
            .iter()
            .take(take.standing)
            .map(|(_, _, _, first)| *first)
            .max()
            .unwrap_or(0)
            .max(
                g.player_city_ids(pid)
                    .into_iter()
                    .map(|cid| g.wdist(g.cities[&cid].pos, g.cities[&take.city].pos))
                    .min()
                    .map_or(0, |tiles| (tiles as u32).div_ceil(2)),
            );
        let lands = g.turn + build as u32 + march + take.turns + SCORE_TAKE_TALLY_MARGIN;
        if g.max_turns > 0 && lands > g.max_turns {
            return Err("the capture would land after the tally".to_string());
        }
        Ok(())
    }

    /// The graded versions' local circumstances: the owner's soldiers in
    /// sight near the city no stronger than the corps, every city of ours in
    /// the owner's reach walled and garrisoned, the treasury carrying the
    /// corps, and (the grab) the capture within its quick window.
    pub(crate) fn score_take_local(
        &self,
        g: &Game,
        pid: usize,
        take: &ScoreTake,
    ) -> Result<(), String> {
        let Some(city) = g.cities.get(&take.city) else {
            return Err("the city is gone".to_string());
        };
        if self.score_take_version() == 4 && take.turns > SCORE_TAKE_QUICK_TURNS {
            return Err(format!(
                "the corps takes it in {} turns, over the {SCORE_TAKE_QUICK_TURNS} a grab at any odds allows",
                take.turns
            ));
        }
        let trainable = Self::score_take_trainable(g, pid);
        let standing: f64 = self
            .score_take_standing(g, pid, city.pos, SCORE_TAKE_REACH)
            .iter()
            .take(take.standing)
            .map(|(_, strength, hp, _)| effective_strength(*strength, *hp))
            .sum();
        let new = (take.bodies - take.standing) as f64;
        let corps = standing + new * trainable.map_or(0.0, |(strength, _, _)| strength);
        let visible = self.battlefront_visibility(g, pid);
        let theirs: f64 = g
            .units
            .values()
            .filter(|unit| unit.owner == city.owner)
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .filter(|unit| g.wdist(unit.pos, city.pos) <= SCORE_TAKE_LOCAL_REACH)
            .filter(|unit| g.sees(&visible, unit.pos) && g.unit_visible_to(unit.id, pid))
            .map(|unit| effective_strength(g.unit_strength(unit, true), unit.hp))
            .sum();
        if theirs > corps {
            return Err(format!(
                "its soldiers within {SCORE_TAKE_LOCAL_REACH} tiles ({theirs:.0}) outweigh the corps ({corps:.0})"
            ));
        }
        let owner_cities: Vec<Pos> = g
            .player_city_ids(city.owner)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        for ours in g.player_city_ids(pid) {
            let home = &g.cities[&ours];
            if !owner_cities
                .iter()
                .any(|pos| g.wdist(*pos, home.pos) <= SCORE_TAKE_BORDER_REACH)
            {
                continue;
            }
            let garrisoned = g.unit_ids_at(home.pos).iter().any(|uid| {
                g.units.get(uid).is_some_and(|unit| {
                    unit.owner == pid && g.rules.units[unit.kind].class == "military"
                })
            });
            if home.wall_hp <= 0 || !garrisoned {
                return Err(format!(
                    "{} lies within {SCORE_TAKE_BORDER_REACH} tiles of its owner without {}",
                    home.name,
                    match (home.wall_hp > 0, garrisoned) {
                        (false, false) => "Walls or a garrison",
                        (false, true) => "Walls",
                        _ => "a garrison",
                    }
                ));
            }
        }
        let upkeep = take.bodies as f64 * trainable.map_or(2.0, |(_, _, upkeep)| upkeep.max(1.0));
        let gold = g.players[pid].gold;
        let per_turn = g.players[pid].gold_per_turn - upkeep;
        if !self.treasury_can_carry_a_war(g, pid)
            || (per_turn < 0.0 && gold + per_turn * SCORE_TAKE_FUNDED_TURNS < 0.0)
        {
            return Err(format!(
                "the treasury would not carry the corps ({gold:.0} Gold at {per_turn:+.0} a turn with its {upkeep:.0} upkeep)"
            ));
        }
        Ok(())
    }

    /// The city the plan takes: the best paying city whose circumstances
    /// hold. `None` with the gene off or off the Score lane.
    pub(crate) fn score_take_pick(
        &self,
        g: &Game,
        pid: usize,
        threatened: bool,
    ) -> Option<ScoreTake> {
        if !self.score_take_on(g) {
            return None;
        }
        self.score_take_board(g, pid)
            .into_iter()
            .filter(Self::score_take_pays)
            .find(|take| {
                self.score_take_circumstances(g, pid, take, threatened)
                    .is_ok()
            })
    }

    /// Once a turn: record the city the plan names, journal the board when
    /// it changes, journal a capture with its points and what the war cost,
    /// and (graded) keep the captured centres and the owners we took from.
    /// Exact no-op off.
    pub(crate) fn score_take_observe(&mut self, g: &Game, pid: usize) {
        if !self.score_take_on(g) {
            self.score_take_state = ScoreTakeState::default();
            return;
        }
        // A capture of the named city.
        if let Some((pos, civ, _)) = self.score_take_state.named.clone() {
            if let Some(cid) = g.city_at(pos).filter(|cid| g.cities[cid].owner == pid) {
                let city = &g.cities[&cid];
                let rival = g.players.iter().find(|p| p.civ == civ).map(|p| p.id);
                let cost = match (rival, self.score_take_state.war_base.as_ref()) {
                    (Some(rival), Some((base_civ, base))) if *base_civ == civ => {
                        let now = self.one_war_ledger(g, pid, rival);
                        format!(
                            "; the war cost {} unit(s) and {} city(ies) of ours, against {} of theirs",
                            now.0.saturating_sub(base.0),
                            now.2.saturating_sub(base.2),
                            now.1.saturating_sub(base.1)
                        )
                    }
                    _ => String::new(),
                };
                think!(self.journal(), Strategy, Strategy,
                       "Took {} from {}", city.name, civ;
                       "score-seat-takes-what-pays: {:.0} points to us ({} citizens, {} districts, {} buildings, {} wonders){cost}; the board names the next paying city or none",
                       Self::score_city_points(g, cid), city.pop, city.districts.len(),
                       city.buildings.len(), city.wonders.len();
                       city.pos);
                if self.score_take_graded() {
                    let until = g.turn + g.standard_duration(SCORE_TAKE_HOLD_TURNS);
                    self.score_take_state.held.insert(pos, (civ.clone(), until));
                    self.score_take_state.took_from.insert(civ, g.turn);
                }
            }
        }
        // Held centres expire or fall; owners we made peace with are done.
        self.score_take_state.held.retain(|pos, (_, until)| {
            g.turn < *until
                && g.city_at(*pos)
                    .is_some_and(|cid| g.cities[&cid].owner == pid)
        });
        self.score_take_state.took_from.retain(|civ, _| {
            g.players
                .iter()
                .any(|p| p.civ == *civ && p.alive && g.is_at_war(pid, p.id))
        });
        let plan = self.plan.clone();
        let threatened = plan
            .as_ref()
            .is_some_and(|plan| plan.threatened_city.is_some());
        let pick = self.score_take_pick(g, pid, threatened);
        let named = pick.as_ref().filter(|take| {
            plan.as_ref().is_some_and(|plan| {
                plan.strategy == GrandStrategy::Conquest && plan.target_city == Some(take.city)
            })
        });
        self.score_take_state.named = named.map(|take| {
            (
                g.cities[&take.city].pos,
                g.players[take.rival].civ.clone(),
                take.swing,
            )
        });
        // The war on the named city's owner: our ledger when it began.
        match named {
            Some(take) if g.is_at_war(pid, take.rival) => {
                let civ = g.players[take.rival].civ.clone();
                if self
                    .score_take_state
                    .war_base
                    .as_ref()
                    .is_none_or(|(base_civ, _)| *base_civ != civ)
                {
                    let base = self.one_war_ledger(g, pid, take.rival);
                    self.score_take_state.war_base = Some((civ, base));
                }
            }
            Some(_) => self.score_take_state.war_base = None,
            None => {}
        }
        self.journal_score_take_board(g, pid, pick.as_ref(), threatened);
    }

    /// Graded: whether peace is offered to `other` because we took a city of
    /// its in this war and no second paying city of its stands in reach
    /// (the grab: at once). `false` with the graded versions off.
    pub(crate) fn score_take_peace_due(&self, g: &Game, pid: usize, other: usize) -> bool {
        if !self.score_take_graded() || !self.score_take_on(g) || !g.is_at_war(pid, other) {
            return false;
        }
        let Some(civ) = g.players.get(other).map(|p| &p.civ) else {
            return false;
        };
        if !self.score_take_state.took_from.contains_key(civ) {
            return false;
        }
        self.score_take_version() == 4
            || self
                .score_take_pick(g, pid, false)
                .is_none_or(|take| take.rival != other)
    }

    /// Graded: a soldier of ours on a centre we took keeps it for
    /// [`SCORE_TAKE_HOLD_TURNS`]; an empty one takes the first soldier within
    /// [`SCORE_TAKE_FILL_REACH`] that can step onto it this turn. `None` with
    /// the graded versions off, or when neither applies.
    pub(super) fn score_take_garrison_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.score_take_graded()
            || self.score_take_state.held.is_empty()
            || !Self::centre_soldier(g, pid, uid)
        {
            return None;
        }
        let at = g.units.get(&uid)?.pos;
        let ours = |g: &Game, pos: Pos| {
            g.city_at(pos)
                .is_some_and(|cid| g.cities[&cid].owner == pid)
        };
        if self.score_take_state.held.contains_key(&at) && ours(g, at) {
            return Some(self.base.fortify_or_stop(g, pid, uid));
        }
        if g.units[&uid].moves_left <= 0.0
            || self.guard_is_bound_to_any_settler(uid)
            || self.guard_is_reserved_for_civilian(uid)
            || g.city_at(at).is_some_and(|cid| g.cities[&cid].owner == pid)
        {
            return None;
        }
        let empty = |g: &Game, pos: Pos| {
            !g.unit_ids_at(pos).iter().any(|id| {
                g.units.get(id).is_some_and(|other| {
                    other.owner == pid && g.rules.units[other.kind].class == "military"
                })
            })
        };
        let mut open: Vec<(i32, Pos)> = self
            .score_take_state
            .held
            .keys()
            .copied()
            .filter(|pos| ours(g, *pos) && empty(g, *pos))
            .map(|pos| (g.wdist(at, pos), pos))
            .filter(|(distance, _)| *distance <= SCORE_TAKE_FILL_REACH)
            .collect();
        open.sort();
        let reach = g.reachable(uid);
        let (_, to) = open.into_iter().find(|(_, pos)| reach.contains(pos))?;
        if !self.base.path_walk_to(g, pid, uid, to) {
            return None;
        }
        let kind = g.units[&uid].kind;
        let name = g
            .city_at(to)
            .map(|cid| g.cities[&cid].name.clone())
            .unwrap_or_default();
        think!(self.journal(), Military, Decision,
               "Garrisoning {name} with the {}", kind.as_str();
               "score-seat-takes-what-pays: a city we took holds a soldier in its centre for {SCORE_TAKE_HOLD_TURNS} standard turns (martial law, and no empty centre to retake)";
               to);
        Some(true)
    }

    fn journal_score_take_board(
        &mut self,
        g: &Game,
        pid: usize,
        pick: Option<&ScoreTake>,
        threatened: bool,
    ) {
        if !self.journal().wants(crate::reasoning::Level::Strategy) {
            return;
        }
        let board = self.score_take_board(g, pid);
        let describe = |take: &ScoreTake| {
            let city = &g.cities[&take.city];
            format!(
                "{} {} {:.0} pts{} for {:.0} Production ({} bodies, {} standing, {} turns to it, {} to take it; {:.1} expected a hundred)",
                g.players[take.rival].civ,
                city.name,
                take.points,
                if take.swing > take.points { " x2 from the leader" } else { "" },
                take.production,
                take.bodies,
                take.standing,
                take.march,
                take.turns,
                take.per_hundred
            )
        };
        let top: Vec<String> = board.iter().take(3).map(describe).collect();
        let why = match pick {
            Some(take) => format!("it takes {}", g.cities[&take.city].name),
            None => {
                let paying: Vec<&ScoreTake> = board
                    .iter()
                    .filter(|take| Self::score_take_pays(take))
                    .collect();
                match paying.first() {
                    None if board.is_empty() => {
                        "no rival or city-state city within reach stands open to the melee we field"
                            .to_string()
                    }
                    None => format!(
                        "no city pays: the best buys {:.1} expected points a hundred Production, the bar is {SCORE_TAKE_PER_HUNDRED:.0} at {SCORE_TAKE_MIN_POINTS:.0} points or more and at most {SCORE_TAKE_MAX_PRODUCTION:.0} Production",
                        board[0].per_hundred
                    ),
                    Some(take) => format!(
                        "{} pays but {}",
                        g.cities[&take.city].name,
                        self.score_take_circumstances(g, pid, take, threatened)
                            .err()
                            .unwrap_or_else(|| "its circumstances do not hold".to_string())
                    ),
                }
            }
        };
        let key = (pick.map(|take| take.city), why.clone());
        let repeat = self
            .score_take_state
            .line
            .as_ref()
            .is_none_or(|(city, reason, turn)| {
                (*city, reason.as_str()) != (key.0, key.1.as_str())
                    || g.turn.saturating_sub(*turn) >= SCORE_TAKE_LINE_EVERY
            });
        if !repeat {
            return;
        }
        self.score_take_state.line = Some((key.0, key.1, g.turn));
        let board_text = if top.is_empty() {
            "the board is empty".to_string()
        } else {
            top.join("; ")
        };
        let cap = if self.score_take_lifts_the_cap() {
            "; the plan names it, so the peacetime army cap is lifted"
        } else {
            ""
        };
        think!(self.journal(), Strategy, Strategy,
               "Score board: {}", why;
               "score-seat-takes-what-pays: {board_text}{cap}");
    }

    /// The turn's one declaration: the war for the plan's paying city, once
    /// the corps is mustered. Our takers within [`SCORE_TAKE_STAGE_REACH`]
    /// tiles and at most [`SCORE_TAKE_MUSTER_TURNS`] turns from the city must
    /// take it, march included, within [`SCORE_TAKE_WALL_WINDOW`] turns (the
    /// walls an open city raises after the war begins) and outweigh the
    /// owner's soldiers in sight. The shipped opening (a denouncement, then
    /// the Formal War) with the treasury, the one-war rule and a peace
    /// treaty still holding it. `false` with the gene off.
    pub(super) fn score_take_opening(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.score_take_on(g) || plan.strategy != GrandStrategy::Conquest {
            return false;
        }
        let Some((rival, cid)) = self.score_take_named_city(g, pid) else {
            return false;
        };
        if plan.target_city != Some(cid)
            || g.is_at_war(pid, rival)
            || g.turn < self.peace_until
            || !self.campaign_target_legal(g, pid, rival)
            || self.one_war_holds_declaration(g, pid, rival)
            || !self.war_is_affordable(g, pid)
        {
            return false;
        }
        let Some(city) = g.cities.get(&cid).cloned() else {
            return false;
        };
        if city.wall_hp > 0 {
            return false;
        }
        let window = self.score_take_window();
        let takers: Vec<(u32, f64, i32, u32)> = self
            .score_take_standing(g, pid, city.pos, SCORE_TAKE_STAGE_REACH)
            .into_iter()
            .filter(|(_, _, _, first)| *first <= SCORE_TAKE_MUSTER_TURNS.min(window))
            .collect();
        let force: Vec<(f64, i32, u32)> = takers
            .iter()
            .map(|(_, strength, hp, first)| (*strength, *hp, *first))
            .collect();
        let (defence, theirs) = self.score_take_defence(g, pid, cid);
        let ids: Vec<u32> = takers.iter().map(|taker| taker.0).collect();
        let ours = Self::campaign_strength_of(g, &ids);
        let reach = takers
            .iter()
            .map(|(_, _, _, first)| *first)
            .max()
            .unwrap_or(0);
        let capture = Self::score_take_turns(&force, defence, city.hp, window);
        // Graded: the assault forecast, read as if the war began this turn,
        // must give the mustered takers a calibrated chance within the window.
        let reading = if self.score_take_graded() && capture.is_some() && theirs <= ours {
            let forecast = self.assault_forecast_at_peace(g, pid, cid, &ids);
            Some((
                forecast
                    .as_ref()
                    .map_or(0.0, |f| f.calibrated_within(window as usize, 0)),
                forecast.as_ref().map_or(0.0, |f| f.units_lost),
            ))
        } else {
            None
        };
        let forecast_short = reading.is_some_and(|(p, _)| p < SCORE_TAKE_FORECAST_P);
        let Some(turns) = capture.filter(|_| theirs <= ours && !forecast_short) else {
            // Still mustering: say so at most every five turns.
            let due = self
                .score_take_state
                .muster_line
                .is_none_or(|turn| g.turn.saturating_sub(turn) >= 5);
            if due && self.journal().wants(crate::reasoning::Level::Detail) {
                self.score_take_state.muster_line = Some(g.turn);
                let civ = g.players[rival].civ.clone();
                let why = if theirs > ours {
                    "are outweighed".to_string()
                } else if let Some((p, _)) = reading.filter(|_| forecast_short) {
                    format!(
                        "read a calibrated {:.0}% within the {window}-turn window, under the {:.0}% the war asks",
                        100.0 * p,
                        100.0 * SCORE_TAKE_FORECAST_P
                    )
                } else {
                    format!("do not take it within the {window}-turn window")
                };
                think!(self.journal(), Military, Detail,
                       "Mustering for {} before the war on {civ}", city.name;
                       "score-seat-takes-what-pays: {} taker(s) within {} tiles and {} turns of it ({ours:.0} strength against {theirs:.0} in sight) {why}; the war waits, since its walls come about {SCORE_TAKE_WALL_WINDOW} turns after it begins; the city is named, so the peacetime army cap is lifted while the corps is raised and walks in",
                       takers.len(), SCORE_TAKE_STAGE_REACH, SCORE_TAKE_MUSTER_TURNS.min(window);
                       city.pos);
            }
            return false;
        };
        let Some(action) = self.preferred_war_opening(g, pid, rival) else {
            return false;
        };
        let denounce = matches!(action, Action::Denounce { .. });
        let civ = g.players[rival].civ.clone();
        let (power, their_power) = (g.military_power(pid), g.military_power(rival));
        let points = Self::score_city_points(g, cid);
        if g.apply(pid, &action).is_err() {
            return false;
        }
        let what = if denounce {
            "Denouncing"
        } else {
            "Declaring war on"
        };
        let forecast = reading.map_or_else(String::new, |(p, lost)| {
            format!(
                "; the forecast reads a calibrated {:.0}% within {window} turns ({:.1} expected points at {SCORE_TAKE_FORECAST_FACTOR}), {lost:.1} unit(s) expected lost",
                100.0 * p,
                points * SCORE_TAKE_FORECAST_FACTOR * f64::from(p)
            )
        });
        think!(self.journal(), Military, Strategy,
               "{what} {civ} for {}", city.name;
               "score-seat-takes-what-pays: {} is worth {points:.0} points; {} taker(s) stand within {reach} turn(s) of it and take it by turn +{turns} against a centre of {defence:.0} ({ours:.0} strength against {theirs:.0} in sight), inside the {window}-turn window before its walls{forecast}; {power:.0} power against their {their_power:.0} (version {}){}",
               city.name, takers.len(), self.score_take_version(),
               if denounce { "; the Formal War opens once the denouncement's five turns have passed, and the window is read again then" } else { "" };
               city.pos);
        true
    }
}

#[cfg(test)]
mod tests;
