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
//! - **After.** A capture is journalled with its points; the board then
//!   names the next paying city or none, and the shipped peace desk decides.

use super::open_city_objective::open_city_take_turns;
use super::soft_city_window::SOFT_CITY_GARRISON_BONUS;
use super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::{effective_strength, Game, Item};
use crate::think;

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

/// What the gene named last turn and has journalled.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ScoreTakeState {
    /// The city the plan is taking, its owner and its points when named.
    pub(crate) named: Option<(usize, u32, f64)>,
    /// The last board line: its key and turn.
    pub(crate) line: Option<(Option<u32>, String, u32)>,
    /// The last turn the declaration said it was holding for the muster.
    pub(crate) muster_line: Option<u32>,
}

impl AdvancedAi {
    /// Whether the gene acts for this seat: on, and on the Score lane.
    pub(crate) fn score_take_on(&self, g: &Game) -> bool {
        self.score_seat_takes_what_pays
            && self.active_victory_target(g) == Some(VictoryTarget::Score)
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
    /// the plan names a paying city. `false` with the gene off.
    pub(crate) fn score_take_lifts_the_cap(&self, g: &Game) -> bool {
        self.score_take_names_a_city(g)
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
    fn score_take_trainable(g: &Game, pid: usize) -> Option<(f64, f64)> {
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
                    .map(|cost| (spec.strength, cost))
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
                let (strength, _) = trainable?;
                force.extend(std::iter::repeat((strength, 100, 1)).take(new));
            }
            if let Some(turns) = Self::score_take_turns(&force, defence, city.hp, SCORE_TAKE_TURNS)
            {
                let production = new as f64 * trainable.map_or(0.0, |(_, cost)| cost);
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
    ) -> Result<(), &'static str> {
        let ours = g.military_power(pid);
        if g.military_power(take.rival) > SCORE_TAKE_POWER_RATIO * ours {
            return Err("its owner's military stands above ours");
        }
        if threatened || Self::our_cities_falling(g, pid) {
            return Err("a city of ours is threatened");
        }
        if self
            .one_war_enemies(g, pid)
            .into_iter()
            .any(|enemy| enemy != take.rival)
        {
            return Err("we are at war with another major");
        }
        let at_war = g.is_at_war(pid, take.rival);
        if !at_war && !self.campaign_target_legal(g, pid, take.rival) {
            return Err("the war would not be legal");
        }
        if take.minor && !at_war {
            if g.envoys_at(pid, take.rival) > 0 {
                return Err("our envoys stand in that city-state");
            }
            if let Some(suzerain) = g.suzerain_of(take.rival) {
                if suzerain == pid || g.military_power(suzerain) > SCORE_TAKE_POWER_RATIO * ours {
                    return Err("its suzerain is us or stronger than us");
                }
            }
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
            return Err("the capture would land after the tally");
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
    /// it changes, and journal a capture with its points. Exact no-op off.
    pub(crate) fn score_take_observe(&mut self, g: &Game, pid: usize) {
        if !self.score_take_on(g) {
            self.score_take_state = ScoreTakeState::default();
            return;
        }
        // A capture of the named city.
        if let Some((rival, cid, _)) = self.score_take_state.named {
            if g.cities.get(&cid).is_some_and(|city| city.owner == pid) {
                let city = &g.cities[&cid];
                think!(self.journal(), Strategy, Strategy,
                       "Took {} from {}", city.name, g.players[rival].civ;
                       "score-seat-takes-what-pays: {:.0} points to us ({} citizens, {} districts, {} buildings, {} wonders); the board names the next paying city or none",
                       Self::score_city_points(g, cid), city.pop, city.districts.len(),
                       city.buildings.len(), city.wonders.len();
                       city.pos);
            }
        }
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
        self.score_take_state.named = named.map(|take| (take.rival, take.city, take.swing));
        self.journal_score_take_board(g, pid, pick.as_ref(), threatened);
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
                            .unwrap_or("its circumstances do not hold")
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
        let cap = if self.score_take_lifts_the_cap(g) {
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
        let Some((rival, cid, _)) = self.score_take_state.named else {
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
        let takers: Vec<(u32, f64, i32, u32)> = self
            .score_take_standing(g, pid, city.pos, SCORE_TAKE_STAGE_REACH)
            .into_iter()
            .filter(|(_, _, _, first)| *first <= SCORE_TAKE_MUSTER_TURNS)
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
        let capture = Self::score_take_turns(&force, defence, city.hp, SCORE_TAKE_WALL_WINDOW);
        let Some(turns) = capture.filter(|_| theirs <= ours) else {
            // Still mustering: say so at most every five turns.
            let due = self
                .score_take_state
                .muster_line
                .is_none_or(|turn| g.turn.saturating_sub(turn) >= 5);
            if due && self.journal().wants(crate::reasoning::Level::Detail) {
                self.score_take_state.muster_line = Some(g.turn);
                let civ = g.players[rival].civ.clone();
                think!(self.journal(), Military, Detail,
                       "Mustering for {} before the war on {civ}", city.name;
                       "score-seat-takes-what-pays: {} taker(s) within {} tiles and {} turns of it ({ours:.0} strength against {theirs:.0} in sight) {}; the war waits, since its walls come about {} turns after it begins",
                       takers.len(), SCORE_TAKE_STAGE_REACH, SCORE_TAKE_MUSTER_TURNS,
                       if theirs > ours { "are outweighed".to_string() } else { format!("do not take it within the {SCORE_TAKE_WALL_WINDOW}-turn window") },
                       SCORE_TAKE_WALL_WINDOW;
                       city.pos);
            }
            return false;
        };
        let Some(action) = self.preferred_war_opening(g, pid, rival) else {
            return false;
        };
        let denounce = matches!(action, crate::game::Action::Denounce { .. });
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
        think!(self.journal(), Military, Strategy,
               "{what} {civ} for {}", city.name;
               "score-seat-takes-what-pays: {} is worth {points:.0} points; {} taker(s) stand within {reach} turn(s) of it and take it by turn +{turns} against a centre of {defence:.0} ({ours:.0} strength against {theirs:.0} in sight), inside the {SCORE_TAKE_WALL_WINDOW}-turn window before its walls; {power:.0} power against their {their_power:.0}{}",
               city.name, takers.len(),
               if denounce { "; the Formal War opens once the denouncement's five turns have passed, and the window is read again then" } else { "" };
               city.pos);
        true
    }
}

#[cfg(test)]
mod tests;
