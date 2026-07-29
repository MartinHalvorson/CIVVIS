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
//! left to overturn it, and collapsed to zero for anybody already eliminated.
//! On turn one there is almost nothing to correct — every empire is a settler
//! and an escort, and what little separates them is discounted for being early
//! — so the two figures start out together and part company as the world
//! diverges. That is what makes the pair readable side by side: the distance
//! between them is everything the game has decided so far.
//!
//! Both are shares of one win, so they sum to one across the table. With teams
//! on they sum to one across the *teams*: a team victory credits every member
//! ([`Game::winning_players`]), so each member carries the whole side's chance
//! of being among the winners rather than a slice of it.
//!
//! ## What this is not
//!
//! It is a rating-and-standing model, not a search. It does not read plans,
//! count what a unit could reach next turn, or roll the game out. It also
//! judges the whole board: like the Elo-implied share it replaces, a seat's
//! odds are computed omnisciently and then withheld from a fogged viewer's
//! ribbon row by row, rather than being re-derived from what that viewer knows.
//!
//! ## Calibration
//!
//! Every coefficient below is in Elo, where 400 points is ten-to-one. The
//! difficulty scale is anchored on the measured ladder: an unhandicapped agent
//! at a four-seat table won 23.0% of its games at Prince, 14.0% at King, 4.0%
//! at Emperor, 1.0% at Immortal and 0.0% at Deity, which in share terms is a
//! deficit of roughly 0, 125, 360, 610 and 800 Elo against the seats getting
//! the bonuses. The AI-side constants reproduce that curve to within about 40
//! Elo a rung. The standing constants are not measured — they are stated
//! judgements about what a lead is worth, kept in one place and named so a
//! later experiment can move them.

use crate::elo::win_shares;
use crate::game::Game;
use crate::league::League;
use std::collections::BTreeMap;

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

/// Elo for a victory race that is finished. A race at the threshold ends the
/// game, so the curve is steep at the top and nearly flat early: capturing one
/// capital of six is a fifth of a Domination victory and worth far less than a
/// fifth of the win it implies.
const RACE_FULL_ELO: f64 = 1500.0;
const RACE_CURVE: f64 = 2.5;

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
const LEAD_SHARPEN: f64 = 2.85;
/// The clock a world with no turn limit is judged against, so an endless game
/// still ages instead of treating turn three as the last turn.
const NOMINAL_HORIZON: f64 = 500.0;

/// Games of evidence a civilization's edge is shrunk against. With none behind
/// it the edge is zero; with plenty it is taken at face value.
const CIV_EDGE_PRIOR_GAMES: f64 = 30.0;

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
/// `prior_elo` supplies the rating each seat sits down with, before difficulty:
/// the league's number where there is one, and the provisional base where there
/// is not. City-states and barbarians are not at this table and are absent from
/// the result.
pub fn table(game: &Game, prior_elo: impl Fn(usize) -> f64) -> BTreeMap<usize, SeatOdds> {
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
    let handicap: Vec<f64> = seats.iter().map(|pid| handicap_elo(game, *pid)).collect();
    let prior: Vec<f64> = seats
        .iter()
        .zip(&handicap)
        .map(|(pid, bonus)| prior_elo(*pid) + bonus)
        .collect();
    let start = team_shares(game, &seats, &win_shares(&prior));

    for ((pid, prior_elo), handicap_elo) in seats.iter().zip(&prior).zip(&handicap) {
        odds.insert(
            *pid,
            SeatOdds {
                start: start[seats.iter().position(|seat| seat == pid).expect("own seat")],
                now: 0.0,
                prior_elo: *prior_elo,
                handicap_elo: *handicap_elo,
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

    let living: Vec<usize> = seats
        .iter()
        .copied()
        .filter(|pid| still_playing(game, *pid))
        .collect();
    if living.is_empty() {
        return odds;
    }

    let standing = standing_elo(game, &living);
    let progress = clock_progress(game);
    let mean_prior = mean(
        &living
            .iter()
            .map(|pid| prior[seat_index(&seats, *pid)])
            .collect::<Vec<_>>(),
    );
    let live_elo: Vec<f64> = living
        .iter()
        .zip(&standing)
        .map(|(pid, position)| {
            (1.0 - PRIOR_FADE * progress) * (prior[seat_index(&seats, *pid)] - mean_prior)
                + (LEAD_BASE + LEAD_SHARPEN * progress) * position.elo
        })
        .collect();
    let now = team_shares(game, &living, &win_shares(&live_elo));
    for (index, pid) in living.iter().enumerate() {
        let seat = odds.get_mut(pid).expect("a living seat is at the table");
        seat.now = now[index];
        seat.standing_elo = standing[index].elo;
        seat.race_pct = standing[index].race_pct;
    }
    odds
}

/// How much stronger than their own rating the roster has been *while playing
/// this civilization*, in Elo.
///
/// A seat whose player already has a civ-specific rating needs none of this —
/// their number is that measurement. This is for everybody else: a player with
/// no games as Rome still drew Rome, and the roster knows something about what
/// that is worth. The edge is games-weighted and shrunk toward zero by the
/// evidence behind it, so a civilization played twice moves a seat barely at
/// all and one played five hundred times moves it fully.
pub fn civ_edge_elo(league: &League, civ: &str) -> f64 {
    let mut games = 0.0;
    let mut weighted = 0.0;
    for player in &league.strategies {
        for combinations in player.leader_elo.values() {
            let Some(rated) = combinations.get(civ) else {
                continue;
            };
            if rated.games == 0 {
                continue;
            }
            let weight = f64::from(rated.games);
            games += weight;
            weighted += weight * (rated.rating - player.rating);
        }
    }
    if games <= 0.0 {
        return 0.0;
    }
    weighted / games * (games / (games + CIV_EDGE_PRIOR_GAMES))
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
    if player.is_minor || player.is_barbarian {
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
    let yields = game.handicap_yield_pct(pid);
    let mean_yield = (yields.production + yields.gold + yields.science + yields.culture
        + yields.faith)
        / 5.0;
    let bonus_units: usize = spec.ai_bonus_units.values().sum();
    HANDICAP_YIELD_ELO * mean_yield
        + HANDICAP_COMBAT_ELO * combat
        + HANDICAP_XP_ELO * experience
        + HANDICAP_ERA_BOOST_ELO * spec.ai_era_boosts as f64
        + HANDICAP_UNIT_ELO * bonus_units as f64
}

/// Whether this seat can still win: alive, and holding either a city or a
/// settler to found one.
///
/// The second half is the engine's own elimination rule (`check_elimination`)
/// read a moment before it fires. A civilization whose last city fell is not
/// marked defeated until something checks, and in between it is a seat with an
/// army, no ground, and no way to take any — a win probability of zero rather
/// than a small one. Saying so early costs nothing and stops a doomed empire
/// from holding a few per cent of the table's win for the rest of a turn.
fn still_playing(game: &Game, pid: usize) -> bool {
    let Some(player) = game.players.get(pid) else {
        return false;
    };
    if !player.alive {
        return false;
    }
    !game.player_city_ids(pid).is_empty()
        || game
            .units
            .values()
            .any(|unit| unit.owner == pid && unit.kind == "settler")
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
fn standing_elo(game: &Game, living: &[usize]) -> Vec<Standing> {
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
        .map(|pid| game.player_city_ids(*pid).len() as f64)
        .collect();
    let races: Vec<f64> = living
        .iter()
        .map(|pid| best_race_pct(game, *pid, leading_score))
        .collect();
    let race_elo: Vec<f64> = races.iter().map(|pct| race_elo(*pct)).collect();
    let (mean_score, mean_military, mean_cities, mean_race) = (
        mean(&scores),
        mean(&militaries),
        mean(&cities),
        mean(&race_elo),
    );
    (0..living.len())
        .map(|i| Standing {
            elo: LEAD_SCORE_ELO * log_ratio(scores[i], mean_score, SCORE_FLOOR)
                + LEAD_MILITARY_ELO * log_ratio(militaries[i], mean_military, MILITARY_FLOOR)
                + LEAD_CITY_ELO * log_ratio(cities[i], mean_cities, CITY_FLOOR)
                + race_elo[i]
                - mean_race,
            race_pct: races[i],
        })
        .collect()
}

/// The enabled victory race this seat is closest to finishing, per cent.
///
/// The five races that end the game by reaching a threshold, and, as in
/// [`Game::victory_threat`], not Score. A Score victory is not a race at all —
/// it is the standing when the clock runs out — and its meter is a ratio to the
/// current leader, which on turn 30 makes a two-point Score lead read as a
/// third of a victory. That reading belongs to the Score term above, which
/// prices the same lead against the field and is already sharpened as the clock
/// runs down. Counting it twice is what makes a young table look decided.
fn best_race_pct(game: &Game, pid: usize, leading_score: i64) -> f64 {
    let races = game.victory_races(pid, leading_score);
    let enabled = &game.victory_conditions;
    [
        (enabled.science, races.science),
        (enabled.culture, races.culture),
        (enabled.religious, races.religious),
        (enabled.diplomatic, races.diplomatic),
        (enabled.domination, races.domination),
    ]
    .into_iter()
    .filter_map(|(on, progress)| on.then_some(progress))
    .fold(0.0_f64, f64::max)
}

fn race_elo(pct: f64) -> f64 {
    RACE_FULL_ELO * (pct / 100.0).clamp(0.0, 1.0).powf(RACE_CURVE)
}

/// How far through its clock this world is.
///
/// A game that was decided and asked for one more turn has no limit left to
/// run out, and is late by definition. A world with no limit at all is judged
/// against a nominal one so that turn three is not treated as the last turn.
fn clock_progress(game: &Game) -> f64 {
    if game.played_on() {
        return 1.0;
    }
    match game.max_turns {
        limit if limit > 0 && limit < 100_000 => {
            (f64::from(game.turn) / f64::from(limit)).clamp(0.0, 1.0)
        }
        _ => (f64::from(game.turn) / NOMINAL_HORIZON).clamp(0.0, 1.0),
    }
}

/// A team wins together, so every member carries the side's whole chance of
/// being among the winners. Seats with no team are left exactly as they are.
fn team_shares(game: &Game, seats: &[usize], shares: &[f64]) -> Vec<f64> {
    seats
        .iter()
        .map(|pid| {
            let members = game.team_members(*pid);
            if members.len() < 2 {
                return shares[seat_index(seats, *pid)];
            }
            members
                .iter()
                .filter_map(|member| seats.iter().position(|seat| seat == member))
                .map(|index| shares[index])
                .sum()
        })
        .collect()
}

fn seat_index(seats: &[usize], pid: usize) -> usize {
    seats
        .iter()
        .position(|seat| *seat == pid)
        .expect("the seat came from this list")
}

fn log_ratio(value: f64, mean: f64, floor: f64) -> f64 {
    ((value.max(0.0) + floor) / (mean.max(0.0) + floor)).ln()
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Game, GameOptions};
    use crate::league::{League, Strategy, StrategyKind};
    use std::collections::BTreeSet;

    fn even_table(players: usize, difficulty: &str, human_seats: &[usize]) -> Game {
        let mut options = GameOptions::new(players, 32, 22, 4_771, 250, 0);
        options.barbarians = false;
        options.difficulty = difficulty.to_string();
        options.human_seats = human_seats.iter().copied().collect::<BTreeSet<_>>();
        Game::new_with(options)
    }

    fn flat(_: usize) -> f64 {
        1500.0
    }

    fn found_capital(game: &mut Game, pid: usize) {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("every seat starts with a settler");
        let position = game.units[&settler].pos;
        game.found_city_for(pid, position, None);
    }

    fn total(odds: &BTreeMap<usize, SeatOdds>, of: impl Fn(&SeatOdds) -> f64) -> f64 {
        odds.values().map(of).sum()
    }


    /// An even table splits one win, and on turn one the two answers still
    /// agree: six settlers and six escorts is nothing to disagree about. They
    /// are not identical — an opening escort can be worth a hair more than
    /// another's — but the gap has to be small enough to read as "even".
    #[test]
    fn an_even_table_splits_one_win_and_both_answers_start_together() {
        let game = even_table(6, "prince", &[]);
        let odds = table(&game, flat);
        assert_eq!(odds.len(), 6);
        for seat in odds.values() {
            assert!((seat.start - 1.0 / 6.0).abs() < 1e-9, "{:?}", seat);
            assert!((seat.now - seat.start).abs() < 0.02, "{:?}", seat);
            assert_eq!(seat.handicap_elo, 0.0, "Prince hands out nothing");
        }
        assert!((total(&odds, |seat| seat.start) - 1.0).abs() < 1e-9);
        assert!((total(&odds, |seat| seat.now) - 1.0).abs() < 1e-9);
    }

    /// The difficulty setting is priced into the start odds, and it is priced
    /// the way the ladder was measured: a human seat is nearly hopeless at
    /// Deity, favoured at Settler, and level at Prince.
    #[test]
    fn the_difficulty_setting_prices_the_human_seat() {
        let ladder = ["settler", "chieftain", "warlord", "prince", "king", "emperor",
                      "immortal", "deity"];
        let mut human_odds = Vec::new();
        for difficulty in ladder {
            let game = even_table(4, difficulty, &[0]);
            let odds = table(&game, flat);
            human_odds.push(odds[&0].start);
            assert!(
                (total(&odds, |seat| seat.start) - 1.0).abs() < 1e-9,
                "{difficulty} still shares out exactly one win"
            );
        }
        let prince = human_odds[3];
        assert!(
            (prince - 0.25).abs() < 1e-9,
            "Prince hands out nothing, so a four-seat table is even: {prince}"
        );
        for (harder, easier) in human_odds.iter().skip(1).zip(&human_odds) {
            assert!(
                harder < easier,
                "every rung up the ladder costs the human seat: {human_odds:?}"
            );
        }
        assert!(
            human_odds[7] < 0.01,
            "a Deity table is close to hopeless: {}",
            human_odds[7]
        );
        assert!(
            human_odds[0] > 0.35,
            "a Settler table favours the human: {}",
            human_odds[0]
        );
    }

    /// Difficulty is a bargain between the human and the AI seats. With nobody
    /// human it is the same for everybody and cannot tilt the table.
    #[test]
    fn an_all_ai_table_is_even_at_every_difficulty() {
        for difficulty in ["prince", "king", "deity"] {
            let game = even_table(4, difficulty, &[]);
            let odds = table(&game, flat);
            for seat in odds.values() {
                assert!(
                    (seat.start - 0.25).abs() < 1e-9,
                    "{difficulty}: {:?}",
                    seat
                );
            }
        }
    }

    /// A stronger rating is favoured before a tile is drawn, by the same
    /// arithmetic the league rates results with.
    #[test]
    fn a_stronger_rating_starts_ahead() {
        let game = even_table(2, "prince", &[]);
        let odds = table(&game, |pid| if pid == 0 { 1700.0 } else { 1500.0 });
        assert!(odds[&0].start > odds[&1].start);
        assert!(
            (odds[&0].start - crate::elo::expected(1700.0, 1500.0)).abs() < 1e-9,
            "a two-seat table is the ordinary Elo question: {:?}",
            odds[&0]
        );
    }

    /// The start odds are a prediction, so the board may not touch them. The
    /// now odds must move with the same evidence that leaves them alone.
    #[test]
    fn a_standing_lead_moves_only_the_now_odds() {
        let mut game = even_table(4, "prince", &[]);
        let before = table(&game, flat);
        for pid in 0..4 {
            found_capital(&mut game, pid);
        }
        game.turn = 120;
        game.players[1].era_score += 400;
        let after = table(&game, flat);
        for pid in 0..4 {
            assert!(
                (after[&pid].start - before[&pid].start).abs() < 1e-9,
                "seat {pid}'s start odds moved: {} -> {}",
                before[&pid].start,
                after[&pid].start
            );
        }
        assert!(
            after[&1].now > before[&1].now,
            "the empire that pulled ahead is now favoured: {} -> {}",
            before[&1].now,
            after[&1].now
        );
        assert!(after[&1].standing_elo > 0.0);
        assert!(after[&0].now < before[&0].now, "and the field gives it up");
        assert!((total(&after, |seat| seat.now) - 1.0).abs() < 1e-9);
    }

    /// Losing every city is a collapse the odds have to say out loud, even
    /// while the seat is still technically alive.
    #[test]
    fn an_empire_with_no_cities_is_written_off_while_it_lives() {
        let mut game = even_table(3, "prince", &[]);
        for pid in 0..3 {
            found_capital(&mut game, pid);
        }
        game.turn = 90;
        let cities: Vec<_> = game
            .cities
            .iter()
            .filter(|(_, city)| city.owner == 2)
            .map(|(id, _)| *id)
            .collect();
        for city in cities {
            game.cities.remove(&city);
        }
        let odds = table(&game, flat);
        assert!(game.players[2].alive, "the seat has not been eliminated yet");
        assert!(
            odds[&2].now < 0.12,
            "and it is marked well down while it still holds a settler: {:?}",
            odds[&2]
        );
        assert!(odds[&2].now > 0.0, "a settler can refound: {:?}", odds[&2]);
        assert!((total(&odds, |seat| seat.now) - 1.0).abs() < 1e-9);

        // Now take the settler too. Nothing left to found with is the engine's
        // own definition of defeat, so the seat is written off outright rather
        // than left holding a few per cent until something checks.
        let settlers: Vec<u32> = game
            .units
            .iter()
            .filter(|(_, unit)| unit.owner == 2 && unit.kind == "settler")
            .map(|(id, _)| *id)
            .collect();
        assert!(!settlers.is_empty(), "the seat had a settler to lose");
        for settler in settlers {
            game.units.remove(&settler);
        }
        let odds = table(&game, flat);
        assert_eq!(odds[&2].now, 0.0);
        assert!(game.players[2].alive, "the engine has not checked yet");
        assert!((total(&odds, |seat| seat.now) - 1.0).abs() < 1e-9);
    }

    /// An eliminated seat cannot win, and the living share out the whole win
    /// between them. Its start odds stand: that is what was predicted.
    #[test]
    fn elimination_zeroes_a_seat_and_the_living_share_the_whole_win() {
        let mut game = even_table(4, "prince", &[]);
        let predicted = table(&game, flat)[&3].start;
        game.players[3].alive = false;
        let odds = table(&game, flat);
        assert_eq!(odds[&3].now, 0.0);
        assert_eq!(odds[&3].start, predicted);
        assert!((total(&odds, |seat| seat.now) - 1.0).abs() < 1e-9);
        for pid in 0..3 {
            assert!((odds[&pid].now - 1.0 / 3.0).abs() < 0.03, "{:?}", odds[&pid]);
        }
    }

    /// The last empire standing has already won.
    #[test]
    fn the_last_survivor_holds_the_whole_win() {
        let mut game = even_table(3, "prince", &[]);
        game.players[1].alive = false;
        game.players[2].alive = false;
        let odds = table(&game, flat);
        assert_eq!(odds[&0].now, 1.0);
        assert_eq!(odds[&1].now, 0.0);
    }

    /// A decided game is not a forecast.
    #[test]
    fn a_finished_game_is_certain() {
        let mut game = even_table(4, "prince", &[]);
        game.winner = Some(2);
        game.victory_type = Some("science".to_string());
        let odds = table(&game, flat);
        assert_eq!(odds[&2].now, 1.0);
        assert_eq!(odds[&0].now, 0.0);
        assert!(
            odds[&2].start < 0.3,
            "and the prediction it beat is left intact: {:?}",
            odds[&2]
        );
    }

    /// A race at its threshold ends the game, so the seat running it away is
    /// nearly certain long before the clock runs out.
    #[test]
    fn a_race_near_its_threshold_all_but_settles_the_game() {
        let mut game = even_table(4, "prince", &[]);
        for pid in 0..4 {
            found_capital(&mut game, pid);
        }
        game.turn = 150;
        let even = table(&game, flat);
        assert!(
            even[&0].now < 0.45,
            "a capital each is not a decided game: {:?}",
            even[&0]
        );
        game.players[0].dvp = 19;
        let racing = table(&game, flat);
        assert!(
            racing[&0].race_pct > 90.0,
            "the Diplomatic race is all but finished: {:?}",
            racing[&0]
        );
        assert!(
            racing[&0].now > 0.9,
            "so the seat running it is nearly certain: {:?}",
            racing[&0]
        );
        assert!((total(&racing, |seat| seat.now) - 1.0).abs() < 1e-9);
    }

    /// The same lead is worth more with less clock left to overturn it.
    #[test]
    fn the_same_lead_is_worth_more_late() {
        let mut game = even_table(4, "prince", &[]);
        for pid in 0..4 {
            found_capital(&mut game, pid);
        }
        game.players[1].era_score += 200;
        game.turn = 40;
        let early = table(&game, flat)[&1].now;
        game.turn = 240;
        let late = table(&game, flat)[&1].now;
        assert!(late > early, "early {early} should be softer than late {late}");
    }

    /// A team wins together, so both members carry the side's chance rather
    /// than half of it, and the table sums to one across the two sides.
    #[test]
    fn teammates_carry_the_whole_side() {
        let mut options = GameOptions::new(4, 32, 22, 91_137, 250, 0);
        options.barbarians = false;
        options.teams = vec![Some(0), Some(0), Some(1), Some(1)];
        let game = Game::new_with(options);
        let odds = table(&game, flat);
        assert!(
            (odds[&0].start - 0.5).abs() < 1e-9,
            "an even two-a-side table is a coin toss: {:?}",
            odds[&0]
        );
        assert!((odds[&0].start - odds[&1].start).abs() < 1e-9);
        assert!((total(&odds, |seat| seat.start) - 2.0).abs() < 1e-9);
    }

    /// Score the live figure against played games, because a probability that
    /// is never checked is only a decoration.
    ///
    /// Twenty-four games are played to a result. At 60% of each one's clock
    /// every living seat's now odds are recorded, and the whole set is scored
    /// with a Brier score against what actually happened — squared error
    /// between the probability and a 1/0 outcome, lower being better. The
    /// baseline is the only honest one available: the uniform prior that every
    /// seat at a four-seat table has a quarter of the win, which is also what
    /// the start odds say about an even table. The model has to beat it, and
    /// its favourite has to win more often than a random seat would.
    ///
    /// Ignored by default: two dozen games to a result is a minute of CPU, far
    /// too long for the ordinary suite. Run it with
    /// `cargo test --profile ci --lib odds:: -- --ignored --nocapture`.
    #[test]
    #[ignore = "plays 24 games to a result"]
    fn the_now_odds_beat_a_uniform_prior_over_played_games() {
        use crate::ai::{run_game, Ai, BasicAi};
        use crate::game::Action;

        const GAMES: u64 = 24;
        const PLAYERS: usize = 4;
        const TURNS: u32 = 220;
        let mut samples: Vec<(f64, bool)> = Vec::new();
        let mut favourite_wins = 0_u32;
        let mut decided = 0_u32;
        for seed in 0..GAMES {
            let mut options = GameOptions::new(PLAYERS, 32, 22, 90_000 + seed, TURNS, 2);
            options.difficulty = "prince".to_string();
            let mut game = Game::new_with(options);
            let mut ais = BasicAi::fleet(&game);
            let sample_at = (TURNS as f64 * 0.6) as u32;
            let mut sampled: Option<BTreeMap<usize, SeatOdds>> = None;
            game.set_fog_memory(false);
            while game.winner.is_none() && game.turn <= game.max_turns {
                if sampled.is_none() && game.turn >= sample_at {
                    sampled = Some(table(&game, flat));
                }
                let pid = game.current;
                ais[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }
            let Some(odds) = sampled else { continue };
            let winners = game.winning_players();
            if winners.is_empty() {
                // No result inside the turn limit: there is nothing to score
                // this sample against, so it is dropped rather than counted as
                // everybody having lost.
                continue;
            }
            decided += 1;
            for (pid, seat) in &odds {
                samples.push((seat.now, winners.contains(pid)));
            }
            let favourite = odds
                .iter()
                .max_by(|a, b| a.1.now.total_cmp(&b.1.now))
                .map(|(pid, _)| *pid)
                .expect("a table has seats");
            favourite_wins += u32::from(winners.contains(&favourite));
        }
        assert!(decided >= GAMES as u32 / 2, "only {decided} games reached a result");
        let brier = samples.iter().map(|(p, won)| {
            let outcome = f64::from(*won);
            (p - outcome) * (p - outcome)
        }).sum::<f64>() / samples.len() as f64;
        let uniform = 1.0 / PLAYERS as f64;
        let baseline = samples.iter().map(|(_, won)| {
            let outcome = f64::from(*won);
            (uniform - outcome) * (uniform - outcome)
        }).sum::<f64>() / samples.len() as f64;
        let favourite_rate = f64::from(favourite_wins) / f64::from(decided);
        println!(
            "{decided} decided games, {} seat samples: Brier {brier:.4} vs uniform {baseline:.4}, \
             favourite won {favourite_wins}/{decided} ({:.0}%)",
            samples.len(),
            favourite_rate * 100.0
        );
        assert!(
            brier < baseline,
            "the live odds must beat a flat quarter: {brier:.4} against {baseline:.4}"
        );
        assert!(
            favourite_rate > uniform,
            "the favourite at 60% of the clock must beat a coin drawn from the table: \
             {favourite_wins}/{decided}"
        );
    }

    /// The civilization edge is learned from the roster, weighted by the games
    /// behind it, and shrunk toward nothing when there are few.
    #[test]
    fn the_civ_edge_is_learned_and_shrunk_by_its_evidence() {
        let mut league = League {
            round: 1,
            strategies: Vec::new(),
            calibration: Default::default(),
        };
        let mut player = Strategy::new(
            "test",
            StrategyKind::Builtin {
                ai: "basic".to_string(),
            },
            1,
        );
        player.rating = 1500.0;
        player.games = 400;
        for (civ, rating, games) in [("Rome", 1700.0, 400_u32), ("Norway", 1700.0, 2_u32)] {
            player
                .leader_elo
                .entry("Leader".to_string())
                .or_default()
                .insert(
                    civ.to_string(),
                    crate::league::CivRating {
                        rating,
                        games,
                        ..Default::default()
                    },
                );
        }
        league.strategies.push(player);
        let rome = civ_edge_elo(&league, "Rome");
        let norway = civ_edge_elo(&league, "Norway");
        assert!(rome > 150.0, "400 games of a +200 edge is nearly all of it: {rome}");
        assert!(norway < 25.0, "two games of the same edge is almost none: {norway}");
        assert_eq!(civ_edge_elo(&league, "Sumeria"), 0.0, "and none is none");
    }
}
