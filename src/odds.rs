//! Two win probabilities for every seat: the one the table had before a tile
//! was drawn, and the one it has now.
//!
//! A spectator asks a game in progress exactly one question — who wins? — and
//! it has two honest answers, so this module publishes both.
//!
//! **Start odds** are the pregame prior. Who is playing, how strong they are,
//! which civilization they drew, what the difficulty setting hands their seat,
//! and how many seats are sharing the table's single win. Nothing that happens
//! on the board moves them, and that is the point: they are a *prediction*,
//! fixed before the first turn, that the finished game can be scored against.
//!
//! **Now odds** are the best guess at this moment. The same prior, corrected by
//! the position each empire has actually built and by how much of the clock is
//! left to overturn it, and collapsed to zero for anybody already eliminated
//! unless a surviving permanent teammate can still win for their side.
//! On turn one there is almost nothing to correct — every empire is a settler
//! and an escort, and what little separates them is discounted for being early
//! — so the two figures start out together and part company as the world
//! diverges. That is what makes the pair readable side by side: the distance
//! between them is everything the game has decided so far.
//!
//! What can end the game counts for more than what merely looks strong. The
//! position term carries each seat's closest victory as far as it has been
//! earned: the capital every seat starts with is no progress toward Domination,
//! and the Science road is priced in techs for the hundred-odd turns before its
//! meter can move.
//!
//! Both are shares of one win, so they sum to one across the table whenever the
//! rules leave a way to win. If every victory condition is disabled, every
//! chance is zero instead of inventing a winner. With teams
//! on they sum to one across the *teams*: a team victory credits every member
//! ([`Game::winning_players`]), so each member carries the whole side's chance
//! of being among the winners rather than a slice of it.
//!
//! ## What this is not
//!
//! It is a rating-and-standing model, not a search. It does not read plans,
//! count what a unit could reach next turn, or roll the game out. It also
//! judges the whole board when one is available, withholding unfamiliar rows
//! from a fogged viewer's ribbon. Partial host and decision boards use public
//! empire totals without inventing hidden locations. An unobserved empire
//! retains its prior with neutral standing evidence, rather than being called
//! defeated. Missing city evidence also prevents a Mercy Rule verdict.
//!
//! ## Calibration
//!
//! Every coefficient below is in Elo, where 400 points is ten-to-one. The
//! difficulty scale is anchored on the measured ladder: an unhandicapped agent
//! at a four-seat table won 23.0% of its games at Prince, 14.0% at King, 4.0%
//! at Emperor, 1.0% at Immortal and 0.0% at Deity, which in share terms is a
//! deficit of roughly 0, 125, 360, 610 and 800 Elo against the seats getting
//! the bonuses. The AI-side constants reproduce that curve to within about 40
//! Elo a rung. The standing components remain stated judgements about what each
//! kind of lead is worth, kept in one place and named. Their clock multiplier is
//! measured against completed games at three phases, using both Brier error and
//! winner log loss so confidence as well as ranking is checked.

use crate::game::Game;
use std::collections::BTreeMap;

/// [`crate::elo::win_shares`], on soft-float `libm::pow`.
///
/// The Mercy Rule makes these odds outcome-bearing: a threshold crossing ends
/// the game, so the same seed must cross on the same turn on every platform.
/// IEEE-754 does not pin `powf`, and the platform math libraries round it
/// differently (docs/FLOAT_DETERMINISM.md, #1061) — every transcendental in
/// the `table` path therefore calls this crate's `libm` implementations. The
/// display-only callers of `elo::win_shares` keep the platform version.
fn win_shares(ratings: &[f64]) -> Vec<f64> {
    let top = ratings.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let weights: Vec<f64> = ratings
        .iter()
        .map(|rating| libm::pow(10.0, (rating - top) / 400.0))
        .collect();
    let total: f64 = weights.iter().sum();
    if total <= 0.0 || !total.is_finite() {
        return vec![1.0 / ratings.len().max(1) as f64; ratings.len()];
    }
    weights.iter().map(|weight| weight / total).collect()
}

/// Elo per point of mean city-yield percentage the difficulty setting adds.
const HANDICAP_YIELD_ELO: f64 = 4.0;
/// Elo per flat point of Combat Strength.
const HANDICAP_COMBAT_ELO: f64 = 100.0;
/// Elo per percentage point of extra experience.
const HANDICAP_XP_ELO: f64 = 1.2;
/// Elo per free Eureka/Inspiration granted on each new world era.
const HANDICAP_ERA_BOOST_ELO: f64 = 15.0;
/// Elo per extra unit standing on the start tile.
const HANDICAP_UNIT_ELO: f64 = 20.0;
/// The human side of the bargain is priced on its own scale. The AI-side
/// numbers were fitted where every bonus rose together, so reusing them for a
/// seat that receives only Combat Strength and experience would overstate a
/// low difficulty by hundreds of Elo.
const HANDICAP_HUMAN_COMBAT_ELO: f64 = 60.0;
const HANDICAP_HUMAN_XP_ELO: f64 = 1.0;
const HANDICAP_HUMAN_CAMP_GOLD_ELO: f64 = 1.0;

/// Elo for doubling the field's Score, its military strength, and its city
/// count. Score is the engine's own composite standing — cities, districts,
/// buildings, Citizens, Great People, religion, techs, civics, wonders and Era
/// Score — so it carries the most weight. Military decides who can take and
/// hold ground. Cities compound into everything else and are counted again,
/// lightly, because Score credits a city once while an empire spends it every
/// turn.
const LEAD_SCORE_ELO: f64 = 350.0;
const LEAD_MILITARY_ELO: f64 = 175.0;
const LEAD_CITY_ELO: f64 = 200.0;
/// Added to both sides of each ratio, so an empty empire is a large deficit
/// rather than a division by zero and a young one is not judged on noise. Each
/// floor is about what a seat holds in its opening turns: without them a single
/// extra point of Score at turn 20, when the whole table is on six, reads as a
/// sixty per cent lead over the field and prices like one.
const SCORE_FLOOR: f64 = 25.0;
const MILITARY_FLOOR: f64 = 40.0;
const CITY_FLOOR: f64 = 0.75;
/// Fixed deficit for a seat that has gone past the opening and holds no city
/// but still has a settler. Global lead calibration must stay soft enough for
/// ordinary late upsets; an empire reduced to a mobile restart is a distinct
/// collapse and should not inherit that softness.
const NO_CITY_ELO: f64 = 100.0;

/// Elo for a victory race that is finished. A race at the threshold ends the
/// game, so the curve is steep at the top and nearly flat early: capturing one
/// capital of six is a fifth of a Domination victory and worth far less than a
/// fifth of the win it implies.
const RACE_FULL_ELO: f64 = 1500.0;
const RACE_CURVE: f64 = 2.5;
/// Elo for one natural-log ratio of known techs against the living field, when
/// the Science Victory is open. Its meter reads zero until the first launch,
/// yet 53 of 66 audited Deity host games were decided by that race, and the
/// tech lead was the strongest out-of-sample addition to this forecast.
/// Deliberately below that sample's optimum: the sample is mostly Science wins,
/// and a game that other victories decide should not be priced as if it were
/// one.
const SCIENCE_ROAD_ELO: f64 = 450.0;
/// Added to both sides of the tech ratio, so a seat with a few techs is not
/// priced as multiples of a seat with fewer in the opening turns.
const TECH_FLOOR: f64 = 5.0;

/// How much of the pregame prior is given up by the final turn. A rating never
/// becomes worthless — a strong player converts a level position better — but
/// by the end the board has outranked it.
const PRIOR_FADE: f64 = 0.5;
/// What a lead is worth as the clock runs: the standing term is multiplied by
/// `LEAD_BASE + LEAD_SHARPEN × clock`.
///
/// The base is deliberately low. An opening lead is mostly noise — one seat's
/// capital happening to sit on a second luxury is three points of Score at turn
/// 20 and says almost nothing about turn 300 — while the same margin with
/// twenty turns left is nearly the result. Discounting early evidence is also
/// what keeps the now odds beside the start odds through the opening, instead of
/// swinging on the first tile either seat improves.
/// The base is not zero, though. A capital captured on turn 30 or a
/// civilization wiped out is decisive whatever the clock says, and the victory
/// races ride on this same weight — discounting them to nothing early would
/// leave a real event unreported for a hundred turns.
const LEAD_BASE: f64 = 0.15;
// Calibrated against completed games rather than inherited from the original
// judgement call. An end weight of 3.0 made the 75%-clock forecast worse than
// the 50% one and assigned punishing log loss to late upsets; 1.25 was the
// minimum-error point of the held forecast sweep while preserving the same
// ordering signal. `LEAD_BASE + LEAD_SHARPEN` is therefore 1.25.
const LEAD_SHARPEN: f64 = 1.10;
/// The Elo-to-Glicko scale: 400 / ln(10). Rating deviation lives on this scale
/// when the forecast attenuates an uncertain rating difference.
const ELO_PER_LOGIT: f64 = 173.717_792_761_797_6;

/// A pregame rating and the uncertainty behind it.
///
/// Elo alone is not a complete forecast. A one-game 1800 and a settled 1800
/// have the same midpoint but should not make the same promise about winning.
/// The deviation is the familiar Glicko RD in Elo points; zero retains ordinary
/// Elo arithmetic for callers that have no uncertainty measurement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PriorRating {
    pub elo: f64,
    pub deviation: f64,
}

impl PriorRating {
    pub fn new(elo: f64, deviation: f64) -> PriorRating {
        PriorRating { elo, deviation }
    }
}

impl From<f64> for PriorRating {
    fn from(elo: f64) -> PriorRating {
        PriorRating::new(elo, 0.0)
    }
}

/// Both odds for one seat, and the terms that produced them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SeatOdds {
    /// Chance this seat wins, judged before the game began.
    pub start: f64,
    /// Chance this seat wins from where the game stands now.
    pub now: f64,
    /// The pregame strength this seat brought, in Elo: its rating, its
    /// civilization's edge, and its difficulty handicap.
    pub prior_elo: f64,
    /// How much of `prior_elo` the difficulty setting alone accounts for.
    pub handicap_elo: f64,
    /// What this seat's position is worth against the rest of the living
    /// field, in Elo. Zero while the table is even, negative when behind.
    pub standing_elo: f64,
    /// The closest enabled victory this seat has come to finishing, per cent.
    pub race_pct: f64,
}

/// Start and now odds for every major at the table, keyed by seat.
///
/// `prior_rating` supplies the rating and deviation each seat sits down with,
/// before difficulty: the provisional base for every seat now that nothing
/// rates them. A plain `f64` is accepted for callers without
/// an RD and is treated as settled. City-states and barbarians are absent.
pub fn table<R>(game: &Game, prior_rating: impl Fn(usize) -> R) -> BTreeMap<usize, SeatOdds>
where
    R: Into<PriorRating>,
{
    table_impl(game, prior_rating, true)
}

/// Reference for replay audits: the former located-city treatment, including
/// its missing-rival exclusions. Other observed standings remain identical.
#[cfg(feature = "developer-tools")]
pub fn table_with_located_cities<R>(
    game: &Game,
    prior_rating: impl Fn(usize) -> R,
) -> BTreeMap<usize, SeatOdds>
where
    R: Into<PriorRating>,
{
    table_impl(game, prior_rating, false)
}

fn table_impl<R>(
    game: &Game,
    prior_rating: impl Fn(usize) -> R,
    public_standings: bool,
) -> BTreeMap<usize, SeatOdds>
where
    R: Into<PriorRating>,
{
    let seats: Vec<usize> = game
        .players
        .iter()
        .filter(|p| !p.is_minor && !p.is_barbarian)
        .map(|p| p.id)
        .collect();
    let mut odds = BTreeMap::new();
    if seats.is_empty() {
        return odds;
    }

    // ---- the pregame prior, which no later turn may move -------------------
    let ratings: Vec<PriorRating> = seats.iter().map(|pid| prior_rating(*pid).into()).collect();
    let rated_strength = uncertainty_adjusted_elos(&ratings);
    let handicap: Vec<f64> = seats.iter().map(|pid| handicap_elo(game, *pid)).collect();
    let prior: Vec<f64> = ratings
        .iter()
        .zip(&handicap)
        .map(|(rating, bonus)| rating.elo + bonus)
        .collect();
    // Difficulty is a known property of the rules, not uncertainty about who
    // sat down, so it is added after rating attenuation rather than blurred by
    // a provisional player's RD.
    let pregame_elo: Vec<f64> = rated_strength
        .iter()
        .zip(&handicap)
        .map(|(rating, bonus)| rating + bonus)
        .collect();
    let any_victory = any_victory_enabled(game);
    let start = if any_victory {
        team_shares(game, &seats, &win_shares(&pregame_elo))
    } else {
        vec![0.0; seats.len()]
    };

    for (index, pid) in seats.iter().enumerate() {
        odds.insert(
            *pid,
            SeatOdds {
                start: start[index],
                now: 0.0,
                prior_elo: prior[index],
                handicap_elo: handicap[index],
                standing_elo: 0.0,
                race_pct: 0.0,
            },
        );
    }

    // ---- and the board as it stands ---------------------------------------
    //
    // A finished game is not a forecast. Whoever the result credits holds the
    // whole win; a world asked for one more turn has no winner again and is
    // estimated like any other live position.
    if game.winner.is_some() {
        let winners = game.winning_players();
        for (pid, seat) in odds.iter_mut() {
            seat.now = f64::from(winners.contains(pid));
        }
        return odds;
    }
    if !any_victory {
        return odds;
    }

    let living: Vec<usize> = seats
        .iter()
        .copied()
        .filter(|pid| still_playing(game, *pid, public_standings))
        .collect();
    if living.is_empty() {
        return odds;
    }

    let standing = standing_elo(game, &living, public_standings);
    let progress = clock_progress(game);
    let mean_prior = mean(
        &living
            .iter()
            .map(|pid| pregame_elo[seat_index(&seats, *pid)])
            .collect::<Vec<_>>(),
    );
    let live_elo: Vec<f64> = living
        .iter()
        .zip(&standing)
        .map(|(pid, position)| {
            (1.0 - PRIOR_FADE * progress) * (pregame_elo[seat_index(&seats, *pid)] - mean_prior)
                + (LEAD_BASE + LEAD_SHARPEN * progress) * position.elo
                - no_city_penalty(game, *pid, public_standings)
        })
        .collect();
    let individual_now = win_shares(&live_elo);
    for (index, pid) in living.iter().enumerate() {
        let seat = odds.get_mut(pid).expect("a living seat is at the table");
        seat.standing_elo = standing[index].elo - no_city_penalty(game, *pid, public_standings);
        seat.race_pct = standing[index].race_pct;
    }
    // A defeated teammate cannot trigger a victory, but `winning_players`
    // still credits them when another member of the permanent side does. The
    // estimate answers that same question — chance of being among the winners
    // — so every member carries the side's probability while any member lives.
    for pid in &seats {
        odds.get_mut(pid).expect("every seat is at the table").now =
            team_share(game, *pid, &living, &individual_now);
    }
    odds
}

/// The seat the Mercy Rule would crown from this position, if any.
///
/// A living seat whose live share meets `threshold` is the mercy winner;
/// `None` while the table is still contested. Flat 1500 priors, always: the
/// rule is part of the game and a game knows nothing about rosters, so only
/// the board, the handicaps and the clock separate the seats — and the same
/// position answers the same on every platform (see `win_shares` above).
/// With permanent teams every living member carries the side's whole share,
/// so the lowest living seat of the leading side is crowned and the normal
/// team-victory credit does the rest.
pub fn mercy_leader(game: &Game, threshold: f64) -> Option<usize> {
    // A partial view cannot establish that an unobserved empire has lost.
    // Missing city evidence must never end a rollout or a player's game.
    if game.players.iter().any(|player| {
        player.alive
            && !player.is_minor
            && !player.is_barbarian
            && city_status_unknown(game, player.id)
    }) {
        return None;
    }
    let odds = table(game, |_pid| 1500.0f64);
    odds.iter()
        .filter(|(pid, seat)| game.players[**pid].alive && seat.now >= threshold)
        .max_by(|a, b| a.1.now.total_cmp(&b.1.now).then_with(|| b.0.cmp(a.0)))
        .map(|(pid, _)| *pid)
}

/// What the difficulty setting is worth to one seat, in Elo.
///
/// Above Prince the bonuses land on the AI seats and below it on the human
/// ones, so this is a difference between seats rather than a level the whole
/// table shares — which is exactly why it belongs in the start odds. In an
/// all-AI exhibition every seat draws the same handicap and it cancels out of
/// the shares, as it should: Deity does not make a field of Deity AIs harder
/// for each other.
///
/// Barbarian scaling is deliberately absent. It is a property of the world, not
/// of a seat, so it cannot move one seat's chance against another's.
fn handicap_elo(game: &Game, pid: usize) -> f64 {
    let Some(player) = game.players.get(pid) else {
        return 0.0;
    };
    if player.is_minor || player.is_barbarian || game.is_handicap_exempt(pid) {
        return 0.0;
    }
    let spec = game.difficulty_spec();
    let combat = game.handicap_combat_strength(pid);
    let experience = game.handicap_xp_pct(pid);
    if game.is_human_seat(pid) {
        return HANDICAP_HUMAN_COMBAT_ELO * combat
            + HANDICAP_HUMAN_XP_ELO * experience
            + HANDICAP_HUMAN_CAMP_GOLD_ELO * spec.human_camp_gold;
    }
    // Food is never handicapped, so the mean is taken over the five yields the
    // ladder actually scales.
    // Pregame odds must remain pregame odds. `handicap_yield_pct` includes
    // the late-science schedule at the *current* turn; reading it here made
    // the published start forecast change when that schedule switched on.
    let yields = game.handicap_yield_spec().ai_yield_pct;
    let mean_yield =
        (yields.production + yields.gold + yields.science + yields.culture + yields.faith) / 5.0;
    let bonus_units: usize = spec.ai_bonus_units.values().sum();
    HANDICAP_YIELD_ELO * mean_yield
        + HANDICAP_COMBAT_ELO * combat
        + HANDICAP_XP_ELO * experience
        + HANDICAP_ERA_BOOST_ELO * spec.ai_era_boosts as f64
        + HANDICAP_UNIT_ELO * bonus_units as f64
}

/// Whether this seat can still win: alive, and holding either a city or a
/// settler to found one. Public city totals and hidden-city existence count
/// on partial boards, just as they do for the engine's elimination check.
///
/// The second half is the engine's own elimination rule (`check_elimination`)
/// read a moment before it fires. A civilization whose last city fell is not
/// marked defeated until something checks, and in between it is a seat with an
/// army, no ground, and no way to take any — a win probability of zero rather
/// than a small one. Saying so early costs nothing and stops a doomed empire
/// from holding a few per cent of the table's win for the rest of a turn.
fn still_playing(game: &Game, pid: usize, public_standings: bool) -> bool {
    let Some(player) = game.players.get(pid) else {
        return false;
    };
    if !player.alive {
        return false;
    }
    // The arena's own elimination rule, mirrored: a side is its army, and it
    // is in the game while it has a unit standing or a city to build one
    // from. The world test below — a city or a Settler — is exactly the pair
    // an arena seat never holds, so without this branch every no-city arena
    // seat read as already out, the living list came back empty, and the
    // whole table sat at zero for the length of the battle. That, more than
    // any coefficient, is why no arena game ever crossed a mercy threshold.
    if game.is_arena() {
        return !game.player_city_ids(pid).is_empty()
            || game.units.values().any(|unit| unit.owner == pid);
    }
    (public_standings && city_status_unknown(game, pid))
        || counted_cities(game, pid, public_standings) > 0
        || game
            .units
            .values()
            .any(|unit| unit.owner == pid && unit.kind == "settler")
}

fn no_city_penalty(game: &Game, pid: usize, public_standings: bool) -> f64 {
    if public_standings && city_status_unknown(game, pid) {
        return 0.0;
    }
    if game.turn > 10 && counted_cities(game, pid, public_standings) == 0 {
        NO_CITY_ELO
    } else {
        0.0
    }
}

fn partial_board(game: &Game) -> bool {
    !game.observed_public_empire_stats.is_empty() || !game.observed_score.is_empty()
}

fn city_status_unknown(game: &Game, pid: usize) -> bool {
    !game.is_arena()
        && partial_board(game)
        && game.player_city_ids(pid).is_empty()
        && !game.unseen_city_owners.contains(&pid)
        && game
            .observed_public_empire_stats
            .get(&pid)
            .and_then(|s| s.city_count)
            .is_none()
}

fn position_known(game: &Game, pid: usize) -> bool {
    !partial_board(game)
        || game.observed_score.contains_key(&pid)
        || game.observed_public_empire_stats.contains_key(&pid)
        || !game.player_city_ids(pid).is_empty()
        || game.unseen_city_owners.contains(&pid)
}

fn counted_cities(game: &Game, pid: usize, public_standings: bool) -> usize {
    if public_standings {
        city_count(game, pid)
    } else {
        game.player_city_ids(pid).len()
    }
}

/// The host's public empire count includes cities whose locations the mirror
/// must not invent. A partial board's object count measures exploration,
/// rather than the rival's economy. Never let a stale reading erase an
/// actually represented city or known hidden-city existence.
fn city_count(game: &Game, pid: usize) -> usize {
    let represented = game.player_city_ids(pid).len();
    game.observed_public_empire_stats
        .get(&pid)
        .and_then(|stats| stats.city_count)
        .unwrap_or(represented)
        .max(represented)
        .max(usize::from(game.unseen_city_owners.contains(&pid)))
}

/// One seat's position, valued against the rest of the living field.
struct Standing {
    elo: f64,
    race_pct: f64,
}

/// Price every living seat's position in Elo, relative to the field.
///
/// Each material term is a log ratio against the field's mean, so the scale of
/// the game drops out: twice the mean Score is worth the same on turn 40 as on
/// turn 400. The victory term is the seat's closest enabled race, which is the
/// one measure that knows about *ending* the game rather than leading it.
fn standing_elo(game: &Game, living: &[usize], public_standings: bool) -> Vec<Standing> {
    let leading_score = game
        .players
        .iter()
        .filter(|p| !p.is_minor && !p.is_barbarian)
        .map(|p| game.team_score_rank_key(p.id).0)
        .max()
        .unwrap_or(0);
    let scores: Vec<f64> = living.iter().map(|pid| game.score(*pid) as f64).collect();
    let militaries: Vec<f64> = living.iter().map(|pid| game.military_power(*pid)).collect();
    let cities: Vec<f64> = living
        .iter()
        .map(|pid| counted_cities(game, *pid, public_standings) as f64)
        .collect();
    let victories: Vec<ClosestVictory> = living
        .iter()
        .map(|pid| closest_victory(game, *pid, leading_score))
        .collect();
    let race_elo: Vec<f64> = victories.iter().map(|v| race_elo(v.earned_pct)).collect();
    let techs: Vec<f64> = victories.iter().map(|v| v.techs).collect();
    let science_road_open = game.victory_conditions.science && !game.is_arena();
    // Unmet seats contain placeholder zeros, not observed economic collapse.
    // Center on the known field and give an unknown position neutral evidence;
    // its actual chance still follows the table's pregame handicap and rating.
    let known: Vec<bool> = living
        .iter()
        .map(|pid| !public_standings || position_known(game, *pid))
        .collect();
    let known_cities: Vec<bool> = living
        .iter()
        .map(|pid| !public_standings || !city_status_unknown(game, *pid))
        .collect();
    let known_mean = |values: &[f64], mask: &[bool]| {
        let values: Vec<f64> = values
            .iter()
            .zip(mask)
            .filter(|(_, known)| **known)
            .map(|(value, _)| *value)
            .collect();
        mean(&values)
    };
    let (mean_score, mean_military, mean_cities, mean_race, mean_techs) = (
        known_mean(&scores, &known),
        known_mean(&militaries, &known),
        known_mean(&cities, &known_cities),
        known_mean(&race_elo, &known),
        known_mean(&techs, &known),
    );
    (0..living.len())
        .map(|i| Standing {
            elo: if known[i] {
                LEAD_SCORE_ELO * log_ratio(scores[i], mean_score, SCORE_FLOOR)
                    + LEAD_MILITARY_ELO * log_ratio(militaries[i], mean_military, MILITARY_FLOOR)
                    + if known_cities[i] {
                        LEAD_CITY_ELO * log_ratio(cities[i], mean_cities, CITY_FLOOR)
                    } else {
                        0.0
                    }
                    + race_elo[i]
                    - mean_race
                    + if science_road_open {
                        SCIENCE_ROAD_ELO * log_ratio(techs[i], mean_techs, TECH_FLOOR)
                    } else {
                        0.0
                    }
            } else {
                0.0
            },
            race_pct: if known[i] { victories[i].pct } else { 0.0 },
        })
        .collect()
}

/// How close one seat stands to ending the game.
struct ClosestVictory {
    /// The closest enabled race as its meter reads it, per cent.
    pct: f64,
    /// The closest enabled race counting only what was earned, per cent.
    earned_pct: f64,
    /// Techs known, the Science road's reading before its meter can move.
    techs: f64,
}

/// The enabled victory race this seat is closest to finishing.
///
/// The five races that end the game by reaching a threshold, and, as in
/// [`Game::victory_threat`], not Score. A Score victory is not a race at all —
/// it is the standing when the clock runs out — and its meter is a ratio to the
/// current leader, which on turn 30 makes a two-point Score lead read as a
/// third of a victory. That reading belongs to the Score term above, which
/// prices the same lead against the field and is already sharpened as the clock
/// runs down. Counting it twice is what makes a young table look decided.
fn closest_victory(game: &Game, pid: usize, leading_score: i64) -> ClosestVictory {
    // On an arena the domination race is the destruction of the other army,
    // and the ordinary meter cannot see it: it counts capitals, an arena
    // holds none (or one apiece that only changes hands as the game ends),
    // and the material terms above are floored for Civ-world magnitudes that
    // an arena's army weights sit under. Measured before this branch existed,
    // a battle standing at twelve units against one still read as contested —
    // zero threshold crossings in twenty-four games at any threshold down to
    // 0.90 — so the Mercy Rule could never fire on the one mode whose games
    // most reliably outlive their result.
    //
    // The share of the surviving strength-weighted army is the arena's race
    // meter. It is the same quantity the arena's own Score victory ranks at
    // the clock (`Game::score_parts`), it needs no history to compute, and it
    // reads 50 exactly when the armies are even, so an undecided battle gains
    // nothing from either side.
    if game.is_arena() {
        // Unconditionally: the arena opens the domination lane whatever the
        // lobby's checkboxes say (`Game::victory_lane_open`), so the race is
        // always on. Reading the raw checkbox here would mute the meter in
        // exactly the games that still end by it.
        let weight = |seat: usize| game.score_parts(seat)[0].max(0) as f64;
        let mine = weight(pid);
        let total: f64 = game
            .players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian && player.alive)
            .map(|player| weight(player.id))
            .sum();
        let pct = if total <= 0.0 {
            0.0
        } else {
            (100.0 * mine / total).clamp(0.0, 100.0)
        };
        return ClosestVictory {
            pct,
            earned_pct: pct,
            techs: 0.0,
        };
    }
    let races = game.victory_races(pid, leading_score);
    let enabled = &game.victory_conditions;
    let closest = |lanes: [(bool, f64); 5]| {
        lanes
            .into_iter()
            .filter_map(|(on, progress)| on.then_some(progress))
            .fold(0.0_f64, f64::max)
    };
    // Every seat starts holding its own capital, which is no step toward
    // Domination. The other races are read as they stand: Religious and
    // Diplomatic meters looked overstated on host games where neither race
    // ever finished, but discounting them cost the actual winner in simulated
    // games those races decided.
    let capitals_held_at_start = game.team_members(pid).len().max(1) as f64;
    let own_capitals = 100.0 * capitals_held_at_start / races.capital_target.max(1) as f64;
    ClosestVictory {
        pct: closest([
            (enabled.science, races.science),
            (enabled.culture, races.culture),
            (enabled.religious, races.religious),
            (enabled.diplomatic, races.diplomatic),
            (enabled.domination, races.domination),
        ]),
        earned_pct: closest([
            (enabled.science, races.science),
            (enabled.culture, races.culture),
            (enabled.religious, races.religious),
            (enabled.diplomatic, races.diplomatic),
            (
                enabled.domination,
                beyond_free_share(races.domination, own_capitals),
            ),
        ]),
        techs: races.techs as f64,
    }
}

/// A meter reading with the share every seat holds for free taken out, rescaled
/// so that the race's finish still reads 100.
fn beyond_free_share(pct: f64, free: f64) -> f64 {
    if free >= 100.0 {
        return pct;
    }
    (100.0 * (pct - free) / (100.0 - free)).clamp(0.0, 100.0)
}

fn race_elo(pct: f64) -> f64 {
    RACE_FULL_ELO * libm::pow((pct / 100.0).clamp(0.0, 1.0), RACE_CURVE)
}

/// How far through its clock this world is.
///
/// A game that was decided and asked for one more turn has no limit left to
/// run out, and is late by definition. A world with no limit at all is judged
/// against its speed's stock clock so that turn three is not treated as the
/// last turn. A fixed 500 here made the same development age only half as
/// quickly on Online and three times as quickly on Marathon.
fn clock_progress(game: &Game) -> f64 {
    if game.played_on() {
        return 1.0;
    }
    match game.max_turns {
        limit if limit > 0 && limit < 100_000 => {
            (f64::from(game.turn) / f64::from(limit)).clamp(0.0, 1.0)
        }
        _ => (f64::from(game.turn) / f64::from(game.speed_spec().turns.max(1))).clamp(0.0, 1.0),
    }
}

/// Whether this ruleset leaves any way for the game to produce a winner.
/// A lobby may deliberately switch every path off; in that world a forced
/// one-win distribution is not a forecast but a contradiction of the rules.
/// Asked of the *effective* conditions: an arena opens domination and score
/// whatever the checkboxes say, so its odds must not read as a zeroed table
/// just because a launch line switched the world lanes off.
fn any_victory_enabled(game: &Game) -> bool {
    let enabled = &game.effective_victory_conditions();
    enabled.science
        || enabled.culture
        || enabled.religious
        || enabled.diplomatic
        || enabled.domination
        || enabled.score
}

/// Pull uncertain rating differences toward the field mean using the symmetric
/// Glicko forecast attenuation. The average pair at this table has twice the
/// mean rating variance; using that quantity makes a two-seat table exactly the
/// ordinary symmetric Glicko matchup and extends the same calibration to a
/// multiplayer softmax.
fn uncertainty_adjusted_elos(ratings: &[PriorRating]) -> Vec<f64> {
    if ratings.is_empty() {
        return Vec::new();
    }
    let mean_elo = ratings.iter().map(|rating| rating.elo).sum::<f64>() / ratings.len() as f64;
    let mean_variance = ratings
        .iter()
        .map(|rating| {
            let phi = rating.deviation.max(0.0) / ELO_PER_LOGIT;
            phi * phi
        })
        .sum::<f64>()
        / ratings.len() as f64;
    let attenuation = 1.0 / (1.0 + 6.0 * mean_variance / (std::f64::consts::PI.powi(2))).sqrt();
    ratings
        .iter()
        .map(|rating| mean_elo + attenuation * (rating.elo - mean_elo))
        .collect()
}

/// A team wins together, so every member carries the side's whole chance of
/// being among the winners. Seats with no team are left exactly as they are.
fn team_shares(game: &Game, seats: &[usize], shares: &[f64]) -> Vec<f64> {
    seats
        .iter()
        .map(|pid| team_share(game, *pid, seats, shares))
        .collect()
}

fn team_share(game: &Game, pid: usize, seats: &[usize], shares: &[f64]) -> f64 {
    let members = game.team_members(pid);
    if members.len() < 2 {
        return seats
            .iter()
            .position(|seat| *seat == pid)
            .map_or(0.0, |index| shares[index]);
    }
    members
        .iter()
        .filter_map(|member| seats.iter().position(|seat| seat == member))
        .map(|index| shares[index])
        .sum()
}

fn seat_index(seats: &[usize], pid: usize) -> usize {
    seats
        .iter()
        .position(|seat| *seat == pid)
        .expect("the seat came from this list")
}

fn log_ratio(value: f64, mean: f64, floor: f64) -> f64 {
    libm::log((value.max(0.0) + floor) / (mean.max(0.0) + floor))
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

#[cfg(test)]
mod tests;
