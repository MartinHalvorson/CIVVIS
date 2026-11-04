//! `builderless-empire-trains-one`: an empire of two or more cities that holds
//! no Builder at all, afield or at the head of any queue, while four or more
//! of its worked tiles stand unimproved, puts one at the head of the fastest
//! safe city's queue.
//!
//! **Why (live Immortal Standard, 16 Gran Colombia games of 2026-11-04
//! 17Z-21Z; Lv 8 Deity Online read the same way).**
//! - Builders alive: a median 0 at turns 40 and 60, 1 from turn 100 on.
//!   Improved share of our owned land 0.12 / 0.18 / 0.26 at turns 40 / 60 /
//!   100 against the rivals' 0.21 / 0.35 / 0.48. Population 17 against the
//!   best rival's 36.5 at turn 100.
//! - A Builder, once trained, is not the problem: a median 3.9 Builder-turns
//!   per improvement by turn 100. Supply is.
//! - On 37% of the turns 15-90 an empire of two or more cities held no Builder
//!   at all against four or more unimproved worked tiles. The capital was
//!   training a Settler on 39% of those turns, and on the rest an Archer
//!   (11%), Walls (9%), a Warrior, a Granary, a Monument, a Campus.
//! - `backlog-builder-takes-the-queue` (armed) asks for the same Builder but
//!   stands down for a due Settler anywhere in the empire's gate, for a city
//!   the plan marks threatened and for a Builder slower than eight turns
//!   (fifteen once the backlog runs away). Its claim fired once before turn
//!   60 across the 16 games.
//! - The first full Deity Online Korea game (civvis-20261104T210826Z): our
//!   science per citizen matched the leader's once the Deity bonus is taken
//!   out; the 2.6x science gap was population (41 against 75 at turn 100).
//!   Busan stalled at four citizens on two Lumber Mills beside an unboated
//!   Fish and unfarmed Maize.
//! - Gold cannot cover it: the host's Builder price is about 245 at Standard
//!   turn 80 against a median treasury of 100-200.
//!
//! **The rule.** Only when no Builder is held anywhere. Among the cities that
//! can train one within `BUILDERLESS_MAX_TURNS` standard turns, the fastest
//! puts a Builder at the head. A city keeps its queue when it was attacked in
//! the last four turns, is besieged, answers barbarians, has an at-war or
//! barbarian soldier within `HOME_THREAT_RADIUS`, holds the early Settler
//! floor, or its head is a Settler, Walls, a wonder or project, the bound
//! Granary, the deterrent, a military unit while at war with a major, or
//! anything finishing this turn. The displaced item keeps its progress. One
//! claim a turn, after the backlog claim (which, when it fires, leaves a
//! Builder held and this a no-op). The governor's rescoring keeps the Builder
//! through [`AdvancedAi::backlog_builder_holds`]. Off, nothing runs.
//!
//! **Version two** (`builderless-empire-trains-one-2`). Live Deity Online
//! civvis-20261104T211604Z: the capital ran Scout, Slinger and four Settlers
//! and held no Builder until turn 30, so version one, which keeps every
//! Settler's queue, never fired. Under version two a Settler with no
//! production invested yields the head to the empire's only Builder; the
//! early Settler floor's own city still keeps it.

use super::*;

/// The slowest Builder this claim asks for, in standard turns: the backlog
/// gate's own runaway limit (`WORKED_BACKLOG_RUNAWAY_MAX_TURNS`).
pub(super) const BUILDERLESS_MAX_TURNS: u32 = 15;

/// The unimproved worked tiles that make an empire without a Builder short of
/// one: the backlog gate's `WORKED_BACKLOG_MIN_TILES`.
pub(super) const BUILDERLESS_MIN_BACKLOG: usize = 4;

impl AdvancedAi {
    /// Whether `pid` holds no Builder afield and none at the head of a queue.
    pub(super) fn empire_holds_no_builder(g: &Game, pid: usize) -> bool {
        !g.player_unit_ids(pid)
            .into_iter()
            .any(|uid| g.units[&uid].kind == "builder")
            && !g.player_city_ids(pid).into_iter().any(|cid| {
                matches!(
                    g.cities[&cid].queue.first(),
                    Some(Item::Unit { unit }) if *unit == "builder"
                )
            })
    }

    /// Whether an at-war soldier or a barbarian raider stands within
    /// `HOME_THREAT_RADIUS` of `cid`.
    fn builderless_city_in_danger(g: &Game, pid: usize, cid: u32) -> bool {
        let pos = g.cities[&cid].pos;
        g.units.values().any(|unit| {
            unit.owner != pid
                && g.wdist(unit.pos, pos) <= crate::ai::HOME_THREAT_RADIUS
                && g.rules.units[unit.kind].class == "military"
                && (BasicAi::is_barbarian_raider(g, unit)
                    || (Some(unit.owner) != g.barb_pid && g.is_at_war(pid, unit.owner)))
        })
    }

    /// See the module: with the gene on, two or more cities, no Builder held
    /// anywhere and a worked backlog of `BUILDERLESS_MIN_BACKLOG`, the fastest
    /// safe city puts a Builder at the head. Exact no-op with the gene off.
    pub(super) fn claim_builderless_empire_builder(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !(self.builderless_empire_trains_one || self.builderless_empire_trains_one_2)
            || self.base.minor
            || self.base.barb
        {
            return;
        }
        let cities = g.player_city_ids(pid);
        if cities.len() < 2 || !Self::empire_holds_no_builder(g, pid) {
            return;
        }
        let backlog = BasicAi::unimproved_worked_tiles(g, pid);
        if backlog < BUILDERLESS_MIN_BACKLOG {
            return;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && !other.is_free_city
                && g.is_at_war(pid, other.id)
        });
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        let max_turns = f64::from(g.standard_duration(BUILDERLESS_MAX_TURNS));
        let mut best: Option<(f64, u32)> = None;
        {
            let _memo = g.query_memo();
            for &cid in &cities {
                let city = &g.cities[&cid];
                if city.owner != pid
                    || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                    || !g.can_produce(pid, cid, &builder)
                    || self.base.besieged_city_item(g, pid, cid).is_some()
                    || self.base.barbarian_defense_item(g, pid, cid).is_some()
                    || Self::builderless_city_in_danger(g, pid, cid)
                    || self.early_settler_floor_holds(g, pid, cid, plan)
                {
                    continue;
                }
                if let Some(head) = city.queue.first() {
                    if !self.builderless_head_yields(g, cid, head, at_major_war)
                        || self.deterrence_unit_holds(g, pid, head)
                        || self.bound_granary_holds(g, pid, cid, plan, head)
                        || self.production_build_turns(g, pid, cid, head) <= 1.0
                    {
                        continue;
                    }
                }
                let turns = self.production_build_turns(g, pid, cid, &builder);
                if turns > max_turns {
                    continue;
                }
                if best.is_none_or(|(old, old_city)| (turns, cid) < (old, old_city)) {
                    best = Some((turns, cid));
                }
            }
        }
        let Some((turns, cid)) = best else {
            return;
        };
        let head = g.cities[&cid].queue.first().cloned();
        let name = g.cities[&cid].name.clone();
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: builder,
            },
        )
        .is_ok()
        {
            think!(self.journal(), Economy, Decision,
                   "{name} trains the empire's only Builder";
                   "builderless-empire-trains-one: no Builder afield or queued against {backlog} \
                    unimproved worked tiles; {turns:.1} turns, ahead of {}",
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }

    /// Whether `head` may yield the queue to the empire's only Builder: not a
    /// Builder, Walls, a wonder, a project, a military unit while at war with
    /// a major, or a Settler -- except, under version two, a Settler with no
    /// production invested.
    fn builderless_head_yields(&self, g: &Game, cid: u32, head: &Item, at_major_war: bool) -> bool {
        match head {
            Item::Unit { unit } => {
                if *unit == "builder" {
                    return false;
                }
                if *unit == "settler" {
                    return self.builderless_empire_trains_one_2
                        && g.item_invested_production(cid, head) <= f64::EPSILON;
                }
                let Some(spec) = g.rules.units.get(unit) else {
                    return false;
                };
                !(at_major_war && spec.class == "military")
            }
            Item::Building { building } => {
                !matches!(
                    building.as_str(),
                    "walls" | "medieval_walls" | "renaissance_walls"
                ) && !g
                    .rules
                    .buildings
                    .get(building)
                    .is_some_and(|spec| spec.wonder)
            }
            Item::District { .. } => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests;
