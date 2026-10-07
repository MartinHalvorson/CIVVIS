//! `surge-fields-the-bombers`: the air surge fields its wing.
//!
//! -d8's census of the 89 Emperor games of October 6-7 that reached Advanced
//! Flight: Bombers alive at AF+15 a median 1 (none in 44%), at AF+30 a median
//! 3 (none in 22%); the first Bomber a median 13 turns after AF, and never in
//! 27%. Major cities taken from AF to AF+40 rise 0.07 -> 0.24 -> 0.60 with 0,
//! 1-2 and 3+ Bombers alive at AF+30, and captures run 0.20 per 100 turns
//! before AF against 1.24 after it.
//!
//! ★★★ THE RESERVED SLOT WAS SPENT BY A GOVERNOR THAT NEVER ASKED.
//! `air_surge_reserves_field_slot` keeps the productive city's last specialty
//! slot for the airfield, but only the strategic scorer and the district plan
//! read it. Live Emperor civvis-20261007T121238Z (game 349): the surge was
//! appointed at turn 90; the delegated governor's Commercial Hub step took
//! Bogota's fifth slot at turn 130 (53 Production) and its industrial-zone
//! step Maracaibo's last at turn 133, the turn Flight landed. The airfield
//! went to Panama, population 1 at 3 Production, as "the empire's fastest
//! city for it"; its first Bomber read 41 turns at turn 151, and at turn 175
//! the empire held Advanced Flight (turn 144), Aluminum 25 and no Bomber.
//! A Bomber trains only in a city holding an Aerodrome
//! (`data/units.json` `requires_district`), so that one slow base was the
//! whole wing.
//!
//! The gene, all of it inert while it is off:
//! - **The slot is held from every governor.** For the production pass the
//!   reserved city's slot-taking districts are entered in the board's
//!   production refusals (`Game::blocked_production`), so the delegated
//!   governor, the strategic-queue steps and every reservation see the same
//!   menu the scorer already prices at a veto. Restored after the pass.
//! - **The airfield goes where the wing trains.** The fast claim ranks a
//!   field by its own turns plus the launch wing's in that city.
//! - **A second field may carry the whole wing**, not only the launch pair,
//!   when it would deliver the remaining Bombers sooner than the first base.
//! - **An airfield city buys a Bomber** when the host prices one and the
//!   treasury keeps [`SURGE_BOMBER_BUY_RESERVE`] after it; one a turn. Faith
//!   cannot buy aircraft (the Grand Master's Chapel buys land units only).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::{AdvancedAi, AIR_SURGE_LAUNCH_BOMBERS};
use crate::game::{Action, Game, Item};
use crate::name::Name;
use crate::think;

/// Gold an airfield city's Bomber purchase must leave in the treasury.
pub(crate) const SURGE_BOMBER_BUY_RESERVE: f64 = 150.0;

/// The board's own refusals before the pass, restored after it: the
/// production keys (`Game::blocked_production`) and the district names
/// (`Game::blocked_districts`, which the district menu reads).
pub(crate) type HeldRefusals = (
    Arc<BTreeMap<u32, BTreeSet<String>>>,
    Arc<BTreeMap<u32, BTreeSet<Name>>>,
);

impl AdvancedAi {
    /// `surge-fields-the-bombers`: the districts that would take the air
    /// plan's reserved airfield slot, per city: every district the city could
    /// start that `air_surge_reserves_field_slot` vetoes, less any it already
    /// has a foundation for. Empty with the gene off, outside Domination, or
    /// with no slot reserved.
    pub(crate) fn surge_field_slot_refusals(
        &self,
        g: &Game,
        pid: usize,
    ) -> BTreeMap<u32, Vec<Item>> {
        let mut refusals = BTreeMap::new();
        if !self.surge_fields_the_bombers {
            return refusals;
        }
        for cid in g.player_city_ids(pid) {
            let founded: BTreeSet<Name> = g.cities[&cid]
                .owned_tiles
                .iter()
                .filter_map(|pos| g.map.tiles[pos].district_foundation.as_ref())
                .map(|foundation| foundation.district)
                .collect();
            let items: Vec<Item> = g
                .producible_items(pid, cid)
                .into_iter()
                .filter(|item| {
                    matches!(item, Item::District { district, .. } if !founded.contains(district))
                })
                .filter(|item| self.air_surge_reserves_field_slot(g, pid, cid, item))
                .collect();
            if !items.is_empty() {
                // The reservation names one city, the most productive that
                // can host the field.
                refusals.insert(cid, items);
                break;
            }
        }
        refusals
    }

    /// Enter the reserved slot's refusals for the production pass. Returns
    /// the board's own refusals to restore with
    /// [`Self::surge_release_field_slot`]; `None` when nothing was held.
    pub(crate) fn surge_hold_field_slot(
        &mut self,
        g: &mut Game,
        pid: usize,
    ) -> Option<HeldRefusals> {
        let refusals = self.surge_field_slot_refusals(g, pid);
        if refusals.is_empty() {
            return None;
        }
        if self.journal().wants(crate::reasoning::Level::Detail) {
            for (cid, items) in &refusals {
                think!(self.journal(), Cities, Detail,
                       "{}'s last district slot waits for the airfield", g.cities[cid].name;
                       "surge-fields-the-bombers: {} district choice{} held back while the air \
                        surge has no airfield; the most productive city trains the wing",
                       items.len(), if items.len() == 1 { "" } else { "s" });
            }
        }
        let held = (
            Arc::clone(&g.blocked_production),
            Arc::clone(&g.blocked_districts),
        );
        let mut production = (*held.0).clone();
        let mut districts = (*held.1).clone();
        for (cid, items) in refusals {
            for item in items {
                if let Item::District { district, .. } = item {
                    districts.entry(cid).or_default().insert(district);
                }
                production
                    .entry(cid)
                    .or_default()
                    .insert(Game::production_block_key(&item));
            }
        }
        g.blocked_districts = Arc::new(districts);
        // Also retires every menu derived before the refusals.
        g.replace_blocked_production(production);
        Some(held)
    }

    /// Restore the board's own refusals after the pass.
    pub(crate) fn surge_release_field_slot(g: &mut Game, held: Option<HeldRefusals>) {
        if let Some((production, districts)) = held {
            g.blocked_districts = districts;
            g.replace_blocked_production((*production).clone());
        }
    }

    /// Turns the launch wing takes in `cid` once it holds the field: the
    /// launch Bombers at the city's own rate for them. `0.0` with the gene
    /// off or no Bomber in the catalogue.
    pub(super) fn surge_launch_wing_turns(&self, g: &Game, pid: usize, cid: u32) -> f64 {
        if !self.surge_fields_the_bombers {
            return 0.0;
        }
        let Some(bomber) = Self::air_surge_bomber(g, pid) else {
            return 0.0;
        };
        let item = Item::Unit { unit: bomber };
        let rate =
            (g.city_yields(cid).production * g.item_prod_mult(pid, cid, Some(&item))).max(0.1);
        AIR_SURGE_LAUNCH_BOMBERS as f64 * g.item_cost_for_city(pid, cid, &item) / rate
    }

    /// The Bombers a second field may be raised for: the launch pair as
    /// shipped, or the whole wing still missing with the gene on.
    pub(super) fn surge_wing_missing(&self, launch_missing: usize, goal_missing: usize) -> usize {
        if self.surge_fields_the_bombers {
            launch_missing.max(goal_missing)
        } else {
            launch_missing
        }
    }

    /// `surge-fields-the-bombers`: buy one Bomber in an airfield city when
    /// the host prices it and the treasury keeps
    /// [`SURGE_BOMBER_BUY_RESERVE`] after it. The cheapest such city; one a
    /// turn. `false` with the gene off or nothing bought.
    pub(super) fn surge_buy_a_bomber(
        &mut self,
        g: &mut Game,
        pid: usize,
        threatened: Option<u32>,
    ) -> bool {
        if !self.surge_fields_the_bombers {
            return false;
        }
        let (Some(bomber), Some(field)) = (
            Self::air_surge_bomber(g, pid),
            Self::air_surge_field(g, pid),
        ) else {
            return false;
        };
        let field = g.district_family(field);
        let gold = g.players[pid].gold;
        let best = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| threatened != Some(*cid))
            .filter(|cid| g.city_has_district_family(&g.cities[cid], field))
            .filter_map(|cid| {
                g.unit_purchase_cost(pid, cid, bomber.as_str(), "gold")
                    .filter(|cost| gold - cost >= SURGE_BOMBER_BUY_RESERVE)
                    .map(|cost| (cost, cid))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let Some((cost, city)) = best else {
            return false;
        };
        if g.apply(
            pid,
            &Action::Buy {
                city,
                unit: bomber,
                formation: 0,
                currency: "gold".to_string(),
            },
        )
        .is_err()
        {
            return false;
        }
        if self.journal().wants(crate::reasoning::Level::Decision) {
            think!(self.journal(), Military, Decision,
                   "{} buys a bomber for the air surge", g.cities[&city].name;
                   "surge-fields-the-bombers: {cost:.0} Gold of {gold:.0}, keeping {:.0}; \
                    the wing trains only where an airfield stands",
                   SURGE_BOMBER_BUY_RESERVE);
        }
        true
    }
}
