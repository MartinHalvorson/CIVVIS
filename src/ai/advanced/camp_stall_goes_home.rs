//! `camp-stall-goes-home`: a ClearCamp party that stands beside a guarded
//! camp for [`CAMP_STALL_TURNS`] turns and strikes no blow closes its row
//! for [`CAMP_STALL_CLOSE_TURNS`] turns, so its soldiers go back to the
//! board's other rows (an Escort, a Defend, the Reserve at home); while the
//! row stands closed, a settle site within [`CAMP_STALL_SITE_TILES`] tiles of
//! that camp loses [`CAMP_STALL_SITE_PENALTY`].
//!
//! **Why (hotseat `hotseat-20261128T031456Z`, seat p4, Phoenicia, the siege
//! agent, 2026-11-27).** The board raised `ClearCamp camp at (35, 20)` (civ6
//! (45,20), five tiles from Tyre) on turn 7 and kept it to the end of the
//! game on turn 16, served by two and then three Warriors. A barbarian
//! Spearman (strength 25 to the Warriors' 20) held the camp from turn 10, and
//! a Warrior of ours stood within two tiles of it at the end of every turn
//! from 10 to 16. No blow was struck: the battle plan read "every blow it has
//! would leave it dead next turn" (t11, t14) and the per-unit ladder "warrior
//! declines spearman ... -26 under a margin of 0" (t12, t14, t15). The
//! Warriors spent seven turns there while Tyre's Settler, out from turn 14,
//! walked toward sites three tiles from the same camp, (38, 20) and
//! (37, 21), and was "HELD short" by the safe-step guard on turns 15 and 16.
//!
//! The board is right that a ClearCamp row with an outmatched party will not
//! be cleared; what it lacks is any memory that the party has been standing
//! there. This is that memory, and a decision on it.
//!
//! ## What the gene does
//!
//! 1. **Watch.** At the end of our unit pass, for every task force the board
//!    holds against a camp: the turn counts as a *contact* turn when one of
//!    its members stands within [`CAMP_STALL_CONTACT_TILES`] of the camp and
//!    a barbarian soldier stands on the camp tile; as a *struck* turn when
//!    any unit of ours attacked or shot at the camp tile in that pass. A
//!    turn's frames all write the same turn, so a replan frame neither
//!    double-counts nor erases.
//! 2. **Close.** When the board is drawn on turn `T` and each of the turns
//!    `T-3 .. T-1` was a contact turn and none of them a struck turn, the
//!    camp's row is not raised from `T` to `T + 10`. Its force has no row to
//!    match, so the board hands its soldiers to the other rows by the usual
//!    allocation. A Decision line names the camp, the stalled turns and the
//!    turn it reopens.
//! 3. **Reopen.** From `T + 10` the row is raised again with a clean record:
//!    three new stalled turns are needed to close it again.
//! 4. **The site.** While a row stands closed, `settle_value_visible` takes
//!    [`CAMP_STALL_SITE_PENALTY`] off every site within three tiles of that
//!    camp. A camp the army is still clearing is left to the existing camp
//!    terms (`settlement_tile_risk_with_support`, `defensibility`), because it
//!    is about to be gone; a camp the army has given up on is not.
//!
//! A camp that leaves the board (cleared by anyone) drops its record. Off,
//! nothing is written or read. Grep the journal for `camp-stall-goes-home:`.

use super::objective_board::ObjectiveKey;
use super::*;

/// Consecutive turns of contact without a blow that close a camp's row.
pub(crate) const CAMP_STALL_TURNS: u32 = 3;

/// How long a stalled camp's row stays closed, in turns.
pub(crate) const CAMP_STALL_CLOSE_TURNS: u32 = 10;

/// How near the camp a member of its party must end our unit pass for the
/// turn to count as contact: a Warrior two tiles out strikes the camp next
/// turn with its two moves.
pub(crate) const CAMP_STALL_CONTACT_TILES: i32 = 2;

/// How near a closed camp a settle site loses value: a camp's units take a
/// Settler or raid a new city from there inside one turn.
pub(crate) const CAMP_STALL_SITE_TILES: i32 = 3;

/// What a site that near a closed camp loses: about a third of the 119.8 the
/// p4 site at (38, 20) was worth, enough for a site four tiles out to win.
pub(crate) const CAMP_STALL_SITE_PENALTY: f64 = 40.0;

/// One camp's record.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CampStall {
    /// Turns a member of the camp's party ended our unit pass within
    /// [`CAMP_STALL_CONTACT_TILES`] of the camp while a barbarian soldier
    /// held it.
    pub(crate) contact: BTreeSet<u32>,
    /// Turns a unit of ours struck the camp tile.
    pub(crate) struck: BTreeSet<u32>,
    /// The turn the closed row is raised again.
    pub(crate) closed_until: Option<u32>,
}

impl AdvancedAi {
    /// Whether a barbarian soldier stands on `camp` on this board.
    fn camp_stall_guarded(g: &Game, camp: Pos) -> bool {
        g.unit_ids_at(camp).iter().any(|uid| {
            g.units.get(uid).is_some_and(|unit| {
                Some(unit.owner) == g.barb_pid
                    && unit.hp > 0
                    && g.rules.units[unit.kind].class == "military"
            })
        })
    }

    /// The watch, at the end of our unit pass: `since` is where the pass
    /// began in `g.log`. Nothing with the gene off.
    pub(super) fn observe_camp_stalls(&mut self, g: &Game, pid: usize, since: usize) {
        if !self.camp_stall_goes_home || self.base.minor || self.base.barb {
            return;
        }
        let turn = g.turn;
        self.camp_stalls
            .retain(|camp, _| g.barb_camps.contains_key(camp));
        let struck: BTreeSet<Pos> = g
            .log
            .since(since.min(g.log.len()))
            .filter(|(seat, _)| *seat == pid)
            .filter_map(|(_, action)| match action {
                Action::Attack { target, .. } | Action::Ranged { target, .. } => Some(*target),
                _ => None,
            })
            .collect();
        let parties: Vec<(Pos, bool)> = self
            .objective_board_state
            .forces
            .iter()
            .filter_map(|force| match force.objective_key {
                ObjectiveKey::Camp(camp) if g.barb_camps.contains_key(&camp) => {
                    let near = force.units.iter().any(|uid| {
                        g.units.get(uid).is_some_and(|unit| {
                            unit.owner == pid && g.wdist(unit.pos, camp) <= CAMP_STALL_CONTACT_TILES
                        })
                    });
                    Some((camp, near))
                }
                _ => None,
            })
            .collect();
        for (camp, near) in parties {
            let guarded = Self::camp_stall_guarded(g, camp);
            let stall = self.camp_stalls.entry(camp).or_default();
            if stall.closed_until.is_some() {
                continue;
            }
            if struck.contains(&camp) {
                stall.struck.insert(turn);
            }
            if near && guarded {
                stall.contact.insert(turn);
            }
            // Only the turns the close reads are kept.
            let oldest = turn.saturating_sub(CAMP_STALL_TURNS);
            stall.contact.retain(|seen| *seen >= oldest);
            stall.struck.retain(|seen| *seen >= oldest);
        }
    }

    /// Whether the board raises no ClearCamp row for `camp` on this turn:
    /// the close and the reopen, read when the board is drawn. Always
    /// `false` with the gene off.
    pub(super) fn camp_stall_closes_the_row(&mut self, g: &Game, camp: Pos) -> bool {
        if !self.camp_stall_goes_home {
            return false;
        }
        let turn = g.turn;
        let Some(stall) = self.camp_stalls.get_mut(&camp) else {
            return false;
        };
        if let Some(until) = stall.closed_until {
            if turn < until {
                return true;
            }
            // Reopened: the party starts a clean record.
            *stall = CampStall::default();
            think!(self.journal(), Military, Decision,
                "The camp at {camp:?} is a ClearCamp row again";
                "camp-stall-goes-home: its row was closed until turn {until}; three new \
                 turns beside it without a blow would close it again");
            return false;
        }
        if turn < CAMP_STALL_TURNS {
            return false;
        }
        let first = turn - CAMP_STALL_TURNS;
        let stalled = (first..turn)
            .all(|seen| stall.contact.contains(&seen) && !stall.struck.contains(&seen));
        if !stalled {
            return false;
        }
        let until = turn + CAMP_STALL_CLOSE_TURNS;
        stall.closed_until = Some(until);
        stall.contact.clear();
        stall.struck.clear();
        let last = turn - 1;
        think!(self.journal(), Military, Decision,
            "The party at the camp at {camp:?} goes home";
            "camp-stall-goes-home: it stood beside the guarded camp on turns {first}-{last} and \
             struck no blow; the row is closed until turn {until} and its soldiers serve the \
             other rows, and sites within {CAMP_STALL_SITE_TILES} tiles of the camp lose \
             {CAMP_STALL_SITE_PENALTY:.0}");
        true
    }

    /// What a settle site at `pos` loses to a camp whose row this gene has
    /// closed: [`CAMP_STALL_SITE_PENALTY`] within [`CAMP_STALL_SITE_TILES`]
    /// of one, else nothing. Zero with the gene off.
    pub(super) fn camp_stall_site_penalty(&self, g: &Game, pos: Pos) -> f64 {
        if !self.camp_stall_goes_home {
            return 0.0;
        }
        let turn = g.turn;
        let near_a_closed_camp = self.camp_stalls.iter().any(|(camp, stall)| {
            stall.closed_until.is_some_and(|until| turn < until)
                && g.barb_camps.contains_key(camp)
                && g.wdist(*camp, pos) <= CAMP_STALL_SITE_TILES
        });
        if near_a_closed_camp {
            CAMP_STALL_SITE_PENALTY
        } else {
            0.0
        }
    }
}
