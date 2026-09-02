//! One war at a time: one campaign front, a home guard, and peace on every
//! other front — and on the campaign front too, once the tide has turned
//! against us for long enough and nothing in reach is worth the next turn.
//!
//! ★★★★ THE WAR DESK COUNTS ITS WARS ONLY WHEN IT OPENS ONE. Every offensive
//! opening already refuses a second front — the elective declaration
//! (`major_wars > 0`), the appointment (`may_form_war_plan`), the air surge
//! (`air_surge_fronts`) and the raid — but nothing decides what to do once a
//! second war *arrives*: a neighbour's declaration, a Joint War accepted at
//! +300, an appointed attack that launches into a war that began after the
//! appointment. From then on the peace desk treats every enemy the same way
//! — outmatched, Recovery, or fatigued — the plan re-aims at whichever rival
//! prices lowest this turn, and the force planner hands every group the
//! union of all enemies, so an empire fighting two neighbours prosecutes both
//! at the same lukewarm pace until one of the generic clauses fires. The
//! operator's rule (2026-08-24): *fight one war at a time; keep some defence
//! at home and concentrate the rest on a single war; fight while there is
//! still something to take and pillage; sue for peace when the tide is no
//! longer in our favour, consistently.*
//!
//! What the gene does, all of it inert while the flag is off:
//!
//! 1. **One front.** Each turn, among the majors we are at war with, one is
//!    the *campaign front*: the front already chosen while it is still at
//!    war with us, else the appointed war's target, else the plan's, else the
//!    enemy whose cities are nearest our soldiers. Every other major at war
//!    with us is a *second front*: offered peace every turn, its white peace
//!    accepted (`incoming_deal_value` +320). A Joint War offer while any war
//!    burns is refused outright.
//! 2. **No second declaration.** The appointed attack and the air surge hold
//!    while a major war is on against anyone else — the appointment gate
//!    runs at appointment, this one at the declaration. The one exception is
//!    a rival about to win (`urgent_victory_threat`): losing the game is the
//!    larger cost.
//! 3. **Concentrate.** `assess` keeps the plan's target on the front while
//!    the front is at war, and the force planner's objective enemies are the
//!    front alone — plus whoever is within relief range of a threatened city
//!    of ours, so the column still turns for a city about to fall. The bounded
//!    barbarian response (`barbarian_garrison_step`,
//!    `barbarian_response_objective`) stays outside the major-war planner.
//! 4. **Fight while there is something to take.** On the front the fatigue
//!    clause (war age ≥ 24, no capture in 12) stands down while a prize is in
//!    reach — a front city our soldiers stand at whose health is falling or
//!    already below `ONE_WAR_CITY_BROKEN_FRACTION`, or an unpillaged tile a
//!    soldier reaches within `ONE_WAR_PILLAGE_REACH_TURNS` — and the tide is
//!    not against us. The outmatched clauses (0.62 offer, 0.85 accept) keep
//!    their shape.
//! 5. **Sue when the tide turns, consistently.** The exchange on the front is
//!    read off the engine's own war ledger (`Game::wars`: units and cities
//!    lost by each side) at every observation. The net over the last
//!    `ONE_WAR_TIDE_WINDOW` standard turns is the tide; when it runs against
//!    us a clock starts, and after `ONE_WAR_TIDE_PATIENCE` standard turns of
//!    it with nothing in reach — or at once on a rout, `ONE_WAR_ROUT_NET` —
//!    peace is offered every turn and a white peace accepted. A capture or a
//!    favourable window stops the clock.
//!
//! The deployment genome pins `one-war-at-a-time` on after its +1.00 pp
//! displayed pooled Diff; the registry row stays a reversible `Kind::OptIn`
//! so `gene_screen --genes one-war-at-a-time` can still price it.

use std::collections::{BTreeMap, VecDeque};

use super::AdvancedAi;
use crate::game::{DiplomaticDeal, Game};
use crate::Pos;

/// The tide is read over this many standard turns of observations.
pub(crate) const ONE_WAR_TIDE_WINDOW: u32 = 8;
/// The tide must run against us for this many standard turns before peace
/// is sued for on the campaign front — "consistently", not one bad turn.
pub(crate) const ONE_WAR_TIDE_PATIENCE: u32 = 6;
/// A window net this far below zero is a rout: sue at once, prizes or not.
pub(crate) const ONE_WAR_ROUT_NET: i32 = -4;
/// What a city changing hands weighs against a unit in the exchange.
pub(crate) const ONE_WAR_CITY_WEIGHT: i32 = 3;
/// Our soldiers within this many tiles of an enemy city are besieging it,
/// for the purpose of "something to take".
pub(crate) const ONE_WAR_SIEGE_REACH: i32 = 3;
/// A pillage prize counts when a soldier reaches it within this many turns.
pub(crate) const ONE_WAR_PILLAGE_REACH_TURNS: i32 = 2;
/// A city whose defence (city + wall health) is below this fraction of full
/// while our soldiers stand at it is a city to take, even if the last
/// observation saw no drop.
pub(crate) const ONE_WAR_CITY_BROKEN_FRACTION: f64 = 0.5;
/// A second-front unit this close to a threatened city of ours keeps that
/// enemy in the force planner's sights: the relief column's own radius.
pub(crate) const ONE_WAR_RELIEF_REACH: i32 = 8;
/// A city's full health; the engine's ceiling (`Game::do_end_turn` heals to
/// 200), walls on top of it.
pub(crate) const ONE_WAR_CITY_FULL_HP: i32 = 200;

/// The campaign front as the gene sees it, carried across turns.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OneWarFront {
    /// The major we are concentrating on.
    pub(crate) target: usize,
    /// The turn this front was chosen.
    pub(crate) since: u32,
    /// The war ledger's loss counts at the last observation:
    /// (our units, their units, our cities, their cities).
    pub(crate) ledger: (u32, u32, u32, u32),
    /// Net exchange per observation over the tide window, newest last.
    pub(crate) window: VecDeque<(u32, i32)>,
    /// The turn the tide turned against us, if it has and has not turned
    /// back since.
    pub(crate) tide_against_since: Option<u32>,
    /// City and wall health of the front's cities at the last observation.
    pub(crate) city_health: BTreeMap<u32, (i32, i32)>,
    /// Cities of the front whose health fell at the last observation.
    pub(crate) sieges_advancing: usize,
}

impl OneWarFront {
    fn new(target: usize, turn: u32) -> Self {
        Self {
            target,
            since: turn,
            ledger: (0, 0, 0, 0),
            window: VecDeque::new(),
            tide_against_since: None,
            city_health: BTreeMap::new(),
            sieges_advancing: 0,
        }
    }

    /// The net exchange over the window: positive is ours.
    pub(crate) fn window_net(&self) -> i32 {
        self.window.iter().map(|(_, net)| *net).sum()
    }
}

/// Why the gene wants peace with a rival this turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OneWarPeace {
    /// Not the campaign front: one war at a time.
    SecondFront,
    /// The campaign front, and the tide has run against us for long enough
    /// with nothing left in reach worth the next turn.
    TideTurned,
    /// The campaign front, and the last window was a rout.
    Rout,
}

impl OneWarPeace {
    pub(crate) fn reason(self) -> &'static str {
        match self {
            OneWarPeace::SecondFront => "one war at a time, and this is not the one",
            OneWarPeace::TideTurned => {
                "the tide has run against us for long enough and nothing in reach is worth the next turn"
            }
            OneWarPeace::Rout => "the last window was a rout",
        }
    }
}

impl AdvancedAi {
    /// The living majors we are at war with.
    pub(crate) fn one_war_enemies(&self, g: &Game, pid: usize) -> Vec<usize> {
        g.players
            .iter()
            .filter(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && g.is_at_war(pid, other.id)
            })
            .map(|other| other.id)
            .collect()
    }

    /// The campaign front's target while the gene is on and a major war is
    /// being fought; `None` otherwise.
    pub(crate) fn one_war_front(&self) -> Option<usize> {
        if !self.one_war_at_a_time {
            return None;
        }
        self.one_war.as_ref().map(|front| front.target)
    }

    /// Choose the front among the enemies: the appointed war's target, then
    /// the plan's, then the one whose nearest city is nearest to our army,
    /// then the lowest id. The choice sticks while its target stays at war
    /// with us, so a reinforcement that arrives does not move the front.
    fn one_war_choose_front(&self, g: &Game, pid: usize, enemies: &[usize]) -> Option<usize> {
        if let Some(current) = self
            .one_war
            .as_ref()
            .map(|front| front.target)
            .filter(|target| enemies.contains(target))
        {
            return Some(current);
        }
        if let Some(appointed) = self
            .war_plan
            .as_ref()
            .map(|war| war.target_player)
            .filter(|target| enemies.contains(target))
        {
            return Some(appointed);
        }
        if let Some(planned) = self
            .plan
            .as_ref()
            .and_then(|plan| plan.target_player)
            .filter(|target| enemies.contains(target))
        {
            return Some(planned);
        }
        let soldiers: Vec<Pos> = self.one_war_soldiers(g, pid);
        enemies.iter().copied().min_by_key(|enemy| {
            let nearest = g
                .player_city_ids(*enemy)
                .into_iter()
                .map(|cid| g.cities[&cid].pos)
                .map(|city| {
                    soldiers
                        .iter()
                        .map(|pos| g.wdist(*pos, city))
                        .min()
                        .unwrap_or(i32::MAX)
                })
                .min()
                .unwrap_or(i32::MAX);
            (nearest, *enemy)
        })
    }

    /// Positions of our land soldiers fit to fight.
    fn one_war_soldiers(&self, g: &Game, pid: usize) -> Vec<Pos> {
        g.player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| {
                let unit = &g.units[&uid];
                let spec = &g.rules.units[unit.kind];
                (spec.class == "military"
                    && spec.domain.as_deref() != Some("air")
                    && spec.domain.as_deref() != Some("sea")
                    && !g.is_embarked(unit))
                .then_some(unit.pos)
            })
            .collect()
    }

    /// The war ledger's loss counts for our war with `other`:
    /// (our units, their units, our cities, their cities). Zero before the
    /// first blow: the record is opened on demand.
    pub(super) fn one_war_ledger(g: &Game, pid: usize, other: usize) -> (u32, u32, u32, u32) {
        let key = (pid.min(other), pid.max(other));
        g.wars
            .get(&key)
            .map(|war| {
                let ours = war.losses.get(&pid);
                let theirs = war.losses.get(&other);
                (
                    ours.map_or(0, |l| l.units),
                    theirs.map_or(0, |l| l.units),
                    ours.map_or(0, |l| l.cities),
                    theirs.map_or(0, |l| l.cities),
                )
            })
            .unwrap_or((0, 0, 0, 0))
    }

    /// The observation pass: pick or keep the front, read the exchange since
    /// the last observation off the war ledger, and run the tide clock.
    /// Called once per acting turn from `observe_campaign`; exact no-op with
    /// the gene off.
    pub(crate) fn one_war_observe(&mut self, g: &Game, pid: usize) {
        if !self.one_war_at_a_time {
            self.one_war = None;
            return;
        }
        let enemies = self.one_war_enemies(g, pid);
        let Some(target) = self.one_war_choose_front(g, pid, &enemies) else {
            self.one_war = None;
            return;
        };
        let mut front = match self.one_war.take() {
            Some(front) if front.target == target => front,
            _ => {
                let mut fresh = OneWarFront::new(target, g.turn);
                fresh.ledger = Self::one_war_ledger(g, pid, target);
                fresh
            }
        };
        let ledger = Self::one_war_ledger(g, pid, target);
        let (our_units, their_units, our_cities, their_cities) = (
            ledger.0.saturating_sub(front.ledger.0) as i32,
            ledger.1.saturating_sub(front.ledger.1) as i32,
            ledger.2.saturating_sub(front.ledger.2) as i32,
            ledger.3.saturating_sub(front.ledger.3) as i32,
        );
        front.ledger = ledger;
        let net = their_units - our_units + ONE_WAR_CITY_WEIGHT * (their_cities - our_cities);
        front.window.push_back((g.turn, net));
        let window = g.standard_duration(ONE_WAR_TIDE_WINDOW).max(1);
        while front
            .window
            .front()
            .is_some_and(|(turn, _)| g.turn.saturating_sub(*turn) >= window)
        {
            front.window.pop_front();
        }
        // The sieges: a front city whose health fell since the last
        // observation is a city being taken.
        let mut health_now = BTreeMap::new();
        let mut advancing = 0;
        for cid in g.player_city_ids(target) {
            let city = &g.cities[&cid];
            let health = (city.hp, city.wall_hp);
            if front
                .city_health
                .get(&cid)
                .is_some_and(|before| health.0 < before.0 || health.1 < before.1)
            {
                advancing += 1;
            }
            health_now.insert(cid, health);
        }
        front.city_health = health_now;
        front.sieges_advancing = advancing;
        // The tide clock: starts when the window runs against us, and only
        // a favourable window or a capture turns it back.
        let window_net = front.window_net();
        if their_cities > 0 || window_net > 0 {
            front.tide_against_since = None;
        } else if window_net < 0 {
            front.tide_against_since.get_or_insert(g.turn);
        }
        self.one_war = Some(front);
    }

    /// Whether the front still offers something worth the next turn: a city
    /// our soldiers are at whose health is falling or already broken, or
    /// unpillaged tiles a soldier reaches within `ONE_WAR_PILLAGE_REACH_TURNS`.
    pub(crate) fn one_war_prizes_in_reach(&self, g: &Game, pid: usize) -> bool {
        let Some(front) = self.one_war.as_ref().filter(|_| self.one_war_at_a_time) else {
            return false;
        };
        let target = front.target;
        let strikers: Vec<(Pos, i32)> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| {
                let unit = &g.units[&uid];
                let spec = &g.rules.units[unit.kind];
                if spec.class != "military"
                    || spec.domain.as_deref() == Some("air")
                    || spec.domain.as_deref() == Some("sea")
                    || g.is_embarked(unit)
                    || unit.hp < 50
                {
                    return None;
                }
                let reach =
                    (g.unit_max_moves(uid).floor() as i32).max(1) * ONE_WAR_PILLAGE_REACH_TURNS;
                Some((unit.pos, reach))
            })
            .collect();
        if strikers.is_empty() {
            return false;
        }
        for cid in g.player_city_ids(target) {
            let city = &g.cities[&cid];
            let at_it = strikers
                .iter()
                .any(|(pos, _)| g.wdist(*pos, city.pos) <= ONE_WAR_SIEGE_REACH);
            if !at_it {
                continue;
            }
            let falling = front
                .city_health
                .get(&cid)
                .is_some_and(|(hp, wall)| (city.hp, city.wall_hp) < (*hp, *wall))
                || front.sieges_advancing > 0;
            let full = ONE_WAR_CITY_FULL_HP + g.city_max_wall_hp(city).max(0);
            let broken = ((city.hp.max(0) + city.wall_hp.max(0)) as f64)
                < full as f64 * ONE_WAR_CITY_BROKEN_FRACTION;
            if falling || broken {
                return true;
            }
        }
        let explored = &g.players[pid].explored;
        for cid in g.player_city_ids(target) {
            let city = &g.cities[&cid];
            for pos in city.owned_tiles.iter().copied() {
                if !explored.contains(&pos) || !g.pillageable_after_declaring(pid, pos) {
                    continue;
                }
                if g.map
                    .get(pos)
                    .is_some_and(|tile| tile.owner_city == Some(cid))
                    && strikers
                        .iter()
                        .any(|(spos, reach)| g.wdist(*spos, pos) <= *reach)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Whether the gene wants peace with `other` this turn, and why.
    pub(crate) fn one_war_peace(&self, g: &Game, pid: usize, other: usize) -> Option<OneWarPeace> {
        let front = self.one_war.as_ref().filter(|_| self.one_war_at_a_time)?;
        if !g.is_at_war(pid, other) || g.players[other].is_minor || g.players[other].is_barbarian {
            return None;
        }
        if front.target != other {
            return Some(OneWarPeace::SecondFront);
        }
        if front.window_net() <= ONE_WAR_ROUT_NET {
            return Some(OneWarPeace::Rout);
        }
        let against_for = front
            .tide_against_since
            .map(|since| g.turn.saturating_sub(since))?;
        if against_for >= g.standard_duration(ONE_WAR_TIDE_PATIENCE).max(1)
            && !self.one_war_prizes_in_reach(g, pid)
        {
            return Some(OneWarPeace::TideTurned);
        }
        None
    }

    /// Whether the gene keeps pressing the war on `other` against the
    /// fatigue clause: `other` is the campaign front, the tide is not
    /// against us, and a prize is in reach.
    pub(crate) fn one_war_presses(&self, g: &Game, pid: usize, other: usize) -> bool {
        let Some(front) = self.one_war.as_ref().filter(|_| self.one_war_at_a_time) else {
            return false;
        };
        front.target == other
            && g.is_at_war(pid, other)
            && front.tide_against_since.is_none()
            && self.one_war_prizes_in_reach(g, pid)
    }

    /// Whether a declaration on `target` is held: a major war is already
    /// being fought against someone else and `target` is not about to win.
    pub(crate) fn one_war_holds_declaration(&self, g: &Game, pid: usize, target: usize) -> bool {
        if !self.one_war_at_a_time || g.is_at_war(pid, target) {
            return false;
        }
        let other_war = self
            .one_war_enemies(g, pid)
            .into_iter()
            .any(|enemy| enemy != target);
        other_war && !self.urgent_victory_threat(g, target)
    }

    /// A Joint War offer while any major war burns is a second front by
    /// treaty: refused outright, whatever the target is worth to the plan.
    pub(crate) fn one_war_refuses_joint_war(
        &self,
        g: &Game,
        pid: usize,
        deal: &DiplomaticDeal,
    ) -> bool {
        self.one_war_at_a_time
            && deal.joint_war_target.is_some()
            && !self.one_war_enemies(g, pid).is_empty()
    }

    /// The enemies the force planner aims a group at: the front alone, plus
    /// any enemy with a unit within relief range of a threatened city of
    /// ours, so the column still turns for a city about to fall. The full
    /// set when the gene is off or no front is chosen.
    pub(crate) fn one_war_objective_enemies(
        &self,
        g: &Game,
        threatened_city: Option<u32>,
        enemies: &[usize],
    ) -> Vec<usize> {
        let Some(front) = self.one_war_front() else {
            return enemies.to_vec();
        };
        if !enemies.contains(&front) {
            return enemies.to_vec();
        }
        let threatened = threatened_city
            .and_then(|cid| g.cities.get(&cid))
            .map(|city| city.pos);
        enemies
            .iter()
            .copied()
            .filter(|enemy| {
                *enemy == front
                    || threatened.is_some_and(|city| {
                        g.units.values().any(|unit| {
                            unit.owner == *enemy
                                && g.rules.units[unit.kind].class == "military"
                                && g.wdist(unit.pos, city) <= ONE_WAR_RELIEF_REACH
                        })
                    })
            })
            .collect()
    }
}
