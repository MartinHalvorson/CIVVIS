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
//! 3. **`gold-for-the-young-city`** — a Gold purchase in a city producing
//!    less than the empire's best city earns a premium proportional to the
//!    deficit, [`YOUNG_CITY_PREMIUM`] at zero output. The same money buys the
//!    same item anywhere, and the city that cannot yet build is where it buys
//!    the most turns.
//! 4. **`native-emergency-purchase`** — the emergency defence purchase fires
//!    on a native signal: a city that has lost health, was struck within the
//!    last [`EMERGENCY_RECENT_TURNS`] turns. The confirmed damage remains
//!    authoritative if the attacker has left the current board; it buys Walls
//!    if the city can raise them, otherwise the best land defender — the live
//!    doctrine's own choice — and it spends through the reserve exactly as
//!    the live path does.
//! 5. **`native-emergency-purchase-2`** — a narrower successor to version
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
//! 5. **`treasury-at-work`** — the reserve is what an emergency costs plus
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
//!    autopsy of `civvis-20260826T112920Z` recorded from turn 85.
//! 6. **`treasury-at-work-2`** — the same reserve, and before the purchase
//!    argmax runs, one under-bought compounding asset is bought outright in
//!    the city producing the least: a Builder whenever the empire has none
//!    and there is tile work, otherwise a Monument for a city without one.
//!    One a turn, at the working reserve. This is
//!    `solvency-first-trade-slot`'s measured pattern — reserving ONE Trader
//!    priced +4.65 pp, reserving every slot −2.80 — applied to the two
//!    assets the argmax prices lowest against a Settler.
//!
//! Replayed over turns 16–52 of the live run above (`--serve --fresh-board`,
//! every other turn): the stock decider issues one purchase (a Scout at
//! t46); `treasury-at-work` issues thirteen (Archers and Warriors for the
//! threatened cities, a Settler at t40, a Granary at t50) at a reserve of
//! 70 (Slinger) then 120 (Archer) Gold; version two sixteen, the Builder in
//! the weakest city first. A fresh-board replay cannot carry a purchase
//! forward, so those counts say the genes bind, not what a game spends.
//!
//! Both are exact no-ops while off: the reserve helper returns the stock
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
/// Purchase premium for a city producing nothing; a city at half the best
/// city's output earns half of it.
pub const YOUNG_CITY_PREMIUM: f64 = 0.5;
/// How recently a city must have been struck for the native emergency.
pub const EMERGENCY_RECENT_TURNS: u32 = 4;
/// Version two only reacts while the damage is still fresh: this turn or the
/// immediately preceding turn. A longer-lived scar belongs to version one's
/// deliberately broader experiment.
pub const EMERGENCY_V2_FRESH_TURNS: u32 = 1;
/// `treasury-at-work`: turns of a recurring deficit the working reserve
/// keeps in the bank, so a purchase never turns a deficit into bankruptcy
/// before the deck or the army can be corrected.
pub const DEFICIT_COVER_TURNS: f64 = 10.0;
/// `treasury-at-work`: the Standard-speed Gold price of one emergency
/// defender when no city can name one — an Archer's 60 Production at the
/// shipped ×4 Gold purchase rate. Scaled by the game speed where it is used.
pub const FALLBACK_DEFENDER_PRICE: f64 = 240.0;
/// The Gold purchase rate per point of Production, the shipped
/// `GOLD_PURCHASE_MULTIPLIER` `unit_purchase_cost_for_formation` applies.
const GOLD_PER_PRODUCTION: f64 = 4.0;

/// The premium a purchase earns in a city producing `here` when the empire's
/// best city produces `best`. Exactly 1.0 at or above the best city.
pub fn young_city_premium_from(here: f64, best: f64) -> f64 {
    if best <= 0.0 {
        return 1.0;
    }
    let deficit = (1.0 - here / best).clamp(0.0, 1.0);
    1.0 + YOUNG_CITY_PREMIUM * deficit
}

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

    /// `gold-for-the-young-city`: the purchase premium for buying in this
    /// city rather than in the empire's best producer. Exactly 1.0 while the
    /// gene is off.
    pub(super) fn young_city_premium(&self, g: &Game, pid: usize, cid: u32) -> f64 {
        if !self.gold_for_the_young_city {
            return 1.0;
        }
        let here = g.city_yields(cid).production.max(0.0);
        let best = g
            .player_city_ids(pid)
            .into_iter()
            .map(|city| g.city_yields(city).production)
            .fold(0.0_f64, f64::max);
        young_city_premium_from(here, best)
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
    /// `treasury-at-work`: the reserve `advanced_gold_spending` keeps back,
    /// given the plan's stock reserve. Returns `stock` untouched while both
    /// versions are off.
    pub(super) fn working_treasury_reserve(&self, g: &Game, pid: usize, stock: f64) -> f64 {
        if !(self.treasury_at_work || self.treasury_at_work_2) {
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

    /// `treasury-at-work`: whether buying `item` leaves the recurring budget
    /// at or above zero. Always true while the family is off, and for
    /// anything that is not a unit with upkeep.
    pub(super) fn treasury_purchase_stays_solvent(
        &self,
        g: &Game,
        pid: usize,
        item: &Item,
    ) -> bool {
        if !(self.treasury_at_work || self.treasury_at_work_2) {
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
        let mut cities = g.player_city_ids(pid);
        cities.sort_by(|left, right| {
            g.city_yields(*left)
                .production
                .total_cmp(&g.city_yields(*right).production)
                .then(left.cmp(right))
        });
        let bank = g.players[pid].gold;
        let city_name = |g: &Game, cid: u32| {
            g.cities
                .get(&cid)
                .map(|city| city.name.clone())
                .unwrap_or_else(|| "the empire".to_string())
        };
        if counts.builders == 0 && BasicAi::has_builder_work(g, pid) {
            let builder = Item::Unit {
                unit: crate::name!("builder"),
            };
            for cid in &cities {
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
                    think!(self.journal(), Economy, Decision,
                        "Buying a builder for {name}, the empire having none";
                        "{price:.0} Gold in the city producing the least; {left:.0} left \
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Action, Game};

    fn board() -> (Game, u32) {
        let mut g = Game::new(2, 24, 16, 71, 250, 0);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        g.current = 0;
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = g.player_city_ids(0)[0];
        (g, city)
    }

    fn settler() -> Item {
        Item::Unit {
            unit: crate::name!("settler"),
        }
    }

    fn monument() -> Item {
        Item::Building {
            building: crate::name!("monument"),
        }
    }

    #[test]
    fn off_by_default_and_toggles() {
        let ai = AdvancedAi::new();
        assert!(!ai.buy_what_cards_cannot_boost, "an opt-in ships off");
        assert!(!ai.build_what_cards_boost, "an opt-in ships off");
        assert!(!ai.gold_for_the_young_city, "an opt-in ships off");
        assert!(!ai.native_emergency_purchase, "an opt-in ships off");
        assert!(!ai.native_emergency_purchase_2, "a successor ships off");
        let legacy = AdvancedAi::legacy();
        assert!(!legacy.buy_what_cards_cannot_boost);
        assert!(!legacy.build_what_cards_boost);
        assert!(!legacy.gold_for_the_young_city);
        assert!(!legacy.native_emergency_purchase);
        assert!(!legacy.native_emergency_purchase_2);

        let mut ai = AdvancedAi::new();
        ai.enable_buy_what_cards_cannot_boost();
        ai.enable_build_what_cards_boost();
        ai.enable_gold_for_the_young_city();
        ai.enable_native_emergency_purchase();
        assert!(ai.buy_what_cards_cannot_boost);
        assert!(ai.build_what_cards_boost);
        assert!(ai.gold_for_the_young_city);
        assert!(ai.native_emergency_purchase);
        assert!(!ai.native_emergency_purchase_2);
        ai.enable_native_emergency_purchase_2();
        assert!(!ai.native_emergency_purchase, "one emergency version plays");
        assert!(ai.native_emergency_purchase_2);
        ai.enable_native_emergency_purchase();
        assert!(ai.native_emergency_purchase);
        assert!(
            !ai.native_emergency_purchase_2,
            "version one also selects its family"
        );
        ai.disable_buy_what_cards_cannot_boost();
        ai.disable_build_what_cards_boost();
        ai.disable_gold_for_the_young_city();
        ai.disable_native_emergency_purchase();
        ai.disable_native_emergency_purchase_2();
        assert!(!ai.buy_what_cards_cannot_boost);
        assert!(!ai.build_what_cards_boost);
        assert!(!ai.gold_for_the_young_city);
        assert!(!ai.native_emergency_purchase);
        assert!(!ai.native_emergency_purchase_2);
    }

    /// `threatened-city-reserve`: the same run bought a Water Mill at t160
    /// with Aquileia named under threat and 399 Gold in hand (a Line
    /// Infantry cost 360), and a Market in Aquileia itself at t162 with the
    /// walls half down. The floor is one emergency defender while a city is
    /// threatened or bleeding, and exactly zero otherwise.
    #[test]
    fn the_treasury_keeps_one_defender_while_a_city_is_threatened() {
        use super::super::GrandStrategy;
        let (mut g, city) = board();
        let threatened = StrategicPlan {
            strategy: GrandStrategy::Expansion,
            target_player: None,
            target_city: None,
            threatened_city: Some(city),
            desired_cities: 3,
            assessed_turn: g.turn,
            rush: false,
        };
        let calm = StrategicPlan {
            threatened_city: None,
            ..threatened.clone()
        };

        let off = AdvancedAi::new();
        assert!(!off.threatened_city_reserve, "a repair gene ships off");
        assert!(!AdvancedAi::legacy().threatened_city_reserve);
        assert_eq!(off.threatened_city_gold_floor(&g, 0, &threatened), 0.0);
        assert_eq!(
            off.reserve_for_the_threatened_city(&g, 0, &threatened, 125.0),
            125.0
        );

        let mut on = AdvancedAi::new();
        on.enable_threatened_city_reserve();
        assert!(on.threatened_city_reserve);
        assert_eq!(
            on.threatened_city_gold_floor(&g, 0, &calm),
            0.0,
            "no threat, no floor"
        );
        let floor = on.threatened_city_gold_floor(&g, 0, &threatened);
        let defender = on
            .emergency_defender_price(&g, 0)
            .unwrap_or_else(|| g.game_speed.scale(FALLBACK_DEFENDER_PRICE));
        assert!(
            floor > 0.0,
            "a threatened city holds a defender's price back"
        );
        assert_eq!(floor, defender);
        assert_eq!(
            on.reserve_for_the_threatened_city(&g, 0, &threatened, 1.0),
            floor
        );
        assert_eq!(
            on.reserve_for_the_threatened_city(&g, 0, &threatened, floor + 1_000.0),
            floor + 1_000.0,
            "a stock reserve already above the floor stands"
        );

        // A bleeding city with no named threat is the other trigger, through
        // `native-emergency-purchase`'s own confirmed-damage signal.
        g.turn = 10;
        {
            let bleeding = g.cities.get_mut(&city).unwrap();
            bleeding.hp = 150;
            bleeding.last_attacked = 9;
        }
        assert_eq!(
            on.threatened_city_gold_floor(&g, 0, &calm),
            0.0,
            "unseen without the signal"
        );
        on.enable_native_emergency_purchase();
        assert_eq!(on.threatened_city_gold_floor(&g, 0, &calm), defender);
        on.disable_threatened_city_reserve();
        assert_eq!(on.threatened_city_gold_floor(&g, 0, &threatened), 0.0);
    }

    #[test]
    fn the_engine_prices_colonization_and_the_controller_reads_it() {
        let (mut g, city) = board();
        assert_eq!(g.item_prod_mult(0, city, Some(&settler())), 1.0);
        g.players[0].policies.insert(crate::name!("colonization"));
        assert!(
            (g.item_prod_mult(0, city, Some(&settler())) - 1.5).abs() < 1e-9,
            "Colonization is +50% toward Settlers"
        );
        assert_eq!(g.item_prod_mult(0, city, Some(&monument())), 1.0);

        let off = AdvancedAi::new();
        assert_eq!(off.purchase_card_multiplier(&g, 0, city, &settler()), 1.0);
        let mut on = AdvancedAi::new();
        on.enable_buy_what_cards_cannot_boost();
        assert!((on.purchase_card_multiplier(&g, 0, city, &settler()) - 1.5).abs() < 1e-9);
        assert_eq!(on.purchase_card_multiplier(&g, 0, city, &monument()), 1.0);
    }

    #[test]
    fn the_governor_leans_toward_the_boosted_item_and_only_upward() {
        let (mut g, city) = board();
        g.players[0].policies.insert(crate::name!("colonization"));
        let off = AdvancedAi::new();
        assert_eq!(
            off.card_boosted_value(&g, 0, city, &settler(), 100.0),
            100.0
        );
        let mut on = AdvancedAi::new();
        on.enable_build_what_cards_boost();
        assert!(
            (on.card_boosted_value(&g, 0, city, &settler(), 100.0) - 125.0).abs() < 1e-9,
            "half of a +50% card"
        );
        assert_eq!(
            on.card_boosted_value(&g, 0, city, &monument(), 100.0),
            100.0
        );
        assert_eq!(
            on.card_boosted_value(&g, 0, city, &settler(), -100.0),
            -100.0
        );
        assert_eq!(on.card_boosted_value(&g, 0, city, &settler(), 0.0), 0.0);
    }

    #[test]
    fn the_young_city_premium_scales_with_the_deficit() {
        assert_eq!(young_city_premium_from(10.0, 10.0), 1.0);
        assert_eq!(young_city_premium_from(12.0, 10.0), 1.0, "never below one");
        assert!((young_city_premium_from(5.0, 10.0) - 1.25).abs() < 1e-9);
        assert!((young_city_premium_from(0.0, 10.0) - 1.5).abs() < 1e-9);
        assert_eq!(
            young_city_premium_from(0.0, 0.0),
            1.0,
            "no producer, no premium"
        );
        let (g, city) = board();
        let off = AdvancedAi::new();
        assert_eq!(off.young_city_premium(&g, 0, city), 1.0);
        let mut on = AdvancedAi::new();
        on.enable_gold_for_the_young_city();
        assert_eq!(
            on.young_city_premium(&g, 0, city),
            1.0,
            "the only city is the best city"
        );
    }

    #[test]
    fn the_native_emergency_needs_damage_and_a_recent_strike() {
        let (mut g, city) = board();
        g.turn = 30;

        let mut on = AdvancedAi::new();
        on.enable_native_emergency_purchase();
        assert!(
            !on.native_city_emergency(&g, 0, city),
            "an unhurt city is not an emergency"
        );

        let state = g.cities.get_mut(&city).unwrap();
        state.hp = 120;
        state.last_attacked = 29;
        assert!(on.native_city_emergency(&g, 0, city));
        let item = on
            .native_emergency_item(&g, 0, city)
            .expect("a recently damaged city has an answer after its attacker leaves sight");
        assert!(
            matches!(item, Item::Building { .. } | Item::Unit { .. }),
            "walls or a land defender: {item:?}"
        );

        let off = AdvancedAi::new();
        assert!(!off.native_city_emergency(&g, 0, city), "off is off");
        assert!(off.native_emergency_item(&g, 0, city).is_none());

        g.cities.get_mut(&city).unwrap().last_attacked = 20;
        assert!(
            !on.native_city_emergency(&g, 0, city),
            "the strike is stale"
        );
        g.cities.get_mut(&city).unwrap().last_attacked = 29;
        assert!(
            on.native_city_emergency(&g, 0, city),
            "a freshly struck city stays an emergency even without a visible attacker"
        );
    }

    #[test]
    fn native_emergency_v2_accepts_a_same_frame_host_hit_or_visible_legal_attacker() {
        let (mut g, city) = board();
        g.turn = 30;
        let city_pos = g.cities[&city].pos;
        {
            let state = g.cities.get_mut(&city).unwrap();
            state.buildings.push(crate::name!("walls"));
            state.wall_hp = 100;
            state.hp = 120;
            state.last_attacked = 29;
        }

        let mut on = AdvancedAi::new();
        on.enable_native_emergency_purchase_2();
        assert!(on.native_emergency_purchase_on());
        assert!(
            !on.native_city_emergency_2(&g, 0, city),
            "a recent scar without a current attacker is not an emergency"
        );
        assert!(on.native_emergency_item(&g, 0, city).is_none());

        g.cities.get_mut(&city).unwrap().last_attacked = g.turn;
        assert!(
            on.native_city_emergency_2(&g, 0, city),
            "a host-confirmed same-frame hit remains an emergency after the attacker leaves sight"
        );
        assert!(
            on.native_emergency_item(&g, 0, city).is_some(),
            "the direct host signal reaches the existing walls-or-defender choice"
        );
        g.cities.get_mut(&city).unwrap().last_attacked = 29;
        for unit in g.player_unit_ids(0) {
            g.remove_unit(unit);
        }

        let attack_tile = g
            .nbrs(city_pos)
            .into_iter()
            .find(|position| {
                g.city_at(*position).is_none()
                    && g.unit_ids_at(*position).is_empty()
                    && g.map
                        .get(*position)
                        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            })
            .expect("the city needs an accessible attack tile");
        g.at_war.insert((0, 1));
        let attacker = g.spawn_test_unit("warrior", 1, attack_tile);
        assert!(g.player_can_see(0, attack_tile), "the attacker is observed");
        assert!(
            g.attack_reach(attacker).contains(&city_pos),
            "the attacker can legally strike the City Center"
        );
        assert!(on.native_city_emergency_2(&g, 0, city));
        assert!(
            on.native_emergency_item(&g, 0, city).is_some(),
            "the corroborated signal reaches the existing walls-or-defender choice"
        );
        let plan = StrategicPlan {
            strategy: super::super::GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: Some(city),
            desired_cities: 1,
            assessed_turn: g.turn,
            rush: false,
        };
        let mut purchase = g.clone();
        purchase.players[0].gold = 1_000.0;
        let before_purchase = purchase.players[0].gold;
        assert!(
            on.emergency_city_defense_purchase(&mut purchase, 0, &plan),
            "version two opens the existing emergency purchase path"
        );
        assert!(
            purchase.players[0].gold < before_purchase,
            "the corroborated emergency spends Gold on its local answer"
        );

        g.remove_unit(attacker);
        assert!(
            !on.native_city_emergency_2(&g, 0, city),
            "a departing attacker clears version two immediately"
        );

        let attacker = g.spawn_test_unit("warrior", 1, attack_tile);
        g.cities.get_mut(&city).unwrap().last_attacked = 28;
        assert!(
            !on.native_city_emergency_2(&g, 0, city),
            "even a current attacker cannot revive a two-turn-old scar"
        );
        assert!(g.attack_reach(attacker).contains(&city_pos));
    }

    #[test]
    fn the_treasury_genes_ship_off_and_toggle() {
        let ai = AdvancedAi::new();
        assert!(!ai.treasury_at_work, "an opt-in ships off");
        assert!(!ai.treasury_at_work_2, "an opt-in ships off");
        let mut ai = AdvancedAi::new();
        ai.enable_treasury_at_work();
        ai.enable_treasury_at_work_2();
        assert!(ai.treasury_at_work && ai.treasury_at_work_2);
        ai.disable_treasury_at_work();
        ai.disable_treasury_at_work_2();
        assert!(!ai.treasury_at_work && !ai.treasury_at_work_2);
    }

    #[test]
    fn the_working_reserve_is_a_defender_plus_ten_turns_of_deficit() {
        // Solvent: the defender alone, whatever the surplus.
        assert_eq!(working_treasury_reserve_from(120.0, 7.0, 0.0), 120.0);
        assert_eq!(working_treasury_reserve_from(120.0, 0.0, 0.0), 120.0);
        // Insolvent: ten turns of the deficit on top.
        assert_eq!(working_treasury_reserve_from(120.0, -8.0, 0.0), 200.0);
        // An appointed war's bill is a floor, never an addend.
        assert_eq!(working_treasury_reserve_from(120.0, -8.0, 500.0), 500.0);
        assert_eq!(working_treasury_reserve_from(120.0, 7.0, 119.0), 120.0);
    }

    #[test]
    fn a_unit_purchase_must_keep_the_recurring_budget_at_or_above_zero() {
        assert!(unit_purchase_keeps_solvent(7.0, 1.0));
        assert!(unit_purchase_keeps_solvent(1.0, 1.0));
        assert!(!unit_purchase_keeps_solvent(0.0, 1.0));
        assert!(!unit_purchase_keeps_solvent(-3.0, 1.0));
        // Free upkeep is always solvent, however deep the deficit.
        assert!(unit_purchase_keeps_solvent(-30.0, 0.0));

        let (mut g, _city) = board();
        let archer = Item::Unit {
            unit: crate::name!("archer"),
        };
        let upkeep = g.rules.units["archer"].maintenance;
        assert!(upkeep > 0.0, "an Archer costs upkeep in the shipped rules");
        let monument = Item::Building {
            building: crate::name!("monument"),
        };
        // Off: never consulted.
        g.players[0].gold_per_turn = -30.0;
        assert!(AdvancedAi::new().treasury_purchase_stays_solvent(&g, 0, &archer));
        // On: the unit is declined at a deficit and allowed with the income
        // to carry it; a building is never a unit.
        let mut ai = AdvancedAi::new();
        ai.enable_treasury_at_work();
        assert!(!ai.treasury_purchase_stays_solvent(&g, 0, &archer));
        assert!(ai.treasury_purchase_stays_solvent(&g, 0, &monument));
        g.players[0].gold_per_turn = upkeep;
        assert!(ai.treasury_purchase_stays_solvent(&g, 0, &archer));
        g.players[0].gold_per_turn = upkeep - 0.5;
        assert!(!ai.treasury_purchase_stays_solvent(&g, 0, &archer));
    }

    #[test]
    fn off_the_reserve_is_the_stock_value_and_on_it_is_a_defender() {
        let (g, _city) = board();
        let ai = AdvancedAi::new();
        assert_eq!(
            ai.working_treasury_reserve(&g, 0, 325.0),
            325.0,
            "off: untouched"
        );

        let mut ai = AdvancedAi::new();
        ai.enable_treasury_at_work();
        let reserve = ai.working_treasury_reserve(&g, 0, 325.0);
        // A fresh capital can build a Slinger, the ancient ranged unit, and
        // the reserve is exactly its Gold price: no city count anywhere.
        let slinger = Item::Unit {
            unit: crate::name!("slinger"),
        };
        assert!(g.can_produce(0, g.player_city_ids(0)[0], &slinger));
        let expected = g.item_cost_for(0, &slinger) * GOLD_PER_PRODUCTION;
        assert!(expected > 0.0);
        assert_eq!(
            reserve, expected,
            "one ranged defender, {reserve} vs {expected}"
        );
        assert!(reserve < 325.0, "far below the plan's 250 + 75 per city");

        // Version two shares the reserve law.
        let mut v2 = AdvancedAi::new();
        v2.enable_treasury_at_work_2();
        assert_eq!(v2.working_treasury_reserve(&g, 0, 325.0), expected);
    }

    #[test]
    fn a_deficit_raises_the_working_reserve_by_ten_turns_of_it() {
        let (mut g, _city) = board();
        let mut ai = AdvancedAi::new();
        ai.enable_treasury_at_work();
        let solvent = ai.working_treasury_reserve(&g, 0, 325.0);
        g.players[0].gold_per_turn = -6.0;
        let insolvent = ai.working_treasury_reserve(&g, 0, 325.0);
        assert_eq!(insolvent, solvent + DEFICIT_COVER_TURNS * 6.0);
    }

    #[test]
    fn without_a_city_the_reserve_is_the_scaled_fallback_defender() {
        let g = Game::new(2, 24, 16, 71, 250, 0);
        assert!(g.player_city_ids(0).is_empty());
        let mut ai = AdvancedAi::new();
        ai.enable_treasury_at_work();
        assert_eq!(
            ai.working_treasury_reserve(&g, 0, 325.0),
            g.game_speed.scale(FALLBACK_DEFENDER_PRICE)
        );
    }

    #[test]
    fn version_two_buys_the_first_builder_then_a_monument_and_never_the_reserve() {
        let (mut g, city) = board();
        let mut ai = AdvancedAi::new();
        ai.enable_treasury_at_work_2();
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        // A building already in the queue is not for sale, and the fresh
        // capital is seeded with its Monument: clear both so the fixture
        // has one to want.
        let capital = g.cities.get_mut(&city).unwrap();
        capital.queue.clear();
        capital.buildings.retain(|building| building != "monument");
        let builder_price = g
            .unit_purchase_cost(0, city, "builder", "gold")
            .expect("a builder is buyable");
        let monument_price = g
            .building_purchase_cost(0, city, "monument", "gold")
            .expect("a monument is buyable");
        let reserve = 100.0;

        // Short of the reserve by one Gold: nothing is bought.
        g.players[0].gold = reserve + builder_price - 1.0;
        assert!(!ai.young_empire_purchase(&mut g, 0, reserve));
        assert_eq!(g.players[0].gold, reserve + builder_price - 1.0);

        // At the reserve: the empire's first Builder, one purchase a turn.
        g.players[0].gold = reserve + builder_price + monument_price;
        assert!(ai.young_empire_purchase(&mut g, 0, reserve));
        let builders = g
            .player_unit_ids(0)
            .into_iter()
            .filter(|uid| g.units[uid].kind == "builder")
            .count();
        assert_eq!(builders, 1, "one Builder bought");
        assert!(g.players[0].gold >= reserve, "the reserve is never spent");
        assert!(
            !g.cities[&city].buildings.iter().any(|b| b == "monument"),
            "one a turn"
        );

        // Next turn, with a Builder on the map, the Monument.
        assert!(ai.young_empire_purchase(&mut g, 0, reserve));
        assert!(g.cities[&city].buildings.iter().any(|b| b == "monument"));
        assert!(g.players[0].gold >= reserve);
        // Nothing under-bought remains: the ladder stands down.
        g.players[0].gold = 10_000.0;
        assert!(!ai.young_empire_purchase(&mut g, 0, reserve));
        let _ = builder;
    }
}
