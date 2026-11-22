//! `pillaged-districts-repair-first`: a city with a pillaged district or
//! building repairs it before anything the routine claims put at its head,
//! once no hostile soldier stands within [`REPAIR_SAFE_TILES`] of the site.
//!
//! **Why (live 6p Immortal and Emperor, 2026-10-31/11-01).**
//! - Pillaged districts sit dead. Across 63 lane seats since 10-31 03:30Z, 26
//!   had a pillaged district: 77 spells, a median 10 turns and a quarter 20 or
//!   more, and 19 never repaired by t150. Campuses led (22), then Government
//!   Plazas, Diplomatic Quarters, Holy Sites and Industrial Zones.
//! - The repair is cheap and highly valued, but it never gets the queue. The
//!   strategic scorer prices a district repair at 1,500 and BasicAi's
//!   `pick_item` ranks repairs first, yet the claim genes run before either:
//!   they fill the idle queue, or take its head, with their own item. Live
//!   civvis-20261101T024926Z (Kristina, Religion): Stockholm's Holy Site was
//!   pillaged at t30 with the seat on 18 of 30 Prophet points. Its queue went
//!   idle at t31, 40, 44, 53, 57 and 62, and each time a claim took it: the
//!   Granary (`granary-claims-the-bound-queue`), a Settler
//!   (`early-settler-floor`), a Campus, a Library, a Diplomatic Quarter. The
//!   site stayed pillaged and the points frozen until the race closed at t66,
//!   with no religion. The repair was offered every turn; Eridu's in another
//!   seat cost 7 Production, one turn.
//!
//! **What the gene does.** After every other claim and before the commitment
//! pass, each city of ours with a repair on its producible list (the engine
//! offers a district repair only on our own pillaged district tile with no
//! enemy standing on it) takes the cheapest, a district before its
//! buildings, when no hostile soldier is within [`REPAIR_SAFE_TILES`] of the
//! site. A head the Theater claim's rule lets defer keeps its progress; a
//! head finishing this turn, or a soldier while a major is at war with us,
//! keeps the queue. A head that is already a repair is left alone.
//!
//! Off, nothing changes.
//!
//! **`stuck-repair-yields-the-queue`** (opt-in, reads this claim): a repair
//! the claim asks for turn after turn without it ever standing at the head
//! is one the host does not start, and the city sat idle behind it. Live
//! Qin `civvis-t7-20261122T095414Z` (Lv 7, level on techs with the leaders
//! at t200): Xi'an's pillaged district at (12,21) was asked on 19 turns
//! running from t198 ("repairs first: repairs to the district ... ahead of an
//! idle queue", 4 Production, "about 1 turn"); the host answered each produce
//! `DISTRICT_GOVERNMENT` with `producing=nothing`, so the capital built
//! nothing until the Netherlands' Science victory at t216. Runs of 7-11 turns
//! recur elsewhere (Paris's Factory and a district, Aduatuca's Arena,
//! Noviodunum's district). With the gene, a repair asked on
//! [`STUCK_REPAIR_TURNS`] consecutive turns (or twice its own turns plus two,
//! if longer) stands aside for [`STUCK_REPAIR_YIELD_TURNS`]: the claim skips
//! it and the strategic scorer does not offer it, so the city builds
//! something else, then the repair is tried again.

use super::*;

/// Consecutive turns a repair may be asked, never started, before it yields.
pub(super) const STUCK_REPAIR_TURNS: u32 = 4;

/// How long a stuck repair stands aside before it is asked again.
pub(super) const STUCK_REPAIR_YIELD_TURNS: u32 = 10;

/// `stuck-repair-yields-the-queue`: one repair site's asking run.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct StuckRepair {
    /// The first turn of the current run of consecutive asks.
    pub(super) run_from: u32,
    /// The last turn it was asked.
    pub(super) last: u32,
    /// Withheld before this turn.
    pub(super) until: u32,
}

/// A hostile soldier this close to the pillaged site holds its repair: it
/// would pillage the site again.
pub(super) const REPAIR_SAFE_TILES: i32 = 2;

impl AdvancedAi {
    /// Whether a hostile land or naval soldier stands within
    /// [`REPAIR_SAFE_TILES`] of `pos`: a barbarian, or a major at war with us.
    pub(super) fn repair_site_threatened(g: &Game, pid: usize, pos: Pos) -> bool {
        g.units.values().any(|unit| {
            unit.owner != pid
                && g.rules.units[unit.kind].class == "military"
                && g.wdist(unit.pos, pos) <= REPAIR_SAFE_TILES
                && g.players.get(unit.owner).is_some_and(|owner| {
                    owner.is_barbarian || (!owner.is_minor && g.is_at_war(pid, unit.owner))
                })
        })
    }

    /// The repair `cid` should produce now. `None` with the gene off, with
    /// no repair on offer, when every site has a hostile soldier near it, or
    /// when the head is already a repair.
    pub(super) fn pillaged_repair_item(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        if !self.pillaged_districts_repair_first {
            return None;
        }
        if matches!(g.cities[&cid].queue.first(), Some(Item::Repair { .. })) {
            return None;
        }
        g.producible_items(pid, cid)
            .into_iter()
            .filter(|item| match item {
                Item::Repair { pos, .. } => !Self::repair_site_threatened(g, pid, *pos),
                _ => false,
            })
            .min_by(|left, right| {
                let rank = |item: &Item| match item {
                    Item::Repair { repair, .. } if *repair == "district" => 0,
                    _ => 1,
                };
                rank(left)
                    .cmp(&rank(right))
                    .then_with(|| {
                        g.item_cost_for_city(pid, cid, left)
                            .total_cmp(&g.item_cost_for_city(pid, cid, right))
                    })
                    .then_with(|| format!("{left:?}").cmp(&format!("{right:?}")))
            })
    }

    /// `stuck-repair-yields-the-queue`: whether `item`, a repair in `cid`,
    /// stands aside this turn. False with the gene off or for any other item.
    pub(super) fn stuck_repair_withheld(&self, g: &Game, cid: u32, item: &Item) -> bool {
        if !self.stuck_repair_yields_the_queue {
            return false;
        }
        let Item::Repair { pos, .. } = item else {
            return false;
        };
        self.stuck_repairs
            .borrow()
            .get(&(cid, *pos))
            .is_some_and(|watch| g.turn < watch.until)
    }

    /// `stuck-repair-yields-the-queue`: count this turn's ask of `repair` in
    /// `cid`; true when it has now been asked long enough without starting
    /// that it yields (and is then withheld).
    fn stuck_repair_yields(&self, g: &Game, pid: usize, cid: u32, repair: &Item) -> bool {
        if !self.stuck_repair_yields_the_queue {
            return false;
        }
        let Item::Repair { pos, .. } = repair else {
            return false;
        };
        let turn = g.turn;
        let own_turns = (g.item_cost_for_city(pid, cid, repair)
            / g.city_yields(cid).production.max(0.5))
        .ceil();
        let limit = (STUCK_REPAIR_TURNS as f64).max(2.0 * own_turns + 2.0);
        let mut watches = self.stuck_repairs.borrow_mut();
        let watch = watches.entry((cid, *pos)).or_default();
        if watch.last + 1 < turn || watch.last > turn {
            watch.run_from = turn;
        }
        watch.last = turn;
        if f64::from(turn - watch.run_from + 1) >= limit {
            watch.until = turn + STUCK_REPAIR_YIELD_TURNS;
            watch.run_from = turn + STUCK_REPAIR_YIELD_TURNS;
            return true;
        }
        false
    }

    /// See the module. Exact no-op with the gene off.
    pub(super) fn claim_pillaged_repairs(&self, g: &mut Game, pid: usize) {
        if !self.pillaged_districts_repair_first {
            return;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        });
        for cid in g.player_city_ids(pid) {
            let Some(repair) = self.pillaged_repair_item(g, pid, cid) else {
                continue;
            };
            // `stuck-repair-yields-the-queue`: a repair standing aside, or one
            // asked long enough without starting, leaves the queue alone.
            if self.stuck_repair_withheld(g, cid, &repair) {
                continue;
            }
            if self.stuck_repair_yields(g, pid, cid, &repair) {
                think!(self.journal(), Cities, Decision,
                       "{} gives a stuck repair the queue back: {}", g.cities[&cid].name,
                       Self::plain_item(&repair);
                       "stuck-repair-yields-the-queue: asked {} turns running and never started, so \
                        it stands aside for {} turns", STUCK_REPAIR_TURNS, STUCK_REPAIR_YIELD_TURNS);
                continue;
            }
            let head = g.cities[&cid].queue.first().cloned();
            if let Some(item) = &head {
                if !Self::culture_theater_may_defer(g, pid, cid, item, at_major_war) {
                    continue;
                }
            }
            let name = g.cities[&cid].name.clone();
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: repair.clone(),
                },
            )
            .is_ok()
            {
                think!(self.journal(), Cities, Decision,
                       "{name} repairs first: {}", Self::plain_item(&repair);
                       "pillaged-districts-repair-first: ahead of {}",
                       head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                           "{} (it keeps its progress)", Self::plain_item(&item))));
            }
        }
    }
}

#[cfg(test)]
mod tests;
