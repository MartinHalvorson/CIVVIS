//! `risk-is-priced`: one survival reading for every pathed move, a price for
//! the unit in hammers, and an allowance of risk bought by what the unit is
//! for.
//!
//! **Why.** 62 live Deity games of 2026-11-12 lost 1,295 of our units; 63% of
//! those killed defending were at 50+ hp when the fight began, and Warriors,
//! Archers, Scouts and Heavy Chariots were 48% of the losses. An earlier
//! census of 765 losses found 580 dead to one hit, typically a healthy,
//! out-teched unit that had moved next to a visible stronger enemy (the killer
//! was visible at our turn end 77% of the time). Survival was seven separate
//! rules that disagree and are not all honoured by every mover, and the two
//! unit-level vetoes failed both ways when measured: `guns-stay-out-of-reach-2`
//! (no step where one upper-roll blow kills, every land unit) read negative on
//! every line of 12 paired games because home defenders must close on their
//! attackers, and `siege-post-yields-to-one-blow` cut captures because a siege
//! needs shooters on the ring. A veto per unit cannot tell those cases from a
//! Warrior marching past a Line Infantry; the mission can.
//!
//! **The reading** ([`death_chance`]). The chance a unit standing on a tile is
//! dead before our next turn: every visible at-war hostile whose strike reach
//! covers the tile (the battle planner's `strike-reach` flood, one move plus a
//! blow, zone of control included) and every walled enemy City Center or
//! Encampment within two tiles strikes it with probability [`HOST_STRIKE_P`],
//! each independently, with the engine's exact roll distribution and the
//! wound penalty compounding inside the volley (`battle_solver::dice`). A
//! known hazard (fire, fallout) lands first at its planning damage. The pairs
//! are the field's own (`DangerField::blow_pairs_at_hp`). Read once a frame
//! per (unit, tile) over every tile the unit can reach this turn, each blow
//! kept with its source, so a hostile killed or a city taken later in the
//! frame stops counting and a unit wounded since is re-read at its health.
//!
//! **The price** ([`unit_price`]): production cost times the hp share, plus
//! [`PROMOTION_PREMIUM`] of the cost per promotion, times
//! [`OBSOLETE_PRICE_SHARE`] when the unit's strength is
//! [`OBSOLETE_STRENGTH_GAP`] or more under the strongest visible hostile's.
//!
//! **The allowance** ([`Mission`]): what the unit is for this frame, read from
//! its Objective Board row when the board holds it and from the shipped force
//! group otherwise. A Defend or Relieve member, an active siege post, or any
//! unit standing by a city of ours with a visible hostile at it, may buy up to
//! [`HOLD_ALLOWANCE`]; a Destroy, Escort or engaged unit [`FIGHT_ALLOWANCE`];
//! marching, staging, mustering, Reserve, Deter, Recon, ClearCamp, healing and
//! exploring units [`MARCH_ALLOWANCE`]; a scout [`SCOUT_ALLOWANCE`].
//!
//! **The rule.** A move whose death chance exceeds the allowance is refused
//! unless it lowers the unit's current death chance. Among allowed moves the
//! one-ply movers subtract `death chance x price` from each tile's score, in
//! the mover's own units: the mover charges `mv_threat x caution` for each hit
//! point of expected damage, so a death is charged as the price's worth of
//! the unit's own hit points (`price / (cost / 100)`). A siege gun standing
//! over its allowance steps to the nearest tile within it.
//!
//! **Where.** `BasicAi::path_step_allowed` (every pathed step and walk),
//! `tactical_step` and `coordinated_tactical_step` (filter and score), the
//! campaign staging and wartime reinforcement walks (their raw final step).
//! Units the battle planner or the battle solver ordered this turn are left
//! alone, and with the gene on `guns-stay-out-of-reach` v1/v2/v3 no longer
//! mark the units this reads (no double veto). Every refusal writes one
//! journal line naming the unit, the tile, the death chance and the
//! allowance. Off by default; nothing is read or written with the gene off.

use std::collections::HashMap;
use std::sync::Arc;

use super::battle_planner::{wounded_penalty, BlowPair, DangerField, STRUCTURE_SOURCE};
use super::battle_solver::dice::VolleyDist;
use super::objective_board::ObjectiveKey;
use super::{AdvancedAi, ForcePosture, ForceRole, StrategicPlan};
use crate::ai::{BasicAi, UnitDoctrine};
use crate::game::Game;
use crate::think;
use crate::Pos;

/// The chance a hostile in reach strikes the unit asked about. The battle
/// solver's `enemy_aggression` (over 115 live Deity games the host struck well
/// under half the units it could reach); a fitted value replaces both.
pub(crate) const HOST_STRIKE_P: f64 = 0.5;
/// A Defend or Relieve member, an active siege post, or a unit standing by a
/// contested city of ours.
pub(crate) const HOLD_ALLOWANCE: f64 = 0.6;
/// A Destroy or Escort member, or a unit whose group is engaged.
pub(crate) const FIGHT_ALLOWANCE: f64 = 0.3;
/// Marching, staging, mustering, Reserve, Deter, Recon, ClearCamp, healing
/// and exploring units.
pub(crate) const MARCH_ALLOWANCE: f64 = 0.1;
/// A scout (the recon line without a ranged attack).
pub(crate) const SCOUT_ALLOWANCE: f64 = 0.05;
/// A promotion's worth, as a share of the unit's production cost.
pub(crate) const PROMOTION_PREMIUM: f64 = 0.25;
/// Strength under the strongest visible hostile's at which a unit is
/// obsolete.
pub(crate) const OBSOLETE_STRENGTH_GAP: f64 = 15.0;
/// What an obsolete unit is worth, as a share of its price.
pub(crate) const OBSOLETE_PRICE_SHARE: f64 = 0.4;
/// Under this health a fighting unit is healing: [`MARCH_ALLOWANCE`].
pub(crate) const HEALING_HP: i32 = 50;
/// A city of ours is contested when a visible at-war hostile stands within
/// this of it, and a unit within this of a contested city is defending it.
pub(crate) const HOME_THREAT_RADIUS: i32 = 3;
/// The city id under the structure tags (`STRUCTURE_SOURCE` and the
/// Encampment bit below it); city ids stay below both.
const CITY_ID_MASK: u32 = (1 << 30) - 1;

/// What a unit is for this frame, which sets how much risk it may buy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mission {
    Defend,
    Relieve,
    SiegePost,
    Destroy,
    Escort,
    Engaged,
    Staging,
    March,
    Reserve,
    Deter,
    Recon,
    ClearCamp,
    Healing,
    Scout,
}

impl Mission {
    pub(crate) fn allowance(self) -> f64 {
        match self {
            Mission::Defend | Mission::Relieve | Mission::SiegePost => HOLD_ALLOWANCE,
            Mission::Destroy | Mission::Escort | Mission::Engaged => FIGHT_ALLOWANCE,
            Mission::Staging
            | Mission::March
            | Mission::Reserve
            | Mission::Deter
            | Mission::Recon
            | Mission::ClearCamp
            | Mission::Healing => MARCH_ALLOWANCE,
            Mission::Scout => SCOUT_ALLOWANCE,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Mission::Defend => "defend",
            Mission::Relieve => "relieve",
            Mission::SiegePost => "siege post",
            Mission::Destroy => "destroy",
            Mission::Escort => "escort",
            Mission::Engaged => "engaged",
            Mission::Staging => "staging",
            Mission::March => "march",
            Mission::Reserve => "reserve",
            Mission::Deter => "deter",
            Mission::Recon => "recon",
            Mission::ClearCamp => "clear camp",
            Mission::Healing => "healing",
            Mission::Scout => "scout",
        }
    }
}

/// Who a blow comes from, so a source gone from the board stops counting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Unit(u32),
    City(u32),
}

impl Source {
    fn stands(self, g: &Game, owner: usize) -> bool {
        match self {
            Source::Unit(id) => g.units.get(&id).is_some_and(|unit| unit.owner != owner),
            Source::City(cid) => g
                .cities
                .get(&cid)
                .is_some_and(|city| city.owner != owner && g.is_at_war(owner, city.owner)),
        }
    }
}

/// One blow on one tile: its source, the striker's strength and our unit's
/// defence before its own wound penalty (re-read at its health by the dice).
#[derive(Clone, Copy, Debug)]
struct Blow {
    source: Source,
    att: f32,
    def_base: f32,
}

/// One tile's reading for one unit.
#[derive(Clone, Debug)]
struct TileRisk {
    hazard: f64,
    blows: Vec<Blow>,
    /// The death chance at the health the frame was read at, every source
    /// standing.
    death: f64,
}

/// A refused move, for the journal.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Refusal {
    pub(crate) death: f64,
    pub(crate) here: f64,
    pub(crate) allowance: f64,
    pub(crate) mission: Mission,
    pub(crate) price: f64,
}

/// One unit's frame: what it is for, what it is worth, and its death chance
/// on every tile it can reach this turn where anything can land.
#[derive(Clone, Debug)]
pub(crate) struct UnitRisk {
    pub(crate) mission: Mission,
    pub(crate) allowance: f64,
    /// Hammers.
    pub(crate) price: f64,
    /// The price in the unit's own hit points: `price / (cost / 100)`.
    pub(crate) hp_equivalent: f64,
    hp: i32,
    tiles: HashMap<Pos, TileRisk>,
}

/// Every covered unit's frame, keyed by unit. Shared, so a clone of the
/// controller for a parallel unit plan is cheap.
pub(crate) type PricedRiskMap = Arc<HashMap<u32, UnitRisk>>;

/// The chance a unit at `hp` is dead before our next turn: `hazard` lands
/// first, then each `(att, def_base)` blow strikes with probability
/// `strike_p`, independently, through the exact roll distribution with the
/// wound penalty compounding (`dice::VolleyDist`). At `strike_p` one this is
/// `dice::kill_probability` of the hazard-wounded unit.
pub(crate) fn death_chance(hp: i32, hazard: f64, blows: &[(f32, f32)], strike_p: f64) -> f64 {
    let start = hp - hazard.round() as i32;
    if start <= 0 {
        return 1.0;
    }
    let mut dist = VolleyDist::new(start);
    for &(att, def_base) in blows {
        let mut struck = dist.clone();
        struck.strike(att, def_base);
        for (stood, hit) in dist.hp.iter_mut().zip(struck.hp.iter()) {
            *stood = (1.0 - strike_p) * *stood + strike_p * *hit;
        }
    }
    dist.p_killed()
}

impl TileRisk {
    fn read(pairs: &[(Option<u32>, BlowPair)], hp: i32) -> Self {
        let mut hazard = 0.0;
        let mut blows = Vec::new();
        for (source, pair) in pairs {
            match *pair {
                BlowPair::Hazard(damage) => hazard += damage,
                BlowPair::Strike { att, def } => {
                    let Some(id) = *source else {
                        continue;
                    };
                    let source = if id & STRUCTURE_SOURCE != 0 {
                        Source::City(id & CITY_ID_MASK)
                    } else {
                        Source::Unit(id)
                    };
                    blows.push(Blow {
                        source,
                        att: att as f32,
                        // The field reads our defence through the wound at
                        // `hp`; the dice want it before, to re-read it after
                        // every hit.
                        def_base: (def + wounded_penalty(hp)) as f32,
                    });
                }
            }
        }
        let pairs: Vec<(f32, f32)> = blows.iter().map(|b| (b.att, b.def_base)).collect();
        let death = death_chance(hp, hazard, &pairs, HOST_STRIKE_P);
        TileRisk {
            hazard,
            blows,
            death,
        }
    }
}

impl UnitRisk {
    /// The unit's death chance on `tile` now: the frame's reading while its
    /// health and every source are as read, otherwise re-read from the
    /// sources still standing at its health.
    pub(crate) fn death_at(&self, g: &Game, uid: u32, tile: Pos) -> f64 {
        let Some(risk) = self.tiles.get(&tile) else {
            return 0.0;
        };
        let Some(unit) = g.units.get(&uid) else {
            return 0.0;
        };
        let all_stand = risk
            .blows
            .iter()
            .all(|blow| blow.source.stands(g, unit.owner));
        if unit.hp == self.hp && all_stand {
            return risk.death;
        }
        let pairs: Vec<(f32, f32)> = risk
            .blows
            .iter()
            .filter(|blow| blow.source.stands(g, unit.owner))
            .map(|blow| (blow.att, blow.def_base))
            .collect();
        death_chance(unit.hp, risk.hazard, &pairs, HOST_STRIKE_P)
    }

    /// Why the move `from -> to` is refused, if it is: its death chance is
    /// over the allowance and no lower than where the unit stands.
    fn refusal(&self, g: &Game, uid: u32, from: Pos, to: Pos) -> Option<Refusal> {
        if from == to {
            return None;
        }
        let death = self.death_at(g, uid, to);
        if death <= self.allowance {
            return None;
        }
        let here = self.death_at(g, uid, from);
        (death >= here).then_some(Refusal {
            death,
            here,
            allowance: self.allowance,
            mission: self.mission,
            price: self.price,
        })
    }
}

/// A unit's strength as a fighter: the better of its melee and ranged.
fn fighting_strength(g: &Game, uid: u32) -> f64 {
    let unit = &g.units[&uid];
    let spec = &g.rules.units[unit.kind];
    let melee = g.unit_strength(unit, false);
    if spec.has_ranged_attack() {
        melee.max(g.unit_ranged_attack_strength(unit))
    } else {
        melee
    }
}

/// What losing `uid` costs, in hammers: production cost at its hit points, a
/// premium per promotion, and a fraction of that once it is obsolete against
/// `strongest_hostile` (the strongest visible hostile's fighting strength).
pub(crate) fn unit_price(g: &Game, uid: u32, strongest_hostile: f64) -> f64 {
    let unit = &g.units[&uid];
    let cost = g.rules.units[unit.kind].cost;
    let mut price = cost * f64::from(unit.hp.clamp(0, 100)) / 100.0
        + PROMOTION_PREMIUM * cost * unit.promotions.len() as f64;
    if strongest_hostile - fighting_strength(g, uid) >= OBSOLETE_STRENGTH_GAP {
        price *= OBSOLETE_PRICE_SHARE;
    }
    price
}

/// The land military units the reading covers: on land, not embarked, with
/// movement left.
fn covered_unit(g: &Game, uid: u32) -> bool {
    let Some(unit) = g.units.get(&uid) else {
        return false;
    };
    let spec = &g.rules.units[unit.kind];
    spec.class == "military"
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
        && unit.moves_left > 0.0
        && !g.is_embarked(unit)
}

/// Visible at-war hostile soldiers, as the danger field counts them.
fn visible_hostiles(g: &Game, pid: usize) -> impl Iterator<Item = u32> + '_ {
    g.units.values().filter_map(move |unit| {
        let spec = &g.rules.units[unit.kind];
        (unit.owner != pid
            && g.is_at_war(pid, unit.owner)
            && spec.class == "military"
            && (spec.is_melee_capable() || spec.has_ranged_attack())
            && g.unit_visible_to(unit.id, pid))
        .then_some(unit.id)
    })
}

impl AdvancedAi {
    /// Whether the reading covers `uid` this frame: the gene on, a land
    /// military unit with movement left, and not one the battle planner or
    /// the battle solver ordered.
    pub(super) fn priced_risk_covers(&self, g: &Game, uid: u32) -> bool {
        self.risk_is_priced && covered_unit(g, uid) && !self.battle_planner_ordered.contains(&uid)
    }

    /// The last frame's reading goes before anything moves this frame.
    pub(super) fn clear_priced_risk(&mut self) {
        if !self.base.priced_risk.is_empty() {
            self.base.priced_risk = PricedRiskMap::default();
        }
    }

    /// What `uid` is for this frame. `contested` holds our cities with a
    /// visible at-war hostile within [`HOME_THREAT_RADIUS`].
    pub(super) fn priced_risk_mission(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
        contested: &[Pos],
    ) -> Mission {
        let unit = &g.units[&uid];
        let spec = &g.rules.units[unit.kind];
        if BasicAi::unit_doctrine(g, uid) == UnitDoctrine::Recon && !spec.has_ranged_attack() {
            return Mission::Scout;
        }
        if self.active_siege_member(g, pid, uid) {
            return Mission::SiegePost;
        }
        let group = self
            .force_groups
            .iter()
            .find(|group| group.units.contains(&uid));
        let engaged = group.is_some_and(|group| group.posture == ForcePosture::Engage);
        let row = self
            .objective_board_state
            .forces
            .iter()
            .find(|force| force.units.contains(&uid))
            .map(|force| force.objective_key);
        let mission = match row {
            Some(ObjectiveKey::Defend(_)) => Mission::Defend,
            Some(ObjectiveKey::Relieve(_)) => Mission::Relieve,
            Some(ObjectiveKey::Siege(_)) if self.staging_siege_member(g, pid, uid) => {
                Mission::Staging
            }
            Some(ObjectiveKey::Siege(_)) if engaged => Mission::Engaged,
            Some(ObjectiveKey::Siege(_)) => Mission::March,
            Some(ObjectiveKey::Destroy(_) | ObjectiveKey::Flag(_)) => Mission::Destroy,
            Some(ObjectiveKey::Escort(_)) => Mission::Escort,
            Some(ObjectiveKey::Camp(_)) => Mission::ClearCamp,
            Some(ObjectiveKey::Deter(_)) => Mission::Deter,
            Some(ObjectiveKey::Recon(_) | ObjectiveKey::FindCapital(_)) => Mission::Recon,
            Some(ObjectiveKey::Reserve) => Mission::Reserve,
            None if self.staging_siege_member(g, pid, uid) => Mission::Staging,
            None => match group {
                Some(group)
                    if g.city_at(group.objective)
                        .is_some_and(|cid| g.cities[&cid].owner == pid)
                        || plan
                            .threatened_city
                            .and_then(|cid| g.cities.get(&cid))
                            .is_some_and(|city| city.pos == group.objective) =>
                {
                    Mission::Relieve
                }
                Some(group) => match group.posture {
                    ForcePosture::Engage => Mission::Engaged,
                    ForcePosture::Recover => Mission::Healing,
                    _ => Mission::March,
                },
                // Exploring, marching alone: the march allowance.
                None => Mission::March,
            },
        };
        // A unit standing by a city of ours with the attacker at it is
        // defending that city, whatever row it serves.
        let mission = if mission.allowance() < HOLD_ALLOWANCE
            && contested
                .iter()
                .any(|city| g.wdist(*city, unit.pos) <= HOME_THREAT_RADIUS)
        {
            Mission::Defend
        } else {
            mission
        };
        if unit.hp < HEALING_HP
            && matches!(
                mission,
                Mission::Destroy | Mission::Escort | Mission::Engaged
            )
        {
            Mission::Healing
        } else {
            mission
        }
    }

    /// Read this frame: for every covered unit, its mission, allowance and
    /// price, and its death chance on each tile it can reach this turn. Left
    /// in `BasicAi::priced_risk` for `path_step_allowed` and the movers.
    /// Nothing with the gene off.
    pub(super) fn mark_priced_risk(&mut self, g: &Game, pid: usize, plan: &StrategicPlan) {
        if !self.risk_is_priced {
            return;
        }
        self.clear_priced_risk();
        let at_war = g
            .players
            .iter()
            .any(|player| player.id != pid && player.alive && g.is_at_war(pid, player.id));
        if !at_war {
            return;
        }
        let units: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| self.priced_risk_covers(g, *uid))
            .collect();
        if units.is_empty() {
            return;
        }
        let hostiles: Vec<u32> = visible_hostiles(g, pid).collect();
        let strongest = hostiles
            .iter()
            .map(|id| fighting_strength(g, *id))
            .fold(0.0_f64, f64::max);
        let contested: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| g.cities.get(&cid))
            .map(|city| city.pos)
            .filter(|at| {
                hostiles
                    .iter()
                    .any(|id| g.wdist(g.units[id].pos, *at) <= HOME_THREAT_RADIUS)
            })
            .collect();
        let mut field = DangerField::with_reach(g, pid, true);
        let mut map = HashMap::new();
        for uid in units {
            let unit = &g.units[&uid];
            let hp = unit.hp;
            let mission = self.priced_risk_mission(g, pid, uid, plan, &contested);
            let cost = g.rules.units[unit.kind].cost.max(1.0);
            let price = unit_price(g, uid, strongest);
            let mut tiles = HashMap::new();
            let mut stands = vec![unit.pos];
            stands.extend(g.reachable(uid));
            for tile in stands {
                if tiles.contains_key(&tile) {
                    continue;
                }
                let pairs = field.blow_pairs_at_hp(tile, uid, hp);
                if pairs.is_empty() {
                    continue;
                }
                tiles.insert(tile, TileRisk::read(&pairs, hp));
            }
            map.insert(
                uid,
                UnitRisk {
                    mission,
                    allowance: mission.allowance(),
                    price,
                    hp_equivalent: price * 100.0 / cost,
                    hp,
                    tiles,
                },
            );
        }
        self.base.priced_risk = Arc::new(map);
    }

    /// A covered siege gun whose own tile reads over its allowance steps to
    /// the nearest tile it can reach within it, or else to the tile that
    /// lowers its death chance most. Guns have no mover that prices the tile
    /// they stand on (version one of `guns-stay-out-of-reach` stepped them
    /// out for the same reason). Nothing with the gene off.
    pub(super) fn priced_risk_guns_step_out(&mut self, g: &mut Game, pid: usize) {
        if !self.risk_is_priced || self.base.priced_risk.is_empty() {
            return;
        }
        let risks = Arc::clone(&self.base.priced_risk);
        let mut guns: Vec<u32> = risks
            .keys()
            .copied()
            .filter(|uid| {
                g.units.contains_key(uid) && matches!(Self::force_role(g, *uid), ForceRole::Siege)
            })
            .collect();
        guns.sort_unstable();
        for uid in guns {
            let risk = &risks[&uid];
            let pos = g.units[&uid].pos;
            let here = risk.death_at(g, uid, pos);
            if here <= risk.allowance {
                continue;
            }
            let read: Vec<(Pos, f64)> = g
                .reachable(uid)
                .into_iter()
                .filter(|tile| *tile != pos)
                .map(|tile| (tile, risk.death_at(g, uid, tile)))
                .collect();
            let to = read
                .iter()
                .filter(|(_, death)| *death <= risk.allowance)
                .min_by_key(|(tile, _)| (g.wdist(pos, *tile), *tile))
                .or_else(|| {
                    read.iter()
                        .filter(|(_, death)| *death < here)
                        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
                })
                .copied();
            let Some((to, death)) = to else {
                continue;
            };
            if self.base.path_walk_to(g, pid, uid, to) {
                think!(self.journal(), Military, Decision,
                    "The {} steps out of a {:.0}% death chance", g.units[&uid].kind, here * 100.0;
                    "risk-is-priced: {:?} reads {:.2} against the {} allowance {:.2}; {:?} reads {:.2}",
                    pos, here, risk.mission.as_str(), risk.allowance, to, death;
                    to);
            }
        }
    }
}

impl BasicAi {
    /// `risk-is-priced`: why the move `from -> to` is refused for `uid`, if
    /// it is. `None` for a unit the frame did not read, and always with the
    /// gene off.
    pub(in crate::ai) fn priced_risk_refusal(
        &self,
        g: &Game,
        uid: u32,
        from: Pos,
        to: Pos,
    ) -> Option<Refusal> {
        self.priced_risk.get(&uid)?.refusal(g, uid, from, to)
    }

    /// The refusal's journal line: the unit, the tile, the death chance and
    /// the allowance.
    pub(in crate::ai) fn note_priced_risk_refusal(
        &self,
        g: &Game,
        uid: u32,
        to: Pos,
        refusal: &Refusal,
    ) {
        let kind = g.units.get(&uid).map(|unit| unit.kind.to_string());
        think!(self.journal, Military, Decision,
            "The {} {} does not step to {:?}: {:.0}% to die there",
            kind.as_deref().unwrap_or("unit"), uid, to, refusal.death * 100.0;
            "risk-is-priced: death chance {:.2} over the {} allowance {:.2} (here {:.2}); priced at {:.0} hammers",
            refusal.death, refusal.mission.as_str(), refusal.allowance, refusal.here, refusal.price;
            to);
    }

    /// Whether the move is refused, writing the journal line when it is.
    pub(in crate::ai) fn priced_risk_refuses(
        &self,
        g: &Game,
        uid: u32,
        from: Pos,
        to: Pos,
    ) -> bool {
        let Some(refusal) = self.priced_risk_refusal(g, uid, from, to) else {
            return false;
        };
        self.note_priced_risk_refusal(g, uid, to, &refusal);
        true
    }

    /// `death chance x price` on `tile`, in a mover's units: `per_hp` is
    /// what the mover charges for one hit point of expected damage. Zero for
    /// a unit the frame did not read.
    pub(in crate::ai) fn priced_risk_cost(
        &self,
        g: &Game,
        uid: u32,
        tile: Pos,
        per_hp: f64,
    ) -> f64 {
        self.priced_risk.get(&uid).map_or(0.0, |risk| {
            risk.death_at(g, uid, tile) * risk.hp_equivalent * per_hp
        })
    }
}

#[cfg(test)]
mod driver_tests;
#[cfg(test)]
mod tests;
