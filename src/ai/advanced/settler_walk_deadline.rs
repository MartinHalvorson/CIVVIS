//! `settler-walk-deadline`: an opening Settler that has been out of a city
//! longer than the walk that wins stops chasing a ranked site and founds
//! the best legal one it can reach at once.
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
//! Counting cities, they stood at 2–5. On the live seat
//! `settler_in_flight_allowed` is answered by `expansion-schedule`, then
//! `land-grab` (two walkers plus one per three cities); `parallel-settlers`
//! never decides and `host-settler-pop` already lowers the floor to the
//! host's two — none of the host-only pipeline genes is the lever left.
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
//! It puts a clock on the walk. A Settler whose walk began by the band turn
//! ([`AdvancedAi::expansion_band_turn`], turn 60 of the ladder's clock) and
//! that has spent [`SETTLER_WALK_DEADLINE_STANDARD`] standard turns out of a
//! city (twelve on Online speed — above the median live walk and under every
//! long one) sets the ordinary site search aside and takes the best legal
//! site within [`SETTLER_WALK_DEADLINE_RADIUS`] tiles of where it stands, the
//! tile underfoot included: founding at once if that is the best, else one
//! safe step and founding on arrival. A site within reach must beat the tile
//! underfoot by [`SETTLER_WALK_DEADLINE_STEP_MARGIN`] to be worth the extra
//! turn, and must be worth at least [`SETTLER_WALK_DEADLINE_VALUE_SHARE`] of
//! the site the Settler was walking to — a city near enough is taken over a
//! better one it has not reached in twelve turns, but not a wasteland over a
//! plan.
//!
//! Turns spent standing on an own city tile or embarked do not count: the
//! clock measures the walk, not a guard wait at home or a crossing to
//! another landmass (the first fires probe of this gene, without those two
//! exclusions and the value floor, packed cities beside the capital and cut
//! overseas colonies short — 0.56 fewer cities a seat, −18 wins per hundred).
//!
//! The choice keeps every legality and Loyalty guard the ordinary founding
//! keeps: the host's blocked plots, this Settler's dead sites, another
//! Settler's reservation, the capture scars, a rival's sphere on a Science
//! lane, the frontier or rate Loyalty verdict, and the engine's own revolt
//! forecast at the twenty-turn floor of the exhaustion search. The step is
//! the ordinary safe step, so it still refuses a tile a visible hostile can
//! take. It runs after the emergency flee and the retreat from a threatened
//! tile, before the target search, so a Settler in a raider's reach still
//! leaves first. It never touches the pipeline, the Settler's production
//! value, or `desired_cities`. Off, every path is byte-identical.
//!
//! ⚠ Native walks are shorter (mean 7.3, p90 16 in `settler_walk_census`),
//! so on the screen this fires on the tail — that is the fires probe under
//! `docs/gene_screens/fires/settler-walk-deadline.json`. The live claim is
//! the table above; the live measurement to read is cities@60 and the
//! settler fates on the next runs with the tag in `~/.civvis-live-force-on`.

use super::settler_never_idles::STRANDED_SITE_MIN_HOLD_TURNS;
use super::{AdvancedAi, VictoryTarget};
use crate::game::{Action, Game};
use crate::think;
use crate::Pos;

/// Standard turns a Settler may spend out of a city before the deadline:
/// twelve on Online speed. The live walks that founded averaged 12.7–14.9
/// turns with the long tail (over 15) taking 63% of all turns walked; the
/// median walk is well under this and the walks that cost the band are all
/// over it.
pub const SETTLER_WALK_DEADLINE_STANDARD: u32 = 24;

/// How far the deadline looks: two tiles, one turn of a Settler's walk on
/// open ground. A site farther than that is the walk the deadline ends.
pub const SETTLER_WALK_DEADLINE_RADIUS: i32 = 2;

/// A site within reach must be worth this much more than the tile the
/// Settler stands on to be worth another turn out of a city.
pub const SETTLER_WALK_DEADLINE_STEP_MARGIN: f64 = 3.0;

/// The least a site within reach may be worth, as a share of the site the
/// Settler was walking to. Half: the deadline trades a better city for a
/// nearer one, never a plan for a wasteland.
pub const SETTLER_WALK_DEADLINE_VALUE_SHARE: f64 = 0.5;

impl AdvancedAi {
    /// The deadline in this game's turns.
    pub(super) fn settler_walk_deadline_turns(g: &Game) -> u32 {
        g.standard_duration(SETTLER_WALK_DEADLINE_STANDARD).max(1)
    }

    /// Advance this Settler's walk clock, once per turn, and return
    /// `(turn the walk began, turns out of a city)`. A turn on an own city
    /// tile or embarked is not a turn out. Entries follow the unit
    /// (`remap_unit_memory`) and die with it.
    pub(super) fn note_settler_walk(&mut self, g: &Game, pid: usize, uid: u32) -> (u32, u32) {
        let unit = &g.units[&uid];
        let at_home = g
            .city_at(unit.pos)
            .is_some_and(|cid| g.cities[&cid].owner == pid);
        let out = !at_home && !g.is_embarked(unit);
        let turn = g.turn;
        self.settler_walk_clock
            .retain(|other, _| g.units.contains_key(other));
        let entry = self
            .settler_walk_clock
            .entry(uid)
            .or_insert((turn, turn, 0));
        if entry.1 != turn {
            entry.1 = turn;
            if out {
                entry.2 += 1;
            }
        }
        (entry.0, entry.2)
    }

    /// Turns this Settler has spent out of a city, as last noted.
    #[cfg(test)]
    pub(super) fn settler_turns_out(&self, uid: u32) -> u32 {
        self.settler_walk_clock
            .get(&uid)
            .map_or(0, |(_, _, out)| *out)
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
        let loyalty_rate = self.base.loyalty_rate_alarm || science_targeted;
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
                    && if loyalty_rate {
                        self.settle_site_loyalty_verdict(g, pid, *pos).is_none()
                    } else {
                        self.settle_site_frontier_loyalty_verdict(g, pid, *pos)
                            .is_none()
                    }
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

    /// The best of [`Self::settler_walk_deadline_candidates`] that clears
    /// the value floor set by the site the Settler was walking to, skipping
    /// the tile underfoot when the engine will not take a city there.
    pub(super) fn settler_walk_deadline_site(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
    ) -> Option<(Pos, f64)> {
        let here = g.units[&uid].pos;
        let floor = self
            .settler_targets
            .get(&uid)
            .map(|target| self.settle_value(g, pid, *target) * SETTLER_WALK_DEADLINE_VALUE_SHARE);
        self.settler_walk_deadline_candidates(g, pid, uid)
            .into_iter()
            .filter(|(pos, worth)| {
                (*pos != here || g.can_found_city(uid)) && floor.is_none_or(|floor| *worth >= floor)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
    }

    /// The deadline's turn, when it has come: `Some(acted)` when the Settler
    /// founded or stepped toward the site within reach, `None` when the walk
    /// began after the band turn, is still inside its deadline, or nothing
    /// within reach may be founded — the ordinary step then runs untouched.
    pub(super) fn settler_walk_deadline_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.settler_walk_deadline {
            return None;
        }
        let (began, out) = self.note_settler_walk(g, pid, uid);
        let deadline = Self::settler_walk_deadline_turns(g);
        if began > Self::expansion_band_turn(g) || out < deadline {
            return None;
        }
        let here = g.units[&uid].pos;
        if g.is_embarked(&g.units[&uid]) {
            return None;
        }
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
