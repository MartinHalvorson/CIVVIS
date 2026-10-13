//! `amphibious-siege-train`: a Domination siege train whose objective has no
//! dry road from our land crosses the water as one group: guns and an escort
//! gather at a coastal city of ours, embark together, sail a lane no visible
//! enemy warship watches, and come ashore together beside the target. A gun
//! never lands alone.
//!
//! **Why (2026-10-11/12 census, five live gate passers with logs).** When a
//! capital was the siege objective after turn 150, the top hold in two of the
//! five was water: civvis-20261010T190448Z (the Emperor win) logged 133 and
//! civvis-20261011T004440Z 342 "the bombard holds on land short of the water
//! | the march crosses water in N steps and come-ashore lands an embarked unit
//! at home; no dry road opens within 128 steps". `stage-march-keeps-to-land`
//! holds such a gun on land because a single gun that embarks is landed at
//! home again the next turn by `come-ashore` (an embarked unit has no siege
//! arm, so the siege doctrine passes it to the force mover, whose first act
//! is to bring it ashore). Across all passers, 51% of at-war capital samples
//! after t150 had no land unit of ours within six tiles, and the bombers
//! strike only a city the land army besieges.
//!
//! **Design.** The pieces reused:
//! - the trigger is the existing water hold: `stage_march` returning
//!   `StageMarch::Hold` (no dry road within `STAGE_DRY_LIMIT`) in the
//!   Stage march (`siege_stage_step`) or the Invest march
//!   (`close_to_staging`), and only for a pure water hold, never one a
//!   closed border causes (`siege-buys-the-passage` and
//!   `blocker-becomes-the-target` own those);
//! - the members are the land force group whose objective is the target:
//!   its siege guns and its melee or anti-cavalry bodies;
//! - the host's embarkation is the order layer's ordinary step: a land unit
//!   with the embark technology steps onto a Coast tile
//!   (`Game::has_embarkation`: Shipbuilding for every land unit in Gathering
//!   Storm, Ocean tiles behind Cartography's `ocean_navigation`), so the lane
//!   is read with [`Game::unit_can_traverse`] for a gun of the train and the
//!   sail is `Game::route_step_to_any` toward the water beside the landing;
//! - landing is a step from the water onto a free land tile beside the
//!   landing tile (`tactical_apply_move`), escorts first.
//!
//! The train owns its members from the first act of the unit step
//! (`advanced_military_step_with_decline`), ahead of the doctrine and the
//! force mover, so `come-ashore` never lands them at home mid-crossing; once
//! every member stands on the far shore the train releases them and the
//! ordinary siege march, which now finds a dry road, takes over. One train at
//! a time. The train abandons the crossing (and `come-ashore` brings any
//! embarked member home) when a visible enemy warship comes within
//! [`AMPHIB_LANE_SAFETY`] of the lane, the target changes hands or the war
//! ends, too few guns or escorts survive, or [`AMPHIB_PATIENCE`] turns pass.
//!
//! The landing tile lies within [`AMPHIB_LANDING_MAX`] of the target, the
//! nearest outside the city's own strike range preferred (three tiles, then
//! two, then one): a train that lands at two or one comes ashore under the
//! city's ranged attack. Off, nothing runs and no order changes.

use super::*;

/// The longest sea lane, in water tiles from the port's shore to the water
/// beside the landing, the train will sail.
pub(super) const AMPHIB_SEA_LANE_MAX: usize = 12;

/// No visible warship of a power at war with us within this many tiles of
/// any tile of the lane.
pub(super) const AMPHIB_LANE_SAFETY: i32 = 3;

/// The landing tile lies within this many tiles of the target.
pub(super) const AMPHIB_LANDING_MAX: i32 = 3;

/// Members within this many tiles of the port are gathered.
pub(super) const AMPHIB_ASSEMBLY_RADIUS: i32 = 2;

/// Members are drawn from the objective's force group within this many tiles
/// of the port.
pub(super) const AMPHIB_MUSTER_RADIUS: i32 = 10;

/// Escort bodies (melee or anti-cavalry) the crossing needs.
pub(super) const AMPHIB_MIN_ESCORTS: usize = 2;

/// The most guns and the most escorts one train carries.
pub(super) const AMPHIB_MAX_GUNS: usize = 3;
pub(super) const AMPHIB_MAX_ESCORTS: usize = 3;

/// Live turns from forming after which an unfinished crossing is abandoned.
pub(super) const AMPHIB_PATIENCE: u32 = 24;

/// A member at sea this many tiles nearer the landing than the furthest
/// member waits for it.
pub(super) const AMPHIB_COHESION: i32 = 2;

/// Where the crossing stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AmphibiousPhase {
    /// Members walk to the port.
    Assemble,
    /// Members embark, sail and land.
    Sail,
}

/// One crossing: see the module documentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AmphibiousTrain {
    pub(super) target: u32,
    pub(super) target_pos: Pos,
    pub(super) port: u32,
    pub(super) port_pos: Pos,
    pub(super) landing: Pos,
    /// The planned lane's water tiles, port shore to landing water.
    pub(super) lane: Vec<Pos>,
    pub(super) members: BTreeSet<u32>,
    pub(super) phase: AmphibiousPhase,
    pub(super) formed: u32,
}

/// A siege gun of the train, read from its type (an embarked unit has no
/// siege arm).
fn is_gun(g: &Game, uid: u32) -> bool {
    g.units.get(&uid).is_some_and(|unit| {
        let spec = &g.rules.units[unit.kind];
        spec.class == "military" && spec.siege && spec.has_ranged_attack()
    })
}

/// An escort body of the train: a land melee-capable soldier that is not a
/// gun.
fn is_escort(g: &Game, uid: u32) -> bool {
    g.units.get(&uid).is_some_and(|unit| {
        let spec = &g.rules.units[unit.kind];
        spec.class == "military"
            && !spec.siege
            && spec.is_melee_capable()
            && spec.domain.as_deref().is_none_or(|domain| domain == "land")
    })
}

/// A land tile a unit may stand on: passable, not water, no city.
fn landable(g: &Game, pos: Pos) -> bool {
    g.map
        .get(pos)
        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        && g.city_at(pos).is_none()
}

/// A water tile `rep` can sail.
fn sailable(g: &Game, rep: u32, pos: Pos) -> bool {
    g.map.get(pos).is_some_and(|tile| g.rules.is_water(tile)) && g.unit_can_traverse(rep, pos)
}

/// The water tiles beside `pos` that `rep` can sail.
fn shore_water(g: &Game, rep: u32, pos: Pos) -> Vec<Pos> {
    g.nbrs(pos)
        .into_iter()
        .filter(|n| sailable(g, rep, *n))
        .collect()
}

/// The shortest sea lane, over water `rep` can sail and no foreign unit
/// holds, from the water beside `from` to the water beside `to`: its tiles,
/// no longer than [`AMPHIB_SEA_LANE_MAX`].
pub(super) fn sea_lane(g: &Game, pid: usize, rep: u32, from: Pos, to: Pos) -> Option<Vec<Pos>> {
    let goals: BTreeSet<Pos> = shore_water(g, rep, to).into_iter().collect();
    if goals.is_empty() {
        return None;
    }
    let open = |pos: Pos| {
        sailable(g, rep, pos)
            && g.unit_ids_at(pos)
                .iter()
                .all(|id| g.units.get(id).is_none_or(|unit| unit.owner == pid))
    };
    let mut parent: BTreeMap<Pos, Option<Pos>> = BTreeMap::new();
    let mut frontier: std::collections::VecDeque<(Pos, usize)> = std::collections::VecDeque::new();
    for start in shore_water(g, rep, from) {
        if open(start) && !parent.contains_key(&start) {
            parent.insert(start, None);
            frontier.push_back((start, 1));
        }
    }
    while let Some((pos, len)) = frontier.pop_front() {
        if goals.contains(&pos) {
            let mut lane = vec![pos];
            let mut cursor = pos;
            while let Some(Some(prev)) = parent.get(&cursor) {
                lane.push(*prev);
                cursor = *prev;
            }
            lane.reverse();
            return Some(lane);
        }
        if len >= AMPHIB_SEA_LANE_MAX {
            continue;
        }
        for next in g.nbrs(pos) {
            if !parent.contains_key(&next) && open(next) {
                parent.insert(next, Some(pos));
                frontier.push_back((next, len + 1));
            }
        }
    }
    None
}

/// Whether a visible warship of a power at war with `pid` stands within
/// [`AMPHIB_LANE_SAFETY`] of any tile of `lane`.
pub(super) fn lane_watched(g: &Game, pid: usize, lane: &[Pos]) -> bool {
    g.units.values().any(|unit| {
        unit.owner != pid
            && g.is_at_war(pid, unit.owner)
            && g.rules.units[unit.kind].domain.as_deref() == Some("sea")
            && g.unit_visible_to(unit.id, pid)
            && lane
                .iter()
                .any(|tile| g.wdist(*tile, unit.pos) <= AMPHIB_LANE_SAFETY)
    })
}

/// The landing tiles for `target`, best first: land beside water `rep` can
/// sail, within [`AMPHIB_LANDING_MAX`] of the target, no foreign unit on it;
/// three tiles out (outside the city's strike) before two before one.
pub(super) fn landing_tiles(g: &Game, pid: usize, rep: u32, target: Pos) -> Vec<Pos> {
    let preference = |distance: i32| match distance {
        3 => 0,
        2 => 1,
        _ => 2,
    };
    let mut tiles: Vec<(i32, Pos)> = g
        .wdisk(target, AMPHIB_LANDING_MAX)
        .into_iter()
        .filter(|pos| {
            *pos != target
                && landable(g, *pos)
                && !shore_water(g, rep, *pos).is_empty()
                && g.unit_ids_at(*pos)
                    .iter()
                    .all(|id| g.units.get(id).is_none_or(|unit| unit.owner == pid))
        })
        .map(|pos| (preference(g.wdist(pos, target)), pos))
        .collect();
    tiles.sort_unstable();
    tiles.into_iter().map(|(_, pos)| pos).collect()
}

/// Whether the player can embark land units at all.
fn can_embark(g: &Game, pid: usize) -> bool {
    g.has_ability(pid, "mana") || g.tree_effect(pid, "land_unit_embark") > 0.0
}

impl AdvancedAi {
    /// Whether `uid` belongs to the crossing in hand. Always false with the
    /// gene off.
    pub(super) fn amphibious_member(&self, uid: u32) -> bool {
        self.amphibious_siege_train
            && self
                .amphibious_train
                .as_ref()
                .is_some_and(|train| train.members.contains(&uid))
    }

    /// The water hold's hook (`siege_stage_step`, `close_to_staging`): a
    /// member's turn when a crossing for `target` forms or is in hand, else
    /// `None` and the hold stands. `None` with the gene off.
    pub(super) fn amphibious_hold_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: u32,
    ) -> Option<bool> {
        if !self.amphibious_siege_train {
            return None;
        }
        let replaceable = self.amphibious_train.as_ref().is_some_and(|train| {
            train.phase == AmphibiousPhase::Assemble
                && train.target != target
                && !original_capital(g, train.target)
                && original_capital(g, target)
        });
        if self.amphibious_train.is_none() || replaceable {
            self.form_amphibious_train(g, pid, target);
        }
        if !self.amphibious_member(uid) {
            return None;
        }
        self.amphibious_member_step(g, pid, uid)
    }

    /// The unit step's first act for a member of the crossing in hand:
    /// `Some(acted)` while the train owns the unit, `None` otherwise and
    /// with the gene off.
    pub(super) fn amphibious_member_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.amphibious_member(uid) {
            return None;
        }
        if !self.amphibious_train_holds(g, pid) {
            return None;
        }
        Some(self.amphibious_member_turn(g, pid, uid))
    }

    /// Form a crossing for `target` if the gate passes (module docs). True
    /// when one forms. A refusal is journalled once per turn and target
    /// ("Siege train cannot ship for"), so a water-locked game shows which
    /// gate held the guns.
    pub(super) fn form_amphibious_train(&mut self, g: &Game, pid: usize, target: u32) -> bool {
        match self.amphibious_gate(g, pid, target) {
            Ok(train) => {
                let guns = train.members.iter().filter(|id| is_gun(g, **id)).count();
                let target_name = g.cities[&target].name.clone();
                let port_name = g.cities[&train.port].name.clone();
                think!(self.journal(), Military, Decision,
                    "Siege train ships for {} from {}", target_name, port_name;
                    "amphibious-siege-train: no dry road reaches {}; {} gun(s) and {} escort(s) \
                     cross a lane of {} water tiles, no enemy warship within {}, to land at {:?}, \
                     {} tiles from the city",
                    target_name, guns, train.members.len() - guns, train.lane.len(),
                    AMPHIB_LANE_SAFETY, train.landing, g.wdist(train.landing, train.target_pos);
                    train.target_pos);
                self.amphibious_train = Some(train);
                true
            }
            Err(reason) => {
                if self.amphibious_refused != Some((g.turn, target)) {
                    self.amphibious_refused = Some((g.turn, target));
                    if let Some(city) = g.cities.get(&target) {
                        think!(self.journal(), Military, Detail,
                            "Siege train cannot ship for {}", city.name;
                            "amphibious-siege-train: {}", reason;
                            city.pos);
                    }
                }
                false
            }
        }
    }

    /// The gate (module docs): the crossing for `target`, or why not.
    fn amphibious_gate(
        &self,
        g: &Game,
        pid: usize,
        target: u32,
    ) -> Result<AmphibiousTrain, String> {
        let Some(city) = g.cities.get(&target) else {
            return Err("the city is gone".into());
        };
        if city.owner == pid || !g.is_at_war(pid, city.owner) {
            return Err("the city is not an enemy's".into());
        }
        if !can_embark(g, pid) {
            return Err("no embarkation technology".into());
        }
        let target_pos = city.pos;
        let Some(group) = self.force_groups.iter().find(|group| {
            group.domain == ForceDomain::Land && g.city_at(group.objective) == Some(target)
        }) else {
            return Err("no land force group has the city as its objective".into());
        };
        let ashore: Vec<u32> = group
            .units
            .iter()
            .copied()
            .filter(|id| {
                g.units
                    .get(id)
                    .is_some_and(|unit| unit.owner == pid && !g.is_embarked(unit) && unit.hp > 0)
            })
            .collect();
        let Some(rep) = ashore.iter().copied().find(|id| is_gun(g, *id)) else {
            return Err("the force group has no gun ashore".into());
        };
        let landings = landing_tiles(g, pid, rep, target_pos);
        if landings.is_empty() {
            return Err(format!(
                "no free landing tile beside the water within {AMPHIB_LANDING_MAX} of the city"
            ));
        }
        let ports: Vec<(u32, Pos)> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| (cid, g.cities[&cid].pos))
            .filter(|(_, pos)| !shore_water(g, rep, *pos).is_empty())
            .collect();
        if ports.is_empty() {
            return Err("no coastal city of ours".into());
        }
        // The shortest lane over every port and landing; nearer members
        // break a tie, then the board's order.
        let mut best: Option<((usize, i32, usize), u32, Pos, Pos, Vec<Pos>)> = None;
        for (rank, landing) in landings.iter().enumerate() {
            for (cid, port) in &ports {
                let Some(lane) = sea_lane(g, pid, rep, *port, *landing) else {
                    continue;
                };
                let near: i32 = ashore
                    .iter()
                    .map(|id| g.wdist(g.units[id].pos, *port))
                    .sum();
                let key = (lane.len(), near, rank);
                if best.as_ref().is_none_or(|(old, ..)| key < *old) {
                    best = Some((key, *cid, *port, *landing, lane));
                }
            }
        }
        let Some((_, port, port_pos, landing, lane)) = best else {
            return Err(format!(
                "no clear sea lane of at most {AMPHIB_SEA_LANE_MAX} tiles from {} port(s) to {} \
                 landing(s)",
                ports.len(),
                landings.len()
            ));
        };
        if lane_watched(g, pid, &lane) {
            return Err(format!(
                "an enemy warship within {AMPHIB_LANE_SAFETY} of the lane from {}",
                g.cities[&port].name
            ));
        }
        let mut near_port: Vec<(i32, u32)> = ashore
            .iter()
            .map(|id| (g.wdist(g.units[id].pos, port_pos), *id))
            .filter(|(distance, _)| *distance <= AMPHIB_MUSTER_RADIUS)
            .collect();
        near_port.sort_unstable();
        let guns: Vec<u32> = near_port
            .iter()
            .map(|(_, id)| *id)
            .filter(|id| is_gun(g, *id))
            .take(AMPHIB_MAX_GUNS)
            .collect();
        let escorts: Vec<u32> = near_port
            .iter()
            .map(|(_, id)| *id)
            .filter(|id| is_escort(g, *id))
            .take(AMPHIB_MAX_ESCORTS)
            .collect();
        if guns.is_empty() || escorts.len() < AMPHIB_MIN_ESCORTS {
            return Err(format!(
                "{} gun(s) and {} escort(s) within {AMPHIB_MUSTER_RADIUS} of {} (needs 1 and \
                 {AMPHIB_MIN_ESCORTS})",
                guns.len(),
                escorts.len(),
                g.cities[&port].name
            ));
        }
        Ok(AmphibiousTrain {
            target,
            target_pos,
            port,
            port_pos,
            landing,
            lane,
            members: guns.iter().chain(escorts.iter()).copied().collect(),
            phase: AmphibiousPhase::Assemble,
            formed: g.turn,
        })
    }

    /// Whether the crossing in hand still holds; abandons it (journal) and
    /// returns false when not. Prunes lost members.
    pub(super) fn amphibious_train_holds(&mut self, g: &Game, pid: usize) -> bool {
        let Some(mut train) = self.amphibious_train.take() else {
            return false;
        };
        train
            .members
            .retain(|id| g.units.get(id).is_some_and(|unit| unit.owner == pid));
        let guns = train.members.iter().filter(|id| is_gun(g, **id)).count();
        let escorts = train.members.iter().filter(|id| is_escort(g, **id)).count();
        let reason = match g.cities.get(&train.target) {
            None => Some("the city is gone".to_string()),
            Some(city) if city.owner == pid || !g.is_at_war(pid, city.owner) => {
                Some("the city is no longer an enemy's".to_string())
            }
            Some(_) if g.turn.saturating_sub(train.formed) > AMPHIB_PATIENCE => {
                Some(format!("{AMPHIB_PATIENCE} turns passed"))
            }
            Some(_) if guns == 0 => Some("no gun is left".to_string()),
            Some(_) if escorts == 0 => Some("no escort is left".to_string()),
            Some(_) if lane_watched(g, pid, &train.lane) => {
                Some("an enemy warship watches the lane".to_string())
            }
            Some(_) => None,
        };
        if let Some(reason) = reason {
            let name = g
                .cities
                .get(&train.target)
                .map_or_else(|| "its target".to_string(), |city| city.name.clone());
            think!(self.journal(), Military, Decision,
                "Siege train abandons the crossing to {}", name;
                "amphibious-siege-train: {}", reason;
                train.target_pos);
            return false;
        }
        self.amphibious_train = Some(train);
        true
    }

    /// One member's turn in the crossing (module docs).
    fn amphibious_member_turn(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let Some(train) = self.amphibious_train.clone() else {
            return false;
        };
        let rep = train
            .members
            .iter()
            .copied()
            .find(|id| is_gun(g, *id))
            .unwrap_or(uid);
        let at = |g: &Game, id: u32| g.units[&id].pos;
        if train.phase == AmphibiousPhase::Assemble {
            let gathered = train.members.iter().all(|id| {
                g.wdist(at(g, *id), train.port_pos) <= AMPHIB_ASSEMBLY_RADIUS
                    && !g.is_embarked(&g.units[id])
            });
            if gathered {
                if let Some(held) = self.amphibious_train.as_mut() {
                    held.phase = AmphibiousPhase::Sail;
                }
                think!(self.journal(), Military, Detail,
                    "Siege train embarks for {} from {}",
                    g.cities[&train.target].name, g.cities[&train.port].name;
                    "amphibious-siege-train: {} member(s) gathered within {} tiles of the port",
                    train.members.len(), AMPHIB_ASSEMBLY_RADIUS;
                    train.target_pos);
                return self.amphibious_member_turn(g, pid, uid);
            }
            if g.wdist(at(g, uid), train.port_pos) <= AMPHIB_ASSEMBLY_RADIUS {
                return self.base.fortify_or_stop(g, pid, uid);
            }
            let step = g
                .route_step_dry(
                    uid,
                    train.port_pos,
                    AMPHIB_ASSEMBLY_RADIUS,
                    super::siege_train::STAGE_DRY_LIMIT,
                )
                .map(|(step, _)| step)
                .or_else(|| g.route_step(uid, train.port_pos, AMPHIB_ASSEMBLY_RADIUS))
                .filter(|pos| landable(g, *pos) && g.can_move(uid, *pos));
            return match step {
                Some(pos) => self.base.tactical_apply_move(g, pid, uid, pos),
                None => self.base.fortify_or_stop(g, pid, uid),
            };
        }
        // Sail. The beach is the landing tile and the land beside it.
        let beach: BTreeSet<Pos> = std::iter::once(train.landing)
            .chain(g.nbrs(train.landing))
            .filter(|pos| landable(g, *pos))
            .collect();
        let ashore_on_beach = |g: &Game, id: u32| {
            let unit = &g.units[&id];
            !g.is_embarked(unit) && g.wdist(unit.pos, train.landing) <= 2
        };
        if train.members.iter().all(|id| ashore_on_beach(g, *id)) {
            think!(self.journal(), Military, Decision,
                "Siege train lands at {:?} before {}", train.landing, g.cities[&train.target].name;
                "amphibious-siege-train: {} member(s) ashore {} tiles from the city; the \
                 siege march takes them on",
                train.members.len(), g.wdist(train.landing, train.target_pos);
                train.target_pos);
            self.amphibious_train = None;
            return false;
        }
        if ashore_on_beach(g, uid) {
            return self.base.fortify_or_stop(g, pid, uid);
        }
        let here = at(g, uid);
        let embarked = g.is_embarked(&g.units[&uid]);
        // A free beach tile beside a member at sea: escorts land once the
        // whole train is at sea within two tiles of the landing; a gun lands
        // only beside an escort already ashore.
        if embarked {
            let free: Vec<Pos> = g
                .nbrs(here)
                .into_iter()
                .filter(|pos| beach.contains(pos) && g.can_move(uid, *pos))
                .collect();
            if !free.is_empty() {
                let arrived = train
                    .members
                    .iter()
                    .all(|id| g.wdist(at(g, *id), train.landing) <= 2);
                if is_escort(g, uid) && arrived {
                    // The escort shields: the free tile nearest the city.
                    let pos = *free
                        .iter()
                        .min_by_key(|pos| (g.wdist(**pos, train.target_pos), **pos))
                        .expect("non-empty");
                    return self.base.tactical_apply_move(g, pid, uid, pos);
                }
                if is_gun(g, uid) {
                    // Beside an escort ashore (the beach is the landing tile
                    // and its ring, so two tiles reach across it), on the
                    // free tile furthest from the city.
                    let covered = free
                        .iter()
                        .copied()
                        .filter(|pos| {
                            train.members.iter().any(|id| {
                                is_escort(g, *id)
                                    && ashore_on_beach(g, *id)
                                    && g.wdist(at(g, *id), *pos) <= 2
                            })
                        })
                        .max_by_key(|pos| {
                            (g.wdist(*pos, train.target_pos), std::cmp::Reverse(*pos))
                        });
                    if let Some(pos) = covered {
                        return self.base.tactical_apply_move(g, pid, uid, pos);
                    }
                }
                // A gun with no escort ashore yet, or an escort waiting for
                // the rest of the train: hold beside the beach.
                return self.base.fortify_or_stop(g, pid, uid);
            }
        }
        // Keep together: a member well ahead of the furthest waits.
        let mine = g.wdist(here, train.landing);
        let furthest = train
            .members
            .iter()
            .filter(|id| !ashore_on_beach(g, **id))
            .map(|id| g.wdist(at(g, *id), train.landing))
            .max()
            .unwrap_or(mine);
        if embarked && mine + AMPHIB_COHESION < furthest {
            return self.base.fortify_or_stop(g, pid, uid);
        }
        // Steer for water beside a FREE beach tile once any lies open, so a
        // member whose beach neighbour an escort took moves along the shore.
        let free_beach: Vec<Pos> = beach
            .iter()
            .copied()
            .filter(|pos| g.unit_ids_at(*pos).is_empty())
            .collect();
        let toward = if free_beach.is_empty() {
            beach.iter().copied().collect::<Vec<_>>()
        } else {
            free_beach
        };
        let anchorage: std::collections::HashSet<Pos> = toward
            .iter()
            .flat_map(|pos| shore_water(g, rep, *pos))
            .collect();
        if anchorage.contains(&here) {
            return self.base.fortify_or_stop(g, pid, uid);
        }
        match g
            .route_step_to_any(uid, &anchorage)
            .filter(|pos| g.can_move(uid, *pos))
        {
            Some(pos) => self.base.tactical_apply_move(g, pid, uid, pos),
            None => self.base.fortify_or_stop(g, pid, uid),
        }
    }

    /// Follow the crossing in hand through a live board rebuild
    /// (`remap_siege_memory`): the target and the port by position, the
    /// members through `units` (old id to new, same owner). The live seat
    /// plays `--fresh-board`, which renumbers cities and units every turn;
    /// kept under the old ids, a replay of live Emperor 004440Z re-formed the
    /// train for Amsterdam on t156 two turns after it shipped, because the
    /// old target id no longer named Amsterdam. A train whose target or port
    /// the new board does not hold is dropped; the next water hold re-forms
    /// it.
    pub(super) fn remap_amphibious_train(
        &mut self,
        previous: &Game,
        next: &Game,
        units: &BTreeMap<u32, u32>,
    ) {
        let Some(mut train) = self.amphibious_train.take() else {
            return;
        };
        let (Some(target), Some(port)) =
            (next.city_at(train.target_pos), next.city_at(train.port_pos))
        else {
            return;
        };
        train.target = target;
        train.port = port;
        train.members = std::mem::take(&mut train.members)
            .into_iter()
            .filter_map(|old| {
                let new = *units.get(&old)?;
                (previous.units.get(&old)?.owner == next.units.get(&new)?.owner).then_some(new)
            })
            .collect();
        self.amphibious_train = Some(train);
    }
}

/// Whether `cid` is a major's original capital still held by its founder.
fn original_capital(g: &Game, cid: u32) -> bool {
    g.cities
        .get(&cid)
        .is_some_and(|city| city.is_capital && city.original_owner == city.owner)
}

#[cfg(test)]
pub(super) mod tests;
