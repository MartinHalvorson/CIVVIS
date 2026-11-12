//! Who ordered the unit that died: the step that last moved, held or struck
//! with each of our military units, counted when the unit is gone.
//!
//! A census of live losses found that for most units that died there was no
//! journal line saying which step had moved them. A military unit's order can
//! come from a dozen pre-passes in `advanced_units` and some forty-five
//! branches of `advanced_military_step_with_decline`, and nothing recorded
//! which one it was. This does, and changes no decision: nothing reads it.
//!
//! **How a step is named.** Every applied action is in `Game::log`, so the
//! census keeps a cursor into it. A pre-pass is followed by one line,
//! `self.credit_orders(g, OrderStep::X)`, which credits every order our seat
//! applied since the cursor to `X`. A ladder branch names itself with one
//! line inside the branch, `self.ordering(OrderStep::X)`, and the ladder's
//! caller (`credited_military_step`) credits that pass's orders to the name,
//! or to `other` when the branch that acted named nothing. An order counts
//! when it moves, holds or strikes with a military unit: `Move`, `MoveTo`,
//! `Swap`, `Fortify`, `Pillage`, the strikes, and the air operations.
//!
//! **What is counted.** At the end of our turn (`close_unit_orders`) every
//! military unit of ours is on the book — one nobody ordered as `unordered` —
//! and each is marked by whether it ended the turn within two tiles of a
//! visible hostile military unit (a unit that died on our own turn, striking,
//! counts as within). At the start of our next unit pass
//! (`open_unit_orders`), each unit on the book that is gone, or no longer
//! ours, adds one to its step's `[within two, beyond]` pair in
//! `StrategyCensus::last_orders`, and the journal says so at Detail.
//!
//! **What it cannot tell apart.** A unit we disband or merge into a corps is
//! taken off the book when the census sees the order, and a levied unit is
//! never put on it. A unit removed by maintenance, fallout or a disaster
//! between our turns is counted as if it had died; those are rare beside
//! combat. A death after the seat's last turn of the game is counted only if
//! the caller asks (`settle_last_orders`). On the live mirror the game is
//! rebuilt every frame, so the cursor is reset whenever the log is shorter
//! than it; that has not been checked against a live seat.

use super::*;

/// Where a military unit's order came from: a pre-pass of the unit pass, a
/// branch of the per-unit ladder, or nothing anybody named. The names are
/// what the census and the gene screen's row print.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OrderStep {
    /// An order no step named. The share to drive down.
    Other,
    /// A unit that never received an order at all.
    Unordered,
    /// Our own orders before the unit pass (the command actions).
    TurnStart,
    Withdraw,
    BattleSolver,
    BattlePlanner,
    Rotation,
    Positions,
    ImmediateKills,
    GunsOutOfReach,
    Air,
    FallingCity,
    Raids,
    Hunts,
    StandDown,
    /// `commitment-owner-acts`, after the unit pass.
    Commitment,
    Escort,
    Breaker,
    Condemn,
    SwapRotation,
    WoundedOutOfReach,
    Healing,
    Retreat,
    Explorer,
    WarObjective,
    UpgradeReturn,
    SiegeDoctrine,
    RushSiege,
    CivilianCapture,
    RaidPrize,
    DoctrineAction,
    Garrison,
    /// `centre-keeps-its-garrison`: a threatened City Center's garrison.
    CentreGarrison,
    Staging,
    Improver,
    Camp,
    Picket,
    TacticalAttack,
    Pillage,
    BarbarianResponse,
    Reinforcement,
    Coordinated,
    Tactical,
    Peacetime,
    Hold,
}

impl OrderStep {
    /// Every step, in the order the census prints them.
    pub const ALL: [OrderStep; 45] = [
        OrderStep::Other,
        OrderStep::Unordered,
        OrderStep::TurnStart,
        OrderStep::Withdraw,
        OrderStep::BattleSolver,
        OrderStep::BattlePlanner,
        OrderStep::Rotation,
        OrderStep::Positions,
        OrderStep::ImmediateKills,
        OrderStep::GunsOutOfReach,
        OrderStep::Air,
        OrderStep::FallingCity,
        OrderStep::Raids,
        OrderStep::Hunts,
        OrderStep::StandDown,
        OrderStep::Commitment,
        OrderStep::Escort,
        OrderStep::Breaker,
        OrderStep::Condemn,
        OrderStep::SwapRotation,
        OrderStep::WoundedOutOfReach,
        OrderStep::Healing,
        OrderStep::Retreat,
        OrderStep::Explorer,
        OrderStep::WarObjective,
        OrderStep::UpgradeReturn,
        OrderStep::SiegeDoctrine,
        OrderStep::RushSiege,
        OrderStep::CivilianCapture,
        OrderStep::RaidPrize,
        OrderStep::DoctrineAction,
        OrderStep::Garrison,
        OrderStep::CentreGarrison,
        OrderStep::Staging,
        OrderStep::Improver,
        OrderStep::Camp,
        OrderStep::Picket,
        OrderStep::TacticalAttack,
        OrderStep::Pillage,
        OrderStep::BarbarianResponse,
        OrderStep::Reinforcement,
        OrderStep::Coordinated,
        OrderStep::Tactical,
        OrderStep::Peacetime,
        OrderStep::Hold,
    ];

    /// The short name the census and the screen's row carry.
    pub fn as_str(self) -> &'static str {
        match self {
            OrderStep::Other => "other",
            OrderStep::Unordered => "unordered",
            OrderStep::TurnStart => "turn_start",
            OrderStep::Withdraw => "withdraw",
            OrderStep::BattleSolver => "battle_solver",
            OrderStep::BattlePlanner => "battle_planner",
            OrderStep::Rotation => "rotation",
            OrderStep::Positions => "positions",
            OrderStep::ImmediateKills => "immediate_kills",
            OrderStep::GunsOutOfReach => "guns_out_of_reach",
            OrderStep::Air => "air",
            OrderStep::FallingCity => "falling_city",
            OrderStep::Raids => "raids",
            OrderStep::Hunts => "hunts",
            OrderStep::StandDown => "stand_down",
            OrderStep::Commitment => "commitment",
            OrderStep::Escort => "escort",
            OrderStep::Breaker => "breaker",
            OrderStep::Condemn => "condemn",
            OrderStep::SwapRotation => "swap_rotation",
            OrderStep::WoundedOutOfReach => "wounded_out_of_reach",
            OrderStep::Healing => "healing",
            OrderStep::Retreat => "retreat",
            OrderStep::Explorer => "explorer",
            OrderStep::WarObjective => "war_objective",
            OrderStep::UpgradeReturn => "upgrade_return",
            OrderStep::SiegeDoctrine => "siege_doctrine",
            OrderStep::RushSiege => "rush_siege",
            OrderStep::CivilianCapture => "civilian_capture",
            OrderStep::RaidPrize => "raid_prize",
            OrderStep::DoctrineAction => "doctrine_action",
            OrderStep::Garrison => "garrison",
            OrderStep::CentreGarrison => "centre_garrison",
            OrderStep::Staging => "staging",
            OrderStep::Improver => "improver",
            OrderStep::Camp => "camp",
            OrderStep::Picket => "picket",
            OrderStep::TacticalAttack => "tactical_attack",
            OrderStep::Pillage => "pillage",
            OrderStep::BarbarianResponse => "barbarian_response",
            OrderStep::Reinforcement => "reinforcement",
            OrderStep::Coordinated => "coordinated",
            OrderStep::Tactical => "tactical",
            OrderStep::Peacetime => "peacetime",
            OrderStep::Hold => "hold",
        }
    }

    fn index(self) -> usize {
        OrderStep::ALL
            .iter()
            .position(|step| *step == self)
            .expect("every step is listed in ALL")
    }
}

/// Our military units that died, by the step that last ordered them:
/// `[ended within two tiles of a visible hostile, did not]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LastOrderCensus {
    deaths: [[u32; 2]; OrderStep::ALL.len()],
}

impl Default for LastOrderCensus {
    fn default() -> Self {
        LastOrderCensus {
            deaths: [[0; 2]; OrderStep::ALL.len()],
        }
    }
}

impl LastOrderCensus {
    fn count(&mut self, step: OrderStep, within_two: bool) {
        self.deaths[step.index()][usize::from(!within_two)] += 1;
    }

    /// One step's `[within two, beyond]` pair.
    pub fn of(&self, step: OrderStep) -> [u32; 2] {
        self.deaths[step.index()]
    }

    /// Every death counted.
    pub fn deaths(&self) -> u32 {
        self.deaths.iter().map(|pair| pair[0] + pair[1]).sum()
    }

    /// The share of deaths whose last order no step named, `None` before
    /// the first death.
    pub fn other_share(&self) -> Option<f64> {
        let deaths = self.deaths();
        (deaths > 0).then(|| {
            let [within, beyond] = self.of(OrderStep::Other);
            f64::from(within + beyond) / f64::from(deaths)
        })
    }

    /// The steps that saw a death, by name: the compact map the screen's row
    /// carries.
    pub fn map(&self) -> BTreeMap<String, [u32; 2]> {
        OrderStep::ALL
            .iter()
            .map(|step| (step.as_str().to_string(), self.of(*step)))
            .filter(|(_, pair)| pair[0] + pair[1] > 0)
            .collect()
    }

    /// Accumulate another agent's deaths into this total.
    pub fn absorb(&mut self, other: &LastOrderCensus) {
        for (mine, theirs) in self.deaths.iter_mut().zip(other.deaths.iter()) {
            mine[0] += theirs[0];
            mine[1] += theirs[1];
        }
    }
}

/// One unit's last order: which step, on which turn, the unit's kind, and
/// whether it ended our last turn within two tiles of a visible hostile.
#[derive(Clone, Copy, Debug)]
pub(super) struct LastOrder {
    step: OrderStep,
    turn: u32,
    kind: Option<Name>,
    within_two: bool,
}

/// The per-unit book the census is kept from. Read by nothing that decides.
#[derive(Clone, Debug, Default)]
pub(super) struct UnitOrderBook {
    seat: Option<usize>,
    /// `Game::log` length the census has read up to.
    cursor: usize,
    /// The ladder branch giving the current unit its orders, if it named
    /// itself.
    naming: Option<OrderStep>,
    last: BTreeMap<u32, LastOrder>,
}

impl UnitOrderBook {
    /// Read one of our applied actions.
    fn read(&mut self, g: &Game, seat: usize, action: &Action, step: OrderStep) {
        let (unit, other, strike) = match action {
            Action::Move { unit, .. }
            | Action::MoveTo { unit, .. }
            | Action::Fortify { unit }
            | Action::Pillage { unit }
            | Action::AirRebase { unit, .. }
            | Action::AirPatrol { unit, .. }
            | Action::AirPillage { unit, .. } => (*unit, None, false),
            Action::Swap { unit, other } => (*unit, Some(*other), false),
            Action::Attack { unit, .. }
            | Action::Ranged { unit, .. }
            | Action::AirStrike { unit, .. }
            | Action::PriorityTarget { unit, .. }
            | Action::CoastalRaid { unit, .. }
            | Action::CondemnHeretic { unit, .. } => (*unit, None, true),
            Action::DisbandUnit { unit } => {
                self.last.remove(unit);
                return;
            }
            Action::CombineUnits { unit, with } => {
                for id in [unit, with] {
                    if !g.units.contains_key(id) {
                        self.last.remove(id);
                    }
                }
                return;
            }
            _ => return,
        };
        let turn = g.turn;
        for uid in std::iter::once(unit).chain(other) {
            match g.units.get(&uid) {
                Some(found)
                    if found.owner == seat
                        && found.levied_from.is_none()
                        && g.rules.units[found.kind].class == "military" =>
                {
                    self.last.insert(
                        uid,
                        LastOrder {
                            step,
                            turn,
                            kind: Some(found.kind),
                            within_two: false,
                        },
                    );
                }
                // Gone after its own strike: it died attacking, in contact.
                None if strike && uid == unit => {
                    let kind = self.last.get(&uid).and_then(|order| order.kind);
                    self.last.insert(
                        uid,
                        LastOrder {
                            step,
                            turn,
                            kind,
                            within_two: true,
                        },
                    );
                }
                _ => {}
            }
        }
    }
}

impl AdvancedAi {
    /// Name the ladder branch giving the current unit its orders. One line
    /// inside each branch, before it returns; `credited_military_step`
    /// credits the pass's orders to it.
    pub(super) fn ordering(&mut self, step: OrderStep) {
        self.unit_orders.naming = Some(step);
    }

    /// Credit every order our seat applied since the census last looked to
    /// `step`. One line after each pre-pass of the unit pass.
    pub(super) fn credit_orders(&mut self, g: &Game, step: OrderStep) {
        let Some(seat) = self.unit_orders.seat else {
            return;
        };
        let book = &mut self.unit_orders;
        // A shorter log is another game (the live mirror rebuilds it every
        // frame): nothing in it is ours to credit.
        let start = book.cursor.min(g.log.len());
        for (actor, action) in g.log.since(start) {
            if *actor == seat {
                book.read(g, seat, action, step);
            }
        }
        book.cursor = g.log.len();
    }

    /// One pass of the per-unit ladder, its orders credited to the branch
    /// that named itself, or `other`. Anything applied before it that no
    /// step credited is `other` as well.
    pub(super) fn credited_military_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
        decline_settlers: bool,
    ) -> bool {
        self.credit_orders(g, OrderStep::Other);
        self.unit_orders.naming = None;
        let acted = self.advanced_military_step_with_decline(g, pid, uid, plan, decline_settlers);
        let step = self.unit_orders.naming.take().unwrap_or(OrderStep::Other);
        self.credit_orders(g, step);
        acted
    }

    /// The stand-down hold (`BasicAi::hold_stood_down_unit`), credited.
    pub(super) fn stand_down(&mut self, g: &mut Game, pid: usize, uid: u32) {
        self.base.hold_stood_down_unit(g, pid, uid);
        self.credit_orders(g, OrderStep::StandDown);
    }

    /// The step that last ordered `uid`, if the book holds it.
    pub(super) fn last_order_step(&self, uid: u32) -> Option<OrderStep> {
        self.unit_orders.last.get(&uid).map(|order| order.step)
    }

    /// The start of our unit pass: credit our orders since the last pass to
    /// `turn_start`, then count every unit on the book that is gone.
    pub(super) fn open_unit_orders(&mut self, g: &Game, pid: usize) {
        if self.unit_orders.seat != Some(pid) {
            self.unit_orders = UnitOrderBook {
                seat: Some(pid),
                cursor: g.log.len(),
                ..UnitOrderBook::default()
            };
        }
        self.credit_orders(g, OrderStep::TurnStart);
        self.count_the_dead(g, pid);
    }

    /// The end of our turn: credit what the commitment owners ordered after
    /// the unit pass, put every military unit of ours on the book, and mark
    /// each by whether it ends the turn within two tiles of a visible
    /// hostile.
    pub fn close_unit_orders(&mut self, g: &Game, pid: usize) {
        if self.unit_orders.seat != Some(pid) {
            return;
        }
        self.credit_orders(g, OrderStep::Commitment);
        let visible = g.player_vision_frame(pid);
        let hostiles: Vec<Pos> = g
            .units
            .values()
            .filter(|unit| {
                unit.owner != pid
                    && g.rules.units[unit.kind].class == "military"
                    && g.is_at_war(pid, unit.owner)
                    && g.sees(&visible, unit.pos)
            })
            .map(|unit| unit.pos)
            .collect();
        let turn = g.turn;
        for uid in g.player_unit_ids(pid) {
            let unit = &g.units[&uid];
            if unit.levied_from.is_some() || g.rules.units[unit.kind].class != "military" {
                continue;
            }
            self.unit_orders.last.entry(uid).or_insert(LastOrder {
                step: OrderStep::Unordered,
                turn,
                kind: Some(unit.kind),
                within_two: false,
            });
        }
        for (uid, order) in self.unit_orders.last.iter_mut() {
            order.within_two = match g.units.get(uid) {
                Some(unit) => hostiles.iter().any(|at| g.wdist(*at, unit.pos) <= 2),
                None => true,
            };
        }
    }

    /// Count the deaths since our last turn without waiting for another one:
    /// for a caller reading the census after the game has ended. A seat that
    /// was eliminated has no next turn, and its army went with it, so
    /// nothing is counted for it.
    pub fn settle_last_orders(&mut self, g: &Game, pid: usize) {
        if self.unit_orders.seat == Some(pid) && g.players.get(pid).is_some_and(|p| p.alive) {
            self.open_unit_orders(g, pid);
        }
    }

    fn count_the_dead(&mut self, g: &Game, pid: usize) {
        let gone: Vec<u32> = self
            .unit_orders
            .last
            .keys()
            .copied()
            .filter(|uid| g.units.get(uid).is_none_or(|unit| unit.owner != pid))
            .collect();
        for uid in gone {
            let Some(order) = self.unit_orders.last.remove(&uid) else {
                continue;
            };
            self.census.last_orders.count(order.step, order.within_two);
            let kind = order.kind.map_or("unit", |kind| kind.as_str());
            think!(self.journal(), Military, Detail,
                   "Lost {kind} {uid}: last ordered by `{}`", order.step.as_str();
                   "on turn {}; it ended our last turn {} two tiles of a visible hostile",
                   order.turn,
                   if order.within_two { "within" } else { "beyond" });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::advanced::{GrandStrategy, VictoryTarget};

    fn board() -> Game {
        let mut g = Game::new_full(2, 40, 24, 936302, 650, 0, false);
        for uid in g.units.keys().copied().collect::<Vec<_>>() {
            g.remove_unit(uid);
        }
        g.barb_camps.clear();
        g.barb_naval_camps.clear();
        for tile in g.map.tiles.values_mut() {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.resource = None;
            tile.hills = false;
        }
        g.found_city_for(0, (10, 12), None);
        g.found_city_for(1, (26, 12), None);
        g.record_contact(0, 1);
        g.at_war.insert((0, 1));
        g.at_war.insert((1, 0));
        g.current = 0;
        g.turn = 180;
        g
    }

    fn plan() -> StrategicPlan {
        StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: 0,
            rush: false,
        }
    }

    /// The enemy kills `victim` with a fresh warrior of its own, on its turn.
    fn enemy_kills(g: &mut Game, victim: u32) {
        let at = g.units[&victim].pos;
        g.units.get_mut(&victim).unwrap().hp = 1;
        let killer = g.spawn_test_unit("warrior", 1, (at.0 + 1, at.1));
        g.current = 1;
        g.apply(
            1,
            &Action::Attack {
                unit: killer,
                target: at,
            },
        )
        .unwrap();
        assert!(!g.units.contains_key(&victim));
        g.current = 0;
    }

    /// ⭐ A unit the battle plan held is the battle planner's when it dies:
    /// the ladder's first branch names itself, the ladder's caller credits
    /// the hold to it, and the next pass counts the death beside a hostile.
    #[test]
    fn a_death_is_counted_against_the_step_that_last_ordered_the_unit() {
        let mut g = board();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        let warrior = g.spawn_test_unit("warrior", 0, (14, 12));
        let idle = g.spawn_test_unit("warrior", 0, (11, 12));
        g.spawn_test_unit("warrior", 1, (16, 12));
        ai.open_unit_orders(&g, 0);
        ai.battle_planner_ordered.insert(warrior);
        ai.credited_military_step(&mut g, 0, warrior, &plan(), false);
        assert!(matches!(
            g.log.last(),
            Some((0, Action::Fortify { unit })) if *unit == warrior
        ));
        assert_eq!(ai.last_order_step(warrior), Some(OrderStep::BattlePlanner));
        ai.close_unit_orders(&g, 0);
        assert_eq!(ai.last_order_step(idle), Some(OrderStep::Unordered));
        enemy_kills(&mut g, warrior);
        ai.open_unit_orders(&g, 0);
        let census = ai.strategy_census().last_orders;
        assert_eq!(census.of(OrderStep::BattlePlanner), [1, 0], "{census:?}");
        assert_eq!(census.deaths(), 1);
        assert_eq!(census.other_share(), Some(0.0));
        assert_eq!(
            census.map(),
            BTreeMap::from([("battle_planner".to_string(), [1, 0])])
        );
        assert_eq!(ai.last_order_step(warrior), None, "counted once");
        ai.open_unit_orders(&g, 0);
        assert_eq!(ai.strategy_census().last_orders.deaths(), 1);
    }

    /// A pre-pass's orders go to the step its one line names; a unit we
    /// disband is not a death; a unit far from any hostile dies `beyond`.
    #[test]
    fn a_pre_pass_is_credited_by_its_line_and_a_disband_is_not_a_death() {
        let mut g = board();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        let scout = g.spawn_test_unit("warrior", 0, (12, 6));
        let spare = g.spawn_test_unit("warrior", 0, (12, 18));
        ai.open_unit_orders(&g, 0);
        g.apply(
            0,
            &Action::Move {
                unit: scout,
                to: (13, 6),
            },
        )
        .unwrap();
        ai.credit_orders(&g, OrderStep::Hunts);
        g.apply(0, &Action::DisbandUnit { unit: spare }).unwrap();
        ai.credit_orders(&g, OrderStep::Other);
        ai.close_unit_orders(&g, 0);
        assert_eq!(ai.last_order_step(scout), Some(OrderStep::Hunts));
        assert_eq!(ai.last_order_step(spare), None);
        enemy_kills(&mut g, scout);
        ai.open_unit_orders(&g, 0);
        let census = ai.strategy_census().last_orders;
        assert_eq!(census.of(OrderStep::Hunts), [0, 1], "{census:?}");
        assert_eq!(census.deaths(), 1, "the disbanded unit did not die");
    }

    /// The census rides `StrategyCensus::absorb` like every other count.
    #[test]
    fn the_census_absorbs_another_agents_deaths() {
        let mut one = LastOrderCensus::default();
        one.count(OrderStep::Healing, true);
        let mut two = LastOrderCensus::default();
        two.count(OrderStep::Healing, false);
        two.count(OrderStep::Other, false);
        let mut total = StrategyCensus {
            last_orders: one,
            ..StrategyCensus::default()
        };
        let other = StrategyCensus {
            last_orders: two,
            ..StrategyCensus::default()
        };
        total.absorb(&other);
        assert_eq!(total.last_orders.of(OrderStep::Healing), [1, 1]);
        assert_eq!(total.last_orders.deaths(), 3);
        assert!((total.last_orders.other_share().unwrap() - 1.0 / 3.0).abs() < 1e-12);
        let names: BTreeSet<&str> = OrderStep::ALL.iter().map(|step| step.as_str()).collect();
        assert_eq!(names.len(), OrderStep::ALL.len(), "every name is distinct");
        for (index, step) in OrderStep::ALL.iter().enumerate() {
            assert_eq!(
                *step as usize, index,
                "ALL lists {step:?} in declaration order"
            );
        }
        assert_eq!(
            OrderStep::Hold as usize + 1,
            OrderStep::ALL.len(),
            "ALL lists every step"
        );
    }
}
