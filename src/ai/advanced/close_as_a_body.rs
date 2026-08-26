//! Close as a body, and screen the shooters: two opt-in genes in the
//! deployed mover's tile score.
//!
//! The doctrine arena's one replicated causal finding is that the share of
//! the force up at first contact predicts the material swing — r ≈ +0.30 on
//! `central_position`, about a thousand material per whole extra share —
//! and its one replicated fact about the deployed controller is that it
//! arrives over roughly twice the span `basic` does (17 of 20 cells) and
//! leaves its ranged units unscreened (25–32 % of shooter-turns against
//! 39–50 %). It trades better once engaged and assembles worse getting
//! there, and `the_reserve` — the position built to charge for the second
//! thing — is the one it loses (−189 material a seed, p = 0.0005). The live
//! ledger's shape for the same thing is 73 of 231 combat losses taken at a
//! strength gap over thirty points: units fed into the fight one at a time.
//!
//! Both genes are terms in `coordinated_tactical_step`'s one-ply tile
//! score, which already knows every member of the force and already carries
//! cohesion, role-depth and a `screen` term of its own. Neither adds a
//! stand: `arrival-waves` held reinforcements out of contact until a wave
//! formed and priced at −3.0 pp, and `contact-posture` stood and received
//! and priced at −1.14. Here every unit spends its movement every turn;
//! what changes is which tile it spends it toward.
//!
//! - **`close-as-a-body`** — on an `Advance`, no unit ends the turn more
//!   than the body's *pace* (the slowest member's movement) plus one tile
//!   closer to the objective than the force's anchor stood at the start of
//!   the turn. A horseman four tiles ahead of the foot is a horseman that
//!   meets the enemy alone; the term makes its forward tiles cost more than
//!   they pay, and it takes the tile beside the line instead. Recon is
//!   exempt — a scout's job is to be ahead — and a unit already in contact
//!   is left to the ordinary score, because the fight has started and the
//!   body is wherever it is.
//! - **`screen-the-shooters`** — the arena's own definition of *screened*
//!   ("a friendly stands beside the shooter and closer to the nearest enemy
//!   than it does"), paid to the shooter: a ranged or siege tile earns two
//!   screen weights when a melee friend stands beside it and nearer the
//!   enemy. The shipped `w.screen` term prices depth along the objective
//!   axis and does not ask for adjacency or for the enemy's actual
//!   direction, which is why a shooter can satisfy it and still stand alone
//!   at the front. The melee half of the same idea — a melee tile paid for
//!   standing in front of an unscreened shooter — was measured and dropped:
//!   it cost −19 ± 12 a seed on the curriculum (`lake_trasimene` −99) and
//!   added nothing to the skirmish, because a line that steps sideways to
//!   cover its archers is a line that is not pressing.
//!
//! Off in `AdvancedAi::new()` and `legacy()`, `Kind::OptIn` rows in
//! `genes.rs`, byte-identical when off: with both flags off neither helper
//! runs and the score is the shipped score to the bit. Priced first on the
//! arena (`doctrine_arena --a advanced+close-as-a-body`, the curriculum and a
//! captured engagement file, healing off and on) and on `battle_bench`; the
//! whole-game screen is the no-harm check afterwards. See
//! `docs/DOCTRINE_ARENA.md`, "The gate for a tactical gene".

use super::{AdvancedAi, ForceGroup, ForcePosture, ForceRole};
use crate::game::Game;
use crate::Pos;

/// Tiles a marcher may end the turn ahead of the pace before the term
/// charges it. One: a body is not a parade, and the tile beside the line is
/// still the line.
pub(super) const BODY_PACE_SLACK: i32 = 1;
/// Per tile beyond the slack, in units of the role's own objective-progress
/// term. Two, so that ahead of the body a tile further forward always costs
/// more than it pays, and the unit prefers the tile that keeps station.
const BODY_PACE_PENALTY: f64 = 2.0;
/// A unit with an enemy this close is in contact, and the pace no longer
/// applies to it — the same two tiles the contact zone and the arrival
/// profile use.
pub(super) const CONTACT_RANGE: i32 = 2;
/// A shooter is worth screening when an enemy stands within this many tiles
/// of the force; beyond it there is nothing to screen against yet and the
/// march terms decide.
const SCREEN_BAND: i32 = 6;
/// The screened tile, in screen weights. Two: the screened tile is usually
/// the one a step behind the exposed one, and the shipped depth and spacing
/// terms price that step at about five, so one screen weight would never
/// win it and the term would be decoration.
const SHOOTER_SCREEN_WEIGHT: f64 = 2.0;

/// The pace of a force on the march, read once per unit turn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BodyPace {
    /// How far the force's anchor stood from the objective at the start of
    /// the turn. The anchor is the group's medoid, fixed for the turn.
    pub anchor_distance: i32,
    /// The slowest member's movement — what the body can cover this turn.
    pub pace: i32,
}

/// What a shooter's tile is read against: the force's melee with their
/// distance to the nearest enemy, and the enemies.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ScreenFrame {
    /// Each melee member other than the mover: its tile and its distance to
    /// the nearest enemy.
    pub melee: Vec<(Pos, i32)>,
    pub enemies: Vec<Pos>,
}

fn nearest(g: &Game, from: Pos, to: &[Pos]) -> i32 {
    to.iter()
        .map(|pos| g.wdist(from, *pos))
        .min()
        .unwrap_or(i32::MAX)
}

impl AdvancedAi {
    /// The pace the mover is held to, or `None` when the gene is off, the
    /// force is not advancing, the unit is alone, a scout, or already in
    /// contact.
    pub(super) fn body_pace(
        &self,
        g: &Game,
        uid: u32,
        group: &ForceGroup,
        target: Pos,
        enemies: &[Pos],
    ) -> Option<BodyPace> {
        if !self.close_as_a_body
            || group.posture != ForcePosture::Advance
            || group.units.len() < 2
            || Self::force_role(g, uid) == ForceRole::Recon
        {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if nearest(g, unit.pos, enemies) <= CONTACT_RANGE {
            return None;
        }
        let pace = group
            .units
            .iter()
            .filter_map(|other| g.units.get(other))
            .map(|other| g.rules.units[other.kind].moves.floor() as i32)
            .min()?
            .max(1);
        Some(BodyPace {
            anchor_distance: g.wdist(group.anchor, target),
            pace,
        })
    }

    /// What a tile costs for being ahead of the body: nothing inside the
    /// pace plus the slack, and twice the role's progress term for every
    /// tile beyond it.
    pub(super) fn body_pace_penalty(
        &self,
        g: &Game,
        tile: Pos,
        target: Pos,
        pace: &BodyPace,
        progress: f64,
    ) -> f64 {
        let ahead = pace.anchor_distance - g.wdist(tile, target);
        let excess = ahead - (pace.pace + BODY_PACE_SLACK);
        if excess <= 0 {
            return 0.0;
        }
        BODY_PACE_PENALTY * self.base.w.objective_progress * progress * f64::from(excess)
    }

    /// The force's melee as the screen term reads them, or `None` when the
    /// gene is off, the mover is not a shooter, the unit is alone, or no
    /// enemy is near enough to screen against.
    pub(super) fn screen_frame(
        &self,
        g: &Game,
        uid: u32,
        group: &ForceGroup,
        enemies: &[Pos],
    ) -> Option<ScreenFrame> {
        if !self.screen_the_shooters
            || group.units.len() < 2
            || enemies.is_empty()
            || !matches!(
                Self::force_role(g, uid),
                ForceRole::Ranged | ForceRole::Siege
            )
        {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if nearest(g, unit.pos, enemies) > SCREEN_BAND {
            return None;
        }
        let melee: Vec<(Pos, i32)> = group
            .units
            .iter()
            .filter(|other| **other != uid)
            .filter_map(|other| {
                let stands = g.units.get(other)?;
                matches!(
                    Self::force_role(g, *other),
                    ForceRole::Vanguard | ForceRole::Mobile
                )
                .then(|| (stands.pos, nearest(g, stands.pos, enemies)))
            })
            .collect();
        if melee.is_empty() {
            return None;
        }
        Some(ScreenFrame {
            melee,
            enemies: enemies.to_vec(),
        })
    }

    /// The screen a shooter's tile receives: `SHOOTER_SCREEN_WEIGHT` screen
    /// weights when a melee friend stands beside the tile and nearer the
    /// enemy than it, else nothing.
    pub(super) fn screen_bonus(&self, g: &Game, tile: Pos, frame: &ScreenFrame) -> f64 {
        let reach = nearest(g, tile, &frame.enemies);
        let covered = frame
            .melee
            .iter()
            .any(|(pos, their_reach)| g.wdist(tile, *pos) == 1 && *their_reach < reach);
        if covered {
            SHOOTER_SCREEN_WEIGHT * self.base.w.screen
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctrine::{build, position};
    use crate::hex;

    /// An empty arena board with the reserve's dimensions, both seats at war.
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

    /// A force of every unit seat 0 owns, on an advance toward `objective`,
    /// anchored on the medoid the way `rebuild_force_groups` anchors it.
    fn advancing(g: &Game, objective: Pos) -> ForceGroup {
        let units = g.player_unit_ids(0);
        let anchor = units
            .iter()
            .map(|uid| g.units[uid].pos)
            .min_by_key(|pos| {
                (
                    units
                        .iter()
                        .map(|other| g.wdist(*pos, g.units[other].pos))
                        .sum::<i32>(),
                    *pos,
                )
            })
            .expect("a force");
        ForceGroup {
            id: units[0],
            domain: super::super::ForceDomain::Land,
            units,
            anchor,
            objective,
            focus_target: None,
            posture: ForcePosture::Advance,
            readiness: 1.0,
            local_strength_ratio: 1.0,
        }
    }

    /// Walk one unit through the deployed mover until it stops, as
    /// `advance_unit_serial` does.
    fn march(g: &mut Game, ai: &AdvancedAi, uid: u32, group: &ForceGroup) {
        for _ in 0..8 {
            if !ai.coordinated_tactical_step(g, 0, uid, group, &[1], false) {
                break;
            }
        }
    }

    #[test]
    fn the_genes_ship_off_and_are_registered() {
        let ai = AdvancedAi::new();
        assert!(!ai.close_as_a_body, "an opt-in ships off");
        assert!(!ai.screen_the_shooters, "an opt-in ships off");
        for field in ["close_as_a_body", "screen_the_shooters"] {
            assert!(
                super::super::GENES
                    .iter()
                    .any(|gene| gene.opt_in() && gene.field == field),
                "{field} is an opt-in gene"
            );
        }
        let mut on = AdvancedAi::new();
        on.enable_close_as_a_body();
        on.enable_screen_the_shooters();
        assert!(on.close_as_a_body && on.screen_the_shooters);
        on.disable_close_as_a_body();
        on.disable_screen_the_shooters();
        assert!(!on.close_as_a_body && !on.screen_the_shooters);
    }

    /// A horseman already ahead of the foot: off, the march terms send it
    /// further forward (progress outweighs cohesion by a point a tile for a
    /// Mobile role); on, every forward tile is beyond the body's pace and it
    /// falls back toward the line instead. The gene changes the tile.
    #[test]
    fn a_horseman_ahead_of_the_foot_falls_back_to_the_body() {
        let run = |gene: bool| -> i32 {
            let mut g = open_field();
            for col in 3..=5 {
                g.spawn_unit("warrior", 0, at(col, 6));
            }
            let horse = g.spawn_unit("horseman", 0, at(9, 6));
            let enemy = at(19, 6);
            g.spawn_unit("warrior", 1, enemy);
            g.spawn_unit("warrior", 1, at(20, 6));
            let mut ai = AdvancedAi::new();
            if gene {
                ai.enable_close_as_a_body();
            }
            let group = advancing(&g, enemy);
            let before = g.wdist(g.units[&horse].pos, enemy);
            march(&mut g, &ai, horse, &group);
            before - g.wdist(g.units[&horse].pos, enemy)
        };
        let ahead_off = run(false);
        let ahead_on = run(true);
        assert!(
            ahead_off > 0,
            "off, the horseman goes forward ({ahead_off})"
        );
        assert!(
            ahead_on <= 0,
            "on, the horseman holds or falls back ({ahead_on})"
        );
    }

    /// The penalty is zero inside the pace and grows by two progress terms a
    /// tile beyond it; a unit in contact, a scout, and a force that is not
    /// advancing are not paced at all.
    #[test]
    fn the_pace_is_read_only_on_an_advance_out_of_contact() {
        let mut ai = AdvancedAi::new();
        ai.enable_close_as_a_body();
        let g = open_field();
        let pace = BodyPace {
            anchor_distance: 10,
            pace: 2,
        };
        let target = at(19, 6);
        let inside = at(12, 6); // 7 away: three closer than the anchor, inside pace + slack
        let beyond = at(14, 6); // 5 away: five closer, two beyond
        assert_eq!(ai.body_pace_penalty(&g, inside, target, &pace, 1.0), 0.0);
        let charged = ai.body_pace_penalty(&g, beyond, target, &pace, 1.0);
        assert!((charged - 2.0 * ai.base.w.objective_progress * 2.0).abs() < 1e-9);
        let mut g = open_field();
        let warrior = g.spawn_unit("warrior", 0, at(4, 6));
        let _scout = g.spawn_unit("scout", 0, at(5, 6));
        let enemy = at(19, 6);
        let group = advancing(&g, target);
        assert!(ai
            .body_pace(&g, warrior, &group, target, &[enemy])
            .is_some());
        assert!(
            ai.body_pace(&g, warrior, &group, target, &[at(6, 6)])
                .is_none(),
            "a unit in contact is not paced"
        );
        let scout = g.player_unit_ids(0)[1];
        assert!(
            ai.body_pace(&g, scout, &group, target, &[enemy]).is_none(),
            "recon is exempt"
        );
        let holding = ForceGroup {
            posture: ForcePosture::Hold,
            ..group.clone()
        };
        assert!(ai
            .body_pace(&g, warrior, &holding, target, &[enemy])
            .is_none());
        ai.disable_close_as_a_body();
        assert!(ai
            .body_pace(&g, warrior, &group, target, &[enemy])
            .is_none());
    }

    /// The screen term reads the arena's own definition from the shooter's
    /// side: a tile beside a melee friend that stands nearer the enemy earns
    /// the weight; a tile in front of the friend does not; a melee mover, a
    /// force with no melee, and a distant enemy give no frame at all.
    #[test]
    fn the_screen_is_adjacency_behind_a_friend_toward_the_enemy() {
        let mut ai = AdvancedAi::new();
        ai.enable_screen_the_shooters();
        let mut g = open_field();
        let archer = g.spawn_unit("archer", 0, at(8, 6));
        let warrior = g.spawn_unit("warrior", 0, at(7, 6));
        let enemy = at(12, 6);
        g.spawn_unit("warrior", 1, enemy);
        let group = advancing(&g, enemy);
        let frame = ai
            .screen_frame(&g, archer, &group, &[enemy])
            .expect("an enemy within the band");
        assert_eq!(frame.melee, vec![(at(7, 6), 5)]);
        assert_eq!(
            ai.screen_bonus(&g, at(6, 6), &frame),
            SHOOTER_SCREEN_WEIGHT * ai.base.w.screen
        );
        assert_eq!(ai.screen_bonus(&g, at(9, 6), &frame), 0.0);
        assert!(
            ai.screen_frame(&g, warrior, &group, &[enemy]).is_none(),
            "a melee mover is not screened"
        );
        assert!(ai.screen_frame(&g, archer, &group, &[at(22, 6)]).is_none());
        ai.disable_screen_the_shooters();
        assert!(ai.screen_frame(&g, archer, &group, &[enemy]).is_none());
    }

    /// The shooter's side: with a warrior two tiles from the enemy, the
    /// archer's screened tile is the one behind the warrior at three, and the
    /// exposed tile is the one beside it at two. Off, depth and spacing take
    /// the exposed tile; on, the archer stays behind the warrior.
    #[test]
    fn the_archer_stays_behind_the_warrior() {
        let run = |gene: bool| -> (bool, Pos) {
            let mut g = open_field();
            let archer = g.spawn_unit("archer", 0, at(8, 5));
            let warrior = g.spawn_unit("warrior", 0, at(10, 6));
            let enemy = at(12, 6);
            g.spawn_unit("warrior", 1, enemy);
            g.spawn_unit("warrior", 1, at(13, 6));
            let mut ai = AdvancedAi::new();
            if gene {
                ai.enable_screen_the_shooters();
            }
            let group = advancing(&g, enemy);
            march(&mut g, &ai, archer, &group);
            let (a, w) = (g.units[&archer].pos, g.units[&warrior].pos);
            (
                g.wdist(a, w) <= 1 && g.wdist(w, enemy) < g.wdist(a, enemy),
                a,
            )
        };
        let (on, where_on) = run(true);
        assert!(
            on,
            "with the gene the archer ends screened (at {where_on:?})"
        );
        assert_eq!(
            g_dist(where_on, at(12, 6)),
            3,
            "behind the warrior, at three"
        );
    }

    fn g_dist(a: Pos, b: Pos) -> i32 {
        hex::distance(a, b)
    }
}
