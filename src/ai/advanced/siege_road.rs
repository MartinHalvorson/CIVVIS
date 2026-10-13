//! `siege-target-needs-a-road`: a siege the army cannot walk to is stood
//! down at once, and its owner leaves the campaign while the road stays shut.
//!
//! `stage-march-keeps-to-land` holds a land unit on land when no dry road
//! within `siege_train::STAGE_DRY_LIMIT` reaches the staging ring, and logs
//! it as "holds on land short of the water". Over the 10-04/05 control runs
//! that line mostly meant a closed border, not the sea: in the windows it
//! dominates, 324 unit-turns stood behind a road open only through closed
//! borders against 1 cut off by water. 19 siege windows whose force stood
//! mostly border-blocked (306 siege-turns) converted 0 times, and in 18 the
//! target was dropped or still stuck 15 turns later; the "nobody went"
//! stand-down waited for the breaker exemption to lapse first. Live King
//! civvis-20261005T200350Z (game 158): the Siege of Apu mustered 7 to 18
//! units at 0% ready from turn 129 to 160; its land road ran through Poland,
//! at peace with closed borders, once Poland took Wolin from our
//! suzerainty, and 30 of those turns read "Holding the capture of Apu for a
//! wall-breaker on its way". Beijing repeated it from 161 to 171.
//!
//! Under the gene the Stage step records each march's outcome; when half or
//! more of a turn's train holds for want of a road [`ROAD_HOLD_TURNS`] turns
//! running, the capture is stood down through `capture_stood_down` whatever
//! the breaker and muster waits say, and the hold line names the border
//! that shuts the road.

use super::siege_train::{StageMarch, STAGE_DRY_LIMIT, STAGING_FAR};
use super::AdvancedAi;
use crate::game::Game;
use crate::think;
use crate::Pos;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Consecutive turns half the train must hold for want of a road before the
/// capture is stood down. Three is `commitments::STALL_TURNS` and
/// `COMMITMENT_PATIENCE`, the controller's own "not getting there".
pub(super) const ROAD_HOLD_TURNS: u32 = 3;

/// `long-road-names-the-blocker`: a dry road at least this many times the
/// road through closed borders is the long way round them.
const LONG_ROAD_FACTOR: usize = 2;

/// `long-road-names-the-blocker`: and at least this many steps longer, so a
/// short road with a small detour still marches.
const LONG_ROAD_EXTRA: usize = 10;

/// `closed-border-blocker-declares`: our military over a blocker's steady
/// reading (and its suzerain's, for a city-state) at or over which its
/// closed borders are declared on: the blocker at half our power or under.
pub(super) const BLOCKER_POWER_RATIO: f64 = 2.0;

/// One siege's march outcomes for the turn, and its run of road-bound turns.
#[derive(Clone, Debug, Default)]
pub(super) struct RoadTally {
    /// The turn `held` and `marched` describe.
    turn: u32,
    /// Units the Stage step held on land this turn, and the owner whose
    /// closed borders shut the road, when one does.
    held: BTreeMap<u32, Option<usize>>,
    /// Units the Stage step marched (or walked a dry road) this turn.
    marched: BTreeSet<u32>,
    /// The last turn the ledger read this tally.
    counted: u32,
    /// Consecutive read turns half the train or more was held.
    pub(super) streak: u32,
}

#[cfg(test)]
impl RoadTally {
    /// Whether this turn's Stage step held `uid` (else marched it, or never
    /// read it), for tests outside this module.
    pub(super) fn held(&self, uid: u32) -> Option<bool> {
        if self.held.contains_key(&uid) {
            Some(true)
        } else {
            self.marched.contains(&uid).then_some(false)
        }
    }
}

/// A capture stood down for want of a road.
#[derive(Clone, Copy, Debug)]
pub(super) struct RoadClosed {
    /// The seat whose road it is.
    seat: usize,
    /// The city's owner when the stand-down fired.
    owner: usize,
    /// The owner whose closed borders shut the road, when one does; `None`
    /// for water or impassable ground.
    blocker: Option<usize>,
}

impl AdvancedAi {
    /// The Stage step's march outcome for `uid` toward `city`. Records the
    /// outcome and, for a hold, returns the name of the civilization whose
    /// closed borders shut the land road, when one does. Off, records
    /// nothing and returns `None`.
    pub(super) fn note_stage_march(
        &mut self,
        g: &Game,
        uid: u32,
        city: u32,
        march: &StageMarch,
    ) -> Option<String> {
        if !self.siege_target_needs_a_road {
            return None;
        }
        let tally = self.siege_road_tally.entry(city).or_default();
        if tally.turn != g.turn {
            tally.turn = g.turn;
            tally.held.clear();
            tally.marched.clear();
        }
        match march {
            StageMarch::Hold { .. } => {
                let blocker = g
                    .cities
                    .get(&city)
                    .and_then(|target| border_blocker(g, uid, target.pos));
                let tally = self.siege_road_tally.entry(city).or_default();
                tally.marched.remove(&uid);
                tally.held.insert(uid, blocker);
                if let Some(owner) = blocker {
                    self.note_closed_border_hold(g, g.units[&uid].owner, owner);
                }
                blocker.map(|owner| g.players[owner].civ.clone())
            }
            _ => {
                tally.held.remove(&uid);
                tally.marched.insert(uid);
                None
            }
        }
    }

    /// `long-road-names-the-blocker`: `march` as the Stage march reads it,
    /// except that a dry road at least [`LONG_ROAD_FACTOR`] times, and
    /// [`LONG_ROAD_EXTRA`] steps over, the road through a peaceful major's
    /// closed borders becomes a hold short of them, so `note_stage_march`
    /// names that major: the passage purchase asks it, and the road
    /// stand-down retargets when the hold persists.
    ///
    /// Live Emperor civvis-20261007T123621Z (game 351): with Japan's borders
    /// closed between our east shore and Mataram, the dry road ran 40-43
    /// steps north round the lake against 7-9 through Japan. From turn 78
    /// to 90 the train's Trebuchets, Men-at-Arms and Knights stood nine to
    /// thirteen tiles out, swapping between two shore tiles each frame or
    /// stepping onto the long road and back, while the siege read "1 of 7-9
    /// staged" and the target moved on at turn 90. No march ever held, so
    /// nothing named Japan, and the first passage ask came at turn 113.
    pub(super) fn long_road_holds(
        &mut self,
        g: &Game,
        uid: u32,
        target: Pos,
        march: StageMarch,
    ) -> StageMarch {
        let StageMarch::Dry { dry, wet, .. } = march else {
            return march;
        };
        if !self.long_road_names_the_blocker {
            return march;
        }
        match border_free_road(g, uid, target) {
            Some((free, Some(owner)))
                if dry >= LONG_ROAD_FACTOR * free && dry >= free + LONG_ROAD_EXTRA =>
            {
                think!(self.journal(), Military, Detail,
                    "Siege march holds for {}'s closed borders", g.players[owner].civ;
                    "the dry road round them runs {dry} steps against {free} through them; the march would cross water in {wet}";
                    target);
                StageMarch::Hold { wet }
            }
            _ => march,
        }
    }

    /// `siege-buys-the-passage`: the majors at peace with us whose closed
    /// borders held a Stage march this turn or last — the passage the
    /// bridge's open-borders purchase asks for first. In 35 of 120 live
    /// Emperor runs of 10-06/07 a siege march held short of a third major's
    /// closed borders, 102 city-holds with the water crossing round them a
    /// median 19 steps (a quarter of them 27 or more), and 1 of those
    /// cities fell. civvis-20261007T063655Z (game 321) stood 17-18 units
    /// short of Persia's borders for 44 turns while Japan, behind them, held
    /// three cities on two military units. The bridge's purchase closed 42
    /// of 47 asks on 10-07, but asked only for exploration. Empty with the
    /// gene off, or with `siege-target-needs-a-road` off (which records the
    /// blocker).
    pub fn siege_passage_blockers(&self, g: &Game, pid: usize) -> BTreeSet<usize> {
        if !self.siege_buys_the_passage {
            return BTreeSet::new();
        }
        // The siege stood down for want of the road keeps asking: the grant
        // lifts the stand-down (`siege_road_reopened` reads open borders).
        // Live Emperor civvis-20261007T074538Z stood its siege down at turn
        // 179, two turns into the hold, before any ask could fire.
        let closed = self.siege_road_closed.iter().filter_map(|(city, closed)| {
            (closed.seat == pid && self.capture_stood_down_holds(g, *city))
                .then_some(closed.blocker)
                .flatten()
        });
        self.siege_road_tally
            .values()
            .filter(|tally| tally.turn + 1 >= g.turn)
            .flat_map(|tally| tally.held.values().flatten().copied())
            .chain(closed)
            .filter(|owner| {
                *owner != pid
                    && g.players.get(*owner).is_some_and(|player| {
                        player.alive && !player.is_minor && !player.is_barbarian
                    })
                    && !g.is_at_war(pid, *owner)
            })
            .collect()
    }

    /// `siege-target-needs-a-road`: the end-of-turn reading. Called by
    /// `reconcile_commitments` after the capture ledger is read; stands the
    /// open capture down when its train has been held for want of a road
    /// [`ROAD_HOLD_TURNS`] turns running.
    pub(super) fn reconcile_siege_roads(&mut self, g: &mut Game, pid: usize) {
        if !self.siege_target_needs_a_road {
            return;
        }
        use super::commitments::{Kind, Owner, Target, CAPTURE_STAND_DOWN_TURNS};
        let Some(Target::City(cid)) = self
            .commitments
            .open_for(Kind::Capture, Owner::Empire)
            .map(|c| c.target)
        else {
            return;
        };
        let Some(city) = g.cities.get(&cid) else {
            return;
        };
        let (name, pos, owner) = (city.name.clone(), city.pos, city.owner);
        // The train within the staging ring never reaches the Stage march,
        // so it is counted here: a road-bound share is of the whole train.
        let near = g
            .units
            .values()
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                unit.owner == pid
                    && spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && g.wdist(unit.pos, pos) <= STAGING_FAR
            })
            .count();
        let tally = self.siege_road_tally.entry(cid).or_default();
        if tally.counted == g.turn {
            return;
        }
        tally.counted = g.turn;
        let fresh = tally.turn == g.turn;
        let held = if fresh { tally.held.len() } else { 0 };
        let marched = if fresh { tally.marched.len() } else { 0 };
        let train = held + marched + near;
        if held > 0 && 2 * held >= train {
            tally.streak += 1;
        } else {
            tally.streak = 0;
        }
        if tally.streak < ROAD_HOLD_TURNS || self.capture_stood_down.contains_key(&cid) {
            return;
        }
        let streak = tally.streak;
        let mut blockers: BTreeMap<usize, usize> = BTreeMap::new();
        for owner in tally.held.values().flatten() {
            *blockers.entry(*owner).or_insert(0) += 1;
        }
        let blocker = blockers
            .into_iter()
            .max_by_key(|(owner, count)| (*count, std::cmp::Reverse(*owner)))
            .map(|(owner, _)| owner);
        self.siege_road_tally.remove(&cid);
        self.capture_stood_down
            .insert(cid, g.turn + CAPTURE_STAND_DOWN_TURNS);
        self.siege_road_closed.insert(
            cid,
            RoadClosed {
                seat: pid,
                owner,
                blocker,
            },
        );
        *g.players[pid]
            .counters
            .entry("commit:capture:road_stand_downs".to_string())
            .or_insert(0) += 1;
        let shut = blocker.map_or_else(
            || "water or impassable ground".to_string(),
            |owner| format!("{}'s closed borders", g.players[owner].civ),
        );
        think!(self.journal(), Military, Strategy, "Standing down a siege the army cannot reach";
               "{name}: {held} of the {train} units of its train held short of {shut} for {streak} turns; no land road opens within {} steps",
               STAGE_DRY_LIMIT; pos);
    }

    /// `siege-target-needs-a-road`: whether a road stand-down of `city` has
    /// lifted early because the border that shut it opened (a war on its
    /// owner, open borders, or the owner gone).
    pub(super) fn siege_road_reopened(&self, g: &Game, city: u32) -> bool {
        self.siege_target_needs_a_road
            && self.siege_road_closed.get(&city).is_some_and(|closed| {
                closed.blocker.is_some_and(|blocker| {
                    g.is_at_war(closed.seat, blocker)
                        || g.has_open_borders(closed.seat, blocker)
                        || !g.players.get(blocker).is_some_and(|player| player.alive)
                })
            })
    }

    /// `siege-target-needs-a-road`: whether `rival` is out of the campaign
    /// ranking because a siege of one of its cities was stood down for want
    /// of a road and still no land road from our cities reaches any of its
    /// cities.
    pub(super) fn siege_road_cuts_off(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.siege_target_needs_a_road
            && self.siege_road_closed.iter().any(|(city, closed)| {
                closed.owner == rival
                    && closed.seat == pid
                    && self.capture_stood_down_holds(g, *city)
            })
            && !self.rival_reachable_by_land(g, pid, rival)
    }

    /// `sea-locked-front-moves-on`: how many of `rival`'s cities have a road
    /// stand-down still holding that named no closed border, the train held
    /// short of water or impassable ground. Zero with
    /// `siege-target-needs-a-road` off (nothing is recorded).
    pub(super) fn water_road_stand_downs(&self, g: &Game, pid: usize, rival: usize) -> usize {
        if !self.siege_target_needs_a_road {
            return 0;
        }
        self.siege_road_closed
            .iter()
            .filter(|(city, closed)| {
                closed.seat == pid
                    && closed.owner == rival
                    && closed.blocker.is_none()
                    && self.capture_stood_down_holds(g, **city)
            })
            .count()
    }

    /// `blocker-becomes-the-target`: whether the second front `rival`, still
    /// at peace, is displaced from the plan by a war the plan keeps: the
    /// plan's target is another rival already at war with us, so neither the
    /// plan nor `second_front_waits_*`'s waiting declaration would ever open
    /// the second front. The declaration desk then declares on it as on a
    /// waiting one. The elimination front (`diplomatic_contender_to_eliminate`)
    /// ranks ahead of the second front in `assess`, and a second front that
    /// does not wait (an urgent clock, the road blocker) was left with no
    /// declaration at all. Live King civvis-20261005T215912Z (game 167) read
    /// "Eliminating Maori" from turn 200 while the Ottomans, the second front
    /// from 201 (an urgent science clock, then the road blocker of five
    /// stood-down Maori sieges), stood at peace at 516-1,234 power against our
    /// 2,870-4,711 until they won on Science at 249; the gene's road blocker
    /// was named every turn and never declared on. Off with the gene.
    pub(crate) fn second_front_displaced(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        plan: &super::StrategicPlan,
    ) -> bool {
        // `capital-taken-moves-on` opens the next capital's owner the same way;
        // `closed-border-blocker-declares` its own weak blocker.
        (self.blocker_becomes_the_target
            || self.capital_taken_moves_on
            || (self.closed_border_blocker_declares
                && self.closed_border_front(g, pid) == Some(rival)))
            && !g.is_at_war(pid, rival)
            && plan
                .target_player
                .is_some_and(|target| target != rival && g.is_at_war(pid, target))
    }

    /// `blocker-becomes-the-target`: the stood-down siege's blocker
    /// (`stood_down_blocker_front`), else, under
    /// `closed-border-blocker-declares`, the weak blocker of a held one
    /// (`closed_border_front`). `None` with both genes off.
    pub(crate) fn road_blocker_front(&self, g: &Game, pid: usize) -> Option<usize> {
        self.stood_down_blocker_front(g, pid)
            .or_else(|| self.closed_border_front(g, pid))
    }

    /// `blocker-becomes-the-target`: the major whose closed borders shut the
    /// road of a siege stood down for want of one, while that stand-down
    /// holds: alive, met, at peace with us and a legal campaign target,
    /// holding an original capital we need (a Domination objective in its own
    /// right), and passing the version-2 declaration edge
    /// (`declaration_edge_2`) whether or not that gene is armed. A war on it
    /// opens the road (`siege_road_reopened`) and is Domination progress.
    /// `None` with the gene off, under another victory target, or when no
    /// such blocker stands; the lowest player id of several.
    pub(super) fn stood_down_blocker_front(&self, g: &Game, pid: usize) -> Option<usize> {
        if !self.blocker_becomes_the_target
            || self.active_victory_target(g) != Some(super::VictoryTarget::Domination)
        {
            return None;
        }
        self.siege_road_closed
            .iter()
            .filter(|(city, closed)| closed.seat == pid && self.capture_stood_down_holds(g, **city))
            .filter_map(|(_, closed)| closed.blocker)
            .filter(|blocker| {
                *blocker != pid
                    && g.players.get(*blocker).is_some_and(|player| {
                        player.alive && !player.is_minor && !player.is_barbarian
                    })
                    && !g.is_at_war(pid, *blocker)
                    && self.campaign_target_legal(g, pid, *blocker)
                    && g.cities.values().any(|city| {
                        city.owner == *blocker
                            && city.is_capital
                            && city.original_owner != pid
                            && !g.same_team(pid, city.original_owner)
                    })
                    && self.declaration_edge_2(g, pid, *blocker).passes()
            })
            .min()
    }

    /// `closed-border-blocker-declares`: the third party whose closed
    /// borders have held a Stage march of ours [`ROAD_HOLD_TURNS`] turns
    /// running, through last turn, or shut the road of a siege stood down
    /// for want of one while that stand-down holds, when it is weak
    /// (`closed_border_blocker_is_weak`). `blocker-becomes-the-target` asked
    /// for the stand-down, an original capital and the version-2 edge; a
    /// train held short of closed borders mostly holds instead -- the
    /// breaker and muster waits keep the capture open -- and over 138 live
    /// Immortal runs of 10-11/13 the gene never fired while 19 games held a
    /// siege behind 27 blockers and 58 of 59 open-borders asks during the
    /// holds were refused. civvis-20261013T083300Z held its Susa train,
    /// bombard to tank, short of Mongolia's borders from turn 175 at about
    /// five times Mongolia's power, stood the Sparda siege down for them at
    /// 190, and lost to Persia's Culture at 193. The declaration desk opens
    /// the war as `road_opening` with every other safety kept. `None` with
    /// the gene off, under another victory target, or when no such blocker
    /// stands; the lowest player id of several.
    pub(crate) fn closed_border_front(&self, g: &Game, pid: usize) -> Option<usize> {
        if !self.closed_border_blocker_declares
            || self.active_victory_target(g) != Some(super::VictoryTarget::Domination)
        {
            return None;
        }
        let held = self
            .closed_border_holds
            .iter()
            .filter(|((seat, _), (first, last))| {
                *seat == pid && last + 1 >= g.turn && last + 1 - first >= ROAD_HOLD_TURNS
            })
            .map(|((_, blocker), _)| *blocker);
        let stood_down = self
            .siege_road_closed
            .iter()
            .filter(|(city, closed)| closed.seat == pid && self.capture_stood_down_holds(g, **city))
            .filter_map(|(_, closed)| closed.blocker);
        held.chain(stood_down)
            .filter(|blocker| self.closed_border_blocker_is_weak(g, pid, *blocker))
            .min()
    }

    /// `closed-border-blocker-declares`: whether `blocker` may be declared on
    /// for its closed borders: alive, at peace with us and a legal campaign
    /// target (met, no friendship or alliance, a city-state not ours or a
    /// friend's), with our military [`BLOCKER_POWER_RATIO`] times its steady
    /// reading and [`super::one_war::CRUSHING_PEAK_FLOOR`] times its 30-turn
    /// peak. A city-state's suzerain, when it has one at peace with us, must
    /// read as weak too: its war comes with the city-state's.
    fn closed_border_blocker_is_weak(&self, g: &Game, pid: usize, blocker: usize) -> bool {
        let Some(player) = g.players.get(blocker) else {
            return false;
        };
        if blocker == pid
            || !player.alive
            || player.is_barbarian
            || g.is_at_war(pid, blocker)
            || !self.campaign_target_legal(g, pid, blocker)
        {
            return false;
        }
        let ours = g.military_power(pid);
        let weak = |rival: usize| {
            ours >= BLOCKER_POWER_RATIO * self.steady_rival_power(g, rival).max(1.0)
                && ours
                    >= super::one_war::CRUSHING_PEAK_FLOOR
                        * self.peak_rival_power(g, rival).max(1.0)
        };
        weak(blocker)
            && (!player.is_minor
                || g.suzerain_of(blocker)
                    .is_none_or(|suzerain| g.is_at_war(pid, suzerain) || weak(suzerain)))
    }

    /// `closed-border-blocker-declares`: a Stage march of `seat`'s held short
    /// of `blocker`'s closed borders this turn. Extends the running hold, or
    /// starts one after a turn without; says once, on the turn the run
    /// reaches [`ROAD_HOLD_TURNS`], whether the blocker reads weak enough to
    /// become the front.
    fn note_closed_border_hold(&mut self, g: &Game, seat: usize, blocker: usize) {
        if !self.closed_border_blocker_declares {
            return;
        }
        let run = self
            .closed_border_holds
            .entry((seat, blocker))
            .or_insert((g.turn, g.turn));
        if g.turn > run.1 + 1 || g.turn < run.0 {
            *run = (g.turn, g.turn);
        }
        let reached = run.1 + 1 - run.0 < ROAD_HOLD_TURNS && g.turn + 1 - run.0 >= ROAD_HOLD_TURNS;
        run.1 = run.1.max(g.turn);
        if !reached {
            return;
        }
        let ours = g.military_power(seat);
        let (steady, peak) = (
            self.steady_rival_power(g, blocker),
            self.peak_rival_power(g, blocker),
        );
        let verdict = if self.closed_border_blocker_is_weak(g, seat, blocker) {
            "weak enough: the war that opens the road is the next front"
        } else {
            "not weak enough, or not a legal target: the train keeps holding"
        };
        think!(self.journal(), Military, Strategy,
               "{}'s closed borders have held our siege march {} turns", g.players[blocker].civ, ROAD_HOLD_TURNS;
               "closed-border-blocker-declares: our {ours:.0} power against their steady {steady:.0} and 30-turn peak {peak:.0} (needs {BLOCKER_POWER_RATIO:.1} and {:.1} times); {verdict}",
               super::one_war::CRUSHING_PEAK_FLOOR);
    }
}

/// The owner whose closed borders shut `uid`'s land road to the staging ring
/// of a city at `target`: the first closed territory on the shortest road
/// over dry, passable ground with every border open. `None` when no such
/// road exists within [`STAGE_DRY_LIMIT`] (water or impassable ground), or
/// when the road needs no closed border.
fn border_blocker(g: &Game, uid: u32, target: Pos) -> Option<usize> {
    border_free_road(g, uid, target).and_then(|(_, blocker)| blocker)
}

/// The length of the shortest dry road from `uid` to the staging ring of
/// `target` with every border open, and the first major at peace whose
/// closed borders it crosses (`None` when it crosses none). `None` when no
/// dry road reaches the ring within [`STAGE_DRY_LIMIT`] steps.
fn border_free_road(g: &Game, uid: u32, target: Pos) -> Option<(usize, Option<usize>)> {
    let unit = g.units.get(&uid)?;
    let seat = unit.owner;
    let open = |owner: usize| {
        owner == seat || g.is_at_war(seat, owner) || g.has_open_borders(seat, owner)
    };
    let mut seen = BTreeSet::from([unit.pos]);
    let mut queue = VecDeque::from([(unit.pos, None::<usize>, 0usize)]);
    while let Some((current, blocker, steps)) = queue.pop_front() {
        if steps >= STAGE_DRY_LIMIT {
            continue;
        }
        for next in g.nbrs(current) {
            if seen.contains(&next) {
                continue;
            }
            let Some(tile) = g.map.get(next) else {
                continue;
            };
            if g.rules.is_water(tile) || !g.rules.is_passable(tile) {
                continue;
            }
            seen.insert(next);
            let closed = tile
                .owner_city
                .and_then(|cid| g.cities.get(&cid))
                .map(|city| city.owner)
                .filter(|owner| !open(*owner));
            let blocker = blocker.or(closed);
            if g.wdist(next, target) <= STAGING_FAR {
                return Some((steps + 1, blocker));
            }
            queue.push_back((next, blocker, steps + 1));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::commitments::{Kind, Owner, Target};
    use super::super::siege_train::StageMarch;
    use super::super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
    use super::ROAD_HOLD_TURNS;
    use crate::game::Game;

    /// A land strip: us at the west end, a civilization with closed borders
    /// across the middle, the target at the east end (at war with us). The
    /// strip is closed at both ends so the wrap cannot go round the middle.
    fn strip() -> (Game, u32, u32) {
        let mut g = Game::new_full(3, 40, 12, 931_036, 300, 0, false);
        for unit in g.units.keys().copied().collect::<Vec<_>>() {
            g.remove_unit(unit);
        }
        for tile in g.map.tiles.values_mut() {
            tile.terrain = if (4..=8).contains(&tile.pos.1) && (2..=36).contains(&tile.pos.0) {
                crate::name!("grassland")
            } else {
                crate::name!("ocean")
            };
            tile.feature = None;
            tile.hills = false;
        }
        g.found_city_for(0, (4, 6), None);
        let screen = g.found_city_for(1, (18, 6), None);
        let target = g.found_city_for(2, (32, 6), None);
        for tile in g.map.tiles.values_mut() {
            if (14..=22).contains(&tile.pos.0) && (4..=8).contains(&tile.pos.1) {
                tile.owner_city = Some(screen);
            }
        }
        g.players[1].borders_enforced = Some(true);
        g.at_war.insert((0, 2));
        g.at_war.insert((2, 0));
        let soldier = g.spawn_test_unit("warrior", 0, (8, 6));
        (g, target, soldier)
    }

    /// `strip` with a land detour north of it: up column 2 from the west
    /// end to row 0, along row 0, and down column 36 into the east end, so a
    /// dry road round the screen exists, more than twice the one through it.
    fn strip_with_a_detour() -> (Game, u32, u32) {
        let (mut g, target, soldier) = strip();
        for tile in g.map.tiles.values_mut() {
            let (x, y) = tile.pos;
            if (y <= 3 && (x == 2 || x == 36)) || (y == 0 && (2..=36).contains(&x)) {
                tile.terrain = crate::name!("grassland");
            }
        }
        (g, target, soldier)
    }

    fn ai(gene: bool, g: &Game, target: u32) -> AdvancedAi {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.enable_capture_go_or_stand_down_2();
        if gene {
            ai.enable_siege_target_needs_a_road();
        }
        ai.plan = Some(StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(2),
            target_city: Some(target),
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        });
        ai
    }

    /// Run `turns` turns in which the Stage step reads `march` for the
    /// soldier, reconciling the ledger after each.
    fn run(ai: &mut AdvancedAi, g: &mut Game, target: u32, soldier: u32, march: StageMarch, turns: u32) {
        for _ in 0..turns {
            let _ = ai.note_stage_march(g, soldier, target, &march);
            ai.reconcile_commitments(g, 0);
            g.turn += 1;
        }
    }

    /// `long-road-names-the-blocker`: the dry road round a peaceful major's
    /// closed borders, over twice the road through them, holds short of
    /// them and names the major as the passage to buy; the gene off, or at
    /// war with the screen's owner, the march keeps its dry road.
    #[test]
    fn a_long_road_round_closed_borders_holds_and_names_the_blocker_under_the_gene() {
        use super::super::siege_train::{STAGE_DRY_LIMIT, STAGING_FAR};
        let (g, target, soldier) = strip_with_a_detour();
        let pos = g.cities[&target].pos;
        let (step, dry) = g
            .route_step_dry(soldier, pos, STAGING_FAR, STAGE_DRY_LIMIT)
            .expect("the way round");
        let free = super::border_free_road(&g, soldier, pos).expect("a road through");
        assert_eq!(free.1, Some(1), "through the screen");
        assert!(dry >= 2 * free.0 && dry >= free.0 + 10, "fixture: {dry} against {free:?}");
        let march = StageMarch::Dry { step, dry, wet: 12 };
        let mut off = ai(true, &g, target);
        off.enable_siege_buys_the_passage();
        assert!(matches!(off.long_road_holds(&g, soldier, pos, march), StageMarch::Dry { .. }), "off");
        let mut on = ai(true, &g, target);
        on.enable_siege_buys_the_passage();
        on.enable_long_road_names_the_blocker();
        let held = on.long_road_holds(&g, soldier, pos, march);
        assert!(matches!(held, StageMarch::Hold { wet: 12 }), "{held:?}");
        let _ = on.note_stage_march(&g, soldier, target, &held);
        assert_eq!(on.siege_passage_blockers(&g, 0), [1].into(), "the screen's owner");
        let mut war = g.clone();
        war.at_war.insert((0, 1));
        war.at_war.insert((1, 0));
        assert!(
            matches!(on.long_road_holds(&war, soldier, pos, march), StageMarch::Dry { .. }),
            "at war the road through is open"
        );
    }

    /// `siege-buys-the-passage`: a march held short of a major's closed
    /// borders names that major as the passage to buy, while we are at peace
    /// with it and for a turn after the hold; the gene off, nobody.
    #[test]
    fn a_held_march_names_the_passage_to_buy_under_the_gene() {
        let (g, target, soldier) = strip();
        let hold = StageMarch::Hold { wet: 30 };
        let mut off = ai(true, &g, target);
        let _ = off.note_stage_march(&g, soldier, target, &hold);
        assert!(off.siege_passage_blockers(&g, 0).is_empty(), "off");
        let mut on = ai(true, &g, target);
        on.enable_siege_buys_the_passage();
        let _ = on.note_stage_march(&g, soldier, target, &hold);
        assert_eq!(on.siege_passage_blockers(&g, 0), [1].into(), "the screen's owner");
        let mut later = g.clone();
        later.turn += 1;
        assert_eq!(on.siege_passage_blockers(&later, 0), [1].into(), "a turn on");
        later.turn += 1;
        assert!(on.siege_passage_blockers(&later, 0).is_empty(), "stale");
        let mut war = g.clone();
        war.at_war.insert((0, 1));
        war.at_war.insert((1, 0));
        assert!(on.siege_passage_blockers(&war, 0).is_empty(), "no purchase at war");
    }

    /// `siege-buys-the-passage`: a siege stood down for want of the road keeps
    /// naming the passage to buy, since the grant would lift the stand-down.
    #[test]
    fn a_stood_down_siege_still_names_the_passage_under_the_gene() {
        let (game, target, soldier) = strip();
        let hold = StageMarch::Hold { wet: 30 };
        let mut on = ai(true, &game, target);
        on.enable_siege_buys_the_passage();
        let mut g = game.clone();
        run(&mut on, &mut g, target, soldier, hold, ROAD_HOLD_TURNS);
        assert!(on.capture_stood_down_holds(&g, target), "fixture: stood down");
        g.turn += 5;
        assert_eq!(
            on.siege_passage_blockers(&g, 0),
            [1].into(),
            "five turns after the last hold, the stood-down siege still asks"
        );
    }

    /// A target whose only land road runs through closed borders is stood
    /// down within `ROAD_HOLD_TURNS` under the gene, names the border, and
    /// drops its owner from the campaign; the gene off, the same holds
    /// stand nothing down.
    #[test]
    fn a_border_blocked_siege_is_stood_down_within_three_turns() {
        let (game, target, soldier) = strip();
        assert!(
            game.route_step_dry(soldier, game.cities[&target].pos, 5, 128).is_none(),
            "fixture: no open land road"
        );
        let hold = StageMarch::Hold { wet: 30 };

        let mut on = ai(true, &game, target);
        let mut g = game.clone();
        assert_eq!(
            on.note_stage_march(&g, soldier, target, &hold).as_deref(),
            Some(g.players[1].civ.as_str()),
            "the hold names the closed border"
        );
        assert!(on.campaign_target_legal(&g, 0, 2), "at war: a legal campaign target");
        run(&mut on, &mut g, target, soldier, hold, ROAD_HOLD_TURNS - 1);
        assert!(!on.capture_stood_down.contains_key(&target), "two holds are not yet the rule");
        run(&mut on, &mut g, target, soldier, hold, 1);
        assert!(on.capture_stood_down.contains_key(&target), "gene on: stood down");
        assert!(on.capture_stood_down_holds(&g, target));
        assert!(on.siege_road_cuts_off(&g, 0, 2), "the campaign drops the rival");
        assert!(!on.campaign_target_legal(&g, 0, 2));
        assert_eq!(
            g.players[0].counters.get("commit:capture:road_stand_downs"),
            Some(&1)
        );
        // A war on the screen opens the road: the stand-down lifts.
        g.at_war.insert((0, 1));
        g.at_war.insert((1, 0));
        assert!(on.siege_road_reopened(&g, target));
        assert!(!on.capture_stood_down_holds(&g, target));
        assert!(!on.siege_road_cuts_off(&g, 0, 2));

        let mut off = ai(false, &game, target);
        let mut g = game.clone();
        assert_eq!(off.note_stage_march(&g, soldier, target, &hold), None);
        run(&mut off, &mut g, target, soldier, hold, ROAD_HOLD_TURNS);
        assert!(
            !off.capture_stood_down.contains_key(&target),
            "gene off: three holds stand nothing down"
        );
        assert!(off.siege_road_closed.is_empty());
    }

    /// A reachable target is unaffected: a train that marches, or one held
    /// only a minority of the time, keeps its capture.
    #[test]
    fn a_reachable_siege_is_not_stood_down() {
        let (mut game, target, soldier) = strip();
        game.players[1].borders_enforced = Some(false);
        assert!(game.route_step_dry(soldier, game.cities[&target].pos, 5, 128).is_some());
        let mut on = ai(true, &game, target);
        let mut g = game.clone();
        run(&mut on, &mut g, target, soldier, StageMarch::Ordinary, ROAD_HOLD_TURNS + 2);
        assert!(!on.capture_stood_down.contains_key(&target));
        assert_eq!(
            on.commitments()
                .open_for(Kind::Capture, Owner::Empire)
                .map(|c| c.target),
            Some(Target::City(target))
        );

        // Held, but by a minority of the train: two marchers stand beside it.
        let (game, target, soldier) = strip();
        let mut g = game.clone();
        let mates: Vec<u32> = [(9, 6), (9, 5)]
            .into_iter()
            .map(|pos| g.spawn_test_unit("warrior", 0, pos))
            .collect();
        let mut on = ai(true, &g, target);
        for _ in 0..ROAD_HOLD_TURNS + 2 {
            let _ = on.note_stage_march(&g, soldier, target, &StageMarch::Hold { wet: 30 });
            for mate in &mates {
                let _ = on.note_stage_march(&g, *mate, target, &StageMarch::Ordinary);
            }
            on.reconcile_commitments(&mut g, 0);
            g.turn += 1;
        }
        assert!(
            !on.capture_stood_down.contains_key(&target),
            "one of three held is not the train"
        );
    }

    fn front(target: usize, turn: u32) -> crate::ai::advanced::one_war::OneWarFront {
        crate::ai::advanced::one_war::OneWarFront {
            target,
            since: turn,
            ledger: (0, 0, 0, 0),
            window: std::collections::VecDeque::new(),
            tide_against_since: None,
            city_health: std::collections::BTreeMap::new(),
            sieges_advancing: 0,
            closure_wanted_since: None,
        }
    }

    /// `blocker-becomes-the-target`: the screen whose closed borders shut the
    /// stood-down siege's road -- weak, met, at peace and holding its own
    /// original capital -- is named as the war that opens the road, and the
    /// one-war gate admits it as the second front beside the cut-off war.
    /// With the gene off nothing is named, and a war on the screen (the road
    /// open) or a screen at strength names nothing either.
    #[test]
    fn a_weak_blocker_holding_a_capital_becomes_the_second_front() {
        let (mut game, target, soldier) = strip();
        game.record_contact(0, 1);
        game.record_contact(0, 2);
        let screen_capital = game.player_city_ids(1)[0];
        assert!(game.cities[&screen_capital].is_capital, "fixture: the screen's capital");
        for pos in [(6, 5), (6, 7)] {
            game.spawn_test_unit("warrior", 0, pos);
        }
        let hold = StageMarch::Hold { wet: 30 };
        for gene in [false, true] {
            let mut ai = ai(true, &game, target);
            if gene {
                ai.enable_blocker_becomes_the_target();
            }
            ai.disable_capital_prey_opens_a_front();
            ai.disable_capital_prey_opens_a_front_2();
            ai.enable_one_war_at_a_time();
            ai.one_war = Some(front(2, game.turn));
            let mut g = game.clone();
            run(&mut ai, &mut g, target, soldier, hold, ROAD_HOLD_TURNS);
            assert!(ai.capture_stood_down_holds(&g, target), "the road stand-down holds");
            assert!(ai.declaration_edge_2(&g, 0, 1).passes(), "fixture: the screen is weak");
            if gene {
                assert_eq!(ai.road_blocker_front(&g, 0), Some(1), "gene on: the screen is named");
                assert_eq!(ai.one_war_second_front(&g, 0), Some(1), "and opens the second front");
                // A war on the screen opens the road: nothing is named any more.
                let mut opened = g.clone();
                opened.at_war.insert((0, 1));
                opened.at_war.insert((1, 0));
                assert_eq!(ai.road_blocker_front(&opened, 0), None, "at war: the road is open");
            } else {
                assert_eq!(ai.road_blocker_front(&g, 0), None, "gene off: nothing is named");
                assert_ne!(ai.one_war_second_front(&g, 0), Some(1), "gene off: no second front on the screen");
            }
        }
    }

    /// `blocker-becomes-the-target`: a screen at strength -- out-producing
    /// nothing and short of the version-2 edge -- is not named, nor is a
    /// screen without an original capital.
    #[test]
    fn a_blocker_short_of_the_edge_or_without_a_capital_is_not_named() {
        let (mut game, target, soldier) = strip();
        game.record_contact(0, 1);
        game.record_contact(0, 2);
        let hold = StageMarch::Hold { wet: 30 };

        // At strength: the screen fields more than half our power.
        let mut strong = game.clone();
        for pos in [(19, 5), (19, 7), (17, 5), (17, 7)] {
            strong.spawn_test_unit("warrior", 1, pos);
        }
        let mut ai_strong = ai(true, &strong, target);
        ai_strong.enable_blocker_becomes_the_target();
        run(&mut ai_strong, &mut strong, target, soldier, hold, ROAD_HOLD_TURNS);
        assert!(ai_strong.capture_stood_down_holds(&strong, target));
        assert!(!ai_strong.declaration_edge_2(&strong, 0, 1).passes(), "fixture: short of the edge");
        assert_eq!(ai_strong.road_blocker_front(&strong, 0), None);

        // Weak, but its capital is not an original capital of a rival we need:
        // the screen's city is ours by origin.
        let screen = game.player_city_ids(1)[0];
        game.cities.get_mut(&screen).unwrap().original_owner = 0;
        let mut ai_nocap = ai(true, &game, target);
        ai_nocap.enable_blocker_becomes_the_target();
        let mut g = game.clone();
        run(&mut ai_nocap, &mut g, target, soldier, hold, ROAD_HOLD_TURNS);
        assert!(ai_nocap.capture_stood_down_holds(&g, target));
        assert_eq!(ai_nocap.road_blocker_front(&g, 0), None);
    }

    /// Live King civvis-20261005T215912Z (game 167)'s shape: at war with the
    /// target (player 2), a Diplomatic Victory contender the elimination
    /// front keeps the plan on; its siege's road shut by the screen's
    /// (player 1's) closed borders and stood down; the screen weak, holding
    /// its original capital, and at 90% of a Science Victory. The road
    /// blocker is named and the plan stays on the elimination front either
    /// way; under the gene the diplomacy desk declares on the screen, off it
    /// nothing does.
    #[test]
    fn a_blocker_the_elimination_front_displaces_is_still_declared_on() {
        let (mut game, target, soldier) = strip();
        game.turn = 200;
        game.record_contact(0, 1);
        game.record_contact(0, 2);
        game.found_city_for(0, (8, 4), None);
        game.players[0].gold = 3000.0;
        game.players[2].dvp = 18;
        game.players[1].science_projects.extend([
            "launch_earth_satellite".to_string(),
            "launch_moon_landing".to_string(),
            "launch_mars_colony".to_string(),
            "exoplanet_expedition".to_string(),
        ]);
        let observed = std::sync::Arc::make_mut(&mut game.observed_public_empire_stats)
            .entry(1)
            .or_default();
        observed.science_victory_points = Some(14.0);
        observed.science_victory_points_needed = Some(25.0);
        observed.science_victory_points_per_turn = Some(6.0);
        for (seat, power) in [(0, 4000.0), (1, 600.0), (2, 500.0)] {
            std::sync::Arc::make_mut(&mut game.observed_military_power).insert(seat, power);
        }
        let hold = StageMarch::Hold { wet: 30 };
        for gene in [false, true] {
            let mut ai = ai(true, &game, target);
            if gene {
                ai.enable_blocker_becomes_the_target();
            }
            ai.enable_diplomatic_contender_eliminated();
            ai.enable_one_war_at_a_time();
            ai.disable_capital_prey_opens_a_front();
            ai.disable_capital_prey_opens_a_front_2();
            ai.coalition_before_war = false;
            ai.coalition_before_war_2 = false;
            ai.coalition_before_war_3 = false;
            ai.one_war = Some(front(2, game.turn));
            let mut g = game.clone();
            run(&mut ai, &mut g, target, soldier, hold, ROAD_HOLD_TURNS);
            assert!(ai.capture_stood_down_holds(&g, target), "the road stand-down holds");
            assert_eq!(ai.diplomatic_contender_to_eliminate(&g, 0), Some(2), "fixture: eliminating the target");
            assert_eq!(ai.rival_pressure(&g, 1).0, GrandStrategy::Science, "fixture: the screen's race");
            let mut plan = ai.plan.clone().expect("the fixture's plan");
            plan.assessed_turn = g.turn;
            for _ in 0..12 {
                if g.is_at_war(0, 1) {
                    break;
                }
                assert_eq!(plan.target_player, Some(2), "the plan stays on the elimination front");
                ai.advanced_diplomacy(&mut g, 0, &plan);
                g.turn += 1;
            }
            if gene {
                assert_eq!(ai.road_blocker_front(&g, 0), None, "at war: the road is open");
                assert!(g.is_at_war(0, 1), "gene on: the displaced blocker is declared on");
            } else {
                assert!(!g.is_at_war(0, 1), "gene off: nothing declares on the screen");
            }
        }
    }

    /// `blocker-becomes-the-target` is a registered, reversible opt-in.
    #[test]
    fn blocker_becomes_the_target_is_a_reversible_opt_in() {
        let mut ai = AdvancedAi::new();
        assert!(!ai.blocker_becomes_the_target);
        ai.enable_blocker_becomes_the_target();
        assert!(ai.blocker_becomes_the_target);
        ai.disable_blocker_becomes_the_target();
        assert!(!ai.blocker_becomes_the_target);
        assert!(super::super::GENES
            .iter()
            .any(|gene| gene.tag == "blocker-becomes-the-target" && gene.opt_in()));
    }

    /// `closed-border-blocker-declares`: `strip` with powers read from the
    /// host -- ours 4,000, the screen's `screen`, the target's 500 -- and
    /// two marchers beside the held soldier, so the train is never half held
    /// and no road stand-down fires: the shape of a siege that holds short
    /// of closed borders instead.
    fn held_strip(screen: f64) -> (Game, u32, u32, Vec<u32>) {
        let (mut game, target, soldier) = strip();
        game.record_contact(0, 1);
        game.record_contact(0, 2);
        for (seat, power) in [(0, 4000.0), (1, screen), (2, 500.0)] {
            std::sync::Arc::make_mut(&mut game.observed_military_power).insert(seat, power);
        }
        let mates = [(9, 6), (9, 5)]
            .into_iter()
            .map(|pos| game.spawn_test_unit("warrior", 0, pos))
            .collect();
        (game, target, soldier, mates)
    }

    /// `turns` turns in which the soldier holds short of the screen and the
    /// mates march, reconciling the ledger after each.
    fn hold_beside_marchers(
        ai: &mut AdvancedAi,
        g: &mut Game,
        target: u32,
        soldier: u32,
        mates: &[u32],
        turns: u32,
    ) {
        for _ in 0..turns {
            let _ = ai.note_stage_march(g, soldier, target, &StageMarch::Hold { wet: 30 });
            for mate in mates {
                let _ = ai.note_stage_march(g, *mate, target, &StageMarch::Ordinary);
            }
            ai.reconcile_commitments(g, 0);
            g.turn += 1;
        }
    }

    fn blocker_ai(gene: bool, g: &Game, target: u32) -> AdvancedAi {
        let mut ai = ai(true, g, target);
        // As armed live: version 1 on beside the gene, or alone.
        ai.enable_blocker_becomes_the_target();
        if gene {
            ai.enable_closed_border_blocker_declares();
        }
        ai.disable_capital_prey_opens_a_front();
        ai.disable_capital_prey_opens_a_front_2();
        ai.enable_one_war_at_a_time();
        ai.one_war = Some(front(2, g.turn));
        ai
    }

    /// `closed-border-blocker-declares`: three turns of holds short of a
    /// screen at half our power or under name it as the road-blocker front
    /// and the second front, with no stand-down; two turns do not, nor does
    /// a run that stopped, nor the gene off (version 1 waits on a stand-down
    /// that never comes).
    #[test]
    fn a_weak_blocker_of_a_held_siege_becomes_the_second_front() {
        let (game, target, soldier, mates) = held_strip(600.0);
        for gene in [false, true] {
            let mut ai = blocker_ai(gene, &game, target);
            let mut g = game.clone();
            hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, ROAD_HOLD_TURNS - 1);
            assert_eq!(ai.road_blocker_front(&g, 0), None, "two turns of holds name nothing");
            hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, 1);
            assert!(!ai.capture_stood_down.contains_key(&target), "fixture: the siege holds, never stood down");
            if gene {
                assert_eq!(ai.closed_border_front(&g, 0), Some(1), "gene on: the screen is named");
                assert_eq!(ai.road_blocker_front(&g, 0), Some(1));
                assert_eq!(ai.one_war_second_front(&g, 0), Some(1), "and opens the second front");
                let plan = ai.plan.clone().expect("the fixture's plan");
                assert!(ai.second_front_displaced(&g, 0, 1, &plan));
                // At war with the screen the road is open: nothing is named.
                let mut opened = g.clone();
                opened.at_war.insert((0, 1));
                opened.at_war.insert((1, 0));
                assert_eq!(ai.road_blocker_front(&opened, 0), None, "at war: the road is open");
                // A run that stopped two turns ago is no hold.
                let mut later = g.clone();
                later.turn += 1;
                assert_eq!(ai.closed_border_front(&later, 0), None, "a stale run names nothing");
            } else {
                assert_eq!(ai.road_blocker_front(&g, 0), None, "gene off: version 1 waits on a stand-down");
                assert_ne!(ai.one_war_second_front(&g, 0), Some(1));
            }
        }
    }

    /// `closed-border-blocker-declares`: a break of a turn restarts the run.
    #[test]
    fn a_broken_hold_restarts_the_run() {
        let (game, target, soldier, mates) = held_strip(600.0);
        let mut ai = blocker_ai(true, &game, target);
        let mut g = game.clone();
        hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, 2);
        g.turn += 1; // a turn without a hold
        hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, 2);
        assert_eq!(ai.closed_border_front(&g, 0), None, "two and two are not three running");
        hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, 1);
        assert_eq!(ai.closed_border_front(&g, 0), Some(1));
    }

    /// `closed-border-blocker-declares`: a screen over half our power, or one
    /// we are friends with, is not named however long it holds the road.
    #[test]
    fn a_strong_or_friendly_blocker_is_not_named() {
        let (game, target, soldier, mates) = held_strip(2_100.0);
        let mut ai = blocker_ai(true, &game, target);
        let mut g = game.clone();
        hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, ROAD_HOLD_TURNS + 2);
        assert_eq!(ai.closed_border_front(&g, 0), None, "over half our power");

        // Weak now, but a 30-turn peak over two-thirds of ours.
        let (game, target, soldier, mates) = held_strip(600.0);
        let mut ai = blocker_ai(true, &game, target);
        ai.rival_power_peak_seen
            .entry(1)
            .or_default()
            .push((game.turn, 3_000.0));
        let mut g = game.clone();
        hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, ROAD_HOLD_TURNS);
        assert_eq!(ai.closed_border_front(&g, 0), None, "a recent peak at strength");

        let (mut game, target, soldier, mates) = held_strip(600.0);
        game.players[0].friends_until.insert(1, game.turn + 30);
        let mut ai = blocker_ai(true, &game, target);
        let mut g = game.clone();
        hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, ROAD_HOLD_TURNS);
        assert_eq!(ai.closed_border_front(&g, 0), None, "a friend is no campaign target");
    }

    /// `closed-border-blocker-declares`: a city-state screen is named alone
    /// or under a suzerain we outgun; a strong suzerain at peace with us
    /// keeps it off the list, and so does our own suzerainty.
    #[test]
    fn a_city_state_blocker_needs_a_weak_suzerain() {
        let (mut game, target, _, _) = held_strip(300.0);
        game.players[1].is_minor = true;
        let ai = blocker_ai(true, &game, target);
        assert!(ai.closed_border_blocker_is_weak(&game, 0, 1), "no suzerain: named");

        // The target, at war with us, is its suzerain: the city-state is in
        // that war already, its borders open, and there is nothing to declare.
        let mut fought = game.clone();
        fought.players[2].envoys.push((1, 5));
        assert_eq!(fought.suzerain_of(1), Some(2), "fixture: the target holds it");
        assert!(fought.is_at_war(0, 1), "fixture: dragged into its suzerain's war");
        assert!(!ai.closed_border_blocker_is_weak(&fought, 0, 1), "already at war: not named");

        // At peace with that suzerain, and it at strength.
        let mut strong = fought.clone();
        strong.at_war.remove(&(0, 2));
        strong.at_war.remove(&(2, 0));
        std::sync::Arc::make_mut(&mut strong.observed_military_power).insert(2, 2_500.0);
        assert!(!ai.closed_border_blocker_is_weak(&strong, 0, 1), "a strong suzerain at peace");
        // At peace, but weak.
        std::sync::Arc::make_mut(&mut strong.observed_military_power).insert(2, 900.0);
        assert!(ai.closed_border_blocker_is_weak(&strong, 0, 1), "a weak suzerain at peace");

        // Ours.
        let mut ours = game.clone();
        ours.players[0].envoys.push((1, 5));
        assert!(!ai.closed_border_blocker_is_weak(&ours, 0, 1), "our own city-state");
    }

    /// `closed-border-blocker-declares` at the declaration desk, with version
    /// 1 armed in both runs as it is live: the plan stays on the target's
    /// war, the siege holds short of the weak screen and is never stood down
    /// for the road, and under the gene the desk declares on the screen; off
    /// it nothing does.
    #[test]
    fn the_desk_declares_on_a_weak_blocker_of_a_held_siege() {
        let (mut game, target, soldier, mates) = held_strip(600.0);
        game.turn = 150;
        game.found_city_for(0, (8, 4), None);
        game.players[0].gold = 3000.0;
        for gene in [false, true] {
            let mut ai = blocker_ai(gene, &game, target);
            ai.coalition_before_war = false;
            ai.coalition_before_war_2 = false;
            ai.coalition_before_war_3 = false;
            let mut g = game.clone();
            hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, ROAD_HOLD_TURNS);
            let mut plan = ai.plan.clone().expect("the fixture's plan");
            for _ in 0..12 {
                if g.is_at_war(0, 1) {
                    break;
                }
                plan.assessed_turn = g.turn;
                assert_eq!(plan.target_player, Some(2), "the plan stays on the target's war");
                ai.advanced_diplomacy(&mut g, 0, &plan);
                hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, 1);
            }
            assert!(ai.siege_road_closed.is_empty(), "no road stand-down for version 1");
            if gene {
                assert!(g.is_at_war(0, 1), "gene on: the weak blocker is declared on");
            } else {
                assert!(!g.is_at_war(0, 1), "gene off: nothing declares on the screen");
            }
        }
    }

    /// `closed-border-blocker-declares` is a registered, reversible opt-in.
    #[test]
    fn closed_border_blocker_declares_is_a_reversible_opt_in() {
        let mut ai = AdvancedAi::new();
        assert!(!ai.closed_border_blocker_declares);
        ai.enable_closed_border_blocker_declares();
        assert!(ai.closed_border_blocker_declares);
        ai.disable_closed_border_blocker_declares();
        assert!(!ai.closed_border_blocker_declares);
        assert!(super::super::GENES
            .iter()
            .any(|gene| gene.tag == "closed-border-blocker-declares" && gene.opt_in()));
    }
}
