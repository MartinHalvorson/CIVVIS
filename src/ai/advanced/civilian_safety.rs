//! `civilian-out-of-reach`: settlers and builders stay out of a hostile
//! reach, flee it, and are stacked with a guard when they cannot. The live
//! bridge also keeps early Settlers close to the capital that launched them.
//!
//! Opt-in (`Kind::OptIn` in `genes.rs`), ships off, priced by `gene_screen`
//! beside every other gene — see `docs/CIVILIAN_SAFETY.md`. Its own file
//! because `src/ai/advanced.rs` is the most contended file in the
//! repository.
//!
//! Three engine facts make the rule exact, and the shipped answers weak:
//!
//! - **Capture is a move.** `resolve_entered_units` transfers a Settler or
//!   Builder to whichever military unit steps onto its tile; a civilian
//!   cannot be attacked at all. The tiles a raider can *end its next move
//!   on* — `Game::threat_reach`, its full allowance, through blockers — are
//!   therefore exactly the tiles a civilian must not stand on alone.
//! - **Barbarians move after every major in the same world turn**, so a
//!   civilian left on such a tile at the end of our turn is taken before it
//!   moves again. "One turn away" means "inside `threat_reach` right now".
//! - **A stacked civilian cannot be captured**: `can_enter_past` refuses the
//!   raider a tile holding a friendly military unit. A guard on the tile is
//!   protection outright; a guard beside it is not.
//!
//! What ships today in a native game prices a Settler with a geometric disk
//! (`wdist ≤ moves.ceil()`, `settlement_tile_risk`) under the *military*
//! model — the capture tiers and the retreat block are gated on the
//! host-only `live_formationless_settler_shadow` — as a soft score a single
//! on-tile guard discounts under the limit, and the stacked-guard system is
//! host-only too. The Builder path (`builder-barbarian-safety`, off) already
//! reads the exact flood but credits no escort and never flees off a job.
//!
//! With this gene on, for every Settler and Builder, every turn:
//!
//! 1. **Flee first.** A civilian standing inside the reach with no guard on
//!    its tile moves before anything else — to the reachable tile outside
//!    the reach that keeps the most progress toward its goal, else the tile
//!    fewest raiders can reach and the farthest from the nearest.
//! 2. **Never step into reach.** The next route step is refused when it is
//!    inside the reach and unguarded; a safe neighbour that still makes
//!    progress is taken instead, otherwise the civilian holds where it is
//!    safe. The doorstep exception: a Settler may enter its own site inside
//!    the reach when it keeps movement to found the city this same turn —
//!    the city is there before the raider moves.
//! 3. **Summon the guard.** A threatened Settler that cannot stay out of the
//!    reach calls a healthy military unit that can reach its tile this turn,
//!    binds it, and the pair walks together: a land guard covers land and can
//!    embark with a long expedition, while a naval guard covers every
//!    water-to-water leg. The settler pulls a stacked guard along with every
//!    step, and the guard's own turn (`stacked_guard_step`) keeps it on the
//!    settler's tile.
//!    The bond is released when no raider is within
//!    `SETTLER_ESCORT_THREAT_RADIUS`, unless the Settler is on a committed
//!    long expedition, or when the settler is gone.
//!
//! Visibility follows every other civilian-risk path: raiders inside the
//! turn-start vision frame (`battlefront_visibility`), never through fog.
//! The live seat only ever exports what it sees, so this is also what the
//! bridge can act on.
//!
//! Off, every touched path is unchanged.
//!
//! **What the live seat learned (2026-08-28, twenty-four captures in ten
//! runs)** — two host-only treatments, on for the Civilization VI seat and
//! inert on a native board, so no screened gene changes under them:
//!
//! - `live-barbarian-scouts-capture`: the host's barbarian recon units DO
//!   capture (one scout took four settlers in run civvis-20260828T122324Z),
//!   so `barbarian_reach` and the other exact capture models count them.
//! - `live-settler-capture-lessons`: (1) a settler that leaves the board
//!   without a city within two tiles of where it stood was TAKEN, and every
//!   site within `SETTLER_CAPTURE_SCAR_RADIUS` of that ground is dead for
//!   every settler for `SETTLER_DEAD_SITE_AVOID_TURNS` (the same nest ate six
//!   settlers in run civvis-20260829T000643Z while each retired it for itself
//!   alone); (2) a flee with no safe tile prefers a tile holding one of our
//!   military units — a stack a raider must first break — to the least
//!   exposed bare tile, and never holds still beside a raider while a
//!   farther tile exists (run civvis-20260829T022749Z t78: "holds inside a
//!   barbarian's reach" beside a skirmisher, two tiles from a full-health
//!   archer); (3) the strongest guard that can reach the settler this turn
//!   is summoned first, not the nearest (an archer was bound over a
//!   warrior and died on the settler's tile); (4) a guard is not released
//!   while a known barbarian camp is within `SETTLER_ESCORT_THREAT_RADIUS`
//!   — "no visible hostile within 8 tiles" preceded six captures by one
//!   turn. (5) early Settlers remember their one-city capital and do not
//!   deliberately target beyond its six-tile opening corridor; an emergency
//!   flee that carries one farther out switches to a homeward return.

use super::{
    AdvancedAi, SETTLER_CAPTURE_SCAR_RADIUS, SETTLER_DEAD_SITE_AVOID_TURNS,
    SETTLER_ESCORT_THREAT_RADIUS, SETTLER_RETREAT_LIMIT, STACKED_GUARD_MIN_HP,
};
use crate::game::{Action, Game};
use crate::reasoning::plain;
use crate::think;
use crate::Pos;

/// Raiders farther than this many times their movement allowance from a
/// civilian cannot reach it even on roads (a road step costs at least a
/// quarter move); everything nearer is flooded exactly.
const REACH_SCAN_MOVE_MULTIPLE: f64 = 2.0;
/// Jobs and sites are priced against raiders this far around the civilian
/// — the raid leash is ten tiles and a Horseman moves four.
pub(super) const REACH_SCAN_RADIUS: i32 = 10;
/// The live Civ VI bridge conservatively treats a visible barbarian Scout as
/// able to capture any land tile within two hexes.  Its path query cannot
/// always answer for a civilian-occupied destination, so the bridge falls
/// back to this geometric floor (`Map.GetPlotDistance <= 2`).  Keep the
/// native live envelope at the same floor; otherwise the planner can send a
/// Settler onto a tile the host will refuse, leaving it exposed on its
/// current tile for the hostile phase.
const LIVE_SCOUT_CAPTURE_RADIUS: i32 = 2;
/// How long a hostile the seat has actually seen keeps counting after it walks
/// back into the fog, under `hostile-memory`.
///
/// ⚠ THE HOSTILE LIST IS VISIBLE-ONLY AND THE SETTLERS DIE TO WHAT IS NOT ON
/// IT. Twenty-eight real settler losses over eleven live King runs
/// (2026-08-29) classify as: TEN walked into a hostile that was not visible
/// anywhere within three tiles on the previous turn's state, eight were taken
/// while parked waiting for a guard, six moved inside a visible hostile's
/// reach unescorted, two were zone-of-control pinned, and two were taken by
/// Free Cities units (player 62), which this reach ignored outright. Four
/// turns is the window the losses live in: the median settler was ten turns
/// old at its capture and the destination had changed within the last eight
/// turns in nineteen of the twenty-eight, so a raider seen five turns ago has
/// usually been overtaken by the walk anyway, while one seen last turn is the
/// single best predictor the board has of what takes the settler next.
pub(super) const HOSTILE_MEMORY_TURNS: u32 = 4;

/// One known raider and the tiles it could end its next move on.
struct Raider {
    pos: Pos,
    sea: bool,
    /// Sorted for binary search.
    reach: Vec<Pos>,
}

/// Everything a known hostile raider could take next turn, around one
/// civilian.
pub(super) struct BarbarianReach {
    raiders: Vec<Raider>,
}

impl BarbarianReach {
    pub(super) fn is_empty(&self) -> bool {
        self.raiders.is_empty()
    }

    /// Could a raider stand on `pos` next turn? A ship takes a coastal land
    /// tile from the water beside it, as `barbarian_capture_reaches` prices
    /// the Builder's job.
    pub(super) fn covers(&self, g: &Game, pos: Pos) -> bool {
        self.raiders_covering(g, pos) > 0
    }

    pub(super) fn raiders_covering(&self, g: &Game, pos: Pos) -> usize {
        self.raiders
            .iter()
            .filter(|raider| {
                if !raider.sea {
                    return raider.pos == pos || raider.reach.binary_search(&pos).is_ok();
                }
                g.nbrs(pos).into_iter().any(|neighbour| {
                    g.map
                        .get(neighbour)
                        .is_some_and(|tile| g.rules.is_water(tile))
                        && raider.reach.binary_search(&neighbour).is_ok()
                })
            })
            .count()
    }

    /// Hex distance to the nearest raider, `i32::MAX` when there is none.
    pub(super) fn nearest(&self, g: &Game, pos: Pos) -> i32 {
        self.raiders
            .iter()
            .map(|raider| g.wdist(raider.pos, pos))
            .min()
            .unwrap_or(i32::MAX)
    }
}

impl AdvancedAi {
    /// The reach of every visible hostile military unit that could capture a
    /// civilian at `around`: raiders within `radius`, each flooded with its
    /// full allowance through blockers (`threat_reach`). Native/evaluator
    /// boards retain the historical barbarian-only envelope. The live seat's
    /// capture lessons widen the current-frame envelope to every at-war owner,
    /// because `resolve_entered_units` gives a civilian to whichever hostile
    /// military unit steps onto it — a major's Horseman or a Free City's
    /// Warrior is not special to the engine. Recon units are engine-managed
    /// scouts, not raiders — see `barbarian_scouts_are_scouts` — except on the
    /// live seat, whose scouts capture (`live_barbarian_scouts_capture`).
    ///
    /// ⭐ `hostile-memory` (opt-in, off) widens exactly two things about the
    /// native set, and nothing else. The live capture lessons already widen
    /// the *current* visible set to every at-war owner; this gene adds that
    /// owner rule to native/evaluator boards and remembers stale sightings.
    ///
    /// 1. **Every owner the seat is at war with**, not the Barbarian player
    ///    alone. `resolve_entered_units` hands a Settler to whichever hostile
    ///    military unit steps onto its tile — the engine has never asked who
    ///    owns that unit — so a Free Cities Warrior and a major's Horseman
    ///    take a civilian on exactly the terms a raider does. Two of the
    ///    twenty-eight live losses of 2026-08-29 were Free Cities captures
    ///    (player 62) that this envelope did not model at all.
    /// 2. **A hostile seen within [`HOSTILE_MEMORY_TURNS`] turns**, projected
    ///    from where it was last seen and padded by the turns since. Ten of
    ///    those twenty-eight losses had NO visible hostile within three tiles
    ///    on the previous turn's state: the unit that took the settler was in
    ///    the fog when the step was priced, and half the time the board had
    ///    watched it walk in there.
    ///
    /// ⚠ A remembered raider is projected with `wdisk` from its LAST SEEN
    /// tile, never `threat_reach`, which floods from the unit's true current
    /// position — fog knowledge the seat does not have and must not spend. The
    /// pad is the turns elapsed, so a raider seen three turns ago is priced as
    /// able to have walked three turns' worth of ground in any direction.
    pub(super) fn barbarian_reach(
        &self,
        g: &Game,
        pid: usize,
        around: Pos,
        radius: i32,
    ) -> BarbarianReach {
        let mut raiders = Vec::new();
        // Native/evaluator boards preserve their screened barbarian-only
        // answer. The live host has proved that the same capture rule applies
        // to any visible owner at war with the seat, even when no Barbarian
        // player is present in the mirrored unit table.
        let current_frame_includes_all_hostiles =
            self.hostile_memory || self.live_settler_capture_lessons;
        let visible = self.battlefront_visibility(g, pid);
        for unit in g.units.values() {
            if current_frame_includes_all_hostiles {
                if unit.owner == pid || !g.is_at_war(pid, unit.owner) {
                    continue;
                }
            } else if Some(unit.owner) != g.barb_pid || g.wdist(unit.pos, around) > radius {
                continue;
            }
            let seen_now = g.sees(&visible, unit.pos) && g.unit_visible_to(unit.id, pid);
            // Where the seat believes this unit stands, and how stale that
            // belief is. Off, only a unit in sight this turn counts and the
            // memory is never written, so `seen_now` is the whole gate and
            // `from` is the unit's own tile — byte for byte what shipped.
            let (from, pad) = if seen_now {
                (unit.pos, 0)
            } else {
                let Some(&(last_seen, when)) = self.hostile_last_seen.get(&unit.id) else {
                    continue;
                };
                let elapsed = g.turn.saturating_sub(when);
                if elapsed > HOSTILE_MEMORY_TURNS {
                    continue;
                }
                (last_seen, elapsed as i32)
            };
            if g.wdist(from, around) > radius + pad {
                continue;
            }
            let spec = &g.rules.units[unit.kind];
            let domain = spec.domain.as_deref();
            // A recon unit is an engine-managed scout for the BARBARIAN seat;
            // a major's or a Free City's Scout captures a civilian like
            // anything else. The owner qualification keeps the native
            // barbarian-scout exemption while the live/current-hostile frame
            // can still price a major or Free City Scout.
            let engine_managed_scout = spec.promotion_class == "recon"
                && !self.live_barbarian_scouts_capture
                && (!current_frame_includes_all_hostiles || g.players[unit.owner].is_barbarian);
            if spec.class != "military" || domain == Some("air") || engine_managed_scout {
                continue;
            }
            let farthest = (g.unit_max_moves(unit.id) * REACH_SCAN_MOVE_MULTIPLE).ceil() as i32 + 1;
            if g.wdist(from, around) > farthest.max(1) + 1 + pad {
                continue;
            }
            let is_live_scout =
                spec.promotion_class == "recon" && self.live_barbarian_scouts_capture;
            let mut reach = if seen_now {
                g.threat_reach(unit.id)
            } else {
                // The projection. `threat_reach` is unavailable by
                // construction — it starts from `units[&uid].pos`, which is
                // where the unit REALLY is, not where the seat last saw it —
                // so this is the geometric envelope the bridge already uses as
                // its own conservative floor, from the remembered tile, grown
                // by one turn of allowance for every turn since.
                let projected = (spec.moves.ceil() as i32 + pad).max(1);
                g.wdisk(from, projected)
                    .into_iter()
                    .filter(|pos| {
                        g.map
                            .get(*pos)
                            .is_some_and(|tile| g.rules.is_water(tile) == (domain == Some("sea")))
                    })
                    .collect()
            };
            if is_live_scout {
                // `threat_reach` is the exact movement flood and remains the
                // source of truth for every native/evaluator unit.  On the
                // live seat, however, the host's conservative scout guard
                // intentionally ignores terrain/path answers when the
                // destination contains a civilian.  Union only its measured
                // two-hex land floor so the native route and host refusal
                // cannot disagree on the first leg.
                reach.extend(
                    g.wdisk(from, LIVE_SCOUT_CAPTURE_RADIUS)
                        .into_iter()
                        .filter(|pos| g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile))),
                );
            } else if self.live_settler_capture_lessons && domain != Some("sea") && spec.moves > 0.0
            {
                // The host bridge has one deliberately conservative fallback
                // that the native movement flood cannot reproduce: when
                // `GetMoveToPathEx` cannot answer for a civilian-occupied
                // destination, it treats every land tile within the unit's
                // static `BaseMoves` radius as reachable.  A barbarian Horse
                // Archer exposed this at live t21: the exact flood stopped at
                // terrain, the host refused the planned escape tile as inside
                // the four-hex envelope, and the Settler was captured on its
                // current tile.  Union the same geometric land floor for the
                // live bridge only; native/evaluator boards retain the exact
                // terrain-aware answer.
                let radius = spec.moves.ceil() as i32;
                reach.extend(
                    g.wdisk(from, radius)
                        .into_iter()
                        .filter(|pos| g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile))),
                );
            }
            reach.sort_unstable();
            reach.dedup();
            raiders.push(Raider {
                pos: from,
                sea: domain == Some("sea"),
                reach,
            });
        }
        BarbarianReach { raiders }
    }

    /// A civilian on `pos` is safe from capture there: nothing can reach
    /// it, or one of our military units shares the tile, or it is inside
    /// one of our cities.
    pub(super) fn civilian_safe_at(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        pos: Pos,
        reach: &BarbarianReach,
    ) -> bool {
        !reach.covers(g, pos) || self.civilian_guarded_at(g, pid, uid, pos)
    }

    /// Whether the Settler's bound guard really protects `pos` this turn.
    ///
    /// A body physically stacked with a civilian stops a capture only while
    /// it can survive the hostile phase.  The default `settler-guard-holds`
    /// policy already gives that fact one definition: a bound, healthy guard
    /// that is not outmatched.  Do not let the earlier civilian-reach pass
    /// accidentally credit the same wounded guard as an unconditional shield
    /// and skip the retreat that definition requires.
    fn bound_guard_protects_settler_at(
        &self,
        g: &Game,
        pid: usize,
        settler: u32,
        pos: Pos,
    ) -> bool {
        let water = g.map.get(pos).is_some_and(|tile| g.rules.is_water(tile));
        self.settler_guard_ids(settler)
            .into_iter()
            .flatten()
            .any(|guard| {
                let Some(unit) = g.units.get(&guard) else {
                    return false;
                };
                let sea = g.rules.units[unit.kind].domain.as_deref() == Some("sea");
                if unit.owner != pid
                    || unit.pos != pos
                    || g.rules.units[unit.kind].class != "military"
                    || g.rules.units[unit.kind].domain.as_deref() == Some("air")
                    || (sea && !water)
                {
                    return false;
                }
                !self.settler_guard_holds_on()
                    || (unit.hp >= STACKED_GUARD_MIN_HP
                        && !self.guard_outmatched_at(
                            g,
                            pid,
                            unit,
                            pos,
                            &self.battlefront_visibility(g, pid),
                        ))
            })
    }

    fn civilian_guarded_at(&self, g: &Game, pid: usize, uid: u32, pos: Pos) -> bool {
        if g.city_at(pos)
            .is_some_and(|city| g.cities[&city].owner == pid)
        {
            return true;
        }
        // The formationless live Settler repair deliberately binds the one
        // military unit whose turn is reserved to the Settler.  Under that
        // default policy, an arbitrary bystander is not a safe excuse to
        // suppress an emergency retreat, and neither is the bound guard once
        // it has dropped below the shared survival floor.
        if g.units.get(&uid).is_some_and(|unit| unit.kind == "settler")
            && self.settler_guard_holds_on()
        {
            return self.bound_guard_protects_settler_at(g, pid, uid, pos);
        }
        g.unit_ids_at(pos).iter().any(|other| {
            *other != uid
                && g.units.get(other).is_some_and(|unit| {
                    unit.owner == pid && g.rules.units[unit.kind].class == "military"
                })
        })
    }

    /// The tile a civilian is trying to reach: its site or job, else the
    /// nearest of our cities.
    fn civilian_goal(&self, g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        let unit = &g.units[&uid];
        if unit.kind == "settler" {
            // A threat response must not flee an early Settler farther from
            // the capital just because its cached settlement site is remote.
            // The home corridor is the emergency goal once the unit has
            // already been pushed beyond it.
            if let Some(home) = self.early_settler_homeward_target(g, uid) {
                return Some(home);
            }
        }
        let planned = match unit.kind.as_str() {
            "settler" => self.settler_targets.get(&uid).copied(),
            "builder" => self.builder_targets.get(&uid).copied(),
            _ => None,
        };
        planned.or_else(|| {
            g.player_city_ids(pid)
                .into_iter()
                .map(|city| g.cities[&city].pos)
                .min_by_key(|pos| (g.wdist(*pos, unit.pos), *pos))
        })
    }

    /// Rule 1: a civilian standing inside the reach, unguarded, moves before
    /// anything else. `Some(moved)` when the rule decided the unit's turn;
    /// `None` when it stands safe (or on a site it can found right now).
    pub(super) fn civilian_flee_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        let current = g.units[&uid].pos;
        let kind = g.units[&uid].kind;
        let reach = self.barbarian_reach(g, pid, current, REACH_SCAN_RADIUS);
        if reach.is_empty() || self.civilian_safe_at(g, pid, uid, current, &reach) {
            return None;
        }
        // A settler on its own site founds: the city is protection, and it
        // is what the walk was for.
        if kind == "settler"
            && self.settler_targets.get(&uid) == Some(&current)
            && g.can_found_city(uid)
        {
            return None;
        }
        // Rule 3 before rule 1: a guard that can stand on this tile now makes
        // it safe without giving up the ground.
        if kind == "settler" && self.summon_guard_to(g, pid, uid, current) {
            return None;
        }
        let goal = self.civilian_goal(g, pid, uid);
        if self.live_settler_capture_lessons {
            let fled = self.flee_under_lessons(g, pid, uid, &reach, goal);
            if fled {
                if kind == "settler" {
                    self.note_capture_retreat(g, uid);
                }
                return Some(true);
            }
            // `flee_under_lessons` deliberately refuses an equal-risk move.
            // Let the formationless settlement-safety fallback below try a
            // one-step risk-reducing retreat when the full-turn escape has no
            // strictly better destination; returning `Some(false)` here
            // would short-circuit that fallback and leave a visible hostile
            // Galley or Horseman free to take the Settler.
            if kind == "settler" {
                if let Some(home) = self.early_settler_homeward_target(g, uid) {
                    return Some(self.return_early_settler_home(g, pid, uid, home));
                }
            }
            return None;
        }
        let here_covering = reach.raiders_covering(g, current);
        let here_nearest = reach.nearest(g, current);
        let mut options: Vec<(bool, usize, i32, i32, Pos)> = g
            .reachable(uid)
            .into_iter()
            // `reachable` contains full-turn destinations. `can_move` asks
            // whether a position is one immediate step away, so applying it
            // here silently drops the two-step safe escape this emergency
            // path exists to use. Re-check the exact route the `MoveTo`
            // order will follow instead.
            .filter(|pos| *pos != current && g.path_to(uid, *pos).is_some())
            .map(|pos| {
                let safe = self.civilian_safe_at(g, pid, uid, pos, &reach);
                let covering = reach.raiders_covering(g, pos);
                let progress = goal.map_or(0, |goal| g.wdist(pos, goal));
                (safe, covering, progress, -reach.nearest(g, pos), pos)
            })
            .collect();
        // Safe tiles first; among them the one nearest the goal, then the
        // farthest from any raider. Failing a safe tile, the fewest raiders
        // reaching it, then distance from them — and only when that beats
        // standing still.
        options.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| if a.0 { a.2.cmp(&b.2) } else { a.3.cmp(&b.3) })
                .then_with(|| a.3.cmp(&b.3))
                .then_with(|| a.4.cmp(&b.4))
        });
        for (safe, covering, _, neg_nearest, pos) in options {
            if !safe
                && (covering > here_covering
                    || (covering == here_covering && -neg_nearest <= here_nearest))
            {
                break;
            }
            if self.base.path_move(g, pid, uid, pos) {
                think!(self.journal(), Expansion, Detail, "{} retreats from a hostile's reach", plain(&kind);
                       "{} raider(s) could take {current:?} next turn; {pos:?} is {}",
                       here_covering,
                       if safe { "out of reach" } else { "the least exposed tile it can reach" }; pos);
                return Some(true);
            }
        }
        think!(self.journal(), Expansion, Detail, "{} holds inside a barbarian's reach", plain(&kind);
               "{} raider(s) could take {current:?} and no reachable tile is better", here_covering; current);
        Some(false)
    }

    /// Rule 3: bring a healthy military unit from the matching movement layer
    /// that can reach `pos` this turn and survive there, then bind it to the
    /// settler. `true` when a guard now shares the tile.
    pub(super) fn summon_guard_to(
        &mut self,
        g: &mut Game,
        pid: usize,
        settler: u32,
        pos: Pos,
    ) -> bool {
        if self.bound_guard_protects_settler_at(g, pid, settler, pos) {
            return true;
        }
        let visible = self
            .settler_guard_holds_on()
            .then(|| self.battlefront_visibility(g, pid));
        let bound = self.all_bound_settler_guards();
        let water = g.map.get(pos).is_some_and(|tile| g.rules.is_water(tile));
        let mut candidates: Vec<(i32, i32, u32)> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                Self::guard_matches_escort_layer(g, unit, water)
                    && unit.hp >= STACKED_GUARD_MIN_HP
                    && unit.linked_to.is_none()
                    && (!bound.contains(uid) || self.guard_is_bound_to_settler(settler, *uid))
                    && (!self.settler_guard_holds_on()
                        || !self.guard_outmatched_at(
                            g,
                            pid,
                            unit,
                            pos,
                            visible.as_ref().expect("computed under the flag"),
                        ))
                    && (unit.pos == pos
                        || (unit.moves_left > 0.0 && g.reachable(*uid).contains(&pos)))
            })
            .map(|uid| {
                let unit = &g.units[&uid];
                let distance = g.wdist(unit.pos, pos);
                let strength = -(g.unit_strength(unit, false) as i32);
                // Every candidate reaches the tile this turn; under the live
                // lessons the strongest of them is the guard, not the nearest
                // — run civvis-20260829T022749Z bound a 15-strength archer
                // over a warrior and lost it, then the settler, to a
                // skirmisher.
                if self.live_settler_capture_lessons {
                    (strength, distance, uid)
                } else {
                    (distance, strength, uid)
                }
            })
            .collect();
        candidates.sort_unstable();
        for (_, _, guard) in candidates {
            // A routed order, not the adjacent step `path_move` issues: the
            // guard may be several tiles out. A guard that gets under way
            // but not all the way is bound too — its own step
            // (`stacked_guard_step`) finishes the walk.
            let before = g.units[&guard].pos;
            let arrived = before == pos
                || (g
                    .apply(
                        pid,
                        &Action::MoveTo {
                            unit: guard,
                            to: pos,
                        },
                    )
                    .is_ok()
                    && g.units[&guard].pos == pos);
            let under_way = arrived || g.units[&guard].pos != before;
            if arrived || under_way {
                self.bind_settler_guard(g, settler, guard);
                self.guard_wait.remove(&settler);
                if arrived && before != pos && self.live_settler_capture_lessons {
                    self.summoned_guard_turn.insert(settler, g.turn);
                }
                think!(self.journal(), Expansion, Detail, "A guard is called to the settler";
                       "sharing the tile blocks capture outright"; pos);
                return arrived;
            }
        }
        false
    }

    /// Release a settler's guard when nothing threatens the walk any more,
    /// so the army gets its unit back.
    pub(super) fn release_guard_if_quiet(
        &mut self,
        g: &Game,
        pid: usize,
        settler: u32,
        reach: &BarbarianReach,
    ) {
        if self
            .settler_guard_ids(settler)
            .into_iter()
            .flatten()
            .next()
            .is_none()
        {
            return;
        }
        let pos = g.units[&settler].pos;
        let quiet = reach.nearest(g, pos) > SETTLER_ESCORT_THREAT_RADIUS
            && self
                .barbarian_reach(g, pid, pos, SETTLER_ESCORT_THREAT_RADIUS)
                .is_empty()
            // A known camp is a raider that has not stepped out of the fog
            // yet; see `live_settler_capture_lessons`.
            && !(self.live_settler_capture_lessons
                && g
                    .barb_camps
                    .keys()
                    .any(|camp| g.wdist(*camp, pos) <= SETTLER_ESCORT_THREAT_RADIUS))
            // Nor is the ground that took a settler quiet because nothing is
            // visible on it today: run civvis-20260829T022749Z released the
            // guard at t103 on the tile of the t78 capture and lost the next
            // settler at t104.
            && !self.scarred_ground(g, pid, pos);
        let long_expedition = self
            .settler_targets
            .get(&settler)
            .is_some_and(|target| self.long_settler_escort_active(settler, *target));
        if quiet && !long_expedition {
            self.clear_settler_guard_bindings(settler);
        }
    }

    /// The stacked guard follows the settler onto `to` when it still has
    /// the movement and can survive there; a settler never leaves a viable
    /// guard behind on purpose.  An outmatched escort is deliberately left
    /// behind so it cannot turn a safe retreat into the same exposed stack.
    fn pull_guard_along(&mut self, g: &mut Game, pid: usize, settler: u32, from: Pos, to: Pos) {
        let visible = self
            .settler_guard_holds_on()
            .then(|| self.battlefront_visibility(g, pid));
        let target_is_water = g.map.get(to).is_some_and(|tile| g.rules.is_water(tile));
        for guard in self.settler_guard_ids(settler).into_iter().flatten() {
            let can_follow = g.units.get(&guard).is_some_and(|unit| {
                unit.owner == pid
                    && unit.pos == from
                    && unit.moves_left > 0.0
                    && g.rules.units[unit.kind].domain.as_deref() != Some("air")
                    // A naval guard ends its duty at landfall; the embarked
                    // land guard is the layer that can stand beside the new
                    // city through the hostile phase.
                    && (g.rules.units[unit.kind].domain.as_deref() != Some("sea")
                        || target_is_water)
                    && (!self.settler_guard_holds_on()
                        || !self.guard_outmatched_at(
                            g,
                            pid,
                            unit,
                            to,
                            visible.as_ref().expect("computed under the flag"),
                        ))
            });
            if can_follow && g.reachable(guard).contains(&to) {
                self.base.path_move(g, pid, guard, to);
            }
        }
    }

    /// Could the bound guard, stacked on `from`, stand on `to` with the
    /// settler after this step?
    fn guard_can_follow(&self, g: &Game, pid: usize, settler: u32, from: Pos, to: Pos) -> bool {
        let visible = self
            .settler_guard_holds_on()
            .then(|| self.battlefront_visibility(g, pid));
        let target_is_water = g.map.get(to).is_some_and(|tile| g.rules.is_water(tile));
        self.settler_guard_ids(settler)
            .into_iter()
            .flatten()
            .any(|guard| {
                g.units.get(&guard).is_some_and(|unit| {
                    unit.owner == pid
                        && unit.pos == from
                        && unit.moves_left > 0.0
                        && g.rules.units[unit.kind].domain.as_deref() != Some("air")
                        && (g.rules.units[unit.kind].domain.as_deref() != Some("sea")
                            || target_is_water)
                        && g.reachable(guard).contains(&to)
                        && (!self.settler_guard_holds_on()
                            || !self.guard_outmatched_at(
                                g,
                                pid,
                                unit,
                                to,
                                visible.as_ref().expect("computed under the flag"),
                            ))
                })
            })
    }

    /// The live bridge must not let a long expedition advance through open
    /// water with only its embarked land guard. A land body cannot intercept
    /// a naval capture: the Caravel that took settler 2162704 on
    /// civvis-20260901T230916Z was hidden on the previous frame, crossed the
    /// fog in the hostile phase, killed the embarked Archer, and captured the
    /// Settler on the same water tile. Requiring the already-bound naval
    /// layer on water-to-water steps preserves the ordinary first embark and
    /// landfall behavior, while making the middle of a committed live
    /// crossing survivable against a threat the fog cannot reveal in time.
    fn live_water_step_needs_naval_guard(
        &self,
        g: &Game,
        pid: usize,
        settler: u32,
        current: Pos,
        next: Pos,
        target: Pos,
    ) -> bool {
        if !self.live_settler_capture_lessons
            || !self.long_settler_escort_active(settler, target)
            || !g
                .map
                .get(current)
                .is_some_and(|tile| g.rules.is_water(tile))
            || !g.map.get(next).is_some_and(|tile| g.rules.is_water(tile))
        {
            return false;
        }
        let visible = self
            .settler_guard_holds_on()
            .then(|| self.battlefront_visibility(g, pid));
        let Some(guard) = self.settler_sea_guards.get(&settler).copied() else {
            return true;
        };
        !self.escort_guard_can_hold(g, pid, guard, current, true, visible.as_ref())
            || !g.reachable(guard).contains(&next)
    }

    /// The ordinary safe-step scorer remains authoritative for route choice.
    /// This wrapper makes its successful move atomic with every currently
    /// stackable guard's follow order, including the quiet portion of a long
    /// expedition where the barbarian reach is empty.
    fn settler_step_toward_safe_with_guards(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> bool {
        let current = g.units[&uid].pos;
        let moved = self.settler_step_toward_safe(g, pid, uid, target);
        if moved {
            let now = g.units[&uid].pos;
            if now != current {
                self.pull_guard_along(g, pid, uid, current, now);
            }
        }
        moved
    }

    /// Rule 2 for a Settler's march: the route step is refused inside the
    /// reach unless a guard walks in with it or the site is founded this
    /// turn; a safe neighbour that still makes progress is taken instead;
    /// otherwise the settler holds on safe ground. Returns whether it moved.
    pub(super) fn settler_step_out_of_reach(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> bool {
        let current = g.units[&uid].pos;
        let reach = self.barbarian_reach(g, pid, current, REACH_SCAN_RADIUS);
        self.release_guard_if_quiet(g, pid, uid, &reach);
        if reach.is_empty() && !self.live_settler_capture_lessons {
            return self.settler_step_toward_safe_with_guards(g, pid, uid, target);
        }
        let Some(next) = g
            .route_step(uid, target, 0)
            .filter(|next| g.can_move(uid, *next))
        else {
            return self.settler_step_toward_safe_with_guards(g, pid, uid, target);
        };
        if self.live_water_step_needs_naval_guard(g, pid, uid, current, next, target) {
            think!(self.journal(), Expansion, Detail, "Settler holds for a naval escort";
                   "an embarked long expedition cannot advance from {current:?} to {next:?} \
                    without a bound naval guard that can follow the water leg"; current);
            return false;
        }
        // See `live_settler_capture_lessons`: the ground that took a settler
        // is entered only stacked, whether or not a raider is visible on it
        // today — the raiders that took the last one walked out of the fog.
        let scarred_next = self.scarred_ground(g, pid, next);
        if reach.is_empty() && !scarred_next {
            return self.settler_step_toward_safe_with_guards(g, pid, uid, target);
        }
        let founds_on_arrival =
            next == target && g.units[&uid].moves_left - g.step_cost_for(uid, current, next) > 1e-9;
        let next_safe = (self.civilian_safe_at(g, pid, uid, next, &reach) && !scarred_next)
            || self.guard_can_follow(g, pid, uid, current, next)
            || founds_on_arrival;
        if next_safe {
            return self.settler_step_toward_safe_with_guards(g, pid, uid, target);
        }
        // The route enters the reach alone: bring a guard onto this tile and
        // walk in together, else sidestep, else hold.
        if self.summon_guard_to(g, pid, uid, current) {
            // See `live_settler_capture_lessons`: the guard that just spent
            // its move reaching this tile is not ordered again this turn. The
            // mirror still credits it the movement to follow, but the host
            // lands one order per unit per frame — run civvis-20260829T040648Z
            // t43: archer called onto (23,27), settler marched on to (24,28)
            // in the same turn, the follow never landed, taken that night.
            if self.live_settler_capture_lessons
                && self.summoned_guard_turn.get(&uid) == Some(&g.turn)
            {
                think!(self.journal(), Expansion, Detail, "Settler waits with the guard it just called";
                       "the guard spent its move reaching this tile; the pair marches next turn"; current);
                return false;
            }
            if self.guard_can_follow(g, pid, uid, current, next) {
                return self.settler_step_toward_safe_with_guards(g, pid, uid, target);
            }
        }
        let current_distance = g.wdist(current, target);
        let mut sidesteps: Vec<(i32, i32, Pos)> = g
            .nbrs(current)
            .into_iter()
            .filter(|pos| {
                *pos != next
                    && g.map.get(*pos).is_some()
                    && g.can_move(uid, *pos)
                    && self.civilian_safe_at(g, pid, uid, *pos, &reach)
                    && !self.scarred_ground(g, pid, *pos)
            })
            .map(|pos| {
                (
                    g.wdist(pos, target) - current_distance,
                    -reach.nearest(g, pos),
                    pos,
                )
            })
            .filter(|(regress, _, _)| *regress <= 0)
            .collect();
        sidesteps.sort_unstable();
        for (_, _, pos) in sidesteps {
            if self.base.path_move(g, pid, uid, pos) {
                think!(self.journal(), Expansion, Detail, "Settler sidesteps a barbarian's reach";
                       "{next:?} could be taken next turn; {pos:?} keeps out of it"; pos);
                return true;
            }
        }
        if reach.is_empty() {
            think!(self.journal(), Expansion, Detail, "Settler will not cross the ground that took a settler alone";
                   "{next:?} is within three tiles of a capture; it waits for a guard to walk in with it"; current);
            return false;
        }
        think!(self.journal(), Expansion, Detail, "Settler waits outside a barbarian's reach";
               "{next:?} could be taken next turn and no safe step makes progress"; current);
        false
    }

    /// Rule 2 for a Builder: the route step is refused inside the reach; a
    /// safe neighbour that still makes progress is taken; else it holds.
    pub(super) fn builder_step_out_of_reach(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> bool {
        let current = g.units[&uid].pos;
        let reach = self.barbarian_reach(g, pid, current, REACH_SCAN_RADIUS);
        if reach.is_empty() {
            return self.builder_step_toward_barbarian_safe(g, pid, uid, target);
        }
        let Some(next) = g
            .route_step(uid, target, 0)
            .filter(|next| g.can_move(uid, *next))
        else {
            return self.builder_step_toward_barbarian_safe(g, pid, uid, target);
        };
        if self.civilian_safe_at(g, pid, uid, next, &reach) {
            return self.builder_step_toward_barbarian_safe(g, pid, uid, target);
        }
        let current_distance = g.wdist(current, target);
        let mut sidesteps: Vec<(i32, i32, Pos)> = g
            .nbrs(current)
            .into_iter()
            .filter(|pos| {
                *pos != next
                    && g.map.get(*pos).is_some()
                    && g.can_move(uid, *pos)
                    && self.civilian_safe_at(g, pid, uid, *pos, &reach)
            })
            .map(|pos| {
                (
                    g.wdist(pos, target) - current_distance,
                    -reach.nearest(g, pos),
                    pos,
                )
            })
            .filter(|(regress, _, _)| *regress < 0)
            .collect();
        sidesteps.sort_unstable();
        for (_, _, pos) in sidesteps {
            if self.base.path_move(g, pid, uid, pos) {
                return true;
            }
        }
        think!(self.journal(), Expansion, Detail, "Builder waits outside a barbarian's reach";
               "{next:?} could be taken next turn"; current);
        false
    }

    /// Job filter for the Builder's sweep: a job tile a raider could stand
    /// on next turn is not a job today.
    pub(super) fn builder_job_out_of_reach(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        pos: Pos,
        reach: &BarbarianReach,
    ) -> bool {
        self.civilian_safe_at(g, pid, uid, pos, reach)
    }

    /// One military unit from the matching movement layer on `pos` that a
    /// settler could bind as its guard by standing with it: healthy, unlinked, not another
    /// settler's guard. Under the live survival bar, an escort the first
    /// visible hostile would break is not a shield at all: the hostile phase
    /// can kill that body and capture the civilian in the same turn. Keep the
    /// old "any healthy stack is better than a bare civilian" answer for the
    /// native/evaluator controllers, but make the live binding agree with
    /// `bound_guard_protects_settler_at` and `settlement_tile_risk`.
    /// Ground within `SETTLER_CAPTURE_SCAR_RADIUS` of a settler's capture,
    /// still under its retirement, and not one of our own cities. Under
    /// `live_settler_capture_lessons` only (the scar map is empty otherwise).
    fn scarred_ground(&self, g: &Game, pid: usize, pos: Pos) -> bool {
        self.live_settler_capture_lessons
            && self.settler_capture_scars.contains_key(&pos)
            && !g
                .city_at(pos)
                .is_some_and(|city| g.cities[&city].owner == pid)
    }

    fn bindable_guard_at(&self, g: &Game, pid: usize, settler: u32, pos: Pos) -> Option<u32> {
        let bound = self.all_bound_settler_guards();
        let water = g.map.get(pos).is_some_and(|tile| g.rules.is_water(tile));
        let visible = self
            .settler_guard_holds_on()
            .then(|| self.battlefront_visibility(g, pid));
        g.unit_ids_at(pos)
            .iter()
            .copied()
            .filter(|uid| {
                let unit = &g.units[uid];
                unit.owner == pid
                    && Self::guard_matches_escort_layer(g, unit, water)
                    && unit.hp >= STACKED_GUARD_MIN_HP
                    && unit.linked_to.is_none()
                    && (!bound.contains(uid) || self.guard_is_bound_to_settler(settler, *uid))
                    && (!self.settler_guard_holds_on()
                        || !self.guard_outmatched_at(
                            g,
                            pid,
                            unit,
                            pos,
                            visible.as_ref().expect("computed under the flag"),
                        ))
            })
            .max_by_key(|uid| (g.unit_strength(&g.units[uid], false) as i32, *uid))
    }

    /// Keep the existing live target-retirement contract when the capture
    /// lessons take the escape branch before the formationless settlement
    /// safety pass. A hostile reach escape spends the same approach attempt
    /// as the older one-step retreat, even when the full-turn escape happens
    /// to make progress toward the target.
    fn note_capture_retreat(&mut self, g: &Game, uid: u32) {
        let Some(target) = self.settler_targets.get(&uid).copied() else {
            return;
        };
        let count = match self.settler_retreats.get(&uid) {
            Some((committed, count)) if *committed == target => count + 1,
            _ => 1,
        };
        self.settler_retreats.insert(uid, (target, count));
        if count < SETTLER_RETREAT_LIMIT {
            return;
        }
        think!(self.journal(), Expansion, Detail, "Settler gives up on {target:?}";
               "retreated {count} times while walking there; the approach is not clearing, so the \
                site is retired for {SETTLER_DEAD_SITE_AVOID_TURNS} standard turns"; target);
        self.settler_dead_sites.entry(uid).or_default().insert(
            target,
            g.turn + g.standard_duration(SETTLER_DEAD_SITE_AVOID_TURNS),
        );
        self.settler_targets.remove(&uid);
        self.settler_stalls.remove(&uid);
        self.settler_blocked_turns.remove(&uid);
        self.settler_retreats.remove(&uid);
    }

    /// Rule 1 under `live_settler_capture_lessons`, for a civilian standing
    /// inside the reach with nothing protecting its tile. Options rank: out
    /// of reach (nearest the goal), then a tile holding a bindable friendly
    /// unit, then the tile farthest from the nearest raider with the fewest
    /// raiders covering it. Standing still competes as one more option and
    /// loses to any of those that is better than here — so a settler beside
    /// a raider never holds while a farther tile or a friendly stack exists.
    /// Returns whether the unit moved.
    fn flee_under_lessons(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        reach: &BarbarianReach,
        goal: Option<Pos>,
    ) -> bool {
        let current = g.units[&uid].pos;
        let kind = g.units[&uid].kind;
        let here_covering = reach.raiders_covering(g, current);
        let here_nearest = reach.nearest(g, current);
        if kind == "settler" {
            if let Some(guard) = self.bindable_guard_at(g, pid, uid, current) {
                // Someone is already standing here: bind it and stay.
                self.bind_settler_guard(g, uid, guard);
                self.guard_wait.remove(&uid);
                think!(self.journal(), Expansion, Detail, "A guard is called to the settler";
                       "it already shares the tile; bound, its own turn keeps it here"; current);
                return false;
            }
        }
        let mut options: Vec<(bool, bool, i32, usize, i32, Pos)> = g
            .reachable(uid)
            .into_iter()
            .filter(|pos| *pos != current && g.path_to(uid, *pos).is_some())
            .map(|pos| {
                let safe = self.civilian_safe_at(g, pid, uid, pos, reach);
                let stacked = !safe
                    && kind == "settler"
                    && self.bindable_guard_at(g, pid, uid, pos).is_some();
                (
                    safe,
                    stacked,
                    reach.nearest(g, pos),
                    reach.raiders_covering(g, pos),
                    goal.map_or(0, |goal| g.wdist(pos, goal)),
                    pos,
                )
            })
            .collect();
        options.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| if a.0 { a.4.cmp(&b.4) } else { b.2.cmp(&a.2) })
                .then_with(|| a.3.cmp(&b.3))
                .then_with(|| b.2.cmp(&a.2))
                .then_with(|| a.4.cmp(&b.4))
                .then_with(|| a.5.cmp(&b.5))
        });
        for (safe, stacked, nearest, covering, _, pos) in options {
            let better = safe
                || stacked
                || nearest > here_nearest
                || (nearest == here_nearest && covering < here_covering);
            if !better {
                break;
            }
            if self.base.path_move(g, pid, uid, pos) {
                let now = g.units[&uid].pos;
                if kind == "settler" {
                    if let Some(guard) = self.bindable_guard_at(g, pid, uid, now) {
                        self.bind_settler_guard(g, uid, guard);
                        self.guard_wait.remove(&uid);
                    } else if now != current {
                        self.pull_guard_along(g, pid, uid, current, now);
                    }
                }
                let noun = if kind == "settler" {
                    "Settler"
                } else {
                    "Builder"
                };
                think!(self.journal(), Expansion, Detail, "{noun} retreats from a hostile's reach";
                       "{} raider(s) could take {current:?} next turn; {pos:?} is {}",
                       here_covering,
                       if safe {
                           "out of reach"
                       } else if stacked {
                           "a tile one of our units already holds — a stack the raider must first break"
                       } else {
                           "the least exposed tile it can reach"
                       }; pos);
                return true;
            }
        }
        think!(self.journal(), Expansion, Detail, "{} holds inside a barbarian's reach", plain(&kind);
               "{} raider(s) could take {current:?} and no reachable tile is better", here_covering; current);
        false
    }

    /// A settler that left the board since the last turn either founded a
    /// city where it stood or was taken. The first leaves one of our cities
    /// within two tiles of its last position; the second retires every site
    /// within `SETTLER_CAPTURE_SCAR_RADIUS` of that ground for EVERY settler
    /// for `SETTLER_DEAD_SITE_AVOID_TURNS` standard turns. Under
    /// `live_settler_capture_lessons` only; expired scars are dropped here.
    pub(super) fn resolve_vanished_settlers(&mut self, g: &Game, pid: usize) {
        if !self.live_settler_capture_lessons {
            return;
        }
        self.settler_capture_scars
            .retain(|_, until| *until > g.turn);
        // A native board keeps its unit ids, so a settler missing from it —
        // or carried by someone else now — vanished without any remap.
        let gone: Vec<u32> = self
            .settler_last_seen
            .keys()
            .copied()
            .filter(|uid| g.units.get(uid).is_none_or(|unit| unit.owner != pid))
            .collect();
        for uid in gone {
            if let Some(pos) = self.settler_last_seen.remove(&uid) {
                self.settler_vanished.push(pos);
            }
        }
        for pos in std::mem::take(&mut self.settler_vanished) {
            let founded = g
                .player_city_ids(pid)
                .into_iter()
                .any(|city| g.wdist(g.cities[&city].pos, pos) <= 2);
            if founded {
                continue;
            }
            let until = g.turn + g.standard_duration(SETTLER_DEAD_SITE_AVOID_TURNS);
            for tile in g.wdisk(pos, SETTLER_CAPTURE_SCAR_RADIUS) {
                let entry = self.settler_capture_scars.entry(tile).or_insert(until);
                *entry = (*entry).max(until);
            }
            think!(self.journal(), Expansion, Decision, "A settler was lost at {pos:?}";
                   "every site within {SETTLER_CAPTURE_SCAR_RADIUS} tiles is retired for \
                    {SETTLER_DEAD_SITE_AVOID_TURNS} standard turns for every settler — the ground \
                    that took one settler takes the next"; pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Action;

    fn open_land(g: &Game, pos: Pos) -> bool {
        g.city_at(pos).is_none()
            && g.unit_ids_at(pos).is_empty()
            && g.map
                .get(pos)
                .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
    }

    /// The live game can hand a Settler to any hostile military owner, not
    /// only the Barbarian seat. Keep the native/evaluator envelope frozen, but
    /// make the host lessons model the same `resolve_entered_units` rule.
    #[test]
    fn live_capture_reach_includes_an_at_war_major_without_barbarians() {
        let mut game = Game::new_full(2, 20, 14, 91_401, 60, 0, false);
        game.current = 0;
        let founding_settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| game.units[uid].kind == "settler")
            .expect("player 0 has a starting Settler");
        game.apply(
            0,
            &Action::FoundCity {
                unit: founding_settler,
            },
        )
        .expect("the starting Settler founds the capital");
        let home = game
            .cities
            .values()
            .find(|city| city.owner == 0)
            .expect("player 0 has a capital")
            .pos;
        let start = game
            .nbrs(home)
            .into_iter()
            .find(|pos| open_land(&game, *pos))
            .expect("open land beside the capital");
        let hostile_pos = game
            .nbrs(start)
            .into_iter()
            .find(|pos| open_land(&game, *pos))
            .expect("open land beside the Settler");
        let settler = game.spawn_test_unit("settler", 0, start);
        let hostile = game.spawn_test_unit("warrior", 1, hostile_pos);
        game.at_war.insert((0, 1));
        game.at_war.insert((1, 0));

        let mut native = AdvancedAi::new();
        native.enable_civilian_out_of_reach();
        assert!(
            !native
                .barbarian_reach(&game, 0, start, REACH_SCAN_RADIUS)
                .covers(&game, start),
            "native screens retain the barbarian-only envelope"
        );

        let mut live = AdvancedAi::new();
        live.enable_live_settler_capture_lessons();
        let reach = live.barbarian_reach(&game, 0, start, REACH_SCAN_RADIUS);
        assert!(
            reach.covers(&game, start),
            "the visible at-war major's Warrior can capture the Settler"
        );
        assert!(
            reach.raiders.iter().any(|raider| raider.pos == hostile_pos),
            "the major's unit is represented in the live reach"
        );

        game.at_war.clear();
        assert!(
            live.barbarian_reach(&game, 0, start, REACH_SCAN_RADIUS)
                .is_empty(),
            "a peaceful rival is not a capture threat"
        );
        let _ = settler;
        let _ = hostile;
    }
}
