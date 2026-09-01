//! `settler-walk-deadline`: a Settler that has been out of a city longer
//! than the walk that wins stops chasing a ranked site and founds the best
//! legal one it can reach at once.
//!
//! ## What the latest live runs say (forensic of 2026-09-01)
//!
//! Every recorded win came from 4–6 cities at turn 60
//! (`advanced/expansion_schedule.rs`). The 08-31 King runs on this machine
//! and the 09-01 Emperor runs on the fleet (`tools/live_ledger.py`) were
//! read settler by settler — the turn each one appeared, the turn it founded
//! or was lost, every turn it stood still and why — with all eight host-only
//! expansion genes (`parallel-settlers`, `land-grab`, `host-settler-pop`,
//! `opening-settler-waits`, `fog-land-capacity`, `era-paced-expansion`,
//! `escort-patience-runs-out`, `live-move-refusal-break`) live and
//! `expansion-schedule` forced on. Settlers counted below are those built by
//! turn 60, the first Settler excluded; "walk" is built → founded.
//!
//! | corpus | runs | cities@60 | built | founded | lost | mean walk | turns per tile | walker-turns still |
//! |---|---:|---|---:|---:|---:|---:|---:|---:|
//! | King 08-31 (local) | 7 | 4, 2, 4, 4, 5, 4, 5 | 35 | 33 | 2 | 14.9 | 1.60 | 32% |
//! | Emperor 09-01 (fleet) | 9 | 2, 4, 3, 2, 3, 2, 7, 3, 4 | 41 | 27 | **13** | 12.7 | 2.83 | **48%** |
//!
//! **The pipeline is not the blocker any more.** Classifying every city-turn
//! up to turn 60 against the schedule's own pace: on pace (cities plus
//! walkers at or above it) 402 of 407 city-turns locally and 501 of 528 on
//! Emperor; behind with a Settler in production 4 and 11; behind because no
//! city had reached the population floor **0 and 0**; behind with a Settler
//! buildable and nothing building it 1 and 16 (one Emperor run). Counting
//! walkers as cities, every one of the sixteen runs stood at 4–7 by turn 60.
//! Counting cities, they stood at 2–5.
//!
//! **The walkers do not convert.** The ten longest local walks (over 15
//! turns) took 311 of the 491 turns walked for 185 straight-line tiles; on
//! Emperor a walk covers a tile every 2.8 turns and a third of the Settlers
//! built by turn 60 were taken before founding. Where the still turns go,
//! on the corpus with a journal: the host's capture hold
//! (`settler_barbarian_combat_capture_hold`) 31% of them, the planner's own
//! "HELD short — the safe-step guard rejected every neighbour" and "holds …
//! every step is in a hostile's reach" 26%, a guard it waits for 6%, a
//! refused host move 5%, the rest unattributed. Run `civvis-20260831T085324Z`
//! is the shape of it: the second Settler left at t19 and founded at t62,
//! setting five different sites aside for "step risk above the limit" and
//! marching between them, 5–7 tiles from where it stood, for 43 turns while
//! the empire held two cities. Every pipeline rule counts that walker as a
//! city (`city_count + settlers`), so it also blocked its own replacement.
//!
//! ## What the gene does
//!
//! It puts a clock on the walk. Once a Settler has been out of a city for
//! [`SETTLER_WALK_DEADLINE_STANDARD`] standard turns (twelve on the ladder's
//! Online speed — above the median live walk and under every long one), the
//! ordinary site search is set aside and it takes the best legal site within
//! [`SETTLER_WALK_DEADLINE_RADIUS`] tiles of where it stands, the tile it
//! stands on included: founding at once if that is the best, else one safe
//! step and founding on arrival. A site within reach must beat the tile
//! underfoot by [`SETTLER_WALK_DEADLINE_STEP_MARGIN`] to be worth the extra
//! turn.
//!
//! The choice keeps every legality and Loyalty guard the exhaustion search
//! keeps (`settler_never_idles`): the host's blocked plots, this Settler's
//! dead sites, another Settler's reservation, the capture scars, a rival's
//! sphere on a Science lane, and the engine's own revolt forecast at the same
//! twenty-turn floor. The step is the ordinary safe step, so it still refuses
//! a tile a visible hostile can take. What it does NOT keep is the ranking:
//! at the deadline a city within reach beats the better city it has not
//! reached in twelve turns, and the corpus above says that trade is the
//! whole opening.
//!
//! It runs after the emergency flee and the retreat from a threatened tile,
//! before the target search, so a Settler in a raider's reach still leaves
//! first. It never touches the pipeline, the Settler's production value, or
//! `desired_cities`; the walk clock is the one `settle-sooner` already keeps
//! (`settler_walk_started`), stamped for either gene. Off, every path is
//! byte-identical.
//!
//! ⚠ Native walks are shorter (mean 7.3, p90 16 in `settler_walk_census`),
//! so on the screen this fires on the tail — that is the fires probe under
//! `docs/gene_screens/fires/settler-walk-deadline.json`. The live claim is
//! the table above; the live measurement to read is cities@60 and the
//! settler-fate columns of `tools/civ6_run_report.py` on the next runs with
//! the tag in `~/.civvis-live-force-on`.

use super::settler_never_idles::STRANDED_SITE_MIN_HOLD_TURNS;
use super::{AdvancedAi, VictoryTarget};
use crate::game::{Action, Game};
use crate::think;
use crate::Pos;

/// Standard turns a Settler may be out of a city before the deadline: twelve
/// on Online speed. The live walks that founded averaged 12.7–14.9 turns with
/// the long tail (over 15) taking 63% of all turns walked; the median walk
/// is well under this and the walks that cost the band are all over it.
pub const SETTLER_WALK_DEADLINE_STANDARD: u32 = 24;

/// How far the deadline looks: two tiles, one turn of a Settler's walk on
/// open ground. A site farther than that is the walk the deadline ends.
pub const SETTLER_WALK_DEADLINE_RADIUS: i32 = 2;

/// A site within reach must be worth this much more than the tile the
/// Settler stands on to be worth another turn out of a city.
pub const SETTLER_WALK_DEADLINE_STEP_MARGIN: f64 = 3.0;

impl AdvancedAi {
    /// The deadline in this game's turns.
    pub(super) fn settler_walk_deadline_turns(g: &Game) -> u32 {
        g.standard_duration(SETTLER_WALK_DEADLINE_STANDARD).max(1)
    }

    /// Turns this Settler has been walking, by the clock
    /// `advanced_settler_step` stamps the first turn it steps the unit.
    pub(super) fn settler_turns_out(&self, g: &Game, uid: u32) -> u32 {
        self.settler_walk_started
            .get(&uid)
            .map_or(0, |started| g.turn.saturating_sub(*started))
    }

    /// Every legal site within the deadline radius and what it is worth,
    /// the tile underfoot included and every other tile charged the step
    /// margin. Empty when nothing within reach may be founded.
    pub(super) fn settler_walk_deadline_candidates(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
    ) -> Vec<(Pos, f64)> {
        let here = g.units[&uid].pos;
        let avoid = self.settler_avoid.get(&uid).map(|(pos, _)| *pos);
        let science_targeted = self.active_victory_target(g) == Some(VictoryTarget::Science);
        g.wdisk(here, SETTLER_WALK_DEADLINE_RADIUS)
            .into_iter()
            .filter(|pos| {
                self.base.valid_settle_site(g, pid, *pos)
                    && !g.blocked_city_sites.contains(pos)
                    && !self.settler_site_is_dead(uid, *pos)
                    && Some(*pos) != avoid
                    && !self.settler_capture_scars.contains_key(pos)
                    && !self.settler_target_reserved_by_other(g, pid, uid, *pos)
                    && (*pos == here || g.route_step(uid, *pos, 0).is_some())
                    && self.exhaustion_site_unpriceable(g, *pos).is_none()
                    && !(science_targeted && Self::inside_rival_sphere(g, pid, *pos))
                    && Self::settle_site_forecast_revolt(g, pid, *pos)
                        .filter(|(_, turns)| {
                            science_targeted || *turns < STRANDED_SITE_MIN_HOLD_TURNS
                        })
                        .is_none()
            })
            .map(|pos| {
                let margin = if pos == here {
                    0.0
                } else {
                    SETTLER_WALK_DEADLINE_STEP_MARGIN
                };
                (pos, self.settle_value(g, pid, pos) - margin)
            })
            .collect()
    }

    /// The best of [`Self::settler_walk_deadline_candidates`].
    pub(super) fn settler_walk_deadline_site(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
    ) -> Option<(Pos, f64)> {
        self.settler_walk_deadline_candidates(g, pid, uid)
            .into_iter()
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
    }

    /// The deadline's turn, when it has come: `Some(acted)` when the Settler
    /// founded or stepped toward the site within reach, `None` when the walk
    /// is still inside its deadline or nothing within reach may be founded —
    /// the ordinary step then runs untouched.
    pub(super) fn settler_walk_deadline_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.settler_walk_deadline {
            return None;
        }
        let out = self.settler_turns_out(g, uid);
        let deadline = Self::settler_walk_deadline_turns(g);
        if out < deadline {
            return None;
        }
        let here = g.units[&uid].pos;
        let (site, worth) = self.settler_walk_deadline_site(g, pid, uid)?;
        if site == here {
            return Some(self.found_at_walk_deadline(g, pid, uid, out, deadline, worth));
        }
        think!(self.journal(), Expansion, Detail,
               "Settler past its walk deadline takes {site:?}";
               "{out} turns out of a city against a deadline of {deadline}; the best legal \
                site within {SETTLER_WALK_DEADLINE_RADIUS} tiles is worth {worth:.1} net of \
                the step, and a city there beats the site it has not reached"; site);
        self.settler_targets.insert(uid, site);
        self.settler_relaxed_targets.insert(uid, site);
        self.settler_stalls.remove(&uid);
        self.settler_closest.remove(&uid);
        if self.settler_step_toward_safe(g, pid, uid, site) {
            return Some(true);
        }
        // The safe step refused every neighbour. Standing here is what the
        // corpus measured as the loss; a city here cannot be captured by the
        // raiders holding it, so found underfoot when that is legal too.
        let here_worth = self
            .settler_walk_deadline_candidates(g, pid, uid)
            .into_iter()
            .find(|(pos, _)| *pos == here)
            .map(|(_, worth)| worth);
        match here_worth {
            Some(worth) if g.can_found_city(uid) => {
                Some(self.found_at_walk_deadline(g, pid, uid, out, deadline, worth))
            }
            _ => None,
        }
    }

    /// Found underfoot at the deadline. A Decision line asserts an applied
    /// action, never an intention, so it is journaled after the engine
    /// answers; the site is priced before, for the reason on the ordinary
    /// founding branch.
    fn found_at_walk_deadline(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        out: u32,
        deadline: u32,
        worth: f64,
    ) -> bool {
        let here = g.units[&uid].pos;
        if !g.can_found_city(uid) {
            return false;
        }
        self.settler_targets.remove(&uid);
        self.settler_relaxed_targets.remove(&uid);
        self.settler_stalls.remove(&uid);
        self.settler_blocked_turns.remove(&uid);
        self.settler_closest.remove(&uid);
        let founded = g.apply(pid, &Action::FoundCity { unit: uid }).is_ok();
        if founded {
            think!(self.journal(), Expansion, Decision,
                   "Founding a city at {here:?} at the walk deadline";
                   "{out} turns out of a city against a deadline of {deadline}; the site is \
                    worth {worth:.1}, and every turn a Settler walks is a city it is not"; here);
        } else {
            think!(self.journal(), Expansion, Detail,
                   "Founding refused at {here:?} at the walk deadline";
                   "the engine would not take the city; the settler will re-plan"; here);
        }
        founded
    }
}
