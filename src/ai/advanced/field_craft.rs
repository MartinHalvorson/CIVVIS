//! Field craft: four opt-in genes that spend the engine's own combat rules
//! the way a human does at the front — before the blow, around the blow,
//! and with the ground and the neighbours the blow is fought on.
//!
//! Operator goal (2026-08-24): *"very smart heuristics around unit tactics in
//! warfare … preserve our units as much as possible, heal up, and use
//! everything to our advantage. flip nearby city states, defend in our
//! territory for maximum healing rates, utilize support bonuses, zone of
//! control and whatever else you can think of."*
//!
//! A stand-still posture screened negatively at 38,160 seats: a unit that
//! stands still in a major war is a unit that is not at the siege, and the
//! whole regime is decided by tempo. So none of the four genes below adds a
//! stand. Each is a piece of *actuation* the controller could not take
//! before, priced on the engine's own arithmetic, and each spends at most the
//! movement the unit was going to spend anyway.
//!
//! ## The four engine facts, and the gene that reads each one
//!
//! 1. **A body that enters our zone of control stops.** `Game::flow_past`
//!    sets a mover's remaining movement to zero on
//!    `formation_enters_enemy_zoc`, and `attack_reach` never strikes from a
//!    tile reached with nothing left — so a melee unit that has to cross one
//!    of our zones to get adjacent cannot step into contact *and* swing on
//!    the same turn. ⚠ **Shooters exert no zone of control** in the shipped
//!    rules (Archer, Slinger, Crossbowman, every siege unit: `zone_of_control`
//!    unset in `data/units.json`, as in Civilization VI), so a *lone* archer
//!    that steps one tile back from a Warrior in open ground is still inside
//!    its reach — the Warrior walks the empty tile and swings. The step pays
//!    exactly where a human takes it: behind a melee friend whose zone
//!    covers the approach, across a river, onto ground the body cannot pay
//!    to enter and attack from. That is [`AdvancedAi::shoot_and_scoot_step`]
//!    — `shoot-and-scoot`: a ranged unit whose tile a hostile melee body can
//!    reach steps to a firing tile inside strictly fewer hostile envelopes
//!    (the engine's own `attack_reach`, which already carries every one of
//!    those facts) from which that body is still a shot — and, in a war,
//!    leaves the shot itself to the attack scan next pass, which prices the
//!    kill, the siege and the reply from the new tile; a siege city in range
//!    now stays in range. Movement it had to spend to shoot at all; no
//!    counter-blow (`do_ranged` has none); and `None` in the open field,
//!    where no such tile exists. The barbarian regime gets the same step
//!    from the peacetime path, firing at the raider directly because no scan
//!    follows on that path.
//!
//! 2. **The same fact, from the other side of the line.** If our melee unit
//!    stands beside our archer, every tile it touches is a stop sign, and the
//!    archer's approaches are inside them — which, since the archer has no
//!    zone of its own, is the *only* thing that keeps a body from walking up
//!    to it and swinging in one turn. The doctrine arena measured this
//!    as the shipped controller's one durable gap — *ranged screened* 25–32%
//!    against `basic`'s 39–50% on `the_reserve`, and it was the remaining
//!    candidate after the march-cohesion lane deflated. That is
//!    [`AdvancedAi::zoc_screen_step`] — `zoc-screen`: a melee unit with
//!    nothing to hit asks, exactly, which stand takes the most (friend, enemy)
//!    reaches off the board — `attack_reach` on a speculative clone with the
//!    unit moved there, against the same reading with the unit absent — and
//!    takes it when it takes at least one. It holds that stand only while it
//!    is load-bearing: the moment no enemy can reach the friend anyway, the
//!    ordinary march resumes. That is the design note the cohesion lane left
//!    behind — *condition any wait on your own force, never on enemy threat*
//!    — read literally: the screen is defined by where OUR archer stands.
//!
//! 3. **A pillage is a heal, and it does not need the turn.** The shipped
//!    `Improvements` table carries `plunder_type: heal, plunder_amount: 50`
//!    for Farms, Pastures, Plantations, Camps and Fishing Boats
//!    (`data/improvements.json`), and `grant_pillage_yield` applies it on the
//!    spot — `hp = min(100, hp + 50)` — where a turn of standing in enemy
//!    territory heals `HealingLocation::EnemyTerritory.rate()` = **0**. So a
//!    wounded unit in the enemy's fields has a fifty-point heal under its feet
//!    that the recovery path never sees: `healing_step` walks it home. That is
//!    [`AdvancedAi::pillage_to_heal_step`] — `pillage-to-heal`: a unit at or
//!    below [`PILLAGE_HEAL_HP_CEILING`] pillages a heal-type tile it stands on,
//!    or steps one tile onto one and pillages it, before the recovery path is
//!    consulted. The pillage also costs the enemy the tile, which
//!    `raid-pillage-prizes` already prices as income; this gene is the
//!    wound's half of the same order.
//!
//! 4. **A suzerain's land is home.** `Game::healing_location` reads a tile
//!    owned by a city-state whose suzerain is us as `FriendlyTerritory` — the
//!    fifteen-a-turn heal, three times neutral ground — and a city-state
//!    follows its suzerain into wars (`is_at_war` derives the client's side
//!    from `suzerain_of`). So the city-state on our border is a forward
//!    hospital and a second army, and the one on our *enemy's* border under
//!    the enemy's envoys is the reverse. `advanced_envoys` prices type
//!    yields, lane alignment, the suzerainty itself and lane denial — never
//!    where the city-state *is*. That is
//!    [`AdvancedAi::flip_nearby_city_state_bonus`] — `flip-nearby-city-states`:
//!    a proximity term inside [`FLIP_RADIUS`] of one of our cities, doubled and
//!    more when the sitting suzerain is at war with us, amortised over the
//!    envoys the flip still needs exactly as the suzerainty prize is. A reorder
//!    of envoys the seat already spends, never a purchase (the v2 lesson:
//!    deltas that spend more lost 2–4 pp of share).
//!
//! ## Support bonuses
//!
//! `Game::support_bonus` and `flanking_bonus` pay +2 per adjacent friendly
//! military unit once `flanking_support` is unlocked, and the wartime mover
//! already carries `mv_support` for the defensive half. Both genes here that
//! choose ground break their ties toward the tile with more friends beside
//! it, so a screen is also a supported line and a firing tile is chosen among
//! its equals by who stands next to it. Nothing re-derives the bonus; the
//! forward model in the attack scan prices it exactly when the blow is scored.
//!
//! ## Where each hook sits
//!
//! - `pillage_to_heal_step`: before `healing_step`, both regimes — the wound
//!   is the trigger and the recovery path is what it pre-empts.
//! - `shoot_and_scoot_step`: inside the war branch before the attack scan,
//!   and at the top of the empty-enemies branch against
//!   the barbarian seat — a raider in contact is the most urgent thing on
//!   that path and nothing below it shoots before it marches.
//! - `zoc_screen_step`: after the attack scan has declined, before the
//!   reinforcement march — a screen is what a unit does *instead* of
//!   marching, never instead of a blow.
//! - `flip_nearby_city_state_bonus`: one term in `advanced_envoys`'s score.
//!
//! All four are off in `AdvancedAi::new()` and `legacy()`, `Kind::OptIn` rows
//! in `genes.rs`, and byte-identical when off. Fires probes under
//! `docs/gene_screens/fires/`.

use std::cmp::Reverse;

use super::AdvancedAi;
use crate::game::{Action, Game};
use crate::think;
use crate::Pos;

/// One firing tile for the scoot, ordered for `min`: hostile envelopes
/// covering it, then the longest shot (negated distance), then the weakest
/// body, then the most friends beside it, then the tile, the body's id and
/// its tile so a tie is deterministic.
type FiringTile = (usize, i32, i32, Reverse<usize>, Pos, u32, Pos);

/// One stand for the screen, ordered for `min`: (friend, enemy) reaches that
/// survive it, hostile envelopes covering it, then the most friends beside
/// it, the best ground, staying put over moving, then the tile.
type Stand = (usize, usize, Reverse<usize>, Reverse<i64>, bool, Pos);

/// A unit at or below this many hit points takes the fifty-point pillage
/// heal. Sixty-five leaves at most fifteen of the fifty unused; above it the
/// pillage is income, and `raid-pillage-prizes` owns income.
pub(super) const PILLAGE_HEAL_HP_CEILING: i32 = 65;

/// How far from a melee unit a friend is worth screening.
const ZOC_SCREEN_FRIEND_RADIUS: i32 = 3;
/// A melee friend this wounded is a friend worth screening too.
const ZOC_SCREEN_WOUNDED_HP: i32 = 50;
/// A screener below this is a wound, not a wall.
const ZOC_SCREEN_MIN_HP: i32 = 40;
/// Stands read exactly per unit-turn; each costs one speculative clone and
/// one flood per threatening enemy.
const ZOC_SCREEN_CANDIDATES: usize = 8;
const ZOC_SCREEN_ENEMIES: usize = 6;

/// A city-state within this many tiles of one of our cities is on our
/// border. Nine is the distance a Warrior walks in a handful of turns and
/// the ring inside which its borders meet ours on the screen's map.
pub(super) const FLIP_RADIUS: i32 = 9;
/// Per tile inside the radius, so the city-state on the border itself is
/// worth ninety before the flip terms — the size of one lane alignment.
const FLIP_PER_TILE: i64 = 10;
/// The sitting suzerain is at war with us: its client fights us today and
/// would fight for us tomorrow.
const FLIP_ENEMY_SUZERAIN: i64 = 200;
/// The sitting suzerain is a rival at peace with us.
const FLIP_RIVAL_SUZERAIN: i64 = 60;

impl AdvancedAi {
    /// Whether this unit is one the field-craft genes act for: the seat's
    /// own land military unit, on the board.
    fn field_craft_unit(&self, g: &Game, pid: usize, uid: u32) -> bool {
        if g.is_arena() || self.base.minor || self.base.barb {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let spec = &g.rules.units[unit.kind];
        unit.owner == pid
            && spec.class == "military"
            && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            && !g.is_embarked(unit)
    }

    /// Whether a flooded tile can be the END of this unit's move: nothing
    /// military is standing on it. `approach_reach` floods through friends
    /// and `Game::can_move` answers one step only, so neither says this.
    fn open_to_stand(g: &Game, pos: Pos) -> bool {
        !g.units_at(pos)
            .into_iter()
            .any(|oid| g.rules.units[g.units[&oid].kind].class == "military")
    }

    /// Walk one already-flooded path, one legal step at a time. `true` when
    /// the unit stands at the path's end.
    fn walk_path(&self, g: &mut Game, pid: usize, uid: u32, path: &[Pos]) -> bool {
        for step in path {
            if !g.can_move(uid, *step) || !self.base.tactical_apply_move(g, pid, uid, *step) {
                return false;
            }
        }
        path.last()
            .is_some_and(|end| g.units.get(&uid).is_some_and(|unit| unit.pos == *end))
    }

    /// How many of this seat's own military units stand beside `pos` —
    /// the count `Game::support_bonus` pays on.
    fn friends_beside(g: &Game, pid: usize, uid: u32, pos: Pos) -> usize {
        g.nbrs(pos)
            .into_iter()
            .flat_map(|n| g.units_at(n))
            .filter(|oid| {
                *oid != uid && {
                    let other = &g.units[oid];
                    other.owner == pid && g.rules.units[other.kind].class == "military"
                }
            })
            .count()
    }

    // ------------------------------------------------------------------
    // shoot-and-scoot
    // ------------------------------------------------------------------

    /// The gene: a ranged unit inside a hostile melee body's reach steps to a
    /// firing tile inside fewer hostile envelopes from which that body is
    /// still a shot. `None` leaves the unit to the ordinary scan and march.
    ///
    /// `keep` is a target the unit must not step out of range of — the
    /// campaign's siege city, when it is in range now — so a sally beside a
    /// bombarding archer cannot pull the archer off the walls. `fire` takes
    /// the shot at the body here and now; off, the ordinary attack scan
    /// runs next pass from the new tile and picks the best target there,
    /// which is the war branch's choice (the scan prices kills, the siege
    /// and the reply; this step only chooses the ground).
    pub(super) fn shoot_and_scoot_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        enemies: &[usize],
        keep: Option<Pos>,
        fire: bool,
    ) -> Option<bool> {
        if !self.shoot_and_scoot || !self.field_craft_unit(g, pid, uid) {
            return None;
        }
        let unit = g.units.get(&uid)?;
        let spec = &g.rules.units[unit.kind];
        if !spec.has_ranged_attack()
            || unit.moves_left <= 0.0
            || unit.attacks_left <= 0
            // `do_ranged` refuses a siege unit that has moved unless it
            // carries the promotion; the scoot would forfeit the shot.
            || (spec.siege && g.promotion_effect(unit, "attack_after_move") == 0.0)
        {
            return None;
        }
        let here = unit.pos;
        let range = g.unit_attack_range(uid).max(1);
        // A siege target in range now stays in range: the walls are what the
        // unit is here for.
        let keep = keep.filter(|target| g.wdist(here, *target) <= range);
        let envelopes = self.base.enemy_attack_envelopes(g, pid);
        // The bodies that can reach us and pay no counter for being shot:
        // hostile melee units. An unanswerable shooter is left to the
        // ordinary tactical movement path.
        let bodies: Vec<(u32, Pos, i32)> = envelopes
            .iter()
            .filter(|(_, reach)| reach.contains(&here))
            .filter_map(|(eid, _)| {
                let enemy = g.units.get(eid)?;
                let enemy_spec = &g.rules.units[enemy.kind];
                (enemies.contains(&enemy.owner)
                    && enemy_spec.is_melee_capable()
                    && !enemy_spec.has_ranged_attack()
                    && enemy_spec.domain.as_deref() != Some("air"))
                .then_some((*eid, enemy.pos, enemy.hp))
            })
            .collect();
        if bodies.is_empty() {
            return None;
        }
        let covered = |pos: &Pos| {
            envelopes
                .iter()
                .filter(|(_, reach)| reach.contains(pos))
                .count()
        };
        let here_covered = covered(&here);
        let frames = (g.player_vision_now(pid), g.visibility_viewers(pid));
        // Every firing tile: reachable with movement left to fire, inside
        // fewer hostile envelopes than here, and with a line of sight to one
        // of the bodies from inside our range. Fewest envelopes first, then
        // the longest shot (the tile the body has furthest to walk), then
        // the weakest body, then the most friends beside the tile, then the
        // tile itself so a tie is deterministic.
        let reach = g.approach_reach(uid);
        let mut best: Option<FiringTile> = None;
        for (tile, (remaining, _)) in &reach {
            if *remaining <= 0.0 || !Self::open_to_stand(g, *tile) {
                continue;
            }
            let tile_covered = covered(tile);
            if tile_covered >= here_covered
                || keep.is_some_and(|target| g.wdist(*tile, target) > range)
            {
                continue;
            }
            for (eid, epos, ehp) in &bodies {
                let distance = g.wdist(*tile, *epos);
                if distance == 0
                    || distance > range
                    || !g.unit_has_line_of_sight_from(uid, *tile, *epos)
                    || !g.combat_target_visible_at(pid, *epos, &frames.0, &frames.1)
                {
                    continue;
                }
                let key = (
                    tile_covered,
                    -distance,
                    *ehp,
                    Reverse(Self::friends_beside(g, pid, uid, *tile)),
                    *tile,
                    *eid,
                    *epos,
                );
                if best.as_ref().is_none_or(|current| key < *current) {
                    best = Some(key);
                }
            }
        }
        let (tile_covered, _, _, _, tile, eid, epos) = best?;
        let path = reach.get(&tile)?.1.clone();
        if !self.walk_path(g, pid, uid, &path) {
            // Blocked part-way: whatever ground was gained stands, and the
            // ordinary scan takes the shot from there next pass.
            return g
                .units
                .get(&uid)
                .is_some_and(|unit| unit.pos != here)
                .then_some(true);
        }
        let fired = fire
            && g.apply(
                pid,
                &Action::Ranged {
                    unit: uid,
                    target: epos,
                },
            )
            .is_ok();
        self.force_groups_dirty |= fired;
        if self.journal().wants(crate::reasoning::Level::Detail) {
            let body = g
                .units
                .get(&eid)
                .map(|enemy| crate::reasoning::plain(&enemy.kind))
                .unwrap_or_else(|| "the body".to_string());
            think!(self.journal(), Military, Detail,
                   "Scoots to {tile:?} and {} {body}", if fired { "shoots" } else { "has a shot at" };
                   "the firing tile is inside {tile_covered} hostile envelope(s) against \
                    {here_covered} here; the body has to walk up again to swing";
                   tile);
        }
        Some(true)
    }

    // ------------------------------------------------------------------
    // pillage-to-heal
    // ------------------------------------------------------------------

    /// Whether pillaging `pos` would heal this seat's unit: a legal pillage
    /// of an improvement whose plunder is hit points.
    fn pillage_heals_at(g: &Game, pid: usize, pos: Pos) -> bool {
        g.pillageable_at(pid, pos)
            && g.map
                .get(pos)
                .and_then(|tile| tile.improvement.as_deref())
                .and_then(|improvement| g.rules.improvements.get(improvement))
                .is_some_and(|spec| spec.plunder_type.as_deref() == Some("heal"))
    }

    /// The gene: a wounded unit pillages the heal-type improvement under it,
    /// or steps one tile onto one and pillages that, before the recovery
    /// path walks it home.
    pub(super) fn pillage_to_heal_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.pillage_to_heal || !self.field_craft_unit(g, pid, uid) {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if unit.hp > PILLAGE_HEAL_HP_CEILING || unit.moves_left <= 0.0 {
            return None;
        }
        let here = unit.pos;
        let hp = unit.hp;
        if Self::pillage_heals_at(g, pid, here) {
            if !g.apply(pid, &Action::Pillage { unit: uid }).is_ok() {
                return None;
            }
            self.force_groups_dirty = true;
            if self.journal().wants(crate::reasoning::Level::Detail) {
                let healed = g.units.get(&uid).map_or(hp, |unit| unit.hp);
                think!(self.journal(), Military, Detail,
                       "Pillages the ground it stands on to heal";
                       "{hp} health becomes {healed}; a turn spent standing here would heal \
                        nothing in enemy territory";
                       here);
            }
            return Some(true);
        }
        // One step onto a healing tile, with movement left to pillage it, and
        // no deeper inside the enemy's reach than the tile it leaves.
        let envelopes = self.base.enemy_attack_envelopes(g, pid);
        let covered = |pos: &Pos| {
            envelopes
                .iter()
                .filter(|(_, reach)| reach.contains(pos))
                .count()
        };
        let here_covered = covered(&here);
        let reach = g.approach_reach(uid);
        let target = reach
            .iter()
            .filter(|(tile, (remaining, _))| {
                *remaining > 0.0
                    && g.wdist(here, **tile) == 1
                    && g.can_move(uid, **tile)
                    && Self::pillage_heals_at(g, pid, **tile)
            })
            .map(|(tile, _)| (covered(tile), *tile))
            .filter(|(tile_covered, _)| *tile_covered <= here_covered)
            .min()
            .map(|(_, tile)| tile)?;
        let path = reach.get(&target)?.1.clone();
        if !self.walk_path(g, pid, uid, &path) {
            return None;
        }
        let pillaged = g.apply(pid, &Action::Pillage { unit: uid }).is_ok();
        self.force_groups_dirty |= pillaged;
        if self.journal().wants(crate::reasoning::Level::Detail) {
            let healed = g.units.get(&uid).map_or(hp, |unit| unit.hp);
            think!(self.journal(), Military, Detail,
                   "Steps onto {target:?} and pillages it to heal";
                   "{hp} health becomes {healed}; the recovery march home would have \
                    healed nothing on the way";
                   target);
        }
        Some(true)
    }

    // ------------------------------------------------------------------
    // zoc-screen
    // ------------------------------------------------------------------

    /// The gene: a melee unit with nothing to hit stands where its zone of
    /// control takes the most enemy reaches off our shooters and wounded.
    /// `Some(false)` holds a load-bearing stand it already occupies.
    pub(super) fn zoc_screen_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        enemies: &[usize],
    ) -> Option<bool> {
        if !self.zoc_screen || !self.field_craft_unit(g, pid, uid) {
            return None;
        }
        let unit = g.units.get(&uid)?;
        let spec = &g.rules.units[unit.kind];
        if !spec.is_melee_capable()
            || spec.has_ranged_attack()
            || !g.exerts_zoc(unit)
            || unit.hp < ZOC_SCREEN_MIN_HP
        {
            return None;
        }
        let here = unit.pos;
        // The friends worth a screen: our shooters and our wounded, in the
        // open, within a short walk.
        let candidates: Vec<(u32, Pos)> = g
            .units
            .values()
            .filter(|friend| {
                let friend_spec = &g.rules.units[friend.kind];
                friend.id != uid
                    && friend.owner == pid
                    && friend_spec.class == "military"
                    && !matches!(friend_spec.domain.as_deref(), Some("sea" | "air"))
                    && !g.is_embarked(friend)
                    && g.wdist(here, friend.pos) <= ZOC_SCREEN_FRIEND_RADIUS
                    && g.city_at(friend.pos).is_none()
                    && (friend_spec.has_ranged_attack() || friend.hp <= ZOC_SCREEN_WOUNDED_HP)
            })
            .map(|friend| (friend.id, friend.pos))
            .collect();
        if candidates.is_empty() {
            return None;
        }
        // The enemies whose reach a zone of control can end, near enough to
        // matter: visible, hostile, not cavalry or air, within a turn's walk
        // and a shot of us. Nearest first, bounded.
        let mut nearby: Vec<(u32, Pos)> = g
            .units
            .values()
            .filter(|enemy| {
                let enemy_spec = &g.rules.units[enemy.kind];
                enemies.contains(&enemy.owner)
                    && enemy_spec.class == "military"
                    && (enemy_spec.is_melee_capable() || enemy_spec.has_ranged_attack())
                    && enemy_spec.domain.as_deref() != Some("air")
                    && g.wdist(here, enemy.pos) <= ZOC_SCREEN_FRIEND_RADIUS + 5
                    && g.unit_visible_to(enemy.id, pid)
                    && !g.unit_ignores_zoc(enemy.id)
            })
            .map(|enemy| (enemy.id, enemy.pos))
            .collect();
        if nearby.is_empty() {
            return None;
        }
        nearby.sort_by_key(|(eid, epos)| (g.wdist(here, *epos), *eid));
        nearby.truncate(ZOC_SCREEN_ENEMIES);
        // ★ THE THREAT IS READ WITH THE SCREENER ABSENT. On the live board a
        // screen that is already standing hides the very reach it stops, so
        // a reading taken there would call the friend safe and march the
        // screen away — the doctrine arena's dissolving line. Every reach
        // below is `attack_reach` on a speculative clone.
        let mut absent = g.speculative_clone();
        absent.remove_unit(uid);
        let mut friends: Vec<(u32, Pos)> = Vec::new();
        let mut threats: Vec<(u32, Pos)> = Vec::new();
        let mut baseline = 0usize;
        for (eid, epos) in &nearby {
            let reach = absent.attack_reach(*eid);
            let mut threatens = false;
            for (fid, fpos) in &candidates {
                if reach.binary_search(fpos).is_ok() {
                    threatens = true;
                    baseline += 1;
                    if !friends.iter().any(|(id, _)| id == fid) {
                        friends.push((*fid, *fpos));
                    }
                }
            }
            if threatens {
                threats.push((*eid, *epos));
            }
        }
        if baseline == 0 {
            return None;
        }
        let nearest_threat = |pos: Pos| {
            threats
                .iter()
                .map(|(_, epos)| g.wdist(pos, *epos))
                .min()
                .unwrap_or(0)
        };
        // The stands: here, and every reachable tile beside a screened friend.
        let reach = g.approach_reach(uid);
        let beside_a_friend = |pos: Pos| friends.iter().any(|(_, fpos)| g.wdist(pos, *fpos) == 1);
        let mut stands: Vec<Pos> = reach
            .keys()
            .copied()
            .filter(|tile| beside_a_friend(*tile) && Self::open_to_stand(g, *tile))
            .collect();
        stands.sort_by_key(|tile| (nearest_threat(*tile), *tile));
        stands.truncate(ZOC_SCREEN_CANDIDATES);
        if beside_a_friend(here) {
            stands.insert(0, here);
        }
        if stands.is_empty() {
            return None;
        }
        // How many (friend, enemy) reaches survive with the screener on a
        // stand.
        let reaches = |world: &Game| -> usize {
            threats
                .iter()
                .map(|(eid, _)| {
                    let reach = world.attack_reach(*eid);
                    friends
                        .iter()
                        .filter(|(_, fpos)| reach.binary_search(fpos).is_ok())
                        .count()
                })
                .sum()
        };
        let envelopes = self.base.enemy_attack_envelopes(g, pid);
        let exposure = |pos: &Pos| {
            envelopes
                .iter()
                .filter(|(_, reach)| reach.contains(pos))
                .count()
        };
        let mut best: Option<Stand> = None;
        for stand in stands {
            let survived = if stand == here {
                reaches(g)
            } else {
                let mut world = g.speculative_clone();
                world.relocate(uid, stand);
                reaches(&world)
            };
            let key = (
                survived,
                exposure(&stand),
                Reverse(Self::friends_beside(g, pid, uid, stand)),
                Reverse((g.tile_defense_bonus(stand) * 10.0).round() as i64),
                stand != here,
                stand,
            );
            if best.as_ref().is_none_or(|current| key < *current) {
                best = Some(key);
            }
        }
        let (survived, _, _, _, _, stand) = best?;
        if survived >= baseline {
            return None;
        }
        let removed = baseline - survived;
        if stand == here {
            let acted = self.base.fortify_or_stop(g, pid, uid);
            if self.journal().wants(crate::reasoning::Level::Detail) {
                think!(self.journal(), Military, Detail,
                       "Holds the screen at {here:?}";
                       "its zone of control keeps {removed} enemy reach(es) off {} friend(s) \
                        that {survived} still cover; leaving would open them",
                       friends.len();
                       here);
            }
            return Some(acted);
        }
        let path = reach.get(&stand)?.1.clone();
        let arrived = self.walk_path(g, pid, uid, &path);
        let moved = g.units.get(&uid).is_some_and(|unit| unit.pos != here);
        if !moved {
            return None;
        }
        self.force_groups_dirty = true;
        if self.journal().wants(crate::reasoning::Level::Detail) {
            think!(self.journal(), Military, Detail,
                   "Screens {} friend(s) from {stand:?}", friends.len();
                   "standing there takes {removed} enemy reach(es) off them ({survived} remain); \
                    a body that steps beside us stops{}",
                   if arrived { "" } else { " — blocked part-way, holding the ground gained" };
                   stand);
        }
        Some(true)
    }

    // ------------------------------------------------------------------
    // flip-nearby-city-states
    // ------------------------------------------------------------------

    /// The gene: what a city-state's *place* is worth to the envoy scorer —
    /// proximity to our cities, and the sitting suzerain we would unseat —
    /// amortised over the envoys the suzerainty still needs. Zero for a
    /// city-state we already hold securely, so a border client past its
    /// contest does not soak up envoys.
    pub(super) fn flip_nearby_city_state_bonus(
        &self,
        g: &Game,
        pid: usize,
        minor: usize,
        needed: i64,
    ) -> i64 {
        if !self.flip_nearby_city_states {
            return 0;
        }
        let Some(seat) = g
            .player_city_ids(minor)
            .into_iter()
            .next()
            .map(|cid| g.cities[&cid].pos)
        else {
            return 0;
        };
        let Some(near) = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.wdist(g.cities[&cid].pos, seat))
            .min()
        else {
            return 0;
        };
        if near > FLIP_RADIUS {
            return 0;
        }
        let holder = g.suzerain_of(minor);
        if holder == Some(pid) {
            let mine = g.envoys_at(pid, minor);
            let rival = g
                .players
                .iter()
                .filter(|p| !p.is_minor && !p.is_barbarian && p.id != pid)
                .map(|p| g.envoys_at(p.id, minor))
                .max()
                .unwrap_or(0);
            if mine > rival + 1 {
                return 0;
            }
        }
        let proximity = i64::from(FLIP_RADIUS + 1 - near) * FLIP_PER_TILE;
        let flip = match holder {
            Some(leader) if leader != pid && g.is_at_war(pid, leader) => FLIP_ENEMY_SUZERAIN,
            Some(leader) if leader != pid => FLIP_RIVAL_SUZERAIN,
            _ => 0,
        };
        (proximity + flip) / needed.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::super::genes::GENES;
    use super::super::AdvancedAi;
    use super::*;
    use crate::game::{Action, Game};
    use crate::name;

    fn opt_in_off_in_both_controllers(tag: &str, read: fn(&AdvancedAi) -> bool) {
        assert!(!read(&AdvancedAi::new()), "{tag} must be off in new()");
        assert!(
            !read(&AdvancedAi::legacy()),
            "{tag} must be off in legacy()"
        );
        let gene = GENES
            .iter()
            .find(|gene| gene.tag == tag)
            .expect("the gene is published for gene_screen");
        assert!(gene.opt_in() && gene.screenable() && !gene.live());
        let mut ai = AdvancedAi::new();
        (gene.enable)(&mut ai);
        assert!(read(&ai));
        (gene.disable)(&mut ai);
        assert!(!read(&ai));
    }

    #[test]
    fn shoot_and_scoot_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("shoot-and-scoot", |ai| ai.shoot_and_scoot);
    }

    #[test]
    fn zoc_screen_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("zoc-screen", |ai| ai.zoc_screen);
    }

    #[test]
    fn pillage_to_heal_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("pillage-to-heal", |ai| ai.pillage_to_heal);
    }

    #[test]
    fn flip_nearby_city_states_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("flip-nearby-city-states", |ai| ai.flip_nearby_city_states);
    }

    /// A flat two-major board at war with every starting unit cleared, the
    /// map explored, no terrain anywhere — so reach, zone of control and the
    /// pillage are the only facts on the board. Returns the game and a patch
    /// of open neutral ground far from both capitals.
    fn open_field(seed: u64) -> (Game, Pos) {
        let mut game = Game::new_full(2, 28, 18, seed, 1_000, 0, false);
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .expect("each fixture major starts with a settler");
            game.found_city_for(pid, game.units[&settler].pos, None);
        }
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        game.barb_camps.clear();
        game.barb_naval_camps.clear();
        for tile in game.map.tiles.values_mut() {
            tile.terrain = name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
        }
        game.players[0].met.insert(1);
        game.players[1].met.insert(0);
        for pid in 0..2 {
            game.players[pid]
                .explored
                .extend(game.map.tiles.keys().copied());
        }
        game.at_war.insert((0, 1));
        game.turn = 60;
        game.current = 0;
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        let rival = game.cities[&game.player_city_ids(1)[0]].pos;
        let field = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|position| {
                game.wdist(*position, home) >= 8
                    && game.wdist(*position, rival) >= 8
                    && game.map.tiles[position].owner_city.is_none()
                    && game.wdisk(*position, 5).len() == game.wdisk(home, 5).len()
            })
            .min()
            .expect("the fixture board has open neutral ground");
        (game, field)
    }

    /// Start of turn for one unit: full movement, nothing spent.
    fn fresh(game: &mut Game, uid: u32) {
        let moves = game.unit_max_moves(uid);
        let unit = game.units.get_mut(&uid).unwrap();
        unit.moves_left = moves;
        unit.attacks_left = 1;
        unit.moved = false;
        unit.acted = false;
    }

    /// A firing position a human takes: the enemy body `enemy`, our archer
    /// `beside` it, the tile `behind` one step back at range two, and a
    /// friendly melee `screen` whose zone covers both tiles the body would
    /// have to cross to reach `behind`. Returned as
    /// `(enemy, beside, behind, screen)`; every tile is open ground.
    fn shooters_line(game: &Game, enemy: Pos) -> (Pos, Pos, Pos) {
        let open = |position: Pos| game.city_at(position).is_none();
        for beside in game.nbrs(enemy) {
            if !open(beside) {
                continue;
            }
            for behind in game.wring(enemy, 2) {
                if !open(behind) || game.wdist(behind, beside) != 1 {
                    continue;
                }
                // The tiles adjacent to both the body and `behind`: every
                // one-step approach the body has.
                let approaches: Vec<Pos> = game
                    .nbrs(enemy)
                    .into_iter()
                    .filter(|tile| game.wdist(*tile, behind) == 1)
                    .collect();
                for screen in game.nbrs(enemy) {
                    if !open(screen)
                        || screen == beside
                        || approaches.contains(&screen)
                        || !approaches.iter().all(|tile| game.wdist(*tile, screen) == 1)
                    {
                        continue;
                    }
                    return (beside, behind, screen);
                }
            }
        }
        panic!("the open field has a shooters' line around {enemy:?}");
    }

    /// ★★★★ THE TWO ENGINE FACTS THE GENE SPENDS, read off `attack_reach`
    /// and not off distance. A shooter exerts no zone of control, so a lone
    /// archer one step back from a Warrior is still inside its reach; put a
    /// melee friend's zone over the approaches and the same tile is not.
    #[test]
    fn a_lone_archer_one_step_back_is_still_reached_and_a_screened_one_is_not() {
        let (mut game, field) = open_field(80_101);
        let warrior = game.spawn_test_unit("warrior", 1, field);
        let (beside, behind, screen) = shooters_line(&game, field);
        let archer = game.spawn_test_unit("archer", 0, beside);
        let reach = game.attack_reach(warrior);
        assert!(
            reach.binary_search(&beside).is_ok(),
            "the body reaches the adjacent archer"
        );
        game.relocate(archer, behind);
        let reach = game.attack_reach(warrior);
        assert!(
            reach.binary_search(&behind).is_ok(),
            "a lone archer one step back exerts no zone: the body walks up and swings"
        );
        let _friend = game.spawn_test_unit("warrior", 0, screen);
        let reach = game.attack_reach(warrior);
        assert!(
            reach.binary_search(&behind).is_err(),
            "behind a friend's zone the same tile is out of reach: {reach:?}"
        );
    }

    /// The gene: an archer beside a hostile Warrior, with a friend whose
    /// zone covers the approaches, steps back to range and shoots; the
    /// Warrior can then no longer reach it. Off, nothing moves. And in the
    /// open field with no friend, the step is correctly declined — there is
    /// no tile inside fewer envelopes to go to.
    #[test]
    fn an_archer_beside_a_warrior_scoots_behind_the_screen_and_shoots() {
        let (mut game, field) = open_field(80_102);
        let warrior = game.spawn_test_unit("warrior", 1, field);
        let (beside, behind, screen) = shooters_line(&game, field);
        let archer = game.spawn_test_unit("archer", 0, beside);
        fresh(&mut game, archer);
        let warrior_hp = game.units[&warrior].hp;

        let mut scooting = AdvancedAi::new();
        scooting.enable_shoot_and_scoot();
        assert_eq!(
            scooting.shoot_and_scoot_step(&mut game, 0, archer, &[1], None, false),
            None,
            "alone in the open there is no safer firing tile"
        );
        assert_eq!(game.units[&archer].pos, beside);

        let _friend = game.spawn_test_unit("warrior", 0, screen);
        let mut stock = AdvancedAi::new();
        assert_eq!(
            stock.shoot_and_scoot_step(&mut game, 0, archer, &[1], None, false),
            None
        );
        assert_eq!(game.units[&archer].pos, beside);
        assert_eq!(game.units[&warrior].hp, warrior_hp);

        // A siege target the step is not allowed to leave the range of:
        // the far side of the body is out of range from `behind`, so the
        // scoot is declined rather than taken at the walls' expense.
        let walls = game
            .wring(field, 2)
            .into_iter()
            .filter(|position| {
                game.wdist(*position, behind) > 2 && game.wdist(*position, beside) <= 2
            })
            .min()
            .expect("a tile in range from beside and out of range from behind");
        assert_eq!(
            scooting.shoot_and_scoot_step(&mut game, 0, archer, &[1], Some(walls), false),
            None,
            "the step keeps the siege target in range or does not happen"
        );
        assert_eq!(game.units[&archer].pos, beside);

        // The war branch's shape: choose the ground, leave the shot to the
        // attack scan next pass — the body is still a legal shot from there.
        assert_eq!(
            scooting.shoot_and_scoot_step(&mut game, 0, archer, &[1], None, false),
            Some(true)
        );
        let now = game.units[&archer].pos;
        assert_eq!(now, behind, "the archer stands one step back, at its range");
        assert_eq!(
            game.units[&warrior].hp, warrior_hp,
            "the scan owns the shot"
        );
        assert!(
            game.units[&archer].moves_left > 0.0,
            "with movement left to fire"
        );
        assert!(
            game.attack_reach(warrior).binary_search(&now).is_err(),
            "the body can no longer reach the archer next turn"
        );
        game.apply(
            0,
            &Action::Ranged {
                unit: archer,
                target: field,
            },
        )
        .expect("the body is a legal shot from the new tile");
        assert!(game.units[&warrior].hp < warrior_hp);
        assert_eq!(game.units[&archer].hp, 100, "a shot pays no counter");
    }

    /// Against the barbarian seat the same step runs from the peacetime
    /// path: an archer beside a raider, with a warrior's zone over the
    /// approaches, steps back and shoots.
    #[test]
    fn an_archer_beside_a_barbarian_raider_scoots_and_shoots_in_peacetime() {
        let mut game = Game::new_full(2, 30, 20, 80_103, 200, 0, true);
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.current = pid;
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        game.barb_camps.clear();
        for tile in game.map.tiles.values_mut() {
            tile.terrain = name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
        }
        game.players[0]
            .explored
            .extend(game.map.tiles.keys().copied());
        game.current = 0;
        let barb = game.barb_pid.unwrap();
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        let field = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|position| {
                game.wdist(*position, home) == 5
                    && game.wdisk(*position, 3).len() == game.wdisk(home, 3).len()
                    && game
                        .wdisk(*position, 3)
                        .into_iter()
                        .all(|tile| game.city_at(tile).is_none())
            })
            .min()
            .unwrap();
        let raider = game.spawn_test_unit("warrior", barb, field);
        let (beside, behind, screen) = shooters_line(&game, field);
        let archer = game.spawn_test_unit("archer", 0, beside);
        let _friend = game.spawn_test_unit("warrior", 0, screen);
        fresh(&mut game, archer);
        let raider_hp = game.units[&raider].hp;
        let mut scooting = AdvancedAi::new();
        scooting.enable_shoot_and_scoot();
        assert_eq!(
            scooting.shoot_and_scoot_step(&mut game, 0, archer, &[barb], None, true),
            Some(true)
        );
        assert_eq!(
            game.units[&archer].pos, behind,
            "the archer left the raider's side"
        );
        assert!(game.units[&raider].hp < raider_hp, "and shot it");
        assert!(game.attack_reach(raider).binary_search(&behind).is_err());
    }

    /// The screen: our archer stands two tiles from a hostile Warrior in a
    /// straight line, inside its reach, with our own Warrior one tile behind
    /// the archer doing nothing. Stepping to a flank tile beside the archer
    /// puts the body's one approach inside the guard's zone — read off
    /// `attack_reach` on the real board after the move — and the guard then
    /// holds that stand while it is load-bearing.
    #[test]
    fn a_warrior_stands_where_its_zone_of_control_covers_the_archer() {
        let (mut game, field) = open_field(80_104);
        let open = |game: &Game, position: Pos| game.city_at(position).is_none();
        let enemy = game.spawn_test_unit("warrior", 1, field);
        // Straight line: exactly one tile is adjacent to both the body and
        // the archer, so one zone over it is a screen.
        let archer_pos = game
            .wring(field, 2)
            .into_iter()
            .filter(|position| {
                open(&game, *position)
                    && game
                        .nbrs(field)
                        .into_iter()
                        .filter(|tile| game.wdist(*tile, *position) == 1)
                        .count()
                        == 1
            })
            .min()
            .expect("a tile two away in a straight line");
        let _archer = game.spawn_test_unit("archer", 0, archer_pos);
        let guard_pos = game
            .nbrs(archer_pos)
            .into_iter()
            .filter(|position| game.wdist(*position, field) == 3 && open(&game, *position))
            .min()
            .expect("a tile behind the archer, three from the enemy");
        let guard = game.spawn_test_unit("warrior", 0, guard_pos);
        fresh(&mut game, guard);
        assert!(
            game.attack_reach(enemy).binary_search(&archer_pos).is_ok(),
            "before the screen the body reaches the archer"
        );
        // The board says a screening stand exists within the guard's walk.
        let screening_stands: Vec<Pos> = game
            .reachable(guard)
            .into_iter()
            .filter(|stand| game.wdist(*stand, archer_pos) == 1)
            .filter(|stand| {
                let mut world = game.speculative_clone();
                world.relocate(guard, *stand);
                world
                    .attack_reach(enemy)
                    .binary_search(&archer_pos)
                    .is_err()
            })
            .collect();
        assert!(
            !screening_stands.is_empty(),
            "some reachable tile beside the archer screens it"
        );

        let mut stock = AdvancedAi::new();
        assert_eq!(stock.zoc_screen_step(&mut game, 0, guard, &[1]), None);
        assert_eq!(game.units[&guard].pos, guard_pos);

        let mut screening = AdvancedAi::new();
        screening.enable_zoc_screen();
        assert_eq!(
            screening.zoc_screen_step(&mut game, 0, guard, &[1]),
            Some(true)
        );
        let stand = game.units[&guard].pos;
        assert!(
            screening_stands.contains(&stand),
            "the guard took a screening stand: {stand:?}"
        );
        assert!(
            game.attack_reach(enemy).binary_search(&archer_pos).is_err(),
            "and the body can no longer reach the archer: it stops in the guard's zone"
        );
        // Standing there is load-bearing, so the next pass holds the stand
        // rather than marching off it.
        fresh(&mut game, guard);
        assert_eq!(
            screening.zoc_screen_step(&mut game, 0, guard, &[1]),
            Some(false)
        );
        assert_eq!(game.units[&guard].pos, stand);
    }

    /// No friend under threat, no screen: the gene answers `None` and the
    /// ordinary march is untouched.
    #[test]
    fn a_warrior_with_no_threatened_friend_does_not_screen() {
        let (mut game, field) = open_field(80_105);
        let _enemy = game.spawn_test_unit("warrior", 1, field);
        let far = game
            .wring(field, 6)
            .into_iter()
            .filter(|position| game.city_at(*position).is_none())
            .min()
            .unwrap();
        let _archer = game.spawn_test_unit("archer", 0, far);
        let guard_pos = game
            .nbrs(far)
            .into_iter()
            .find(|position| game.city_at(*position).is_none())
            .unwrap();
        let guard = game.spawn_test_unit("warrior", 0, guard_pos);
        fresh(&mut game, guard);
        let mut screening = AdvancedAi::new();
        screening.enable_zoc_screen();
        assert_eq!(screening.zoc_screen_step(&mut game, 0, guard, &[1]), None);
    }

    /// A wounded warrior on an enemy Farm pillages it and heals fifty; one
    /// standing beside the Farm steps onto it and pillages it. Off, neither
    /// happens and the Farm stands.
    #[test]
    fn a_wounded_unit_pillages_the_farm_under_it_to_heal() {
        let (mut game, _) = open_field(80_106);
        let rival_city = game.player_city_ids(1)[0];
        let rival = game.cities[&rival_city].pos;
        let farm = game
            .wring(rival, 1)
            .into_iter()
            .filter(|position| {
                game.map.tiles[position].owner_city == Some(rival_city)
                    && game.city_at(*position).is_none()
                    && game.map.tiles[position].district.is_none()
            })
            .min()
            .expect("the rival capital owns its first ring");
        game.map.tiles.get_mut(&farm).unwrap().improvement = Some(name!("farm"));
        assert!(
            game.pillageable_at(0, farm),
            "an enemy farm at war is pillageable"
        );
        let warrior = game.spawn_test_unit("warrior", 0, farm);
        game.units.get_mut(&warrior).unwrap().hp = 40;
        fresh(&mut game, warrior);

        let mut stock = AdvancedAi::new();
        assert_eq!(stock.pillage_to_heal_step(&mut game, 0, warrior), None);
        assert_eq!(game.units[&warrior].hp, 40);
        assert!(!game.map.tiles[&farm].pillaged);

        let mut healing = AdvancedAi::new();
        healing.enable_pillage_to_heal();
        assert_eq!(
            healing.pillage_to_heal_step(&mut game, 0, warrior),
            Some(true)
        );
        assert_eq!(
            game.units[&warrior].hp, 90,
            "the farm heals fifty on the spot"
        );
        assert!(game.map.tiles[&farm].pillaged);

        // A healthy unit leaves the farm to the income genes.
        let farm_2 = game
            .wring(rival, 1)
            .into_iter()
            .filter(|position| {
                *position != farm
                    && game.map.tiles[position].owner_city == Some(rival_city)
                    && game.city_at(*position).is_none()
                    && game.map.tiles[position].district.is_none()
            })
            .min()
            .unwrap();
        game.map.tiles.get_mut(&farm_2).unwrap().improvement = Some(name!("farm"));
        let healthy = game.spawn_test_unit("warrior", 0, farm_2);
        fresh(&mut game, healthy);
        assert_eq!(healing.pillage_to_heal_step(&mut game, 0, healthy), None);
        assert!(!game.map.tiles[&farm_2].pillaged);
    }

    #[test]
    fn a_wounded_unit_beside_the_farm_steps_onto_it_and_pillages() {
        let (mut game, _) = open_field(80_107);
        let rival_city = game.player_city_ids(1)[0];
        let rival = game.cities[&rival_city].pos;
        let farm = game
            .wring(rival, 1)
            .into_iter()
            .filter(|position| {
                game.map.tiles[position].owner_city == Some(rival_city)
                    && game.city_at(*position).is_none()
                    && game.map.tiles[position].district.is_none()
            })
            .min()
            .unwrap();
        game.map.tiles.get_mut(&farm).unwrap().improvement = Some(name!("farm"));
        let beside = game
            .nbrs(farm)
            .into_iter()
            .filter(|position| {
                game.wdist(*position, rival) == 2 && game.city_at(*position).is_none()
            })
            .min()
            .unwrap();
        let warrior = game.spawn_test_unit("warrior", 0, beside);
        game.units.get_mut(&warrior).unwrap().hp = 30;
        fresh(&mut game, warrior);
        let mut healing = AdvancedAi::new();
        healing.enable_pillage_to_heal();
        assert_eq!(
            healing.pillage_to_heal_step(&mut game, 0, warrior),
            Some(true)
        );
        assert_eq!(game.units[&warrior].pos, farm);
        assert_eq!(game.units[&warrior].hp, 80);
        assert!(game.map.tiles[&farm].pillaged);
    }

    /// The envoy term: a city-state on our border under an enemy's envoys
    /// outranks the same city-state at peace, which outranks one nobody
    /// holds, and a city-state beyond the radius is worth nothing. Off, all
    /// of it is zero.
    #[test]
    fn a_nearby_city_state_under_the_enemy_is_worth_flipping() {
        let mut game = Game::new_full(2, 40, 24, 80_108, 300, 2, false);
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.found_city_for(pid, game.units[&settler].pos, None);
        }
        let minors: Vec<usize> = game
            .players
            .iter()
            .filter(|p| p.is_minor && !p.is_barbarian)
            .map(|p| p.id)
            .collect();
        assert_eq!(minors.len(), 2);
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        // Seat the two city-states by hand: one four tiles from our
        // capital, one far beyond the radius.
        for (minor, distance) in minors.iter().zip([4, FLIP_RADIUS + 6]) {
            for cid in game.player_city_ids(*minor) {
                game.cities.remove(&cid);
            }
            for unit in game.player_unit_ids(*minor) {
                game.remove_unit(unit);
            }
            let seat = game
                .map
                .tiles
                .keys()
                .copied()
                .filter(|position| {
                    game.wdist(*position, home) == distance
                        && game.rules.is_passable(&game.map.tiles[position])
                        && !game.rules.is_water(&game.map.tiles[position])
                        && game.map.tiles[position].owner_city.is_none()
                        && game.city_at(*position).is_none()
                })
                .min()
                .expect("open ground at the distance");
            game.found_city_for(*minor, seat, None);
        }
        let (near, far) = (minors[0], minors[1]);
        let mut stock = AdvancedAi::new();
        assert_eq!(stock.flip_nearby_city_state_bonus(&game, 0, near, 2), 0);
        let mut flipping = AdvancedAi::new();
        flipping.enable_flip_nearby_city_states();
        let unheld = flipping.flip_nearby_city_state_bonus(&game, 0, near, 2);
        assert!(unheld > 0, "a border city-state is worth envoys: {unheld}");
        assert_eq!(flipping.flip_nearby_city_state_bonus(&game, 0, far, 2), 0);

        // The rival takes the suzerainty at peace, then at war.
        game.players[1].envoys_free = 3;
        game.players[1].met.insert(near);
        game.players[near].met.insert(1);
        for _ in 0..3 {
            game.current = 1;
            game.apply(1, &Action::SendEnvoy { player: near })
                .expect("the rival places an envoy");
        }
        game.current = 0;
        assert_eq!(game.suzerain_of(near), Some(1));
        let rival_held = flipping.flip_nearby_city_state_bonus(&game, 0, near, 2);
        assert!(rival_held > unheld, "{rival_held} > {unheld}");
        game.at_war.insert((0, 1));
        let enemy_held = flipping.flip_nearby_city_state_bonus(&game, 0, near, 2);
        assert!(enemy_held > rival_held, "{enemy_held} > {rival_held}");
        // Amortised: the same flip four envoys away is worth half as much
        // per envoy as two away.
        assert_eq!(
            flipping.flip_nearby_city_state_bonus(&game, 0, near, 4),
            enemy_held * 2 / 4
        );
        stock.disable_flip_nearby_city_states();
    }
}
