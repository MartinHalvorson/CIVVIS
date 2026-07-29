//! Returns for actions actually taken: the dataset a Q or advantage head needs.
//!
//! This exists because of a measured dead end. `PolicyAi` ranks candidate
//! actions by the *state* value of the position each one leads to, and that
//! cannot be made to work by improving the estimate. The net is already
//! accurate — validation BCE 0.3685 against 0.5623 for a constant predictor,
//! and better calibrated on the states this agent reaches (ECE 0.064) than on
//! the expert's (0.103). It still loses 313 Elo when it is given features fine
//! enough to tell candidates apart (`policy_wide`, 34/240 against `advanced`).
//!
//! The reason is in `src/policy.rs` and it is a property of the target, not of
//! the fit: a net trained on outcomes encodes **correlation**, so an argmax
//! over sibling actions optimises whichever correlate is cheapest to move. Over
//! 468 committed decisions the chosen action raised the adjacent-enemy count by
//! 0.137 where the average legal candidate raised it by 0.002 — seventy-eight
//! times the field — while own HP-weighted material fell. Units stand in
//! contact in games `advanced` *wins* because a strong empire is pressing an
//! attack. Contact is a symptom of strength; maximising a symptom walks units
//! into fights they lose. Freezing exactly those two terms recovers the whole
//! loss (`policy_wide_frozen`, 120/240, Elo 0), which is as clean a
//! confirmation as this codebase has produced.
//!
//! So ranking actions needs a signal about *taking* the action rather than
//! about the position it lands in — Q, or advantage, trained on returns for
//! actions actually taken. Nothing in the repository emits that. This does.
//!
//! ```text
//! q_dataset --games 40 --players 4 --turns 200 --out evolved/q.csv
//! ```
//!
//! **How it records without disturbing what it records.** The agent applies its
//! own actions internally, so there is no callback to hang this on. Instead the
//! game is played once at full speed, and then *replayed* from `game.log` —
//! which is a complete replay record by construction — against a fresh game of
//! the same seed. At each applied action the state and the action are encoded
//! from the position as it stood *before* the action, which is exactly the pair
//! a Q head is asked to score. Replay costs no AI thinking, so the second pass
//! is a small fraction of the first.
//!
//! **Rows are grouped by game and the group is emitted, because splitting this
//! by row is a trap this repository has already fallen into**: a per-sample
//! split of the value-net data reported 98.8% accuracy where a per-game split
//! reported 75.0%. Positions within one game share an outcome and are highly
//! correlated; any split that mixes them measures memorisation. `game` is the
//! first column so the trainer can group on it, and it is a globally unique id
//! rather than an index into this run.
//!
//! **Negatives are optional and are drawn from the legal set.** With
//! `--negatives K` each decision also emits up to K actions that were legal and
//! *not* taken, labelled with the same return and marked `chosen=0`. A Q head
//! can be trained on the chosen rows alone; an advantage or a pairwise ranker
//! needs the siblings, and they cost a `legal_actions` call per decision, so
//! they are opt-in.
use civvis::action_space;
use civvis::ai::{run_game, AdvancedAi};
use civvis::decision_features::{decision_features, WIDTH as STATE_WIDTH};
use civvis::game::{Action, Game};
use civvis::parallel;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn text(args: &[String], flag: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

/// What one game produced: its rows, and the counts the report is built from.
struct Harvest {
    rows: String,
    decisions: usize,
    negatives: usize,
    wins: usize,
    turns: u32,
    finished: bool,
    /// Applications the replay refused. Must be zero: a replay that diverges
    /// from the game it is replaying encodes states that never happened, and
    /// every row after the first divergence is fiction. This is the one number
    /// that decides whether the file is data or noise.
    rejected: usize,
    /// Chosen rows whose label is a win. Counted while writing rather than
    /// derived from the seat count afterwards: winners take more actions than
    /// losers because a bigger empire has more to move, so the share of *rows*
    /// carrying a win is not the share of *seats* that won, and a trainer
    /// weighting its classes needs the row number.
    winning_rows: usize,
    /// Whether the replay ended on the same score line as the played game.
    /// Cheaper than comparing whole states and it catches a silent divergence
    /// that still applied cleanly.
    agrees: bool,
}

/// The outcome a decision is labelled with.
///
/// Win is the target that matters, because winning is what the promotion gate
/// measures and a head trained on anything else inherits the objective split
/// that `evolve` already carries. Score share rides along as a second column
/// so a denser regression target is available without regenerating the data:
/// wins are rare in a four-player game and a head fit to them alone sees three
/// zeros for every one.
fn outcomes(game: &Game, seats: usize) -> Vec<(f32, f32)> {
    let scores: Vec<i64> = (0..seats).map(|pid| game.score(pid)).collect();
    let total: i64 = scores.iter().sum::<i64>().max(1);
    (0..seats)
        .map(|pid| {
            let won = matches!(game.winner, Some(w) if w == pid);
            (won as u8 as f32, scores[pid] as f32 / total as f32)
        })
        .collect()
}

fn harvest(
    game_id: u64,
    seats: usize,
    width: i32,
    height: i32,
    turns: u32,
    city_states: usize,
    negatives: usize,
    same_kind: bool,
) -> Harvest {
    // Pass one: play it. Nothing is recorded here beyond the log the engine
    // keeps anyway, so the agents behave exactly as they do in any other run.
    let mut played = Game::new(seats, width, height, game_id, turns, city_states);
    let mut fleet = AdvancedAi::fleet(&played);
    run_game(&mut played, &mut fleet);
    let label = outcomes(&played, seats);
    let finished = played.winner.is_some();
    let played_turns = played.turn;
    let wins = label.iter().filter(|(won, _)| *won > 0.5).count();

    // Pass two: replay the log against a fresh game of the same seed, encoding
    // each action against the position as it stood before it was applied.
    let mut replay = Game::new(seats, width, height, game_id, turns, city_states);
    replay.set_fog_memory(false);
    let log: Vec<(usize, Action)> = played.log.iter().cloned().collect();
    let mut rows = String::new();
    let mut decisions = 0usize;
    let mut emitted_negatives = 0usize;
    let mut rejected = 0usize;
    let mut winning_rows = 0usize;
    let mut picker = game_id.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;

    for (seat, action) in log {
        // EndTurn is the loop's exit rather than a choice, and every turn has
        // exactly one, so scoring it teaches a head only how long a game is.
        if matches!(action, Action::EndTurn) || seat >= seats {
            rejected += replay.apply(seat, &action).is_err() as usize;
            continue;
        }
        let (won, share) = label[seat];
        let state = decision_features(&replay, seat);
        let chosen = action_space::features(&replay, seat, &action);
        write_row(
            &mut rows, game_id, replay.turn, seat, 1, &state, &chosen, won, share,
        );
        decisions += 1;
        winning_rows += (won > 0.5) as usize;

        if negatives > 0 {
            // Siblings are drawn from the legal set with the taken action
            // removed. A stride rather than a prefix, because `legal_actions`
            // is ordered by unit and a prefix would sample one unit's options
            // over and over.
            let legal = replay.legal_actions(seat);
            // Siblings of the *same kind* when asked for. A head trained on
            // mixed-kind negatives learns which kind of action the expert
            // takes, which is real but coarse: it cannot choose between two
            // moves. Restricting the negatives to the chosen action's own kind
            // makes the kind one-hot constant across the group, so the only
            // thing left to learn from is the geometry — which is the signal a
            // search prior over sibling moves actually needs. Measured on
            // stride-sampled data, only 103 of 531,892 decisions came out
            // same-kind by chance, which is why this is a switch and not a
            // filter applied afterwards.
            let wanted = action_space::kind_name(&action);
            let others: Vec<&Action> = legal
                .iter()
                .filter(|candidate| **candidate != action && !matches!(candidate, Action::EndTurn))
                .filter(|candidate| !same_kind || action_space::kind_name(candidate) == wanted)
                .collect();
            if !others.is_empty() {
                let stride = (others.len() / negatives.max(1)).max(1);
                picker = picker
                    .wrapping_add(0x9E37_79B9_7F4A_7C15)
                    .rotate_left(31)
                    .wrapping_mul(0xBF58_476D_1CE4_E5B9);
                let start = (picker >> 33) as usize % others.len();
                for step in 0..negatives.min(others.len()) {
                    let other = others[(start + step * stride) % others.len()];
                    let row = action_space::features(&replay, seat, other);
                    write_row(
                        &mut rows, game_id, replay.turn, seat, 0, &state, &row, won, share,
                    );
                    emitted_negatives += 1;
                }
            }
        }
        rejected += replay.apply(seat, &action).is_err() as usize;
    }

    let agrees = (0..seats).all(|pid| replay.score(pid) == played.score(pid));

    Harvest {
        rows,
        decisions,
        negatives: emitted_negatives,
        wins,
        turns: played_turns,
        finished,
        rejected,
        agrees,
        winning_rows,
    }
}

#[allow(clippy::too_many_arguments)]
fn write_row(
    out: &mut String,
    game: u64,
    turn: u32,
    seat: usize,
    chosen: u8,
    state: &[f32],
    action: &[f32],
    won: f32,
    share: f32,
) {
    let _ = write!(out, "{game},{turn},{seat},{chosen}");
    for value in state.iter().chain(action.iter()) {
        let _ = write!(out, ",{value:.5}");
    }
    let _ = writeln!(out, ",{won:.0},{share:.5}");
}

fn header() -> String {
    let mut head = String::from("game,turn,seat,chosen");
    for index in 0..STATE_WIDTH {
        let _ = write!(head, ",s{index}");
    }
    for index in 0..action_space::FEATURE_WIDTH {
        let _ = write!(head, ",a{index}");
    }
    head.push_str(",won,score_share\n");
    head
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let games = number(&args, "--games", 8);
    let seats = number(&args, "--players", 4);
    let width = number(&args, "--width", 44) as i32;
    let height = number(&args, "--height", 28) as i32;
    let turns = number(&args, "--turns", 200) as u32;
    let city_states = number(&args, "--city-states", 0);
    let negatives = number(&args, "--negatives", 0);
    let same_kind = args.iter().any(|arg| arg == "--negatives-same-kind");
    let seed = number(&args, "--seed", 90_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());
    let out = text(&args, "--out", "evolved/q_dataset.csv");

    println!(
        "q_dataset: {games} games, {seats} players, {width}x{height}, {turns} turns, \
         {negatives} negatives per decision{}, jobs {jobs}",
        if same_kind { " (same kind only)" } else { "" }
    );
    println!(
        "state {STATE_WIDTH} + action {} = {} features per row",
        action_space::FEATURE_WIDTH,
        STATE_WIDTH + action_space::FEATURE_WIDTH
    );

    let harvests = parallel::map(games, jobs, |index| {
        harvest(
            seed + index as u64,
            seats,
            width,
            height,
            turns,
            city_states,
            negatives,
            same_kind,
        )
    });

    if let Some(parent) = std::path::Path::new(&out).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
    let mut file = match fs::File::create(&out) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("cannot write {out}: {error}");
            std::process::exit(1);
        }
    };
    let _ = file.write_all(header().as_bytes());
    let mut decisions = 0usize;
    let mut negative_rows = 0usize;
    let mut winning_seats = 0usize;
    let mut finished = 0usize;
    let mut total_turns = 0u64;
    let mut rejected = 0usize;
    let mut disagreed = 0usize;
    let mut winning_rows = 0usize;
    for harvest in &harvests {
        let _ = file.write_all(harvest.rows.as_bytes());
        decisions += harvest.decisions;
        negative_rows += harvest.negatives;
        winning_seats += harvest.wins;
        finished += harvest.finished as usize;
        total_turns += harvest.turns as u64;
        rejected += harvest.rejected;
        disagreed += !harvest.agrees as usize;
        winning_rows += harvest.winning_rows;
    }

    let seat_games = games * seats;
    println!(
        "wrote {out}: {decisions} chosen rows, {negative_rows} negatives, \
         {} rows total",
        decisions + negative_rows
    );
    println!(
        "games: {games}, {finished} decided by victory, average {:.0} turns",
        total_turns as f64 / games.max(1) as f64
    );
    // Nothing downstream can recover from a divergent replay, so it fails here
    // rather than writing a file somebody trusts.
    println!("replay: {rejected} rejected applications, {disagreed}/{games} games ended on a different score");
    if rejected > 0 || disagreed > 0 {
        eprintln!(
            "q_dataset: replay diverged from the played game -- every row after a \
             divergence encodes a state that never happened. Refusing to claim this file."
        );
        std::process::exit(2);
    }
    // The label balance is the first thing that makes a head untrainable, so it
    // is reported rather than left for the trainer to discover. A run where no
    // game is decided has every `won` at zero and a head fit to it learns the
    // constant.
    println!(
        "label balance: {winning_seats}/{seat_games} seat-games won ({:.1}%), \
         {winning_rows}/{decisions} chosen rows carry a win ({:.1}%)",
        100.0 * winning_seats as f64 / seat_games.max(1) as f64,
        100.0 * winning_rows as f64 / decisions.max(1) as f64
    );
}

#[cfg(test)]
mod tests {
    use super::{harvest, header};
    use civvis::action_space;
    use civvis::decision_features::WIDTH as STATE_WIDTH;

    /// The whole method rests on one property: replaying `game.log` against a
    /// fresh game of the same seed reproduces the game. If it does not, every
    /// row encodes a position that never happened and the file is fiction that
    /// trains cleanly. The emitter refuses to write in that case; this pins
    /// that the case does not arise.
    #[test]
    fn replay_reproduces_the_game_it_records() {
        let run = harvest(4_242, 3, 24, 16, 40, 0, 0, false);
        assert_eq!(run.rejected, 0, "replay rejected an action the game applied");
        assert!(run.agrees, "replay ended on a different score line");
        assert!(run.decisions > 0, "a 40-turn game made no recordable decision");
    }

    /// Every row is the meta block, the state vector, the action vector and the
    /// two labels — and the header names exactly that many columns. A width
    /// that drifts from the header is the failure a trainer discovers as
    /// nonsense weeks later.
    #[test]
    fn rows_are_as_wide_as_the_header_says() {
        let expected = 4 + STATE_WIDTH + action_space::FEATURE_WIDTH + 2;
        assert_eq!(header().trim_end().split(',').count(), expected);
        let run = harvest(77, 3, 24, 16, 30, 0, 2, false);
        for line in run.rows.lines() {
            assert_eq!(line.split(',').count(), expected, "row width: {line:.60}");
        }
        assert!(run.negatives > 0, "--negatives 2 emitted no sibling rows");
    }
}
