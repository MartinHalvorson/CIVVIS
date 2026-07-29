//! Does `StrategicAi` search its two axes in the wrong order?
//!
//! The macro search chooses two things: a victory **lane** and a **doctrine**.
//! It chooses them one after the other. `lane_values` projects each lane using
//! `self.weights` — the doctrine currently in force — and `doctrine_values`
//! then projects each doctrine under the lane that won. That is coordinate
//! descent over two axes, and coordinate descent finds the joint optimum only
//! when the axes do not interact.
//!
//! They plausibly do. A Domination lane wants Militarize; a Science lane wants
//! Expand or Consolidate. If the incumbent doctrine is Consolidate, the
//! Domination lane is projected with consolidating weights — the worst version
//! of itself — and can lose a comparison it would win under the doctrine it
//! would actually be played with. The lane is then never chosen, so the
//! doctrine that would have rescued it is never offered.
//!
//! **This measures whether that happens before anything is built to fix it.**
//! Four doctrines against up to seven lanes is 28 joint branches where the
//! sequential search spends 11 — about 2.5×, which is inside the band this
//! repository has already measured as productive (doubling the search wins at
//! p=0.0023; quadrupling adds nothing). That is a reasonable price for a
//! structural fix and a bad one for a null.
//!
//! ```text
//! joint_axes --games 24 --players 4 --turns 200 --reviews 6
//! ```
//!
//! **The whole matrix comes out of the shipped public API.** `doctrine_values(g,
//! pid, lane)` is exactly one column of it, so this changes nothing in
//! `strategic.rs` and measures the search that actually ships rather than a
//! reimplementation of it that could disagree.
//!
//! Three numbers decide it:
//!
//! - **disagreement rate** — how often the joint argmax names a different
//!   (lane, doctrine) pair than the sequential procedure. This is the headline:
//!   at zero, coordinate descent is already finding the joint optimum and the
//!   whole idea is dead for the price of one run.
//! - **value left on the table** — mean `V(joint) − V(sequential)` in the
//!   evaluator's own units, and how that compares with
//!   `DOCTRINE_COMMITMENT_MARGIN`. A gap smaller than the margin the agent
//!   already requires before switching is a gap it would refuse to act on
//!   anyway, which is the trap `doctrine_values`' own doc comment warns about.
//! - **interaction** — how often the best doctrine differs between lanes. This
//!   is the mechanism: if one doctrine dominates under every lane, the axes are
//!   separable and disagreement can only be noise.
//!
//! **A self-check runs alongside.** `lane_values` under the incumbent doctrine
//! must equal that row of the matrix built from `doctrine_values`, because both
//! are the same rollout. Any mismatch means the matrix is not the search's own
//! numbers and the run says so instead of reporting them.
use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::ai::VictoryTarget;
use civvis::game::{Action, Game};
use civvis::parallel;
use civvis::strategic::{Doctrine, StrategicAi};

/// The margins `StrategicAi` requires before it will act, mirrored from
/// `src/strategic.rs`. They are private there, so these are copies and the
/// self-check cannot catch drift in them — if that file changes these numbers,
/// change them here. They are the whole point of this probe: a joint optimum
/// worth less than the margin is a cell the agent finds and then declines.
const TARGET_COMMITMENT_MARGIN: f64 = 0.01;
const DOCTRINE_COMMITMENT_MARGIN: f64 = 0.002;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// One review point, fully projected.
#[derive(Clone)]
struct Review {
    /// `value[lane][doctrine]`, with the lane list alongside.
    lanes: Vec<Option<VictoryTarget>>,
    value: Vec<Vec<f64>>,
    /// The doctrine the probe was in. Retained for the self-check only; the
    /// analysis sweeps every incumbent instead of trusting this one.
    incumbent: usize,
    /// Largest absolute disagreement between `lane_values` and the matrix row
    /// it should reproduce.
    self_check: f64,
}

impl Review {
    /// What the shipped search picks: best lane under a given incumbent
    /// doctrine, then best doctrine under that lane.
    ///
    /// The incumbent is a parameter rather than `self.incumbent` because a
    /// probe that never reviews sits in `Doctrine::Incumbent` forever, while a
    /// real agent's doctrine drifts. Keying the baseline to the probe's own
    /// frozen state would measure the probe: if Incumbent happens to be a poor
    /// doctrine for a position, the sequential lane choice made under it is
    /// worse than a real agent's would be, and the joint gain is overstated.
    /// Every incumbent is evaluated instead, and the report carries the range.
    fn sequential_from(&self, incumbent: usize) -> (usize, usize) {
        let lane = (0..self.lanes.len())
            .max_by(|a, b| {
                self.value[*a][incumbent]
                    .partial_cmp(&self.value[*b][incumbent])
                    .unwrap()
            })
            .unwrap_or(0);
        let doctrine = (0..Doctrine::ALL.len())
            .max_by(|a, b| self.value[lane][*a].partial_cmp(&self.value[lane][*b]).unwrap())
            .unwrap_or(0);
        (lane, doctrine)
    }

    /// The best cell in the whole matrix.
    fn joint(&self) -> (usize, usize) {
        let mut best = (0usize, 0usize);
        for lane in 0..self.lanes.len() {
            for doctrine in 0..Doctrine::ALL.len() {
                if self.value[lane][doctrine] > self.value[best.0][best.1] {
                    best = (lane, doctrine);
                }
            }
        }
        best
    }

    /// Whether the best doctrine is the same under every lane. If it is, the
    /// axes are separable and coordinate descent cannot be losing anything
    /// structural.
    fn doctrine_depends_on_lane(&self) -> bool {
        let best_for = |lane: usize| {
            (0..Doctrine::ALL.len())
                .max_by(|a, b| self.value[lane][*a].partial_cmp(&self.value[lane][*b]).unwrap())
                .unwrap_or(0)
        };
        let first = best_for(0);
        (1..self.lanes.len()).any(|lane| best_for(lane) != first)
    }
}

/// Play one game, stopping at `reviews` evenly spaced turns to project the
/// whole matrix from the position actually reached.
///
/// The projections are read off a *clone* and thrown away, so the game the
/// agents play is the game they would have played without this instrument
/// watching. A probe that steered the trajectory it sampled would be measuring
/// itself.
fn examine(
    seed: u64,
    seats: usize,
    width: i32,
    height: i32,
    turns: u32,
    reviews: usize,
) -> Vec<Review> {
    let mut game = Game::new(seats, width, height, seed, turns, 0);
    // The trajectory is played by the stock fleet and only *probed* by a
    // StrategicAi. A fleet of StrategicAi seats would run its own rollouts on
    // every turn of every game, which put a twelve-game run past ten minutes
    // and buys nothing here: what is being measured is a property of the value
    // matrix at a position, not of who walked to that position. The limit is
    // real and is stated in the report — these are AdvancedAi trajectories, and
    // StrategicAi is AdvancedAi plus lane commitment, so they are close but not
    // identical.
    let mut fleet = AdvancedAi::fleet(&game);
    let probe = StrategicAi::with_weights(Weights::default());

    // Spread the sample over the middle of the game: turn 1 has nothing to
    // project and the last turns are decided.
    let first = turns / 6;
    let last = turns - turns / 6;
    let step = ((last - first) as usize / reviews.max(1)).max(1);
    let mut next = first;
    let mut out = Vec::new();

    game.set_fog_memory(false);
    while game.winner.is_none() && game.turn <= game.max_turns {
        let pid = game.current;
        let major = !game.players[pid].is_minor && !game.players[pid].is_barbarian;
        if pid == 0 && major && game.turn >= next && out.len() < reviews {
            next = game.turn + step as u32;
            if let Some(review) = project(&probe, &game, pid) {
                out.push(review);
            }
        }
        fleet[pid].take_turn(&mut game, pid);
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
    }
    out
}

fn project(probe: &StrategicAi, game: &Game, pid: usize) -> Option<Review> {
    let snapshot = game.clone();
    let rows = probe.lane_values(&snapshot, pid);
    if rows.len() < 2 {
        return None;
    }
    let lanes: Vec<Option<VictoryTarget>> = rows.iter().map(|(_, lane)| *lane).collect();
    let incumbent = Doctrine::ALL
        .iter()
        .position(|doctrine| *doctrine == probe.doctrine())
        .unwrap_or(0);

    let mut value = Vec::with_capacity(lanes.len());
    let mut self_check = 0.0f64;
    for (index, lane) in lanes.iter().enumerate() {
        let column = probe.doctrine_values(&snapshot, pid, *lane);
        let row: Vec<f64> = Doctrine::ALL
            .iter()
            .map(|doctrine| {
                column
                    .iter()
                    .find_map(|(candidate, value)| (candidate == doctrine).then_some(*value))
                    .unwrap_or(f64::MIN)
            })
            .collect();
        // `lane_values` projected this same lane under the incumbent doctrine.
        // If the two disagree, the matrix is not the search's own numbers.
        self_check = self_check.max((row[incumbent] - rows[index].0).abs());
        value.push(row);
    }
    Some(Review {
        lanes,
        value,
        incumbent,
        self_check,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let games = number(&args, "--games", 24);
    let seats = number(&args, "--players", 4);
    let width = number(&args, "--width", 44) as i32;
    let height = number(&args, "--height", 28) as i32;
    let turns = number(&args, "--turns", 200) as u32;
    let reviews = number(&args, "--reviews", 6);
    let seed = number(&args, "--seed", 97_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());

    println!(
        "joint_axes: {games} games, {seats} players, {width}x{height}, {turns} turns, \
         up to {reviews} review points each, jobs {jobs}"
    );
    println!(
        "axes: {} doctrines x up to 7 lanes; sequential spends ~11 rollouts a review, joint ~28",
        Doctrine::ALL.len()
    );

    // Reported per game, because this run is long and silent otherwise. The
    // first 30-game attempt ran for the better part of an hour with no way to
    // tell whether it was progressing or wedged: late-game rollouts cost
    // several times what an early-game pilot suggests, since every projected
    // round carries more units and more cities.
    let harvest = parallel::map_reporting(
        games,
        jobs,
        |index| examine(seed + index as u64, seats, width, height, turns, reviews),
        |index, produced: &Vec<Review>| {
            eprintln!("  game {} done, {} review points", index + 1, produced.len())
        },
    );
    let reviews: Vec<Review> = harvest.into_iter().flatten().collect();
    let n = reviews.len();
    if n == 0 {
        eprintln!("joint_axes: no review point produced a projection");
        std::process::exit(1);
    }

    let worst_check = reviews.iter().fold(0.0f64, |worst, r| worst.max(r.self_check));
    if worst_check > 1e-9 {
        eprintln!(
            "joint_axes: the matrix disagrees with lane_values by up to {worst_check:.6} -- \
             it is not the shipped search's own numbers. Refusing to report."
        );
        std::process::exit(2);
    }

    let mut interacts = 0usize;
    for review in &reviews {
        interacts += review.doctrine_depends_on_lane() as usize;
    }

    // Swept over every possible incumbent, so the headline cannot be an
    // artifact of the doctrine the probe happened to be frozen in.
    let mut per_incumbent = Vec::new();
    for incumbent in 0..Doctrine::ALL.len() {
        let (mut lane_differs, mut doctrine_differs, mut either) = (0usize, 0usize, 0usize);
        let (mut over_doctrine, mut over_target) = (0usize, 0usize);
        let (mut gap_sum, mut gap_max) = (0.0f64, 0.0f64);
        for review in &reviews {
            let (sl, sd) = review.sequential_from(incumbent);
            let (jl, jd) = review.joint();
            lane_differs += (sl != jl) as usize;
            doctrine_differs += (sd != jd) as usize;
            either += (sl != jl || sd != jd) as usize;
            let gap = review.value[jl][jd] - review.value[sl][sd];
            gap_sum += gap;
            gap_max = gap_max.max(gap);
            over_doctrine += (gap > DOCTRINE_COMMITMENT_MARGIN) as usize;
            over_target += (gap > TARGET_COMMITMENT_MARGIN) as usize;
        }
        per_incumbent.push((
            Doctrine::ALL[incumbent],
            lane_differs,
            doctrine_differs,
            either,
            gap_sum / n as f64,
            gap_max,
            over_doctrine,
            over_target,
        ));
    }
    // The worst case over incumbents is what a real agent can actually be in,
    // so the verdict is taken there rather than on a flattering average.
    let (either, over_doctrine, over_target) = per_incumbent.iter().fold(
        (0usize, 0usize, 0usize),
        |(e, d, t), row| (e.max(row.3), d.max(row.6), t.max(row.7)),
    );
    let gap_sum = per_incumbent
        .iter()
        .map(|row| row.4)
        .fold(0.0f64, f64::max)
        * n as f64;
    let gap_max = per_incumbent.iter().map(|row| row.5).fold(0.0f64, f64::max);
    let lane_differs = per_incumbent.iter().map(|row| row.1).max().unwrap_or(0);
    let doctrine_differs = per_incumbent.iter().map(|row| row.2).max().unwrap_or(0);

    let share = |count: usize| 100.0 * count as f64 / n as f64;
    println!("\n{n} review points projected; self-check clean (max deviation {worst_check:.2e})");
    println!("\nby incumbent doctrine (the baseline a real agent would be searching from):");
    println!("  doctrine      differs   lane  doctr   mean gap    max gap  >0.002  >0.01");
    for (doctrine, lane_d, doc_d, either_d, mean, max, over_d, over_t) in &per_incumbent {
        println!(
            "  {:<12} {:>6.1}% {:>6} {:>6} {:>10.5} {:>10.5} {:>6} {:>6}",
            doctrine.name(),
            share(*either_d),
            lane_d,
            doc_d,
            mean,
            max,
            over_d,
            over_t
        );
    }
    println!("\nworst case over incumbents (what the verdict is taken on):");
    println!(
        "joint argmax differs from the sequential pick: {either}/{n} ({:.1}%)",
        share(either)
    );
    println!(
        "  of which lane differs {lane_differs} ({:.1}%), doctrine differs {doctrine_differs} ({:.1}%)",
        share(lane_differs),
        share(doctrine_differs)
    );
    println!(
        "best doctrine depends on the lane: {interacts}/{n} ({:.1}%)",
        share(interacts)
    );
    println!(
        "value left on the table: mean {:.4}, max {:.4} (evaluator units)",
        gap_sum / n as f64,
        gap_max
    );
    println!(
        "gap clears the doctrine margin ({DOCTRINE_COMMITMENT_MARGIN}): {over_doctrine}/{n} ({:.1}%)",
        share(over_doctrine)
    );
    println!(
        "gap clears the lane margin ({TARGET_COMMITMENT_MARGIN}): {over_target}/{n} ({:.1}%)",
        share(over_target)
    );
    // A gap the agent would refuse to act on is not a gap worth 2.5x the
    // rollouts. `doctrine_values`' own comment makes this point about the
    // doctrine axis; it applies to the joint search too.
    // The verdict keys on the *actionable* gap, not on the disagreement rate.
    // Those come apart badly: an early run disagreed on 100% of reviews while
    // the mean gap was 0.0009 -- below the margin the agent needs to change
    // doctrine at all, so joint search would have found a better cell on every
    // review and committed to almost none of them. Rate is the wrong headline.
    println!(
        "\nverdict: {}",
        if either == 0 {
            "coordinate descent already finds the joint optimum -- joint search is dead, at the cost of one run"
        } else if share(over_doctrine) < 5.0 {
            "the joint optimum is real but sub-threshold -- the agent would decline almost every cell it found, \
             so 2.5x the rollouts buys nothing without also revisiting the commitment margins"
        } else if share(over_target) < 5.0 {
            "worth building on the doctrine axis alone: the gap clears the doctrine margin often enough to act on, \
             but almost never the lane margin"
        } else {
            "the axes disagree by more than both margins often enough to be worth building and measuring in play"
        }
    );
}
