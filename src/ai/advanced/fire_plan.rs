//! The fire plan: order the turn and focus the fire, without a clone.
//!
//! The joint engagement search that was removed on 2026-08-25 measured its
//! value mostly in *ordering*: its static seed — every unit choosing against
//! the untouched board — lost 700 kills on identical total damage against
//! the sequential rule (2,306 v 3,009 kills at 456,200 v 456,220 damage,
//! `docs/TACTICS.md` §3). Damage that finishes a unit is worth the unit;
//! damage spread across units that all survive is worth what they heal
//! back. The deployed controller plays its units in one fixed order —
//! ranged, siege, melee, each by id — and each prices its own attack when
//! its turn comes, with the force's `focus_target` as the one shared hint.
//!
//! This gene plans the turn's fire once, before the unit loop, from the
//! engine's own arithmetic and nothing else:
//!
//! 1. `legal_actions_within(UNITS)` names every strike a unit could make
//!    from where it stands — visibility, line of sight, zone of control,
//!    embarkation, formation, all the engine's predicates, none reproduced
//!    here. One call a turn, the same call `prioritize_immediate_kills`
//!    makes.
//! 2. Each strike is priced at the engine's centre roll with
//!    `ranged_strike_strengths` / `melee_exchange_strengths` and
//!    `expected_damage` — matchup, flanking, support, terrain, river,
//!    fortification and promotions included, no clone. A melee blow whose
//!    return would kill the attacker is not a strike.
//! 3. Kills are allocated greedily: the target whose finish costs the
//!    fewest shooters (ranged before melee, the heaviest blow first) is
//!    planned first, with a margin over the centre roll where there are
//!    shooters to spare; then the next, until nothing else can be finished
//!    by what is left.
//!
//! What the plan then does is small and exact: the planned shooters go
//! **first** in the unit order, kill by kill, ranged before the melee
//! finisher, and each planned shooter's attack scan gets the same bias
//! toward its planned target the force's `focus_target` already gets. The
//! exact, clone-verified attack decision is unchanged — a planned strike
//! that the board no longer supports is declined by the same rule that
//! declines any other. Everything else keeps its order and its scorer.
//!
//! Off in `AdvancedAi::new()` and `legacy()`, a `Kind::OptIn` row in
//! `genes.rs`, byte-identical when off: the plan is empty, every unit ranks
//! zero, the sort key is the shipped key. Priced first on the arena
//! (`doctrine_arena --a advanced+fire-plan`, a captured engagement file,
//! healing off and on) and on `battle_bench`; the whole-game screen is the
//! no-harm check afterwards. See `docs/DOCTRINE_ARENA.md`, "The gate for a
//! tactical gene".

use std::collections::BTreeMap;

use super::AdvancedAi;
use crate::game::{expected_damage, Action, ActionFamilies, Game};
use crate::Pos;

/// Planned damage over the target's hit points before a shooter is spared
/// for the next kill. The engine's roll is uniform on 0.8–1.2 of the
/// centre, so a plan at exactly the centre fails half the time; fifteen
/// percent over it is the cheapest margin that makes the miss the
/// exception.
const KILL_MARGIN: f64 = 1.15;
/// Shooters a single kill may spend. Four: past that the plan is spreading
/// an army over one unit, and the ordinary scorer does that better.
const SHOOTERS_PER_KILL: usize = 4;

/// One strike the engine would allow, priced at its centre roll.
#[derive(Clone, Debug, PartialEq)]
struct Shot {
    unit: u32,
    target: Pos,
    defender: u32,
    damage: f64,
    ranged: bool,
}

/// This turn's planned kills: each planned shooter, its target, and where
/// it goes in the unit order.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct FirePlan {
    /// Shooter → (target tile, rank). Rank orders the unit loop: kill one's
    /// shooters, then kill two's, ranged before melee within a kill.
    pub assigned: BTreeMap<u32, (Pos, u32)>,
    /// Kills the plan expects, for the journal and the census.
    pub kills: usize,
}

impl AdvancedAi {
    /// Plan this turn's fire, or leave the plan empty when the gene is off
    /// or nothing can be finished. Called once per turn from
    /// `advanced_units`, before the unit order is drawn.
    pub(super) fn plan_fire(&mut self, g: &Game, pid: usize) {
        self.fire_plan_orders = FirePlan::default();
        if !self.fire_plan {
            return;
        }
        let shots = Self::priced_shots(g, pid);
        if shots.is_empty() {
            return;
        }
        self.fire_plan_orders = Self::allocate_kills(g, &shots);
    }

    /// Every strike the engine allows from where each unit stands, priced at
    /// the centre roll. Melee blows whose return would finish the attacker
    /// are dropped: the plan never spends a unit to make a kill.
    fn priced_shots(g: &Game, pid: usize) -> Vec<Shot> {
        let mut shots = Vec::new();
        for action in g.legal_actions_within(pid, ActionFamilies::UNITS) {
            let (unit, target, ranged) = match action {
                Action::Ranged { unit, target } => (unit, target, true),
                Action::Attack { unit, target } => (unit, target, false),
                _ => continue,
            };
            let Some(attacker) = g.units.get(&unit) else {
                continue;
            };
            if attacker.attacks_left <= 0 {
                continue;
            }
            // The defender the engine will resolve against: the strongest
            // military unit on the tile, as the exchange scorer reads it.
            let Some(defender) = g
                .unit_ids_at(target)
                .iter()
                .filter(|oid| {
                    let other = &g.units[oid];
                    other.owner != pid
                        && g.is_at_war(pid, other.owner)
                        && g.rules.units[other.kind].class == "military"
                })
                .max_by(|a, b| {
                    let strength = |id: &u32| {
                        let other = &g.units[id];
                        crate::game::effective_strength(g.unit_strength(other, true), other.hp)
                    };
                    strength(a).total_cmp(&strength(b)).then_with(|| a.cmp(b))
                })
            else {
                continue;
            };
            let damage = if ranged {
                let Some((att, def)) = g.ranged_strike_strengths(unit, *defender, target) else {
                    continue;
                };
                expected_damage(att, def)
            } else {
                let Some((att, def)) = g.melee_exchange_strengths(unit, *defender) else {
                    continue;
                };
                let dealt = expected_damage(att, def);
                let taken = expected_damage(def, att);
                if taken >= f64::from(attacker.hp) && dealt < f64::from(g.units[&defender].hp) {
                    continue;
                }
                dealt
            };
            shots.push(Shot {
                unit,
                target,
                defender: *defender,
                damage,
                ranged,
            });
        }
        shots
    }

    /// Greedy kill allocation over priced shots. Each round plans the target
    /// whose finish costs the fewest shooters — ties to the most valuable
    /// target, then the lowest hit points — spending ranged shots before
    /// melee and the heaviest blow first, with `KILL_MARGIN` over the
    /// centre roll where a spare shooter allows it.
    fn allocate_kills(g: &Game, shots: &[Shot]) -> FirePlan {
        let mut plan = FirePlan::default();
        let mut spent: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        let mut finished: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        let mut rank = 0u32;
        loop {
            let mut best: Option<(usize, f64, i32, u32, Vec<&Shot>)> = None;
            let mut defenders: Vec<u32> = shots.iter().map(|shot| shot.defender).collect();
            defenders.sort_unstable();
            defenders.dedup();
            for defender in defenders {
                if finished.contains(&defender) {
                    continue;
                }
                let Some(victim) = g.units.get(&defender) else {
                    continue;
                };
                let hp = f64::from(victim.hp);
                let mut available: Vec<&Shot> = shots
                    .iter()
                    .filter(|shot| shot.defender == defender && !spent.contains(&shot.unit))
                    .collect();
                // One shot per shooter: the heaviest it has on this target.
                available.sort_by(|a, b| {
                    a.unit
                        .cmp(&b.unit)
                        .then_with(|| b.damage.total_cmp(&a.damage))
                });
                available.dedup_by_key(|shot| shot.unit);
                // Ranged first — a shot returns nothing — then the heaviest
                // blow, then the lowest id, so the plan is a function of the
                // board alone.
                available.sort_by(|a, b| {
                    b.ranged
                        .cmp(&a.ranged)
                        .then_with(|| b.damage.total_cmp(&a.damage))
                        .then_with(|| a.unit.cmp(&b.unit))
                });
                let mut chosen: Vec<&Shot> = Vec::new();
                let mut planned = 0.0;
                for shot in available.iter().take(SHOOTERS_PER_KILL) {
                    if planned >= hp * KILL_MARGIN {
                        break;
                    }
                    chosen.push(shot);
                    planned += shot.damage;
                }
                if planned < hp {
                    continue;
                }
                let value = g.rules.units[victim.kind].cost;
                let candidate = (chosen.len(), value, victim.hp, defender, chosen);
                let better = match &best {
                    None => true,
                    Some(current) => {
                        candidate.0 < current.0
                            || (candidate.0 == current.0
                                && (candidate.1 > current.1
                                    || (candidate.1 == current.1
                                        && (candidate.2 < current.2
                                            || (candidate.2 == current.2
                                                && candidate.3 < current.3)))))
                    }
                };
                if better {
                    best = Some(candidate);
                }
            }
            let Some((_, _, _, defender, chosen)) = best else {
                break;
            };
            for shot in chosen {
                spent.insert(shot.unit);
                plan.assigned.insert(shot.unit, (shot.target, rank));
                rank += 1;
            }
            finished.insert(defender);
            plan.kills += 1;
        }
        plan
    }

    /// Where a unit goes in the unit order: planned shooters first, in plan
    /// order; everyone else after, in the shipped order. Zero for every unit
    /// with the gene off, so the shipped key is unchanged.
    pub(super) fn fire_plan_rank(&self, uid: u32) -> u32 {
        match self.fire_plan_orders.assigned.get(&uid) {
            Some((_, rank)) => *rank,
            None if self.fire_plan_orders.assigned.is_empty() => 0,
            None => u32::MAX,
        }
    }

    /// The tile a planned shooter is to strike this turn, if any.
    pub(super) fn fire_plan_target(&self, uid: u32) -> Option<Pos> {
        self.fire_plan_orders
            .assigned
            .get(&uid)
            .map(|(target, _)| *target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctrine::{build, position};
    use crate::hex;

    fn open_field() -> Game {
        let mut g = build(position("the_reserve").expect("known"), 3).expect("buildable");
        let seeded: Vec<u32> = (0..2).flat_map(|pid| g.player_unit_ids(pid)).collect();
        for uid in seeded {
            g.remove_unit(uid);
        }
        g
    }

    fn at(col: i32, row: i32) -> Pos {
        hex::offset_to_axial(col, row)
    }

    #[test]
    fn the_gene_ships_off_and_is_registered() {
        let ai = AdvancedAi::new();
        assert!(!ai.fire_plan, "an opt-in ships off");
        assert!(super::super::GENES
            .iter()
            .any(|gene| gene.opt_in() && gene.field == "fire_plan"));
        let mut on = AdvancedAi::new();
        on.enable_fire_plan();
        assert!(on.fire_plan);
        on.disable_fire_plan();
        assert!(!on.fire_plan);
    }

    /// Two archers and a warrior beside a wounded enemy: the plan finishes
    /// it with the two ranged shots, ranked first and ahead of the warrior,
    /// and leaves the warrior to the ordinary scorer.
    #[test]
    fn two_archers_finish_the_wounded_warrior_before_the_melee_moves() {
        let mut g = open_field();
        // (9,6) and (9,8) are two tiles from (10,7) on the odd-row board;
        // (9,7) is beside it.
        let archer_a = g.spawn_unit("archer", 0, at(9, 6));
        let archer_b = g.spawn_unit("archer", 0, at(9, 8));
        let warrior = g.spawn_unit("warrior", 0, at(9, 7));
        let victim = g.spawn_unit("warrior", 1, at(10, 7));
        g.units.get_mut(&victim).expect("victim").hp = 55;
        let mut ai = AdvancedAi::new();
        assert_eq!(ai.fire_plan_rank(warrior), 0, "off, every unit ranks zero");
        ai.enable_fire_plan();
        ai.plan_fire(&g, 0);
        let plan = &ai.fire_plan_orders;
        assert_eq!(plan.kills, 1, "{plan:?}");
        assert_eq!(ai.fire_plan_target(archer_a), Some(at(10, 7)));
        assert_eq!(ai.fire_plan_target(archer_b), Some(at(10, 7)));
        assert!(
            ai.fire_plan_rank(archer_a) < ai.fire_plan_rank(warrior)
                && ai.fire_plan_rank(archer_b) < ai.fire_plan_rank(warrior),
            "the shooters go first: {plan:?}"
        );
        assert!(
            ai.fire_plan_target(warrior).is_none(),
            "two shots finish a 55-hp warrior with margin; the melee is spared: {plan:?}"
        );
    }

    /// A single archer against a whole warrior cannot finish it, so nothing
    /// is planned and the order is the shipped one.
    #[test]
    fn a_kill_that_cannot_be_finished_is_not_planned() {
        let mut g = open_field();
        let archer = g.spawn_unit("archer", 0, at(9, 6));
        g.spawn_unit("warrior", 1, at(10, 7));
        let mut ai = AdvancedAi::new();
        ai.enable_fire_plan();
        ai.plan_fire(&g, 0);
        assert_eq!(ai.fire_plan_orders, FirePlan::default());
        assert_eq!(ai.fire_plan_rank(archer), 0);
    }

    /// With the gene off, the plan stays empty whatever the board says.
    #[test]
    fn off_the_plan_is_empty() {
        let mut g = open_field();
        g.spawn_unit("archer", 0, at(8, 6));
        g.spawn_unit("archer", 0, at(8, 8));
        let victim = g.spawn_unit("warrior", 1, at(10, 7));
        g.units.get_mut(&victim).expect("victim").hp = 40;
        let mut ai = AdvancedAi::new();
        ai.plan_fire(&g, 0);
        assert_eq!(ai.fire_plan_orders, FirePlan::default());
    }
}
