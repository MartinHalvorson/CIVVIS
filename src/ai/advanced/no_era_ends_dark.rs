//! `no-era-ends-dark`: an Era Score budget from each era's first turn. The
//! seat takes the Dedication whose Normal-Age clause pays most for what this
//! era can actually build, and schedules the clause's cheapest building early
//! enough to bank before the era turns.
//!
//! ## What the live games say
//!
//! Deity Gran Colombia, 4-player Tiny Pangaea, Online (eras end at t30, 50,
//! 70, 90, 110): the 38 games `civvis-20261112T100013Z` to `T171603Z`.
//!
//! - Era Score earned per era, median: Ancient 9, Classical 19, Medieval 11,
//!   Renaissance 12, Industrial 8, against 8 / 12 / 14 / 11 / 11 needed for
//!   a Normal Age. Dark at the era's end: 17 / 2 / 23 / 18 / 18 of 38 / 38 /
//!   37 / 35 / 31.
//! - Medieval entered from a Classical Golden Age (12 games) ended Dark in all
//!   12: a Golden Age's Dedication banks nothing. Entered from a Normal Age (23)
//!   it ended Dark in 11, short by 1, 2, 2, 2, 3, 4, 5, 5, 7, 7, 7.
//! - What the Dedication's own clause paid, per era: Classical 3.3, Medieval
//!   2.2, Renaissance 2.5, Industrial 0.2 (Heartbeat of Steam, while the seat
//!   built Walls, Universities, Granaries, Monuments and Libraries: 4 of 290
//!   buildings in the Industrial era were Industrial). The choice itself is
//!   right in four eras of five on the counts the seat produced; what is
//!   missing is the buildings the clause pays for. At the Medieval era's start
//!   the seat held 2.5 to 2.9 cities of two citizens or more with no Monument
//!   and built 0.1 to 0.4 in the era.
//! - The end-of-era closers buy nothing: `age-closer-2` wrote "No Great Person
//!   closes the era" about 1,500 times over these games (the cheapest race
//!   450 to 2,250 Faith against 200 to 320 banked), and camps stop scoring
//!   after the Medieval era.
//! - The cost is cities. Of 34 loyalty flips to Free Cities today, 27 came in
//!   our Dark Age, 28 with a Governor already seated and 25 with no Monument.
//!   Our citizens' pressure at a Normal Age (`Expansion1_Leaders.xml`
//!   `DARK_AGE_CITY_IDENTITY` -0.5 a citizen; `Game::loyalty_change_for_city`)
//!   would have added a median 6.2 Loyalty a turn at the turn before the
//!   flip: every one of the 27 outlasts its flip turn, and 10 stop declining.
//!
//! ## What the gene does
//!
//! While the seat's age banks Dedication score (Normal or Dark; Georgia in
//! any age):
//!
//! 1. **Budget** ([`AdvancedAi::era_budget`]). The target is the Normal
//!    threshold plus [`ERA_BUDGET_MARGIN`]. The era's end is the host's
//!    countdown, else the Online schedule. The usual pace
//!    ([`ERA_PASSIVE_PER_ERA`] a whole era) and the paying buildings already
//!    at a queue head that finish in time are counted first; what is left is
//!    the shortfall.
//! 2. **Dedication** ([`AdvancedAi::choose_era_dedication`]). Each offered
//!    Dedication is worth last era's measured record of its triggers (the
//!    engine's own `projected_dedication_score`) plus the buildings of its
//!    clause the budget can schedule this era (at most
//!    [`ERA_MOMENT_PLAN_CAP`] a trigger, and no more than the era is short:
//!    on course, the record alone ranks). Ties keep the `Banking` order.
//! 3. **Moments** ([`AdvancedAi::claim_era_moments`]). While short, one city a
//!    turn puts the cheapest building the active clause pays for (a Monument
//!    under Pen, Brush and Voice; a Library under Free Inquiry; an Industrial
//!    building under Heartbeat of Steam) at the head of its queue.
//!
//! **What a scheduled moment may cost, and the cap.** The building must
//! finish within [`ERA_MOMENT_MAX_STANDARD`] standard turns (8 at Online) and
//! before the era ends: a Monument is 30 Production at Online, a Library 45.
//! At most [`ERA_MOMENT_MAX_OPEN`] paying buildings stand at queue heads at
//! once, and one city is claimed a turn. It goes only ahead of an idle queue,
//! a Granary, an Aqueduct or, at peace with nothing pressing the city, a
//! routine land soldier (`domination_monument_passes`), so a Settler, a
//! Builder, a Campus or Library, Walls, a wonder, a siege gun and saved work
//! near done keep their queue; never in the plan's threatened city, a city
//! attacked lately, under the early Settler floor, outside the rear under the
//! war's hold, or once the endgame army's clock runs.
//!
//! In a Golden or Heroic Age nothing is scheduled: the clause pays nothing.
//! The age closers (`age-closer-2`, `camps-close-the-age-3`) stay the
//! last-turns backstop.
//!
//! ## `score-seat-banks-era-score`: Era Score is score
//!
//! On a Score-target seat (`VictoryTarget::Score`) Era Score counts 1:1 in
//! `Game::score_parts`, so the second gene drops the Normal-Age target and
//! buys every moment worth its price, all game. Live Deity Rome, 6-player
//! Continents, Online, t170 limit (`civvis-20261112T190022Z`, `T191211Z`):
//! our Era Score 86 and 151 at t171 against the winners' 269 and 275.
//!
//! 1. **Clause buildings, all game.** The scheduler runs whether or not the
//!    era is short. A clause building is worth its Era Score plus the one
//!    building point it adds; it is taken when it costs at most
//!    [`SCORE_PRODUCTION_PER_POINT`] Production a point (a Library at 45
//!    Production for 2 points, an Amphitheater at 75), with
//!    [`SCORE_MOMENT_MAX_OPEN`] at queue heads at once. The Dedication is
//!    ranked on its record plus everything schedulable (no shortfall cap).
//! 2. **Great People.** `advanced_great_people` lifts the closeness limit
//!    for both currencies and ranks the offers by price per point (5 score,
//!    plus 1 Era Score, or 3 when the patronage pays more than half), cheapest
//!    first. The Faith reserve is waived for the last
//!    [`SCORE_FINAL_STANDARD`] standard turns, so the game does not end on
//!    Faith that buys one. Gold keeps its reserve.
//! 3. **Trading Posts.** A Trader with a free origin opens a route to a
//!    rival major in which we hold no Trading Post (`trading_post_in:` on the
//!    model; no current route of ours there on a mirrored board), best route
//!    value first: `MOMENT_TRADING_POST_CONSTRUCTED_IN_OTHER_CIV`, +1 a
//!    civilization and +3 for every one.
//!
//! A Golden Age pays no clause, so there the scheduler takes nothing; the
//! Great People and the Trading Posts still score (their Moments are not
//! Dedications). Grep the journal for `score-seat-banks-era-score:`.
//!
//! Off, every entry point returns before reading the board. Grep the journal
//! for `no-era-ends-dark:`. Tests: `no_era_ends_dark/tests.rs`.

use super::*;

/// Era Score the budget aims above the Normal threshold.
pub(super) const ERA_BUDGET_MARGIN: i64 = 2;
/// Era Score a whole era brings without this gene's moments: the live
/// Medieval-to-Industrial medians (11, 12, 8) less what the clause paid.
pub(super) const ERA_PASSIVE_PER_ERA: f64 = 9.0;
/// Most paying buildings at queue heads at once, the plan's own included.
pub(super) const ERA_MOMENT_MAX_OPEN: usize = 2;
/// Longest a scheduled building may take, in standard turns (8 at Online).
pub(super) const ERA_MOMENT_MAX_STANDARD: u32 = 12;
/// Most buildings of one trigger the Dedication choice counts on scheduling.
pub(super) const ERA_MOMENT_PLAN_CAP: i64 = 3;
/// Standard turns of the first era (Online closes it at t30) and of every
/// later one (20 turns at Online); see `unique_unit_closes_the_age.rs`.
const FIRST_ERA_STANDARD: u32 = 45;
const LATER_ERA_STANDARD: u32 = 30;
/// The world era from which a building pays Heartbeat of Steam
/// (`Game::note_dedicated_building`).
const INDUSTRIAL_ERA: usize = 4;
/// The building-shaped Dedication triggers the budget can schedule.
const SCHEDULED_TRIGGERS: [&str; 3] = [
    "culture_building",
    "science_building",
    "industrial_building",
];
/// Turns of the era between two budget lines in the journal.
const BUDGET_JOURNAL_EVERY: u32 = 5;
/// `score-seat-banks-era-score`: the most Production one score point may
/// cost when the scheduler buys it (an Era Score point or a building point).
pub(super) const SCORE_PRODUCTION_PER_POINT: f64 = 40.0;
/// `score-seat-banks-era-score`: paying buildings at queue heads at once.
pub(super) const SCORE_MOMENT_MAX_OPEN: usize = 3;
/// `score-seat-banks-era-score`: standard turns before the turn limit from
/// which the Faith reserve is spent on Great People (10 at Online).
pub(super) const SCORE_FINAL_STANDARD: u32 = 15;
/// Score a recruited Great Person is worth (`Game::score_parts`).
const SCORE_PER_GREAT_PERSON: f64 = 5.0;

/// One era's Era Score budget. See [`AdvancedAi::era_budget`].
#[derive(Clone, Debug, PartialEq)]
pub(super) struct EraBudget {
    /// The turn the era opened (the host's or the Online schedule's).
    pub start: u32,
    /// The turn the next era opens.
    pub end: u32,
    pub turns_left: u32,
    /// Era Score earned since the era opened.
    pub earned: i64,
    /// The Normal threshold plus [`ERA_BUDGET_MARGIN`].
    pub target: i64,
    /// What the usual pace adds by the era's end.
    pub passive: f64,
    /// Era Score the paying buildings at queue heads bank before the end.
    pub in_hand: i64,
    /// Era Score still missing once both are counted; may be zero or less.
    pub short: i64,
    /// Whether this age pays Dedication score at all.
    pub banks: bool,
    /// `score-seat-banks-era-score` is reading it: no target, every moment
    /// worth its price.
    pub score: bool,
}

impl AdvancedAi {
    /// `score-seat-banks-era-score` is on and the seat plays for Score.
    pub(super) fn score_era_on(&self, g: &Game) -> bool {
        self.score_seat_banks_era_score
            && self.active_victory_target(g) == Some(VictoryTarget::Score)
    }

    /// The journal tag of whichever gene is reading the budget.
    fn era_tag(score: bool) -> &'static str {
        if score {
            "score-seat-banks-era-score"
        } else {
            "no-era-ends-dark"
        }
    }

    /// Whether a Dedication's Normal-Age half pays in the seat's age: a Normal
    /// or Dark Age, or Georgia's Strength in Unity in any age (the same rule as
    /// `policy_deck::dedication_banks`).
    fn era_dedication_banks(g: &Game, pid: usize) -> bool {
        !matches!(g.players[pid].age.as_str(), "golden" | "heroic")
            || g.civ_effect(pid, "golden_dedication_era_score") > 0.0
    }

    /// The turns the world's current era opened and will close: the host's
    /// countdown when it runs, else the Online schedule. A mirrored board
    /// carries no `world_era_since`, so its era start is read off the schedule.
    pub(super) fn era_window(g: &Game) -> (u32, u32) {
        let first = g.standard_duration(FIRST_ERA_STANDARD);
        let later = g.standard_duration(LATER_ERA_STANDARD);
        let length = if g.world_era == 0 { first } else { later };
        let mut start = if g.world_era == 0 || g.world_era_since > 0 {
            g.world_era_since
        } else {
            first.saturating_add(later.saturating_mul(g.world_era as u32 - 1))
        };
        let end = g
            .world_era_countdown_end
            .unwrap_or_else(|| start.saturating_add(length));
        // A world that reached this era early: the countdown is the fact, and
        // the scheduled start moves back a whole era length from it.
        if start >= end {
            start = end.saturating_sub(length);
        }
        (start, end.max(start.saturating_add(1)))
    }

    /// Era Score a finished `building` banks under the seat's active
    /// Dedications in its current age. Zero in an age that does not bank.
    fn era_building_pay(g: &Game, pid: usize, building: &Name) -> i64 {
        let Some(spec) = g.rules.buildings.get(building) else {
            return 0;
        };
        if spec.wonder || !Self::era_dedication_banks(g, pid) {
            return 0;
        }
        g.players[pid]
            .dedications
            .iter()
            .filter_map(|dedication| g.rules.dedications.get(dedication.as_str()))
            .flat_map(|spec| spec.triggers.iter())
            .filter(|(trigger, _)| Self::era_building_pays(g, building, trigger))
            .map(|(_, amount)| *amount)
            .sum()
    }

    /// Whether a finished `building` fires the Dedication `trigger`, read the
    /// way `Game::note_dedicated_building` fires it.
    fn era_building_pays(g: &Game, building: &Name, trigger: &str) -> bool {
        let Some(spec) = g.rules.buildings.get(building) else {
            return false;
        };
        if spec.wonder {
            return false;
        }
        match trigger {
            "culture_building" => spec.yields.culture > 0.0,
            "science_building" => spec.yields.science > 0.0,
            "industrial_building" => {
                let era = spec
                    .tech
                    .as_ref()
                    .and_then(|tech| g.rules.techs.get(tech))
                    .map(|tech| tech.era)
                    .or_else(|| {
                        spec.civic
                            .as_ref()
                            .and_then(|civic| g.rules.civics.get(civic))
                            .map(|civic| civic.era)
                    })
                    .unwrap_or(0);
                era >= INDUSTRIAL_ERA
            }
            _ => false,
        }
    }

    /// The era's Era Score budget. `None` with the gene off, for a minor, and
    /// in the last era (no next age to decide).
    pub(super) fn era_budget(&self, g: &Game, pid: usize) -> Option<EraBudget> {
        if !self.no_era_ends_dark && !self.score_seat_banks_era_score {
            return None;
        }
        let score = self.score_era_on(g);
        if !self.no_era_ends_dark && !score {
            return None;
        }
        let player = g.players.get(pid)?;
        if player.is_minor
            || player.is_barbarian
            || player.is_free_city
            || g.world_era + 1 >= crate::rules::ERA_NAMES.len()
        {
            return None;
        }
        let (start, end) = Self::era_window(g);
        let turns_left = end.saturating_sub(g.turn);
        let length = end.saturating_sub(start).max(1);
        let passive = ERA_PASSIVE_PER_ERA * f64::from(turns_left.min(length)) / f64::from(length);
        let banks = Self::era_dedication_banks(g, pid);
        let in_hand = if banks {
            g.player_city_ids(pid)
                .into_iter()
                .filter_map(|cid| {
                    let Some(Item::Building { building }) = g.cities[&cid].queue.first() else {
                        return None;
                    };
                    let pay = Self::era_building_pay(g, pid, building);
                    let item = Item::Building {
                        building: *building,
                    };
                    (pay > 0
                        && self.production_build_turns(g, pid, cid, &item).ceil()
                            < f64::from(turns_left))
                    .then_some(pay)
                })
                .sum()
        } else {
            0
        };
        let target = player.normal_age_threshold + ERA_BUDGET_MARGIN;
        let short =
            (target as f64 - player.era_score as f64 - passive - in_hand as f64).ceil() as i64;
        Some(EraBudget {
            start,
            end,
            turns_left,
            earned: player.era_score - player.era_score_baseline,
            target,
            passive,
            in_hand,
            short,
            banks,
            score,
        })
    }

    /// The cheapest building `cid` can raise for `trigger` (or, with `None`,
    /// for the seat's active clause) inside the cap and before the era's
    /// end: the building, its turns and the Era Score it banks under the
    /// active Dedications.
    fn era_moment_building(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        trigger: Option<&str>,
        turns_left: u32,
        score: bool,
    ) -> Option<(Item, f64, i64)> {
        let cap = f64::from(
            g.standard_duration(ERA_MOMENT_MAX_STANDARD)
                .min(turns_left.saturating_sub(1)),
        );
        let city = g.cities.get(&cid)?;
        g.rules
            .buildings
            .iter()
            .filter(|(name, spec)| {
                !spec.wonder
                    && !city.buildings.contains(*name)
                    && match trigger {
                        Some(trigger) => Self::era_building_pays(g, name, trigger),
                        None => Self::era_building_pay(g, pid, name) > 0,
                    }
            })
            .map(|(name, _)| *name)
            .filter(|building| {
                g.can_produce(
                    pid,
                    cid,
                    &Item::Building {
                        building: *building,
                    },
                )
            })
            .map(|building| {
                let item = Item::Building { building };
                let turns = self.production_build_turns(g, pid, cid, &item);
                (building, turns, Self::era_building_pay(g, pid, &building))
            })
            .filter(|(_, turns, _)| turns.is_finite() && turns.ceil() <= cap)
            // On a Score seat a building is worth its Era Score and its own
            // building point, and is bought at no more than the exchange rate.
            .filter(|(building, _, pay)| {
                !score || {
                    let item = Item::Building {
                        building: *building,
                    };
                    let points = (*pay).max(0) as f64 + 1.0;
                    g.item_remaining_cost_for_city(pid, cid, &item)
                        <= SCORE_PRODUCTION_PER_POINT * points
                }
            })
            .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
            .map(|(building, turns, pay)| (Item::Building { building }, turns, pay))
    }

    /// Whether `cid` is a city the budget may take: not the threatened city,
    /// not attacked lately, behind the front under the war's hold. With
    /// `army`, whether a soldier at its head may wait (at peace, nothing
    /// pressing it). `None` for a city the budget leaves alone.
    fn era_moment_city(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<bool> {
        let city = g.cities.get(&cid)?;
        let memory = super::hint_runs_the_claims::HINT_ATTACK_MEMORY;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= memory)
            || !self.claim_open_behind_the_front(cid)
        {
            return None;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && g.is_at_war(pid, other.id)
        });
        let pressed = self.hint_city_in_reach_of_war(g, pid, cid);
        (!(at_major_war && pressed)).then_some(!at_major_war && !pressed)
    }

    /// `no-era-ends-dark`: take this age's Dedications by what they pay for
    /// this era. Returns false, having done nothing, with the gene off, with
    /// nothing offered, and in an age that banks no Dedication score (the
    /// caller's own choice stands there).
    pub(super) fn choose_era_dedication(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if !(self.no_era_ends_dark || self.score_seat_banks_era_score)
            || pid >= g.players.len()
            || !Self::era_dedication_banks(g, pid)
            || g.available_dedications(pid).is_empty()
        {
            return false;
        }
        let score = self.score_era_on(g);
        if !self.no_era_ends_dark && !score {
            return false;
        }
        let (_, end) = Self::era_window(g);
        let turns_left = end.saturating_sub(g.turn);
        // The budget schedules no more than the era is short, so a clause is
        // credited no more buildings than that: on course, the record alone
        // ranks, exactly as `Banking` does.
        let short = if score {
            ERA_MOMENT_PLAN_CAP
        } else {
            self.era_budget(g, pid)
                .map_or(0, |budget| budget.short.max(0))
        };
        let cities: Vec<u32> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| self.era_moment_city(g, pid, *cid, plan).is_some())
            .collect();
        let age = g.players[pid].age.clone();
        loop {
            let offered = g.available_dedications(pid);
            if offered.is_empty() {
                return true;
            }
            let mut ranked: Vec<(Name, i64, i64)> = offered
                .into_iter()
                .map(|dedication| {
                    let tally = g.projected_dedication_score(pid, dedication.as_str());
                    let scheduled: i64 = g.rules.dedications[dedication.as_str()]
                        .triggers
                        .iter()
                        .filter(|(trigger, _)| SCHEDULED_TRIGGERS.contains(&trigger.as_str()))
                        .map(|(trigger, amount)| {
                            let open = cities
                                .iter()
                                .filter(|cid| {
                                    self.era_moment_building(
                                        g,
                                        pid,
                                        **cid,
                                        Some(trigger),
                                        turns_left,
                                        score,
                                    )
                                    .is_some()
                                })
                                .count() as i64;
                            amount * open.min(ERA_MOMENT_PLAN_CAP).min(short)
                        })
                        .sum();
                    (dedication, tally, scheduled)
                })
                .collect();
            ranked.sort_by(|a, b| {
                (b.1 + b.2)
                    .cmp(&(a.1 + a.2))
                    .then(b.1.cmp(&a.1))
                    .then(a.0.cmp(&b.0))
            });
            let runner_up = ranked
                .get(1)
                .map(|(name, tally, scheduled)| (plain(name.as_str()), tally + scheduled));
            let Some((chosen, tally, scheduled)) = ranked.into_iter().find(|(dedication, _, _)| {
                g.apply(
                    pid,
                    &Action::ChooseDedication {
                        dedication: *dedication,
                    },
                )
                .is_ok()
            }) else {
                return true;
            };
            let against = runner_up
                .map(|(name, value)| format!("{name} at {value}"))
                .unwrap_or_else(|| "nothing else offered".to_string());
            think!(self.journal(), Diplomacy, Decision,
                   "Dedicating the {age} age to {}", chosen.as_str();
                   "{}: expecting {} Era Score ({tally} on last era's record, \
                    {scheduled} the budget can schedule) against {against}",
                   Self::era_tag(score), tally + scheduled);
        }
    }

    /// `no-era-ends-dark`: while the era's budget is short, one city puts the
    /// cheapest building the active clause pays for at the head of its queue.
    /// `score-seat-banks-era-score`: on a Score seat, whenever such a building
    /// is worth its Production at [`SCORE_PRODUCTION_PER_POINT`], short or
    /// not. Exact no-op with both genes off.
    pub(super) fn claim_era_moments(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let Some(budget) = self.era_budget(g, pid) else {
            return;
        };
        let tag = Self::era_tag(budget.score);
        let era = crate::rules::ERA_NAMES
            .get(g.world_era)
            .copied()
            .unwrap_or("this");
        let journal_turn = g.turn == budget.start
            || g.turn.saturating_sub(budget.start) % BUDGET_JOURNAL_EVERY == 0;
        if !budget.banks {
            if g.turn == budget.start {
                think!(self.journal(), Economy, Detail,
                       "Era budget: the {era} age banks no Dedication score";
                       "{tag}: a Golden Age's Dedication pays its Golden half only; earned {} so \
                        far, Normal at {}", budget.earned, g.players[pid].normal_age_threshold);
            }
            return;
        }
        let on_course = budget.short <= 0 && !budget.score;
        if on_course || budget.turns_left <= 1 || self.endgame_army_clock(g, pid).is_some() {
            if journal_turn && on_course {
                think!(self.journal(), Economy, Detail,
                       "Era budget: the {era} era is on course for a Normal Age";
                       "{tag}: earned {}, {} turns left; the usual pace adds {:.1} and {} is under \
                        way against {} (Normal +{ERA_BUDGET_MARGIN})",
                       budget.earned, budget.turns_left, budget.passive, budget.in_hand,
                       budget.target);
            }
            return;
        }
        let open = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| match g.cities[cid].queue.first() {
                Some(Item::Building { building }) => Self::era_building_pay(g, pid, building) > 0,
                _ => false,
            })
            .count();
        let dedications = {
            let names: Vec<String> = g.players[pid]
                .dedications
                .iter()
                .map(|d| plain(d))
                .collect();
            if names.is_empty() {
                "no Dedication".to_string()
            } else {
                names.join(" and ")
            }
        };
        if journal_turn {
            if budget.score {
                think!(self.journal(), Economy, Detail,
                       "Era budget: every {era} moment worth its price";
                       "{tag}: Era Score is score; earned {} this era, {} turns left to t{}; \
                        {open} paying building(s) at a queue head under {dedications}, at most \
                        {SCORE_PRODUCTION_PER_POINT:.0} Production a point",
                       budget.earned, budget.turns_left, budget.end);
            } else {
                think!(self.journal(), Economy, Detail,
                       "Era budget: the {era} era is {} Era Score short of a Normal Age",
                       budget.short;
                       "{tag}: earned {}, {} turns left to t{}; the usual pace adds {:.1} and {} \
                        is under way against {} (Normal +{ERA_BUDGET_MARGIN}); {open} paying \
                        building(s) at a queue head under {dedications}",
                       budget.earned, budget.turns_left, budget.end, budget.passive,
                       budget.in_hand, budget.target);
            }
        }
        let max_open = if budget.score {
            SCORE_MOMENT_MAX_OPEN
        } else {
            ERA_MOMENT_MAX_OPEN
        };
        if open >= max_open {
            return;
        }
        let n_cities = g.player_city_ids(pid).len();
        let settlers = self.counts(g, pid).settlers;
        let mut candidates: Vec<(f64, u32, Item, i64, bool)> = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| {
                let army = self.era_moment_city(g, pid, cid, plan)?;
                let (item, turns, pay) =
                    self.era_moment_building(g, pid, cid, None, budget.turns_left, budget.score)?;
                Some((turns, cid, item, pay, army))
            })
            .collect();
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (turns, cid, item, pay, army) in candidates {
            if self.early_settler_floor_holds(g, pid, cid, plan) {
                continue;
            }
            let head = g.cities[&cid].queue.first().cloned();
            let passed = match &head {
                Some(head) if *head == item => continue,
                Some(head) => {
                    match self.domination_monument_passes(g, pid, cid, plan, head, army) {
                        Some(passed) => format!("{passed} (it keeps its progress)"),
                        None => continue,
                    }
                }
                None if self.base.settler_due(g, pid, cid, n_cities, settlers) => continue,
                None => "an idle queue".to_string(),
            };
            let name = g.cities[&cid].name.clone();
            let building = Self::plain_item(&item);
            let cost = g.item_remaining_cost_for_city(pid, cid, &item);
            if g.apply(pid, &Action::Produce { city: cid, item }).is_ok() {
                if budget.score {
                    think!(self.journal(), Cities, Decision,
                           "{name} builds its {building} for the score";
                           "{tag}: +{pay} Era Score under {dedications} and +1 building, \
                            {cost:.0} Production in {turns:.1} turns before the era ends at t{}; \
                            it goes ahead of {passed}", budget.end);
                } else {
                    think!(self.journal(), Cities, Decision,
                           "{name} builds its {building} for the age";
                           "{tag}: +{pay} Era Score under {dedications}, {turns:.1} turns before \
                            the era ends at t{}; it goes ahead of {passed}; the era is {} short",
                           budget.end, budget.short);
                }
                return;
            }
        }
    }

    /// `score-seat-banks-era-score`: Score a patronised Great Person adds:
    /// five points and its Moment (+3 Era Score when the currency pays more
    /// than half of it, `Game::recruit_great_person`, else +1).
    pub(super) fn score_patronage_points(g: &Game, pid: usize, kind: &str) -> f64 {
        let points = g.players[pid].gpp.get(kind).copied().unwrap_or(0.0);
        let cost = g.gp_cost(pid, kind);
        let moment = if (cost - points) > cost / 2.0 {
            3.0
        } else {
            1.0
        };
        SCORE_PER_GREAT_PERSON + moment
    }

    /// `score-seat-banks-era-score`: the last turns, in which the Faith
    /// reserve buys a Great Person instead of ending the game unspent.
    pub(super) fn score_final_turns(g: &Game) -> bool {
        g.max_turns > 0
            && g.turn
                .saturating_add(g.standard_duration(SCORE_FINAL_STANDARD))
                >= g.max_turns
    }

    /// `score-seat-banks-era-score`: the rival city a Trader in `origin`
    /// should open a route to so we hold a Trading Post in every rival
    /// civilization, with that route's value; `None` off, on another target,
    /// or once every reachable rival holds our post or a current route.
    pub(super) fn score_route_claim(
        &self,
        g: &Game,
        pid: usize,
        origin: u32,
        strategy: GrandStrategy,
    ) -> Option<(f64, u32)> {
        if !self.score_era_on(g) {
            return None;
        }
        let _memo = g.query_memo();
        let claim = g
            .cities
            .values()
            .filter(|city| {
                let owner = city.owner;
                let rival = &g.players[owner];
                owner != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !rival.is_free_city
                    && !g.same_team(pid, owner)
                    && !g.is_at_war(pid, owner)
                    && !g.players[pid]
                        .counters
                        .contains_key(&format!("trading_post_in:{owner}"))
                    && !g.has_tourism_trade_route(pid, owner)
            })
            .filter(|city| g.can_establish_trade_route(pid, origin, city.id))
            .map(|city| {
                (
                    self.trade_route_destination_value_from(g, pid, Some(origin), city, strategy),
                    city.id,
                )
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
            });
        if let Some((_, city)) = claim {
            let destination = &g.cities[&city];
            let civ = &g.players[destination.owner].civ;
            think!(self.journal(), Economy, Decision,
                   "A Trader opens a route to {}", destination.name;
                   "score-seat-banks-era-score: no Trading Post of ours in {civ} yet: +1 Era \
                    Score, and +3 once every civilization holds one");
        }
        claim
    }
}

#[cfg(test)]
mod tests;
