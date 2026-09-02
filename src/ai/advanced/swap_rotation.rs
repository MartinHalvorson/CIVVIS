//! Swap rotation: the wounded front-liner steps back and the fresh unit
//! behind it takes its place, in one action.
//!
//! `Action::Swap` has been in the engine — legal, tested, and refused
//! correctly for linked escorts, mismatched stacking layers and tiles
//! carrying based aircraft — since `do_swap` landed, and **no controller has
//! ever chosen one**. `docs/MOVEMENT.md` names this gene as the obvious
//! first use, and says why the rule exists at all: *"it is the reason that
//! rule costs a front line nothing: a damaged unit trades places with the
//! healthy one behind it without either leaving the line."*
//!
//! What the controller does today instead: `healing_step` files a unit below
//! `withdraw_hp` into `recovering_units` and `retreat_step` walks it away.
//! The unit leaves — and so does the tile it was holding. On a two-tile
//! front, that is a hole the enemy walks into; on a siege ring it is the
//! ring opening. The doctrine arena's `the_reserve` is the position built to
//! charge for exactly this kind of piecemeal handling, and the live ledger's
//! shape for it is 73 of 231 combat losses taken at a strength gap over
//! thirty points.
//!
//! A rotation is one action and costs both units their fortification and a
//! step's movement, which is what makes it a trade rather than a free
//! reposition. So the gene is deliberately narrow:
//!
//! - the unit rotating out is **in contact** (a hostile adjacent) and at or
//!   below `w.withdraw_hp` — the same line recovery already uses, so the
//!   gene rotates exactly the units the controller was about to walk away;
//! - the unit rotating in is an adjacent friendly **melee-capable** military
//!   unit, healthier by `ROTATION_HP_MARGIN`, and **further from the enemy**
//!   than the wounded one — it is behind the line, not beside it on the same
//!   front, so the swap does not simply exchange two exposed tiles;
//! - it never swaps a shooter forward: a ranged unit in contact is the
//!   position `screen-the-shooters` exists to avoid;
//! - one rotation per unit per turn, and the engine refuses anything its own
//!   rules forbid, so the gene never has to reproduce them.
//!
//! ⚠ **The healing is not where the value is, and that was worth finding
//! out.** This gene was written expecting to need `--heal`: nothing recovers
//! on a Tactics arena, so a unit rotated out looked like a unit that had
//! simply left the fight. Measured both ways on the same forty seeds, the
//! curriculum reads **+15.6 ± 11.3 a seed with healing on and +14.4 ± 9.6
//! with it off** — the same number. The wounded unit does not have to come
//! back for the rotation to pay. What pays is that the tile stays held: the
//! swap is the one action in the engine that takes a unit out of the line
//! without opening it, and the alternative — recovery walking it away — is a
//! hole whether or not anything heals.
//!
//! `Kind::OptIn`, off in `AdvancedAi::new()` and `legacy()`, byte-identical
//! when off. Priced on the arena first; the whole-game screen is the
//! no-harm check (`docs/DOCTRINE_ARENA.md`, "The gate for a tactical gene").
//!
//! **Live bridge.** The host's own swap operation was not driven over the
//! Civilization VI channel when V1 landed, and two `MOVE_TO` orders cannot
//! emulate one — the first is refused. The bridge now
//! translates `Action::Swap` as the host's `SWAP_UNITS` operation, so a
//! controller choice here reaches both native and live play.
//!
//! **Version two preserves V1 and adds the emergency V1 cannot see.** At or
//! below `withdraw_hp` it takes exactly V1's relief, including its health-first
//! order. Above that static line V2 also rotates a unit when the top of the
//! visible attackers' damage rolls can remove it next turn. That extra case is
//! accepted only when the wounded unit takes less roll-top damage on the
//! relief's tile and the relief survives the same field on the front. It does
//! not widen the geometry to siege rings or alter the joint battle planner's
//! separately measured rotation policy.

use super::{AdvancedAi, ForcePosture};
use crate::ai::{BasicAi, COMBAT_ROLL_MAX};
use crate::game::{Action, Game};

/// How much healthier the relief has to be before a rotation is worth two
/// units' fortification and a step each. Twenty-five hit points: below that
/// the pair can trade places every turn without either becoming fit to
/// fight, which is the failure mode `arrival-waves` was removed for in a
/// different shape.
pub(super) const ROTATION_HP_MARGIN: i32 = 25;

impl AdvancedAi {
    /// The relief this unit should trade places with, if the gene is on and
    /// the board offers one.
    ///
    /// Deliberately a pure read: it names the swap and the caller applies
    /// it, so the choice is testable without a board mutation and the engine
    /// keeps the last word on legality.
    pub(super) fn swap_rotation_relief(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        _objective: Option<crate::Pos>,
    ) -> Option<u32> {
        if !self.swap_rotation && !self.swap_rotation_2 {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if unit.owner != pid || unit.moves_left <= 0.0 || unit.linked_to.is_some() {
            return None;
        }
        if g.rules.units[unit.kind].class != "military" {
            return None;
        }
        let withdraw_at = self.base.w.withdraw_hp.round() as i32;
        // V1 is left exact below its static line. V2's only new reach is a
        // unit above that line whose visible roll-top field is already lethal.
        let emergency = if self.swap_rotation_2 && unit.hp > withdraw_at {
            let envelopes = self.base.enemy_attack_envelopes(g, pid);
            let here =
                BasicAi::incoming_damage(g, pid, uid, unit.pos, &envelopes).total * COMBAT_ROLL_MAX;
            (here + f64::EPSILON >= f64::from(unit.hp)).then_some((envelopes, here))
        } else {
            None
        };
        if unit.hp > withdraw_at && emergency.is_none() {
            return None;
        }
        // In contact: the whole point is to hold the tile while the wounded
        // unit leaves it. A unit nobody is standing next to can simply walk.
        let hostile_reach = |pos: crate::Pos| {
            g.units
                .values()
                .filter(|other| {
                    other.owner != pid
                        && g.is_at_war(pid, other.owner)
                        && g.rules.units[other.kind].class == "military"
                })
                .map(|other| g.wdist(pos, other.pos))
                .min()
        };
        let ours = hostile_reach(unit.pos)?;
        if ours > 1 {
            return None;
        }
        // The relief: adjacent, ours, military, melee-capable, healthier by
        // the margin, and standing further from the enemy contact. The V2-only
        // emergency also has to make both resulting stands survivable.
        let mut best: Option<(i32, u32)> = None;
        for pos in g.nbrs(unit.pos) {
            for other_id in g.unit_ids_at(pos) {
                let Some(other) = g.units.get(other_id) else {
                    continue;
                };
                if other.owner != pid || other.id == uid || other.linked_to.is_some() {
                    continue;
                }
                if other.moves_left <= 0.0 {
                    continue;
                }
                let spec = &g.rules.units[other.kind];
                if spec.class != "military" || !spec.is_melee_capable() {
                    continue;
                }
                if other.hp < unit.hp + ROTATION_HP_MARGIN {
                    continue;
                }
                let theirs = hostile_reach(other.pos).unwrap_or(i32::MAX);
                if theirs <= ours {
                    continue;
                }
                if let Some((envelopes, here)) = emergency.as_ref() {
                    let withdrawal = BasicAi::incoming_damage(g, pid, uid, other.pos, envelopes)
                        .total
                        * COMBAT_ROLL_MAX;
                    let relief = BasicAi::incoming_damage(g, pid, other.id, unit.pos, envelopes)
                        .total
                        * COMBAT_ROLL_MAX;
                    if withdrawal + f64::EPSILON >= *here
                        || relief + f64::EPSILON >= f64::from(other.hp)
                    {
                        continue;
                    }
                }
                // V2 intentionally keeps V1's health-first order so every
                // historical trigger is behavior-identical.
                let better =
                    best.is_none_or(|(hp, id)| other.hp > hp || (other.hp == hp && other.id < id));
                if better {
                    best = Some((other.hp, other.id));
                }
            }
        }
        best.map(|(_, id)| id)
    }

    /// Rotate the wounded front-liner out if the board offers a relief.
    /// `Some(true)` when the swap happened, `None` when the gene declines —
    /// so the caller's ordinary ladder continues untouched with the gene off.
    pub(super) fn swap_rotation_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        // A front only exists while a group is fighting for it. A muster or a
        // march has no line to keep whole, and `Recover` is the posture that
        // has already decided to leave.
        let group = self
            .force_groups
            .iter()
            .find(|group| group.units.contains(&uid));
        let objective = group.map(|group| group.objective);
        let engaged = group.is_some_and(|group| {
            matches!(group.posture, ForcePosture::Engage | ForcePosture::Hold)
        });
        if !engaged {
            return None;
        }
        let relief = self.swap_rotation_relief(g, pid, uid, objective)?;
        let action = Action::Swap {
            unit: uid,
            other: relief,
        };
        // The engine owns legality: a linked escort, a mismatched stacking
        // layer, based aircraft, a step either half cannot pay. A refusal is
        // a decline, not a fallback into some other behaviour.
        if g.apply(pid, &action).is_ok() {
            self.census.swap_rotations += 1;
            self.force_groups_dirty = true;
            return Some(true);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctrine::{build, position};
    use crate::hex;
    use crate::Pos;

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

    fn wound(g: &mut Game, uid: u32, hp: i32) {
        g.units.get_mut(&uid).expect("unit").hp = hp;
    }

    #[test]
    fn the_gene_ships_off_and_is_registered() {
        let ai = AdvancedAi::new();
        assert!(!ai.swap_rotation, "an opt-in ships off");
        assert!(!ai.swap_rotation_2, "the successor also ships off");
        assert!(super::super::GENES
            .iter()
            .any(|gene| gene.opt_in() && gene.field == "swap_rotation"));
        assert!(super::super::GENES
            .iter()
            .any(|gene| gene.opt_in() && gene.field == "swap_rotation_2"));
        let mut on = AdvancedAi::new();
        on.enable_swap_rotation();
        assert!(on.swap_rotation);
        assert!(!on.swap_rotation_2);
        on.enable_swap_rotation_2();
        assert!(!on.swap_rotation);
        assert!(on.swap_rotation_2);
        on.enable_swap_rotation();
        assert!(on.swap_rotation);
        assert!(!on.swap_rotation_2, "selecting v1 clears v2");
        on.disable_swap_rotation();
        assert!(!on.swap_rotation);
    }

    /// The line holds while the wounded unit leaves it: after the rotation
    /// the front tile is still ours, and it is the fresh unit standing on it.
    #[test]
    fn the_wounded_front_liner_trades_places_with_the_fresh_one_behind() {
        let mut g = open_field();
        let front = at(10, 6);
        let behind = at(9, 6);
        let hurt = g.spawn_unit("warrior", 0, front);
        let fresh = g.spawn_unit("warrior", 0, behind);
        g.spawn_unit("warrior", 1, at(11, 6));
        wound(&mut g, hurt, 30);
        let mut ai = AdvancedAi::new();
        assert!(
            ai.swap_rotation_relief(&g, 0, hurt, None).is_none(),
            "off, the gene names nothing"
        );
        ai.enable_swap_rotation();
        assert_eq!(ai.swap_rotation_relief(&g, 0, hurt, None), Some(fresh));
        assert!(g
            .apply(
                0,
                &Action::Swap {
                    unit: hurt,
                    other: fresh
                }
            )
            .is_ok());
        assert_eq!(g.units[&fresh].pos, front, "the fresh unit holds the line");
        assert_eq!(
            g.units[&hurt].pos, behind,
            "and the wounded one is out of it"
        );
    }

    /// Every condition, one at a time — each is a way the rotation would be
    /// a worse move than the ordinary ladder's.
    #[test]
    fn the_rotation_declines_what_it_should() {
        let front = at(10, 6);
        let behind = at(9, 6);
        let enemy = at(11, 6);
        let setup = |hurt_hp: i32, relief_kind: &str, relief_hp: i32, relief_at: Pos| {
            let mut g = open_field();
            let hurt = g.spawn_unit("warrior", 0, front);
            let relief = g.spawn_unit(relief_kind, 0, relief_at);
            g.spawn_unit("warrior", 1, enemy);
            wound(&mut g, hurt, hurt_hp);
            wound(&mut g, relief, relief_hp);
            let mut ai = AdvancedAi::new();
            ai.enable_swap_rotation();
            (g, ai, hurt, relief)
        };
        // A healthy front-liner is not rotated: it is still the right unit
        // for the tile.
        let (g, ai, hurt, _) = setup(90, "warrior", 100, behind);
        assert_eq!(ai.swap_rotation_relief(&g, 0, hurt, None), None);
        // A relief no healthier than the unit it replaces buys nothing.
        let (g, ai, hurt, _) = setup(30, "warrior", 40, behind);
        assert_eq!(ai.swap_rotation_relief(&g, 0, hurt, None), None);
        // A shooter is not put into contact; that is the position
        // `screen-the-shooters` exists to avoid.
        let (g, ai, hurt, _) = setup(30, "archer", 100, behind);
        assert_eq!(ai.swap_rotation_relief(&g, 0, hurt, None), None);
        // A relief no further from the enemy than we are is not behind the
        // line — swapping two exposed tiles is not a rotation.
        let (g, ai, hurt, _) = setup(30, "warrior", 100, at(10, 5));
        assert_eq!(
            g.wdist(at(10, 5), enemy),
            g.wdist(front, enemy),
            "the fixture puts both on the same rank"
        );
        assert_eq!(ai.swap_rotation_relief(&g, 0, hurt, None), None);
        // And a wounded unit nobody is standing next to can simply walk.
        let mut g = open_field();
        let hurt = g.spawn_unit("warrior", 0, front);
        let _fresh = g.spawn_unit("warrior", 0, behind);
        g.spawn_unit("warrior", 1, at(16, 6));
        wound(&mut g, hurt, 30);
        let mut ai = AdvancedAi::new();
        ai.enable_swap_rotation();
        assert_eq!(ai.swap_rotation_relief(&g, 0, hurt, None), None);
    }

    /// Below the historical withdrawal line V2 is deliberately V1: even when
    /// a stronger but slightly less healthy relief exists, both versions make
    /// the same health-first choice.
    #[test]
    fn version_two_preserves_v1_below_the_static_line() {
        let mut g = open_field();
        let front = at(10, 6);
        let hurt = g.spawn_unit("warrior", 0, front);
        let freshest = g.spawn_unit("warrior", 0, at(9, 6));
        let stronger = g.spawn_unit("swordsman", 0, at(9, 7));
        g.spawn_unit("warrior", 1, at(11, 6));
        wound(&mut g, hurt, 30);
        wound(&mut g, stronger, 90);

        let mut v1 = AdvancedAi::new();
        v1.enable_swap_rotation();
        assert_eq!(v1.swap_rotation_relief(&g, 0, hurt, None), Some(freshest));

        let mut v2 = AdvancedAi::new();
        v2.enable_swap_rotation_2();
        assert_eq!(
            v2.swap_rotation_relief(&g, 0, hurt, None),
            Some(freshest),
            "the new version does not rewrite V1's proven trigger"
        );
    }

    /// Above the static line V1 sees no recovery event. V2 sees that the top
    /// of the visible blow can remove the unit, that the city behind is safer,
    /// and that the modern relief survives the inherited front.
    #[test]
    fn version_two_rotates_a_roll_top_emergency() {
        let mut g = open_field();
        let front = at(10, 6);
        let hurt = g.spawn_unit("warrior", 0, front);
        let behind = at(9, 6);
        let relief = g.spawn_unit("modern_armor", 0, behind);
        g.spawn_unit("tank", 1, at(11, 6));
        wound(&mut g, hurt, 60);
        g.place_city(0, behind, Some("Refuge".to_string()));

        let mut v1 = AdvancedAi::new();
        v1.enable_swap_rotation();
        assert_eq!(
            v1.swap_rotation_relief(&g, 0, hurt, None),
            None,
            "sixty hit points is above V1's static line"
        );

        let mut v2 = AdvancedAi::new();
        v2.enable_swap_rotation_2();
        assert_eq!(v2.swap_rotation_relief(&g, 0, hurt, None), Some(relief));
    }

    /// The step fires only for a group that is holding a front, and it
    /// counts what it did.
    #[test]
    fn the_step_rotates_only_an_engaged_front_and_is_counted() {
        let mut g = open_field();
        let front = at(10, 6);
        let hurt = g.spawn_unit("warrior", 0, front);
        let fresh = g.spawn_unit("warrior", 0, at(9, 6));
        g.spawn_unit("warrior", 1, at(11, 6));
        wound(&mut g, hurt, 30);
        let mut ai = AdvancedAi::new();
        ai.enable_swap_rotation();
        // No force groups yet: nothing is holding a front.
        assert_eq!(ai.swap_rotation_step(&mut g, 0, hurt), None);
        let plan = super::super::StrategicPlan {
            strategy: super::super::GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: g.turn,
            rush: false,
        };
        ai.rebuild_force_groups(&g, 0, &plan);
        assert_eq!(ai.swap_rotation_step(&mut g, 0, hurt), Some(true));
        assert_eq!(g.units[&fresh].pos, front);
        assert_eq!(ai.census.swap_rotations, 1);
    }
}
