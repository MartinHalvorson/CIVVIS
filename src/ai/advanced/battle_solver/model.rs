//! The local battle as plain data, and the rules that play it forward.
//!
//! Nothing here knows about `Game`. A snapshot (`extract.rs`) reads every
//! number from the engine's own arithmetic once; from then on a turn costs
//! microseconds instead of a board clone, which is what lets the search look
//! several turns out and still play fast. `extract_tests.rs` holds the model
//! to the engine on real positions so the two cannot drift.

use super::dice::{effective_strength, mean_damage};

pub const NO_TILE: u16 = u16::MAX;
pub const NO_UNIT: u16 = u16::MAX;
pub const NO_CITY: u8 = u8::MAX;
/// Fixed-point movement: one movement point. Twelfths are exact for the
/// engine's 1, 1/2, 1/3 and 1/4 route costs.
pub const MP: u16 = 12;
pub const IMPASSABLE: u16 = u16::MAX;

pub const F_ZOC: u8 = 1;
pub const F_IGNORES_ZOC: u8 = 2;
pub const F_MELEE: u8 = 4;
pub const F_RANGED: u8 = 8;
pub const F_SIEGE: u8 = 16;
pub const F_MOVE_AND_FIRE: u8 = 32;
pub const F_CAPTURES: u8 = 64;

pub const OURS: u8 = 0;
pub const THEIRS: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unit {
    pub tile: u16,
    pub hp: i16,
    pub mp: u16,
    pub max_mp: u16,
    pub value: f32,
    pub side: u8,
    pub attacks: u8,
    pub max_attacks: u8,
    pub fort: u8,
    pub flags: u8,
    pub range: u8,
    pub class: u8,
    pub acted: bool,
}

impl Unit {
    #[inline]
    pub fn alive(&self) -> bool {
        self.hp > 0
    }
    #[inline]
    pub fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }
    #[inline]
    pub fn shoots(&self) -> bool {
        self.flags & (F_RANGED | F_SIEGE) != 0
    }
    #[inline]
    pub fn fires_after_moving(&self) -> bool {
        !self.has(F_SIEGE) || self.has(F_MOVE_AND_FIRE)
    }
}

#[derive(Clone, Debug)]
pub struct CitySpec {
    pub tile: u16,
    /// Melee and ranged defence before the city's own wound penalty.
    pub strength: f32,
    pub wall_max: i16,
    /// The city's own strike, zero when it has none.
    pub strike: f32,
    pub value: f32,
    pub zoc: bool,
}

/// Everything about a battle that does not change while it is fought.
#[derive(Clone, Debug, Default)]
pub struct Board {
    pub tiles: usize,
    pub nbrs: Vec<[u16; 6]>,
    /// Bit `d`: the edge to `nbrs[tile][d]` crosses a river.
    pub river: Vec<u8>,
    pub defense: Vec<f32>,
    pub land: Vec<bool>,
    /// `tiles`×`tiles` hex distance.
    pub dist: Vec<u8>,
    /// `classes`×`tiles`×6 movement cost to enter each neighbour.
    pub edge: Vec<u16>,
    pub city_at: Vec<u8>,
    pub cities: Vec<CitySpec>,
    /// Heal per turn by tile and side.
    pub heal: Vec<[u8; 2]>,
    pub flank: [f32; 2],
    pub support: [f32; 2],
    pub n: usize,
    /// `n`×`n`, attacker then defender: strength before position and wounds.
    pub melee_att: Vec<f32>,
    pub melee_def: Vec<f32>,
    pub ranged_att: Vec<f32>,
    pub ranged_def: Vec<f32>,
    /// `n`×cities: a unit's strength against each city.
    pub city_melee: Vec<f32>,
    pub city_ranged: Vec<f32>,
    /// Exact first-turn reach of each unit from the engine: `(tile, moves
    /// kept)`. Empty for a unit the snapshot left to the model's flood.
    pub reach0: Vec<Vec<(u16, u16)>>,
    /// Sorted `(from, to)` pairs with line of sight, for this turn's shots.
    pub los0: Vec<(u16, u16)>,
    /// Whether `los0` is populated; a test board with none skips the check.
    pub has_los: bool,
}

impl Board {
    #[inline]
    pub fn d(&self, a: u16, b: u16) -> u8 {
        self.dist[a as usize * self.tiles + b as usize]
    }
    #[inline]
    pub fn edge_cost(&self, class: u8, tile: u16, dir: usize) -> u16 {
        self.edge[(class as usize * self.tiles + tile as usize) * 6 + dir]
    }
    #[inline]
    pub fn crosses_river(&self, from: u16, to: u16) -> bool {
        self.nbrs[from as usize]
            .iter()
            .position(|n| *n == to)
            .is_some_and(|d| self.river[from as usize] & (1 << d) != 0)
    }
    #[inline]
    pub fn shot_visible(&self, from: u16, to: u16) -> bool {
        !self.has_los || self.los0.binary_search(&(from, to)).is_ok()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Aim {
    None,
    Unit(u16),
    City(u8),
}

/// One unit's whole turn: where it ends, and what it strikes from there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Line {
    pub tile: u16,
    pub aim: Aim,
}

/// How our side plays the turns after the one being decided.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Posture {
    /// Strike what can be reached and press the objective.
    Press,
    /// Strike what comes into reach; give no ground and take none.
    Hold,
    /// Break contact; shoot only what can be shot for free.
    Withdraw,
}

/// What this battle is for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Objective {
    /// Destroy what is here at the best exchange.
    Field,
    /// Take this city.
    Capture(u8),
    /// Keep this city.
    Hold(u8),
}

#[derive(Clone, Debug)]
pub struct State {
    pub units: Vec<Unit>,
    /// Tile → unit standing there, or `NO_UNIT`.
    pub occ: Vec<u16>,
    pub city_hp: Vec<i16>,
    pub city_wall: Vec<i16>,
    pub city_side: Vec<u8>,
}

impl State {
    pub fn new(board: &Board, units: Vec<Unit>, cities: Vec<(i16, i16, u8)>) -> Self {
        let mut occ = vec![NO_UNIT; board.tiles];
        for (i, unit) in units.iter().enumerate() {
            if unit.alive() {
                occ[unit.tile as usize] = i as u16;
            }
        }
        Self {
            units,
            occ,
            city_hp: cities.iter().map(|c| c.0).collect(),
            city_wall: cities.iter().map(|c| c.1).collect(),
            city_side: cities.iter().map(|c| c.2).collect(),
        }
    }

    fn copy_from(&mut self, other: &State) {
        self.units.clear();
        self.units.extend_from_slice(&other.units);
        self.occ.clear();
        self.occ.extend_from_slice(&other.occ);
        self.city_hp.clear();
        self.city_hp.extend_from_slice(&other.city_hp);
        self.city_wall.clear();
        self.city_wall.extend_from_slice(&other.city_wall);
        self.city_side.clear();
        self.city_side.extend_from_slice(&other.city_side);
    }
}

#[inline]
fn mix(a: u64) -> u64 {
    let mut z = a.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Weights the rollouts score the end of the battle with, in hammers.
#[derive(Clone, Copy, Debug)]
pub struct Weights {
    /// What a lost hammer of ours costs against an enemy hammer destroyed.
    pub loss: f32,
    /// A share of a unit's value that its hit points do not touch: a wounded
    /// unit that lives is worth more than its hit points say.
    pub alive_floor: f32,
    /// Extra hammers for reaching the objective, on top of the city's value.
    pub objective_bonus: f32,
    /// Per-turn discount on the objective bonus: sooner is worth more.
    pub haste: f32,
    /// Enemy retreat threshold in rollouts.
    pub enemy_retreat_hp: i16,
    /// Our retreat threshold in rollouts.
    pub our_retreat_hp: i16,
    /// Chance a modelled enemy takes a favourable strike. Below one because
    /// the real AI does not always swing when it could; one is the prudent
    /// reading and the setting the benches use.
    pub enemy_aggression: f32,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            loss: 1.15,
            alive_floor: 0.35,
            objective_bonus: 150.0,
            haste: 0.92,
            enemy_retreat_hp: 25,
            our_retreat_hp: 40,
            enemy_aggression: 1.0,
        }
    }
}

/// The rollout engine: one state plus the scratch space to play it.
pub struct Sim<'b> {
    pub b: &'b Board,
    pub s: State,
    w: Weights,
    seed: u64,
    round: u32,
    objective: Objective,
    /// Round at which the objective was reached.
    pub reached: Option<u32>,
    stamp: u32,
    seen: Vec<u32>,
    left: Vec<u16>,
    queue: Vec<u16>,
    reach: Vec<(u16, u16)>,
    order: Vec<u16>,
    dice: u32,
}

impl<'b> Sim<'b> {
    pub fn new(b: &'b Board, s: &State, w: Weights, objective: Objective) -> Self {
        Self {
            b,
            s: s.clone(),
            w,
            seed: 0,
            round: 0,
            objective,
            reached: None,
            stamp: 0,
            seen: vec![0; b.tiles],
            left: vec![0; b.tiles],
            queue: Vec::with_capacity(b.tiles),
            reach: Vec::with_capacity(b.tiles),
            order: Vec::with_capacity(s.units.len()),
            dice: 0,
        }
    }

    pub fn reset(&mut self, s: &State, seed: u64) {
        self.s.copy_from(s);
        self.seed = seed;
        self.round = 0;
        self.reached = None;
        self.dice = 0;
    }

    /// One die per (unit, round, draw): a plan that changes one unit leaves
    /// every other unit's rolls alone, so two plans compare on the same luck.
    #[inline]
    fn roll(&mut self, unit: usize) -> f32 {
        self.dice = self.dice.wrapping_add(1);
        let key = self.seed
            ^ ((unit as u64) << 40)
            ^ ((self.round as u64) << 28)
            ^ (self.dice as u64 & 0xff_ffff);
        0.8 + 0.4 * ((mix(key) >> 40) as f32 / 16_777_216.0)
    }

    #[inline]
    fn damage(&mut self, att: f32, def: f32, unit: usize) -> i16 {
        let rolled = 30.0 * ((att - def) / 25.0).exp() * self.roll(unit);
        (rolled.round() as i32).clamp(1, 100) as i16
    }

    #[inline]
    fn adjacent_friends(&self, side: u8, tile: u16, exclude: usize, skip_river: bool) -> u32 {
        let mut count = 0;
        for (d, n) in self.b.nbrs[tile as usize].iter().enumerate() {
            if *n == NO_TILE {
                continue;
            }
            let at = self.s.occ[*n as usize];
            if at == NO_UNIT || at as usize == exclude {
                continue;
            }
            let other = &self.s.units[at as usize];
            if other.side != side {
                continue;
            }
            if skip_river && self.b.river[tile as usize] & (1 << d) != 0 {
                continue;
            }
            count += 1;
        }
        count
    }

    /// A blow's strengths for the exact volley: the attacker's after its
    /// wound penalty, the defence before the target's.
    pub fn blow_bases(&self, i: usize, j: usize) -> (f32, f32) {
        let (a, d) = (self.s.units[i], self.s.units[j]);
        let n = self.b.n;
        if a.shoots() {
            (
                effective_strength(self.b.ranged_att[i * n + j], a.hp as i32),
                self.b.ranged_def[i * n + j]
                    + 3.0 * d.fort as f32
                    + self.b.defense[d.tile as usize],
            )
        } else {
            let (att, _) = self.melee_strengths(i, j);
            let support = self.b.support[d.side as usize]
                * self.adjacent_friends(d.side, d.tile, j, false) as f32;
            let river = if self.b.crosses_river(a.tile, d.tile) {
                5.0
            } else {
                0.0
            };
            (
                att,
                self.b.melee_def[i * n + j]
                    + 3.0 * d.fort as f32
                    + self.b.defense[d.tile as usize]
                    + support
                    + river,
            )
        }
    }

    /// `melee_exchange_strengths`, read from the model.
    pub fn melee_strengths(&self, i: usize, j: usize) -> (f32, f32) {
        let (a, d) = (self.s.units[i], self.s.units[j]);
        let n = self.b.n;
        let target = d.tile;
        let flank =
            self.b.flank[a.side as usize] * self.adjacent_friends(a.side, target, i, true) as f32;
        let support = self.b.support[d.side as usize]
            * self.adjacent_friends(d.side, target, j, false) as f32;
        let river = if self.b.crosses_river(a.tile, target) {
            5.0
        } else {
            0.0
        };
        let att = self.b.melee_att[i * n + j] + flank;
        let def = self.b.melee_def[i * n + j]
            + 3.0 * d.fort as f32
            + self.b.defense[target as usize]
            + support
            + river;
        (
            effective_strength(att, a.hp as i32),
            effective_strength(def, d.hp as i32),
        )
    }

    /// `ranged_strike_strengths`, read from the model.
    pub fn ranged_strengths(&self, i: usize, j: usize) -> (f32, f32) {
        let (a, d) = (self.s.units[i], self.s.units[j]);
        let n = self.b.n;
        (
            effective_strength(self.b.ranged_att[i * n + j], a.hp as i32),
            effective_strength(
                self.b.ranged_def[i * n + j]
                    + 3.0 * d.fort as f32
                    + self.b.defense[d.tile as usize],
                d.hp as i32,
            ),
        )
    }

    #[inline]
    pub fn city_strength(&self, c: usize) -> f32 {
        let wounded = (10.0 - self.s.city_hp[c].clamp(0, 200) as f32 / 20.0).round();
        (self.b.cities[c].strength - wounded).max(0.0)
    }

    fn kill(&mut self, j: usize) {
        let tile = self.s.units[j].tile as usize;
        self.s.units[j].hp = 0;
        if self.s.occ[tile] == j as u16 {
            self.s.occ[tile] = NO_UNIT;
        }
    }

    fn relocate(&mut self, i: usize, to: u16) {
        let from = self.s.units[i].tile as usize;
        if self.s.occ[from] == i as u16 {
            self.s.occ[from] = NO_UNIT;
        }
        self.s.units[i].tile = to;
        self.s.occ[to as usize] = i as u16;
    }

    fn spend_attack(&mut self, i: usize) {
        let unit = &mut self.s.units[i];
        unit.attacks = unit.attacks.saturating_sub(1);
        if unit.attacks == 0 {
            unit.mp = 0;
        }
        unit.fort = 0;
        unit.acted = true;
    }

    /// `do_attack` against a unit.
    pub fn strike_melee(&mut self, i: usize, j: usize) {
        let (att, def) = self.melee_strengths(i, j);
        let out = self.damage(att, def, i);
        let back = self.damage(def, att, i);
        self.s.units[j].hp -= out;
        self.s.units[i].hp -= back;
        self.spend_attack(i);
        let target = self.s.units[j].tile;
        let dead = self.s.units[j].hp <= 0;
        if dead {
            self.kill(j);
        }
        if self.s.units[i].hp <= 0 {
            self.kill(i);
            return;
        }
        if dead
            && self.s.occ[target as usize] == NO_UNIT
            && self.b.city_at[target as usize] == NO_CITY
        {
            self.relocate(i, target);
        }
    }

    /// `do_ranged` against a unit.
    pub fn strike_ranged(&mut self, i: usize, j: usize) {
        let (att, def) = self.ranged_strengths(i, j);
        let out = self.damage(att, def, i);
        self.s.units[j].hp -= out;
        self.spend_attack(i);
        if self.s.units[j].hp <= 0 {
            self.kill(j);
        }
    }

    /// `city_take_damage`.
    fn city_takes(&mut self, c: usize, dmg: i16, wall_mult: f32) {
        let (wall, max) = (self.s.city_wall[c], self.b.cities[c].wall_max);
        if wall > 0 && max > 0 {
            let frac = wall as f32 / max as f32;
            let through = if frac >= 0.8 {
                1
            } else if frac >= 0.2 {
                dmg / 2
            } else {
                dmg
            };
            let hit = ((dmg as f32 * wall_mult).round() as i16).max(1);
            self.s.city_wall[c] = (wall - hit).max(0);
            self.s.city_hp[c] -= through.max(1);
        } else {
            self.s.city_hp[c] -= dmg;
        }
    }

    fn capture(&mut self, i: usize, c: usize) {
        let side = self.s.units[i].side;
        self.s.city_side[c] = side;
        self.s.city_hp[c] = self.s.city_hp[c].max(1);
        self.relocate(i, self.b.cities[c].tile);
        let reached = match self.objective {
            Objective::Capture(city) => city as usize == c && side == OURS,
            _ => false,
        };
        if reached && self.reached.is_none() {
            self.reached = Some(self.round);
        }
    }

    /// `do_attack` against a city; captures when it brings the city down.
    pub fn strike_city_melee(&mut self, i: usize, c: usize) {
        if self.s.city_hp[c] <= 0 {
            self.spend_attack(i);
            if self.s.units[i].has(F_CAPTURES) {
                self.capture(i, c);
            }
            return;
        }
        let att = effective_strength(
            self.b.city_melee[i * self.b.cities.len() + c],
            self.s.units[i].hp as i32,
        );
        let def = self.city_strength(c);
        let out = self.damage(att, def, i);
        let back = self.damage(def, att, i);
        self.city_takes(c, out, 0.15);
        self.s.units[i].hp -= back;
        self.spend_attack(i);
        if self.s.units[i].hp <= 0 {
            self.kill(i);
            self.s.city_hp[c] = self.s.city_hp[c].max(1);
            return;
        }
        if self.s.city_hp[c] <= 0 && self.s.units[i].has(F_CAPTURES) {
            self.capture(i, c);
        }
    }

    /// `do_ranged` against a city.
    pub fn strike_city_ranged(&mut self, i: usize, c: usize) {
        let att = effective_strength(
            self.b.city_ranged[i * self.b.cities.len() + c],
            self.s.units[i].hp as i32,
        );
        let def = self.city_strength(c);
        let was = self.s.city_hp[c];
        let dmg = self.damage(att, def, i);
        let siege = self.s.units[i].has(F_SIEGE);
        self.city_takes(c, dmg, if siege { 1.0 } else { 0.5 });
        self.s.city_hp[c] = if was <= 0 || (self.s.city_hp[c] <= 0 && siege) {
            0
        } else {
            self.s.city_hp[c].max(1)
        };
        self.spend_attack(i);
    }

    #[inline]
    fn in_zoc(&self, side: u8, tile: u16, ignores: bool) -> bool {
        if ignores {
            return false;
        }
        for n in self.b.nbrs[tile as usize] {
            if n == NO_TILE {
                continue;
            }
            let at = self.s.occ[n as usize];
            if at != NO_UNIT {
                let other = &self.s.units[at as usize];
                if other.side != side && other.has(F_ZOC) {
                    return true;
                }
            }
            let c = self.b.city_at[n as usize];
            if c != NO_CITY && self.b.cities[c as usize].zoc && self.s.city_side[c as usize] != side
            {
                return true;
            }
        }
        false
    }

    /// Flood this unit's movement into `self.reach`: `(tile, moves kept)`
    /// for every tile it may stop on, its own tile included. Stops at a zone
    /// of control but keeps the movement for the blow, as `approach_reach`
    /// does.
    fn flood(&mut self, i: usize) {
        let unit = self.s.units[i];
        self.stamp = self.stamp.wrapping_add(1);
        self.reach.clear();
        self.queue.clear();
        let start = unit.tile;
        self.seen[start as usize] = self.stamp;
        self.left[start as usize] = unit.mp;
        self.reach.push((start, unit.mp));
        if unit.mp == 0 {
            return;
        }
        self.queue.push(start);
        let ignores = unit.has(F_IGNORES_ZOC);
        let mut head = 0;
        while head < self.queue.len() {
            let tile = self.queue[head];
            head += 1;
            let have = self.left[tile as usize];
            if tile != start && self.in_zoc(unit.side, tile, ignores) {
                continue;
            }
            for dir in 0..6 {
                let next = self.b.nbrs[tile as usize][dir];
                if next == NO_TILE {
                    continue;
                }
                let cost = self.b.edge_cost(unit.class, tile, dir);
                if cost == IMPASSABLE || cost > have {
                    continue;
                }
                let at = self.s.occ[next as usize];
                if at != NO_UNIT && self.s.units[at as usize].side != unit.side {
                    continue;
                }
                let city = self.b.city_at[next as usize];
                if city != NO_CITY && self.s.city_side[city as usize] != unit.side {
                    continue;
                }
                let kept = have - cost;
                let fresh = self.seen[next as usize] != self.stamp;
                if fresh || self.left[next as usize] < kept {
                    if fresh && at == NO_UNIT {
                        self.reach.push((next, kept));
                    } else if at == NO_UNIT {
                        if let Some(slot) = self.reach.iter_mut().find(|r| r.0 == next) {
                            slot.1 = kept;
                        }
                    }
                    self.seen[next as usize] = self.stamp;
                    self.left[next as usize] = kept;
                    self.queue.push(next);
                }
            }
        }
    }

    /// [`Self::flood`] for the search's own reading of a unit's reach.
    pub fn flood_public(&mut self, i: usize) {
        self.flood(i);
    }

    pub fn reach_public(&self) -> &[(u16, u16)] {
        &self.reach
    }

    #[inline]
    fn reachable_with(&self, tile: u16) -> Option<u16> {
        (self.seen[tile as usize] == self.stamp).then(|| self.left[tile as usize])
    }

    /// Walk to a stand and strike: the turn a `Line` describes.
    pub fn play_line(&mut self, i: usize, line: Line) -> bool {
        let unit = self.s.units[i];
        if !unit.alive() {
            return false;
        }
        if line.tile != unit.tile {
            let free = self.s.occ[line.tile as usize] == NO_UNIT;
            if !free {
                return false;
            }
            self.relocate(i, line.tile);
            self.s.units[i].fort = 0;
            self.s.units[i].acted = true;
        }
        match line.aim {
            Aim::None => true,
            Aim::Unit(j) => {
                let j = j as usize;
                let target = self.s.units[j];
                if !target.alive() || self.s.units[i].attacks == 0 {
                    return false;
                }
                let d = self.b.d(self.s.units[i].tile, target.tile);
                if unit.shoots() {
                    if d as i32 > unit.range as i32 {
                        return false;
                    }
                    self.strike_ranged(i, j);
                } else {
                    if d != 1 {
                        return false;
                    }
                    self.strike_melee(i, j);
                }
                true
            }
            Aim::City(c) => {
                let c = c as usize;
                if self.s.city_side[c] == unit.side || self.s.units[i].attacks == 0 {
                    return false;
                }
                let d = self.b.d(self.s.units[i].tile, self.b.cities[c].tile);
                if unit.shoots() {
                    if d as i32 > unit.range as i32 {
                        return false;
                    }
                    self.strike_city_ranged(i, c);
                } else {
                    if d != 1 {
                        return false;
                    }
                    self.strike_city_melee(i, c);
                }
                true
            }
        }
    }

    /// The price of a blow to the scripted player: damage dealt less damage
    /// taken, in hammers, with a kill counted whole.
    fn blow_value(&self, i: usize, dealt: f32, taken: f32, hp: i16, target_value: f32) -> f32 {
        let me = self.s.units[i];
        let killed = dealt >= hp as f32;
        let dies = taken >= me.hp as f32 && !killed;
        let mut v = (dealt.min(hp as f32) / 100.0) * target_value;
        if killed {
            v += 0.8 * target_value;
        }
        v -= (taken.min(me.hp as f32) / 100.0) * me.value;
        if dies {
            v -= 0.8 * me.value;
        }
        v
    }

    fn enemy_near(&self, side: u8, tile: u16) -> (u16, u8) {
        let mut best = (NO_TILE, u8::MAX);
        for other in &self.s.units {
            if other.alive() && other.side != side {
                let d = self.b.d(tile, other.tile);
                if d < best.1 {
                    best = (other.tile, d);
                }
            }
        }
        best
    }

    /// The scripted player's turn for one unit.
    fn act(&mut self, i: usize, posture: Posture) {
        let me = self.s.units[i];
        if !me.alive() || me.mp == 0 {
            return;
        }
        self.flood(i);
        let side = me.side;
        let retreat_hp = if side == OURS {
            self.w.our_retreat_hp
        } else {
            self.w.enemy_retreat_hp
        };
        let breaking = side == OURS && posture == Posture::Withdraw;
        let low = me.hp <= retreat_hp;
        let holds = side == OURS && posture == Posture::Hold;
        // The best blow within reach. An even trade is a blow worth striking:
        // whoever hits first leaves the other weaker for its answer, and a
        // scripted line that refuses even trades never closes at all.
        let mut best_value = if side == THEIRS || posture == Posture::Press {
            -12.0f32
        } else {
            0.0f32
        };
        let mut best: Option<(u16, Aim)> = None;
        if me.attacks > 0 {
            for j in 0..self.s.units.len() {
                let other = self.s.units[j];
                if !other.alive() || other.side == side {
                    continue;
                }
                let range = self.b.d(me.tile, other.tile);
                if range > 8 {
                    continue;
                }
                if me.shoots() {
                    if !me.fires_after_moving() {
                        if range as i32 > me.range as i32 {
                            continue;
                        }
                    }
                    let (att, def) = self.ranged_strengths(i, j);
                    let dealt = mean_damage(att, def);
                    let v = self.blow_value(i, dealt, 0.0, other.hp, other.value);
                    if v > best_value {
                        // The shooter's stand: its own tile if in range, else
                        // the nearest reach tile that is.
                        let stand = if range as i32 <= me.range as i32 {
                            Some(me.tile)
                        } else if holds || !me.fires_after_moving() {
                            None
                        } else {
                            self.reach
                                .iter()
                                .filter(|(t, kept)| {
                                    *kept > 0
                                        && (self.b.d(*t, other.tile) as i32) <= me.range as i32
                                })
                                .min_by_key(|(t, _)| self.b.d(*t, me.tile))
                                .map(|(t, _)| *t)
                        };
                        if let Some(stand) = stand {
                            best_value = v;
                            best = Some((stand, Aim::Unit(j as u16)));
                        }
                    }
                } else if me.has(F_MELEE) {
                    if breaking {
                        continue;
                    }
                    // The cheapest stand beside the target, preferring cover.
                    let mut stand: Option<(u16, f32)> = None;
                    for (d, t) in self.b.nbrs[other.tile as usize].iter().enumerate() {
                        let _ = d;
                        if *t == NO_TILE {
                            continue;
                        }
                        let free = self.s.occ[*t as usize] == NO_UNIT || *t == me.tile;
                        if !free {
                            continue;
                        }
                        let Some(kept) = self.reachable_with(*t) else {
                            continue;
                        };
                        if kept == 0 && *t != me.tile {
                            continue;
                        }
                        if holds && *t != me.tile {
                            continue;
                        }
                        let score = self.b.defense[*t as usize];
                        if stand.is_none_or(|(_, s)| score > s) {
                            stand = Some((*t, score));
                        }
                    }
                    let Some((stand, _)) = stand else {
                        continue;
                    };
                    let (att, def) = self.melee_strengths(i, j);
                    let dealt = mean_damage(att, def);
                    let taken = mean_damage(def, att);
                    let v = self.blow_value(i, dealt, taken, other.hp, other.value);
                    if v > best_value {
                        best_value = v;
                        best = Some((stand, Aim::Unit(j as u16)));
                    }
                }
            }
            // Cities.
            for c in 0..self.b.cities.len() {
                if self.s.city_side[c] == side {
                    continue;
                }
                let tile = self.b.cities[c].tile;
                let range = self.b.d(me.tile, tile);
                if me.shoots() && (range as i32) <= me.range as i32 {
                    let dealt = mean_damage(
                        effective_strength(
                            self.b.city_ranged[i * self.b.cities.len() + c],
                            me.hp as i32,
                        ),
                        self.city_strength(c),
                    );
                    let v = dealt * 0.6 + self.city_pull(c, side);
                    if v > best_value {
                        best_value = v;
                        best = Some((me.tile, Aim::City(c as u8)));
                    }
                } else if me.has(F_MELEE) && !breaking {
                    let taken = mean_damage(
                        self.city_strength(c),
                        effective_strength(
                            self.b.city_melee[i * self.b.cities.len() + c],
                            me.hp as i32,
                        ),
                    );
                    if self.s.city_wall[c] > 0
                        && self.s.city_hp[c] > 30
                        && taken * 2.0 > me.hp as f32
                    {
                        continue;
                    }
                    if self.b.d(me.tile, tile) > 3 {
                        continue;
                    }
                    let mut stand = None;
                    for t in self.b.nbrs[tile as usize] {
                        if t == NO_TILE {
                            continue;
                        }
                        let free = self.s.occ[t as usize] == NO_UNIT || t == me.tile;
                        if free
                            && self
                                .reachable_with(t)
                                .is_some_and(|k| k > 0 || t == me.tile)
                        {
                            stand = Some(t);
                            break;
                        }
                    }
                    let Some(stand) = stand else { continue };
                    let dealt = mean_damage(
                        effective_strength(
                            self.b.city_melee[i * self.b.cities.len() + c],
                            me.hp as i32,
                        ),
                        self.city_strength(c),
                    );
                    let v = (dealt - taken) * 0.5 + self.city_pull(c, side);
                    if v > best_value {
                        best_value = v;
                        best = Some((stand, Aim::City(c as u8)));
                    }
                }
            }
        }
        if side == THEIRS && self.w.enemy_aggression < 1.0 {
            let swings = (self.roll(i) - 0.8) / 0.4 < self.w.enemy_aggression;
            if !swings {
                best = None;
            }
        }
        if let Some((stand, aim)) = best {
            if !low || matches!(aim, Aim::Unit(_)) {
                self.play_line(i, Line { tile: stand, aim });
                return;
            }
        }
        if me.mp == 0 || (holds && !low) {
            return;
        }
        // Nothing to strike: move.
        let (near, dist) = self.enemy_near(side, me.tile);
        if near == NO_TILE {
            return;
        }
        let goal = if low || breaking {
            NO_TILE
        } else {
            self.goal(side, near)
        };
        let mut pick = me.tile;
        let mut pick_score = f32::MIN;
        for k in 0..self.reach.len() {
            let (t, _) = self.reach[k];
            let enemy_gap = self.enemy_near(side, t).1 as f32;
            let score = if low || breaking {
                enemy_gap * 3.0
                    + self.b.heal[t as usize][side as usize] as f32 * 0.1
                    + self.b.defense[t as usize] * 0.1
            } else {
                let closes = self.b.d(t, goal) as f32;
                let wanted = if me.shoots() { me.range as f32 } else { 1.0 };
                -(closes - wanted).abs() + self.b.defense[t as usize] * 0.05
            };
            if score > pick_score {
                pick_score = score;
                pick = t;
            }
        }
        let _ = dist;
        if pick != me.tile {
            self.play_line(
                i,
                Line {
                    tile: pick,
                    aim: Aim::None,
                },
            );
        }
    }

    #[inline]
    fn city_pull(&self, c: usize, side: u8) -> f32 {
        match self.objective {
            Objective::Capture(city) if city as usize == c && side == OURS => 40.0,
            Objective::Hold(city) if city as usize == c && side == THEIRS => 40.0,
            _ => 10.0,
        }
    }

    fn goal(&self, side: u8, near: u16) -> u16 {
        match self.objective {
            Objective::Capture(c) if side == OURS => self.b.cities[c as usize].tile,
            Objective::Hold(c) if side == THEIRS => self.b.cities[c as usize].tile,
            _ => near,
        }
    }

    /// Start of a side's turn: `turn.rs`'s heal, fresh moves, fortify.
    pub fn begin_side(&mut self, side: u8) {
        for i in 0..self.s.units.len() {
            let unit = self.s.units[i];
            if !unit.alive() || unit.side != side {
                continue;
            }
            let heal = if unit.acted {
                0
            } else {
                self.b.heal[unit.tile as usize][side as usize] as i16
            };
            let u = &mut self.s.units[i];
            u.hp = (u.hp + heal).min(100);
            u.mp = u.max_mp;
            u.attacks = u.max_attacks;
            if !u.acted && u.fort < 2 && !u.shoots() {
                u.fort += 1;
            }
            u.acted = false;
        }
        for c in 0..self.b.cities.len() {
            if self.s.city_side[c] != side {
                continue;
            }
            if !self.besieged(c) {
                self.s.city_hp[c] = (self.s.city_hp[c] + 20).min(200);
            }
            // A city with walls standing strikes the most wounded unit in
            // range: the host AI's and our own habit.
            let strike = self.b.cities[c].strike;
            if strike > 0.0 && self.s.city_wall[c] > 0 {
                let tile = self.b.cities[c].tile;
                let mut target = None;
                let mut lowest = i16::MAX;
                for j in 0..self.s.units.len() {
                    let other = self.s.units[j];
                    if other.alive() && other.side != side && self.b.d(tile, other.tile) <= 2 {
                        if other.hp < lowest {
                            lowest = other.hp;
                            target = Some(j);
                        }
                    }
                }
                if let Some(j) = target {
                    let def = effective_strength(
                        self.b.melee_def[j * self.b.n + j]
                            + 3.0 * self.s.units[j].fort as f32
                            + self.b.defense[self.s.units[j].tile as usize],
                        self.s.units[j].hp as i32,
                    );
                    let dmg = self.damage(strike, def, j);
                    self.s.units[j].hp -= dmg;
                    if self.s.units[j].hp <= 0 {
                        self.kill(j);
                    }
                }
            }
        }
    }

    /// `district_under_siege`: every land neighbour held or covered.
    fn besieged(&self, c: usize) -> bool {
        let tile = self.b.cities[c].tile;
        let side = self.s.city_side[c];
        for n in self.b.nbrs[tile as usize] {
            if n == NO_TILE || !self.b.land[n as usize] {
                continue;
            }
            let at = self.s.occ[n as usize];
            let enemy_on = at != NO_UNIT && self.s.units[at as usize].side != side;
            if !enemy_on && !self.in_zoc_of_units(side, n) {
                return false;
            }
        }
        true
    }

    fn in_zoc_of_units(&self, side: u8, tile: u16) -> bool {
        self.b.nbrs[tile as usize].iter().any(|n| {
            *n != NO_TILE && {
                let at = self.s.occ[*n as usize];
                at != NO_UNIT && {
                    let other = &self.s.units[at as usize];
                    other.side != side && other.has(F_ZOC)
                }
            }
        })
    }

    /// One side's whole scripted turn: shooters first, then the line.
    pub fn side_turn(&mut self, side: u8, posture: Posture) {
        self.order.clear();
        for (i, unit) in self.s.units.iter().enumerate() {
            if unit.alive() && unit.side == side {
                self.order.push(i as u16);
            }
        }
        let units = &self.s.units;
        self.order
            .sort_by_key(|i| (!units[*i as usize].shoots(), *i));
        for k in 0..self.order.len() {
            let i = self.order[k] as usize;
            self.act(i, posture);
        }
    }

    /// Our plan for this turn, in the order that gives each blow its best
    /// chance: shots first, then the melee, then everyone who only moves.
    pub fn play_plan(&mut self, plan: &[(usize, Line)]) {
        for pass in 0..3 {
            for (i, line) in plan {
                let unit = self.s.units[*i];
                let wanted = match pass {
                    0 => unit.shoots() && line.aim != Aim::None,
                    1 => !unit.shoots() && line.aim != Aim::None,
                    _ => line.aim == Aim::None,
                };
                if !wanted || !unit.alive() {
                    continue;
                }
                let played = self.play_line(*i, *line);
                if !played && pass < 2 {
                    // Its target fell to an earlier blow: a real turn
                    // re-plans here, so the rollout does too.
                    self.act(*i, Posture::Press);
                }
            }
        }
    }

    /// Play `plan` this turn, then `horizon` rounds of reply and follow-up.
    pub fn rollout(&mut self, plan: &[(usize, Line)], posture: Posture, horizon: u32) {
        self.round = 0;
        self.play_plan(plan);
        for round in 0..horizon {
            self.round = round + 1;
            self.begin_side(THEIRS);
            self.side_turn(THEIRS, Posture::Press);
            if round + 1 == horizon {
                break;
            }
            self.begin_side(OURS);
            self.side_turn(OURS, posture);
        }
    }

    /// Hammers on the board, from our side, at the end of a rollout.
    pub fn score(&self) -> f32 {
        let mut v = 0.0f32;
        for unit in &self.s.units {
            if !unit.alive() {
                continue;
            }
            let worth = unit.value
                * (self.w.alive_floor + (1.0 - self.w.alive_floor) * unit.hp as f32 / 100.0);
            if unit.side == OURS {
                v += worth * self.w.loss;
            } else {
                v -= worth;
            }
        }
        for (c, city) in self.b.cities.iter().enumerate() {
            let ours = self.s.city_side[c] == OURS;
            if ours {
                v += city.value;
            } else {
                // A city we have worn down is worth a share of having it.
                let worn = 1.0 - self.s.city_hp[c] as f32 / 200.0;
                v -= city.value * (1.0 - 0.25 * worn);
            }
        }
        match self.objective {
            Objective::Capture(c) if self.s.city_side[c as usize] == OURS => {
                let at = self.reached.unwrap_or(0) as i32;
                v += self.w.objective_bonus * self.w.haste.powi(at);
            }
            Objective::Hold(c) if self.s.city_side[c as usize] == OURS => {
                v += self.w.objective_bonus;
            }
            _ => {}
        }
        v
    }

    pub fn own_alive(&self) -> usize {
        self.s
            .units
            .iter()
            .filter(|u| u.alive() && u.side == OURS)
            .count()
    }

    pub fn enemy_alive(&self) -> usize {
        self.s
            .units
            .iter()
            .filter(|u| u.alive() && u.side == THEIRS)
            .count()
    }

    /// Whether the objective holds at the end of the rollout.
    pub fn objective_met(&self) -> bool {
        match self.objective {
            Objective::Field => self.enemy_alive() == 0,
            Objective::Capture(c) => self.s.city_side[c as usize] == OURS,
            Objective::Hold(c) => self.s.city_side[c as usize] == OURS,
        }
    }
}
