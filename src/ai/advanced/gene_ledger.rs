//! The gene ledger: what the screen has measured about every gene, and the
//! deployment genome that follows from it.
//!
//! ⭐ ONE SCREEN. Operator directive 2026-08-22: every row here comes from the
//! same shape — six majors on 74x46 continents with nine city-states, Online
//! speed to its own 250-turn clock, all six victory lanes, every seat carrying
//! its own drawn genome against the best-genome baseline. There is no second
//! regime to reconcile, and `tools/genes.py` refuses a source played at
//! any other profile. The columns carried over from the pre-2026-08-22 Pangaea
//! screens are marked `legacy` in `docs/gene_ledger.json`: history the
//! deployment genome stands on until the screen re-prices each gene.
//!
//! ★★★★ THE DEFAULTS ARE THE BEST GENOME, AND THE BEST GENOME IS DATA.
//! Operator directive 2026-08-20: the defaults for the genes reflect our best
//! genome — so every verification game (the live seat, the `live` and
//! `advanced_synergy` arms, the ladder) plays the genome the measurements
//! support, while the screens keep testing and trying to improve the rest.
//! Until this module, "on by default" meant "somebody wrote `self.enable_x()`
//! into the bundle", and the phase-1 anchors measured the all-on bundle at
//! 7.5% wins against 27% for all-off.
//!
//! ★★★★ THE DEFAULT IS READ OFF THE RANKING'S TWO WIN COLUMNS.
//! Operator directive 2026-08-22: a gene may default on when **both** its
//! last and prior win columns are positive, or when their average
//! clears +15 with neither column below −10. With exactly one populated
//! column, it may provisionally default on when that reading is above +20;
//! every other gene defaults off.
//! A win column is wins added per 10,000 on-arm seats at the gene's measured on-rate
//! in one screen — `(win_on − 1/players) × 10,000`, against the 1-in-
//! `players` a seat wins by chance — and it is the same number
//! `HEURISTIC_GENE_RANKING.md` prints.
//!
//! ★★★★ AND IT IS PUBLISHED BESIDE A PRECISION-WEIGHTED POSTERIOR.
//! A threshold in column units is not a threshold in evidence: the screens
//! those columns come from resolve between ±29 and ±101 at 80% power, so the
//! same reading decides differently depending only on which screen priced the
//! gene. `posterior_pp` / `posterior_se_pp` are a random-effects
//! (DerSimonian–Laird) inverse-variance pool of every screen's on−off
//! difference on the win column's scale, with the between-screen
//! disagreement carried in the interval. They are **published, not in
//! force**: `AUTHORITY` in `tools/genes.py` is `columns` and the
//! generated table records it, so `deployment_default_on` re-derives exactly
//! what shipped. `HEURISTIC_GENE_RANKING.md` prints what the other authority
//! would change.
//!
//! What that replaced: the default used to be `verdict == helps`, one
//! screen's significance test. The verdicts are still
//! recorded and still say what the screens proved; they no longer decide what
//! ships. A gene can now be `helps` and off (its prior reading was against
//! it) or `hurts` and on (two positive win columns since).
//!
//! The verdict block at the end of `genes.rs` is **generated** by
//! `tools/genes.py` from `gene_screen --analyze --json` outputs and
//! mirrored in `docs/gene_ledger.json`; a test holds the generated file and
//! the JSON together, and another re-derives every `default_on` from the two
//! columns beside it. The verdict rules live in the tool and are repeated
//! here so the reader of either side finds them:
//!
//! - `helps`: win z ≥ 2 with share z > −2, or share
//!   z ≥ 2 with win z > −2 (the screen's own `*` flag; `**` past the
//!   family-wise bar is recorded as strength, not required — with sixty-odd
//!   genes the family-wise bar would leave three on).
//! - `hurts`: the mirror image.
//! - `unresolved`: everything else, including a gene whose two axes
//!   disagree past |z| ≥ 2 and a gene the screens have not measured.
//!
//! The verdict is read off the newest screen that priced the gene, and it does
//! not reach the default: the win columns decide that.
//!
//! `apply_gene_ledger` is what `enable_live_bridge` and
//! `enable_engine_repairs` end with: every live treatment and production
//! treatment the ledger does not default on is withheld, every opt-in it
//! defaults on is enabled, and a gene with no ledger row (the
//! Firaxis-only flags, which the screen cannot price) is left exactly as
//! the bundle set it. The `_universe` twins of those two helpers set every
//! flag and skip the ledger: they are the genome's universe, for
//! `gene_screen` (which sets each gene to its drawn state explicitly) and for
//! the membership tests.

use super::AdvancedAi;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Helps,
    Hurts,
    Unresolved,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Helps => "helps",
            Verdict::Hurts => "hurts",
            Verdict::Unresolved => "unresolved",
        }
    }
}

/// The screen's measurement of one gene, as it printed it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    /// Complete pairs (seat pairs in an all-seats file) the estimate rests on.
    pub pairs: usize,
    pub win_delta_pp: f64,
    pub win_z: f64,
    pub share_delta_pp: f64,
    pub share_z: f64,
    /// The rows file the number was read from.
    pub source: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeneVerdict {
    pub tag: &'static str,
    pub verdict: Verdict,
    /// Whether the gene is on in the deployment genome — the win-column rule
    /// in the module header, decided by `tools/genes.py` and checked
    /// here against the figures below by `the_default_follows_the_ledgers_authority`.
    pub default_on: bool,
    /// ± wins per 10,000 on-arm seats at the gene's measured on-rate in the latest
    /// screen that priced it: `HEURISTIC_GENE_RANKING.md`'s
    /// *± Wins / 10k seats*. `None` when no screen has priced it.
    pub wins_last_10k: Option<i32>,
    /// The same figure from the screen before that — *± Wins / 10k seats prior*.
    /// `None` when the gene has only one reading.
    ///
    /// ⭐ THERE IS A THIRD WINDOW, AND IT IS DELIBERATELY NOT HERE. The ledger
    /// JSON and `HEURISTIC_GENE_RANKING.md` carry `wins_third_10k`, the screen
    /// before this one, so a reader can see whether the two readings the rule
    /// stands on are a trend or a bounce (operator request 2026-08-23). It
    /// decides nothing, and this table exists to re-derive what the rule
    /// decides from exactly the figures it reads — carrying a column no
    /// authority consults would invite a later rule to read it by accident.
    pub wins_prior_10k: Option<i32>,
    /// `HEURISTIC_GENE_RANKING.md`'s *Diff*: the pooled on win rate minus the
    /// pooled off win rate in percentage points, over **every** screen that
    /// priced the gene, each weighted by its on-arm seats. Negative vetoes the
    /// default. `None` when no screen has priced it.
    pub win_diff_pp: Option<f64>,
    /// The precision-weighted pooled effect, in wins per 10,000 on-arm seats:
    /// a random-effects (DerSimonian–Laird) inverse-variance pool of every
    /// screen's on−off difference on the win column's scale, with the
    /// between-screen disagreement carried in the error rather than assumed
    /// away. `None` when no screen has priced the gene.
    pub posterior_pp: Option<f64>,
    /// That pooled effect's standard error, same units. A 95% interval is
    /// `posterior_pp ± 1.96 × posterior_se_pp`.
    pub posterior_se_pp: Option<f64>,
    /// Past the family-wise bar of the screen that supplied the verdict.
    pub family_wise: bool,
    /// ⭐ A VERSIONED GENE'S RUNNER-UP. `war-economy-2` is screened beside
    /// `war-economy` as a gene of its own; the deployment genome carries at
    /// most one version of a family, the best of those the rule would turn
    /// on (`tools/genes.py::choose_family_heads`). A version the rule
    /// passes that is not its family's head is recorded here and ships off,
    /// which is the one case `default_on` is not the rule's own answer.
    pub family_runner_up: bool,
    /// The newest screen's paired contrast for this gene.
    pub screen: Option<Measure>,
}

/// A gene's columns clear the deployment rule: the win-column clause below,
/// vetoed by a negative pooled on-off difference. The mirror of
/// `tools/genes.py`'s `default_from_columns`, so a hand-edited table
/// cannot quietly ship a gene the rule does not.
///
/// The veto is one-way, and it is the one clause that lets a screen older than
/// the last two speak: a gene whose whole record is negative ships off however
/// its two newest columns read, while a positive record promotes nothing on its
/// own. An unpriced gene has no difference and is decided on the columns.
pub fn columns_default_on(last: Option<i32>, prior: Option<i32>, diff_pp: Option<f64>) -> bool {
    /// No column reading ships a gene that has not won more than it lost over
    /// its whole record.
    const DIFF_FLOOR: f64 = 0.0;
    if diff_pp.is_some_and(|diff| diff < DIFF_FLOOR) {
        return false;
    }
    win_columns_default_on(last, prior)
}

/// A two-sided 95% interval.
const Z95: f64 = 1.959_963_984_540_054;

/// What the precision-weighted pooled estimate says on its own: `on` where its
/// 95% interval lies wholly above zero, `off` where wholly below, and
/// `Unresolved` where it straddles. The mirror of
/// `tools/genes.py`'s `posterior_call`.
pub fn posterior_call(effect: Option<f64>, se: Option<f64>) -> Verdict {
    let (Some(effect), Some(se)) = (effect, se) else {
        return Verdict::Unresolved;
    };
    if se <= 0.0 {
        return Verdict::Unresolved;
    }
    if effect - Z95 * se > 0.0 {
        return Verdict::Helps;
    }
    if effect + Z95 * se < 0.0 {
        return Verdict::Hurts;
    }
    Verdict::Unresolved
}

/// The posterior authority's deployment call: it decides where its interval
/// excludes zero and **defers to `fallback`** — the threshold rule's own call —
/// where it straddles, rather than churning the genome on noise. The mirror of
/// `tools/genes.py`'s `default_from_posterior`.
pub fn posterior_default_on(effect: Option<f64>, se: Option<f64>, fallback: bool) -> bool {
    match posterior_call(effect, se) {
        Verdict::Helps => true,
        Verdict::Hurts => false,
        Verdict::Unresolved => fallback,
    }
}

/// The win-column clause vetoed only by a **resolved** negative record — the
/// posterior's 95% interval wholly below zero — rather than by the bare sign
/// of a pooled difference that carries no error at all. The mirror of
/// `tools/genes.py`'s `default_from_resolved_veto`.
pub fn resolved_veto_default_on(
    last: Option<i32>,
    prior: Option<i32>,
    effect: Option<f64>,
    se: Option<f64>,
) -> bool {
    if posterior_call(effect, se) == Verdict::Hurts {
        return false;
    }
    win_columns_default_on(last, prior)
}

/// `default_on` under whichever rule the ledger records as its authority:
/// `"columns"` for the operator's threshold rule as it ships, `"posterior-veto"`
/// for that rule with an error bar on its veto, `"posterior"` for the
/// precision-weighted pool deciding wherever its interval excludes zero. The
/// switch is `AUTHORITY` in `tools/genes.py`; the generated table carries
/// the answer it was written under, and
/// `the_default_follows_the_ledgers_authority` re-derives every row under it.
pub fn deployment_default_on(
    authority: &str,
    last: Option<i32>,
    prior: Option<i32>,
    diff_pp: Option<f64>,
    effect: Option<f64>,
    se: Option<f64>,
) -> bool {
    if authority == "columns" {
        return columns_default_on(last, prior, diff_pp);
    }
    let resolved = resolved_veto_default_on(last, prior, effect, se);
    match authority {
        "posterior-veto" => resolved,
        "posterior" => posterior_default_on(effect, se, resolved),
        // An authority this build does not know must never silently invent a
        // genome: fall back to the rule that ships.
        _ => columns_default_on(last, prior, diff_pp),
    }
}

/// A gene's win columns clear the deployment rule's column clause: one
/// populated column above `SINGLE_COLUMN_BAR`, both positive, or an average
/// above `AVERAGE_BAR` with neither column below `COLUMN_FLOOR`. No populated
/// columns means off — the mirror of `tools/genes.py`'s
/// `default_from_win_columns`. This is the clause alone; `columns_default_on`
/// is the deployment call.
pub fn win_columns_default_on(last: Option<i32>, prior: Option<i32>) -> bool {
    /// A sole provisional reading must strictly clear this bar.
    const SINGLE_COLUMN_BAR: i32 = 20;
    /// Wins per ten thousand games the two-column average must clear.
    const AVERAGE_BAR: f64 = 15.0;
    /// No column may sit below this, however good the other one is.
    const COLUMN_FLOOR: i32 = -10;
    let (last, prior) = match (last, prior) {
        (Some(value), None) | (None, Some(value)) => return value > SINGLE_COLUMN_BAR,
        (Some(last), Some(prior)) => (last, prior),
        (None, None) => return false,
    };
    if last > 0 && prior > 0 {
        return true;
    }
    f64::from(last + prior) / 2.0 > AVERAGE_BAR && last >= COLUMN_FLOOR && prior >= COLUMN_FLOOR
}

/// The verdicts: the generated block at the end of `genes.rs`, written by
/// `python3 tools/genes.py write` under the rows it judges.
mod table {
    pub(super) use super::super::genes::{LEDGER_AUTHORITY as AUTHORITY, VERDICTS as ROWS};
}

/// Every gene the ledger has a verdict for, in the generated order.
pub fn gene_ledger() -> &'static [GeneVerdict] {
    table::ROWS
}

/// Which rule decided every `default_on` in the generated table — `columns`,
/// `posterior-veto` or `posterior`. Written by `tools/genes.py` from its
/// `AUTHORITY` constant, so anything reporting the deployment genome can say
/// what decided it rather than assuming.
pub fn ledger_authority() -> &'static str {
    table::AUTHORITY
}

/// The ledger's row for a published tag, if the screens have measured it.
pub fn ledger_verdict(tag: &str) -> Option<&'static GeneVerdict> {
    table::ROWS.iter().find(|row| row.tag == tag)
}

/// Whether the screen can price a tag at all: the engine repairs, the
/// production genes and the opt-ins — `gene_screen`'s own universe. A
/// host-only flag is not here, and the ledger has nothing to say about it.
pub fn screenable(tag: &str) -> bool {
    super::gene(tag).is_some_and(|gene| gene.screenable())
}

/// Whether a gene is on in the deployment genome: the ledger's own
/// `default_on`, which is the win-column rule in the module header — so a
/// screenable gene no screen has measured is off; one measured once follows
/// the provisional single-column bar.
/// `None` for a gene the screen cannot price (the Firaxis-only flags),
/// which the bundle leaves as it set it.
pub fn ledger_default_on(tag: &str) -> Option<bool> {
    // A host-only flag is never governed by a screen row — even when one
    // exists: such a row measured a native stand-in that no longer runs
    // (`step-and-reassess`, 2026-08-21) and must not govern the bridge.
    if !screenable(tag) {
        return None;
    }
    Some(ledger_verdict(tag).is_some_and(|row| row.default_on))
}

/// Whether a live treatment is normally present in the universe but held out
/// of deployment by the ledger. These are the only live genes an explicit
/// verification arm may force on: host-only rows already ship as the universe
/// set them, while production opt-ins are not part of that universe at all.
pub fn ledger_held_live_treatment(tag: &str) -> bool {
    ledger_default_on(tag) == Some(false) && super::gene(tag).is_some_and(|gene| gene.live())
}

/// Every live treatment an explicit ledger-override arm may restore, in
/// registry order. This is deliberately narrower than every default-off gene:
/// the caller begins from the live universe, so only its withheld rows can be
/// restored without silently enabling a different bundle.
pub fn ledger_held_live_treatments() -> Vec<&'static str> {
    super::GENES
        .iter()
        .filter(|gene| gene.live())
        .map(|gene| gene.tag)
        .filter(|tag| ledger_held_live_treatment(tag))
        .collect()
}

/// The treatments a live arm actually plays: every live treatment the ledger
/// does not hold off, plus any explicitly forced ledger-held live treatment,
/// and every opt-in the ledger turns on. This is what the live seat's `genome`
/// event reports — the list that used to be `LIVE_BRIDGE_TREATMENTS` whole,
/// which the ledger makes untrue.
pub fn deployment_treatments_with_forced_live(forced_on: &[&str]) -> Vec<&'static str> {
    let mut tags: Vec<&'static str> = super::GENES
        .iter()
        .filter(|gene| gene.live())
        .map(|gene| gene.tag)
        .filter(|tag| {
            ledger_default_on(tag) != Some(false)
                || (ledger_held_live_treatment(tag) && forced_on.contains(tag))
        })
        .collect();
    for gene in super::GENES.iter().filter(|gene| gene.opt_in()) {
        if ledger_default_on(gene.tag) == Some(true) && !tags.contains(&gene.tag) {
            tags.push(gene.tag);
        }
    }
    tags
}

/// The unmodified deployment genome. Kept as a wrapper so every ordinary
/// controller continues to take the exact existing path.
pub fn deployment_treatments() -> Vec<&'static str> {
    deployment_treatments_with_forced_live(&[])
}

/// What `apply_gene_ledger` did, for the decide note and the tests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GeneLedgerApplied {
    /// Live or production treatments the bundle had on and the ledger holds off.
    pub withheld: Vec<&'static str>,
    /// Opt-ins the ledger turned on.
    pub enabled: Vec<&'static str>,
    /// Ledger-held live treatments an explicit verification arm restored.
    pub forced: Vec<&'static str>,
}

impl AdvancedAi {
    /// Bring a bundle to the deployment genome. See the module header.
    pub fn apply_gene_ledger(&mut self) -> GeneLedgerApplied {
        self.apply_gene_ledger_with_forced_live(&[])
    }

    /// Bring a live universe to the deployment genome while restoring only
    /// named ledger-held live treatments for a deliberately labeled
    /// verification arm. Callers must validate names before this point; an
    /// unrecognized, host-only, or already-deployed tag is ignored here rather
    /// than becoming a back door around the ledger.
    pub fn apply_gene_ledger_with_forced_live(&mut self, forced_on: &[&str]) -> GeneLedgerApplied {
        let mut applied = GeneLedgerApplied::default();
        // What the bundle had on — every live gene and the production genes —
        // the ledger may hold off; what it had off — the opt-ins — the ledger
        // may turn on. `joint-tactics` is both live and an opt-in: as a live
        // gene it is withheld or kept like any other.
        for gene in super::GENES
            .iter()
            .filter(|gene| gene.live() || gene.production())
        {
            let tag = gene.tag;
            if ledger_held_live_treatment(tag) && forced_on.contains(&tag) {
                applied.forced.push(tag);
            } else if ledger_default_on(tag) == Some(false) {
                (gene.disable)(self);
                applied.withheld.push(tag);
            }
        }
        for gene in super::GENES.iter().filter(|gene| gene.opt_in()) {
            if ledger_default_on(gene.tag) == Some(true) {
                (gene.enable)(self);
                applied.enabled.push(gene.tag);
            }
        }
        applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generated table and `docs/gene_ledger.json` are two writings of
    /// one measurement; `tools/genes.py write` produces both.
    #[test]
    fn the_generated_table_matches_the_json_ledger() {
        let json: serde_json::Value =
            serde_json::from_str(include_str!("../../../docs/gene_ledger.json"))
                .expect("docs/gene_ledger.json parses");
        assert_eq!(
            json["rules"]["authority"].as_str(),
            Some(table::AUTHORITY),
            "the JSON ledger and the generated table were written under different authorities"
        );
        let genes = json["genes"].as_array().expect("genes array");
        assert_eq!(
            genes.len(),
            gene_ledger().len(),
            "the Rust table and the JSON ledger hold different gene counts; run \
             `python3 tools/genes.py write`"
        );
        for entry in genes {
            let tag = entry["tag"].as_str().expect("tag");
            let row = ledger_verdict(tag)
                .unwrap_or_else(|| panic!("{tag} is in the JSON ledger but not the table"));
            assert_eq!(
                row.verdict.as_str(),
                entry["verdict"].as_str().expect("verdict"),
                "{tag}: verdict differs between the table and the JSON"
            );
            assert_eq!(
                row.default_on,
                entry["default_on"].as_bool().expect("default_on"),
                "{tag}: default differs between the table and the JSON"
            );
            for (column, recorded) in [
                ("wins_last_10k", row.wins_last_10k),
                ("wins_prior_10k", row.wins_prior_10k),
            ] {
                assert_eq!(
                    recorded.map(i64::from),
                    entry[column].as_i64(),
                    "{tag}: {column} differs between the table and the JSON"
                );
            }
            for (column, recorded) in [
                ("win_diff_pp", row.win_diff_pp),
                ("posterior_pp", row.posterior_pp),
                ("posterior_se_pp", row.posterior_se_pp),
            ] {
                assert_eq!(
                    recorded,
                    entry[column].as_f64(),
                    "{tag}: {column} differs between the table and the JSON"
                );
            }
        }
    }

    /// The rule itself: every default in the generated table is the one the
    /// ledger's recorded authority produces from the figures beside it. The
    /// tool decides; this re-derives, so a hand-edited table cannot quietly
    /// ship a gene no rule does.
    #[test]
    fn the_default_follows_the_ledgers_authority() {
        let mut on = 0;
        for row in gene_ledger() {
            assert_eq!(
                row.default_on,
                deployment_default_on(
                    table::AUTHORITY,
                    row.wins_last_10k,
                    row.wins_prior_10k,
                    row.win_diff_pp,
                    row.posterior_pp,
                    row.posterior_se_pp,
                ) && !row.family_runner_up,
                "{}: default {} does not follow the `{}` authority on columns {:?}/{:?}, \
                 difference {:?} and posterior {:?} ± {:?}",
                row.tag,
                row.default_on,
                table::AUTHORITY,
                row.wins_last_10k,
                row.wins_prior_10k,
                row.win_diff_pp,
                row.posterior_pp,
                row.posterior_se_pp
            );
            on += usize::from(row.default_on);
        }
        assert!(on > 0, "a genome with no gene on is a broken regeneration");
    }

    /// ⚠ THE SWITCH IS NOT THROWN. The shipped genome is the operator's
    /// threshold rule, and the posterior is published beside it. Flipping
    /// `AUTHORITY` in `tools/genes.py` and regenerating is the whole
    /// change; this fails first if it happens without the decision being
    /// taken deliberately.
    #[test]
    fn the_threshold_rule_is_still_the_authority() {
        assert_eq!(
            ledger_authority(),
            "columns",
            "the ledger was regenerated under a different authority: that re-decides the \
             deployment genome and is the operator's call, not a regeneration's"
        );
        for row in gene_ledger() {
            assert_eq!(
                row.default_on,
                columns_default_on(row.wins_last_10k, row.wins_prior_10k, row.win_diff_pp),
                "{}: the published default is not the threshold rule's",
                row.tag
            );
        }
    }

    /// The posterior's three states, at the boundary of each, and the one-way
    /// deferral that keeps a straddling interval from churning the genome.
    #[test]
    fn the_posterior_decides_only_where_its_interval_excludes_zero() {
        // 20 ± 1.96·10 = [0.4, 39.6] — wholly above zero.
        assert_eq!(posterior_call(Some(20.0), Some(10.0)), Verdict::Helps);
        assert!(posterior_default_on(Some(20.0), Some(10.0), false));
        assert_eq!(posterior_call(Some(-20.0), Some(10.0)), Verdict::Hurts);
        assert!(!posterior_default_on(Some(-20.0), Some(10.0), true));
        // 19 ± 1.96·10 straddles: the incumbent call stands, either way.
        assert_eq!(posterior_call(Some(19.0), Some(10.0)), Verdict::Unresolved);
        assert!(posterior_default_on(Some(19.0), Some(10.0), true));
        assert!(!posterior_default_on(Some(19.0), Some(10.0), false));
        // An unpriced gene has no posterior at all.
        assert_eq!(posterior_call(None, None), Verdict::Unresolved);
        assert!(!posterior_default_on(None, None, false));
        // And the dispatcher: `columns` never reads the posterior, and an
        // unknown authority falls back to the rule that ships.
        assert!(!deployment_default_on(
            "columns",
            Some(1),
            Some(-5),
            Some(1.0),
            Some(200.0),
            Some(1.0)
        ));
        assert!(deployment_default_on(
            "posterior",
            Some(1),
            Some(-5),
            Some(1.0),
            Some(200.0),
            Some(1.0)
        ));
        assert!(!deployment_default_on(
            "no-such-authority",
            Some(1),
            Some(-5),
            Some(1.0),
            Some(200.0),
            Some(1.0)
        ));
    }

    /// The veto with an error bar. `war-economy` is the live case: two
    /// positive columns removed by a record of −0.78 pp that no screen in the
    /// ledger can tell from zero.
    #[test]
    fn the_resolved_veto_fires_only_on_a_resolved_negative_record() {
        // The shipped veto removes it on the sign alone.
        assert!(!columns_default_on(Some(38), Some(8), Some(-0.78)));
        // Its pooled record is −48 ± 70: nowhere near resolved, so the
        // columns decide as they always did.
        assert!(resolved_veto_default_on(
            Some(38),
            Some(8),
            Some(-48.4),
            Some(69.7)
        ));
        // A record that IS resolved negative still vetoes.
        assert!(!resolved_veto_default_on(
            Some(38),
            Some(8),
            Some(-86.5),
            Some(18.6)
        ));
        // And it is strictly weaker than the shipped veto: it can only
        // re-admit genes the columns already like, never promote one.
        assert!(!resolved_veto_default_on(
            Some(-5),
            Some(-11),
            Some(200.0),
            Some(1.0)
        ));
    }

    /// The veto, at its boundary and in its one direction.
    #[test]
    fn a_negative_pooled_difference_vetoes_the_columns() {
        assert!(
            columns_default_on(Some(78), None, Some(1.56)),
            "columns that clear the bar, on a positive record"
        );
        assert!(
            !columns_default_on(Some(78), None, Some(-0.01)),
            "the strongest single column does not survive a negative record"
        );
        assert!(
            !columns_default_on(Some(38), Some(8), Some(-0.78)),
            "two positive columns do not survive a negative record"
        );
        assert!(
            columns_default_on(Some(38), Some(8), Some(0.0)),
            "a record of exactly zero is not negative"
        );
        assert!(
            !columns_default_on(Some(-5), Some(-11), Some(1.20)),
            "a positive record promotes nothing: the columns still decide"
        );
        assert!(
            !columns_default_on(None, None, None),
            "unmeasured has no record and no columns"
        );
    }

    /// The column clause's three branches, at their boundaries.
    #[test]
    fn the_win_column_rule_reads_both_columns() {
        assert!(win_columns_default_on(Some(1), Some(1)), "both positive");
        assert!(
            !win_columns_default_on(Some(1), Some(0)),
            "zero is not positive"
        );
        assert!(
            !win_columns_default_on(Some(39), Some(-26)),
            "one strong reading does not carry an average of 6.5"
        );
        assert!(
            win_columns_default_on(Some(48), Some(-10)),
            "an average of 19 with the floor exactly met"
        );
        assert!(
            !win_columns_default_on(Some(50), Some(-11)),
            "a column below the floor is off however good the average"
        );
        assert!(
            !win_columns_default_on(Some(30), Some(0)),
            "an average of exactly 15 does not clear +15, and 0 is not positive"
        );
        assert!(
            win_columns_default_on(Some(21), None),
            "one reading above +20"
        );
        assert!(
            win_columns_default_on(None, Some(21)),
            "either column may be populated"
        );
        assert!(
            !win_columns_default_on(Some(20), None),
            "one reading at +20 does not clear the strict bar"
        );
        assert!(!win_columns_default_on(None, None), "unmeasured is off");
    }

    /// A screenable gene the screens have not measured is not proven, so it
    /// is off; a Firaxis-only flag has no instrument and is left alone.
    #[test]
    fn unmeasured_genes_are_off_only_when_a_screen_could_have_priced_them() {
        assert_eq!(
            ledger_default_on("live-trader-route"),
            None,
            "Firaxis-only: untouched"
        );
        assert!(!screenable("live-trader-route"));
        // A host-only flag with a row from its retired native stand-in.
        assert!(ledger_verdict("step-and-reassess").is_some());
        assert_eq!(
            ledger_default_on("step-and-reassess"),
            None,
            "a host-only flag is never governed by a screen row"
        );
        for repair in super::super::genes::repair_tags() {
            assert!(screenable(repair));
            assert!(
                ledger_default_on(repair).is_some(),
                "{repair}: a screenable gene always has a default"
            );
        }
        assert_eq!(ledger_default_on("no-such-gene"), None);
    }

    /// Every ledger tag is a gene the repository knows, so a renamed
    /// treatment cannot leave a stale verdict governing nothing.
    #[test]
    fn every_ledger_tag_names_a_known_gene() {
        for row in gene_ledger() {
            assert!(
                super::super::gene(row.tag).is_some(),
                "ledger row {} names no gene in the registry",
                row.tag
            );
        }
    }

    /// The deployment genome is the universe minus the genes the ledger
    /// holds off, plus the opt-ins it turns on — and a gene the ledger has
    /// never measured is left as the bundle set it.
    #[test]
    fn apply_gene_ledger_withholds_what_is_not_proven_and_enables_proven_opt_ins() {
        let mut ai = AdvancedAi::new();
        ai.enable_live_bridge_universe();
        let applied = ai.apply_gene_ledger();
        for tag in super::super::GENES
            .iter()
            .filter(|gene| gene.live() || gene.production())
            .map(|gene| gene.tag)
        {
            match ledger_default_on(tag) {
                Some(false) => assert!(applied.withheld.contains(&tag), "{tag} should be withheld"),
                _ => assert!(!applied.withheld.contains(&tag), "{tag} should stand"),
            }
        }
        for tag in super::super::GENES
            .iter()
            .filter(|gene| gene.opt_in())
            .map(|gene| gene.tag)
        {
            assert_eq!(
                applied.enabled.contains(&tag),
                ledger_default_on(tag) == Some(true),
                "{tag}: an opt-in is enabled exactly when the ledger says it helps"
            );
        }
        // The published deployment list is the universe minus the withheld
        // plus the enabled opt-ins — the same arithmetic, from the tags.
        let deployed = deployment_treatments();
        for tag in &applied.withheld {
            assert!(
                !deployed.contains(tag),
                "{tag} is withheld yet listed as deployed"
            );
        }
        for tag in &applied.enabled {
            assert!(
                deployed.contains(tag),
                "{tag} is enabled yet not listed as deployed"
            );
        }
        for tag in super::super::genes::live_tags() {
            assert_eq!(
                deployed.contains(&tag),
                ledger_default_on(tag) != Some(false),
                "{tag}: deployed exactly unless the ledger holds it off"
            );
        }
    }

    #[test]
    fn a_live_arm_can_restore_only_a_named_ledger_held_gene() {
        let forced = ["governor-every-lane"];
        assert!(ledger_held_live_treatment("governor-every-lane"));
        assert!(
            !ledger_held_live_treatment("parallel-settlers"),
            "host-only treatments already follow their live-universe default"
        );
        assert!(
            !ledger_held_live_treatment("founder-temple"),
            "a withheld production opt-in is not a live-universe override"
        );
        assert!(ledger_held_live_treatments().contains(&"governor-every-lane"));

        let deployed = deployment_treatments();
        let forced_deployment = deployment_treatments_with_forced_live(&forced);
        assert!(
            !deployed.contains(&"governor-every-lane"),
            "the verification override must not change deployment"
        );
        assert!(
            forced_deployment.contains(&"governor-every-lane"),
            "the genome event must name the treatment the arm actually restored"
        );
        assert_eq!(
            forced_deployment.len(),
            deployed.len() + 1,
            "one explicit live gene restores exactly one deployment row"
        );

        let mut ai = AdvancedAi::new();
        ai.enable_live_bridge_universe();
        let applied = ai.apply_gene_ledger_with_forced_live(&forced);
        assert!(ai.governor_victory_lanes, "the named live treatment stands");
        assert!(
            !ai.war_patience,
            "another ledger-held treatment stays off unless named too"
        );
        assert_eq!(applied.forced, vec!["governor-every-lane"]);
        assert!(
            !applied.withheld.contains(&"governor-every-lane"),
            "an explicit arm cannot report its restored gene as withheld"
        );
    }
}
