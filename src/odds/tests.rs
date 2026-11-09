use super::*;
use crate::game::{Game, GameOptions};
use std::collections::BTreeSet;

#[path = "fog_tests.rs"]
mod fog_tests;

#[path = "prior_tests.rs"]
mod prior_tests;

#[path = "clock_tests.rs"]
mod clock_tests;

fn arena(cities: u8) -> Game {
    let mut options = GameOptions::new(2, 10, 10, 90_411, 250, 0);
    options.map_script = crate::setup::MapScript::Battlefield;
    options.tactics = crate::setup::TacticsRules {
        cities,
        ..Default::default()
    };
    Game::new_with(options)
}

/// An arena that has been all but decided must say so. Before the arena
/// race meter existed the model read cities, capitals and Civ-scale
/// material floors — all silent on a battlefield — and a twelve-to-one
/// battle still read as contested, so the Mercy Rule could never fire on
/// the one mode whose games most reliably outlive their result.
#[test]
fn a_shattered_arena_army_concedes_the_odds() {
    let mut game = arena(0);
    // Reduce one side to a single wounded warrior, by the book: units are
    // removed through the same call combat resolution uses.
    let losing: Vec<u32> = game
        .units
        .values()
        .filter(|unit| unit.owner == 1)
        .map(|unit| unit.id)
        .collect();
    for unit in losing.iter().skip(1) {
        game.remove_unit(*unit);
    }
    game.units.get_mut(&losing[0]).unwrap().hp = 20;
    // Mid-battle on the clock, not the opening: the standing multiplier
    // sharpens with progress, and a decided battle is decided mid-game.
    game.turn = 150;

    let table = table(&game, |_pid| 1500.0f64);
    assert!(
        table[&0].now > 0.95,
        "twelve fresh units against one wounded warrior reads {:.3}",
        table[&0].now
    );
    assert_eq!(
        mercy_leader(&game, 0.95),
        Some(0),
        "the Mercy Rule crowns the side that holds the field"
    );
}

/// Even arena armies are an even table: the race meter reads half each,
/// so nothing separates the sides but their priors.
#[test]
fn an_even_arena_is_an_even_table() {
    let mut game = arena(0);
    game.turn = 150;
    let table = table(&game, |_pid| 1500.0f64);
    assert!(
        (table[&0].now - table[&1].now).abs() < 0.02,
        "even armies read {:.3} against {:.3}",
        table[&0].now,
        table[&1].now
    );
    assert_eq!(mercy_leader(&game, 0.95), None);
}

/// The arena plays domination whatever the lobby checkboxes say, and the
/// odds must speak the same rule: a battlefield launched with only the
/// world lanes ticked still has a winner coming, so its table is not
/// zeroed and its race meter still moves.
#[test]
fn arena_odds_survive_a_lobby_that_switched_the_world_lanes_off() {
    let mut game = arena(0);
    game.victory_conditions = crate::game::VictoryConditions {
        science: true,
        culture: false,
        religious: false,
        diplomatic: false,
        domination: false,
        score: false,
    };
    let losing: Vec<u32> = game
        .units
        .values()
        .filter(|unit| unit.owner == 1)
        .map(|unit| unit.id)
        .collect();
    for unit in losing.iter().skip(1) {
        game.remove_unit(*unit);
    }
    game.turn = 150;
    let table = table(&game, |_pid| 1500.0f64);
    assert!(
        table[&0].now > 0.95,
        "the arena's own lanes carry the odds, not the checkboxes: {:.3}",
        table[&0].now
    );
}

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
    let ladder = [
        "settler",
        "chieftain",
        "warlord",
        "prince",
        "king",
        "emperor",
        "immortal",
        "deity",
    ];
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
            assert!((seat.start - 0.25).abs() < 1e-9, "{difficulty}: {:?}", seat);
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

/// Rating deviation belongs in a forecast, not just beside it in the UI.
/// Two barely measured ratings may have different midpoints without
/// supporting the same confidence as two settled ones.
#[test]
fn rating_uncertainty_tempers_the_start_forecast() {
    let game = even_table(2, "prince", &[]);
    let settled = table(&game, |pid| if pid == 0 { 1700.0 } else { 1500.0 });
    let uncertain = table(&game, |pid| {
        PriorRating::new(if pid == 0 { 1700.0 } else { 1500.0 }, 350.0)
    });
    assert!(uncertain[&0].start > 0.5);
    assert!(
        uncertain[&0].start < settled[&0].start,
        "uncertain {:?} should be less emphatic than settled {:?}",
        uncertain[&0],
        settled[&0]
    );
    assert!((total(&uncertain, |seat| seat.start) - 1.0).abs() < 1e-9);
}

/// The lobby can switch off every victory path. That world ends without a
/// winner, so forcing one unit of probability across its seats would claim
/// somebody can achieve an outcome the rules prohibit.
#[test]
fn no_enabled_victory_means_no_win_probability() {
    let mut game = even_table(4, "prince", &[]);
    game.victory_conditions.science = false;
    game.victory_conditions.culture = false;
    game.victory_conditions.religious = false;
    game.victory_conditions.diplomatic = false;
    game.victory_conditions.domination = false;
    game.victory_conditions.score = false;
    let odds = table(&game, flat);
    for seat in odds.values() {
        assert_eq!(seat.start, 0.0);
        assert_eq!(seat.now, 0.0);
    }
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
    assert!(
        game.players[2].alive,
        "the seat has not been eliminated yet"
    );
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
        assert!(
            (odds[&pid].now - 1.0 / 3.0).abs() < 0.03,
            "{:?}",
            odds[&pid]
        );
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

/// The Science meter reads nothing until the first launch, so before then the
/// road to that victory is the tech lead, and it counts only while Science can
/// still end the game.
#[test]
fn a_tech_lead_is_the_road_to_science() {
    let mut game = even_table(4, "prince", &[]);
    for pid in 0..4 {
        found_capital(&mut game, pid);
    }
    game.turn = 120;
    let techs: Vec<_> = game.rules.techs.keys().take(24).cloned().collect();
    game.players[1].techs.extend(techs);
    let science_open = table(&game, flat);
    assert_eq!(
        game.victory_races(1, 0).science,
        0.0,
        "no launch yet, so the meter cannot see it"
    );
    game.victory_conditions.science = false;
    let science_closed = table(&game, flat);
    assert!(
        science_open[&1].now > science_closed[&1].now,
        "with Science open the tech lead is worth more: {} vs {}",
        science_open[&1].now,
        science_closed[&1].now
    );
    assert!((total(&science_open, |seat| seat.now) - 1.0).abs() < 1e-9);
}

/// Religious and Diplomatic races really do end games, so their meters are read
/// as they stand. Squaring them looked right on Deity host games, where neither
/// race ever finished, and cost the eventual winner about twenty points in the
/// last thirty turns of the simulated games those races decided.
#[test]
fn a_contested_race_is_read_at_face_value() {
    let mut game = even_table(4, "prince", &[]);
    for pid in 0..4 {
        found_capital(&mut game, pid);
    }
    game.turn = 150;
    game.players[0].dvp = 15;
    assert_eq!(closest_victory(&game, 0, 0).earned_pct, 75.0);
    let odds = table(&game, flat);
    assert!(
        odds[&0].now > 0.85,
        "three quarters of a race that ends the game is close to it: {}",
        odds[&0].now
    );
}

/// Holding your own capital is where every seat starts. It is not a sixth of a
/// Domination victory, and finishing the race must still read as finished.
#[test]
fn the_share_every_seat_holds_for_free_is_not_progress() {
    assert_eq!(beyond_free_share(25.0, 25.0), 0.0);
    assert_eq!(beyond_free_share(10.0, 25.0), 0.0, "a lost capital is not negative");
    assert!((beyond_free_share(50.0, 25.0) - 100.0 / 3.0).abs() < 1e-9);
    assert_eq!(beyond_free_share(100.0, 25.0), 100.0);
    assert_eq!(beyond_free_share(40.0, 100.0), 40.0, "no share left to remove");
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
    assert!(
        late > early,
        "early {early} should be softer than late {late}"
    );
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

/// A defeated teammate is still credited if the surviving member later
/// wins. Live odds answer that exact "among the winners" question too.
#[test]
fn a_defeated_teammate_keeps_the_surviving_sides_chance() {
    let mut options = GameOptions::new(4, 32, 22, 91_138, 250, 0);
    options.barbarians = false;
    options.teams = vec![Some(0), Some(0), Some(1), Some(1)];
    let mut game = Game::new_with(options);
    game.players[1].alive = false;
    let odds = table(&game, flat);
    assert!(odds[&0].now > 0.0);
    assert_eq!(odds[&1].now, odds[&0].now);
    assert_eq!(odds[&2].now, odds[&3].now);
    assert!((odds[&0].now + odds[&2].now - 1.0).abs() < 1e-9);
}

/// Score the live figure throughout played games, because a probability
/// that is never checked is only a decoration.
///
/// The old audit took one sample at 60% of the clock and only asked it to
/// beat a uniform quarter. That can bless a badly calibrated model: making
/// the right seat a 99% favourite half the time and the wrong one the other
/// half still ranks better than random while publishing terrible odds. It
/// also says nothing about the opening or the late game. This audit samples
/// every surviving game at 25%, 50% and 75%, and scores both Brier error over
/// all seats and log loss on the eventual winner against the fixed start
/// forecast. Ranking and calibration therefore have to improve together.
///
/// Ignored by default: playing the games to a result is too slow for the
/// ordinary suite. Run it with `cargo test --profile ci --lib
/// the_now_odds_beat_the_start_forecast -- --ignored --nocapture`.
#[test]
#[ignore = "plays 24 games to a result"]
fn the_now_odds_beat_the_start_forecast() {
    use crate::ai::{Ai, BasicAi};
    use crate::game::Action;

    const GAMES: u64 = 24;
    const PLAYERS: usize = 4;
    const TURNS: u32 = 220;
    const PHASES: [f64; 3] = [0.25, 0.50, 0.75];
    #[derive(Default)]
    struct Score {
        games: u32,
        seats: u32,
        now_brier: f64,
        start_brier: f64,
        now_log_loss: f64,
        start_log_loss: f64,
        favourite_wins: u32,
    }
    let mut scores: [Score; PHASES.len()] = Default::default();
    let seed_offset = std::env::var("CIVVIS_ODDS_SEED_OFFSET")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    for seed in seed_offset..seed_offset + GAMES {
        let mut options = GameOptions::new(PLAYERS, 32, 22, 90_000 + seed, TURNS, 2);
        options.difficulty = "prince".to_string();
        let mut game = Game::new_with(options);
        let mut ais = BasicAi::fleet(&game);
        let mut sampled: [Option<BTreeMap<usize, SeatOdds>>; PHASES.len()] = Default::default();
        game.set_fog_memory(false);
        while game.winner.is_none() && game.turn <= game.max_turns {
            for (index, phase) in PHASES.iter().enumerate() {
                if sampled[index].is_none() && game.turn >= (TURNS as f64 * phase).round() as u32 {
                    sampled[index] = Some(table(&game, flat));
                }
            }
            let pid = game.current;
            ais[pid].take_turn(&mut game, pid);
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
        }
        let winners = game.winning_players();
        if winners.is_empty() {
            continue;
        }
        for (index, odds) in sampled.into_iter().enumerate() {
            let Some(odds) = odds else { continue };
            let score = &mut scores[index];
            score.games += 1;
            let favourite = odds
                .iter()
                .max_by(|a, b| a.1.now.total_cmp(&b.1.now))
                .map(|(pid, _)| *pid)
                .expect("a table has seats");
            score.favourite_wins += u32::from(winners.contains(&favourite));
            let mut winner_now = 0.0;
            let mut winner_start = 0.0;
            for (pid, seat) in &odds {
                let outcome = f64::from(winners.contains(pid));
                score.now_brier += (seat.now - outcome) * (seat.now - outcome);
                score.start_brier += (seat.start - outcome) * (seat.start - outcome);
                score.seats += 1;
                if outcome > 0.0 {
                    winner_now += seat.now;
                    winner_start += seat.start;
                }
            }
            score.now_log_loss -= winner_now.max(1e-12).ln();
            score.start_log_loss -= winner_start.max(1e-12).ln();
        }
    }
    for (phase, score) in PHASES.iter().zip(&scores) {
        assert!(
            score.games >= GAMES as u32 / 2,
            "only {} games reached the {:.0}% sample and a result",
            score.games,
            phase * 100.0
        );
        let now_brier = score.now_brier / f64::from(score.seats);
        let start_brier = score.start_brier / f64::from(score.seats);
        let now_log_loss = score.now_log_loss / f64::from(score.games);
        let start_log_loss = score.start_log_loss / f64::from(score.games);
        let favourite_rate = f64::from(score.favourite_wins) / f64::from(score.games);
        println!(
            "{:.0}%: {} games / {} seats, Brier now {now_brier:.4} vs start \
                 {start_brier:.4}, log loss now {now_log_loss:.4} vs start \
                 {start_log_loss:.4}, favourite {}/{} ({:.0}%)",
            phase * 100.0,
            score.games,
            score.seats,
            score.favourite_wins,
            score.games,
            favourite_rate * 100.0,
        );
        assert!(
            now_brier < start_brier,
            "at {:.0}%, live Brier {now_brier:.4} did not beat start {start_brier:.4}",
            phase * 100.0
        );
        assert!(
            now_log_loss < start_log_loss,
            "at {:.0}%, live log loss {now_log_loss:.4} did not beat start {start_log_loss:.4}",
            phase * 100.0
        );
    }
}
