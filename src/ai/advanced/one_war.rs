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
//!    the *campaign front*: an urgent actionable military denial first (unless
//!    the operator ordered a target), then the front already chosen while it
//!    is still at war with us. A Domination front that no longer holds any
//!    required original capital hands the army to another active war that
//!    does. Otherwise use the appointed war's target, the plan's, then the
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

use super::{AdvancedAi, GrandStrategy, VictoryTarget};
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
/// A breached city this low can be taken before an army redeploys to a new
/// rival. The capture body may be a few tiles behind the guns.
pub(crate) const ONE_WAR_FINISH_HP: i32 = 60;
pub(crate) const ONE_WAR_FINISH_REACH: i32 = 4;
/// A front we outgun this many times over is not traded away for a
/// counter-campaign against a rival whose clock is not yet urgent.
pub(crate) const ONE_WAR_CRUSHED_RATIO: f64 = 4.0;
/// A front we outgun this many times over is still being won: a bad window
/// of losses there is the price of a siege, not a rout or a turned tide.
pub(crate) const ONE_WAR_WINNING_RATIO: f64 = 2.0;
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
    /// The required capital is secure and another capital remains to pursue.
    CapitalSecured,
    /// Another rival now holds this front's original capital.
    CapitalElsewhere,
    /// Another rival's religious or culture finish outranks optional conquest.
    VictoryThreat,
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
            OneWarPeace::CapitalSecured => {
                "the required capital is secure and another capital remains to pursue"
            }
            OneWarPeace::CapitalElsewhere => {
                "this rival's original capital is now held by another opponent"
            }
            OneWarPeace::VictoryThreat => {
                "freeing the Domination army to counter a rival victory threat"
            }
            OneWarPeace::TideTurned => {
                "the tide has run against us for long enough and nothing in reach is worth the next turn"
            }
            OneWarPeace::Rout => "the last window was a rout",
        }
    }
}

impl AdvancedAi {
    /// A congress vote can jump Diplomatic Victory pressure once, then stay
    /// flat for the whole congress interval. Below the ordinary denial bar,
    /// only the projected slope can call that jump urgent. It should still
    /// prepare a counter, but cannot pin a Domination army after we safely
    /// capture that rival's capital or a third party takes it.
    fn one_war_projected_diplomacy_below_bar(&self, g: &Game, rival: usize) -> bool {
        let pressure = self.rival_victory_pressure(g, rival);
        pressure.strategy == GrandStrategy::Diplomacy && pressure.progress < super::STOCK_DENIAL_BAR
    }

    /// A stable captured capital, or one captured by a third party, completes
    /// this front's Domination purpose. Prefer another known capital owner
    /// over the former owner's ordinary towns. The ordinary city chooser
    /// still enforces occupation safety and can prepare through that next
    /// rival's frontier before taking its capital.
    pub(super) fn domination_followup_target(
        &self,
        g: &Game,
        pid: usize,
        completed_rival: Option<usize>,
    ) -> Option<usize> {
        if !self.one_war_at_a_time
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || self.forced_target_player.is_some()
        {
            return None;
        }
        let displaced_owner = completed_rival.and_then(|other| {
            g.cities
                .values()
                .find(|city| {
                    city.is_capital
                        && city.original_owner == other
                        && city.owner != other
                        && city.owner != pid
                        && !g.same_team(pid, city.owner)
                        && g.players
                            .get(city.owner)
                            .is_some_and(|player| !player.is_minor && !player.is_barbarian)
                        && self.campaign_target_legal(g, pid, city.owner)
                })
                .map(|city| city.owner)
        });
        let secured = g.cities.values().any(|city| {
            city.owner == pid
                && city.is_capital
                && city.original_owner != pid
                && completed_rival.is_none_or(|other| city.original_owner == other)
                && !g.players[city.original_owner].is_minor
                && !g.players[city.original_owner].is_barbarian
                && city.loyalty >= 75.0
                && g.city_loyalty_per_turn(city) >= 0.0
        });
        if completed_rival.is_some_and(|other| {
            g.emergency_war_pair(pid, other)
                || (self.urgent_victory_threat(g, other)
                    && !((displaced_owner.is_some() || secured)
                        && self.one_war_projected_diplomacy_below_bar(g, other)))
                || g.cities.values().any(|city| {
                    city.owner == other
                        && city.is_capital
                        && !g.players[city.original_owner].is_minor
                        && !g.players[city.original_owner].is_barbarian
                })
        }) {
            return None;
        }
        // We do not need to finish this rival's ordinary towns if a third
        // party took its original capital. That third party now owns the
        // Domination objective even when we captured no capital ourselves.
        if let Some(owner) = displaced_owner {
            return Some(owner);
        }
        // A rival can consolidate another major's original capital before
        // we take one. In that case a former owner with only ordinary towns
        // is not a useful peacetime target either.
        let foreign_consolidation = completed_rival.is_none()
            && g.cities.values().any(|city| {
                city.is_capital
                    && city.owner != pid
                    && city.owner != city.original_owner
                    && !g.same_team(pid, city.owner)
                    && g.players
                        .get(city.owner)
                        .is_some_and(|owner| !owner.is_minor && !owner.is_barbarian)
                    && g.players
                        .get(city.original_owner)
                        .is_some_and(|founder| !founder.is_minor && !founder.is_barbarian)
            });
        if !secured && !foreign_consolidation {
            return None;
        }
        g.cities
            .values()
            .filter(|city| {
                city.is_capital
                    && city.owner != pid
                    && Some(city.owner) != completed_rival
                    && !g.same_team(pid, city.owner)
                    && !g.players[city.original_owner].is_minor
                    && !g.players[city.original_owner].is_barbarian
                    && (!g.same_team(pid, city.original_owner) || city.owner != city.original_owner)
                    && self.campaign_target_legal(g, pid, city.owner)
            })
            .min_by(|left, right| {
                self.campaign_city_value(g, pid, left, GrandStrategy::Conquest)
                    .total_cmp(&self.campaign_city_value(g, pid, right, GrandStrategy::Conquest))
                    .then(left.id.cmp(&right.id))
            })
            .map(|city| city.owner)
    }

    /// The living majors whose cities still form a war front. A defeated
    /// rival can remain alive and at war in a live host after losing its last
    /// city; that stale war must not hold the next capital's declaration.
    pub(crate) fn one_war_enemies(&self, g: &Game, pid: usize) -> Vec<usize> {
        g.players
            .iter()
            .filter(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && !g.player_city_ids(other.id).is_empty()
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
    /// with us, unless an urgent military denial needs a different active
    /// front or its known original capital has changed hands and another
    /// active front holds a required capital. An explicit operator target
    /// keeps its existing precedence.
    fn one_war_choose_front(&self, g: &Game, pid: usize, enemies: &[usize]) -> Option<usize> {
        let current = self
            .one_war
            .as_ref()
            .map(|front| front.target)
            .filter(|target| enemies.contains(target));
        let capital_handoff = current
            .and_then(|front| self.domination_followup_target(g, pid, Some(front)))
            .filter(|target| enemies.contains(target));
        // The declaration gate already admits urgent victory denial. Once
        // that war exists, concentrate on it instead of immediately offering
        // the winning rival peace as a second front. Keep the ordinary rout
        // and sustained losing-tide safeguards on whichever front is chosen.
        if self.forced_target_player.is_none() {
            if let Some((rival, GrandStrategy::Conquest)) = self.actionable_victory_denial(g, pid) {
                if enemies.contains(&rival) && self.urgent_victory_threat(g, rival) {
                    // A congress jump can make a subthreshold Diplomatic
                    // score look urgent even after this rival lost its
                    // original capital. Follow the active war for that
                    // capital while retaining the projected warning.
                    if !(current == Some(rival)
                        && capital_handoff.is_some()
                        && self.one_war_projected_diplomacy_below_bar(g, rival))
                    {
                        return Some(rival);
                    }
                }
            }
        }
        // When the front's original capital moves to another rival, the old
        // war no longer advances Domination. If its new owner is already at
        // war with us, move the army there and offer the old front peace.
        if let Some(next) = capital_handoff {
            return Some(next);
        }
        if let Some(current) = current {
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

    /// Cumulative losses against `other`: our units, their units, our cities,
    /// their cities. Native unit counts come from confirmed combat events;
    /// each newly observed front seeds its baseline before reading changes.
    pub(super) fn one_war_ledger(
        &self,
        g: &Game,
        pid: usize,
        other: usize,
    ) -> (u32, u32, u32, u32) {
        let key = (pid.min(other), pid.max(other));
        let mut ledger = g
            .wars
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
            .unwrap_or((0, 0, 0, 0));
        if let Some(losses) = &self.host_war_unit_losses {
            ledger.0 = losses.get(&(pid, other)).copied().unwrap_or(0);
            ledger.1 = losses.get(&(other, pid)).copied().unwrap_or(0);
        }
        ledger
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
                fresh.ledger = self.one_war_ledger(g, pid, target);
                fresh
            }
        };
        let ledger = self.one_war_ledger(g, pid, target);
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

    /// Do not end a Domination front with a breached city and a healthy
    /// capture unit close enough to finish it. A distant or merely damaged
    /// city must not delay an urgent counter-campaign.
    fn one_war_capture_at_hand(&self, g: &Game, pid: usize, other: usize) -> bool {
        g.player_city_ids(other).into_iter().any(|cid| {
            let city = &g.cities[&cid];
            city.wall_hp <= 0
                && city.hp <= ONE_WAR_FINISH_HP
                && g.player_unit_ids(pid).into_iter().any(|uid| {
                    let unit = &g.units[&uid];
                    let spec = &g.rules.units[unit.kind];
                    spec.class == "military"
                        && spec.domain.as_deref() != Some("air")
                        && spec.domain.as_deref() != Some("sea")
                        && spec.is_melee_capable()
                        && !g.is_embarked(unit)
                        && unit.hp >= 50
                        && g.wdist(unit.pos, city.pos) <= ONE_WAR_FINISH_REACH
                })
        })
    }

    /// A front we outgun [`ONE_WAR_CRUSHED_RATIO`] times over. Peace there
    /// hands a beaten rival the turns to rebuild: on King
    /// `civvis-20260929T020236Z` the seat offered Norway peace at 812
    /// military against 36 to counter Mali's 61% culture reading; Norway
    /// accepted at turn 188, rebuilt to 1,308 military and more than tripled
    /// its visiting tourists, and won on culture at 206. Only an urgent
    /// clock is worth freeing the army from such a front.
    fn one_war_front_crushed(&self, g: &Game, pid: usize, other: usize) -> bool {
        g.military_power(pid) >= ONE_WAR_CRUSHED_RATIO * g.military_power(other).max(1.0)
    }

    /// A front we outgun [`ONE_WAR_WINNING_RATIO`] times over, or one of
    /// whose cities our siege train is reducing or taking, is being won
    /// whatever the recent exchange says. Live King civvis-20261001T033711Z:
    /// the first Reduce of the session, on unwalled Samarobriva, ended the
    /// same turn with "the last window was a rout" at 477 power against 169.
    /// civvis-20261001T022028Z offered Arabia the same peace at 1,461
    /// against 66, with Cairo its last city.
    fn one_war_still_winning(&self, g: &Game, pid: usize, other: usize) -> bool {
        g.military_power(pid) >= ONE_WAR_WINNING_RATIO * g.military_power(other).max(1.0)
            || self.sieges.iter().any(|(cid, siege)| {
                matches!(
                    siege.stage,
                    super::siege_train::SiegeStage::Reduce | super::siege_train::SiegeStage::Take
                ) && g.cities.get(cid).is_some_and(|city| city.owner == other)
            })
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
        if let Some(next) = self.domination_followup_target(g, pid, Some(other)) {
            let displaced = g
                .cities
                .values()
                .any(|city| city.is_capital && city.original_owner == other && city.owner == next);
            return Some(if displaced {
                OneWarPeace::CapitalElsewhere
            } else {
                OneWarPeace::CapitalSecured
            });
        }
        // Keep the current front until peace is actually accepted. A public
        // victory clock is a reason to offer peace, never proof that the old
        // enemy has stopped attacking or permission to erase its threat field.
        if self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && self.forced_target_player.is_none()
            && !g.emergency_war_pair(pid, other)
            && !self.urgent_victory_threat(g, other)
            && self
                .actionable_victory_denial(g, pid)
                .is_some_and(|(rival, counter)| {
                    rival != other
                        && counter == GrandStrategy::Conquest
                        && self.domination_counter_target(g, pid, rival)
                        && (self.urgent_victory_threat(g, rival)
                            || !self.one_war_front_crushed(g, pid, other))
                })
            && !self.one_war_capture_at_hand(g, pid, other)
        {
            return Some(OneWarPeace::VictoryThreat);
        }
        if self.one_war_still_winning(g, pid, other) {
            return None;
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
    /// fatigue clause. A breached city with a nearby capture body remains a
    /// finishable prize even if the recent exchange has turned against us.
    pub(crate) fn one_war_presses(&self, g: &Game, pid: usize, other: usize) -> bool {
        let Some(front) = self.one_war.as_ref().filter(|_| self.one_war_at_a_time) else {
            return false;
        };
        front.target == other
            && g.is_at_war(pid, other)
            && self
                .domination_followup_target(g, pid, Some(other))
                .is_none()
            && (self.one_war_capture_at_hand(g, pid, other)
                || (front.tide_against_since.is_none() && self.one_war_prizes_in_reach(g, pid)))
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

#[cfg(test)]
mod capital_handoff_tests;

#[cfg(test)]
mod urgent_denial_tests;
