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

use super::*;

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
