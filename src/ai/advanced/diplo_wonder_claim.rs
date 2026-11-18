//! `diplo-seat-wonder-claim`: a seat assigned the Diplomatic lane keeps one
//! Diplomatic Victory Point wonder in production in its strongest city.
//!
//! **Why.** The Statue of Liberty pays four of the twenty points, the
//! Mahabodhi Temple two, the Potala Palace one
//! (`Expansion1_Expansion2.xml` / `Expansion2_Buildings.xml`
//! `MODIFIER_PLAYER_ADJUST_DIPLOMATIC_VICTORY_POINTS`). Live King
//! civvis-20261030T074500Z (Diplomatic) stood at 18 points at t201 and lost to
//! a rival Culture victory at t207; the first live Diplomatic win,
//! 081846Z at t208, needed one point between sessions. Under the live
//! `lane-delegates-production-2` an assigned lane's routine queues belong to
//! the baseline governor, which builds a wonder only once a city holds every
//! building, and `strategic_wonder_value` -- the only place these points are
//! priced -- is never asked. -43's Culture wonder lane measured harm natively
//! for Culture wonders (forced extras displace compounding builds); a
//! Diplomatic point is worth far more per Production, so this is one lane,
//! in the top-production city, with no wonder-gap gate.
//!
//! **What the gene does.** For an assigned Diplomatic seat that has not lost
//! its lane, with no Diplomatic-point wonder already at the head of any of
//! its queues: the unthreatened city that can finish one in
//! [`DIPLO_WONDER_MAX_TURNS`] (the most points first, then the soonest
//! finish, then the most Production; [`diplo_wonder_ranks_before`]) claims it -- an idle queue, or one over a
//! head [`AdvancedAi::culture_theater_may_defer`] lets wait and that no other
//! claim holds. The strategic governor's rescoring leaves the claimed wonder
//! alone ([`AdvancedAi::diplo_wonder_holds`]).
//!
//! **`science-seat-builds-the-statue`** (2026-11-06) carries the claim to a seat
//! aiming at Science, for the Statue alone. Deity 11-05, 51 games: rivals block
//! Congress points from about 16, and points from outside the Congress step
//! over that wall. The Statue costs 620 Production on Online, about 11 turns of
//! the best city, and rivals rarely take it. See
//! `advanced/diplo_statue_harbor.rs` for the Harbor it needs.
//!
//! **`diplo-seat-statue-is-the-finisher`** (2026-11-13, reworked 2026-11-18):
//! the Statue is started at once, built to one turn from done, and lands
//! only when its four points end the game.
//!
//! *Why hold it.* Rivals vote two points away from a seat that enters a
//! Diplomatic Victory session on about fifteen or more. Over every session
//! on disk on 2026-11-13 (278, all lanes) nobody voted against us at
//! fourteen or fewer going in but for one stray ballot, and at 16-19 four or
//! five rivals did. Three Deity Diplomatic games of 2026-11-18 (6-player
//! Continents, Online): `civvis-20261118T072032Z` (Khmer) landed the Statue
//! at t149 on 8 points, stood on 15 at t155 and 17 at t174, was stripped to
//! 15 at the t175 session and lost to a Science victory at t198 on 18;
//! `055703Z` (Gaul) was stripped from 16 to 14 at t169 and lost on 18 at
//! t191; `061033Z` (Hungary) carried its own resolution at 13 going in and
//! won on 21 at t183, the turn its Statue landed. Four points banked in the
//! queue are four points the scoreboard does not show: Khmer's board would
//! have read 13 at that session.
//!
//! *Why start it at once.* The claim waits for a city that can finish in
//! [`DIPLO_WONDER_MAX_TURNS`]. Hungary's host offered the Statue at 33 turns
//! from t140 and the claim took it at t152; Gaul's offered it at 57 turns
//! from t159 (11 Production) and the claim took it at t182, nine turns before
//! the game ended, with 230 of 620 in. A wonder that is going to wait at its
//! last turn loses nothing by being slow, so a finisher is claimed whatever
//! its build time, as long as the game's own clock leaves room for it.
//!
//! With the tag on, for a seat whose ballot races (the Diplomatic lane, or a
//! Science seat under `science-seat-races-the-congress`), a wonder worth
//! [`DIPLO_FINISHER_POINTS`]:
//!
//! - is **claimed past the 30-turn bar** ([`AdvancedAi::diplo_wonder_item`]),
//!   in the city that finishes it soonest or one within
//!   [`DIPLO_FINISHER_START_SLACK`] of it;
//! - is built exactly as before until it is within
//!   [`DIPLO_FINISHER_HOLD_TURNS`] of done;
//! - is then **held** while its points would not reach twenty: the claim does
//!   not offer it, a head that is it is not kept
//!   ([`AdvancedAi::diplo_wonder_holds`]), and, as the turn's last production
//!   writer, [`AdvancedAi::hold_the_diplo_finisher`] replaces it at its
//!   queue's head with what the city's governor would start on an empty
//!   queue, so the city goes on building. The wonder keeps its progress, and
//!   no other city of ours is offered the same wonder meanwhile;
//! - is **finished** the turn [`AdvancedAi::diplo_finisher_release`] names a
//!   reason: our points and its own reach twenty, or the game is about to
//!   end anyway (a rival's expedition arrives, or the turn limit falls,
//!   within [`DIPLO_FINISHER_LAST_TURNS`]) and the points may as well stand
//!   on the last board. The same writer puts it back at the head, whatever
//!   stands there, and it lands on the next turn.
//!
//! (The board's `blocked_wonders` is not used for the hold: the engine reads
//! a blocked wonder as having no site in that city, and the writer that must
//! bring it back could no longer see it.)
//!
//! *What it does not do.* It does not finish early because a rival is
//! further along its space race than that: a launched Exoplanet Expedition
//! stood 17 and 19 turns from arrival in the Khmer and Gaul games, with a
//! session inside both windows, and four points put on the board there are
//! the points the next session votes away. And it cannot see the wonder
//! race. The tile export names a wonder from the turn its plot is placed and
//! marks it complete from that turn (our own Statue read so for twelve turns
//! before it landed), a rival's queue is not exported, and once a rival's
//! plot shows a wonder the engine reads it as built. A rival that completes
//! the Statue while ours waits takes the four points for good. In the 18
//! games on disk on 2026-11-18 no rival placed one on a plot we had seen
//! (ours was placed in six); the 2026-11-05 census read the same. No native
//! screen prices either side of the hold: the native Congress has no bloc.
//!
//! Off, or for a seat not assigned Diplomacy, nothing changes.

use super::*;

/// The slowest a claimed Diplomatic-point wonder may be, in turns at the
/// claiming city's Production. The Statue of Liberty (1,240) at Modern-era
/// Production of 45-60 is 21-28.
pub(super) const DIPLO_WONDER_MAX_TURNS: f64 = 30.0;

/// The fewest cities a seat holds before it claims one.
pub(super) const DIPLO_WONDER_MIN_CITIES: usize = 3;

/// The fewest points a wonder pays before a Science seat claims it
/// (`science-seat-builds-the-statue`): the Statue of Liberty's four.
pub(super) const SCIENCE_SEAT_WONDER_POINTS: f64 = 4.0;

/// `diplo-seat-statue-is-the-finisher`: the fewest points a wonder pays to be
/// held as the finishing move: the Statue of Liberty's four.
pub(super) const DIPLO_FINISHER_POINTS: f64 = 4.0;

/// `diplo-seat-statue-is-the-finisher`: a finisher this many turns or fewer
/// from done is held until its points win. One: it would land at the end of
/// this turn.
pub(super) const DIPLO_FINISHER_HOLD_TURNS: f64 = 1.0;

/// `diplo-seat-statue-is-the-finisher`: with a rival's expedition or the turn
/// limit this many turns or fewer away the game is ending anyway and a held
/// finisher is completed. Short on purpose: sessions come twenty turns apart
/// on Online, and a wider window would put the points on the board for the
/// session that votes them away.
pub(super) const DIPLO_FINISHER_LAST_TURNS: f64 = 3.0;

/// `diplo-seat-statue-is-the-finisher`: a finisher past the 30-turn bar is
/// started in a city only if it needs at most this many times the turns of
/// our quickest city for it. Live Gaul 2026-11-18, replayed at t181: with
/// Ratumacos (32 turns) finishing a Shipyard that turn, the claim placed the
/// Statue in Divodurum at 50.
pub(super) const DIPLO_FINISHER_START_SLACK: f64 = 1.25;

/// Whether a candidate `(points, turns, production, city)` claims ahead of
/// `other`: the most points, then the soonest finish, then the most
/// Production, then the lowest city id. The finish comes before the
/// Production because the point lands when the wonder does: live Immortal
/// civvis-20261030T190332Z claimed the Statue of Liberty in Argos at t141 (27
/// Production, 25 turns) and it landed about t175 as the seat lost to
/// Technology on 18 points.
pub(super) fn diplo_wonder_ranks_before(
    candidate: (f64, f64, f64, u32),
    other: (f64, f64, f64, u32),
) -> bool {
    let (points, turns, production, city) = candidate;
    let (o_points, o_turns, o_production, o_city) = other;
    (-points, turns, -production, city) < (-o_points, o_turns, -o_production, o_city)
}

/// Whether two queue items are the same wonder, whatever plot each names:
/// a wonder the live host is already building is carried on the mirrored
/// queue keyed by name, and the production menu offers it with its site.
fn same_wonder(first: &Item, second: &Item) -> bool {
    matches!((first, second),
        (Item::Wonder { wonder: first, .. }, Item::Wonder { wonder: second, .. }) if first == second)
}

/// `diplo-seat-statue-is-the-finisher`: whether a wonder that needs `turns`
/// in a city is started there, with our quickest city for it at `soonest`:
/// under the 30-turn bar always, past it only within
/// [`DIPLO_FINISHER_START_SLACK`] of the quickest.
pub(super) fn diplo_finisher_starts_here(turns: f64, soonest: Option<f64>) -> bool {
    turns <= DIPLO_WONDER_MAX_TURNS
        || soonest.is_none_or(|soonest| turns <= soonest * DIPLO_FINISHER_START_SLACK)
}

fn diplomatic_points(g: &Game, item: &Item) -> f64 {
    match item {
        Item::Wonder { wonder, .. } => g
            .rules
            .wonders
            .get(wonder)
            .and_then(|spec| spec.effects.get("diplomatic_victory_points").copied())
            .unwrap_or(0.0),
        _ => 0.0,
    }
}

impl AdvancedAi {
    /// Whether the gene acts for this seat this turn.
    fn diplo_wonder_claim_on(&self, g: &Game) -> bool {
        self.diplo_seat_wonder_claim
            && match self.active_victory_target(g) {
                Some(VictoryTarget::Diplomacy) => !self.lane_lost,
                // A lost Science race is when this seat most needs the points.
                Some(VictoryTarget::Science) => self.science_seat_builds_the_statue,
                _ => false,
            }
    }

    /// Whether this seat's claim takes a wonder worth `points`. A Science seat
    /// takes only the Statue of Liberty: a lesser wonder's points do not pay
    /// for the Production it takes from the science race.
    fn diplo_wonder_worth_it(&self, g: &Game, points: f64) -> bool {
        points > 0.0
            && (self.active_victory_target(g) != Some(VictoryTarget::Science)
                || points >= SCIENCE_SEAT_WONDER_POINTS)
    }

    /// The Diplomatic-point wonder `cid` would claim: its points and build
    /// turns. `None` with the gene off, for another target, a threatened or
    /// struck city, or nothing it can finish in [`DIPLO_WONDER_MAX_TURNS`].
    pub(super) fn diplo_wonder_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64, f64)> {
        // One memo for the whole sweep; see `Game::query_memo`.
        let _memo = g.query_memo();
        if !self.diplo_wonder_claim_on(g) {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || g.player_city_ids(pid).len() < DIPLO_WONDER_MIN_CITIES
        {
            return None;
        }
        let refused = g.blocked_wonders.get(&cid);
        // `diplo-seat-statue-is-the-finisher`: a finisher held at its last
        // turn is not offered, here or in another city of ours, and one not
        // yet that close is claimed past the 30-turn bar. Nothing is held and
        // the bar stands while the gene is off.
        let finisher_on = self.diplo_finisher_on(g);
        let held: Vec<Item> = if finisher_on {
            g.player_city_ids(pid)
                .into_iter()
                .filter_map(|other| self.diplo_finisher_held(g, pid, other))
                .collect()
        } else {
            Vec::new()
        };
        let clock = g
            .turn_limit()
            .map_or(f64::INFINITY, |limit| limit.saturating_sub(g.turn) as f64);
        let production = g.city_yields(cid).production.max(0.5);
        g.producible_items(pid, cid)
            .into_iter()
            .filter_map(|item| {
                let Item::Wonder { wonder, .. } = &item else {
                    return None;
                };
                if refused.is_some_and(|blocked| blocked.contains(wonder))
                    || g.host_unavailable_wonders.contains(wonder)
                {
                    return None;
                }
                if held.iter().any(|kept| same_wonder(kept, &item)) {
                    return None;
                }
                let points = diplomatic_points(g, &item);
                if !self.diplo_wonder_worth_it(g, points) {
                    return None;
                }
                let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                    let remaining = g.item_cost_for_city(pid, cid, &item)
                        - g.item_invested_production(cid, &item);
                    remaining.max(0.0) / production
                });
                let early = finisher_on && points >= DIPLO_FINISHER_POINTS && turns <= clock;
                (early || turns <= DIPLO_WONDER_MAX_TURNS).then_some((item, points, turns))
            })
            .max_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then_with(|| b.2.total_cmp(&a.2))
                    .then_with(|| format!("{:?}", b.0).cmp(&format!("{:?}", a.0)))
            })
    }

    /// Whether `item` at the head of `cid` is a Diplomatic-point wonder this
    /// gene would keep: still legal, in an unthreatened city of ours.
    pub(super) fn diplo_wonder_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !self.diplo_wonder_claim_on(g) || diplomatic_points(g, item) <= 0.0 {
            return false;
        }
        // `diplo-seat-statue-is-the-finisher`: a held finisher is not kept.
        if self
            .diplo_finisher_held(g, pid, cid)
            .is_some_and(|held| same_wonder(&held, item))
        {
            return false;
        }
        g.cities
            .get(&cid)
            .is_some_and(|city| city.owner == pid && plan.threatened_city != Some(cid))
            && g.can_produce(pid, cid, item)
    }

    /// `diplo-seat-statue-is-the-finisher`: whether the rule acts for this
    /// seat. The wonder claim must be the seat's and its ballot must race:
    /// the Diplomatic lane, or a Science seat under
    /// `science-seat-races-the-congress`.
    fn diplo_finisher_on(&self, g: &Game) -> bool {
        self.diplo_seat_statue_is_the_finisher
            && self.diplo_wonder_claim_on(g)
            && match self.active_victory_target(g) {
                Some(VictoryTarget::Diplomacy) => true,
                Some(VictoryTarget::Science) => self.science_seat_races_the_congress,
                _ => false,
            }
    }

    /// The finishing wonder `cid` could land within
    /// [`DIPLO_FINISHER_HOLD_TURNS`], and its points: a wonder it may produce
    /// that pays [`DIPLO_FINISHER_POINTS`] or more. `None` with the rule off.
    fn diplo_finisher_in_reach(&self, g: &Game, pid: usize, cid: u32) -> Option<(Item, f64)> {
        if !self.diplo_finisher_on(g) {
            return None;
        }
        // One memo for the whole sweep; see `Game::query_memo`.
        let _memo = g.query_memo();
        let city = g.cities.get(&cid)?;
        if city.owner != pid {
            return None;
        }
        let production = g.city_yields(cid).production.max(0.5);
        g.producible_items(pid, cid).into_iter().find_map(|item| {
            if !matches!(item, Item::Wonder { .. }) {
                return None;
            }
            let points = diplomatic_points(g, &item);
            if points < DIPLO_FINISHER_POINTS {
                return None;
            }
            let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                let remaining =
                    g.item_cost_for_city(pid, cid, &item) - g.item_invested_production(cid, &item);
                remaining.max(0.0) / production
            });
            (turns <= DIPLO_FINISHER_HOLD_TURNS).then_some((item, points))
        })
    }

    /// The fewest turns any city of ours needs for the wonder `item` names,
    /// whatever its queue holds this turn.
    fn diplo_finisher_soonest(&self, g: &Game, pid: usize, item: &Item) -> Option<f64> {
        g.player_city_ids(pid)
            .into_iter()
            .flat_map(|cid| {
                let production = g.city_yields(cid).production.max(0.5);
                g.producible_items(pid, cid)
                    .into_iter()
                    .filter(|offer| same_wonder(offer, item))
                    .map(move |offer| {
                        g.host_production_turns(cid, &offer).unwrap_or_else(|| {
                            let remaining = g.item_cost_for_city(pid, cid, &offer)
                                - g.item_invested_production(cid, &offer);
                            remaining.max(0.0) / production
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .min_by(f64::total_cmp)
    }

    /// Why a finisher worth `points` is completed now, if it is: our points
    /// and its own make twenty, or the game is about to end and the points
    /// go on the last board (a rival's Exoplanet Expedition arrives, by the
    /// host's own reading of its trip where the board carries one, or the
    /// turn limit falls, within [`DIPLO_FINISHER_LAST_TURNS`]). `None` is the
    /// hold. See the module for what is not read here.
    pub(super) fn diplo_finisher_release(
        &self,
        g: &Game,
        pid: usize,
        points: f64,
    ) -> Option<&'static str> {
        let have = g.players[pid].dvp as f64;
        if have + points >= crate::game::DIPLOMATIC_VICTORY_POINTS as f64 {
            return Some("its points make twenty");
        }
        if g.turn_limit()
            .is_some_and(|limit| limit.saturating_sub(g.turn) as f64 <= DIPLO_FINISHER_LAST_TURNS)
        {
            return Some("the turn limit is here");
        }
        let arriving = g.players.iter().any(|rival| {
            if rival.id == pid
                || !rival.alive
                || rival.is_minor
                || rival.is_barbarian
                || g.same_team(pid, rival.id)
            {
                return false;
            }
            let speed = g.exoplanet_speed(rival.id);
            speed > 0.0
                && (g.science_victory_points_needed(rival.id) - g.science_victory_points(rival.id))
                    .max(0.0)
                    / speed
                    <= DIPLO_FINISHER_LAST_TURNS
        });
        arriving.then_some("a rival's expedition is arriving")
    }

    /// The finishing wonder `cid` holds back: in reach, with no reason to
    /// complete it ([`AdvancedAi::diplo_finisher_release`]). `None` with the
    /// rule off, with nothing in reach, or once it is to be finished.
    pub(super) fn diplo_finisher_held(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let (item, points) = self.diplo_finisher_in_reach(g, pid, cid)?;
        self.diplo_finisher_release(g, pid, points)
            .is_none()
            .then_some(item)
    }

    /// What a city builds in a held finisher's place when its governor names
    /// nothing: the cheapest building, else project, else unit it may
    /// produce.
    fn diplo_finisher_stand_in(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let rank = |item: &Item| match item {
            Item::Building { .. } => 0,
            Item::Project { .. } => 1,
            Item::Unit { .. } => 2,
            _ => 3,
        };
        g.producible_items(pid, cid)
            .into_iter()
            .filter(|item| rank(item) < 3)
            .min_by(|a, b| {
                rank(a)
                    .cmp(&rank(b))
                    .then_with(|| {
                        g.item_cost_for_city(pid, cid, a)
                            .total_cmp(&g.item_cost_for_city(pid, cid, b))
                    })
                    .then_with(|| format!("{a:?}").cmp(&format!("{b:?}")))
            })
    }

    /// `diplo-seat-statue-is-the-finisher`, the turn's last production
    /// writer. A city whose queue head is a held finisher builds what its
    /// governor would start on an empty queue instead (the wonder keeps its
    /// progress); a city with a finisher in reach whose points now win puts
    /// it at the head whatever stands there. Exact no-op with the rule off.
    pub(super) fn hold_the_diplo_finisher(&mut self, g: &mut Game, pid: usize) {
        if !self.diplo_finisher_on(g) {
            return;
        }
        let have = g.players[pid].dvp;
        for cid in g.player_city_ids(pid) {
            let Some((finisher, points)) = self.diplo_finisher_in_reach(g, pid, cid) else {
                continue;
            };
            let at_head = g.cities[&cid]
                .queue
                .first()
                .is_some_and(|head| same_wonder(head, &finisher));
            let name = g.cities[&cid].name.clone();
            if let Some(reason) = self.diplo_finisher_release(g, pid, points) {
                let produce = Action::Produce {
                    city: cid,
                    item: finisher.clone(),
                };
                if !at_head && g.apply(pid, &produce).is_ok() {
                    think!(self.journal(), Cities, Decision,
                           "{name} finishes the {}", Self::plain_item(&finisher);
                           "diplo-seat-statue-is-the-finisher: {reason}; we hold {have} of \
                            twenty and it pays {points:.0}");
                }
                continue;
            }
            if !at_head {
                continue;
            }
            let Some(next) = self
                .preview_live_production(g, pid, cid)
                .filter(|item| !matches!(item, Item::Wonder { .. }))
                .or_else(|| Self::diplo_finisher_stand_in(g, pid, cid))
            else {
                continue;
            };
            let produce = Action::Produce {
                city: cid,
                item: next.clone(),
            };
            if g.apply(pid, &produce).is_ok() {
                think!(self.journal(), Cities, Decision,
                       "{name} holds the {} at its last turn", Self::plain_item(&finisher);
                       "diplo-seat-statue-is-the-finisher: {have} of twenty points, and its \
                        {points:.0} would not win; rivals vote two away from a seat that enters \
                        a session on about fifteen or more, so it lands only when it ends the \
                        game. {} is built meanwhile (the wonder keeps its progress)",
                       Self::plain_item(&next));
            }
        }
    }

    /// See the module: one Diplomatic-point wonder in production, in the
    /// strongest city that can finish one. Exact no-op with the gene off or
    /// for another target.
    pub(super) fn claim_diplo_wonder(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.diplo_wonder_claim_on(g) {
            return;
        }
        let city_ids = g.player_city_ids(pid);
        if city_ids.iter().any(|cid| {
            g.cities[cid]
                .queue
                .first()
                .is_some_and(|head| diplomatic_points(g, head) > 0.0)
        }) {
            return;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        });
        let mut best: Option<(f64, f64, f64, u32, Item)> = None;
        for cid in city_ids {
            let Some((item, points, turns)) = self.diplo_wonder_item(g, pid, cid, plan) else {
                continue;
            };
            if let Some(head) = g.cities[&cid].queue.first() {
                if self.early_settler_floor_holds(g, pid, cid, plan)
                    || self.bound_granary_holds(g, pid, cid, plan, head)
                    || self.capital_monument_holds(g, pid, cid, plan, head)
                    || self.deterrence_unit_holds(g, pid, head)
                    || self.idle_route_trader_holds(g, pid, cid, plan, head)
                    || self.backlog_builder_holds(g, pid, cid, plan, head)
                    || self.capped_aqueduct_holds(g, pid, cid, plan, head)
                    || Self::queued_wall_breaker(g, head)
                    || !Self::culture_theater_may_defer(g, pid, cid, head, at_major_war)
                {
                    continue;
                }
            }
            let production = g.city_yields(cid).production;
            if best
                .as_ref()
                .is_none_or(|(old_points, old_production, old_turns, old_city, _)| {
                    diplo_wonder_ranks_before(
                        (points, turns, production, cid),
                        (*old_points, *old_turns, *old_production, *old_city),
                    )
                })
            {
                best = Some((points, production, turns, cid, item));
            }
        }
        let Some((points, production, turns, cid, item)) = best else {
            return;
        };
        // `diplo-seat-statue-is-the-finisher`: past the 30-turn bar a wonder
        // is started only where it finishes soonest. A city whose head must
        // not wait this turn is passed over above, and the plot a slower city
        // places is the one the wonder is then built on.
        if turns > DIPLO_WONDER_MAX_TURNS
            && !diplo_finisher_starts_here(turns, self.diplo_finisher_soonest(g, pid, &item))
        {
            return;
        }
        let head = g.cities[&cid].queue.first().cloned();
        let name = g.cities[&cid].name.clone();
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Cities, Decision,
                   "{name} claims the {} for the Diplomatic victory", Self::plain_item(&item);
                   "diplo-seat-wonder-claim: {points:.0} of the twenty points, \
                    {production:.0} Production, {turns:.1} turns, ahead of {}",
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
