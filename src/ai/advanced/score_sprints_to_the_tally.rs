//! `score-seat-sprints-to-the-tally` (opt-in): in a Score seat's last twenty
//! Online turns every queue and every coin goes to what the tally pays for.
//!
//! ## Why (live Emperor Rome, 6 players, Continents, Online, turn limit 180)
//!
//! `civvis-20261116T005841Z` led at turn 170 (1,043 against Byzantium's
//! 1,037) and lost the turn-181 tally 1,086 to 1,136: over the last eleven
//! turns we gained 43 points and Byzantium 99. The state rows of turns
//! 160-180 (the last row of each turn) show where the end went:
//!
//! - **Production on what scores nothing.** 8,597 Production in all, 2,390
//!   of it (28%) on projects: Campus Research Grants 585, Bread and Circuses
//!   569, the Manhattan Project 516, Encampment Training 306, reactor
//!   conversions 414. A Spaceport that never finished took 621 more, soldiers
//!   712 (no war with any major) and Spies 344. A building is one point
//!   (`Game::score_parts`), a district two; Neighborhoods at 199, Museums and
//!   Shipyards at 145 stood on those cities' menus.
//! - **A bank that bought what scores nothing.** Gold opened every turn at
//!   612-1,363 (+238 to +591 a turn); three Traders and five upgrades took
//!   some of it and 566 stood at the tally. Faith opened at 214-1,325 and 749
//!   stood at the tally while the host offered an Admiral (five points) at
//!   630.
//!
//! ## What the gene does
//!
//! Only on a seat whose assigned target is Score, from
//! [`TALLY_SPRINT_STANDARD_TURNS`] standard turns before the turn limit (20
//! at Online: turn 160 of 180) to the limit; an exact no-op off, on every
//! other target and before the window.
//!
//! 1. **The queues** ([`AdvancedAi::claim_score_sprint_queues`]). Each city's
//!    head is the item with the most tally points per Production still owed
//!    that it can finish before the tally ([`AdvancedAi::score_sprint_offer`]):
//!    a building one point, a district two (four if unique) plus one for its
//!    cheapest building when both fit, a wonder fifteen plus its Era Score
//!    moment times the chance to finish first (`score-seat-raises-wonders`'
//!    reading), a Settler five when it can found before the tally on a site
//!    within [`TALLY_SPRINT_SETTLER_REACH`] tiles. Nothing else scores: no
//!    project of any kind, no Spaceport, no Spy, no Builder, no soldier. A
//!    head that scores keeps its place unless the best offer pays
//!    [`TALLY_SPRINT_KEEP_SLACK`] times as much a Production; points per
//!    Production *still owed* is what makes paid progress count. A city with
//!    nothing that scores in time keeps its queue. A threatened city, one
//!    attacked within [`TALLY_SPRINT_ATTACK_MEMORY`] turns, a repair and,
//!    while a major is at war with us, a unit at the head are left alone;
//!    the defence handoffs run after this claim and keep the last word.
//! 2. **The bank** ([`AdvancedAi::score_sprint_purchases`]). Every turn,
//!    before the stock patronage and purchase passes but after the defence
//!    purchases, Faith above [`TALLY_SPRINT_FAITH_RESERVE`] and then Gold
//!    above [`TALLY_SPRINT_GOLD_RESERVE`] (never below the turns left of a
//!    deficit, nor the reserve the treasury genes hold) buy the offer with
//!    the fewest coins a point: a building the host sells (one point; a building
//!    with Production already in it is left to its queue), a district (two,
//!    four if unique) or a Great Person's patronage (five). When the best
//!    offer is out of reach now but the income reaches it before the tally,
//!    the bank only buys what still leaves it reachable.
//!
//! Journal reasons start `score-seat-sprints-to-the-tally:` and state the
//! points, the cost and the turns.
//!
//! ## Version 2: `score-seat-sprints-to-the-tally-2` (opt-in; turns version 1 off)
//!
//! Live Immortal Rome `civvis-20261116T102757Z` (v1 armed, turn limit 180)
//! lost the tally 1,196 to 1,242: 53 of the gap was Era Score and 120 the
//! districts, buildings, Great People and religion. Era Score counts one
//! point a point at the tally (`Game::score_parts`), and v1 priced only the
//! wonders' moment. Its Gold saved turns 160-162 for a 2,510-Gold Merchant
//! priced at five points, which Faith bought at turn 172 for 790; three of
//! the seven classes on offer were exhausted by turn 174 and all of them by
//! the tally. Version 2 is version 1 plus three things:
//!
//! 1. **A Great Person's Historic Moment in its price**
//!    ([`AdvancedAi::score_sprint_patronage_era`]): every recruitment pays
//!    one (`MOMENT_GREAT_PERSON_CREATED_GAME_ERA` / `_PAST_ERA`, Era Score 1
//!    each), and a patronage whose coins cover more than half the person's
//!    points pays `MOMENT_GREAT_PERSON_CREATED_PATRONAGE_GOLD_OVER_HALF` /
//!    `_FAITH_OVER_HALF` (3) in its place (`Expansion1_Moments.xml`; neither
//!    row has an era window or a cooldown, and the host fired the Faith row
//!    instead of the era row at turn 89 of `civvis-20261116T034312Z`). A
//!    wonder keeps version 1's 15 plus 4 (its own era) or 3. The
//!    high-adjacency district rows are not priced: each is "your
//!    civilization's first" of its type (the 102757Z board held each once),
//!    so none repeats at the end of a game; a National Park is no purchase.
//! 2. **One currency a Great Person** ([`AdvancedAi::score_sprint_patrons`]):
//!    each class goes to the currency that reaches its price before the
//!    tally and gives up the fewest points of its other purchases — none
//!    for an idle bank, which Faith is wherever it buys no building — then
//!    the one that pays it sooner, then the smaller share of that bank. The
//!    other currency neither buys nor saves for it.
//! 3. **The Great People open first**: the purchase pass opens
//!    [`TALLY_SPRINT_GREAT_PERSON_STANDARD_TURNS`] standard turns before the
//!    limit (30 at Online: turn 150 of 180) and buys only patronage until
//!    version 1's window opens: a class whose last person a rival claims is
//!    gone. The queues keep version 1's window.
//!
//! Its journal reasons start `score-seat-sprints-to-the-tally-2:` and give
//! the Era Score part of each price.
//!
//! **What the replay reads.** Both versions' bank passes over turns 150-180
//! of 102757Z and `civvis-20261116T005841Z`, on each turn's own menus,
//! patron prices, points and income: 32 against 32 and 46 against 45 points
//! when every coin above the reserve is the sprint's, 25 against 25 and 15
//! against 15 on the live banks (the version 1 arm reproduces 102757Z's
//! thirteen buildings and Merchant). The over-half moment never paid: a
//! class more than half short cost 1,820-7,790 Faith or 2,705-11,660 Gold
//! there (230-970 Faith a point with the moment), and neither arm ever chose
//! one.

use super::*;
use crate::shared_sort::SharedSort;

/// The window, in standard turns before the turn limit: 30 standard is 20
/// at Online (`GameSpeed::scale_turns`), turn 160 of a 180-turn game.
pub(crate) const TALLY_SPRINT_STANDARD_TURNS: u32 = 30;

/// `score-seat-sprints-to-the-tally-2`: the Great Person purchases open this
/// many standard turns before the turn limit, 30 at Online (turn 150 of 180).
pub(crate) const TALLY_SPRINT_GREAT_PERSON_STANDARD_TURNS: u32 = 45;

/// Historic Moments a patronized Great Person pays (`Expansion1_Moments.xml`).
const MOMENT_PATRONAGE_GOLD_OVER_HALF: &str =
    "MOMENT_GREAT_PERSON_CREATED_PATRONAGE_GOLD_OVER_HALF";
const MOMENT_PATRONAGE_FAITH_OVER_HALF: &str =
    "MOMENT_GREAT_PERSON_CREATED_PATRONAGE_FAITH_OVER_HALF";
const MOMENT_GREAT_PERSON_GAME_ERA: &str = "MOMENT_GREAT_PERSON_CREATED_GAME_ERA";
const MOMENT_GREAT_PERSON_PAST_ERA: &str = "MOMENT_GREAT_PERSON_CREATED_PAST_ERA";

/// Gold the purchase pass leaves in the bank: one emergency defender's
/// worth of the 2026-11-16 game's late prices was 330-540.
pub(crate) const TALLY_SPRINT_GOLD_RESERVE: f64 = 150.0;

/// Faith the purchase pass leaves in the bank.
pub(crate) const TALLY_SPRINT_FAITH_RESERVE: f64 = 50.0;

/// A head that scores keeps its place unless the best offer pays this many
/// times its points per Production: a switch that gains less churns.
pub(crate) const TALLY_SPRINT_KEEP_SLACK: f64 = 1.25;

/// A district's own points; a unique one is worth twice this.
pub(crate) const TALLY_SPRINT_DISTRICT_POINTS: f64 = 2.0;

/// A wonder's points before its Era Score moment.
pub(crate) const TALLY_SPRINT_WONDER_POINTS: f64 = 15.0;

/// A Settler founds a city (five points); its first citizen replaces the
/// one it took from its city.
pub(crate) const TALLY_SPRINT_SETTLER_POINTS: f64 = 5.0;

/// A Settler pays only with a site this many tiles from its city.
pub(crate) const TALLY_SPRINT_SETTLER_REACH: i32 = 8;

/// A Great Person is five points on the tally.
pub(crate) const TALLY_SPRINT_GREAT_PERSON_POINTS: f64 = 5.0;

/// A city attacked this many turns ago or fewer belongs to the defence.
pub(crate) const TALLY_SPRINT_ATTACK_MEMORY: u32 = 3;

/// At most this many purchases per currency a turn.
const TALLY_SPRINT_MAX_BUYS: usize = 16;

/// What an item pays at the tally: its points (`era` of them Era Score), the
/// Production it still owes (with the follow-up building for a district) and
/// the turns to finish.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SprintOffer {
    pub(crate) item: Item,
    pub(crate) points: f64,
    pub(crate) era: f64,
    pub(crate) cost: f64,
    pub(crate) turns: f64,
}

impl SprintOffer {
    /// Points per Production still owed.
    pub(crate) fn rate(&self) -> f64 {
        self.points / self.cost.max(1.0)
    }
}

/// A purchase the bank may make: the action, its points (`era` of them Era
/// Score) and its price.
#[derive(Clone, Debug)]
pub(crate) struct SprintBuy {
    pub(crate) action: Action,
    pub(crate) label: String,
    pub(crate) points: f64,
    pub(crate) era: f64,
    pub(crate) price: f64,
}

impl SprintBuy {
    /// Coins a point.
    fn per_point(&self) -> f64 {
        self.price / self.points.max(f64::EPSILON)
    }
}

impl AdvancedAi {
    /// A version of the gene is on and the seat aims at Score.
    pub(super) fn score_sprint_on(&self) -> bool {
        (self.score_seat_sprints_to_the_tally || self.score_seat_sprints_to_the_tally_2)
            && self.victory_target == Some(VictoryTarget::Score)
    }

    /// The tag journal reasons start with: the version that plays.
    fn score_sprint_tag(&self) -> &'static str {
        if self.score_seat_sprints_to_the_tally_2 {
            "score-seat-sprints-to-the-tally-2"
        } else {
            "score-seat-sprints-to-the-tally"
        }
    }

    /// Version 2 names the Era Score part of a price; version 1 says nothing.
    fn score_sprint_era_note(&self, era: f64) -> String {
        if self.score_seat_sprints_to_the_tally_2 {
            format!(" ({era:.1} of them Era Score)")
        } else {
            String::new()
        }
    }

    /// Turns left before the tally (this turn's own end of turn included)
    /// inside a window of `standard` standard turns; `None` off and outside.
    fn score_sprint_window(&self, g: &Game, standard: u32) -> Option<u32> {
        if !self.score_sprint_on() || g.max_turns == 0 || g.turn > g.max_turns {
            return None;
        }
        let left = g.max_turns - g.turn + 1;
        (left <= g.standard_duration(standard) + 1).then_some(left)
    }

    /// Turns of production left before the tally (this turn's own end of
    /// turn included) while the sprint is open; `None` off and outside the
    /// window.
    pub(super) fn score_sprint_left(&self, g: &Game) -> Option<u32> {
        self.score_sprint_window(g, TALLY_SPRINT_STANDARD_TURNS)
    }

    /// Turns left before the tally while the bank may patronize Great People:
    /// version 2 opens [`TALLY_SPRINT_GREAT_PERSON_STANDARD_TURNS`] before the
    /// limit, version 1 with the rest of the sprint.
    pub(super) fn score_sprint_great_person_left(&self, g: &Game) -> Option<u32> {
        if self.score_seat_sprints_to_the_tally_2 {
            self.score_sprint_window(g, TALLY_SPRINT_GREAT_PERSON_STANDARD_TURNS)
        } else {
            self.score_sprint_left(g)
        }
    }

    /// Era Score a Historic Moment pays in the world's era: its catalogue
    /// value, nothing outside its era window.
    fn score_sprint_moment(g: &Game, moment: &str) -> f64 {
        g.rules
            .historic_moments
            .get(moment)
            .filter(|spec| {
                spec.minimum_game_era.is_none_or(|era| g.world_era >= era)
                    && spec.maximum_game_era.is_none_or(|era| g.world_era <= era)
                    && spec.obsolete_era.is_none_or(|era| g.world_era < era)
            })
            .map_or(0.0, |spec| spec.era_score.max(0) as f64)
    }

    /// `score-seat-sprints-to-the-tally-2`: the Era Score `pid` gains when it
    /// patronizes the Great `kind` with `currency` now. The host pays one
    /// Historic Moment a recruitment: the patronage row (3) when the coins
    /// cover more than half the person's points (`claim_great_person`:
    /// missing points above half the cost), else the era row (1).
    pub(crate) fn score_sprint_patronage_era(
        g: &Game,
        pid: usize,
        kind: &str,
        currency: &str,
    ) -> f64 {
        let points = g.players[pid].gpp.get(kind).copied().unwrap_or(0.0);
        let cost = g.gp_cost(pid, kind);
        let moment = if (cost - points).max(0.0) > cost / 2.0 {
            if currency == "faith" {
                MOMENT_PATRONAGE_FAITH_OVER_HALF
            } else {
                MOMENT_PATRONAGE_GOLD_OVER_HALF
            }
        } else if g
            .current_great_person(kind)
            .is_some_and(|(_, spec)| spec.era < g.world_era)
        {
            MOMENT_GREAT_PERSON_PAST_ERA
        } else {
            MOMENT_GREAT_PERSON_GAME_ERA
        };
        Self::score_sprint_moment(g, moment)
    }

    /// Whether a major is at war with `pid`.
    fn score_sprint_at_war(g: &Game, pid: usize) -> bool {
        g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && g.is_at_war(pid, player.id)
        })
    }

    /// Production a turn in `cid`, never zero.
    fn score_sprint_production(g: &Game, cid: u32) -> f64 {
        g.city_yields(cid).production.max(0.5)
    }

    /// The cheapest first-tier building of `district`'s family that `pid`
    /// can build once it stands, and its Production cost in `cid`.
    fn score_sprint_follow_up(g: &Game, pid: usize, cid: u32, district: Name) -> Option<f64> {
        let family = g.district_family(district);
        let player = &g.players[pid];
        g.rules
            .buildings
            .iter()
            .filter(|(_, spec)| {
                spec.buildable
                    && !spec.wonder
                    && spec.requires.is_empty()
                    && spec.requires_any.is_empty()
                    && spec.replaces.is_none()
                    && spec.unique_to.is_none()
                    && spec
                        .district
                        .is_some_and(|home| g.district_family(home) == family)
                    && spec.tech.is_none_or(|tech| player.techs.contains(&tech))
                    && spec
                        .civic
                        .is_none_or(|civic| player.civics.contains(&civic))
            })
            .map(|(name, _)| g.item_cost_for_city(pid, cid, &Item::Building { building: *name }))
            .filter(|cost| cost.is_finite() && *cost > 0.0)
            .min_by(f64::total_cmp)
    }

    /// What `item` pays at the tally when `cid` builds it, with `left` turns
    /// of production before the tally: see the module, point 1. `None` when
    /// it scores nothing or cannot finish (a Settler: cannot found) in time.
    pub(crate) fn score_sprint_offer(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        left: u32,
    ) -> Option<SprintOffer> {
        let left = left as f64;
        let cost = g.item_remaining_cost_for_city(pid, cid, item);
        if !cost.is_finite() {
            return None;
        }
        let production = Self::score_sprint_production(g, cid);
        let turns = g
            .host_production_turns(cid, item)
            .filter(|turns| turns.is_finite() && *turns >= 0.0)
            .unwrap_or(cost / production)
            .max(1.0);
        if turns.ceil() > left {
            return None;
        }
        // A wonder: 15 points and its Era Score moment, both times the chance
        // of finishing first.
        let wonder = |name: &Name| {
            let era = Self::score_wonder_era(g, name);
            let moment = Self::score_wonder_moment(era, g.world_era) as f64;
            let chance = Self::score_wonder_chance(era, g.world_era, turns);
            (
                (TALLY_SPRINT_WONDER_POINTS + moment) * chance,
                moment * chance,
            )
        };
        let mut era = 0.0;
        let (points, cost) = match item {
            Item::Building { building } => match g.rules.buildings.get(building) {
                Some(spec) if spec.wonder => {
                    let (points, moment) = wonder(building);
                    era = moment;
                    (points, cost)
                }
                Some(_) => (1.0, cost),
                None => return None,
            },
            Item::Wonder { wonder: name, .. } => {
                let (points, moment) = wonder(name);
                era = moment;
                (points, cost)
            }
            Item::District { district, .. } => {
                if g.district_family(*district).as_str() == "spaceport" {
                    return None;
                }
                let own = if g
                    .rules
                    .districts
                    .get(district)
                    .is_some_and(|spec| spec.unique_to.is_some())
                {
                    2.0 * TALLY_SPRINT_DISTRICT_POINTS
                } else {
                    TALLY_SPRINT_DISTRICT_POINTS
                };
                match Self::score_sprint_follow_up(g, pid, cid, *district) {
                    Some(next)
                        if (turns + next / production).ceil() <= left
                            && (own + 1.0) / (cost + next).max(1.0) > own / cost.max(1.0) =>
                    {
                        (own + 1.0, cost + next)
                    }
                    _ => (own, cost),
                }
            }
            Item::Unit { unit } if unit.as_str() == "settler" => {
                let walk = super::score_seat::SCORE_SPRINT_SETTLER_WALK;
                let home = g.cities.get(&cid)?.pos;
                if (turns.ceil() + walk) > left
                    || !self.settle_site_exists(g, pid, home, TALLY_SPRINT_SETTLER_REACH)
                {
                    return None;
                }
                (TALLY_SPRINT_SETTLER_POINTS, cost)
            }
            _ => return None,
        };
        (points > 0.0).then(|| SprintOffer {
            item: item.clone(),
            points,
            era,
            cost,
            turns,
        })
    }

    /// The best offer of `cid`'s menu: the most points per Production, then
    /// the cheaper, then menu order.
    pub(crate) fn score_sprint_best(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        left: u32,
    ) -> Option<SprintOffer> {
        g.producible_items(pid, cid)
            .iter()
            .filter_map(|item| self.score_sprint_offer(g, pid, cid, item, left))
            .fold(None, |best: Option<SprintOffer>, offer| match best {
                Some(best)
                    if best.rate() > offer.rate()
                        || (best.rate() == offer.rate() && best.cost <= offer.cost) =>
                {
                    Some(best)
                }
                _ => Some(offer),
            })
    }

    /// Whether the sprint may take `cid`'s queue: see the module, point 1.
    fn score_sprint_may_take(g: &Game, plan: &StrategicPlan, cid: u32, at_war: bool) -> bool {
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        let attacked = city.last_attacked > 0
            && g.turn.saturating_sub(city.last_attacked) <= TALLY_SPRINT_ATTACK_MEMORY;
        let head_held = match city.queue.first() {
            Some(Item::Repair { .. }) => true,
            Some(Item::Unit { unit }) | Some(Item::Formation { unit, .. }) => {
                at_war && unit.as_str() != "settler"
            }
            _ => false,
        };
        plan.threatened_city != Some(cid) && !attacked && !head_held
    }

    /// See the module, point 1: every city's queue head becomes its best
    /// offer unless the head scores nearly as well. Exact no-op off and
    /// outside the window.
    pub(super) fn claim_score_sprint_queues(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let Some(left) = self.score_sprint_left(g) else {
            return;
        };
        let at_war = Self::score_sprint_at_war(g, pid);
        for cid in g.player_city_ids(pid) {
            if !Self::score_sprint_may_take(g, plan, cid, at_war) {
                continue;
            }
            let head = g.cities[&cid].queue.first().cloned();
            let held = head
                .as_ref()
                .and_then(|item| self.score_sprint_offer(g, pid, cid, item, left));
            let Some(best) = self.score_sprint_best(g, pid, cid, left) else {
                continue;
            };
            if held
                .as_ref()
                .is_some_and(|held| best.rate() <= TALLY_SPRINT_KEEP_SLACK * held.rate())
            {
                continue;
            }
            let name = g.cities[&cid].name.clone();
            let label = Self::plain_item(&best.item);
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: best.item.clone(),
                },
            )
            .is_ok()
            {
                let replaced = match (&head, &held) {
                    (None, _) => "an idle queue".to_string(),
                    (Some(item), None) => {
                        format!("{}, which scores nothing in time", Self::plain_item(item))
                    }
                    (Some(item), Some(held)) => format!(
                        "{} at {:.1} points a hundred Production",
                        Self::plain_item(item),
                        100.0 * held.rate()
                    ),
                };
                let tag = self.score_sprint_tag();
                let era = self.score_sprint_era_note(best.era);
                think!(self.journal(), Economy, Decision,
                    "{name} sprints to the tally with {label}";
                    "{tag}: {:.1} points{era} for {:.0} Production in {:.0} turns ({:.1} a \
                     hundred), {left} turns before the tally; it replaces {replaced}",
                    best.points, best.cost, best.turns.ceil(), 100.0 * best.rate());
            }
        }
    }

    /// Faith a turn across `pid`'s cities.
    fn score_sprint_faith_income(g: &Game, pid: usize) -> f64 {
        g.player_city_ids(pid)
            .into_iter()
            .map(|cid| g.city_yields(cid).faith)
            .sum()
    }

    /// Every purchase in `currency` the tally pays for: see the module,
    /// point 2.
    pub(crate) fn score_sprint_buys(g: &Game, pid: usize, currency: &str) -> Vec<SprintBuy> {
        let mut buys: Vec<SprintBuy> = g
            .legal_purchase_actions(pid)
            .into_iter()
            .filter_map(|action| {
                let (points, label) = match &action {
                    Action::BuyBuilding {
                        city,
                        building,
                        currency: paid,
                    } if paid.as_str() == currency => {
                        let spec = g.rules.buildings.get(building)?;
                        let item = Item::Building {
                            building: *building,
                        };
                        if spec.wonder || g.item_invested_production(*city, &item) > 0.0 {
                            return None;
                        }
                        (
                            1.0,
                            format!("{} in {}", plain(building.as_str()), g.cities[city].name),
                        )
                    }
                    Action::BuyDistrict {
                        city,
                        district,
                        currency: paid,
                        ..
                    } if paid.as_str() == currency => {
                        if g.district_family(*district).as_str() == "spaceport" {
                            return None;
                        }
                        let unique = g
                            .rules
                            .districts
                            .get(district)
                            .is_some_and(|spec| spec.unique_to.is_some());
                        let points = if unique {
                            2.0 * TALLY_SPRINT_DISTRICT_POINTS
                        } else {
                            TALLY_SPRINT_DISTRICT_POINTS
                        };
                        (
                            points,
                            format!("{} in {}", plain(district.as_str()), g.cities[city].name),
                        )
                    }
                    _ => return None,
                };
                let price = g.purchase_quote(pid, &action)?;
                (price.is_finite() && price > 0.0).then_some(SprintBuy {
                    action,
                    label,
                    points,
                    era: 0.0,
                    price,
                })
            })
            .collect();
        let classes: std::collections::BTreeSet<String> = g
            .rules
            .great_people
            .values()
            .map(|person| person.kind.clone())
            .collect();
        for kind in classes {
            if !g.great_person_class_offered_now(pid, &kind) {
                continue;
            }
            let Some(price) = g.great_person_patronage_price(pid, &kind, currency) else {
                continue;
            };
            if price.is_finite() && price > 0.0 {
                buys.push(SprintBuy {
                    label: format!("the Great {}", plain(&kind)),
                    action: Action::PatronizeGreatPerson {
                        kind,
                        currency: currency.to_string(),
                    },
                    points: TALLY_SPRINT_GREAT_PERSON_POINTS,
                    era: 0.0,
                    price,
                });
            }
        }
        buys
    }

    /// The bank of `currency` above which the pass spends.
    fn score_sprint_reserve(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        currency: &str,
        left: u32,
    ) -> f64 {
        if currency == "faith" {
            return TALLY_SPRINT_FAITH_RESERVE;
        }
        let deficit = (-g.players[pid].gold_per_turn).max(0.0) * left as f64;
        let reserve = TALLY_SPRINT_GOLD_RESERVE.max(deficit);
        // `purchases-keep-the-upkeep-reserve` and `threatened-city-reserve`:
        // `stock` comes back unchanged while those genes are off.
        let reserve = self.purchase_upkeep_floor(g, pid, reserve);
        self.reserve_for_the_threatened_city(g, pid, plan, reserve)
    }

    /// The bank of `currency`.
    fn score_sprint_bank(g: &Game, pid: usize, currency: &str) -> f64 {
        if currency == "gold" {
            g.players[pid].gold
        } else {
            g.players[pid].faith
        }
    }

    /// The income of `currency` a turn, never below zero.
    fn score_sprint_income(g: &Game, pid: usize, currency: &str) -> f64 {
        if currency == "gold" {
            g.players[pid].gold_per_turn
        } else {
            Self::score_sprint_faith_income(g, pid)
        }
        .max(0.0)
    }

    /// The bank's menu in `currency`: version 1's
    /// ([`Self::score_sprint_buys`]). Version 2 adds each Great Person's
    /// Historic Moment to its points, leaves a class to the currency
    /// `patrons` names, and before version 1's window (`full` false) offers
    /// Great People only.
    fn score_sprint_menu(
        &self,
        g: &Game,
        pid: usize,
        currency: &str,
        full: bool,
        patrons: &BTreeMap<String, &'static str>,
    ) -> Vec<SprintBuy> {
        let mut buys = Self::score_sprint_buys(g, pid, currency);
        if !self.score_seat_sprints_to_the_tally_2 {
            return buys;
        }
        buys.retain_mut(|buy| match &buy.action {
            Action::PatronizeGreatPerson { kind, .. } => {
                if patrons.get(kind).is_some_and(|patron| *patron != currency) {
                    return false;
                }
                buy.era = Self::score_sprint_patronage_era(g, pid, kind, currency);
                buy.points += buy.era;
                true
            }
            _ => full,
        });
        buys
    }

    /// `score-seat-sprints-to-the-tally-2`: the currency that patronizes each
    /// Great Person class on offer (see the module, version 2 point 2): of
    /// the currencies whose bank reaches the price before the tally, the one
    /// that gives up the fewest points of its other purchases (an idle bank
    /// gives up none), then the one that pays it sooner, then the one the
    /// price takes the smaller share of. Empty with version 1 and for a class
    /// neither currency reaches.
    pub(crate) fn score_sprint_patrons(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        left: u32,
        full: bool,
    ) -> BTreeMap<String, &'static str> {
        if !self.score_seat_sprints_to_the_tally_2 {
            return BTreeMap::new();
        }
        // Per class: (points given up, turns until paid, share of the bank)
        // and the currency.
        let mut best: BTreeMap<String, ([f64; 3], &'static str)> = BTreeMap::new();
        for currency in ["faith", "gold"] {
            let reserve = self.score_sprint_reserve(g, pid, plan, currency, left);
            let bank = Self::score_sprint_bank(g, pid, currency);
            let income = Self::score_sprint_income(g, pid, currency);
            let reach = bank + income * left.saturating_sub(1) as f64 - reserve;
            let buys = self.score_sprint_menu(g, pid, currency, full, &BTreeMap::new());
            // Coins a point of the best other purchase the bank reaches.
            let other = buys
                .iter()
                .filter(|buy| {
                    !matches!(buy.action, Action::PatronizeGreatPerson { .. })
                        && buy.price <= reach + f64::EPSILON
                })
                .map(SprintBuy::per_point)
                .min_by(f64::total_cmp);
            for buy in &buys {
                let Action::PatronizeGreatPerson { kind, .. } = &buy.action else {
                    continue;
                };
                if buy.price > reach + f64::EPSILON {
                    continue;
                }
                let forgone = other.map_or(0.0, |rate| buy.price / rate.max(f64::EPSILON));
                let short = (buy.price + reserve - bank).max(0.0);
                let wait = if short > 1e-9 {
                    short / income.max(1e-9)
                } else {
                    0.0
                };
                let key = [forgone, wait, buy.price / reach.max(1.0)];
                let better = best.get(kind).is_none_or(|(was, _)| {
                    key.iter()
                        .zip(was)
                        .find(|(now, then)| (*now - *then).abs() > 1e-9)
                        .is_some_and(|(now, then)| now < then)
                });
                if better {
                    best.insert(kind.clone(), (key, currency));
                }
            }
        }
        best.into_iter()
            .map(|(kind, (_, currency))| (kind, currency))
            .collect()
    }

    /// See the module, point 2: Gold and Faith above their reserves buy the
    /// fewest coins a point. Exact no-op off and outside the window.
    pub(super) fn score_sprint_purchases(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let Some(left) = self.score_sprint_great_person_left(g) else {
            return;
        };
        // Version 2 opens for Great People only before version 1's window.
        let full = self.score_sprint_left(g).is_some();
        let patrons = self.score_sprint_patrons(g, pid, plan, left, full);
        let tag = self.score_sprint_tag();
        // Faith first: it buys only Great People and a few buildings, while
        // Gold also buys every building. At turn 180 of the 2026-11-16 game
        // an Admiral cost 630 Faith or 1,025 Gold, and Gold's next best was
        // a 580-Gold Bank.
        for currency in ["faith", "gold"] {
            let reserve = self.score_sprint_reserve(g, pid, plan, currency, left);
            let income = Self::score_sprint_income(g, pid, currency);
            // Income still to come before the tally's last purchase.
            let coming = income * left.saturating_sub(1) as f64;
            for _ in 0..TALLY_SPRINT_MAX_BUYS {
                let bank = Self::score_sprint_bank(g, pid, currency);
                let mut buys = self.score_sprint_menu(g, pid, currency, full, &patrons);
                buys.sort_by_shared(|a, b| a.per_point().total_cmp(&b.per_point()));
                // The best offer the bank can still reach before the tally.
                let saving = buys
                    .iter()
                    .find(|buy| buy.price + reserve <= bank + coming + f64::EPSILON)
                    .cloned();
                let Some(saving) = saving else {
                    break;
                };
                let pick = buys.into_iter().find(|buy| {
                    let after = bank - buy.price;
                    after + f64::EPSILON >= reserve
                        && (buy.per_point() <= saving.per_point()
                            || after + coming + f64::EPSILON >= saving.price + reserve)
                });
                let Some(pick) = pick else {
                    let era = self.score_sprint_era_note(saving.era);
                    think!(self.journal(), Economy, Detail,
                        "The bank saves for {}", saving.label;
                        "{tag}: {:.0} points{era} for {:.0} {currency} ({:.0} a point); \
                         {bank:.0} in the bank, +{income:.0} a turn, {left} turns before the \
                         tally",
                        saving.points, saving.price, saving.per_point());
                    break;
                };
                if g.apply(pid, &pick.action).is_err() {
                    break;
                }
                let era = self.score_sprint_era_note(pick.era);
                think!(self.journal(), Economy, Decision,
                    "Buying {} for the tally", pick.label;
                    "{tag}: {:.0} points{era} for {:.0} {currency} ({:.0} a point), {left} turns \
                     before the tally; {:.0} left over a {reserve:.0} reserve",
                    pick.points, pick.price, pick.per_point(), bank - pick.price);
            }
        }
    }
}

#[cfg(test)]
mod tests;
