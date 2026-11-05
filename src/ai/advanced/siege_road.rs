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

#[cfg(test)]
mod native_memory_tests;

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
        if !self.siege_target_needs_a_road && !self.envoys_open_the_siege_road {
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

    pub(super) fn remap_siege_road_cities(&mut self, previous: &Game, next: &Game) {
        let city = |id| {
            previous
                .cities
                .get(&id)
                .and_then(|city| next.city_at(city.pos))
        };
        self.siege_road_tally = std::mem::take(&mut self.siege_road_tally)
            .into_iter()
            .filter_map(|(id, tally)| city(id).map(|id| (id, tally)))
            .collect();
        self.siege_road_closed = std::mem::take(&mut self.siege_road_closed)
            .into_iter()
            .filter_map(|(id, closed)| city(id).map(|id| (id, closed)))
            .collect();
    }

    /// Fresh native boards assign new unit ids. Preserve only holds whose
    /// actual host unit survived the adapter's explicit identity mapping.
    pub(super) fn remap_envoy_road_units(&mut self, map: &BTreeMap<u32, u32>) {
        if !self.envoys_open_the_siege_road && !self.siege_target_needs_a_road {
            return;
        }
        for tally in self.siege_road_tally.values_mut() {
            tally.held = tally
                .held
                .iter()
                .filter_map(|(uid, owner)| map.get(uid).map(|new| (*new, *owner)))
                .collect();
            tally.marched = tally
                .marched
                .iter()
                .filter_map(|uid| map.get(uid).copied())
                .collect();
        }
    }

    /// Current or previous-turn Stage holds on an existing war's major city.
    /// Recheck the live route and unit owner; a stale hold, dead unit or a
    /// finished war cannot spend envoys on an unrelated border.
    pub(super) fn siege_minor_passage_blockers(&self, g: &Game, pid: usize) -> BTreeSet<usize> {
        self.siege_road_tally
            .iter()
            .filter_map(|(cid, tally)| {
                let city = g.cities.get(cid)?;
                let enemy = g.players.get(city.owner)?;
                (tally.turn <= g.turn
                    && g.turn - tally.turn <= 1
                    && enemy.alive
                    && !enemy.is_minor
                    && !enemy.is_barbarian
                    && g.is_at_war(pid, city.owner))
                .then_some((city, tally))
            })
            .flat_map(|(city, tally)| {
                tally.held.iter().filter_map(move |(uid, recorded)| {
                    let owner = (*recorded)?;
                    let minor = g.players.get(owner)?;
                    (minor.alive
                        && minor.is_minor
                        && !minor.is_barbarian
                        && g.units.get(uid).is_some_and(|unit| unit.owner == pid)
                        && border_blocker(g, *uid, city.pos) == Some(owner))
                    .then_some(owner)
                })
            })
            .collect()
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
            // `racer-blocker-declares`: see `advanced/silent_counter.rs`.
            .or_else(|| self.racer_blocker_front(g, pid))
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
    pub(super) fn closed_border_blocker_is_weak(
        &self,
        g: &Game,
        pid: usize,
        blocker: usize,
    ) -> bool {
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
    let open =
        |owner: usize| owner == seat || g.is_at_war(seat, owner) || g.has_open_borders(seat, owner);
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
mod tests;
