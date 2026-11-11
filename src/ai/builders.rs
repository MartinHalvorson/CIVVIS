//! Builders: what a Builder improves next, and whether a city should train
//! another.
//!
//! `builder_step` plays a Builder's turn, and `best_improvement` chooses what
//! it lays. The backlog is what is left to do (`has_builder_work`,
//! `unimproved_worked_tiles`), set against the charges already covering it
//! (`builder_charge_cover`), which decides whether a city trains the next one
//! (`worked_backlog_builder`, `worked_backlog_claim_turns`).
//!
//! Moved verbatim out of the `impl BasicAi` block in `ai.rs` (2026-11-06).
//! Methods the rest of `ai` calls are `pub(super)`, the same reach a private
//! method of `ai` had; the code is otherwise unchanged.

use super::*;

impl BasicAi {
    /// Whether this empire has any tile left for a Builder to work on: an
    /// improvement to lay or a pillaged one to repair. Builders were produced
    /// to a flat quota per city regardless, so an empire that had improved
    /// everything it owned kept paying for Builders that then stood on a tile
    /// for the rest of the game - the audit counted nearly two hundred of
    /// them across six games, some idle from turn 25 to the end.
    pub(crate) fn has_builder_work(g: &Game, pid: usize) -> bool {
        // Every owned tile asks the same empire-wide questions of the same
        // empire; hold a memo scope over the whole sweep.
        let _memo = g.query_memo();
        g.player_city_ids(pid).into_iter().any(|cid| {
            g.cities[&cid].owned_tiles.iter().any(|pos| {
                let repairable = g
                    .map
                    .get(*pos)
                    .is_some_and(|tile| tile.pillaged && tile.improvement.is_some());
                repairable
                    || g.valid_improvements(pid, *pos)
                        .iter()
                        .any(|improvement| g.rules.improvements[improvement].builder_buildable)
            })
        })
    }

    /// Worked tiles across the empire that stand unimproved (or pillaged)
    /// and could take a Builder's improvement. See
    /// `builder_before_the_army_3`.
    pub(crate) fn unimproved_worked_tiles(g: &Game, pid: usize) -> usize {
        g.unimproved_worked_tiles(pid)
    }

    /// [`BasicAi::unimproved_worked_tiles`] for one city.
    pub(crate) fn unimproved_worked_tiles_in(g: &Game, pid: usize, cid: u32) -> usize {
        g.city_unimproved_worked_tiles(pid, cid)
    }

    /// Builder charges `pid` holds on Builders in the field, plus the charges
    /// each Builder its cities have queued will carry.
    pub(crate) fn builder_charge_cover(g: &Game, pid: usize) -> i32 {
        let in_hand: i32 = g
            .player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| g.units.get(&uid))
            .filter(|unit| unit.kind.as_str() == "builder")
            .map(|unit| unit.charges.max(0))
            .sum();
        let queued = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                matches!(
                    g.cities[cid].queue.first(),
                    Some(Item::Unit { unit }) if unit == "builder"
                )
            })
            .count() as i32;
        in_hand + queued * g.builder_charges(pid).max(1)
    }

    /// See `builders_cover_the_worked_backlog`: a backlog at least this many
    /// times the charges in hand and queued is running away from them, and
    /// the Builder may take up to `WORKED_BACKLOG_RUNAWAY_MAX_TURNS`.
    pub(super) const WORKED_BACKLOG_RUNAWAY_RATIO: f64 = 2.0;
    /// See `WORKED_BACKLOG_RUNAWAY_RATIO`, in standard turns. Live G383
    /// (civvis-20261008T102717Z) left Builders to cities of 3-4 Production
    /// whose Builder took 7-10 Online turns against the six of
    /// `WORKED_BACKLOG_BUILDER_MAX_TURNS`, while the backlog grew from 11
    /// worked tiles at t63 to 25 at t97 against 0-8 charges.
    pub(super) const WORKED_BACKLOG_RUNAWAY_MAX_TURNS: u32 = 15;

    /// See `builders_cover_the_worked_backlog`: the backlog, the cover and the
    /// turns when `cid` should train a Builder for the unimproved worked
    /// tiles: the charges in hand and queued cover less than
    /// `WORKED_BACKLOG_COVER` of them, at most one Builder per city, from a
    /// city that finishes one within `WORKED_BACKLOG_BUILDER_MAX_TURNS`
    /// (`WORKED_BACKLOG_RUNAWAY_MAX_TURNS` while the backlog runs away).
    pub(super) fn worked_backlog_builder(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        builders: usize,
    ) -> Option<(usize, i32, f64)> {
        if n_cities < 2 || builders >= n_cities {
            return None;
        }
        let city = &g.cities[&cid];
        let threatened = self.plan_threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4);
        if threatened {
            return None;
        }
        let backlog = Self::unimproved_worked_tiles(g, pid);
        // `prod-tiles-call-the-builders`: the quota in charges. Off, it is
        // `WORKED_BACKLOG_COVER` of `backlog`, and the gates below are the
        // stock ones. See `ai/prod_tiles_call_the_builders.rs`.
        let (wanted, production) = self.worked_backlog_charges_wanted(g, pid, backlog);
        if production.is_none() && backlog < Self::WORKED_BACKLOG_MIN_TILES {
            return None;
        }
        if production.is_some()
            && wanted < Self::WORKED_BACKLOG_MIN_TILES as f64 * Self::WORKED_BACKLOG_COVER
        {
            return None;
        }
        let cover = Self::builder_charge_cover(g, pid);
        if production.is_none() && f64::from(cover) >= backlog as f64 * Self::WORKED_BACKLOG_COVER {
            return None;
        }
        if production.is_some() && f64::from(cover) >= wanted {
            return None;
        }
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        if !g.can_produce(pid, cid, &builder) {
            return None;
        }
        // On, the runaway reading compares the quota with the cover in the
        // same charge units the stock rule uses: a backlog twice the cover is
        // a quota `2 x WORKED_BACKLOG_COVER` times it.
        let runaway = match production {
            None => backlog as f64 >= f64::from(cover) * Self::WORKED_BACKLOG_RUNAWAY_RATIO,
            Some(_) => {
                wanted
                    >= f64::from(cover)
                        * Self::WORKED_BACKLOG_RUNAWAY_RATIO
                        * Self::WORKED_BACKLOG_COVER
            }
        };
        let max_turns = if runaway {
            Self::WORKED_BACKLOG_RUNAWAY_MAX_TURNS
        } else {
            Self::WORKED_BACKLOG_BUILDER_MAX_TURNS
        };
        let turns = Self::unit_build_turns(g, pid, cid, "builder");
        (turns <= g.standard_duration(max_turns) as f64).then_some((backlog, cover, turns))
    }

    /// See `builders_cover_the_worked_backlog`: the turns `cid` takes to train
    /// the backlog's Builder when an idle queue is offered to it ahead of the
    /// controller's other idle-queue claims, under the same gates `pick_item`
    /// puts ahead of the step: a siege or barbarian answer, an emergency
    /// defence and a due Settler all keep the queue.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn worked_backlog_claim_turns(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        settlers: usize,
        builders: usize,
        military: usize,
    ) -> Option<f64> {
        if !self.builders_cover_the_worked_backlog || self.minor || self.barb {
            return None;
        }
        let at_major_war = g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_barbarian
                && !player.is_minor
                && g.is_at_war(pid, player.id)
        });
        let alarm =
            self.barbarian_tactics && self.barbarian_local_alarm_for_controller(g, pid, cid);
        if ((at_major_war || alarm) && military < n_cities.max(1))
            || self.besieged_city_item(g, pid, cid).is_some()
            || self.barbarian_defense_item(g, pid, cid).is_some()
            || self.settler_due(g, pid, cid, n_cities, settlers)
        {
            return None;
        }
        self.worked_backlog_builder(g, pid, cid, n_cities, builders)
            .map(|(_, _, turns)| turns)
    }

    pub(super) fn best_improvement(g: &Game, pos: Pos, options: &[Name]) -> Option<Name> {
        let resource = g.map.get(pos).and_then(|tile| tile.resource);
        options
            .iter()
            .max_by(|a, b| {
                let score = |name: &Name| {
                    let spec = &g.rules.improvements[name];
                    let works_resource = resource
                        .as_ref()
                        .is_some_and(|resource| spec.resources.iter().any(|r| r == resource));
                    let yields = spec.yields.production * 3.0
                        + spec.yields.food * 2.0
                        + spec.yields.science * 3.0
                        + spec.yields.culture * 3.0
                        + spec.yields.gold * 2.0
                        + spec.yields.faith
                        + spec.housing * 2.0;
                    (works_resource, yields)
                };
                let (a_resource, a_yield) = score(a);
                let (b_resource, b_yield) = score(b);
                a_resource
                    .cmp(&b_resource)
                    .then(a_yield.partial_cmp(&b_yield).unwrap_or(Ordering::Equal))
                    .then_with(|| b.cmp(a))
            })
            .cloned()
    }

    pub(super) fn builder_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let upos = g.units[&uid].pos;
        let project = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|city| {
                g.project_contribution_target(pid, city)
                    .map(|position| (g.wdist(upos, position), position, city))
            })
            .min();
        if let Some((_, position, city)) = project {
            if upos == position && g.can_contribute_project(pid, uid, city) {
                return g
                    .apply(pid, &Action::ContributeProject { unit: uid, city })
                    .is_ok();
            }
            if self.step_toward(g, pid, uid, position) {
                return true;
            }
        }
        let repairable = g.map.get(upos).is_some_and(|tile| {
            tile.pillaged
                && tile.improvement.is_some()
                && tile
                    .owner_city
                    .and_then(|city| g.cities.get(&city))
                    .is_some_and(|city| city.owner == pid)
        });
        if repairable {
            return g
                .apply(pid, &Action::RepairImprovement { unit: uid })
                .is_ok();
        }
        let imps: Vec<Name> = g
            .valid_improvements(pid, upos)
            .into_iter()
            .filter(|improvement| g.rules.improvements[improvement].builder_buildable)
            .collect();
        if let Some(improvement) = Self::best_improvement(g, upos, &imps) {
            return g
                .apply(
                    pid,
                    &Action::Improve {
                        unit: uid,
                        improvement: Name::new(&improvement),
                    },
                )
                .is_ok();
        }
        // Nearest-first is the right default for ordinary tiles, but an
        // unopened strategic deposit is not an ordinary tile: until one is
        // mined the empire accumulates none of the material that every modern
        // unit costs to train and to upgrade into, so those tiles are taken
        // before anything else regardless of distance.
        // ⚠ `builder_tries_the_next_tile` keeps every candidate, not only the
        // nearest, so a target that cannot be stepped toward does not end the
        // turn. See the tail of this function.
        let mut candidates: Vec<(bool, i32, Pos)> = Vec::new();
        let mut best: Option<(bool, i32, Pos)> = None;
        {
            // Read-only sweep of the whole empire's tiles: a memo scope makes
            // the empire-wide questions each tile asks cost one answer, not one
            // per tile. The borrow checker rejects the guard if anything in
            // here starts mutating.
            let _memo = g.query_memo();
            for cid in g.player_city_ids(pid) {
                for pos in g.cities[&cid].owned_tiles.clone() {
                    // ⚠⚠ A PILLAGED IMPROVEMENT IS WORK, AND THIS SWEEP COULD
                    // NOT SEE IT. Repair was handled above for the tile the
                    // Builder already stands on and nowhere else, so a razed
                    // farm across the empire was never a destination: the
                    // Builder had to wander onto it by accident. Meanwhile
                    // `has_builder_work` — the gate that decides whether to
                    // *build* a Builder — counts exactly that tile, so the
                    // empire trained Builders for work its Builders could not
                    // walk to.
                    //
                    // Counted over three 250-turn six-player games before this:
                    // Builders reached a decision with no target 3,704 times,
                    // and on 508 of those `has_builder_work` said there was
                    // work. A pillaged tile also earns nothing until it is
                    // repaired, so it is the one kind of work with a running
                    // cost.
                    let repairable = g.map.get(pos).is_some_and(|tile| {
                        tile.pillaged
                            && tile.improvement.is_some()
                            && tile
                                .owner_city
                                .and_then(|city| g.cities.get(&city))
                                .is_some_and(|city| city.owner == pid)
                    });
                    if repairable
                        || g.valid_improvements(pid, pos)
                            .iter()
                            .any(|improvement| g.rules.improvements[improvement].builder_buildable)
                    {
                        let urgent = Self::unopened_strategic_source(g, pos);
                        let d = g.wdist(upos, pos);
                        let candidate = (!urgent, d, pos);
                        if best.map(|b| candidate < b).unwrap_or(true) {
                            best = Some(candidate);
                        }
                        if self.builder_tries_the_next_tile {
                            candidates.push(candidate);
                        }
                    }
                }
            }
        }
        if !self.builder_tries_the_next_tile {
            return match best {
                Some((_, _, pos)) => self.step_toward(g, pid, uid, pos),
                None => false,
            };
        }
        // ⚠⚠ THE NEAREST TILE IS NOT THE REACHABLE ONE. `wdist` is a straight
        // line across the map; a ridge, a zone of control or a unit in the way
        // is enough to make `step_toward` refuse. Shipped, that refusal ends
        // the turn, and the sweep re-picks the same unreachable tile the next
        // turn and every turn after: measured at the deployment genome, seed
        // 21000000, builder 312 of seat 5 stood at (10, 29) for THIRTY TURNS
        // holding three charges with improvable work five tiles away on its
        // own landmass. Twenty-two builders across eight games sat still for
        // 25+ turns.
        //
        // So take the next candidate instead of the turn. Bounded: a builder
        // may ask `step_toward` at most `BUILDER_ROUTE_ATTEMPTS` times, which
        // keeps a sweep that finds a hundred tiles from becoming a hundred
        // pathfinds.
        candidates.sort_unstable();
        candidates.dedup();
        for (_, _, pos) in candidates.into_iter().take(BUILDER_ROUTE_ATTEMPTS) {
            if self.step_toward(g, pid, uid, pos) {
                return true;
            }
            // A Builder with nothing left to spend refuses every tile; that is
            // not this gene's failure, and re-asking is pure cost.
            if g.units[&uid].moves_left <= 0.0 {
                return false;
            }
        }
        false
    }

    /// Whether this tile holds a strategic deposit that is not yet connected
    /// by the improvement which harvests it.
    pub(crate) fn unopened_strategic_source(g: &Game, pos: Pos) -> bool {
        let Some(tile) = g.map.get(pos) else {
            return false;
        };
        let Some(resource) = tile.resource.as_deref() else {
            return false;
        };
        let Some(spec) = g.rules.resources.get(resource) else {
            return false;
        };
        spec.class == "strategic" && tile.improvement.as_deref() != Some(spec.improvement.as_str())
    }
}
