//! Does the combat term give military genes a breeding signal score lacks?
//!
//! Every endpoint intervention is evaluated through the exact production
//! fitness path on common seeds. The primary gate is pre-registered in
//! `docs/GENE_OBJECTIVE.md`; exploratory leave-one-map-out selection is
//! reported separately and cannot override it.
use civvis::ai::{PolicyDeck, Weights};
use civvis::evolve::{fitness_observations, load_champion, EvoCfg, FitnessObservation};
use civvis::parallel;

const SCORE_WEIGHT: f64 = 50.0;
const COMBAT_WEIGHT: f64 = 12.0;
const EPSILON: f64 = 1e-12;

/// Genes whose direct purpose is army size, war/peace, tactical exchange, or
/// hierarchical battlefield control. The indices are pinned by a test against
/// `Weights::gene_names`, so a future genome edit cannot silently relabel the
/// intervention.
const MILITARY_GENES: [usize; 21] = [
    3, 5, 6, 7, 8, 9, 10, 11, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39,
];

#[derive(Clone)]
struct Candidate {
    gene: Option<usize>,
    label: String,
    weights: Weights,
}

#[derive(Clone, Copy, Debug, Default)]
struct DeltaStats {
    score_abs: f64,
    combat_abs: f64,
    full_abs: f64,
    score_mean: f64,
    score_se: f64,
    full_mean: f64,
    full_se: f64,
    score_changed: usize,
    full_changed: usize,
    reversals: usize,
    jointly_changed: usize,
}

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

fn policy_deck(name: &str) -> Option<PolicyDeck> {
    match name {
        "artifact" => None,
        "live" => Some(PolicyDeck::Live),
        "legacy" => Some(PolicyDeck::Legacy),
        "empty" => Some(PolicyDeck::Empty),
        other => {
            eprintln!(
                "gene_objective_probe: unknown policy deck {other:?}; use artifact, live, legacy or empty"
            );
            std::process::exit(2);
        }
    }
}

fn mean_se(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    if values.len() < 2 {
        return (mean, 0.0);
    }
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64;
    (mean, (variance / values.len() as f64).sqrt())
}

fn components(observation: FitnessObservation, players: usize) -> (f64, f64, f64) {
    let score = SCORE_WEIGHT * players as f64 * observation.score_share;
    let combat = COMBAT_WEIGHT * players as f64 * observation.combat_share;
    (score, combat, score + combat)
}

fn delta_stats(
    treatment: &[FitnessObservation],
    baseline: &[FitnessObservation],
    players: usize,
) -> DeltaStats {
    let mut score_deltas = Vec::with_capacity(treatment.len());
    let mut full_deltas = Vec::with_capacity(treatment.len());
    let mut score_abs = 0.0;
    let mut combat_abs = 0.0;
    let mut full_abs = 0.0;
    let mut score_changed = 0;
    let mut full_changed = 0;
    let mut reversals = 0;
    let mut jointly_changed = 0;
    for (treatment, baseline) in treatment.iter().zip(baseline) {
        let (t_score, t_combat, t_full) = components(*treatment, players);
        let (b_score, b_combat, b_full) = components(*baseline, players);
        let score_delta = t_score - b_score;
        let combat_delta = t_combat - b_combat;
        let full_delta = t_full - b_full;
        score_abs += score_delta.abs();
        combat_abs += combat_delta.abs();
        full_abs += full_delta.abs();
        score_changed += usize::from(score_delta.abs() > EPSILON);
        full_changed += usize::from(full_delta.abs() > EPSILON);
        if score_delta.abs() > EPSILON && full_delta.abs() > EPSILON {
            jointly_changed += 1;
            reversals += usize::from(score_delta.signum() != full_delta.signum());
        }
        score_deltas.push(score_delta);
        full_deltas.push(full_delta);
    }
    let count = treatment.len().max(1) as f64;
    let (score_mean, score_se) = mean_se(&score_deltas);
    let (full_mean, full_se) = mean_se(&full_deltas);
    DeltaStats {
        score_abs: score_abs / count,
        combat_abs: combat_abs / count,
        full_abs: full_abs / count,
        score_mean,
        score_se,
        full_mean,
        full_se,
        score_changed,
        full_changed,
        reversals,
        jointly_changed,
    }
}

fn objective(observation: FitnessObservation, players: usize, combat: bool) -> f64 {
    let (score, _, full) = components(observation, players);
    if combat {
        full
    } else {
        score
    }
}

fn best_candidate(
    results: &[Vec<FitnessObservation>],
    held_out: usize,
    players: usize,
    combat: bool,
) -> usize {
    (0..results.len())
        .max_by(|left, right| {
            let mean = |candidate: usize| {
                let values = &results[candidate];
                let count = values.len().saturating_sub(1).max(1) as f64;
                values
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != held_out)
                    .map(|(_, observation)| objective(*observation, players, combat))
                    .sum::<f64>()
                    / count
            };
            mean(*left)
                .partial_cmp(&mean(*right))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| right.cmp(left))
        })
        .unwrap_or(0)
}

fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 6).max(2);
    let games = number(&args, "--games", 24).max(2);
    let width = number(&args, "--width", 74).max(8) as i32;
    let height = number(&args, "--height", 46).max(8) as i32;
    let turns = number(&args, "--turns", 250).max(1) as u32;
    let seed = number(&args, "--seed", 9_800_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs()).max(1);
    let speed = text(&args, "--speed", "online");
    let artifact = text(&args, "--base", "evolved");
    let deck_arg = text(&args, "--policy-deck", "artifact");
    let Some(mut base) = load_champion(&artifact) else {
        eprintln!("gene_objective_probe: no valid champion in {artifact:?}");
        std::process::exit(2);
    };
    if let Some(deck) = policy_deck(&deck_arg) {
        base.policy_deck = deck;
    }

    let names = Weights::gene_names();
    let bounds = Weights::bounds();
    let base_values = base.to_vec();
    let mut candidates = vec![Candidate {
        gene: None,
        label: "incumbent".to_string(),
        weights: base.clone(),
    }];
    for gene in MILITARY_GENES {
        let (low, high) = bounds[gene];
        for (endpoint, value) in [("low", low), ("high", high)] {
            if (value - base_values[gene]).abs() <= EPSILON {
                continue;
            }
            let mut values = base_values.clone();
            values[gene] = value;
            candidates.push(Candidate {
                gene: Some(gene),
                label: format!("{}={endpoint}", names[gene]),
                weights: Weights::from_vec_like(&values, &base),
            });
        }
    }

    let cfg = EvoCfg {
        generations: 1,
        pop: candidates.len(),
        games,
        players,
        width,
        height,
        max_turns: turns,
        seed,
        threads: jobs,
        dir: String::new(),
        speed: speed.clone(),
    };
    let opponents = vec![base];
    println!(
        "gene objective probe: {} endpoint candidates x {games} paired games; \
         {players}p {width}x{height} {speed}, {turns}t (every third doubles), seed {seed}",
        candidates.len() - 1
    );
    println!(
        "base/opponents: champion from {artifact}; policy deck {deck_arg}; jobs {jobs}\n"
    );
    let results = parallel::map_reporting(
        candidates.len(),
        jobs,
        |index| fitness_observations(&candidates[index].weights, &opponents, &cfg, 0, games),
        |index, _| {
            eprintln!(
                "  finished {:>2}/{}: {}",
                index + 1,
                candidates.len(),
                candidates[index].label
            )
        },
    );

    println!(
        "{:<24} {:>8} {:>8} {:>8} {:>9} {:>18} {:>18} {:>8}",
        "endpoint",
        "|score|",
        "|combat|",
        "|full|",
        "score hit",
        "mean score +/- SE",
        "mean full +/- SE",
        "reverse"
    );
    let mut endpoint_stats = Vec::with_capacity(candidates.len());
    endpoint_stats.push(DeltaStats::default());
    for index in 1..candidates.len() {
        let stats = delta_stats(&results[index], &results[0], players);
        println!(
            "{:<24} {:>8.3} {:>8.3} {:>8.3} {:>4}/{:<4} {:+8.3} +/- {:<7.3} {:+8.3} +/- {:<7.3} {:>3}/{:<3}",
            candidates[index].label,
            stats.score_abs,
            stats.combat_abs,
            stats.full_abs,
            stats.score_changed,
            games,
            stats.score_mean,
            stats.score_se,
            stats.full_mean,
            stats.full_se,
            stats.reversals,
            stats.jointly_changed,
        );
        endpoint_stats.push(stats);
    }

    let mut covered = 0;
    let mut combat_only = 0;
    let mut ratios = Vec::new();
    println!("\nper-gene incumbent-favorable endpoint (maximum |full delta|):");
    println!(
        "{:<20} {:<5} {:>8} {:>8} {:>9} {:>8} {:>12}",
        "gene", "bound", "|score|", "|full|", "score hit", "ratio", "classification"
    );
    for gene in MILITARY_GENES {
        let selected = candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.gene == Some(gene))
            .max_by(|(left, _), (right, _)| {
                endpoint_stats[*left]
                    .full_abs
                    .partial_cmp(&endpoint_stats[*right].full_abs)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(index, candidate)| (index, candidate))
            .expect("each military gene has at least one non-incumbent bound");
        let stats = endpoint_stats[selected.0];
        let score_rate = stats.score_changed as f64 / games as f64;
        let full_rate = stats.full_changed as f64 / games as f64;
        covered += usize::from(score_rate >= 0.25);
        let is_combat_only = full_rate >= 0.25 && score_rate < 0.10;
        combat_only += usize::from(is_combat_only);
        let ratio = if stats.full_abs > EPSILON {
            stats.score_abs / stats.full_abs
        } else if stats.score_abs > EPSILON {
            1.0
        } else {
            0.0
        };
        ratios.push(ratio);
        let bound = selected.1.label.rsplit('=').next().unwrap_or("?");
        let classification = if is_combat_only {
            "COMBAT-ONLY"
        } else if score_rate >= 0.25 {
            "score reaches"
        } else {
            "weak reach"
        };
        println!(
            "{:<20} {:<5} {:>8.3} {:>8.3} {:>4}/{:<4} {:>8.3} {:>12}",
            names[gene],
            bound,
            stats.score_abs,
            stats.full_abs,
            stats.score_changed,
            games,
            ratio,
            classification,
        );
    }
    let median_ratio = median(ratios);
    let coverage_pass = covered >= 16;
    let retention_pass = median_ratio >= 0.50;
    let stranded_pass = combat_only < 6;
    let passed = coverage_pass && retention_pass && stranded_pass;

    let mut same_pick = 0;
    let mut score_wins = 0;
    let mut full_wins = 0;
    let mut win_for = 0;
    let mut win_against = 0;
    let mut score_share_deltas = Vec::with_capacity(games);
    for held_out in 0..games {
        let score_pick = best_candidate(&results, held_out, players, false);
        let full_pick = best_candidate(&results, held_out, players, true);
        same_pick += usize::from(score_pick == full_pick);
        let score_observation = results[score_pick][held_out];
        let full_observation = results[full_pick][held_out];
        score_wins += usize::from(score_observation.won);
        full_wins += usize::from(full_observation.won);
        win_for += usize::from(score_observation.won && !full_observation.won);
        win_against += usize::from(!score_observation.won && full_observation.won);
        score_share_deltas.push(score_observation.score_share - full_observation.score_share);
    }
    let (heldout_score_delta, heldout_score_se) = mean_se(&score_share_deltas);

    println!("\npre-registered fires-check:");
    println!(
        "  score coverage: {covered}/21 genes at >=25% maps (need >=16): {}",
        if coverage_pass { "PASS" } else { "FAIL" }
    );
    println!(
        "  median |score delta| / |full delta|: {median_ratio:.3} (need >=0.500): {}",
        if retention_pass { "PASS" } else { "FAIL" }
    );
    println!(
        "  combat-only genes: {combat_only}/21 (need <6): {}",
        if stranded_pass { "PASS" } else { "FAIL" }
    );
    println!("  FIRES-CHECK: {}", if passed { "PASS" } else { "FAIL" });
    println!("\nexploratory leave-one-map-out selection (not a gate):");
    println!("  same candidate: {same_pick}/{games}");
    println!(
        "  held-out wins: score-only {score_wins}/{games}, full {full_wins}/{games}; \
         discordant maps {win_for} for / {win_against} against"
    );
    println!(
        "  held-out score-share delta, score-only minus full: {heldout_score_delta:+.5} +/- {heldout_score_se:.5}"
    );
    println!(
        "\nDECISION: {}",
        if passed {
            "score retains enough causal military reach to earn an independent objective-selection A/B; no production default changed"
        } else {
            "retain the combat term; score alone fails the preregistered military-signal gate"
        }
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(score_share: f64, combat_share: f64) -> FitnessObservation {
        FitnessObservation {
            score_share,
            combat_share,
            value: 0.0,
            won: false,
        }
    }

    #[test]
    fn military_gene_indices_are_stable() {
        let names = Weights::gene_names();
        let selected: Vec<&str> = MILITARY_GENES.iter().map(|index| names[*index]).collect();
        assert_eq!(
            selected,
            [
                "mil_per_city",
                "war_ratio",
                "war_margin",
                "peace_ratio",
                "war_min_turn",
                "attack_floor",
                "kill_bonus",
                "trade_caution",
                "mv_support",
                "mv_threat",
                "command_radius",
                "muster_radius",
                "muster_readiness",
                "cohesion",
                "focus_fire",
                "screen",
                "role_spacing",
                "objective_progress",
                "local_superiority",
                "withdraw_hp",
                "rejoin_hp",
            ]
        );
    }

    #[test]
    fn shipped_champion_survives_gene_vector_round_trip() {
        let champion = load_champion("evolved").expect("embedded champion");
        assert_eq!(
            champion,
            Weights::from_vec_like(&champion.to_vec(), &champion)
        );
    }

    #[test]
    fn paired_components_report_reversals() {
        let baseline = [observation(0.25, 0.25), observation(0.25, 0.25)];
        let treatment = [observation(0.30, 0.0), observation(0.20, 0.50)];
        let stats = delta_stats(&treatment, &baseline, 4);
        assert!((stats.score_abs - 10.0).abs() < 1e-9);
        assert!((stats.combat_abs - 12.0).abs() < 1e-9);
        assert!((stats.full_abs - 2.0).abs() < 1e-9);
        assert_eq!(stats.score_changed, 2);
        assert_eq!(stats.full_changed, 2);
        assert_eq!(stats.reversals, 2);
    }

    #[test]
    fn leave_one_out_never_reads_the_held_out_value() {
        let results = vec![
            vec![observation(1.0, 0.0), observation(0.0, 0.0)],
            vec![observation(0.0, 0.0), observation(0.5, 0.0)],
        ];
        assert_eq!(best_candidate(&results, 0, 2, false), 1);
        assert_eq!(best_candidate(&results, 1, 2, false), 0);
    }
}
