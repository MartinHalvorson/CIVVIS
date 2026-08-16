//! Arena: batch rating events over standardized league games.
//!
//! The continuous league updates a Glicko-2 placement rating a few games at a
//! time, across whatever table sizes and profiles its rounds happened to
//! play. That is the right engine for *matchmaking and selection*, and the
//! wrong artifact to *publish as strength*: settings drift between rounds,
//! small updates churn the table daily, and a closed pool's scale slowly
//! walks. (`docs/RATING.md` records the audit where one confounded slice of
//! that history carried literally no information.)
//!
//! An arena is the batch alternative: a rating **event**. It takes the league
//! directory's full match history, keeps only games at one standardized table
//! size, refits the corrected contextual model (placement stages weighted by
//! measured information, a shared per-civilization edge, balanced-seating
//! assumptions — `src/rating.rs`) from scratch, and pins the scale to the
//! anchor strategies so the numbers stay comparable from one arena to the
//! next even after every founder has been replaced. Ratings therefore move
//! only when an arena runs, every rated game was played under one profile,
//! and 1500 always means the same thing: the anchors' mean.
//!
//! The scale is internal for now, anchored to `advanced`/`basic`. The
//! `external_anchor` field is the reserved seam for the day the live bridge
//! has enough real-Civilization-VI games to place the deployed agent against
//! Firaxis' own AI at a named difficulty: measuring that once turns the whole
//! anchored scale into "Elo relative to shipping Civ 6", without refitting
//! anything — it is a labelled offset, exactly like the anchor itself.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::rating::{load_history, MatchRecord, RatingCfg};

/// What one arena event publishes for one strategy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArenaRow {
    pub name: String,
    /// Anchored rating: the anchors' mean is `anchor_elo` by construction.
    pub elo: f64,
    pub rd: f64,
    /// `elo - 2·rd`: the guarded number to compare newcomers by.
    pub conservative_elo: f64,
    /// Games and outright wins inside this arena's standardized slice.
    pub games: u32,
    pub wins: u32,
    /// The previous arena's rating, so a batch event shows its whole step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_elo: Option<f64>,
}

/// The external anchor: not yet measured, deliberately present in the schema.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExternalAnchor {
    /// What the offset was measured against, e.g. `firaxis_prince`.
    pub reference: String,
    /// The Elo the reference is defined to hold on this scale.
    pub elo: f64,
    /// Where the evidence lives (a run set, an EVAL.md entry).
    pub evidence: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArenaReport {
    /// 1-based event sequence; the previous artifact's `arena + 1`.
    pub arena: u32,
    pub generated_utc: String,
    /// Which history this event rated — a league directory is a pool, and an
    /// artifact that outlives the directory must say whose games it read.
    #[serde(default)]
    pub source: String,
    /// The one table size every rated game was played at.
    pub seats: usize,
    pub games_rated: usize,
    /// Strategy names whose mean rating defines the scale, and the value it
    /// is held at.
    pub anchors: Vec<String>,
    pub anchor_elo: f64,
    /// `None` until the deployed agent has been placed against Firaxis'
    /// own AI on the live bridge; see the module doc.
    pub external_anchor: Option<ExternalAnchor>,
    /// Strongest first by anchored rating.
    pub strategies: Vec<ArenaRow>,
}

/// Table size to standardize on: the size the history actually plays.
///
/// Explicit is better, but a default has to exist and "whatever size the
/// league predominantly played" is the only defensible one — an arena is a
/// reading of evidence, and most of the evidence is at the modal size.
pub fn modal_seats(history: &[MatchRecord]) -> usize {
    let mut counts: BTreeMap<usize, usize> = BTreeMap::new();
    for m in history {
        *counts.entry(m.seats.len()).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(seats, _)| seats)
        .unwrap_or(0)
}

/// Run one arena event over `history`, standardized to `seats`-player games.
///
/// `previous` is the last arena's artifact if one exists: it supplies the
/// sequence number and the per-strategy previous ratings, so the report
/// shows each strategy's whole step since the last event.
pub fn run(
    history: &[MatchRecord],
    seats: usize,
    anchors: &[String],
    anchor_elo: f64,
    previous: Option<&ArenaReport>,
    generated_utc: String,
    source: String,
) -> Result<ArenaReport, String> {
    let slice: Vec<&MatchRecord> =
        history.iter().filter(|m| m.seats.len() == seats).collect();
    if slice.len() < 2 {
        return Err(format!(
            "only {} game(s) at {seats} seats; an arena needs at least 2",
            slice.len()
        ));
    }

    let mut cfg = RatingCfg::default();
    cfg.anchor_elo = anchor_elo;
    for anchor in anchors {
        cfg.anchors.insert(anchor.clone());
    }
    let mut rating = crate::rating::ContextualRating::new(cfg);
    let mut played: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    for m in &slice {
        rating.observe(m);
        // An outright win is rank 0 held alone; a tied first is not a win,
        // which matches how the league's own Wilson-bound objective counts.
        let winners: Vec<&str> = m
            .seats
            .iter()
            .filter(|s| s.rank == 0)
            .map(|s| s.player.as_str())
            .collect();
        for seat in &m.seats {
            let entry = played.entry(seat.player.clone()).or_default();
            entry.0 += 1;
            if seat.rank == 0 && winners.len() == 1 {
                entry.1 += 1;
            }
        }
    }

    let before: BTreeMap<&str, f64> = previous
        .map(|p| {
            p.strategies
                .iter()
                .map(|s| (s.name.as_str(), s.elo))
                .collect()
        })
        .unwrap_or_default();

    let mut rows: Vec<ArenaRow> = played
        .iter()
        .map(|(name, (games, wins))| {
            let belief = rating.player(name);
            ArenaRow {
                name: name.clone(),
                elo: round2(belief.elo()),
                rd: round2(belief.rd()),
                conservative_elo: round2(belief.conservative_elo()),
                games: *games,
                wins: *wins,
                previous_elo: before.get(name.as_str()).copied(),
            }
        })
        .collect();
    rows.sort_by(|a, b| b.elo.total_cmp(&a.elo).then(a.name.cmp(&b.name)));

    Ok(ArenaReport {
        arena: previous.map(|p| p.arena + 1).unwrap_or(1),
        generated_utc,
        source,
        seats,
        games_rated: slice.len(),
        anchors: anchors.to_vec(),
        anchor_elo,
        external_anchor: previous.and_then(|p| p.external_anchor.clone()),
        strategies: rows,
    })
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// The printed table: what an operator reads at the end of an arena event.
pub fn render(report: &ArenaReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "arena {} — {} games at {} seats from {}, anchored {} = {:.0}\n",
        report.arena,
        report.games_rated,
        report.seats,
        report.source,
        report.anchors.join("+"),
        report.anchor_elo,
    ));
    match &report.external_anchor {
        Some(ext) => out.push_str(&format!(
            "external anchor: {} = {:.0} ({})\n",
            ext.reference, ext.elo, ext.evidence
        )),
        None => out.push_str(
            "external anchor: none yet — scale is internal (see docs/LEAGUE.md)\n",
        ),
    }
    out.push_str(&format!(
        "{:<24}{:>9}{:>7}{:>9}{:>8}{:>7}{:>9}\n",
        "strategy", "elo", "rd", "cons.", "games", "wins", "Δ arena"
    ));
    for row in &report.strategies {
        let delta = row
            .previous_elo
            .map(|p| format!("{:+.0}", row.elo - p))
            .unwrap_or_else(|| "new".to_string());
        out.push_str(&format!(
            "{:<24}{:>9.1}{:>7.0}{:>9.1}{:>8}{:>7}{:>9}\n",
            row.name, row.elo, row.rd, row.conservative_elo, row.games, row.wins, delta
        ));
    }
    out
}

/// Load, run, persist, and describe one arena event for `dir`.
pub fn run_dir(
    dir: &str,
    seats: usize,
    anchors: &[String],
    anchor_elo: f64,
    now: std::time::SystemTime,
) -> Result<String, String> {
    let history =
        load_history(dir).map_err(|e| format!("cannot read {dir}/matches.csv: {e}"))?;
    let seats = if seats > 0 { seats } else { modal_seats(&history) };
    let artifact = Path::new(dir).join("arena.json");
    let previous: Option<ArenaReport> = std::fs::read_to_string(&artifact)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok());
    let report = run(
        &history,
        seats,
        anchors,
        anchor_elo,
        previous.as_ref(),
        crate::civ6::utc_stamp(now),
        format!("{dir}/matches.csv"),
    )?;
    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| format!("cannot serialize the arena report: {e}"))?;
    std::fs::write(&artifact, json + "\n")
        .map_err(|e| format!("cannot write {}: {e}", artifact.display()))?;
    Ok(format!("{}wrote {}\n", render(&report), artifact.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rating::Seat;

    fn game(seats: usize, order: &[&str], round: u32) -> MatchRecord {
        MatchRecord {
            seats: order
                .iter()
                .take(seats)
                .enumerate()
                .map(|(rank, player)| Seat {
                    player: player.to_string(),
                    leader: String::new(),
                    civ: ["Rome", "Egypt", "Greece", "China", "Aztec", "Sumeria"]
                        [(rank + round as usize) % 6]
                        .to_string(),
                    rank: rank as u32,
                })
                .collect(),
            round,
            turn: 200,
            victory: "science".into(),
        }
    }

    fn anchors() -> Vec<String> {
        vec!["advanced".to_string(), "basic".to_string()]
    }

    #[test]
    fn an_arena_rates_only_the_standardized_table_size() {
        let mut history = Vec::new();
        for round in 0..30 {
            history.push(game(4, &["climber", "advanced", "basic", "filler"], round));
        }
        // Two 6-seat games where the climber loses; they must not count.
        history.push(game(6, &["advanced", "basic", "x", "y", "z", "climber"], 90));
        history.push(game(6, &["advanced", "basic", "x", "y", "z", "climber"], 91));
        let report =
            run(&history, 4, &anchors(), 1500.0, None, "t".into(), "test".into()).expect("arena runs");
        assert_eq!(report.games_rated, 30);
        let climber = report
            .strategies
            .iter()
            .find(|s| s.name == "climber")
            .expect("climber rated");
        assert_eq!(climber.games, 30, "the 6-seat games stayed out");
        assert_eq!(climber.wins, 30);
        assert!(
            climber.elo > 1500.0,
            "an undefeated strategy rates above the anchors: {}",
            climber.elo
        );
    }

    #[test]
    fn the_anchors_mean_is_the_published_scale() {
        let history: Vec<MatchRecord> = (0..40)
            .map(|r| game(4, &["climber", "advanced", "basic", "filler"], r))
            .collect();
        let report =
            run(&history, 4, &anchors(), 1500.0, None, "t".into(), "test".into()).expect("arena runs");
        let advanced = report.strategies.iter().find(|s| s.name == "advanced").unwrap();
        let basic = report.strategies.iter().find(|s| s.name == "basic").unwrap();
        let mean = (advanced.elo + basic.elo) / 2.0;
        assert!(
            (mean - 1500.0).abs() < 0.01,
            "anchored mean must sit at 1500: {mean}"
        );
    }

    #[test]
    fn the_next_arena_reports_each_step_and_advances_the_sequence() {
        let early: Vec<MatchRecord> = (0..20)
            .map(|r| game(4, &["climber", "advanced", "basic", "filler"], r))
            .collect();
        let first =
            run(&early, 4, &anchors(), 1500.0, None, "t1".into(), "test".into()).expect("first arena");
        assert_eq!(first.arena, 1);
        assert!(first.strategies.iter().all(|s| s.previous_elo.is_none()));

        // The climber's luck turns; the second event shows the whole step.
        let mut later = early.clone();
        for r in 20..60 {
            later.push(game(4, &["filler", "advanced", "basic", "climber"], r));
        }
        let second = run(&later, 4, &anchors(), 1500.0, Some(&first), "t2".into(), "test".into())
            .expect("second arena");
        assert_eq!(second.arena, 2);
        let climber = second.strategies.iter().find(|s| s.name == "climber").unwrap();
        let before = climber.previous_elo.expect("previous rating carried");
        assert!(
            climber.elo < before,
            "the fall shows as one batch step: {} -> {}",
            before,
            climber.elo
        );
    }

    #[test]
    fn ties_for_first_are_not_outright_wins() {
        let mut m = game(4, &["a", "b", "advanced", "basic"], 0);
        m.seats[1].rank = 0; // a and b tie for first
        let history = vec![m, game(4, &["a", "b", "advanced", "basic"], 1)];
        let report =
            run(&history, 4, &anchors(), 1500.0, None, "t".into(), "test".into()).expect("arena runs");
        let a = report.strategies.iter().find(|s| s.name == "a").unwrap();
        assert_eq!(a.games, 2);
        assert_eq!(a.wins, 1, "the tied first must not count as an outright win");
    }

    #[test]
    fn modal_seats_reads_the_history_not_a_constant() {
        let mut history: Vec<MatchRecord> =
            (0..5).map(|r| game(6, &["a", "b", "c", "d", "e", "f"], r)).collect();
        history.push(game(4, &["a", "b", "c", "d"], 9));
        assert_eq!(modal_seats(&history), 6);
    }
}
