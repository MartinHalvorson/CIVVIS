//! Gold and the production cards: four opt-in genes that decide WHICH of the
//! two currencies pays for an item. From the operator's heuristic
//! (2026-08-25):
//!
//! > production is most efficiently spent in combination with boosted
//! > production policy cards. money is flexible — a new city will always
//! > have low production but has access to the same money as anywhere else.
//! > somewhat optimal to spend money on things that can't be boosted by
//! > production policy cards and to spend money on new cities (that take a
//! > while to scale production) and on emergencies. money is also immediate
//! > — whereas production takes turns to build.
//!
//! ## What the controller did before
//!
//! The engine knows exactly what a slotted card is worth to a build:
//! [`Game::item_prod_mult`] returns the +% Production a city's queue head
//! earns from Agoge, Colonization, Ilkum, Maritime Industries, Maneuver,
//! Limes, Feudal Contract and the rest, and `process_city` applies it every
//! turn. **No controller ever read it** — `grep item_prod_mult src/ai` is
//! empty before this module. Both places that price an item against a city's
//! Production, `gold_purchase_score` and `production_value`, divide the
//! remaining cost by the RAW city yield. A Settler under Colonization
//! therefore looks half again slower to build than it is, which makes it a
//! BETTER Gold purchase in the scorer's eyes: the shipped purchaser is biased
//! toward buying exactly the items a card is already discounting, and the
//! governor is indifferent to the discount.
//!
//! The emergency purchase, `emergency_city_defense_purchase`, is gated on
//! `garrison_under_fire`, a live-bridge-only flag no native controller sets
//! (`BasicAi::besieged_city_item` says so in its own comment), so on a native
//! board a city can bleed to nothing beside a full treasury while the
//! purchaser runs its ordinary reserve arithmetic.
//!
//! ## The Gold and emergency genes
//!
//! 1. **`buy-what-cards-cannot-boost`** — the Gold purchase scorer prices the
//!    turns a build would take at the card-boosted rate, and scales the Gold
//!    price by the same multiplier: a Gold spent on a boosted item replaces
//!    less Production than the same Gold spent on an item no card touches.
//!    Boosted items lose purchase priority to unboosted ones; an item at
//!    multiplier 1 scores exactly as it did.
//! 2. **`build-what-cards-boost`** — the city production governor scales a
//!    positive item value by [`BUILD_BOOST_SHARE`] of the card's bonus (a +50%
//!    card is +25% value), capped at [`BUILD_BOOST_CAP`], so the queue leans
//!    toward what the slotted deck makes cheap. It only reorders: a value at
//!    or below zero is never raised.
//! 3. **`native-emergency-purchase`** — the emergency defence purchase fires
//!    on a native signal: a city that has lost health, was struck within the
//!    last [`EMERGENCY_RECENT_TURNS`] turns. The confirmed damage remains
//!    authoritative if the attacker has left the current board; it buys Walls
//!    if the city can raise them, otherwise the best land defender — the live
//!    doctrine's own choice — and it spends through the reserve exactly as
//!    the live path does.
//! 4. **`native-emergency-purchase-2`** — a narrower successor to version
//!    one. A host-confirmed hit in the current turn is enough on its own;
//!    damage from the preceding turn still requires a currently visible,
//!    at-war military unit whose legal attack envelope reaches the City
//!    Center. That keeps a same-frame siege hit actionable after its attacker
//!    leaves sight, while rejecting an older uncorroborated scar.
//!
//! ⚠ Version one asks for DAMAGE, not for a hostile in sight.
//! `besieged_city_item` records why: reacting to a single raider in range
//! "bought city count while COSTING score: walls and defenders displace the
//! buildings and districts score is actually made of". Version two preserves
//! that lesson by requiring either the host's same-frame strike fact or the
//! stronger fact that a visible attacker can legally strike the City Center
//! now, rather than treating mere proximity as proof.
//!
//! Each gene is byte-identical when off: every multiplier reads exactly 1.0
//! (and `x * 1.0`, `x / (p * 1.0)` are exact in IEEE arithmetic), and the
//! emergency branch is not consulted.
//!
//! ## The treasury genes (2026-08-26)
//!
//! The live King seat `civvis-20260826T164105Z` held 286 Gold at turn 36 on
//! +7 a turn and had bought nothing in 36 turns; the 130 live runs of the
//! three days before it banked 250–330 Gold by turn 50 and made 0–3 Gold
//! purchases in their first 100 turns. Not one `purchase` order reached the
//! host in the opening of any of them. The cause is the reserve in
//! `advanced_gold_spending`: `250 + 75` Gold per city under Expansion
//! (`300 + 75` under Diplomacy and Culture), and a purchase must leave the
//! whole of it behind — so with one city the seat needs 325 Gold *plus* the
//! price, and with five 625 plus the price, while a Settler costs 160 and a
//! Builder 100 at Online speed. The only Gold purchases in the whole census
//! were made under Recovery's `75 + 25`, and the journal wrote "above a
//! reserve of 150–175" on every one of them. `BasicAi::spend_gold`'s far
//! smaller `100 + 25` runs only while the four-build opening book is in play,
//! when the bank is still under 100.
//!
//! 5. **`treasury-at-work-2`** — the reserve is what an emergency costs plus
//!    what a deficit would drain before it can be corrected: the Gold price
//!    of the dearest land ranged unit some city can build now (the dearest
//!    land melee unit failing that, [`FALLBACK_DEFENDER_PRICE`] failing
//!    both) plus [`DEFICIT_COVER_TURNS`] turns of any recurring deficit,
//!    never below `war_treasury_floor`'s appointed bill. It scales with the
//!    era through the defender's price and with insolvency through the
//!    deficit; it does not scale with the number of cities, which is the
//!    number that made the shipped reserve unreachable. A treasury at work
//!    also stays solvent: a unit whose upkeep would take the recurring
//!    budget below zero is not bought (`unit_purchase_keeps_solvent`), so
//!    the open reserve cannot buy the empire into the bankruptcy the King
//!    autopsy of `civvis-20260826T112920Z` recorded from turn 85. And before
//!    the purchase argmax runs, one under-bought compounding asset is bought
//!    outright in
//!    the city producing the least: a Builder whenever the empire has none
//!    and there is tile work, otherwise a Monument for a city without one.
//!    One a turn, at the working reserve. This is
//!    `solvency-first-trade-slot`'s measured pattern — reserving ONE Trader
//!    priced +4.65 pp, reserving every slot −2.80 — applied to the two
//!    assets the argmax prices lowest against a Settler. Version one
//!    (`treasury-at-work`, the reserve alone) left the code on 2026-09-08
//!    with version two shipping on.
//!
//! Replayed over turns 16–52 of the live run above (`--serve --fresh-board`,
//! every other turn): the stock decider issues one purchase (a Scout at
//! t46); the reserve alone issued thirteen (Archers and Warriors for the
//! threatened cities, a Settler at t40, a Granary at t50) at a reserve of
//! 70 (Slinger) then 120 (Archer) Gold; version two sixteen, the Builder in
//! the weakest city first. A fresh-board replay cannot carry a purchase
//! forward, so those counts say the gene binds, not what a game spends.
//!
//! It is an exact no-op while off: the reserve helper returns the stock
//! value untouched and the ladder is not consulted.

use super::{AdvancedAi, StrategicPlan, CITY_MAX_HP};
use crate::ai::BasicAi;
use crate::game::{Action, Game, Item};
use crate::name::Name;
use crate::think;

/// Share of a card's Production bonus the governor turns into item value: a
/// +50% card raises a positive value by 25%.
pub const BUILD_BOOST_SHARE: f64 = 0.5;
/// The most card bonus the governor prices. Limes and a Maritime Industries
/// Galley reach +100%; nothing is priced past a doubling.
pub const BUILD_BOOST_CAP: f64 = 1.0;
/// The card multiplier is clamped into this range when pricing a purchase, so
/// an item a Congress treaty forbids building (multiplier 0) reads as a slow
/// build rather than an infinite one.
pub const PURCHASE_CARD_MULT_RANGE: (f64, f64) = (0.25, 4.0);
/// How recently a city must have been struck for the native emergency.
pub const EMERGENCY_RECENT_TURNS: u32 = 4;
/// Version two only reacts while the damage is still fresh: this turn or the
/// immediately preceding turn. A longer-lived scar belongs to version one's
/// deliberately broader experiment.
pub const EMERGENCY_V2_FRESH_TURNS: u32 = 1;
/// `treasury-at-work-2`: turns of a recurring deficit the working reserve
/// keeps in the bank, so a purchase never turns a deficit into bankruptcy
/// before the deck or the army can be corrected.
pub const DEFICIT_COVER_TURNS: f64 = 10.0;
/// `treasury-at-work-2`: the Standard-speed Gold price of one emergency
/// defender when no city can name one — an Archer's 60 Production at the
/// shipped ×4 Gold purchase rate. Scaled by the game speed where it is used.
pub const FALLBACK_DEFENDER_PRICE: f64 = 240.0;
/// The Gold purchase rate per point of Production, the shipped
/// `GOLD_PURCHASE_MULTIPLIER` `unit_purchase_cost_for_formation` applies.
const GOLD_PER_PRODUCTION: f64 = 4.0;

/// The working reserve: one emergency defender at `defender` Gold plus
/// [`DEFICIT_COVER_TURNS`] turns of any recurring deficit in `gold_per_turn`,
/// never below `war_floor`, the appointed war's upgrade bill.
pub fn working_treasury_reserve_from(defender: f64, gold_per_turn: f64, war_floor: f64) -> f64 {
    (defender + DEFICIT_COVER_TURNS * (-gold_per_turn).max(0.0)).max(war_floor)
}

/// Whether a unit costing `maintenance` a turn leaves a recurring budget of
/// `gold_per_turn` at or above zero.
pub fn unit_purchase_keeps_solvent(gold_per_turn: f64, maintenance: f64) -> bool {
    maintenance <= 0.0 || gold_per_turn - maintenance >= 0.0
}

/// `upkeep-reserve`: turns of the empire's unit bill an upgrade pass leaves
/// in the treasury.
///
/// Both upgrade passes spent down to a flat 30 Gold at war. On live King
/// 2026-10-03T040354Z the army's bill was 130-220 Gold a turn, and a deck
/// that dropped Levée en Masse for one turn (a government change, a
/// re-ranked military slot) added 60-70 to it: upgrades took the treasury
/// from 155 to 30 at turn 179 and from 459 to 104 at turn 192, the swing
/// then ran it to zero at turns 180-186 and 207-213 (net -108 and -128 a
/// turn), and bankruptcy's Amenity penalty took 100-140 Science a turn while
/// Kongo flew to its Technology victory. One and a half turns of the bill
/// rides out a one-turn swing and most of a second.
pub const UPKEEP_RESERVE_TURNS: f64 = 1.5;

/// The treasury an upgrade pass keeps: `floor`, raised to
/// [`UPKEEP_RESERVE_TURNS`] turns of a unit bill of `unit_upkeep`.
pub fn upgrade_floor_from(floor: f64, unit_upkeep: f64) -> f64 {
    floor.max(UPKEEP_RESERVE_TURNS * unit_upkeep.max(0.0))
}

impl AdvancedAi {
    /// `buy-what-cards-cannot-boost`: the card multiplier the purchase scorer
    /// prices this item's build at. Exactly 1.0 while the gene is off.
    pub(super) fn purchase_card_multiplier(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> f64 {
        if !self.buy_what_cards_cannot_boost {
            return 1.0;
        }
        let (floor, ceiling) = PURCHASE_CARD_MULT_RANGE;
        g.item_prod_mult(pid, cid, Some(item)).clamp(floor, ceiling)
    }

    /// `build-what-cards-boost`: a positive governor value leans toward what
    /// the slotted deck makes cheap. Unchanged while the gene is off, and for
    /// any value at or below zero.
    pub(super) fn card_boosted_value(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        value: f64,
    ) -> f64 {
        if !self.build_what_cards_boost || value <= 0.0 {
            return value;
        }
        let bonus = (g.item_prod_mult(pid, cid, Some(item)) - 1.0).clamp(0.0, BUILD_BOOST_CAP);
        if bonus <= 0.0 {
            return value;
        }
        value * (1.0 + BUILD_BOOST_SHARE * bonus)
    }

    /// Whether either native emergency-purchase family member is armed.
    /// Individual versions remain mutually exclusive through their toggles,
    /// but the shared purchase and reserve paths deliberately ask this one
    /// family gate.
    pub(super) fn native_emergency_purchase_on(&self) -> bool {
        self.native_emergency_purchase || self.native_emergency_purchase_2
    }

    /// `native-emergency-purchase`: whether this city has confirmed recent
    /// damage. The attacker may already have moved outside the reconstructed
    /// board, but the city damage and attack timestamp remain native evidence
    /// that its defence must take priority. Version one intentionally retains
    /// that broad scar-only rule for direct family comparison.
    pub(super) fn native_city_emergency(&self, g: &Game, pid: usize, cid: u32) -> bool {
        if !self.native_emergency_purchase {
            return false;
        }
        let Some(city) = g.cities.get(&cid).filter(|city| city.owner == pid) else {
            return false;
        };
        if city.hp >= CITY_MAX_HP {
            return false;
        }
        if city.last_attacked == 0
            || g.turn.saturating_sub(city.last_attacked) > EMERGENCY_RECENT_TURNS
        {
            return false;
        }
        true
    }

    /// `native-emergency-purchase-2`: a damaged city is an emergency when
    /// the host just observed its health fall, or when a still-fresh native
    /// damage record agrees with a present, visible legal attacker. The
    /// same-frame host signal remains authoritative after a siege unit fires
    /// from fog and leaves sight; an older scar still needs corroboration.
    /// Reuse the battlefront's terrain- and movement-accurate envelope rather
    /// than approximating danger with a radius or peeking through fog.
    pub(super) fn native_city_emergency_2(&self, g: &Game, pid: usize, cid: u32) -> bool {
        if !self.native_emergency_purchase_2 {
            return false;
        }
        let Some(city) = g.cities.get(&cid).filter(|city| city.owner == pid) else {
            return false;
        };
        if city.hp >= CITY_MAX_HP
            || city.last_attacked == 0
            || city.last_attacked > g.turn
            || g.turn - city.last_attacked > EMERGENCY_V2_FRESH_TURNS
        {
            return false;
        }
        if city.last_attacked == g.turn {
            return true;
        }
        let visible = g.player_vision_frame(pid);
        Self::imminent_city_attack(g, pid, cid, &visible)
    }

    /// The native emergency signal selected by the active family member.
    pub(super) fn native_city_emergency_on(&self, g: &Game, pid: usize, cid: u32) -> bool {
        self.native_city_emergency(g, pid, cid) || self.native_city_emergency_2(g, pid, cid)
    }

    /// The emergency's answer for a bleeding city: Walls if the city can raise
    /// them, otherwise the best land defender. This is
    /// `BasicAi::besieged_city_item`'s own choice without its live gate.
    pub(super) fn native_emergency_item(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        if !self.native_city_emergency_on(g, pid, cid) {
            return None;
        }
        for building in ["walls", "medieval_walls", "renaissance_walls"] {
            let wall = Item::Building {
                building: Name::new(building),
            };
            if g.can_produce(pid, cid, &wall) {
                return Some(wall);
            }
        }
        self.base
            .best_military(g, pid, cid, Some(false))
            .map(|unit| Item::Unit {
                unit: Name::new(&unit),
            })
    }
}

impl AdvancedAi {
    /// `upkeep-reserve`: the treasury an upgrade pass keeps, never below
    /// [`UPKEEP_RESERVE_TURNS`] turns of the empire's unit bill (the host's
    /// own figure when it exports one). `stock` comes back unchanged while
    /// the gene is off.
    pub(super) fn upgrade_treasury_floor(&self, g: &Game, pid: usize, stock: f64) -> f64 {
        if !self.upkeep_reserve {
            return stock;
        }
        upgrade_floor_from(stock, g.unit_gold_maintenance(pid))
    }

    /// `purchases-keep-the-upkeep-reserve`: the treasury a discretionary Gold
    /// buy leaves, never below [`UPKEEP_RESERVE_TURNS`] turns of the empire's
    /// unit bill -- the floor `upgrade_treasury_floor` keeps. `stock` comes
    /// back unchanged while the gene is off.
    pub(super) fn purchase_upkeep_floor(&self, g: &Game, pid: usize, stock: f64) -> f64 {
        // `edge-unit-takes-the-soft-city`: the upgrade bill of the bodies
        // that upgrade into the edge unit is held back from every
        // discretionary buy while its window is open (and, with
        // `edge-unit-opens-from-the-start`, while the opening is live). Zero
        // (the floor unchanged) with the gene off. See
        // `advanced/edge_unit_takes_the_soft_city.rs`.
        let held = self.edge_upgrade_gold_held(g, pid);
        let floor = if !self.purchases_keep_the_upkeep_reserve {
            stock
        } else {
            upgrade_floor_from(stock, g.unit_gold_maintenance(pid))
        };
        if held > 0.0 {
            floor.max(held)
        } else {
            floor
        }
    }

    /// `treasury-at-work-2`: the reserve `advanced_gold_spending` keeps back,
    /// given the plan's stock reserve. Returns `stock` untouched while the
    /// gene is off.
    pub(super) fn working_treasury_reserve(&self, g: &Game, pid: usize, stock: f64) -> f64 {
        if !(self.treasury_at_work_2 || self.treasury_at_work_2_2) {
            return stock;
        }
        let defender = self
            .emergency_defender_price(g, pid)
            .unwrap_or_else(|| g.game_speed.scale(FALLBACK_DEFENDER_PRICE));
        working_treasury_reserve_from(
            defender,
            g.players[pid].gold_per_turn,
            self.war_treasury_floor(g, pid),
        )
    }

    /// `treasury-at-work-2`: whether buying `item` leaves the recurring budget
    /// at or above zero. Always true while the gene is off, and for
    /// anything that is not a unit with upkeep.
    pub(super) fn treasury_purchase_stays_solvent(
        &self,
        g: &Game,
        pid: usize,
        item: &Item,
    ) -> bool {
        if !(self.treasury_at_work_2 || self.treasury_at_work_2_2) {
            return true;
        }
        let unit = match item {
            Item::Unit { unit } | Item::Formation { unit, .. } => unit,
            _ => return true,
        };
        let maintenance = g.rules.units.get(unit).map_or(0.0, |spec| spec.maintenance);
        unit_purchase_keeps_solvent(g.players[pid].gold_per_turn, maintenance)
    }

    /// `threatened-city-reserve`: the Gold an ordinary purchase must leave in
    /// the treasury while a city of ours is threatened (`plan.threatened_city`)
    /// or bleeding (`native_city_emergency_on`) — one emergency defender at
    /// today's price, [`FALLBACK_DEFENDER_PRICE`] when nothing military is
    /// unlocked. Exactly 0.0 while the gene is off and while no city is under
    /// threat, so both buyers keep their stock reserve then.
    pub(super) fn threatened_city_gold_floor(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> f64 {
        if self.threatened_city_reserve_2 {
            return self.local_defender_gold_floor(g, pid, plan);
        }
        if !self.threatened_city_reserve {
            return 0.0;
        }
        let threatened = plan
            .threatened_city
            .and_then(|cid| g.cities.get(&cid))
            .is_some_and(|city| city.owner == pid)
            || g.player_city_ids(pid)
                .into_iter()
                .any(|cid| self.native_city_emergency_on(g, pid, cid));
        if !threatened {
            return 0.0;
        }
        self.emergency_defender_price(g, pid)
            .unwrap_or_else(|| g.game_speed.scale(FALLBACK_DEFENDER_PRICE))
    }

    /// The reserve `advanced_gold_spending` keeps, lifted to
    /// [`Self::threatened_city_gold_floor`]. `stock` unchanged while the gene
    /// is off.
    pub(super) fn reserve_for_the_threatened_city(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        stock: f64,
    ) -> f64 {
        stock.max(self.threatened_city_gold_floor(g, pid, plan))
    }

    /// The Gold price of the dearest land ranged unit some city of `pid` can
    /// build now, else the dearest land melee unit; `None` with no city or
    /// nothing military unlocked. Priced at the shipped ×4 rate on the
    /// speed-scaled Production cost, deliberately without the per-city
    /// purchase-slot rule: the reserve is for the city that will be
    /// attacked, not the one whose garrison stands on the centre today.
    fn emergency_defender_price(&self, g: &Game, pid: usize) -> Option<f64> {
        let cities = g.player_city_ids(pid);
        let mut ranged: Option<f64> = None;
        let mut melee: Option<f64> = None;
        for (name, spec) in g.rules.units.iter() {
            if spec.class != "military" || matches!(spec.domain.as_deref(), Some("sea" | "air")) {
                continue;
            }
            let item = Item::Unit { unit: *name };
            if !cities.iter().any(|cid| g.can_produce(pid, *cid, &item)) {
                continue;
            }
            let price = g.item_cost_for(pid, &item) * GOLD_PER_PRODUCTION;
            let best = if spec.has_ranged_attack() {
                &mut ranged
            } else {
                &mut melee
            };
            *best = Some(best.map_or(price, |dearest| dearest.max(price)));
        }
        ranged.or(melee)
    }

    /// `treasury-at-work-2`: buy one under-bought compounding asset ahead of
    /// the purchase argmax — a Builder whenever the empire has none (on the
    /// map or at the head of a queue) and there is tile work, otherwise a
    /// Monument for a city that has none — in the city producing the least,
    /// leaving `reserve` in the bank. At most one purchase a turn; false when
    /// nothing qualifies or clears the reserve.
    pub(super) fn young_empire_purchase(&self, g: &mut Game, pid: usize, reserve: f64) -> bool {
        let counts = self.counts(g, pid);
        // Cities by Production, least first. Each yield is read once, under
        // one memo, rather than twice per comparison.
        let cities: Vec<u32> = {
            let _memo = g.query_memo();
            let mut production: Vec<_> = g
                .player_city_ids(pid)
                .into_iter()
                .map(|cid| (cid, g.city_yields(cid).production))
                .collect();
            production.sort_by(|left, right| left.1.total_cmp(&right.1).then(left.0.cmp(&right.0)));
            production.into_iter().map(|(cid, _)| cid).collect()
        };
        let bank = g.players[pid].gold;
        let city_name = |g: &Game, cid: u32| {
            g.cities
                .get(&cid)
                .map(|city| city.name.clone())
                .unwrap_or_else(|| "the empire".to_string())
        };
        if counts.builders == 0 && (self.treasury_at_work_2_2 || BasicAi::has_builder_work(g, pid))
        {
            let builder = Item::Unit {
                unit: crate::name!("builder"),
            };
            for cid in &cities {
                if self.treasury_at_work_2_2 && !self.treasury_builder_city_has_work(g, pid, *cid) {
                    continue;
                }
                let Some(price) = g.unit_purchase_cost(pid, *cid, "builder", "gold") else {
                    continue;
                };
                if bank + f64::EPSILON < reserve + price || g.purchase_is_blocked(*cid, &builder) {
                    continue;
                }
                let action = Action::Buy {
                    city: *cid,
                    unit: crate::name!("builder"),
                    formation: 0,
                    currency: "gold".to_string(),
                };
                if g.apply(pid, &action).is_err() {
                    continue;
                }
                if self.journal().wants(crate::reasoning::Level::Decision) {
                    let name = city_name(g, *cid);
                    let left = g.players[pid].gold - reserve;
                    let placement = if self.treasury_at_work_2_2 {
                        "in the lowest-production eligible city with local Builder work"
                    } else {
                        "in the city producing the least"
                    };
                    think!(self.journal(), Economy, Decision,
                        "Buying a builder for {name}, the empire having none";
                        "{price:.0} Gold {placement}; {left:.0} left \
                         above a working reserve of {reserve:.0}");
                }
                return true;
            }
        }
        let monument = Item::Building {
            building: crate::name!("monument"),
        };
        for cid in &cities {
            let city = &g.cities[cid];
            if city.buildings.iter().any(|building| building == "monument")
                || matches!(
                    city.queue.first(),
                    Some(Item::Building { building }) if building == "monument"
                )
            {
                continue;
            }
            let Some(price) = g.building_purchase_cost(pid, *cid, "monument", "gold") else {
                continue;
            };
            if bank + f64::EPSILON < reserve + price || g.purchase_is_blocked(*cid, &monument) {
                continue;
            }
            let action = Action::BuyBuilding {
                city: *cid,
                building: crate::name!("monument"),
                currency: "gold".to_string(),
            };
            if g.apply(pid, &action).is_err() {
                continue;
            }
            if self.journal().wants(crate::reasoning::Level::Decision) {
                let name = city_name(g, *cid);
                let left = g.players[pid].gold - reserve;
                think!(self.journal(), Economy, Decision,
                    "Buying a monument for {name}";
                    "{price:.0} Gold for a city without one; {left:.0} left above a \
                     working reserve of {reserve:.0}");
            }
            return true;
        }
        false
    }

    /// Locality is the city's owned tiles, not a forecast of a future route.
    /// Repairs count even when no new improvement can be built. A recent
    /// attack or current barbarian alarm keeps this civilian purchase away.
    fn treasury_builder_city_has_work(&self, g: &Game, pid: usize, cid: u32) -> bool {
        let city = &g.cities[&cid];
        if (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || self.base.barbarian_local_alarm_for_controller(g, pid, cid)
        {
            return false;
        }
        let _memo = g.query_memo();
        city.owned_tiles.iter().any(|pos| {
            let Some(tile) = g.map.get(*pos) else {
                return false;
            };
            tile.owner_city == Some(cid)
                && ((tile.pillaged && tile.improvement.is_some())
                    || g.valid_improvements(pid, *pos)
                        .iter()
                        .any(|name| g.rules.improvements[name].builder_buildable))
        })
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "purchase_upkeep_floor_tests.rs"]
mod purchase_upkeep_floor_tests;
