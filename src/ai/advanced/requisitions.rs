//! Requisitions: the Objective Board's shortfall reaches production and the
//! treasury. Opt-in gene `requisitions` (field `requisitions`), off, an exact
//! no-op when off, and inert without `objective-board` — there is no board
//! to read.
//!
//! **What the shipped layer does.** `production_value` sizes the army by a
//! headcount — `city_count`, doubled at war — and every military unit is
//! chosen by `BasicAi::best_military`, the strongest producible unit of a
//! role with no regard to what it costs. The city-defence purchases run
//! their own detectors: `besieged_city_item` for a bleeding city,
//! `border_parity_*` for the strongest bordering major, each with its own
//! reserve and its own unit choice, none of them knowing what the army in
//! the field is short of. The board (`objective_board.rs`) knows — it
//! publishes `requisitions()`, the unmet `ForceNeed` per row — and until
//! this gene nothing read it.
//!
//! **What this does.**
//!
//! - **Production** ([`AdvancedAi::requisition_production_item`]): ahead of
//!   every economic reserve item in `advanced_production`, an idle city
//!   starts the unit a requisition asks of it — the row's nearest city, or
//!   the nearest idle city that can build the kind when that one is busy or
//!   cannot. The kind follows the unmet need: siege while the row lacks
//!   siege, ranged while it lacks ranged, melee while it lacks melee; a bare
//!   strength shortfall asks a shooter for a city (Defend, Relieve, Deter —
//!   the city-defence doctrine seats a shooter on the centre) and balances
//!   melee against ranged in the field (Siege, Destroy, ClearCamp, Escort),
//!   cavalry for a Destroy that falls due within [`CAVALRY_DEADLINE`] turns,
//!   a ship for a row only sea units serve. Within the kind the unit is the
//!   best **worth per hammer**, not the strongest: worth is
//!   e^([`WORTH_PER_STRENGTH`]·strength), the two-sided exchange the damage
//!   formula (30·e^{0.04Δ}) gives ten points of strength, over the
//!   production cost — so a Swordsman outbids a Warrior and a Crossbowman an
//!   Archer once the exchange gap outgrows the cost gap, and never the other
//!   way. One unit per city per turn (the city is no longer idle); units
//!   already queued for the kind count against the request in rank order,
//!   so a shortfall of two does not start six.
//! - **Gold** ([`AdvancedAi::requisition_purchase`]): after the emergency
//!   city-defence purchase, the highest-ranked requisition whose unit the
//!   treasury covers above the reserve is bought at its city — or the
//!   nearest city where the purchase is legal — one a turn.
//! - **Routing**: with the gene on, `border_parity_purchase` and the
//!   `border-parity-2` idle-queue block stand down (the Deter row's
//!   requisition is the same gap at the same city — `objective_board.rs`
//!   raises it under this gene), `border_parity_production`'s severe-deficit
//!   pre-emption takes its city and its unit from the Deter requisition, and
//!   `emergency_city_defense_purchase` takes a bleeding city's unit from its
//!   Defend requisition (the wall answer is untouched — the board does not
//!   model walls). So the board is the single source of what is raised for
//!   the war.
//! - **Composition** ([`AdvancedAi::requisition_army_target`]): when the
//!   land military is under the board's summed need — bodies in the land
//!   forces plus bodies requisitioned — `production_value`'s
//!   `desired_military` is that need, never over
//!   [`ARMY_TARGET_CAP_PER_CITY`] a city, instead of the headcount.
//!
//! One "Cities/Decision" line per start or purchase (`Requisition: 2 ranged
//! for Defend Aquileia by t92, Rome builds Archer`); `StrategyCensus` gains
//! `requisition_items`, `requisition_purchases` and `requisition_unserved`
//! (open requisitions no city of ours could build for, per turn).

use std::collections::BTreeMap;

use super::objective_board::{ObjectiveKey, ObjectiveKind, Requisition};
use super::{AdvancedAi, BasicAi, ForceDomain, StrategicPlan};
use crate::game::{Action, Game, Item};
use crate::name::Name;
use crate::reasoning::plain;
use crate::rules::UnitSpec;
use crate::think;

/// A Deter requisition asks at most this many bodies.
pub const DETER_REQUISITION_MAX: usize = 3;
/// A Destroy row due within this many turns asks cavalry: the body must
/// arrive before the row is gone.
pub const CAVALRY_DEADLINE: u32 = 3;
/// The board never sizes the army over this many land units a city.
pub const ARMY_TARGET_CAP_PER_CITY: usize = 4;
/// Ten points of strength are worth e^(10·this) in the exchange, both ways:
/// the damage formula's 30·e^{0.04Δ} dealt and its inverse taken.
pub const WORTH_PER_STRENGTH: f64 = 0.08;

/// What kind of unit a shortfall asks production for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnitRole {
    /// Melee and anti-cavalry on foot.
    Melee,
    /// A shooter that is not siege.
    Ranged,
    Siege,
    /// Light and heavy cavalry.
    Cavalry,
    /// A ship with a blow.
    Naval,
}

impl UnitRole {
    pub fn as_str(self) -> &'static str {
        match self {
            UnitRole::Melee => "melee",
            UnitRole::Ranged => "ranged",
            UnitRole::Siege => "siege",
            UnitRole::Cavalry => "cavalry",
            UnitRole::Naval => "naval",
        }
    }

    /// The roles tried, in order, when no unit of this one can be built.
    fn fallbacks(self) -> &'static [UnitRole] {
        match self {
            UnitRole::Melee => &[UnitRole::Ranged, UnitRole::Cavalry],
            UnitRole::Ranged => &[UnitRole::Melee, UnitRole::Cavalry],
            UnitRole::Siege => &[UnitRole::Ranged, UnitRole::Melee],
            UnitRole::Cavalry => &[UnitRole::Melee, UnitRole::Ranged],
            UnitRole::Naval => &[],
        }
    }

    /// Whether a unit plays the role.
    pub fn fits(self, spec: &UnitSpec) -> bool {
        if spec.class != "military"
            || spec.promotion_class == "recon"
            || spec.domain.as_deref() == Some("air")
        {
            return false;
        }
        let sea = spec.domain.as_deref() == Some("sea");
        let mounted = matches!(
            spec.promotion_class.as_str(),
            "light_cavalry" | "heavy_cavalry"
        );
        let ranged = spec.has_ranged_attack();
        match self {
            UnitRole::Naval => sea && (ranged || spec.is_melee_capable()),
            UnitRole::Siege => !sea && ranged && spec.siege,
            UnitRole::Ranged => !sea && ranged && !spec.siege,
            UnitRole::Melee => !sea && !ranged && spec.is_melee_capable() && !mounted,
            UnitRole::Cavalry => !sea && !ranged && spec.is_melee_capable() && mounted,
        }
    }

    /// The strength the role fights with.
    fn power(self, spec: &UnitSpec) -> f64 {
        match self {
            UnitRole::Ranged | UnitRole::Siege | UnitRole::Naval => {
                spec.ranged_attack_strength().max(spec.strength)
            }
            UnitRole::Melee | UnitRole::Cavalry => spec.strength,
        }
    }

    /// The role a shortfall asks for; `None` for a row production does not
    /// serve (Recon — the exploration governor buys scouts).
    pub fn of(requisition: &Requisition, turn: u32) -> Option<UnitRole> {
        if requisition.kind == ObjectiveKind::Recon {
            return None;
        }
        if requisition.sea_only {
            return Some(UnitRole::Naval);
        }
        let unmet = &requisition.unmet;
        if unmet.siege > 0 {
            return Some(UnitRole::Siege);
        }
        if unmet.ranged > 0 {
            return Some(UnitRole::Ranged);
        }
        if unmet.melee > 0 {
            return Some(UnitRole::Melee);
        }
        let have = &requisition.have;
        Some(match requisition.kind {
            // A city wants a shooter on its centre first, then bodies.
            ObjectiveKind::Defend | ObjectiveKind::Relieve | ObjectiveKind::Deter => {
                if have.ranged <= have.melee {
                    UnitRole::Ranged
                } else {
                    UnitRole::Melee
                }
            }
            ObjectiveKind::Destroy
                if requisition
                    .by_turn
                    .is_some_and(|by| by <= turn.saturating_add(CAVALRY_DEADLINE)) =>
            {
                UnitRole::Cavalry
            }
            _ => {
                if have.melee <= have.ranged {
                    UnitRole::Melee
                } else {
                    UnitRole::Ranged
                }
            }
        })
    }
}

/// Worth per hammer of a unit in a role: e^(WORTH_PER_STRENGTH·power) over
/// its production cost.
pub fn worth_per_hammer(role: UnitRole, spec: &UnitSpec) -> f64 {
    (WORTH_PER_STRENGTH * role.power(spec)).exp() / spec.cost.max(1.0)
}

/// What a requisition resolved to at a city, for the record.
#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub requisition: Requisition,
    pub role: UnitRole,
    pub unit: Name,
    pub city: u32,
}

impl Order {
    /// `Requisition: 2 ranged for Defend Aquileia by t92`.
    pub fn headline(&self) -> String {
        let by = self
            .requisition
            .by_turn
            .map(|turn| format!(" by t{turn}"))
            .unwrap_or_default();
        format!(
            "Requisition: {} {} for {} {}{by}",
            self.requisition.count,
            self.role.as_str(),
            self.requisition.kind.as_str(),
            self.requisition.label
        )
    }
}

/// What this turn's purchases already answered, kept on the controller
/// behind a `RefCell` because the purchase pass borrows the controller
/// immutably. Production starts need no record: the queue is the record.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Served {
    turn: u32,
    /// Bodies bought this turn, per `(row kind, row city)`.
    bought: BTreeMap<(ObjectiveKind, Option<u32>), usize>,
    /// Purchases not yet moved into the census by the production pass.
    purchases_uncounted: u32,
}

impl Served {
    fn this_turn(&mut self, turn: u32) -> &mut Self {
        if self.turn != turn {
            self.turn = turn;
            self.bought.clear();
        }
        self
    }

    fn bought_this_turn(&self, turn: u32) -> bool {
        self.turn == turn && self.bought.values().any(|count| *count > 0)
    }

    fn bought_for(&self, turn: u32, requisition: &Requisition) -> usize {
        if self.turn != turn {
            return 0;
        }
        self.bought
            .get(&(requisition.kind, requisition.city))
            .copied()
            .unwrap_or(0)
    }
}

impl AdvancedAi {
    /// The gene is live: on, and the board it reads is on.
    pub(super) fn requisitions_on(&self) -> bool {
        self.requisitions && self.objective_board
    }

    /// The best unit of `role` this city can build now, by worth per hammer;
    /// the role's fallbacks when it has none.
    pub(super) fn requisition_unit(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        role: UnitRole,
    ) -> Option<(UnitRole, Name)> {
        std::iter::once(role)
            .chain(role.fallbacks().iter().copied())
            .find_map(|role| {
                let mut best: Option<(f64, f64, Name)> = None;
                for (name, spec) in &g.rules.units {
                    if !role.fits(spec) || !g.can_produce(pid, cid, &Item::Unit { unit: *name }) {
                        continue;
                    }
                    let worth = worth_per_hammer(role, spec);
                    let power = role.power(spec);
                    let better = best.is_none_or(|(had, had_power, had_name)| {
                        worth > had + 1e-12
                            || ((worth - had).abs() <= 1e-12
                                && (power > had_power
                                    || (power == had_power && name.as_str() < had_name.as_str())))
                    });
                    if better {
                        best = Some((worth, power, *name));
                    }
                }
                best.map(|(_, _, name)| (role, name))
            })
    }

    /// Units of `role` at the head of a queue of ours: they count against a
    /// request, so the same shortfall is not started in every city.
    fn requisition_pipeline(g: &Game, pid: usize, role: UnitRole) -> usize {
        g.player_city_ids(pid)
            .into_iter()
            .filter(|cid| match g.cities[cid].queue.first() {
                Some(Item::Unit { unit }) | Some(Item::Formation { unit, .. }) => {
                    g.rules.units.get(unit).is_some_and(|spec| role.fits(spec))
                }
                _ => false,
            })
            .count()
    }

    /// The requisitions still open this turn, in board rank order, each with
    /// the role it asks: what the board published, less this turn's
    /// purchases and the units already queued for the kind (credited to the
    /// higher-ranked rows first).
    fn open_requisitions(&self, g: &Game, pid: usize) -> Vec<(Requisition, UnitRole)> {
        let served = self.requisitions_served.borrow();
        let mut pipeline: BTreeMap<UnitRole, usize> = BTreeMap::new();
        let mut open = Vec::new();
        for requisition in self.requisitions() {
            let Some(role) = UnitRole::of(&requisition, g.turn) else {
                continue;
            };
            let mut wanted = requisition
                .count
                .saturating_sub(served.bought_for(g.turn, &requisition));
            let queued = pipeline
                .entry(role)
                .or_insert_with(|| Self::requisition_pipeline(g, pid, role));
            let credited = wanted.min(*queued);
            *queued -= credited;
            wanted -= credited;
            if wanted > 0 {
                open.push((requisition, role));
            }
        }
        open
    }

    /// Whether `cid` serves `requisition`: the row's own nearest city when
    /// it is idle and can build the kind, else the nearest idle city that
    /// can. The unit it would build.
    fn requisition_serving_city(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        requisition: &Requisition,
        role: UnitRole,
    ) -> Option<(UnitRole, Name)> {
        let named = requisition.city?;
        let at = g.cities.get(&named)?.pos;
        let can_serve = |city: u32| -> Option<(UnitRole, Name)> {
            if role == UnitRole::Naval && !BasicAi::city_is_coastal(g, city) {
                return None;
            }
            self.requisition_unit(g, pid, city, role)
        };
        if city_idle(g, named) {
            if let Some(unit) = can_serve(named) {
                return (named == cid).then_some(unit);
            }
        }
        // The named city is busy or cannot build the kind: the nearest idle
        // city that can.
        let mut cities: Vec<u32> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|city| *city != named && city_idle(g, *city))
            .collect();
        cities.sort_by_key(|city| (g.wdist(g.cities[city].pos, at), *city));
        let (serving, unit) = cities
            .into_iter()
            .find_map(|city| can_serve(city).map(|unit| (city, unit)))?;
        (serving == cid).then_some(unit)
    }

    /// The production item a requisition asks of this idle city, if any;
    /// the caller applies it and reports with
    /// [`AdvancedAi::requisition_started`].
    pub(super) fn requisition_production_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> Option<Order> {
        if !self.requisitions_on() || !city_idle(g, cid) {
            return None;
        }
        self.open_requisitions(g, pid)
            .into_iter()
            .find_map(|(requisition, role)| {
                let (role, unit) =
                    self.requisition_serving_city(g, pid, cid, &requisition, role)?;
                Some(Order {
                    requisition,
                    role,
                    unit,
                    city: cid,
                })
            })
    }

    /// The order was applied: count it and write the line.
    pub(super) fn requisition_started(&mut self, g: &Game, order: &Order) {
        self.census.requisition_items += 1;
        if self.journal().wants(crate::reasoning::Level::Decision) {
            let city_name = g.cities[&order.city].name.clone();
            think!(self.journal(), Cities, Decision,
                "{}, {city_name} builds {}", order.headline(), plain(order.unit.as_str());
                "the Objective Board's {} row is short {} {} unit(s), strength {:.0} of {:.0} in hand; \
                 the unit is the best worth per hammer of its kind this city can build",
                order.requisition.kind.as_str(), order.requisition.count, order.role.as_str(),
                order.requisition.have.strength,
                order.requisition.have.strength + order.requisition.unmet.strength);
        }
    }

    /// Buy the highest-ranked open requisition the treasury covers above
    /// `reserve`, at its city or the nearest city where the purchase is
    /// legal. One a turn. Exact no-op with the gene off.
    pub(super) fn requisition_purchase(
        &self,
        g: &mut Game,
        pid: usize,
        reserve: f64,
    ) -> Option<Order> {
        if !self.requisitions_on() || self.requisitions_served.borrow().bought_this_turn(g.turn) {
            return None;
        }
        let bank = g.players[pid].gold;
        let purchases = self.legal_purchase_actions(g, pid);
        for (requisition, role) in self.open_requisitions(g, pid) {
            let Some(at) = requisition
                .city
                .and_then(|named| g.cities.get(&named))
                .map(|city| city.pos)
            else {
                continue;
            };
            let mut cities: Vec<u32> = g.player_city_ids(pid);
            cities.sort_by_key(|city| (g.wdist(g.cities[city].pos, at), *city));
            for city in cities {
                if role == UnitRole::Naval && !BasicAi::city_is_coastal(g, city) {
                    continue;
                }
                let Some((role, unit)) = self.requisition_unit(g, pid, city, role) else {
                    continue;
                };
                let Some(cost) = g.unit_purchase_cost(pid, city, unit.as_str(), "gold") else {
                    continue;
                };
                if bank + f64::EPSILON < reserve + cost {
                    continue;
                }
                let legal = purchases.iter().any(|action| {
                    matches!(
                        action,
                        Action::Buy { city: buyer, unit: bought, formation, currency }
                            if *buyer == city && *formation == 0 && currency == "gold" && *bought == unit
                    )
                });
                if !legal {
                    continue;
                }
                let action = Action::Buy {
                    city,
                    unit,
                    formation: 0,
                    currency: "gold".to_string(),
                };
                if g.apply(pid, &action).is_err() {
                    continue;
                }
                let order = Order {
                    requisition,
                    role,
                    unit,
                    city,
                };
                {
                    let mut served = self.requisitions_served.borrow_mut();
                    let served = served.this_turn(g.turn);
                    *served
                        .bought
                        .entry((order.requisition.kind, order.requisition.city))
                        .or_insert(0) += 1;
                    served.purchases_uncounted += 1;
                }
                if self.journal().wants(crate::reasoning::Level::Decision) {
                    let city_name = g.cities[&city].name.clone();
                    think!(self.journal(), Cities, Decision,
                        "{}, {city_name} buys {}", order.headline(), plain(order.unit.as_str());
                        "{cost:.0} of {bank:.0} Gold above a reserve of {reserve:.0}; \
                         the Objective Board's {} row is short {} {} unit(s)",
                        order.requisition.kind.as_str(), order.requisition.count, order.role.as_str());
                }
                return Some(order);
            }
        }
        None
    }

    /// The unit a requisition of one of `kinds` asks for this city — the
    /// bleeding city's defender under `emergency_city_defense_purchase`
    /// (Defend, Relieve) and the Deter row's under
    /// `border_parity_production`.
    pub(super) fn requisition_item_for_city(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        kinds: &[ObjectiveKind],
    ) -> Option<Item> {
        if !self.requisitions_on() {
            return None;
        }
        self.requisitions()
            .into_iter()
            .filter(|requisition| {
                requisition.city == Some(cid) && kinds.contains(&requisition.kind)
            })
            .find_map(|requisition| {
                let role = UnitRole::of(&requisition, g.turn)?;
                let (_, unit) = self.requisition_unit(g, pid, cid, role)?;
                Some(Item::Unit { unit })
            })
    }

    /// The Deter requisition's contact city, when one stands.
    pub(super) fn deter_requisition_city(&self) -> Option<u32> {
        if !self.requisitions_on() {
            return None;
        }
        self.requisitions()
            .into_iter()
            .find(|requisition| requisition.kind == ObjectiveKind::Deter)
            .and_then(|requisition| requisition.city)
    }

    /// The board's summed land need: bodies in the land task forces (the
    /// Reserve excepted) plus bodies requisitioned for land rows production
    /// serves.
    pub(super) fn board_land_headcount(&self, g: &Game) -> usize {
        let board = self.objective_board();
        let in_forces: usize = board
            .forces
            .iter()
            .filter(|force| {
                force.domain == ForceDomain::Land && force.objective_key != ObjectiveKey::Reserve
            })
            .map(|force| force.units.len())
            .sum();
        let requisitioned: usize = self
            .requisitions()
            .iter()
            .filter(|requisition| {
                !requisition.sea_only && UnitRole::of(requisition, g.turn).is_some()
            })
            .map(|requisition| requisition.count)
            .sum();
        in_forces + requisitioned
    }

    /// `production_value`'s army target under the gene: the board's summed
    /// land need when the land military is under it, capped at
    /// [`ARMY_TARGET_CAP_PER_CITY`] a city; `desired` otherwise, and with
    /// the gene off.
    pub(super) fn requisition_army_target(
        &self,
        g: &Game,
        pid: usize,
        desired: usize,
        land_military: usize,
    ) -> usize {
        if !self.requisitions_on() {
            return desired;
        }
        let cap = ARMY_TARGET_CAP_PER_CITY * g.player_city_ids(pid).len().max(1);
        let need = self.board_land_headcount(g).min(cap);
        if land_military < need {
            desired.max(need)
        } else {
            desired
        }
    }

    /// The production pass's first word: the board assessed for this turn
    /// (so the requisitions are this turn's, not last turn's), the purchases
    /// moved into the census, and the open requisitions no city of ours can
    /// build for counted. Exact no-op with the gene off.
    pub(super) fn requisitions_before_production(
        &mut self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.requisitions_on() {
            return;
        }
        self.assess_board_if_stale(g, pid, plan);
        let purchases =
            std::mem::take(&mut self.requisitions_served.borrow_mut().purchases_uncounted);
        self.census.requisition_purchases += purchases;
        let cities = g.player_city_ids(pid);
        let unserved = self
            .open_requisitions(g, pid)
            .into_iter()
            .filter(|(_, role)| {
                !cities
                    .iter()
                    .any(|city| self.requisition_unit(g, pid, *city, *role).is_some())
            })
            .count() as u32;
        self.census.requisition_unserved += unserved;
    }
}

/// An idle city: nothing in its queue.
fn city_idle(g: &Game, cid: u32) -> bool {
    g.cities.get(&cid).is_some_and(|city| city.queue.is_empty())
}

#[cfg(test)]
mod tests {
    use super::super::GrandStrategy;
    use super::*;
    use crate::game::Game;
    use crate::name;
    use crate::Pos;

    /// A flat board, the shape `objective_board::tests` uses: every starting
    /// unit cleared, everyone met, turn 60, nobody at war.
    fn flat_board(seed: u64, capitals: &[Pos]) -> Game {
        let mut game = Game::new_full(capitals.len(), 36, 22, seed, 1_000, 0, false);
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
        for (pid, pos) in capitals.iter().enumerate() {
            game.found_city_for(pid, *pos, None);
        }
        for pid in 0..capitals.len() {
            for other in 0..capitals.len() {
                if pid != other {
                    game.players[pid].met.insert(other);
                }
            }
            game.players[pid]
                .explored
                .extend(game.map.tiles.keys().copied());
        }
        game.at_war.clear();
        game.turn = 60;
        game.current = 0;
        game
    }

    fn at(col: i32, row: i32) -> Pos {
        crate::hex::offset_to_axial(col, row)
    }

    fn war(g: &mut Game, a: usize, b: usize) {
        g.at_war.insert((a.min(b), a.max(b)));
    }

    fn spawn(g: &mut Game, kind: &str, pid: usize, pos: Pos) -> u32 {
        let uid = g.spawn_test_unit(kind, pid, pos);
        let moves = g.unit_max_moves(uid);
        let unit = g.units.get_mut(&uid).unwrap();
        unit.moves_left = moves;
        unit.attacks_left = 1;
        uid
    }

    fn conquest(g: &Game, target: Option<u32>) -> StrategicPlan {
        StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: target.map(|cid| g.cities[&cid].owner),
            target_city: target,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: g.turn,
            rush: false,
        }
    }

    fn on() -> AdvancedAi {
        let mut ai = AdvancedAi::new();
        ai.enable_objective_board();
        ai.enable_requisitions();
        ai
    }

    fn city_of(g: &Game, pid: usize, pos: Pos) -> u32 {
        g.city_at(pos)
            .filter(|cid| g.cities[cid].owner == pid)
            .expect("a city of ours")
    }

    /// Our capital under pressure from three enemy warriors, one warrior of
    /// ours beside it, Archery known: the Defend row's melee is met and its
    /// strength is not.
    fn pressured_capital(seed: u64) -> (Game, u32) {
        let mut g = flat_board(seed, &[at(6, 8), at(30, 8)]);
        war(&mut g, 0, 1);
        let home = city_of(&g, 0, at(6, 8));
        for pos in [at(8, 8), at(7, 9), at(7, 7)] {
            spawn(&mut g, "warrior", 1, pos);
        }
        spawn(&mut g, "warrior", 0, at(5, 8));
        g.players[0].techs.insert(name!("archery"));
        g.cities.get_mut(&home).unwrap().queue.clear();
        (g, home)
    }

    #[test]
    fn the_gene_ships_off_and_is_registered() {
        let ai = AdvancedAi::new();
        assert!(!ai.requisitions, "an opt-in ships off");
        assert!(!ai.requisitions_on());
        assert!(super::super::GENES.iter().any(|gene| gene.opt_in()
            && gene.tag == "requisitions"
            && gene.field == "requisitions"));
        let mut on = AdvancedAi::new();
        on.enable_requisitions();
        assert!(on.requisitions);
        assert!(!on.requisitions_on(), "inert without the board");
        on.enable_objective_board();
        assert!(on.requisitions_on());
        on.disable_requisitions();
        assert!(!on.requisitions);
        super::super::test_support::opt_in_off_in_both_controllers("requisitions", |ai| {
            ai.requisitions
        });
    }

    /// The worth-per-hammer choice: a Swordsman outbids a Warrior once both
    /// can be built; roles read off the spec.
    #[test]
    fn the_unit_is_the_best_worth_per_hammer_of_its_kind() {
        let g = Game::new_full(2, 36, 22, 1, 1_000, 0, false);
        let rules = &g.rules;
        let warrior = &rules.units[name!("warrior")];
        let swordsman = &rules.units[name!("swordsman")];
        assert!(UnitRole::Melee.fits(warrior) && UnitRole::Melee.fits(swordsman));
        assert!(
            worth_per_hammer(UnitRole::Melee, swordsman)
                > worth_per_hammer(UnitRole::Melee, warrior),
            "the exchange gap outgrows the cost gap"
        );
        let archer = &rules.units[name!("archer")];
        assert!(UnitRole::Ranged.fits(archer) && !UnitRole::Melee.fits(archer));
        let catapult = &rules.units[name!("catapult")];
        assert!(UnitRole::Siege.fits(catapult) && !UnitRole::Ranged.fits(catapult));
        let horseman = &rules.units[name!("horseman")];
        assert!(UnitRole::Cavalry.fits(horseman) && !UnitRole::Melee.fits(horseman));
        assert!(!UnitRole::Melee.fits(&rules.units[name!("scout")]));
    }

    /// A Defend row's unmet ranged need becomes an Archer at the contact
    /// city — the row's own city, idle, starts it ahead of the economy.
    #[test]
    fn a_defend_rows_unmet_need_becomes_an_archer_at_the_contact_city() {
        let (mut g, home) = pressured_capital(21);
        let mut ai = on();
        let plan = conquest(&g, None);
        ai.rebuild_force_groups(&g, 0, &plan);
        let requisition = ai
            .requisitions()
            .into_iter()
            .find(|requisition| requisition.kind == ObjectiveKind::Defend)
            .expect("the Defend row is short");
        assert_eq!(requisition.city, Some(home));
        assert_eq!(
            requisition.unmet.melee, 0,
            "the warrior beside the city is the melee"
        );
        assert!(requisition.unmet.strength > 0.0);
        assert_eq!(UnitRole::of(&requisition, g.turn), Some(UnitRole::Ranged));
        let order = ai
            .requisition_production_item(&g, 0, home)
            .expect("the contact city serves the requisition");
        assert_eq!(order.city, home);
        assert_eq!(order.role, UnitRole::Ranged);
        assert_eq!(order.unit.as_str(), "archer");
        assert!(order.headline().starts_with("Requisition: "));
        assert!(order.headline().contains("ranged for Defend"));
        // The whole production pass: the city's queue holds the Archer.
        ai.advanced_production(&mut g, 0, &plan, false);
        assert_eq!(
            g.cities[&home].queue.first(),
            Some(&Item::Unit {
                unit: name!("archer")
            })
        );
        assert_eq!(ai.census.requisition_items, 1);
        // The queued Archer is credited: the city, were it idle again, would
        // not start a second for the same row.
        assert!(ai.requisition_production_item(&g, 0, home).is_none());
    }

    /// A met need produces nothing: with enough of ours beside the city the
    /// Defend row is served and no requisition stands.
    #[test]
    fn a_met_need_produces_nothing() {
        let mut g = flat_board(22, &[at(6, 8), at(30, 8)]);
        war(&mut g, 0, 1);
        let home = city_of(&g, 0, at(6, 8));
        for pos in [at(8, 8), at(7, 9)] {
            spawn(&mut g, "warrior", 1, pos);
        }
        for pos in [at(5, 8), at(5, 9), at(4, 8), at(6, 9)] {
            spawn(&mut g, "warrior", 0, pos);
        }
        g.players[0].techs.insert(name!("archery"));
        g.cities.get_mut(&home).unwrap().queue.clear();
        g.players[0].gold = 1_000.0;
        let mut ai = on();
        let plan = conquest(&g, None);
        ai.rebuild_force_groups(&g, 0, &plan);
        assert!(
            ai.requisitions()
                .iter()
                .all(|requisition| requisition.kind != ObjectiveKind::Defend),
            "the Defend row is met: {:?}",
            ai.requisitions()
        );
        assert!(ai.requisition_production_item(&g, 0, home).is_none());
        assert!(ai.requisition_purchase(&mut g, 0, 0.0).is_none());
        ai.advanced_production(&mut g, 0, &plan, false);
        assert_eq!(ai.census.requisition_items, 0);
    }

    /// Gold buys the requested unit when the treasury covers it above the
    /// reserve, and not when it does not; one a turn.
    #[test]
    fn gold_buys_the_requested_unit_when_affordable() {
        let (mut g, home) = pressured_capital(23);
        let mut ai = on();
        let plan = conquest(&g, None);
        ai.rebuild_force_groups(&g, 0, &plan);
        let cost = g
            .unit_purchase_cost(0, home, "archer", "gold")
            .expect("an Archer has a price");
        g.players[0].gold = cost - 1.0;
        assert!(
            ai.requisition_purchase(&mut g, 0, 0.0).is_none(),
            "one Gold short buys nothing"
        );
        g.players[0].gold = cost + 50.0;
        assert!(
            ai.requisition_purchase(&mut g, 0, 100.0).is_none(),
            "the reserve is kept"
        );
        let before = g.player_unit_ids(0).len();
        let order = ai
            .requisition_purchase(&mut g, 0, 0.0)
            .expect("the treasury covers the Archer");
        assert_eq!(order.unit.as_str(), "archer");
        assert_eq!(order.city, home);
        assert_eq!(g.player_unit_ids(0).len(), before + 1);
        assert!(g.players[0].gold < cost + 50.0);
        // One a turn.
        g.players[0].gold = 10_000.0;
        assert!(ai.requisition_purchase(&mut g, 0, 0.0).is_none());
        // The production pass moves the purchase into the census.
        ai.advanced_production(&mut g, 0, &plan, false);
        assert_eq!(ai.census.requisition_purchases, 1);
    }

    /// A Siege row against a walled city requests a siege unit.
    #[test]
    fn a_siege_row_requests_a_siege_unit() {
        let mut g = flat_board(24, &[at(6, 8), at(20, 8)]);
        war(&mut g, 0, 1);
        let home = city_of(&g, 0, at(6, 8));
        let target = city_of(&g, 1, at(20, 8));
        g.cities.get_mut(&target).unwrap().wall_hp = 100;
        for pos in [at(8, 8), at(8, 9), at(9, 8)] {
            spawn(&mut g, "warrior", 0, pos);
        }
        for tech in [
            "mining",
            "bronze_working",
            "archery",
            "masonry",
            "the_wheel",
            "engineering",
        ] {
            g.players[0].techs.insert(Name::new(tech));
        }
        g.cities.get_mut(&home).unwrap().queue.clear();
        let mut ai = on();
        let plan = conquest(&g, Some(target));
        ai.rebuild_force_groups(&g, 0, &plan);
        let requisition = ai
            .requisitions()
            .into_iter()
            .find(|requisition| requisition.kind == ObjectiveKind::Siege)
            .expect("the Siege row is short");
        assert!(requisition.unmet.siege >= 1, "walls stand: {requisition:?}");
        assert_eq!(UnitRole::of(&requisition, g.turn), Some(UnitRole::Siege));
        let order = ai
            .requisition_production_item(&g, 0, home)
            .expect("the capital serves it");
        assert_eq!(order.role, UnitRole::Siege);
        assert!(
            g.rules.units[order.unit].siege,
            "{} is a siege unit",
            order.unit.as_str()
        );
    }

    /// The army target under the gene: the board's summed need when the
    /// land military is under it, capped; unchanged with the gene off.
    #[test]
    fn the_army_target_is_sized_from_the_boards_need() {
        let (g, _home) = pressured_capital(25);
        let mut ai = on();
        let plan = conquest(&g, None);
        ai.rebuild_force_groups(&g, 0, &plan);
        let need = ai.board_land_headcount(&g);
        assert!(
            need >= 2,
            "one warrior in the force and at least one requisitioned: {need}"
        );
        assert_eq!(
            ai.requisition_army_target(&g, 0, 1, 1),
            need.min(ARMY_TARGET_CAP_PER_CITY)
        );
        assert_eq!(ai.requisition_army_target(&g, 0, 9, 1), 9, "never lowered");
        assert_eq!(
            ai.requisition_army_target(&g, 0, 1, need),
            1,
            "met: unchanged"
        );
        ai.disable_requisitions();
        assert_eq!(ai.requisition_army_target(&g, 0, 1, 0), 1);
    }
}
