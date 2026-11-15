//! `gold-clears-the-denial-spy-queue` (opt-in): while a Spy slot stands
//! free against a space racer, Gold buys whatever holds the best Spy city's
//! queue, and the Spy trains there at once.
//!
//! ## Why
//!
//! Live Deity Culture seats, 2026-11-15. Russia civvis-20261115T114552Z
//! reached 217 visiting tourists against a bar of 430 at t188 and lost at
//! t189 to Brazil's Science Victory; the first rival Exoplanet moved at t168.
//! Spy capacity 1 at t100, 2 at t112, 3 at t134, 4 at t140; Spies afield 0-1
//! until t128 and 2-3 from t140. Each Spy cost 112-187 Production in cities
//! making 12-19 a turn, while 419-601 Gold sat banked from t150 to the end.
//! Spies afield over capacity after t80, six full Culture games: 45%, 18%,
//! 29%, 57%, 47%, 48%. The first rival Exoplanet showed at t164-180 in the
//! day's Culture games and the game ended 12-25 turns later: that launch is
//! the denial's deadline.
//!
//! ## Gold cannot buy the Spy itself
//!
//! `UNIT_SPY` carries no `PurchaseYield`
//! (`Base/Assets/Gameplay/Data/Units.xml:769`, and no expansion row adds
//! one), where `UNIT_TRADER` (`:753`) and `UNIT_BUILDER` carry
//! `PurchaseYield="YIELD_GOLD"`; the simulator refuses it the same way
//! (`do_buy_formation`: "Spies cannot be purchased with Gold or Faith").
//! What Gold can do is clear the way. `science-denial-trains-spies` puts a
//! Spy only in a city whose queue head it may displace: never a military
//! unit, a Settler, Walls, a Wonder or a project, and under
//! `denial-spies-fill-every-slot` on the Culture aim never a Theater Square,
//! its buildings or a building with Great Work slots. On a Culture seat the
//! strongest cities are the ones building exactly those, so the Spy goes to
//! a weak city or waits. This gene buys the head instead, so the strongest
//! city trains the Spy now and the head it held is built, not delayed.
//!
//! ## What the gene does
//!
//! Inert with the flag off, off a Culture or Domination aim (the aims
//! `denial-spies-fill-every-slot` covers), with the Science Victory off,
//! with `science-denial-trains-spies` off (the claim it feeds) or paused by
//! `denial-spies-count-their-losses`. On, once a turn, just before that
//! claim:
//!
//! 1. **A free slot against a racer.** A rival is on the denial board
//!    (`denial_spy_racers`: a decisive space racer, or under
//!    `space-denial-spies-from-rocketry` one holding Rocketry), and the Spies
//!    held plus those in any queue stand below the claim's own target
//!    (`denial_training_target`: our whole capacity under
//!    `denial-spies-fill-every-slot`). A Spy already in a queue keeps it: its
//!    Production is banked in that city, and moving it would waste it.
//! 2. **The best Spy city is blocked.** Among the cities the claim could
//!    use (not threatened, not attacked within four turns, not the held
//!    launch pad, and offered a Spy), the one that finishes a Spy soonest
//!    (the host's quote, else the Production left over the city's rate)
//!    has a head the claim will not displace, and that head is a building or
//!    unit Gold can buy with at most [`SPY_QUEUE_MAX_PAID_SHARE`] of it
//!    already paid (a purchase throws that Production away). In practice a
//!    Settler or a building: a land combat unit at the head bars every land
//!    combat purchase in its city (`land_combat_purchase_slot_open`), and
//!    districts, Wonders and projects are never for sale.
//! 3. **It saves real time.** That city's Spy comes at least
//!    [`SPY_QUEUE_MIN_TURNS_SAVED`] turns sooner than in the best city the
//!    claim could take without Gold, or no such city exists.
//! 4. **Above the reserve.** The price fits in the Gold above
//!    `purchase_upkeep_floor`, the reserve every purchase keeps.
//!
//! Then it buys the head and puts the Spy at the head of that queue (the
//! claim's own switch). The claim that follows sees the slot filled and
//! fills any other free slot as before. One purchase a turn. The journal
//! reason starts with the tag and states the Gold, the price, the slots
//! and the Spies.

use super::*;

/// A city attacked within this many turns is not where the Spy is placed
/// (the denial claim's own memory).
const SPY_QUEUE_ATTACK_MEMORY: u32 = 4;

/// The share of the head's cost already paid above which the gene does not
/// buy it: a purchase discards the head's Production, so a head mostly built
/// finishes on its own first.
pub(crate) const SPY_QUEUE_MAX_PAID_SHARE: f64 = 0.25;

/// How many turns sooner the bought queue must finish the Spy than the best
/// queue the claim can take without Gold.
pub(crate) const SPY_QUEUE_MIN_TURNS_SAVED: f64 = 2.0;

/// One purchase the gene would make this turn.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SpyQueuePurchase {
    /// The city whose head is bought and which then trains the Spy.
    pub(crate) city: u32,
    /// The head bought.
    pub(crate) head: Item,
    /// Its Gold price.
    pub(crate) price: f64,
    /// Turns to a Spy in that city.
    pub(crate) turns: f64,
    /// The best city the claim could take without Gold and its turns.
    pub(crate) open: Option<(u32, f64)>,
    /// Spies held, in a queue, and the slots the claim trains to.
    pub(crate) held: usize,
    pub(crate) queued: usize,
    pub(crate) slots: usize,
    /// Gold above the reserve before the purchase.
    pub(crate) budget: f64,
    /// The first rival on the denial board.
    pub(crate) racer: usize,
}

impl AdvancedAi {
    /// Whether the gene can act on this board: the flag, the claim it feeds,
    /// the Science Victory, and a Culture or Domination aim.
    pub(super) fn gold_clears_the_denial_spy_queue_on(&self, g: &Game) -> bool {
        self.gold_clears_the_denial_spy_queue
            && self.science_denial_trains_spies
            && g.victory_conditions.science
            && matches!(
                self.active_victory_target(g),
                Some(VictoryTarget::Culture | VictoryTarget::Domination)
            )
    }

    /// Turns to a Spy in `cid`: the host's quote when the export carries
    /// one, else the Production left (net of a Spy's banked progress) over
    /// the city's rate. `None` for a city making nothing.
    fn denial_spy_turns(g: &Game, pid: usize, cid: u32) -> Option<f64> {
        let spy = Item::Unit {
            unit: crate::name!("spy"),
        };
        if let Some(turns) = g
            .host_production_turns(cid, &spy)
            .filter(|turns| turns.is_finite() && *turns > 0.0)
        {
            return Some(turns);
        }
        let city = g.cities.get(&cid)?;
        let rate = g.city_yields(cid).production * g.item_prod_mult(pid, cid, Some(&spy));
        if rate <= 0.0 {
            return None;
        }
        let banked = if city.queue.first() == Some(&spy) {
            city.production
        } else {
            city.production_progress
                .get(&Game::item_progress_key(&spy))
                .copied()
                .unwrap_or(0.0)
        };
        let left = (g.item_cost_for_city(pid, cid, &spy) - banked).max(0.0);
        Some((left / rate).ceil().max(1.0))
    }

    /// The Gold price of `cid`'s queue head when Gold may buy it and at most
    /// [`SPY_QUEUE_MAX_PAID_SHARE`] of it is paid. `None` otherwise.
    fn denial_spy_head_price(g: &Game, pid: usize, cid: u32) -> Option<(Item, f64)> {
        let city = g.cities.get(&cid)?;
        let head = city.queue.first()?;
        if g.purchase_is_blocked(cid, head) {
            return None;
        }
        let cost = g.item_cost_for_city(pid, cid, head);
        if cost <= 0.0 || city.production > SPY_QUEUE_MAX_PAID_SHARE * cost {
            return None;
        }
        let price = match head {
            Item::Building { building } => g.building_gold_purchase_cost(pid, cid, building),
            Item::Unit { unit } if *unit != "spy" => g.unit_purchase_cost(pid, cid, unit, "gold"),
            _ => None,
        }?;
        (price.is_finite() && price > 0.0).then(|| (head.clone(), price))
    }

    /// See the module: the purchase the gene would make this turn, if any.
    /// `None` with the gene off.
    pub(super) fn denial_spy_queue_purchase(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<SpyQueuePurchase> {
        if !self.gold_clears_the_denial_spy_queue_on(g)
            || self.base.minor
            || self.base.barb
            || self.spy_losses_pause_training(g)
        {
            return None;
        }
        let racers = self.denial_spy_racers(g, pid);
        let racer = *racers.iter().next()?;
        let spy = Item::Unit {
            unit: crate::name!("spy"),
        };
        let cities = g.player_city_ids(pid);
        let queued = cities
            .iter()
            .filter(|cid| g.cities[cid].queue.contains(&spy))
            .count();
        let held = g.spy_agents(pid);
        let slots = self.denial_training_target(g, pid, &racers);
        if held + queued >= slots {
            return None;
        }
        let budget = g.players[pid].gold - self.purchase_upkeep_floor(g, pid, 0.0);
        if budget <= 0.0 {
            return None;
        }
        let mut open: Option<(u32, f64)> = None;
        let mut blocked: Option<(u32, f64, Item, f64)> = None;
        for &cid in &cities {
            let city = &g.cities[&cid];
            let attacked = city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= SPY_QUEUE_ATTACK_MEMORY;
            if plan.threatened_city == Some(cid)
                || attacked
                || self.launch_pad_holds_city(g, pid, cid)
                || !g.can_produce(pid, cid, &spy)
            {
                continue;
            }
            let Some(turns) = Self::denial_spy_turns(g, pid, cid) else {
                continue;
            };
            // The claim's own displacement test.
            let claimable = city
                .queue
                .first()
                .is_none_or(|head| Self::spy_displaces(g, head))
                && !self.denial_fill_keeps_the_culture_head(g, cid);
            if claimable {
                if open.is_none_or(|(_, best)| turns < best) {
                    open = Some((cid, turns));
                }
                continue;
            }
            let Some((head, price)) = Self::denial_spy_head_price(g, pid, cid) else {
                continue;
            };
            if price > budget + 1e-9 {
                continue;
            }
            let better = blocked.as_ref().is_none_or(|(_, best, _, best_price)| {
                turns < *best || (turns == *best && price < *best_price)
            });
            if better {
                blocked = Some((cid, turns, head, price));
            }
        }
        let (city, turns, head, price) = blocked?;
        if open.is_some_and(|(_, best)| turns + SPY_QUEUE_MIN_TURNS_SAVED > best) {
            return None;
        }
        Some(SpyQueuePurchase {
            city,
            head,
            price,
            turns,
            open,
            held,
            queued,
            slots,
            budget,
            racer,
        })
    }

    /// See the module: buy the head of the best Spy city and train the Spy
    /// there. True when it did. Exact no-op with the gene off.
    pub(super) fn clear_queue_for_denial_spy(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        let Some(buy) = self.denial_spy_queue_purchase(g, pid, plan) else {
            return false;
        };
        let bank = g.players[pid].gold;
        let action = match &buy.head {
            Item::Building { building } => Action::BuyBuilding {
                city: buy.city,
                building: *building,
                currency: "gold".into(),
            },
            Item::Unit { unit } => Action::Buy {
                city: buy.city,
                unit: *unit,
                formation: 0,
                currency: "gold".into(),
            },
            _ => return false,
        };
        if g.apply(pid, &action).is_err() {
            return false;
        }
        let spy = Item::Unit {
            unit: crate::name!("spy"),
        };
        let trained = g
            .apply(
                pid,
                &Action::Produce {
                    city: buy.city,
                    item: spy,
                },
            )
            .is_ok();
        let paid = bank - g.players[pid].gold;
        let open = buy.open.map_or(
            "no other city can take a Spy now".to_string(),
            |(cid, turns)| {
                format!(
                    "{turns:.0} in {}, the best queue without Gold",
                    g.cities[&cid].name
                )
            },
        );
        think!(self.journal(), Military, Decision,
            "{} buys its {} so it can train a Spy against {}'s space race",
            g.cities[&buy.city].name, Self::plain_item(&buy.head), g.players[buy.racer].civ;
            "gold-clears-the-denial-spy-queue: {paid:.0} Gold of {bank:.0} (price {:.0}, {:.0} above the reserve); \
             {} Spies held and {} queued against {} slots; the Spy takes {:.0} turns here against {open}{}",
            buy.price, buy.budget, buy.held, buy.queued, buy.slots, buy.turns,
            if trained { "" } else { "; the host then refused the Spy" });
        trained
    }
}

#[cfg(test)]
mod tests;
