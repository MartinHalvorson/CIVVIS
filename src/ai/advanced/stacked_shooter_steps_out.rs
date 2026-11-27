//! `stacked-shooter-steps-out`: a City Center of ours on which a shooter
//! shares the tile with another of our soldiers, while an enemy soldier is in
//! sight nearby, keeps one shooter on the centre and steps every other soldier
//! there onto the best free tile beside it, before the battle is planned. The
//! shooters can then fire; stacked, none of them can.
//!
//! **The host rule.** Civilization VI puts a unit trained or bought in a
//! city whose centre is already held onto the centre itself, and offers a
//! shooter that shares a City Center with another of its owner's combat
//! units no target. The engine models the refusal
//! ([`Game::shooter_stacked_in_city`]: 657 of 746 such shots refused across
//! 70 live logs of 2026-11-10, against 3% for a shooter alone on its tile),
//! but nothing in the seat ever moved the second soldier out, and
//! `shooters-man-the-capital` (armed live) calls a shooter that stands on the
//! capital's centre, cannot fire it there, and holds it there for the turn.
//!
//! **Why (live Deity, 6 players, Small Continents, Online: the 28 games of
//! 2026-11-22..27 in which our civilization was eliminated, read from their
//! `events.jsonl` on the ledger branch; `docs/DEFENSE_TACTICS.md`).**
//!
//! - 531 of our turns began with two or more of our soldiers on one of our
//!   City Centers; on 468 of them an enemy soldier stood within four tiles.
//!   Every one of the 28 games had such turns.
//! - A shooter stacked on a centre with an enemy within its range at the
//!   start of our turn fired on 68 of 408 such turns (17%). A shooter alone on
//!   a centre fired on 322 of 384 (84%).
//! - In 11 Deity games of the same days that were invaded and survived, the
//!   stacked share of shooter-turns with a target was 6% (43 of 745); in the
//!   eliminated games it was 25% (408 of 1,641).
//! - Lisbon, `civvis-t7-20261123T091247Z`: walls, and five Archers at full
//!   health in and beside the City Center from turn 54 to 59, three of them on
//!   the centre. Byzantium's Catapult stood beside the city from turn 57, the
//!   walls fell from 100 to 26 to 0 on turns 58-59 and the city on turn 60.
//!   Our Archers fired three times in those six turns, every shot from the
//!   one Archer that stood alone on its tile; the three on the centre were
//!   refused every Fortify, Alert and Skip the bridge gave them.
//!
//! ## What the gene does
//!
//! At the start of our unit turn, before `shooters-man-the-capital` and the
//! battle plan, for each City Center of ours that holds a shooter and another
//! of our combat units (a ship in port counts: the engine's reading of the
//! host rule counts any unit with strength),
//! while a land soldier (no recon) of a power
//! at war with us, barbarians included, stands in sight within
//! [`STEP_OUT_THREAT_TILES`] tiles of it:
//!
//! 1. **Who keeps the centre.** A soldier there that guards a civilian (in a
//!    formation, a Settler's bound escort, a Builder's reserved guard) keeps
//!    its charge and the centre. With none, the best shooter keeps it: a
//!    land one before a ship, the strongest ranged attack, then the most
//!    health, then the lowest id. A
//!    shooter behind the city's walls fires every turn and cannot be struck
//!    while the city stands, the operator's "archer in city is solid".
//! 2. **Who steps out.** Every other soldier on the centre that is no guard
//!    and still has movement steps once onto a passable tile beside the
//!    centre, dry land for a land unit and water for a ship, that holds no
//!    City Center, no unit of another player and no soldier of ours. The tile is the one with the best score:
//!    [`STEP_OUT_SHOT`] when a shooter keeps movement after the step and has a
//!    visible enemy soldier within its range and line of sight from there,
//!    minus [`STEP_OUT_BESIDE_ENEMY`] for each enemy melee soldier beside the
//!    tile and [`STEP_OUT_NEAR_ENEMY`] for each two tiles away, plus the
//!    tile's defence (hills, woods and rainforest +3: "hide behind a river,
//!    hills, woods/jungle"), plus [`STEP_OUT_OWN_GROUND`] on our own land;
//!    the lowest tile on a tie. When the engine refuses the step (zone of
//!    control, a cost it cannot pay), the next tile is tried. With no tile it
//!    can take (a coastal centre ringed by our own soldiers: Lisbon t56) the
//!    stack stands and a Detail line says so.
//! 3. **Nothing else.** The step is the gene's only order. The shot, and
//!    the rest of the unit's turn, are the battle plan's and the per-unit
//!    ladder's, which now see a shooter that may fire.
//!
//! Stateless: every turn is read from that turn's board. Off, nothing here
//! runs. Grep the journal for `stacked-shooter-steps-out:`.

use super::*;

/// How near a centre an enemy soldier must stand for the stack to break: one
/// turn's march of a two-move soldier from a shooter's two-tile reach of the
/// tile beside the centre.
pub(crate) const STEP_OUT_THREAT_TILES: i32 = 4;

/// The score of a tile from which the stepped-out shooter can fire this turn.
pub(crate) const STEP_OUT_SHOT: i64 = 6;

/// The cost of each enemy melee soldier standing beside the tile: it strikes
/// a soldier there next turn without moving.
pub(crate) const STEP_OUT_BESIDE_ENEMY: i64 = 4;

/// The cost of each enemy melee soldier two tiles from the tile.
pub(crate) const STEP_OUT_NEAR_ENEMY: i64 = 1;

/// The score of a tile inside our borders.
pub(crate) const STEP_OUT_OWN_GROUND: i64 = 1;

/// An enemy soldier read by the pass: its position, and whether it strikes in
/// melee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StepOutEnemy {
    pos: Pos,
    melee: bool,
}

impl AdvancedAi {
    /// A combat unit of ours, land or sea: what shares a centre under the
    /// engine's reading of the host rule (`Game::shooter_stacked_in_city`
    /// counts any unit with strength, a Galley in port included).
    fn step_out_soldier(g: &Game, pid: usize, uid: u32) -> bool {
        g.units.get(&uid).is_some_and(|unit| {
            let spec = &g.rules.units[unit.kind];
            unit.owner == pid
                && unit.hp > 0
                && spec.class == "military"
                && spec.strength > 0.0
                && spec.domain.as_deref() != Some("air")
        })
    }

    /// A ship: it steps onto water, everything else onto land.
    fn step_out_ship(g: &Game, uid: u32) -> bool {
        g.units
            .get(&uid)
            .is_some_and(|unit| g.rules.units[unit.kind].domain.as_deref() == Some("sea"))
    }

    /// A unit with a ranged attack.
    fn step_out_shooter(g: &Game, uid: u32) -> bool {
        g.units
            .get(&uid)
            .is_some_and(|unit| g.rules.units[unit.kind].has_ranged_attack())
    }

    /// A soldier that is a civilian's: in a formation, a Settler's bound
    /// escort or a Builder's reserved guard. It keeps its charge's tile.
    fn step_out_guard(&self, g: &Game, uid: u32) -> bool {
        g.units[&uid].linked_to.is_some()
            || self.guard_is_bound_to_any_settler(uid)
            || self.guard_is_reserved_for_civilian(uid)
    }

    /// The enemy land soldiers we see within `radius` of `at`, recon left out.
    fn step_out_enemies(g: &Game, pid: usize, at: Pos, radius: i32) -> Vec<StepOutEnemy> {
        let visible = g.player_vision_frame(pid);
        let mut seen: Vec<(u32, StepOutEnemy)> = g
            .units
            .values()
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                unit.owner != pid
                    && unit.hp > 0
                    && spec.class == "military"
                    && spec.promotion_class != "recon"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && g.wdist(unit.pos, at) <= radius
                    && g.is_at_war(pid, unit.owner)
                    && !g.is_embarked(unit)
                    && g.sees(&visible, unit.pos)
                    && g.unit_visible_to(unit.id, pid)
            })
            .map(|unit| {
                (
                    unit.id,
                    StepOutEnemy {
                        pos: unit.pos,
                        melee: g.rules.units[unit.kind].is_melee_capable(),
                    },
                )
            })
            .collect();
        seen.sort_unstable_by_key(|(id, _)| *id);
        seen.into_iter().map(|(_, enemy)| enemy).collect()
    }

    /// Whether `tile` beside a centre may take a stepped-out soldier:
    /// passable, dry for a land unit and water for a ship, no City Center, no
    /// unit of another player, no soldier of ours.
    fn step_out_tile_open(g: &Game, pid: usize, tile: Pos, ship: bool) -> bool {
        let Some(ground) = g.map.get(tile) else {
            return false;
        };
        g.rules.is_passable(ground)
            && g.rules.is_water(ground) == ship
            && g.city_at(tile).is_none()
            && g.unit_ids_at(tile).iter().all(|id| {
                g.units.get(id).is_none_or(|other| {
                    other.owner == pid && g.rules.units[other.kind].strength <= 0.0
                })
            })
    }

    /// The tiles beside `centre` that `uid` may step onto, best first, each
    /// with its score and whether a shooter keeps a shot from it.
    fn step_out_tiles(
        g: &Game,
        pid: usize,
        uid: u32,
        centre: Pos,
        enemies: &[StepOutEnemy],
    ) -> Vec<(i64, Pos, bool)> {
        let Some(unit) = g.units.get(&uid) else {
            return Vec::new();
        };
        let shooter = Self::step_out_shooter(g, uid);
        let ship = Self::step_out_ship(g, uid);
        let reach = g.unit_attack_range(uid);
        let moves = unit.moves_left;
        // Leaving one enemy's zone of control for another's costs every point
        // of movement: a shooter that does so cannot fire this turn.
        let centre_in_zoc = enemies
            .iter()
            .any(|enemy| enemy.melee && g.wdist(enemy.pos, centre) <= 1);
        let mut tiles: Vec<(i64, Pos, bool)> = g
            .nbrs(centre)
            .into_iter()
            .filter(|tile| Self::step_out_tile_open(g, pid, *tile, ship))
            .map(|tile| {
                let beside = enemies
                    .iter()
                    .filter(|enemy| enemy.melee && g.wdist(enemy.pos, tile) == 1)
                    .count() as i64;
                let near = enemies
                    .iter()
                    .filter(|enemy| enemy.melee && g.wdist(enemy.pos, tile) == 2)
                    .count() as i64;
                let keeps_moves = moves - g.step_cost_for(uid, centre, tile) > 1e-9
                    && !(centre_in_zoc && beside > 0);
                let shot = shooter
                    && keeps_moves
                    && enemies.iter().any(|enemy| {
                        g.wdist(tile, enemy.pos) <= reach
                            && g.unit_has_line_of_sight_from(uid, tile, enemy.pos)
                    });
                let own = g
                    .map
                    .get(tile)
                    .and_then(|ground| ground.owner_city)
                    .and_then(|city| g.cities.get(&city))
                    .is_some_and(|city| city.owner == pid);
                let shot_score = if shot { STEP_OUT_SHOT } else { 0 };
                let ground_score = if own { STEP_OUT_OWN_GROUND } else { 0 };
                let score =
                    shot_score - STEP_OUT_BESIDE_ENEMY * beside - STEP_OUT_NEAR_ENEMY * near
                        + g.tile_defense_bonus(tile).round() as i64
                        + ground_score;
                (score, tile, shot)
            })
            .collect();
        tiles.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        tiles
    }

    /// The pre-pass: see the module. The number of soldiers stepped out.
    /// Nothing with the gene off.
    pub(super) fn step_stacked_soldiers_out(&mut self, g: &mut Game, pid: usize) -> usize {
        if !self.stacked_shooter_steps_out || self.base.minor || self.base.barb {
            return 0;
        }
        let mut centres: Vec<(u32, Pos)> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| (cid, g.cities[&cid].pos))
            .collect();
        centres.sort_unstable();
        let mut stepped = 0;
        for (cid, centre) in centres {
            let mut soldiers: Vec<u32> = g
                .unit_ids_at(centre)
                .iter()
                .copied()
                .filter(|uid| Self::step_out_soldier(g, pid, *uid))
                .collect();
            soldiers.sort_unstable();
            if soldiers.len() < 2 || !soldiers.iter().any(|uid| Self::step_out_shooter(g, *uid)) {
                continue;
            }
            // Read two tiles past the threat radius: a tile beside the centre
            // reaches that far.
            let enemies = Self::step_out_enemies(g, pid, centre, STEP_OUT_THREAT_TILES + 2);
            if !enemies
                .iter()
                .any(|enemy| g.wdist(enemy.pos, centre) <= STEP_OUT_THREAT_TILES)
            {
                continue;
            }
            let guards: Vec<u32> = soldiers
                .iter()
                .copied()
                .filter(|uid| self.step_out_guard(g, *uid))
                .collect();
            let keeper = if guards.is_empty() {
                soldiers
                    .iter()
                    .copied()
                    .filter(|uid| Self::step_out_shooter(g, *uid))
                    .max_by_key(|uid| {
                        let unit = &g.units[uid];
                        let strength = g.rules.units[unit.kind].ranged_attack_strength();
                        (
                            !Self::step_out_ship(g, *uid),
                            (strength * 100.0).round() as i64,
                            unit.hp,
                            std::cmp::Reverse(*uid),
                        )
                    })
            } else {
                None
            };
            let movers: Vec<u32> = soldiers
                .iter()
                .copied()
                .filter(|uid| {
                    Some(*uid) != keeper
                        && !guards.contains(uid)
                        && g.units[uid].moves_left > 0.0
                        && !g.is_embarked(&g.units[uid])
                })
                .collect();
            let city = g.cities[&cid].name.clone();
            let held_by = keeper.or_else(|| guards.first().copied()).map_or_else(
                || "nobody".to_string(),
                |uid| plain(g.units[&uid].kind.as_str()),
            );
            for uid in movers {
                let kind = g.units[&uid].kind;
                let tiles = Self::step_out_tiles(g, pid, uid, centre, &enemies);
                let open = tiles.len();
                let mut moved = false;
                for (score, tile, shot) in tiles {
                    if g.apply(
                        pid,
                        &Action::Move {
                            unit: uid,
                            to: tile,
                        },
                    )
                    .is_err()
                    {
                        continue;
                    }
                    stepped += 1;
                    moved = true;
                    think!(self.journal(), Military, Decision,
                        "The {} steps out of {city}'s crowded City Center", plain(kind.as_str());
                        "stacked-shooter-steps-out: {} of our soldiers shared the centre, which \
                         the {held_by} keeps; {centre:?} -> {tile:?}, score {score}{}",
                        soldiers.len(),
                        if shot { ", a shot from there" } else { "" });
                    break;
                }
                if !moved {
                    // Every tile beside the centre is water, a city, foreign
                    // or held by a soldier of ours, or the engine refused the
                    // step: the stack stands this turn.
                    think!(self.journal(), Military, Detail,
                        "The {} stays on {city}'s crowded City Center", plain(kind.as_str());
                        "stacked-shooter-steps-out: {} of our soldiers share the centre and \
                         {open} tile(s) beside it are open, none of them a step it can take",
                        soldiers.len());
                }
            }
        }
        if stepped > 0 {
            self.force_groups_dirty = true;
        }
        stepped
    }
}

#[cfg(test)]
mod tests;
