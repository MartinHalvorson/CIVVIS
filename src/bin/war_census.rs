//! Does this AI build an army *toward* a war, or declare one and then build?
//!
//! The repository knows what its wars achieve — 0.33 cities captured a game,
//! no capital ever taken, and not one peace treaty across twelve full-length
//! games — and it knows the proximate cause: 67% of siege work opens a city
//! with nobody able to walk in. What nothing has measured is the step before
//! all of that. **A war is an appointment.** A strong player picks a target,
//! reads how long the force needed to take it will take to build and to walk,
//! and declares when it arrives. If instead the declaration comes first and
//! the army is raised afterwards, then every later failure — the missing
//! escort, the lone siege engine, the war that never ends — is downstream of a
//! decision made with no force in hand.
//!
//! So this measures, per declaration:
//!
//! - **force in position**: the declarer's military units already within reach
//!   of the target's nearest city on the turn war is declared;
//! - **the peak** it ever reaches during that war, and when;
//! - power ratio at declaration, against the `war_ratio` the genome nominally
//!   asks for;
//! - whether the war ever took a city, and how long that took.
//!
//! The reading that would vindicate the current design is force-in-position
//! high at turn zero of the war. The reading that would condemn it is a peak
//! arriving tens of turns late, or never.
//!
//! ```text
//! war_census --players 4 --maps 8 --turns 400
//! ```
//!
//! Diagnostic only: it never changes a decision, and no agent can name it.
use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::game::{Action, Game};
use civvis::Pos;
use civvis::parallel;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// How near a unit must be to count as available for the opening blow.
const REACH: i32 = 4;

#[derive(Clone)]
struct War {
    declared_turn: u32,
    force_at_declaration: usize,
    peak_force: usize,
    peak_turn: u32,
    power_ratio: f64,
    captured_turn: Option<u32>,
    ended: bool,
}

/// Military units of `pid` within `REACH` of the nearest city of `foe`.
fn force_in_position(g: &Game, pid: usize, foe: usize) -> usize {
    let targets: Vec<Pos> = g
        .player_city_ids(foe)
        .into_iter()
        .filter_map(|cid| g.cities.get(&cid).map(|c| c.pos))
        .collect();
    if targets.is_empty() {
        return 0;
    }
    g.player_unit_ids(pid)
        .into_iter()
        .filter(|uid| {
            let Some(unit) = g.units.get(uid) else {
                return false;
            };
            let military = g
                .rules
                .units
                .get(unit.kind.as_str())
                .is_some_and(|spec| spec.class == "military");
            military && targets.iter().any(|t| g.wdist(unit.pos, *t) <= REACH)
        })
        .count()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 8);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 400) as u32;
    let seed0 = number(&args, "--seed", 800_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());

    println!(
        "war_census: {maps} maps, {players}p {width}x{height}, {turns} turns, seed {seed0}, \
         reach {REACH} tiles"
    );

    let per_map = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let mut game = Game::new(players, width, height, seed, turns, 0);
        let stock = Weights::default();
        let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &stock);
        let majors: Vec<usize> = (0..game.players.len())
            .filter(|pid| !game.players[*pid].is_minor)
            .collect();
        let mut open: Vec<(usize, usize, War)> = Vec::new();
        let mut done: Vec<War> = Vec::new();
        let mut city_count: Vec<usize> = majors.iter().map(|p| game.player_city_ids(*p).len()).collect();

        for turn in 0..turns {
            if game.winner.is_some() {
                break;
            }
            for pid in 0..game.players.len() {
                if game.winner.is_some() {
                    break;
                }
                fleet[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }

            // Open a record the turn a war appears, close it when it ends.
            for (a_index, a) in majors.iter().enumerate() {
                for b in majors.iter().skip(a_index + 1) {
                    let at_war = game.is_at_war(*a, *b);
                    let known = open.iter().position(|(x, y, _)| x == a && y == b);
                    match (at_war, known) {
                        (true, None) => {
                            let power_a = game.military_power(*a).max(1.0);
                            let power_b = game.military_power(*b).max(1.0);
                            // Credit the war to whichever side has force forward;
                            // the engine does not record who declared.
                            let (attacker, defender) =
                                if force_in_position(&game, *a, *b) >= force_in_position(&game, *b, *a) {
                                    (*a, *b)
                                } else {
                                    (*b, *a)
                                };
                            let force = force_in_position(&game, attacker, defender);
                            open.push((
                                *a,
                                *b,
                                War {
                                    declared_turn: turn,
                                    force_at_declaration: force,
                                    peak_force: force,
                                    peak_turn: turn,
                                    power_ratio: if attacker == *a {
                                        power_a / power_b
                                    } else {
                                        power_b / power_a
                                    },
                                    captured_turn: None,
                                    ended: false,
                                },
                            ));
                        }
                        (true, Some(slot)) => {
                            let (x, y, war) = &mut open[slot];
                            let force = force_in_position(&game, *x, *y)
                                .max(force_in_position(&game, *y, *x));
                            if force > war.peak_force {
                                war.peak_force = force;
                                war.peak_turn = turn;
                            }
                        }
                        (false, Some(slot)) => {
                            let (_, _, mut war) = open.remove(slot);
                            war.ended = true;
                            done.push(war);
                        }
                        (false, None) => {}
                    }
                }
            }

            // A city changing hands anywhere marks the first capture of every
            // war still running; coarse, but the capture count is so low that
            // attribution is not the binding uncertainty.
            for (k, pid) in majors.iter().enumerate() {
                let now = game.player_city_ids(*pid).len();
                if now < city_count[k] {
                    for (_, _, war) in open.iter_mut() {
                        if war.captured_turn.is_none() {
                            war.captured_turn = Some(turn);
                        }
                    }
                }
                city_count[k] = now;
            }
        }
        for (_, _, war) in open {
            done.push(war);
        }
        done
    });

    let wars: Vec<War> = per_map.into_iter().flatten().collect();
    if wars.is_empty() {
        println!("  no wars in {maps} maps -- raise --maps or --turns");
        return;
    }
    let n = wars.len() as f64;
    let mean = |f: &dyn Fn(&War) -> f64| wars.iter().map(f).sum::<f64>() / n;

    let with_none = wars.iter().filter(|w| w.force_at_declaration == 0).count();
    let took = wars.iter().filter(|w| w.captured_turn.is_some()).count();
    let ended = wars.iter().filter(|w| w.ended).count();
    let lag: Vec<f64> = wars
        .iter()
        .map(|w| (w.peak_turn - w.declared_turn) as f64)
        .collect();
    let mean_lag = lag.iter().sum::<f64>() / n;

    println!("  wars observed            {}", wars.len());
    println!(
        "  force in position at declaration   mean {:.2} units",
        mean(&|w| w.force_at_declaration as f64)
    );
    println!(
        "  ...declared with NONE in position  {with_none}/{} ({:.0}%)",
        wars.len(),
        100.0 * with_none as f64 / n
    );
    println!(
        "  peak force during the war          mean {:.2} units",
        mean(&|w| w.peak_force as f64)
    );
    println!("  turns from declaration to peak     mean {mean_lag:.1}");
    println!(
        "  power ratio at declaration         mean {:.2} (genome war_ratio asks 1.80)",
        mean(&|w| w.power_ratio)
    );
    println!(
        "  wars that ever took a city         {took}/{} ({:.0}%)",
        wars.len(),
        100.0 * took as f64 / n
    );
    println!(
        "  wars that ever ended               {ended}/{} ({:.0}%)",
        wars.len(),
        100.0 * ended as f64 / n
    );
    // Read the data, do not recite a conclusion written before it. The first
    // version of this line asserted "the army is raised after the declaration"
    // unconditionally, and the very first run refuted it -- 0 of 5 wars opened
    // with nothing in position and the peak arrived 4.4 turns later.
    let ready = 100.0 * (1.0 - with_none as f64 / n);
    if mean_lag <= 10.0 && ready >= 75.0 {
        println!(
            "\nThe force is already in position when war opens ({ready:.0}% of wars, peak only \
             {mean_lag:.1} turns later).\nBuildout timing is NOT the failure -- look downstream, \
             at what the assembled army then does."
        );
    } else {
        println!(
            "\nA war planned around a target has its force in position when it opens. \
             {ready:.0}% do,\nand the peak arrives {mean_lag:.1} turns after the declaration -- \
             the army is being raised after\nthe decision rather than toward it."
        );
    }
    if wars.len() < 20 {
        println!(
            "\n⚠ {} wars is a small sample. Raise --maps before drawing anything from the rates.",
            wars.len()
        );
    }
}
