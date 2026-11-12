//! One owner per unit: which claim owns each of our military units, how often
//! a step orders a unit another claim owns, and the opt-in gene
//! `one-owner-per-unit` that refuses those orders.
//!
//! **Why.** The design says each unit has one owner per turn. The controller
//! keeps a dozen claim sets instead, and not every step reads every set:
//! `reserved_units` (the siege's taker) is honoured by the ladder's siege
//! branch, the battle solver and `battle-planner-3`, not by battle planner
//! versions one and two nor by `prioritize_immediate_kills`; the pre-passes
//! after the battle plan (`falling_city_takers`, `band_hunt`, `launcher_war`,
//! the air surge's raids and heretic hunt) exclude only the `claimed` set
//! handed to them, not `battle_planner_ordered`; `rebuild_force_groups` and
//! `prioritize_immediate_kills` skip a Builder's guard but not a Settler's,
//! and `condemn_step` runs before the ladder's guard branch. Comments in the
//! code record what that has cost: the battle plan pulling a reinforcing unit
//! back so a siege staged nothing for nineteen turns (`tactics.rs`), the
//! evacuation walking full-health besiegers off their posts (`units.rs`,
//! `siege_post_holds`), Destroy rows taking four or five units a turn off a
//! siege and guns going to the anvil while the siege waited for a breaker
//! (`objective_board.rs`).
//!
//! **The read.** [`AdvancedAi::owner_of`] consults every existing claim in
//! the fixed precedence of [`Owner::ALL`] and returns the first that holds.
//! It adds no state. The precedence, most binding first:
//!
//! 1. `siege_taker` — `reserved_units`: the one unit a siege names to walk in.
//! 2. `amphibious` — a member of the crossing in hand (`amphibious_member`).
//! 3. `centre_keeper` — the soldier keeping a threatened City Center of ours
//!    (`keeps_the_centre`; only with `centre-keeps-its-garrison`).
//! 4. `defend_post` — posted this turn on a threatened city's centre or ring
//!    (`defend_hold_plan.posts`; only with `defend-force-holds-the-city`).
//! 5. `settler_guard` — bound to a Settler (`settler_guards`,
//!    `settler_sea_guards`).
//! 6. `civilian_guard` — reserved to cover a Builder (`builder_support`).
//! 7. `siege_member` — in a Domination siege force that is staging or
//!    investing (`active_siege_member`, `staging_siege_member`).
//! 8. `battle_plan` — the battle plan or solver ordered it this turn
//!    (`battle_planner_ordered`; before this turn's plan runs the set still
//!    holds last turn's units, which no longer count).
//! 9. `recovering` — out to heal (`battle_planner_recovering`, the base
//!    controller's `recovering_units`).
//! 10. `wonder_clearance` — clearing a wonder site for a Settler.
//! 11. `camp_bounty` — this turn's barbarian-camp errand (`camp_bounty_claims`).
//! 12. `retainers_garrison` — garrisoning an Amenity-short city under
//!     Retainers (`retainers_garrison_claims`).
//!
//! **Who acts for whom** ([`Owner::acts_for`]). A step that orders a unit
//! acts for its owner when it is that claim's own step:
//! - Sieges (`siege_taker`, `siege_member`) are ordered by the siege doctrine,
//!   a rush siege and the falling-city takers. Members are also ordered by
//!   the breakers and the amphibious train.
//! - `amphibious` is ordered by its train and the siege doctrine.
//! - `centre_keeper` is ordered by the centre and garrison steps, and
//!   `defend_post` by the defend-hold pre-pass and step as well as those.
//! - Guards (`settler_guard`, `civilian_guard`, `wonder_clearance`) are
//!   ordered by the escort steps.
//! - `battle_plan` is ordered by the planner's own sub-steps: withdraw,
//!   solver, blows, rotation, positions.
//! - `recovering` is ordered by healing, retreat, swap rotation,
//!   wounded-out-of-reach, the rotation and the withdraw.
//! - `camp_bounty` is ordered by the camp errand. `retainers_garrison` is
//!   ordered by the peacetime step that runs it and by garrisons.
//!
//! Two steps act for every owner: `stand_down` holds a unit every step
//! declined, and `kept` is this gene's own hold. Any other step ordering an
//! owned unit is a **crossing**.
//!
//! **Deliberate crossings** ([`deliberate_crossing`]), counted apart and never
//! refused:
//! - *A shot from its post.* A ranged blow does not move the unit, so the
//!   tile its claim holds is still held. This covers recovering units,
//!   guards, centre keepers and posted defenders. Two genes already allow it on purpose:
//!   `ranged-kills-are-free` lets a recovering shooter or a civilian's guard
//!   take a ranged kill from its own tile, and `centre-keeps-its-garrison`
//!   lets a keeper fire from the centre.
//! - *A wounded besieger goes to heal.* The siege keeps only its healthy
//!   members on their posts: `siege_post_holds` in the ladder and the battle
//!   plan's rotation ("Only the wounded come out") both let a member under
//!   `ROTATE_HP` leave. Healing, retreat, swap rotation, wounded-out-of-reach,
//!   rotation and withdraw orders on such a member are not crossings.
//!
//! **The census** is always on and changes no decision. Wherever
//! `advanced/last_orders.rs` credits an order to a step, the unit's owner is
//! read, and every crossing is counted by (owner, step) in
//! `StrategyCensus::claims`.
//! - For a ladder pass the owner is read before the pass.
//! - For a pre-pass it is read right after it. Claims that pre-pass made
//!   itself (the battle plan's) are its own, and the higher claims it could
//!   cross are not ones it makes.
//! - Every turn also counts our military units and those that received no
//!   order from any step (`unordered_unit_turns`).
//!
//! What the census cannot see: a claim moved by a re-assignment rather than
//! an order. The objective board taking a member off a siege row changes the
//! unit's force group before anything orders it, so the order that follows
//! is no longer a crossing.
//!
//! **The gene** `one-owner-per-unit` (opt-in): a step that would cross a claim
//! does not order the unit.
//! - A pre-pass is first played on a clone of the controller and the board.
//!   If the clone crossed a claim, the real pass runs with those units held
//!   (no movement, no attack, restored afterwards).
//! - A ladder pass for an owned unit is played on a clone the same way. If
//!   it would cross, the unit is handed to its owner's own step, or is held
//!   (`kept`) when that step has nothing for it.
//! - Each refusal writes one journal line and is counted as `refused`.
//! - With the gene off nothing is cloned and nothing is held. With it on,
//!   every pre-pass runs twice (the rehearsal, then the real pass), and so
//!   does every ladder pass of an owned unit; that cost is unmeasured.
//!
//! **What it found** (CI driver `one_owner_per_unit_paired_evidence` in
//! `src/bin/victory_eval/domination_pair_tests.rs`: six paired Domination
//! games, the live bundle's Gran Colombia on 4-player Pangaea, King, 200
//! turns at most):
//! - With the gene off there were 632 crossings in 40,765 unit-turns, 1.55
//!   per 100.
//! - They are concentrated: the battle planner and the coordinated step
//!   ordered siege members 274 times, and the healing step ordered the
//!   siege's taker 30 times.
//! - On 21.9% of unit-turns no step ordered the unit.
//! - The six paired seeds moved both ways. Summed, units killed went 384 to
//!   259, units lost 214 to 138, cities taken 11 to 7 and cities lost 14 to
//!   17. Three games ended earlier with the gene on, and one seed's losses
//!   went from 6 to 42.
//! - That is a finding, not a lever: the gene is a guard and ships unarmed.

use super::last_orders::OrderStep;
use super::*;

/// The claim that owns a unit this turn, in precedence order. See the module
/// for what each reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Owner {
    SiegeTaker,
    Amphibious,
    CentreKeeper,
    DefendPost,
    SettlerGuard,
    CivilianGuard,
    SiegeMember,
    BattlePlan,
    Recovering,
    WonderClearance,
    CampBounty,
    RetainersGarrison,
}

impl Owner {
    /// Every owner, most binding first: the precedence `owner_of` reads.
    pub const ALL: [Owner; 12] = [
        Owner::SiegeTaker,
        Owner::Amphibious,
        Owner::CentreKeeper,
        Owner::DefendPost,
        Owner::SettlerGuard,
        Owner::CivilianGuard,
        Owner::SiegeMember,
        Owner::BattlePlan,
        Owner::Recovering,
        Owner::WonderClearance,
        Owner::CampBounty,
        Owner::RetainersGarrison,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Owner::SiegeTaker => "siege_taker",
            Owner::Amphibious => "amphibious",
            Owner::CentreKeeper => "centre_keeper",
            Owner::DefendPost => "defend_post",
            Owner::SettlerGuard => "settler_guard",
            Owner::CivilianGuard => "civilian_guard",
            Owner::SiegeMember => "siege_member",
            Owner::BattlePlan => "battle_plan",
            Owner::Recovering => "recovering",
            Owner::WonderClearance => "wonder_clearance",
            Owner::CampBounty => "camp_bounty",
            Owner::RetainersGarrison => "retainers_garrison",
        }
    }

    fn index(self) -> usize {
        Owner::ALL
            .iter()
            .position(|owner| *owner == self)
            .expect("every owner is listed in ALL")
    }

    /// Whether `step` is this claim's own step (or one that acts for every
    /// claim). See the module's table.
    pub fn acts_for(self, step: OrderStep) -> bool {
        use OrderStep as S;
        if matches!(step, S::StandDown | S::Kept) {
            return true;
        }
        match self {
            Owner::SiegeTaker => matches!(step, S::SiegeDoctrine | S::RushSiege | S::FallingCity),
            Owner::SiegeMember => matches!(
                step,
                S::SiegeDoctrine | S::RushSiege | S::FallingCity | S::Breaker | S::Amphibious
            ),
            Owner::Amphibious => matches!(step, S::Amphibious | S::SiegeDoctrine),
            Owner::CentreKeeper => matches!(step, S::CentreGarrison | S::Garrison),
            Owner::DefendPost => {
                matches!(step, S::DefendHold | S::CentreGarrison | S::Garrison)
            }
            Owner::SettlerGuard | Owner::CivilianGuard | Owner::WonderClearance => {
                matches!(step, S::Escort)
            }
            Owner::BattlePlan => matches!(
                step,
                S::BattlePlanner | S::BattleSolver | S::Rotation | S::Positions | S::Withdraw
            ),
            Owner::Recovering => matches!(
                step,
                S::Healing
                    | S::Retreat
                    | S::SwapRotation
                    | S::WoundedOutOfReach
                    | S::Rotation
                    | S::Withdraw
            ),
            Owner::CampBounty => matches!(step, S::Camp),
            Owner::RetainersGarrison => matches!(step, S::Peacetime | S::Garrison),
        }
    }
}

/// A crossing the code makes on purpose, named in the module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deliberate {
    /// A ranged blow from where the unit stands: its claim keeps the tile.
    ShotFromItsPost,
    /// A siege member under `ROTATE_HP` sent to heal.
    WoundedBesiegerHeals,
}

/// Whether ordering `uid`, owned by `owner`, from `step` is one of the
/// crossings the code makes on purpose. `shot` says the order was a ranged
/// blow, which does not move the unit.
pub fn deliberate_crossing(
    g: &Game,
    uid: u32,
    owner: Owner,
    step: OrderStep,
    shot: bool,
) -> Option<Deliberate> {
    use OrderStep as S;
    match owner {
        Owner::Recovering
        | Owner::SettlerGuard
        | Owner::CivilianGuard
        | Owner::CentreKeeper
        | Owner::DefendPost
            if shot =>
        {
            Some(Deliberate::ShotFromItsPost)
        }
        Owner::SiegeMember
            if matches!(
                step,
                S::Healing
                    | S::Retreat
                    | S::Rotation
                    | S::WoundedOutOfReach
                    | S::SwapRotation
                    | S::Withdraw
            ) && g
                .units
                .get(&uid)
                .is_some_and(|unit| unit.hp < battle_planner::ROTATE_HP) =>
        {
            Some(Deliberate::WoundedBesiegerHeals)
        }
        _ => None,
    }
}

/// Crossings by (owner, step): what happened, what was deliberate, and what
/// the gene refused; and the unit-turns no step ordered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClaimCensus {
    crossed: [[u32; OrderStep::ALL.len()]; Owner::ALL.len()],
    deliberate: [[u32; OrderStep::ALL.len()]; Owner::ALL.len()],
    refused: [[u32; OrderStep::ALL.len()]; Owner::ALL.len()],
    /// Our military units at the end of each of our turns, summed.
    pub unit_turns: u32,
    /// Of those, the ones no step ordered that turn.
    pub unordered_unit_turns: u32,
}

impl Default for ClaimCensus {
    fn default() -> Self {
        let zero = [[0; OrderStep::ALL.len()]; Owner::ALL.len()];
        ClaimCensus {
            crossed: zero,
            deliberate: zero,
            refused: zero,
            unit_turns: 0,
            unordered_unit_turns: 0,
        }
    }
}

impl ClaimCensus {
    fn cell(owner: Owner, step: OrderStep) -> (usize, usize) {
        (
            owner.index(),
            OrderStep::ALL.iter().position(|s| *s == step).unwrap(),
        )
    }

    pub(super) fn count_crossing(&mut self, owner: Owner, step: OrderStep, deliberate: bool) {
        let (o, s) = Self::cell(owner, step);
        if deliberate {
            self.deliberate[o][s] += 1;
        } else {
            self.crossed[o][s] += 1;
        }
    }

    pub(super) fn count_refusal(&mut self, owner: Owner, step: OrderStep) {
        let (o, s) = Self::cell(owner, step);
        self.refused[o][s] += 1;
    }

    /// `[crossed, deliberate, refused]` for one cell.
    pub fn of(&self, owner: Owner, step: OrderStep) -> [u32; 3] {
        let (o, s) = Self::cell(owner, step);
        [
            self.crossed[o][s],
            self.deliberate[o][s],
            self.refused[o][s],
        ]
    }

    /// `[crossed, deliberate, refused]` summed over every step, for one owner.
    pub fn of_owner(&self, owner: Owner) -> [u32; 3] {
        OrderStep::ALL.iter().fold([0; 3], |sum, step| {
            let cell = self.of(owner, *step);
            [sum[0] + cell[0], sum[1] + cell[1], sum[2] + cell[2]]
        })
    }

    /// Every crossing that happened and was not deliberate.
    pub fn crossings(&self) -> u32 {
        self.crossed.iter().flatten().sum()
    }

    pub fn deliberate_crossings(&self) -> u32 {
        self.deliberate.iter().flatten().sum()
    }

    pub fn refusals(&self) -> u32 {
        self.refused.iter().flatten().sum()
    }

    /// The cells that saw anything, as `"owner:step": [crossed, deliberate,
    /// refused]`: the compact map the screen's row carries.
    pub fn map(&self) -> BTreeMap<String, [u32; 3]> {
        let mut map = BTreeMap::new();
        for owner in Owner::ALL {
            for step in OrderStep::ALL {
                let cell = self.of(owner, step);
                if cell.iter().any(|n| *n > 0) {
                    map.insert(format!("{}:{}", owner.as_str(), step.as_str()), cell);
                }
            }
        }
        map
    }

    /// Accumulate another agent's counts into this total.
    pub fn absorb(&mut self, other: &ClaimCensus) {
        for (mine, theirs) in [
            (&mut self.crossed, &other.crossed),
            (&mut self.deliberate, &other.deliberate),
            (&mut self.refused, &other.refused),
        ] {
            for (row, other_row) in mine.iter_mut().zip(theirs.iter()) {
                for (cell, other_cell) in row.iter_mut().zip(other_row.iter()) {
                    *cell += other_cell;
                }
            }
        }
        self.unit_turns += other.unit_turns;
        self.unordered_unit_turns += other.unordered_unit_turns;
    }
}

impl AdvancedAi {
    /// Whether `owner`'s claim holds `uid` right now. Reads the existing
    /// claim sets; adds nothing.
    pub(super) fn claim_holds(&self, g: &Game, pid: usize, uid: u32, owner: Owner) -> bool {
        match owner {
            Owner::SiegeTaker => self.unit_is_reserved(uid),
            Owner::Amphibious => self.amphibious_member(uid),
            Owner::CentreKeeper => self.keeps_the_centre(g, pid, uid),
            Owner::DefendPost => {
                self.defend_hold_on()
                    && self.defend_hold_plan.turn == g.turn
                    && self.defend_hold_plan.posts.contains_key(&uid)
            }
            Owner::SettlerGuard => self.guard_is_bound_to_any_settler(uid),
            Owner::CivilianGuard => self.builder_guard_reserved(uid),
            Owner::SiegeMember => {
                self.active_siege_member(g, pid, uid) || self.staging_siege_member(g, pid, uid)
            }
            Owner::BattlePlan => {
                self.battle_planner_claims(uid) && self.battle_plan_is_current(g.turn)
            }
            Owner::Recovering => {
                self.battle_planner_recovering.contains(&uid)
                    || self.base.recovering_units.contains(&uid)
            }
            Owner::WonderClearance => self
                .wonder_clearance
                .values()
                .any(|clearance| clearance.until > g.turn && clearance.units.contains(&uid)),
            Owner::CampBounty => self
                .base
                .camp_bounty_claims
                .values()
                .any(|(turn, claimant)| *turn == g.turn && *claimant == uid),
            Owner::RetainersGarrison => self
                .base
                .retainers_garrison_claims
                .values()
                .any(|claimant| *claimant == uid),
        }
    }

    /// The one claim that owns `uid` this turn, by the precedence of
    /// [`Owner::ALL`], or `None` for a unit no claim holds.
    pub(super) fn owner_of(&self, g: &Game, pid: usize, uid: u32) -> Option<Owner> {
        Owner::ALL
            .into_iter()
            .find(|owner| self.claim_holds(g, pid, uid, *owner))
    }

    /// Count one order of `step` on `uid`, whose owner was `owner`: a
    /// crossing, a deliberate one, or nothing. Returns the crossing when it
    /// is one the gene would refuse.
    pub(super) fn note_claimed_order(
        &mut self,
        g: &Game,
        uid: u32,
        owner: Option<Owner>,
        step: OrderStep,
        shot: bool,
    ) {
        let Some(owner) = owner else {
            return;
        };
        if owner.acts_for(step) {
            return;
        }
        let deliberate = deliberate_crossing(g, uid, owner, step, shot).is_some();
        self.census.claims.count_crossing(owner, step, deliberate);
        if !deliberate {
            self.note_crossing_for_the_gene(uid, owner, step);
        }
    }

    /// `one-owner-per-unit`: a pre-pass of the unit pass, played under the
    /// one-owner rule, then credited to `step`. With the gene off this is the
    /// pass and its credit line, nothing else.
    pub(super) fn owned_pass<R>(
        &mut self,
        g: &mut Game,
        pid: usize,
        step: OrderStep,
        mut pass: impl FnMut(&mut Self, &mut Game) -> R,
    ) -> R {
        if !self.one_owner_per_unit {
            let out = pass(self, g);
            self.credit_orders(g, step);
            return out;
        }
        let mut rehearsal = self.clone();
        let mut board = g.clone();
        rehearsal.watch_crossings();
        pass(&mut rehearsal, &mut board);
        rehearsal.credit_orders(&board, step);
        let crossings = rehearsal.watched_crossings();
        if crossings.is_empty() {
            let out = pass(self, g);
            self.credit_orders(g, step);
            return out;
        }
        let mut held = Vec::new();
        for (uid, owner, crossing) in &crossings {
            self.census.claims.count_refusal(*owner, *crossing);
            let kind = g.units.get(uid).map_or("unit", |unit| unit.kind.as_str());
            think!(self.journal(), Military, Detail,
                   "Kept {kind} {uid} for its {}: `{}` would have ordered it",
                   owner.as_str(), crossing.as_str();
                   "one-owner-per-unit: a step that would cross a claim does not order the unit");
            if let Some(unit) = g.units.get_mut(uid).filter(|unit| unit.owner == pid) {
                held.push((*uid, unit.moves_left, unit.attacks_left));
                unit.moves_left = 0.0;
                unit.attacks_left = 0;
            }
        }
        let out = pass(self, g);
        for (uid, moves, attacks) in held {
            if let Some(unit) = g.units.get_mut(&uid) {
                unit.moves_left = moves;
                unit.attacks_left = attacks;
            }
        }
        self.credit_orders(g, step);
        out
    }

    /// `one-owner-per-unit` for one ladder pass of an owned unit: rehearse
    /// the pass on a clone, and when it would cross the claim, hand the unit
    /// to its owner's own step instead. `None` when the real pass may run.
    pub(super) fn owned_ladder_pass(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        owner: Owner,
        plan: &StrategicPlan,
        decline_settlers: bool,
    ) -> Option<bool> {
        let mut rehearsal = self.clone();
        let mut board = g.clone();
        rehearsal.watch_crossings();
        rehearsal.unit_orders_ladder(Some((uid, Some(owner))));
        rehearsal.unit_orders_name(None);
        rehearsal.advanced_military_step_with_decline(&mut board, pid, uid, plan, decline_settlers);
        let named = rehearsal.unit_orders_named().unwrap_or(OrderStep::Other);
        rehearsal.credit_orders(&board, named);
        let crossing = rehearsal
            .watched_crossings()
            .into_iter()
            .find(|(crossed, _, _)| *crossed == uid)?;
        self.census.claims.count_refusal(owner, crossing.2);
        let kind = g.units.get(&uid).map_or("unit", |unit| unit.kind.as_str());
        think!(self.journal(), Military, Detail,
               "Kept {kind} {uid} for its {}: `{}` would have ordered it",
               owner.as_str(), crossing.2.as_str();
               "one-owner-per-unit: the unit takes its owner's step or holds");
        Some(self.owner_keeps(g, pid, uid, owner, plan))
    }

    /// The owner's own step for a unit whose ladder pass was refused, or a
    /// hold (`kept`) when that step has nothing for it.
    fn owner_keeps(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        owner: Owner,
        plan: &StrategicPlan,
    ) -> bool {
        let owned = match owner {
            Owner::SiegeTaker | Owner::SiegeMember => self
                .siege_doctrine_step(g, pid, uid, plan)
                .map(|acted| (OrderStep::SiegeDoctrine, acted)),
            Owner::Amphibious => self
                .amphibious_member_step(g, pid, uid)
                .map(|acted| (OrderStep::Amphibious, acted)),
            Owner::CentreKeeper => self
                .centre_garrison_step(g, pid, uid)
                .map(|acted| (OrderStep::CentreGarrison, acted)),
            Owner::DefendPost => self
                .defend_hold_step(g, pid, uid)
                .map(|acted| (OrderStep::DefendHold, acted)),
            Owner::SettlerGuard => self
                .stacked_guard_step(g, pid, uid)
                .or_else(|| self.settler_escort_step(g, pid, uid, plan))
                .map(|acted| (OrderStep::Escort, acted)),
            Owner::WonderClearance => self
                .wonder_clearance_step(g, pid, uid)
                .map(|acted| (OrderStep::Escort, acted)),
            Owner::Recovering => self
                .base
                .healing_step(g, pid, uid)
                .map(|acted| (OrderStep::Healing, acted))
                .or_else(|| {
                    self.base
                        .retreat_step(g, pid, uid)
                        .map(|acted| (OrderStep::Retreat, acted))
                }),
            Owner::CivilianGuard
            | Owner::BattlePlan
            | Owner::CampBounty
            | Owner::RetainersGarrison => None,
        };
        let (step, acted) = owned.unwrap_or_else(|| {
            let acted = self.base.fortify_or_stop(g, pid, uid);
            (OrderStep::Kept, acted)
        });
        self.ordering(step);
        acted
    }
}

#[cfg(test)]
mod tests;
